//! Rescheduling one leaf and the work after it, for leveling.
//!
//! Leveling delays one task at a time, and only that task and the tasks its links reach
//! can move. Instead of a full forward pass after every delay, this recomputes those
//! leaves in leaf order with the same `schedule_leaf` and stops wherever dates come out
//! unchanged. Late dates, slack, and summary dates are left for one full
//! `schedule_dates` once leveling is done: leveling itself reads only leaf dates.

use std::collections::{BTreeSet, HashMap};

use crate::graph::Graph;
use crate::model::{ConstraintType, DepKind, Project};
use crate::schedule::{schedule_leaf, write_early, Dates};
use crate::split;

pub(crate) struct Ripple {
    /// Each leaf's place in the graph's leaf order, so queued leaves come out with
    /// their predecessors first.
    rank: Vec<usize>,
    /// Leaf early dates as the last pass left them.
    early: Dates,
    /// Summary finish pushes never happen here (see `Ripple::new`).
    no_shift: HashMap<u32, i64>,
}

impl Ripple {
    /// `None` when one forward pass does not settle the dates and leveling must
    /// reschedule the whole project after each delay: an ALAP task (placed from late
    /// dates), or a summary that must finish by a link or a date (its leaves are pushed
    /// over extra passes). Call it right after a full `schedule_dates`.
    pub(crate) fn new(project: &Project, graph: &Graph) -> Option<Self> {
        if !one_pass_settles(project, graph) {
            return None;
        }
        let mut rank = vec![usize::MAX; project.tasks.len()];
        for (place, &index) in graph.order().iter().enumerate() {
            rank[index] = place;
        }
        let early = project
            .tasks
            .iter()
            .map(|task| (!task.summary).then_some((task.early_start, task.early_finish)))
            .collect();
        Some(Self {
            rank,
            early,
            no_shift: HashMap::new(),
        })
    }

    /// Reschedule the leaf at `from` and every leaf its new dates move. Each leaf whose
    /// dates change gets the (start, finish) it had before its first change in `before`.
    pub(crate) fn run(
        &mut self,
        project: &mut Project,
        graph: &Graph,
        from: usize,
        before: &mut HashMap<usize, (i64, i64)>,
    ) {
        let mut queue: BTreeSet<(usize, usize)> = BTreeSet::new();
        queue.insert((self.rank[from], from));
        while let Some((_, index)) = queue.pop_first() {
            let (es, ef, conflict) =
                schedule_leaf(project, graph, index, &self.early, &self.no_shift);
            write_early(&mut project.tasks[index], es, ef, conflict);
            let old = self.early[index].replace((es, ef));
            if old == Some((es, ef)) {
                continue;
            }
            if let Some(old) = old {
                before.entry(index).or_insert(old);
            }
            if !project.tasks[index].splits.is_empty() {
                let segments = split::segments_of(project, &project.tasks[index]);
                project.tasks[index].segments = segments;
            }
            for leaf in moved_by(project, graph, index) {
                queue.insert((self.rank[leaf], leaf));
            }
        }
        #[cfg(test)]
        same_as_full_pass(project, graph);
    }
}

/// In tests, every quick reschedule must leave each leaf as a full pass would.
#[cfg(test)]
fn same_as_full_pass(project: &Project, graph: &Graph) {
    let mut full = project.clone();
    crate::schedule::schedule_dates(&mut full, graph);
    for (quick, full) in project.tasks.iter().zip(&full.tasks).filter(|(task, _)| !task.summary) {
        assert_eq!(
            (quick.start, quick.finish, quick.early_start, quick.early_finish, quick.conflict),
            (full.start, full.finish, full.early_start, full.early_finish, full.conflict),
            "task {} after a quick reschedule",
            quick.uid
        );
        assert_eq!(quick.segments, full.segments, "segments of task {}", quick.uid);
    }
}

/// True when no task is ALAP and no summary must finish by an FF/SF link or an
/// FNET/MFO date: then one forward pass gives the dates and late dates never feed back.
fn one_pass_settles(project: &Project, graph: &Graph) -> bool {
    project.tasks.iter().enumerate().all(|(index, task)| {
        if task.constraint_type == ConstraintType::Alap {
            return false;
        }
        if !task.summary {
            return true;
        }
        let finish_date = matches!(task.constraint_type, ConstraintType::Fnet | ConstraintType::Mfo)
            && task.constraint_date.is_some();
        let finish_link = graph.preds(index).iter().any(|&link| {
            matches!(project.dependencies[link].kind, DepKind::Ff | DepKind::Sf)
        });
        !finish_date && !finish_link
    })
}

/// Leaves whose bounds read the dates of the leaf at `index`: successors of the leaf and
/// of every summary above it (a summary's dates span its leaves), with a summary
/// successor standing for all its leaves.
fn moved_by(project: &Project, graph: &Graph, index: usize) -> Vec<usize> {
    let mut nodes = vec![index];
    let mut parent = project.tasks[index].parent_uid;
    while let Some(position) = parent.and_then(|uid| graph.position(uid)) {
        nodes.push(position);
        parent = project.tasks[position].parent_uid;
    }
    let mut leaves = Vec::new();
    for node in nodes {
        for &link in graph.succs(node) {
            let Some(successor) = graph.position(project.dependencies[link].successor_uid) else {
                continue;
            };
            if project.tasks[successor].summary {
                leaves.extend(graph.leaves_under(project, successor));
            } else {
                leaves.push(successor);
            }
        }
    }
    leaves
}

#[cfg(test)]
mod tests {
    use super::Ripple;
    use crate::calendar::TimeRange;
    use crate::command::{apply_one, Command};
    use crate::availability::AvailabilityPeriod;
    use crate::document::Document;
    use crate::fields::{FieldKind, FieldValue};
    use crate::level_order::{Criterion, SortKey};
    use crate::level::{level_with, CAME_ROUND, SEARCH_IN_FULL};
    use crate::model::{ConstraintType, DepKind, Project, ResourceKind};
    use crate::schedule::{prepare, schedule, schedule_dates};
    use crate::timeutil::civil;

    const HOUR: i64 = 60;
    const HOUR_MS: i64 = 3_600_000;
    const DAY_MS: i64 = 86_400_000;
    /// Projects per run: about twenty seconds in a debug build, most of it the check
    /// after every quick reschedule.
    const SEEDS: u64 = 150;

    /// A fixed linear congruential generator, so every run builds the same projects.
    struct Random(u64);

    impl Random {
        fn below(&mut self, n: u64) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) % n.max(1)
        }

        fn chance(&mut self, percent: u64) -> bool {
            self.below(100) < percent
        }

        fn pick<T: Copy>(&mut self, items: &[T]) -> T {
            items[self.below(items.len() as u64) as usize]
        }
    }

    /// Apply one command without scheduling, much quicker than `Document::apply` in a
    /// debug build. A command that fails, or leaves a link cycle, changes nothing.
    fn apply(project: &mut Project, command: Command) {
        let mut trial = project.clone();
        if apply_one(&mut trial, &command).is_ok() && prepare(&mut trial).is_ok() {
            *project = trial;
        }
    }

    /// A new calendar with `days` from Monday on and the rest of the week off.
    fn calendar(project: &mut Project, name: &str, mut days: Vec<Vec<TimeRange>>) -> Option<u32> {
        apply(project, Command::CreateCalendar { name: name.into(), base_id: None });
        let calendar_id = project.calendars.last()?.id;
        days.resize(7, Vec::new());
        apply(project, Command::SetCalendarWeek { calendar_id, week: days });
        Some(calendar_id)
    }

    /// What a generated project may hold beyond the common mix.
    #[derive(Clone, Copy, PartialEq)]
    enum Mix {
        /// Nothing that makes `Ripple::new` decline, so the quick path is tested.
        OnePass,
        /// FF and SF links into groups and FNET dates on groups: no ALAP, so a group
        /// that must finish later is what makes `Ripple::new` decline.
        GroupFinish,
        /// Many ALAP tasks, placed from late dates, and no group finish dates.
        Alap,
        /// ALAP tasks and finish dates on groups too.
        Anything,
    }

    /// A project with groups, links of every kind (to and from groups too), splits,
    /// constraints, started and manual tasks, other calendars, holidays, and resources
    /// with different units and calendars.
    fn project(seed: u64, mix: Mix) -> Project {
        let mut random = Random(seed);
        let mut project = Document::new_default().project;
        let tasks = 20 + random.below(40) as u32;
        for uid in 2..=tasks {
            let name = format!("Việc {uid}");
            apply(&mut project, Command::AddTask { name, after_uid: Some(uid - 1) });
        }
        for uid in 2..=tasks {
            if random.chance(25) {
                apply(&mut project, Command::Indent { uid });
            }
        }
        let start = project.start;
        // Most task calendars work inside the standard hours and resource calendars
        // around them, so most tasks can find room. A few tasks work the long days,
        // which a resource on the standard hours never fully covers: leveling gives up
        // on them once the search comes round to the same point of the week.
        let morning = vec![TimeRange::new(8 * 60, 12 * 60)];
        let mornings = calendar(&mut project, "Buổi sáng", vec![morning; 5]);
        let long_day = vec![TimeRange::new(7 * 60, 18 * 60)];
        let long_days = calendar(&mut project, "Ngày dài", vec![long_day; 6]);
        for day in [3, 11, 17] {
            if random.chance(50) {
                let date = civil(start + day * DAY_MS).0.format("%Y-%m-%d").to_string();
                apply(&mut project, Command::AddHoliday { date });
            }
        }
        for uid in 1..=tasks {
            let minutes = HOUR * random.pick(&[0, 1, 2, 4, 8, 12, 16, 24, 40]);
            apply(&mut project, Command::SetDuration { uid, minutes });
            let priority = random.pick(&[300, 500, 500, 500, 700, 1000]);
            apply(&mut project, Command::SetPriority { uid, priority });
            if random.chance(15) {
                apply(&mut project, Command::SetTaskCalendar { uid, calendar_id: mornings });
            } else if random.chance(8) {
                apply(&mut project, Command::SetTaskCalendar { uid, calendar_id: long_days });
            }
        }
        let kinds = [DepKind::Fs, DepKind::Fs, DepKind::Ss, DepKind::Ff, DepKind::Sf];
        for _ in 0..tasks {
            let predecessor_uid = 1 + random.below(u64::from(tasks)) as u32;
            let successor_uid = 1 + random.below(u64::from(tasks)) as u32;
            let kind = random.pick(&kinds);
            let to_summary = project.task(successor_uid).is_some_and(|task| task.summary);
            if mix == Mix::OnePass && to_summary && matches!(kind, DepKind::Ff | DepKind::Sf) {
                continue;
            }
            let lag = random.pick(&[0, 0, -2 * HOUR, 4 * HOUR, 8 * HOUR]);
            let link = Command::AddDependency { predecessor_uid, successor_uid, kind, lag };
            apply(&mut project, link);
        }
        for uid in 1..=tasks {
            let at = start + random.below(10) as i64 * DAY_MS + random.below(9) as i64 * HOUR_MS;
            let summary = project.task(uid).is_some_and(|task| task.summary);
            let constraint_type = match random.below(20) {
                0 => ConstraintType::Snet,
                1 => ConstraintType::Fnlt,
                2 => ConstraintType::Snlt,
                3 if !summary => ConstraintType::Mso,
                4 if matches!(mix, Mix::Alap | Mix::Anything) => ConstraintType::Alap,
                7..=10 if mix == Mix::Alap => ConstraintType::Alap,
                5 if matches!(mix, Mix::GroupFinish | Mix::Anything) || !summary => {
                    ConstraintType::Fnet
                }
                6 if !summary => ConstraintType::Mfo,
                _ => continue,
            };
            let date = Some(at);
            apply(&mut project, Command::SetConstraint { uid, constraint_type, date });
        }
        // Splits are placed from the scheduled start.
        schedule(&mut project).expect("schedules");
        for uid in 1..=tasks {
            let task_start = project.task(uid).map(|task| task.start);
            if let Some(task_start) = task_start.filter(|_| random.chance(10)) {
                let gap = HOUR * random.pick(&[2, 8, 16]);
                apply(&mut project, Command::SplitTask { uid, at: task_start + 2 * HOUR_MS, gap });
            }
            if random.chance(5) {
                let actual = Some(start + random.below(3) as i64 * DAY_MS);
                apply(&mut project, Command::SetActualStart { uid, start: actual });
            }
            if random.chance(4) {
                let manual = Command::SetManual { uid, manual: true, start: None, finish: None };
                apply(&mut project, manual);
            }
        }
        let people = 2 + random.below(4) as u32;
        for person in 0..people {
            let name = format!("Người {person}");
            apply(&mut project, Command::AddResource { name, kind: ResourceKind::Person });
        }
        let resources: Vec<u32> = project.resources.iter().map(|resource| resource.uid).collect();
        for &uid in &resources {
            let max_units = random.pick(&[0.5, 1.0, 1.0, 2.0]);
            let calendar_id = if random.chance(20) { long_days } else { None };
            let update = Command::UpdateResource {
                uid,
                name: format!("Người {uid}"),
                kind: ResourceKind::Person,
                max_units,
                calendar_id,
                standard_rate: 0.0,
                overtime_rate: 0.0,
                cost_per_use: 0.0,
            };
            apply(&mut project, update);
        }
        for task_uid in 1..=tasks {
            for _ in 0..1 + random.below(2) {
                let resource_uid = random.pick(&resources);
                let units = random.pick(&[0.5, 1.0, 1.0]);
                apply(&mut project, Command::AssignResource { task_uid, resource_uid, units });
            }
        }
        if random.chance(60) {
            leveling_order(&mut project, &mut random, tasks);
        }
        if random.chance(60) {
            availability(&mut project, &mut random);
        }
        schedule(&mut project).expect("schedules");
        project
    }

    /// A few periods of other units on some resources: leaves, half time, double time,
    /// some open at one end.
    fn availability(project: &mut Project, random: &mut Random) {
        let start = civil(project.start).0;
        let uids: Vec<u32> = project.resources.iter().map(|resource| resource.uid).collect();
        for uid in uids {
            if !random.chance(60) {
                continue;
            }
            let mut periods = Vec::new();
            let mut day = random.below(6) as i64;
            for index in 0..1 + random.below(3) {
                let from = (index > 0 || !random.chance(15)).then(|| start + chrono::Duration::days(day));
                let length = random.below(8) as i64;
                let open = random.chance(10);
                let to = (!open).then(|| start + chrono::Duration::days(day + length));
                let units = random.pick(&[0.0, 0.0, 0.5, 2.0]);
                periods.push(AvailabilityPeriod { from, to, units });
                if open {
                    break;
                }
                day += length + 1 + random.below(6) as i64;
            }
            apply(project, Command::SetResourceAvailability { uid, periods });
        }
    }

    /// Custom fields with values on most tasks, and a leveling order of one to four
    /// criteria among them and the built-in ones.
    fn leveling_order(project: &mut Project, random: &mut Random, tasks: u32) {
        let options: Vec<String> = ["Gấp", "Thường", "Để sau"].iter().map(|o| o.to_string()).collect();
        let kinds = [FieldKind::Choice { options: options.clone() }, FieldKind::Number, FieldKind::Text];
        for (name, kind) in ["Mức", "Điểm", "Mã"].into_iter().zip(kinds) {
            apply(project, Command::AddField { name: name.into(), kind });
        }
        let ids: Vec<u32> = project.fields.iter().map(|field| field.id).collect();
        for uid in 1..=tasks {
            let values = [
                FieldValue::Text(options[random.below(3) as usize].clone()),
                FieldValue::Number(random.pick(&[-1.5, 0.0, 2.0, 2.0, 7.0])),
                FieldValue::Text(random.pick(&["a", "B", "b", "Đ"]).into()),
            ];
            for (&field_id, value) in ids.iter().zip(values) {
                if random.chance(80) {
                    apply(project, Command::SetFieldValue { uid, field_id, value: Some(value) });
                }
            }
        }
        let mut criteria = vec![
            Criterion::Start,
            Criterion::Priority,
            Criterion::Slack,
            Criterion::Deadline,
            Criterion::Duration,
            Criterion::Id,
        ];
        criteria.extend(ids.iter().map(|&id| Criterion::Field { id }));
        let mut order = Vec::new();
        for _ in 0..1 + random.below(4) {
            let criterion = criteria.remove(random.below(criteria.len() as u64) as usize);
            order.push(SortKey { criterion, descending: random.chance(50) });
        }
        for uid in 1..=tasks {
            if random.chance(30) {
                let date = Some(project.start + random.below(15) as i64 * DAY_MS);
                apply(project, Command::SetDeadline { uid, date });
            }
        }
        apply(project, Command::SetLevelingOrder { order });
    }

    fn leveled(mut project: Project, quick: bool) -> Project {
        level_with(&mut project, quick).expect("levels");
        project
    }

    /// How often the generated projects hold what the test is meant to cover.
    #[derive(Default)]
    struct Seen {
        delayed: usize,
        delayed_splits: usize,
        started: usize,
        manual: usize,
        links_into_groups: usize,
        /// Projects whose leveling order has a custom field.
        ordered_by_field: usize,
        /// Projects their own leveling order levels differently from the default one.
        reordered: usize,
        /// Resources with periods of other units.
        with_availability: usize,
        /// Projects whose periods change how they level.
        moved_by_availability: usize,
    }

    impl Seen {
        fn count(&mut self, before: &Project, after: &Project) {
            for task in after.tasks.iter().filter(|task| task.leveling_delay > 0) {
                self.delayed += 1;
                self.delayed_splits += usize::from(!task.splits.is_empty());
            }
            self.started += before.tasks.iter().filter(|task| task.actual_start.is_some()).count();
            self.manual += before.tasks.iter().filter(|task| task.manual).count();
            self.links_into_groups += before
                .dependencies
                .iter()
                .filter(|dep| before.task(dep.successor_uid).is_some_and(|task| task.summary))
                .count();
            let by_field = |key: &SortKey| matches!(key.criterion, Criterion::Field { .. });
            self.ordered_by_field += usize::from(before.leveling_order.iter().any(by_field));
            self.with_availability += before.resources.iter().filter(|r| !r.availability.is_empty()).count();
        }
    }

    #[test]
    fn the_quick_reschedule_levels_exactly_like_the_full_one() {
        let mut seen = Seen::default();
        for seed in 0..SEEDS {
            let mixes = [Mix::OnePass, Mix::GroupFinish, Mix::OnePass, Mix::Alap, Mix::OnePass, Mix::Anything];
            let mix = mixes[seed as usize % mixes.len()];
            let mut base = project(seed, mix);
            let graph = prepare(&mut base).expect("graph");
            schedule_dates(&mut base, &graph);
            let quick_path = Ripple::new(&base, &graph).is_some();
            assert!(quick_path || mix != Mix::OnePass, "seed {seed} should take the quick path");
            let base_order = base.leveling_order.clone();
            let before_leveling = base.clone();
            let quick = leveled(base.clone(), true);
            let full = leveled(base, false);
            for (a, b) in quick.tasks.iter().zip(&full.tasks) {
                assert_eq!(
                    (a.uid, a.leveling_delay, a.start, a.finish, a.total_slack),
                    (b.uid, b.leveling_delay, b.start, b.finish, b.total_slack),
                    "seed {seed}"
                );
            }
            assert_eq!(quick.overallocations, full.overallocations, "seed {seed}");
            seen.count(&full, &quick);
            if before_leveling.resources.iter().any(|resource| !resource.availability.is_empty()) {
                let mut plain = before_leveling.clone();
                for resource in &mut plain.resources {
                    resource.availability.clear();
                }
                let plain = leveled(plain, true);
                let delays = |project: &Project| -> Vec<i64> {
                    project.tasks.iter().map(|task| task.leveling_delay).collect()
                };
                seen.moved_by_availability += usize::from(delays(&plain) != delays(&quick));
            }
            if !base_order.is_empty() {
                let mut default = before_leveling;
                default.leveling_order.clear();
                let default = leveled(default, true);
                let delays = |project: &Project| -> Vec<i64> {
                    project.tasks.iter().map(|task| task.leveling_delay).collect()
                };
                seen.reordered += usize::from(delays(&default) != delays(&quick));
            }
        }
        // Without these the test would prove little: leveling must move many tasks,
        // split ones among them, around started and manual tasks and links into groups,
        // and many projects must level in an order of their own and by capacity by date.
        assert!(seen.delayed >= 5 * SEEDS as usize, "only {} delayed", seen.delayed);
        assert!(seen.delayed_splits >= 20, "only {} split tasks delayed", seen.delayed_splits);
        assert!(seen.started >= 50, "only {} started tasks", seen.started);
        assert!(seen.manual >= 50, "only {} manual tasks", seen.manual);
        assert!(seen.links_into_groups >= 100, "only {} links into groups", seen.links_into_groups);
        assert!(seen.ordered_by_field >= 40, "only {} ordered by a field", seen.ordered_by_field);
        assert!(seen.reordered >= 50, "only {} leveled apart from the default", seen.reordered);
        assert!(seen.with_availability >= 120, "only {} resources with periods", seen.with_availability);
        assert!(seen.moved_by_availability >= 50, "only {} moved by periods", seen.moved_by_availability);
    }

    /// Projects for the check against searching four years: a search that never fits
    /// takes a few thousand rounds then.
    const FULL_SEARCH_SEEDS: u64 = 60;

    #[test]
    fn ending_searches_that_came_round_levels_like_searching_four_years() {
        let mut came_round = 0;
        for seed in 0..FULL_SEARCH_SEEDS {
            let base = project(seed, Mix::OnePass);
            CAME_ROUND.with(|count| count.set(0));
            let early = leveled(base.clone(), true);
            let stopped = CAME_ROUND.with(|count| count.get());
            SEARCH_IN_FULL.with(|full| full.set(true));
            CAME_ROUND.with(|count| count.set(0));
            let full = leveled(base, true);
            SEARCH_IN_FULL.with(|full| full.set(false));
            // Searching on, every search that came round still gives up in the end.
            assert_eq!(CAME_ROUND.with(|count| count.get()), stopped, "seed {seed}");
            for (a, b) in early.tasks.iter().zip(&full.tasks) {
                assert_eq!(
                    (a.uid, a.leveling_delay, a.start, a.finish, a.total_slack),
                    (b.uid, b.leveling_delay, b.start, b.finish, b.total_slack),
                    "seed {seed}"
                );
            }
            assert_eq!(early.overallocations, full.overallocations, "seed {seed}");
            came_round += stopped;
        }
        assert!(came_round >= 25, "only {came_round} searches came round");
    }
}
