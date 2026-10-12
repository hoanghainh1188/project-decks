//! Resource overallocation and leveling.
//!
//! A resource is overallocated when, at some instant, the units of the assignments that
//! are working then add up to more than it has: its max units while its own calendar
//! works, nothing otherwise. Work counts only where the task's calendar works and inside
//! the task's segments, so two tasks on the same day at different hours do not clash,
//! and a split gap carries no load.
//!
//! Each resource is swept along time: load and availability change only at piece and
//! calendar boundaries, so the elementary stretches between boundaries have one load
//! and one availability each.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;

use crate::calendar::Calendar;
use crate::error::EngineError;
use crate::model::{ConstraintType, Overallocation, Project, Task};
use crate::schedule::{prepare, publish_derived, schedule_dates};
use crate::split;
use crate::timeutil::{civil, compose, date_key};

/// Units below this are rounding noise.
const EPSILON: f64 = 1e-6;

pub fn publish_overallocations(project: &mut Project) {
    project.overallocations = find_overallocations(project);
}

/// One row per resource and day with an overload: the peak units that day and what the
/// resource had at that moment.
pub fn find_overallocations(project: &Project) -> Vec<Overallocation> {
    let mut days: BTreeMap<(u32, NaiveDate), (f64, f64)> = BTreeMap::new();
    for sweep in sweeps(project) {
        for stretch in sweep.stretches.iter().filter(|stretch| stretch.over()) {
            let (date, _) = civil(stretch.start);
            let entry = days
                .entry((sweep.resource_uid, date))
                .or_insert((stretch.load, stretch.available));
            if stretch.load > entry.0 {
                *entry = (stretch.load, stretch.available);
            }
        }
    }
    days.into_iter()
        .map(
            |((resource_uid, date), (units, max_units))| Overallocation {
                resource_uid,
                date: date_key(date),
                units,
                max_units,
            },
        )
        .collect()
}

/// A piece of one assignment's work inside one calendar range of one day.
#[derive(Clone, Copy)]
struct Piece {
    start: i64,
    end: i64,
    units: f64,
}

/// A stretch of time with one load and one availability.
struct Stretch {
    start: i64,
    load: f64,
    available: f64,
}

impl Stretch {
    fn over(&self) -> bool {
        self.load > self.available + EPSILON
    }
}

struct Sweep {
    resource_uid: u32,
    stretches: Vec<Stretch>,
}

fn sweeps(project: &Project) -> Vec<Sweep> {
    let mut pieces: BTreeMap<u32, Vec<Piece>> = BTreeMap::new();
    for assignment in &project.assignments {
        if assignment.units <= 0.0 {
            continue;
        }
        let Some(task) = project.task(assignment.task_uid) else {
            continue;
        };
        if task.summary || task.finish <= task.start {
            continue;
        }
        let into = pieces.entry(assignment.resource_uid).or_default();
        working_pieces(project, task, assignment.units, into);
    }
    pieces
        .into_iter()
        .filter_map(|(resource_uid, pieces)| {
            let resource = project.resources.iter().find(|r| r.uid == resource_uid)?;
            let calendar = project.resource_calendar(resource);
            let stretches = stretches(
                &pieces,
                |date| working_ranges(calendar, date),
                resource.max_units,
            );
            Some(Sweep {
                resource_uid,
                stretches,
            })
        })
        .collect()
}

/// The task's work cut at its own calendar's ranges, day by day.
fn working_pieces(project: &Project, task: &Task, units: f64, into: &mut Vec<Piece>) {
    let calendar = project.task_calendar(task);
    cut_to_calendar(calendar, &split::work_intervals(task), units, into);
}

fn cut_to_calendar(
    calendar: &Calendar,
    intervals: &[(i64, i64)],
    units: f64,
    into: &mut Vec<Piece>,
) {
    for &(low, high) in intervals {
        if high <= low {
            continue;
        }
        let (mut date, _) = civil(low);
        let (last, _) = civil(high);
        loop {
            for range in calendar.ranges_on(date) {
                let start = low.max(compose(date, range.start_minute));
                let end = high.min(compose(date, range.end_minute));
                if end > start {
                    into.push(Piece { start, end, units });
                }
            }
            match date.succ_opt() {
                Some(next) if date < last => date = next,
                _ => break,
            }
        }
    }
}

/// Load and availability between every pair of consecutive boundaries, in time order,
/// from the first day with work to the last.
fn stretches(
    pieces: &[Piece],
    ranges_on: impl Fn(NaiveDate) -> Vec<(i64, i64)>,
    max_units: f64,
) -> Vec<Stretch> {
    // (instant, load change, availability change)
    let mut events: Vec<(i64, f64, f64)> = Vec::with_capacity(pieces.len() * 2);
    for piece in pieces {
        events.push((piece.start, piece.units, 0.0));
        events.push((piece.end, -piece.units, 0.0));
    }
    let first = pieces.iter().map(|piece| civil(piece.start).0).min();
    let last = pieces.iter().map(|piece| civil(piece.start).0).max();
    if let (Some(mut date), Some(last)) = (first, last) {
        loop {
            for (start, end) in ranges_on(date) {
                events.push((start, 0.0, max_units));
                events.push((end, 0.0, -max_units));
            }
            match date.succ_opt() {
                Some(next) if date < last => date = next,
                _ => break,
            }
        }
    }
    events.sort_by_key(|event| event.0);
    let mut stretches = Vec::new();
    let (mut load, mut available) = (0.0f64, 0.0f64);
    let mut index = 0;
    while index < events.len() {
        let at = events[index].0;
        while index < events.len() && events[index].0 == at {
            load += events[index].1;
            available += events[index].2;
            index += 1;
        }
        if let Some(next) = events.get(index).map(|event| event.0) {
            if next > at {
                stretches.push(Stretch {
                    start: at,
                    load: load.max(0.0),
                    available: available.max(0.0),
                });
            }
        }
    }
    stretches
}

/// Serial leveling, close to Project's. Tasks are placed one at a time: those leveling
/// will not delay first, then tasks whose predecessors are placed, then earliest start,
/// higher priority, less slack. Each goes to the first moment every one of its resources
/// has room for it for its whole duration, next to the tasks already placed, and placing
/// it reschedules the dates so its successors move before they are placed. Placed tasks
/// do not move again, so no idle gap is left that a later choice could fill.
///
/// Writes the result into `leveling_delay` and never splits a task (existing splits
/// move with it). Tasks pinned by hand, by an actual start, or by a must-start/finish-on
/// date, and tasks with priority 1000, stay where they are.
pub fn level_project(project: &mut Project) -> Result<(), EngineError> {
    for task in &mut project.tasks {
        if movable(task) {
            task.leveling_delay = 0;
        }
    }
    // Leveling changes dates, not links or the outline, so the graph is built once.
    let graph = prepare(project)?;
    schedule_dates(project, &graph);
    let predecessors = leaf_predecessors(project);
    let mut placed: Vec<u32> = Vec::new();
    let mut done: BTreeSet<u32> = BTreeSet::new();
    let mut busy: BTreeMap<u32, Busy> = BTreeMap::new();
    let leaves = project.tasks.iter().filter(|task| !task.summary).count();
    while done.len() < leaves {
        let Some(uid) = next_to_place(project, &done, &predecessors) else {
            break;
        };
        done.insert(uid);
        let Some(task) = project.task(uid) else {
            continue;
        };
        let resources = resources_of(project, uid);
        let before = dates_of(project, &placed);
        if levelable(task) && !resources.is_empty() {
            let start = earliest_fit(project, task, &resources, &busy);
            if start > task.start {
                let delay = task.leveling_delay;
                let bump = project
                    .task_calendar(task)
                    .working_between(task.start, start)
                    .max(1);
                set_delay(project, uid, delay + bump);
                schedule_dates(project, &graph);
                // A constraint can hold the task where it was: keep the clash rather than
                // a delay that does nothing.
                if project.task(uid).is_some_and(|task| task.start < start) {
                    set_delay(project, uid, delay);
                    schedule_dates(project, &graph);
                }
            }
        }
        // A placed task can still move when a predecessor placed after it is delayed
        // (a summary link, say); keep the busy profile true to the current dates.
        if dates_of(project, &placed) != before {
            placed.push(uid);
            busy = busy_of(project, &placed);
        } else {
            placed.push(uid);
            add_to_busy(project, uid, &mut busy);
        }
    }
    publish_derived(project, &graph);
    Ok(())
}

/// A task whose dates leveling may not touch: pinned by hand, by an actual start, or by
/// a hard date. Its leveling delay is kept as it is.
fn movable(task: &Task) -> bool {
    !(task.summary
        || task.manual
        || task.actual_start.is_some()
        || matches!(
            task.constraint_type,
            ConstraintType::Mso | ConstraintType::Mfo
        ))
}

/// A task leveling may delay: movable, and not marked "do not level" (priority 1000).
fn levelable(task: &Task) -> bool {
    movable(task) && task.priority < 1000
}

/// The next leaf to place. Tasks leveling will not delay come first: their place is
/// fixed, so the others must fit around them. Then tasks whose leaf predecessors are
/// all placed, so a predecessor delayed later cannot push a placed task. Then the
/// earliest start, higher priority, less slack, lower uid.
fn next_to_place(
    project: &Project,
    done: &BTreeSet<u32>,
    predecessors: &BTreeMap<u32, Vec<u32>>,
) -> Option<u32> {
    project
        .tasks
        .iter()
        .filter(|task| !task.summary && !done.contains(&task.uid))
        .min_by_key(|task| {
            let waiting = predecessors
                .get(&task.uid)
                .is_some_and(|preds| preds.iter().any(|pred| !done.contains(pred)));
            (
                levelable(task),
                waiting,
                task.start,
                -task.priority,
                task.total_slack,
                task.uid,
            )
        })
        .map(|task| task.uid)
}

/// Direct leaf predecessors of every task, from links between leaves.
fn leaf_predecessors(project: &Project) -> BTreeMap<u32, Vec<u32>> {
    let leaves: BTreeSet<u32> = project
        .tasks
        .iter()
        .filter(|task| !task.summary)
        .map(|task| task.uid)
        .collect();
    let mut map: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for dep in &project.dependencies {
        if leaves.contains(&dep.predecessor_uid) && leaves.contains(&dep.successor_uid) {
            map.entry(dep.successor_uid)
                .or_default()
                .push(dep.predecessor_uid);
        }
    }
    map
}

/// Units per resource the task needs, from its assignments.
fn resources_of(project: &Project, uid: u32) -> Vec<(u32, f64)> {
    let mut units: BTreeMap<u32, f64> = BTreeMap::new();
    for assignment in project
        .assignments
        .iter()
        .filter(|a| a.task_uid == uid && a.units > 0.0)
    {
        *units.entry(assignment.resource_uid).or_insert(0.0) += assignment.units;
    }
    units.into_iter().collect()
}

fn set_delay(project: &mut Project, uid: u32, minutes: i64) {
    if let Some(task) = project.task_mut(uid) {
        task.leveling_delay = minutes;
    }
}

fn dates_of(project: &Project, uids: &[u32]) -> Vec<(i64, i64)> {
    uids.iter()
        .filter_map(|uid| project.task(*uid).map(|task| (task.start, task.finish)))
        .collect()
}

/// The work already placed on one resource, with its load swept along time.
#[derive(Default)]
struct Busy {
    pieces: Vec<Piece>,
    stretches: Vec<Stretch>,
    /// End of the last placed piece: the resource is free after it.
    until: i64,
}

/// The busy profile of every resource from the placed tasks' current dates.
fn busy_of(project: &Project, placed: &[u32]) -> BTreeMap<u32, Busy> {
    let mut busy = BTreeMap::new();
    for uid in placed {
        add_to_busy(project, *uid, &mut busy);
    }
    busy
}

fn add_to_busy(project: &Project, uid: u32, busy: &mut BTreeMap<u32, Busy>) {
    let Some(task) = project.task(uid) else {
        return;
    };
    for (resource_uid, units) in resources_of(project, uid) {
        let Some(resource) = project.resources.iter().find(|r| r.uid == resource_uid) else {
            continue;
        };
        let entry = busy.entry(resource_uid).or_default();
        working_pieces(project, task, units, &mut entry.pieces);
        let calendar = project.resource_calendar(resource);
        entry.stretches = stretches(
            &entry.pieces,
            |date| working_ranges(calendar, date),
            resource.max_units,
        );
        entry.until = entry
            .pieces
            .iter()
            .map(|piece| piece.end)
            .max()
            .unwrap_or(0);
    }
}

/// The working ranges of a day, sorted and with overlaps merged, so availability is
/// never counted twice.
fn working_ranges(calendar: &Calendar, date: NaiveDate) -> Vec<(i64, i64)> {
    let mut ranges: Vec<(i64, i64)> = calendar
        .ranges_on(date)
        .iter()
        .filter(|range| range.start_minute < range.end_minute)
        .map(|range| {
            (
                compose(date, range.start_minute),
                compose(date, range.end_minute),
            )
        })
        .collect();
    ranges.sort_unstable();
    let mut merged: Vec<(i64, i64)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

/// The first start from the task's current start at which its work fits next to the
/// placed work of every resource. A resource the task alone overloads (more units than
/// it has) cannot be fixed by waiting and is left out of the check.
fn earliest_fit(
    project: &Project,
    task: &Task,
    resources: &[(u32, f64)],
    busy: &BTreeMap<u32, Busy>,
) -> i64 {
    let calendar = project.task_calendar(task);
    let duration = task.duration.max(0);
    let mut start = task.start;
    // Each round moves past one busy block. Past four years the resource will not have
    // room (its calendar never meets the task's, say): leave the task where it was.
    let limit = task.start.saturating_add(4 * 366 * 86_400_000);
    for _ in 0..10_000 {
        if start > limit {
            break;
        }
        let segments = split::layout(calendar, start, duration, &task.splits);
        let intervals: Vec<(i64, i64)> = segments
            .iter()
            .map(|segment| (segment.start, segment.finish))
            .collect();
        let mut pieces = Vec::new();
        cut_to_calendar(calendar, &intervals, 0.0, &mut pieces);
        let clash = resources
            .iter()
            .filter_map(|(resource_uid, units)| {
                let resource = project.resources.iter().find(|r| r.uid == *resource_uid)?;
                if *units > resource.max_units + EPSILON {
                    return None;
                }
                let empty = Busy::default();
                let placed = busy.get(resource_uid).unwrap_or(&empty);
                let resource_calendar = project.resource_calendar(resource);
                let (from, to) = first_clash(
                    resource_calendar,
                    resource.max_units,
                    &placed.pieces,
                    &pieces,
                    *units,
                )?;
                Some((from, room_from(placed, resource_calendar, to, *units)))
            })
            .min();
        let Some((from, room)) = clash else {
            return start;
        };
        // Slide the task so the work that met the clash starts where the resource next
        // has room, past the whole busy block at once.
        let shift = calendar.working_between(from, room).max(1);
        start = calendar.add_lag(start, shift);
    }
    task.start
}

/// The first time from `at` on when the resource works and has room for `units` more:
/// inside its placed work, or once that is over, when its calendar next works.
fn room_from(busy: &Busy, calendar: &Calendar, at: i64, units: f64) -> i64 {
    let first = busy.stretches.partition_point(|stretch| stretch.start < at);
    busy.stretches[first..]
        .iter()
        .find(|stretch| {
            stretch.available > EPSILON && stretch.load + units <= stretch.available + EPSILON
        })
        .map_or_else(
            || calendar.next_work_start(busy.until.max(at)),
            |stretch| stretch.start,
        )
}

/// The earliest stretch in which `pieces` at `units` do not fit next to `others` on a
/// resource with `max_units` while its calendar works.
fn first_clash(
    calendar: &Calendar,
    max_units: f64,
    others: &[Piece],
    pieces: &[Piece],
    units: f64,
) -> Option<(i64, i64)> {
    for piece in pieces {
        let (date, _) = civil(piece.start);
        let mut cuts = vec![piece.start, piece.end];
        let overlapping: Vec<&Piece> = others
            .iter()
            .filter(|other| other.start < piece.end && other.end > piece.start)
            .collect();
        for other in &overlapping {
            cuts.push(other.start.clamp(piece.start, piece.end));
            cuts.push(other.end.clamp(piece.start, piece.end));
        }
        let ranges = working_ranges(calendar, date);
        for (start, end) in &ranges {
            cuts.push((*start).clamp(piece.start, piece.end));
            cuts.push((*end).clamp(piece.start, piece.end));
        }
        cuts.sort_unstable();
        cuts.dedup();
        for window in cuts.windows(2) {
            let (from, to) = (window[0], window[1]);
            let load: f64 = overlapping
                .iter()
                .filter(|other| other.start < to && other.end > from)
                .map(|other| other.units)
                .sum();
            let works = ranges
                .iter()
                .any(|(start, end)| *start <= from && to <= *end);
            let available = if works { max_units } else { 0.0 };
            if load + units > available + EPSILON {
                return Some((from, to));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(start: i64, end: i64, units: f64) -> Piece {
        Piece { start, end, units }
    }

    #[test]
    fn stretches_follow_load_and_availability() {
        let day = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        let at = |minute: u16| compose(day, minute);
        let pieces = [piece(at(480), at(720), 1.0), piece(at(600), at(720), 0.5)];
        let found = stretches(&pieces, |_| vec![(at(480), at(720))], 1.0);
        let summary: Vec<(i64, f64, f64)> = found
            .iter()
            .map(|s| (s.start, s.load, s.available))
            .collect();
        assert_eq!(summary, vec![(at(480), 1.0, 1.0), (at(600), 1.5, 1.0)]);
        assert!(!found[0].over());
        assert!(found[1].over());
    }
}
