//! The finish forecast (phase 18 P4, step 4b): how likely each task and the project are to
//! finish by a date, from the effort ranges of `Task::work_low`/`work_high`.
//!
//! The project is scheduled and leveled once with today's effort. Each person's tasks then
//! keep the order leveling gave them, as hidden finish-to-start links between tasks of
//! theirs that follow one another, so a run never puts two of them at once without
//! leveling again (which would cost about 60 ms a run at 5,000 tasks). Each run samples
//! every ranged task's effort from a BetaPERT over low, likely (today's effort, clamped to
//! the range) and high, scales its duration by the ratio (only the part left for started
//! work), and reruns the date pass on the prepared graph. Correlation is a Gaussian copula:
//! a draw shared by every task, one shared by the tasks with the same value of the chosen
//! field, and one of the task's own. A read: nothing goes into the command log, and the
//! same project gives the same numbers on any number of threads, since every run has its
//! own seed.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;

use crate::error::EngineError;
use crate::fields::FieldValue;
use crate::graph::Graph;
use crate::level::level_project;
use crate::model::{DepKind, Dependency, Project, TaskType};
use crate::schedule::{background_tasks, prepare, schedule, schedule_dates};
use crate::split::Split;
use crate::tracking::is_complete;

const DAY_MS: i64 = 86_400_000;
const MINUTE_MS: i64 = 60_000;
/// Fixed, so the same project gives the same forecast.
const SEED: u64 = 0x5EED_F0CA_57A7_0001;
/// Points of each BetaPERT quantile table, and of the grid its CDF is integrated on.
const QUANTILES: usize = 257;
const GRID: usize = 1024;
/// The most of a task's variation shared with others, so its own part never vanishes.
const MAX_SHARED: f64 = 0.9;
/// The most runs one forecast makes; every run keeps each task's finish until the end
/// (4 bytes a task a run), so this bounds the memory.
pub const MAX_ITERATIONS: u32 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Percentiles {
    pub p50: i64,
    pub p80: i64,
    pub p90: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TaskForecast {
    pub uid: u32,
    /// Finish dates (project wall-clock ms) by which this share of the runs had finished.
    pub p50: i64,
    pub p80: i64,
    pub p90: i64,
    /// Share of the runs finished by the task's deadline; none without one.
    pub on_time: Option<f64>,
    /// Share of the runs in which the task was critical.
    pub criticality: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Forecast {
    pub iterations: u32,
    /// The project finish as scheduled now.
    pub planned_finish: i64,
    /// The project finish once leveled, with today's effort and no variation: what the
    /// runs vary around. Later than `planned_finish` when people are overallocated.
    pub leveled_finish: i64,
    /// The project finish over the runs.
    pub finish: Percentiles,
    /// Runs finishing the project on each day (midnight of that day), in date order.
    pub histogram: Vec<(i64, u32)>,
    /// Every task, in project order.
    pub tasks: Vec<TaskForecast>,
}

/// Forecast on as many threads as the machine has.
pub fn forecast(project: &Project, iterations: u32, cancel: &AtomicBool) -> Result<Forecast, EngineError> {
    let threads = std::thread::available_parallelism().map_or(1, |count| count.get());
    forecast_with_threads(project, iterations, threads, cancel)
}

/// Forecast on `threads` threads; the result does not depend on how many.
pub fn forecast_with_threads(
    project: &Project,
    iterations: u32,
    threads: usize,
    cancel: &AtomicBool,
) -> Result<Forecast, EngineError> {
    let iterations = iterations.clamp(1, MAX_ITERATIONS);
    let stopped = || if cancel.load(Ordering::Relaxed) { Err(EngineError::Cancelled) } else { Ok(()) };
    stopped()?;
    let (base, graph) = frozen_base(project)?;
    stopped()?;
    let model = Model::new(&base);
    let tasks = base.tasks.len();
    let background = background_tasks(&base);
    let origin = base.start;
    let leveled_finish = {
        let mut reference = base.clone();
        schedule_dates(&mut reference, &graph);
        project_finish_of(&reference, &background)
    };

    let runs = iterations as usize;
    let threads = threads.clamp(1, runs);
    let block = runs.div_ceil(threads);
    let outcomes: Vec<Result<Block, EngineError>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|thread| {
                let range = (thread * block).min(runs)..((thread + 1) * block).min(runs);
                let (base, graph, model, background) = (&base, &graph, &model, &background);
                scope.spawn(move || run_block(base, graph, model, background, origin, range, cancel))
            })
            .collect();
        handles.into_iter().map(|handle| handle.join().expect("forecast thread")).collect()
    });

    let mut finishes: Vec<i32> = Vec::with_capacity(runs * tasks);
    let mut critical = vec![0u32; tasks];
    let mut project_finish: Vec<i64> = Vec::with_capacity(runs);
    for outcome in outcomes {
        let block = outcome?;
        finishes.extend(block.finishes);
        project_finish.extend(block.project_finish);
        for (total, count) in critical.iter_mut().zip(block.critical) {
            *total += count;
        }
    }

    let at = |minutes: i32| origin + i64::from(minutes) * MINUTE_MS;
    let mut column = vec![0i32; runs];
    let results = base
        .tasks
        .iter()
        .enumerate()
        .map(|(index, task)| {
            for (run, slot) in column.iter_mut().enumerate() {
                *slot = finishes[run * tasks + index];
            }
            column.sort_unstable();
            let deadline = task.deadline;
            TaskForecast {
                uid: task.uid,
                p50: at(percentile(&column, 0.5)),
                p80: at(percentile(&column, 0.8)),
                p90: at(percentile(&column, 0.9)),
                on_time: deadline.map(|date| column.iter().filter(|&&minutes| at(minutes) <= date).count() as f64 / runs as f64),
                criticality: f64::from(critical[index]) / runs as f64,
            }
        })
        .collect();

    project_finish.sort_unstable();
    let mut days: BTreeMap<i64, u32> = BTreeMap::new();
    for finish in &project_finish {
        *days.entry(finish.div_euclid(DAY_MS) * DAY_MS).or_default() += 1;
    }
    Ok(Forecast {
        iterations,
        planned_finish: project_finish_of(project, &background_tasks(project)),
        leveled_finish,
        finish: Percentiles {
            p50: percentile(&project_finish, 0.5),
            p80: percentile(&project_finish, 0.8),
            p90: percentile(&project_finish, 0.9),
        },
        histogram: days.into_iter().collect(),
        tasks: results,
    })
}

/// The value at or below which `share` of the sorted `values` lie (nearest rank).
fn percentile<T: Copy>(sorted: &[T], share: f64) -> T {
    let rank = ((share * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
    sorted[rank - 1]
}

fn project_finish_of(project: &Project, background: &[bool]) -> i64 {
    project
        .tasks
        .iter()
        .zip(background)
        .filter(|(task, &background)| !task.summary && !background)
        .map(|(task, _)| task.finish)
        .max()
        .unwrap_or(project.start)
}

/// The project scheduled and, when anyone is overallocated, leveled, with each person's
/// order made into hidden links, and its graph.
fn frozen_base(project: &Project) -> Result<(Project, Graph), EngineError> {
    let mut base = project.clone();
    schedule(&mut base)?;
    if !base.overallocations.is_empty() {
        level_project(&mut base)?;
    }
    let plain = base.clone();
    add_order_links(&mut base);
    match prepare(&mut base) {
        Ok(graph) => Ok((base, graph)),
        // Unusual links (start-to-start, finish-to-finish with negative lags) can make the
        // order loop; the forecast then goes by the links alone.
        Err(EngineError::DependencyCycle) => {
            let mut base = plain;
            let graph = prepare(&mut base)?;
            Ok((base, graph))
        }
        Err(other) => Err(other),
    }
}

/// Hidden finish-to-start links that keep each person's order: every open leaf task of
/// theirs waits for the one of their earlier tasks that finishes last while still before
/// it starts (part-time work side by side stays side by side, and a task leveling put
/// after a long half-time one waits for that one, not for a short task beside it). A task
/// that gets such a link drops its leveling delay and leveling's pauses, which the link
/// now stands for; a task without one keeps them.
fn add_order_links(project: &mut Project) {
    let position: HashMap<u32, usize> = project.tasks.iter().enumerate().map(|(index, task)| (task.uid, index)).collect();
    let mut by_person: HashMap<u32, Vec<usize>> = HashMap::new();
    for assignment in &project.assignments {
        let Some(&index) = position.get(&assignment.task_uid) else { continue };
        let task = &project.tasks[index];
        if task.summary || task.milestone || is_complete(task) {
            continue;
        }
        by_person.entry(assignment.resource_uid).or_default().push(index);
    }
    let mut people: Vec<_> = by_person.into_iter().collect();
    people.sort_unstable_by_key(|(person, _)| *person);
    let mut links: Vec<(usize, usize)> = Vec::new();
    for (_, mut tasks) in people {
        tasks.sort_unstable_by_key(|&index| {
            let task = &project.tasks[index];
            (task.start, task.finish, task.uid)
        });
        tasks.dedup();
        for (place, &after) in tasks.iter().enumerate() {
            let start = project.tasks[after].start;
            let before = tasks[..place]
                .iter()
                .copied()
                .filter(|&earlier| project.tasks[earlier].finish <= start)
                .max_by_key(|&earlier| (project.tasks[earlier].finish, project.tasks[earlier].uid));
            if let Some(before) = before {
                links.push((before, after));
            }
        }
    }
    links.sort_unstable();
    links.dedup();
    let mut linked = HashSet::new();
    for (from, to) in links {
        let uid = project.next_dependency_uid;
        project.next_dependency_uid += 1;
        let (predecessor_uid, successor_uid) = (project.tasks[from].uid, project.tasks[to].uid);
        project.dependencies.push(Dependency { uid, predecessor_uid, successor_uid, kind: DepKind::Fs, lag: 0 });
        linked.insert(to);
    }
    for index in linked {
        let task = &mut project.tasks[index];
        task.leveling_delay = 0;
        task.splits.retain(|split| !split.leveling);
    }
}

/// One ranged task: where its duration comes from and how its effort is drawn.
struct Sampled {
    index: usize,
    /// Fractions of the range at evenly spaced probabilities.
    table: Arc<Vec<f64>>,
    low: f64,
    high: f64,
    /// Today's effort (or duration, without people): the sample's ratio to it scales the
    /// duration. The BetaPERT's likely value is this clamped to the range.
    amount: f64,
    duration: i64,
    /// The user's pauses, as in the base: their offsets scale with the work.
    splits: Vec<Split>,
    remaining: i64,
    started: bool,
    group: Option<usize>,
    /// Weights of the shared, group and own draws.
    weights: (f64, f64, f64),
}

struct Model {
    sampled: Vec<Sampled>,
    groups: usize,
}

impl Model {
    fn new(project: &Project) -> Self {
        let settings = &project.forecast;
        let field = settings.field.filter(|id| project.fields.iter().any(|field| field.id == *id));
        let common = settings.common.share();
        let group_share = if field.is_some() { settings.group.share() } else { 0.0 };
        let assigned: HashSet<u32> = project.assignments.iter().map(|assignment| assignment.task_uid).collect();
        let mut groups: HashMap<String, usize> = HashMap::new();
        let mut tables: HashMap<(i64, i64), Arc<Vec<f64>>> = HashMap::new();
        let mut sampled = Vec::new();
        for (index, task) in project.tasks.iter().enumerate() {
            let (Some(low), Some(high)) = (task.work_low, task.work_high) else { continue };
            // A backwards range can come from a file the commands never checked.
            if task.summary || task.milestone || is_complete(task) || high <= 0 || low > high || low < 0 {
                continue;
            }
            // Effort moves the dates of a task only when its duration follows its work.
            let has_people = assigned.contains(&task.uid);
            if has_people && task.task_type == TaskType::FixedDuration {
                continue;
            }
            let amount = if has_people { task.work } else { task.duration };
            if amount <= 0 {
                continue;
            }
            let likely = amount.clamp(low, high) as f64;
            let (low, high) = (low as f64, high as f64);
            let table = if high > low {
                let alpha = 1.0 + 4.0 * (likely - low) / (high - low);
                let beta = 1.0 + 4.0 * (high - likely) / (high - low);
                let key = ((alpha * 100.0).round() as i64, (beta * 100.0).round() as i64);
                tables.entry(key).or_insert_with(|| Arc::new(beta_quantiles(alpha, beta))).clone()
            } else {
                Arc::new(vec![0.0; QUANTILES])
            };
            let group = field
                .and_then(|id| match task.values.get(&id) {
                    Some(FieldValue::Text(text)) if !text.trim().is_empty() => Some(text.trim().to_lowercase()),
                    _ => None,
                })
                .map(|key| {
                    let next = groups.len();
                    *groups.entry(key).or_insert(next)
                });
            let mut shared = common + if group.is_some() { group_share } else { 0.0 };
            let (mut c, mut g) = (common, if group.is_some() { group_share } else { 0.0 });
            if shared > MAX_SHARED {
                let scale = MAX_SHARED / shared;
                c *= scale;
                g *= scale;
                shared = MAX_SHARED;
            }
            let started = task.percent_complete > 0.0 || task.actual_start.is_some();
            sampled.push(Sampled {
                index,
                table,
                low,
                high,
                amount: amount as f64,
                duration: task.duration,
                splits: task.splits.clone(),
                remaining: task.remaining_duration,
                started,
                group,
                weights: (c.sqrt(), g.sqrt(), (1.0 - shared).sqrt()),
            });
        }
        Model { sampled, groups: groups.len() }
    }
}

struct Block {
    /// Finish of every task, in minutes from the project start, run by run.
    finishes: Vec<i32>,
    critical: Vec<u32>,
    project_finish: Vec<i64>,
}

fn run_block(
    base: &Project,
    graph: &Graph,
    model: &Model,
    background: &[bool],
    origin: i64,
    runs: std::ops::Range<usize>,
    cancel: &AtomicBool,
) -> Result<Block, EngineError> {
    let tasks = base.tasks.len();
    let mut project = base.clone();
    let mut block = Block {
        finishes: Vec::with_capacity(runs.len() * tasks),
        critical: vec![0; tasks],
        project_finish: Vec::with_capacity(runs.len()),
    };
    let mut group_draws = vec![0.0; model.groups];
    for run in runs {
        if cancel.load(Ordering::Relaxed) {
            return Err(EngineError::Cancelled);
        }
        // Every run starts from the base, so it does not depend on the run before it (or
        // on which thread made it).
        project.clone_from(base);
        let mut random = Random::for_run(run as u64);
        let shared = random.normal();
        for draw in &mut group_draws {
            *draw = random.normal();
        }
        for item in &model.sampled {
            let (c, g, own) = item.weights;
            let z = c * shared + item.group.map_or(0.0, |group| g * group_draws[group]) + own * random.normal();
            let effort = item.low + (item.high - item.low) * lookup(&item.table, phi(z));
            let ratio = effort / item.amount;
            let task = &mut project.tasks[item.index];
            // Work done stays; what is left (all of it before a start) scales.
            let done = if item.started { item.duration - item.remaining } else { 0 };
            let scale = |minutes: i64| done + ((minutes - done) as f64 * ratio).round() as i64;
            task.duration = scale(item.duration).max(done);
            task.remaining_duration = task.duration - done;
            if !item.splits.is_empty() {
                task.splits.clone_from(&item.splits);
                for split in &mut task.splits {
                    if split.offset > done {
                        split.offset = scale(split.offset);
                    }
                }
                // A pause pushed to the very end (or past it) no longer interrupts anything.
                let duration = task.duration;
                task.splits.retain(|split| split.offset > 0 && split.offset < duration);
            }
        }
        schedule_dates(&mut project, graph);
        for (index, task) in project.tasks.iter().enumerate() {
            let minutes = (task.finish - origin).div_euclid(MINUTE_MS);
            block.finishes.push(minutes.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32);
            if task.critical {
                block.critical[index] += 1;
            }
        }
        block.project_finish.push(project_finish_of(&project, background));
    }
    Ok(block)
}

/// The fraction of the range at probability `u`, interpolated in the table.
fn lookup(table: &[f64], u: f64) -> f64 {
    let position = u.clamp(0.0, 1.0) * (table.len() - 1) as f64;
    let below = position.floor() as usize;
    let above = (below + 1).min(table.len() - 1);
    let part = position - below as f64;
    table[below] + (table[above] - table[below]) * part
}

/// Quantiles of Beta(alpha, beta) on [0, 1] at `QUANTILES` evenly spaced probabilities,
/// from its density integrated on a grid (alpha and beta are at least 1, so the density
/// is bounded).
fn beta_quantiles(alpha: f64, beta: f64) -> Vec<f64> {
    let density = |x: f64| x.powf(alpha - 1.0) * (1.0 - x).powf(beta - 1.0);
    let step = 1.0 / (GRID - 1) as f64;
    let mut cdf = vec![0.0; GRID];
    let mut previous = density(0.0);
    for index in 1..GRID {
        let current = density(index as f64 * step);
        cdf[index] = cdf[index - 1] + (previous + current) * step / 2.0;
        previous = current;
    }
    let total = cdf[GRID - 1];
    let mut quantiles = Vec::with_capacity(QUANTILES);
    let mut cursor = 0;
    for point in 0..QUANTILES {
        let target = point as f64 / (QUANTILES - 1) as f64 * total;
        while cursor + 1 < GRID - 1 && cdf[cursor + 1] < target {
            cursor += 1;
        }
        let (from, to) = (cdf[cursor], cdf[cursor + 1]);
        let part = if to > from { ((target - from) / (to - from)).clamp(0.0, 1.0) } else { 0.0 };
        quantiles.push(((cursor as f64 + part) * step).clamp(0.0, 1.0));
    }
    quantiles
}

/// The standard normal distribution function (Abramowitz and Stegun 7.1.26 for erf, error
/// below 1.5e-7).
fn phi(z: f64) -> f64 {
    let x = z.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly = t * (0.254_829_592 + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let erf = 1.0 - poly * (-x * x).exp();
    if z >= 0.0 { 0.5 * (1.0 + erf) } else { 0.5 * (1.0 - erf) }
}

/// xoshiro256**, seeded per run through SplitMix64.
struct Random([u64; 4]);

impl Random {
    fn for_run(run: u64) -> Self {
        let mut state = SEED ^ run.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let mut next = || {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };
        Random([next(), next(), next(), next()])
    }

    fn next_u64(&mut self) -> u64 {
        let s = &mut self.0;
        let result = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// Uniform on (0, 1), never 0 or 1.
    fn uniform(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    /// Standard normal (Box–Muller, one value per pair of uniforms).
    fn normal(&mut self) -> f64 {
        let (u, v) = (self.uniform(), self.uniform());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beta_quantiles_match_the_uniform_and_a_symmetric_shape() {
        let uniform = beta_quantiles(1.0, 1.0);
        assert!((uniform[128] - 0.5).abs() < 1e-3);
        assert!((uniform[64] - 0.25).abs() < 1e-3);
        // BetaPERT with the likely value in the middle: alpha = beta = 3, median 0.5.
        let middle = beta_quantiles(3.0, 3.0);
        assert!((middle[128] - 0.5).abs() < 1e-3);
        assert!(middle[64] > 0.25, "less spread than uniform");
        assert!(middle.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn phi_and_the_generator_behave() {
        assert!((phi(0.0) - 0.5).abs() < 1e-7);
        assert!((phi(1.959_964) - 0.975).abs() < 1e-5);
        let mut random = Random::for_run(3);
        let draws: Vec<f64> = (0..20_000).map(|_| random.normal()).collect();
        let mean = draws.iter().sum::<f64>() / draws.len() as f64;
        let variance = draws.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / draws.len() as f64;
        assert!(mean.abs() < 0.03 && (variance - 1.0).abs() < 0.05, "{mean} {variance}");
        assert_eq!(Random::for_run(7).next_u64(), Random::for_run(7).next_u64());
    }
}
