//! Urgent tasks when leveling (`Task::interrupts`). A child of `level.rs`, so it shares
//! its sweep and search; it runs only when the project has an urgent task.
//!
//! An urgent task U goes ahead of work in progress ranked below it (by the leveling
//! order, start aside: `Order::ranks_above`) on the people it shares with that work. When
//! U is placed, the load of such work is left out of the search, so U takes the first
//! moment its people would have without it; each piece of that work U now overlaps is then
//! split where U begins (after the part done, for work actually started) and its rest
//! resumes once its people have room again. Work not yet running by then is only delayed,
//! as leveling always does. A task placed after U that ranks below it flows around U's
//! work the same way: it does its first part, pauses while U runs, and goes on after.
//!
//! Work that cannot move is never split: manual, must start or finish on a date, not to
//! be leveled (priority 1000), urgent itself, or started with nothing done yet. The splits
//! are marked as leveling's, so the next run and Clear Leveling take them back.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::{
    cut_to_calendar, delay_to, earliest_fit, reschedule, room_from, stretches, working_pieces,
    working_ranges, Busy, Checked, Lookup, Order, Piece, EPSILON, NO_OWNER,
};
use crate::availability::Capacity;
use crate::graph::Graph;
use crate::level_cycle::midnight_of;
use crate::model::{ConstraintType, Project, Task};
use crate::ripple::Ripple;
use crate::split::{self, Split};
use crate::timeutil::civil;
use crate::tracking::done_minutes;

/// What placing one leaf reads and changes, borrowed from `level_with`.
pub(super) struct Placing<'a> {
    pub graph: &'a Graph,
    pub ripple: &'a mut Option<Ripple>,
    pub lookup: &'a Lookup,
    pub order: &'a Order,
    pub busy: &'a BTreeMap<u32, Busy>,
    pub is_placed: &'a [bool],
    pub before: &'a mut HashMap<usize, (i64, i64)>,
}

/// What `make_way` did to a victim.
#[derive(PartialEq)]
enum Way {
    Unchanged,
    Changed,
    /// It still clashes and cannot be split or delayed: the urgent task has to wait.
    Stuck,
}

/// Past the last placed work by this much, a clash comes from the calendars alone and
/// waiting longer does not help.
const PAST_THE_LOAD: i64 = 14 * 86_400_000;

impl Placing<'_> {
    /// Place the levelable leaf at `index`. Returns the resources whose busy profile has
    /// to be built again because work already placed was split or moved.
    pub(super) fn place(&mut self, project: &mut Project, index: usize) -> BTreeSet<u32> {
        if project.tasks[index].interrupts {
            self.place_urgent(project, index)
        } else {
            self.place_around(project, index);
            BTreeSet::new()
        }
    }

    fn place_urgent(&mut self, project: &mut Project, index: usize) -> BTreeSet<u32> {
        let resources = &self.lookup.units[index];
        let mine: BTreeSet<u32> = resources.iter().map(|(uid, _)| *uid).collect();
        // The work it may interrupt: placed, ranked below it, on one of its people.
        let mut victims: BTreeSet<usize> = BTreeSet::new();
        for uid in &mine {
            for piece in self.busy.get(uid).map_or(&[][..], |busy| &busy.pieces) {
                let other = piece.owner;
                if other != NO_OWNER
                    && !victims.contains(&other)
                    && may_be_interrupted(&project.tasks[other])
                    && self.order.ranks_above(index, other)
                {
                    victims.insert(other);
                }
            }
        }
        if victims.is_empty() {
            let start = earliest_fit(
                project,
                self.lookup,
                &project.tasks[index],
                resources,
                self.busy,
            );
            delay_to(project, self.graph, self.ripple, index, start, self.before);
            return BTreeSet::new();
        }
        // The load of every person involved, kept up to date while the victims move.
        let mut people = mine.clone();
        for &victim in &victims {
            people.extend(self.lookup.units[victim].iter().map(|(uid, _)| *uid));
        }
        let mut local: BTreeMap<u32, Busy> = people
            .iter()
            .filter_map(|uid| {
                Some((
                    *uid,
                    Busy {
                        pieces: self.busy.get(uid)?.pieces.clone(),
                        ..Busy::default()
                    },
                ))
            })
            .collect();
        // Without the victims' work that can still be taken back.
        let open: HashMap<usize, i64> = victims
            .iter()
            .map(|&other| (other, open_from(project, &project.tasks[other])))
            .collect();
        let without = self.swept(project, &local, |piece| match open.get(&piece.owner) {
            Some(&from) if piece.end > from => (piece.start < from).then_some(Piece {
                end: from,
                ..*piece
            }),
            _ => Some(*piece),
        });
        let start = earliest_fit(
            project,
            self.lookup,
            &project.tasks[index],
            resources,
            &without,
        );
        delay_to(project, self.graph, self.ripple, index, start, self.before);
        self.replace_work(project, &mut local, index);
        let mut ordered: Vec<usize> = victims.into_iter().collect();
        ordered.sort_by_key(|&other| (project.tasks[other].start, other));
        let mut stuck = false;
        for victim in ordered {
            match self.make_way(project, &local, victim) {
                Way::Changed => self.replace_work(project, &mut local, victim),
                Way::Unchanged => {}
                Way::Stuck => stuck = true,
            }
        }
        if stuck {
            // Work that can neither pause nor wait keeps its place: the urgent task goes
            // after it (pauses already made for its first place stay, harmless).
            let busy = self.swept(project, &local, |piece| {
                (piece.owner != index).then_some(*piece)
            });
            let start = earliest_fit(
                project,
                self.lookup,
                &project.tasks[index],
                resources,
                &busy,
            );
            delay_to(project, self.graph, self.ripple, index, start, self.before);
            self.replace_work(project, &mut local, index);
        }
        self.settle_moved(project, &mut local, &mut people, index);
        people
    }

    /// Placed work the splits and delays pushed later (a victim's successors) is placed
    /// again next to the load as it now is, and so on down the links, so no clash is left
    /// among work already placed.
    fn settle_moved(
        &mut self,
        project: &mut Project,
        local: &mut BTreeMap<u32, Busy>,
        people: &mut BTreeSet<u32>,
        index: usize,
    ) {
        // The dates each task had when last placed again: one pushed once more (a link
        // other than FS can push an earlier task later) is placed again too. Every round
        // only moves work later, and the rounds are capped, so this ends.
        let mut handled: HashMap<usize, (i64, i64)> = HashMap::new();
        for _ in 0..4 * project.tasks.len().max(1) {
            let mut moved: Vec<usize> = self
                .before
                .iter()
                .filter(|(&other, &dates)| {
                    let now = (project.tasks[other].start, project.tasks[other].finish);
                    other != index
                        && self.is_placed[other]
                        && dates != now
                        && handled.get(&other) != Some(&now)
                })
                .map(|(&other, _)| other)
                .collect();
            if moved.is_empty() {
                return;
            }
            moved.sort_by_key(|&other| (project.tasks[other].start, other));
            for other in moved {
                for &(uid, _) in &self.lookup.units[other] {
                    if people.insert(uid) {
                        if let Some(busy) = self.busy.get(&uid) {
                            local.insert(
                                uid,
                                Busy {
                                    pieces: busy.pieces.clone(),
                                    ..Busy::default()
                                },
                            );
                        }
                    }
                }
                self.replace_work(project, local, other);
                if !super::levelable(&project.tasks[other]) {
                    handled.insert(
                        other,
                        (project.tasks[other].start, project.tasks[other].finish),
                    );
                    continue;
                }
                let busy = self.swept(project, local, |piece| {
                    (piece.owner != other).then_some(*piece)
                });
                let resources = &self.lookup.units[other];
                let start = earliest_fit(
                    project,
                    self.lookup,
                    &project.tasks[other],
                    resources,
                    &busy,
                );
                if start > project.tasks[other].start {
                    delay_to(project, self.graph, self.ripple, other, start, self.before);
                    self.replace_work(project, local, other);
                }
                handled.insert(
                    other,
                    (project.tasks[other].start, project.tasks[other].finish),
                );
            }
        }
    }

    /// Put the current work of the task at `index` into `local`, in place of its earlier.
    fn replace_work(&self, project: &Project, local: &mut BTreeMap<u32, Busy>, index: usize) {
        for busy in local.values_mut() {
            busy.pieces.retain(|piece| piece.owner != index);
        }
        for &(resource_uid, units) in &self.lookup.units[index] {
            if let Some(busy) = local.get_mut(&resource_uid) {
                working_pieces(
                    project,
                    &project.tasks[index],
                    units,
                    index,
                    &mut busy.pieces,
                );
            } else {
                let mut pieces = Vec::new();
                working_pieces(project, &project.tasks[index], units, index, &mut pieces);
                local.insert(
                    resource_uid,
                    Busy {
                        pieces,
                        ..Busy::default()
                    },
                );
            }
        }
    }

    /// Split or delay `victim` so it no longer clashes with the work in `local`.
    fn make_way(
        &mut self,
        project: &mut Project,
        local: &BTreeMap<u32, Busy>,
        victim: usize,
    ) -> Way {
        let busy = self.swept(project, local, |piece| {
            (piece.owner != victim).then_some(*piece)
        });
        let task = &project.tasks[victim];
        let resources = &self.lookup.units[victim];
        let checked = checked(project, self.lookup, task, resources, &busy);
        let calendar = project.task_calendar(task);
        let intervals = split::work_intervals(task);
        let mut pieces = Vec::new();
        cut_to_calendar(calendar, &intervals, 0.0, NO_OWNER, &mut pieces);
        let Some((from, _)) = checked
            .iter()
            .filter_map(|resource| super::first_clash(resource, &pieces))
            .min()
        else {
            return Way::Unchanged;
        };
        let worked = split::working_within(calendar, &intervals, task.start, from);
        let done = if task.actual_start.is_some() {
            done_minutes(task)
        } else {
            0
        };
        let offset = worked.max(done);
        let gap = if offset > 0 && offset < task.duration {
            gap_after(task, calendar, offset, &checked)
        } else {
            None
        };
        let Some(gap) = gap else {
            // Not running yet when the urgent task begins, or no pause lets the rest fit:
            // wait for it, as leveling does; work started cannot wait.
            if task.actual_start.is_some() {
                return Way::Stuck;
            }
            let start = earliest_fit(project, self.lookup, task, resources, &busy);
            let was = (task.start, task.finish);
            delay_to(project, self.graph, self.ripple, victim, start, self.before);
            let now = (project.tasks[victim].start, project.tasks[victim].finish);
            return if now != was { Way::Changed } else { Way::Stuck };
        };
        let task = &mut project.tasks[victim];
        task.splits.push(Split {
            offset,
            gap,
            leveling: true,
        });
        split::normalize_task(task);
        reschedule(project, self.graph, self.ripple, victim, self.before);
        Way::Changed
    }

    /// Place an ordinary leaf, letting it flow around the work of urgent tasks ranked
    /// above it.
    fn place_around(&mut self, project: &mut Project, index: usize) {
        let resources = &self.lookup.units[index];
        let order = self.order;
        let urgent_above = |owner: usize| {
            owner != NO_OWNER && project.tasks[owner].interrupts && order.ranks_above(owner, index)
        };
        let near = resources.iter().any(|(uid, _)| {
            self.busy
                .get(uid)
                .is_some_and(|busy| busy.pieces.iter().any(|piece| urgent_above(piece.owner)))
        });
        let task = &project.tasks[index];
        if !near {
            let start = earliest_fit(project, self.lookup, task, resources, self.busy);
            delay_to(project, self.graph, self.ripple, index, start, self.before);
            return;
        }
        let (start, made) = fit_around(
            project,
            self.lookup,
            task,
            resources,
            self.busy,
            &urgent_above,
        );
        delay_to(project, self.graph, self.ripple, index, start, self.before);
        // The pauses were found for that start; a constraint holding the task elsewhere
        // makes them meaningless.
        if made.is_empty() || project.tasks[index].start != start {
            return;
        }
        let task = &mut project.tasks[index];
        task.splits.extend(made);
        split::normalize_task(task);
        reschedule(project, self.graph, self.ripple, index, self.before);
    }

    /// `local`'s pieces as `keep` maps them (`None` drops one), swept into load.
    fn swept(
        &self,
        project: &Project,
        local: &BTreeMap<u32, Busy>,
        keep: impl Fn(&Piece) -> Option<Piece>,
    ) -> BTreeMap<u32, Busy> {
        local
            .iter()
            .filter_map(|(&resource_uid, busy)| {
                let resource = self.lookup.resource(project, resource_uid)?;
                let pieces: Vec<Piece> = busy.pieces.iter().filter_map(&keep).collect();
                let calendar = project.resource_calendar(resource);
                let capacity = Capacity::of(resource);
                let stretches = stretches(
                    &pieces,
                    |date| working_ranges(calendar, date),
                    |date| capacity.units_on(date),
                );
                let until = pieces.iter().map(|piece| piece.end).max().unwrap_or(0);
                Some((
                    resource_uid,
                    Busy {
                        pieces,
                        stretches,
                        until,
                    },
                ))
            })
            .collect()
    }
}

/// Work an urgent task may split: not pinned in place and not urgent itself.
fn may_be_interrupted(task: &Task) -> bool {
    !(task.summary
        || task.manual
        || task.interrupts
        || task.priority >= 1000
        || matches!(
            task.constraint_type,
            ConstraintType::Mso | ConstraintType::Mfo
        ))
}

/// From when a task's work can be taken back: after the part done when it has started
/// (`i64::MIN` for all of it otherwise).
fn open_from(project: &Project, task: &Task) -> i64 {
    if task.actual_start.is_none() {
        return i64::MIN;
    }
    let done = done_minutes(task);
    if done <= 0 {
        // Started with nothing done: its start is pinned, so none of it can move.
        return i64::MAX;
    }
    split::resume_at(project.task_calendar(task), task, done)
}

/// The people `earliest_fit` would check for `task`, with their load.
fn checked<'a>(
    project: &'a Project,
    lookup: &Lookup,
    task: &Task,
    resources: &[(u32, f64)],
    busy: &'a BTreeMap<u32, Busy>,
) -> Vec<Checked<'a>> {
    let (first_day, _) = civil(task.start);
    resources
        .iter()
        .filter_map(|&(resource_uid, units)| {
            let index = *lookup.resource.get(&resource_uid)?;
            let resource = &project.resources[index];
            let capacity = Capacity::of(resource);
            if units > capacity.most_from(first_day) + EPSILON {
                return None;
            }
            Some(Checked {
                units,
                // A person with no placed work still has a calendar and capacity to check.
                placed: busy.get(&resource_uid).unwrap_or(&NO_LOAD),
                calendar: project.resource_calendar(resource),
                capacity,
            })
        })
        .collect()
}

/// The shortest gap, in working minutes on `calendar`, after the first `offset` minutes
/// of `task`'s work that lets the rest fit next to every person's load; `None` when it
/// does not within four years.
fn gap_after(
    task: &Task,
    calendar: &crate::calendar::Calendar,
    offset: i64,
    checked: &[Checked],
) -> Option<i64> {
    let limit = 4 * 366 * 8 * 60;
    let past = past_the_load(checked);
    let mut gap = 0i64;
    for _ in 0..10_000 {
        let mut splits = task.splits.clone();
        splits.push(Split {
            offset,
            gap,
            leveling: true,
        });
        split::normalize(&mut splits, task.duration);
        let intervals: Vec<(i64, i64)> =
            split::layout(calendar, task.start, task.duration.max(0), &splits)
                .iter()
                .map(|segment| (segment.start, segment.finish))
                .collect();
        let mut pieces = Vec::new();
        cut_to_calendar(calendar, &intervals, 0.0, NO_OWNER, &mut pieces);
        let clash = checked
            .iter()
            .filter_map(|resource| {
                let (from, to) = super::first_clash(resource, &pieces)?;
                Some((from, room_from(resource, to)))
            })
            .min();
        let Some((from, room)) = clash else {
            return (gap > 0).then_some(gap);
        };
        // A clash in the part before the split, or past all placed work (the calendars
        // never meet), cannot be helped by a longer gap.
        if from > past || split::working_within(calendar, &intervals, task.start, from) < offset {
            return None;
        }
        gap += calendar.working_between(from, room).max(1);
        if gap > limit {
            return None;
        }
    }
    None
}

/// No load: a person with no placed work.
static NO_LOAD: Busy = Busy {
    pieces: Vec::new(),
    stretches: Vec::new(),
    until: 0,
};

/// Well past the end of every checked person's placed work and their last change of
/// capacity (a long leave after the work is still waited out).
fn past_the_load(checked: &[Checked]) -> i64 {
    let until = checked
        .iter()
        .map(|resource| {
            resource
                .placed
                .until
                .max(midnight_of(resource.capacity.steady_from()))
        })
        .max()
        .unwrap_or(i64::MIN);
    until.saturating_add(PAST_THE_LOAD)
}

/// `earliest_fit` for a task that may pause where only the work of urgent tasks ranked
/// above it (`urgent`) leaves no room: the start, and the splits that make the pauses.
fn fit_around(
    project: &Project,
    lookup: &Lookup,
    task: &Task,
    resources: &[(u32, f64)],
    busy: &BTreeMap<u32, Busy>,
    urgent: &dyn Fn(usize) -> bool,
) -> (i64, Vec<Split>) {
    let calendar = project.task_calendar(task);
    let duration = task.duration.max(0);
    let checked = checked(project, lookup, task, resources, busy);
    let limit = task.start.saturating_add(4 * 366 * 86_400_000);
    let past = past_the_load(&checked);
    let mut start = task.start;
    let mut made: Vec<Split> = Vec::new();
    for _ in 0..10_000 {
        if start > past {
            // No urgent work out here: leveling's own search, which knows when to stop.
            return (
                earliest_fit(project, lookup, task, resources, busy),
                Vec::new(),
            );
        }
        if start > limit {
            break;
        }
        let mut splits = task.splits.clone();
        splits.extend(made.iter().cloned());
        split::normalize(&mut splits, duration);
        let intervals: Vec<(i64, i64)> = split::layout(calendar, start, duration, &splits)
            .iter()
            .map(|segment| (segment.start, segment.finish))
            .collect();
        let mut pieces = Vec::new();
        cut_to_calendar(calendar, &intervals, 0.0, NO_OWNER, &mut pieces);
        let clash = checked
            .iter()
            .filter_map(|resource| {
                let (from, to, only_urgent) = clash_by_whom(resource, &pieces, urgent)?;
                Some((from, to, only_urgent, resource))
            })
            .min_by_key(|(from, to, ..)| (*from, *to));
        let Some((from, to, only_urgent, resource)) = clash else {
            return (start, made);
        };
        if only_urgent {
            let worked = split::working_within(calendar, &intervals, start, from);
            if worked > 0 && worked < duration {
                let gap = calendar.working_between(from, to).max(1);
                match made.iter_mut().find(|split| split.offset == worked) {
                    Some(split) => split.gap += gap,
                    None => made.push(Split {
                        offset: worked,
                        gap,
                        leveling: true,
                    }),
                }
                continue;
            }
        }
        // Ordinary work in the way (or urgent work at the very start): move the whole task.
        made.clear();
        let room = room_from(resource, to);
        start = calendar.add_lag(start, calendar.working_between(from, room).max(1));
    }
    (task.start, Vec::new())
}

/// The first stretch in which `pieces` do not fit next to the resource's load, and
/// whether they would without the work of the tasks `urgent` names.
fn clash_by_whom(
    resource: &Checked,
    pieces: &[Piece],
    urgent: &dyn Fn(usize) -> bool,
) -> Option<(i64, i64, bool)> {
    let (calendar, others, units) = (resource.calendar, &resource.placed.pieces, resource.units);
    for piece in pieces {
        let (date, _) = civil(piece.start);
        let overlapping: Vec<&Piece> = others
            .iter()
            .filter(|other| other.start < piece.end && other.end > piece.start)
            .collect();
        let mut cuts = vec![piece.start, piece.end];
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
        let max_units = resource.capacity.units_on(date);
        for window in cuts.windows(2) {
            let (from, to) = (window[0], window[1]);
            let works = ranges
                .iter()
                .any(|(start, end)| *start <= from && to <= *end);
            let available = if works { max_units } else { 0.0 };
            let (mut load, mut ordinary) = (0.0f64, 0.0f64);
            for other in overlapping
                .iter()
                .filter(|other| other.start < to && other.end > from)
            {
                load += other.units;
                if !urgent(other.owner) {
                    ordinary += other.units;
                }
            }
            if load + units > available + EPSILON {
                let only_urgent = available > EPSILON && ordinary + units <= available + EPSILON;
                return Some((from, to, only_urgent));
            }
        }
    }
    None
}

/// Sweep the load of `resources` again from the placed tasks' current dates.
pub(super) fn rebuild(
    project: &Project,
    lookup: &Lookup,
    placed: &[usize],
    resources: &BTreeSet<u32>,
    busy: &mut BTreeMap<u32, Busy>,
) {
    let mut pieces: BTreeMap<u32, Vec<Piece>> =
        resources.iter().map(|uid| (*uid, Vec::new())).collect();
    for &index in placed {
        for &(resource_uid, units) in &lookup.units[index] {
            if let Some(into) = pieces.get_mut(&resource_uid) {
                working_pieces(project, &project.tasks[index], units, index, into);
            }
        }
    }
    for (resource_uid, pieces) in pieces {
        let Some(resource) = lookup.resource(project, resource_uid) else {
            continue;
        };
        let calendar = project.resource_calendar(resource);
        let capacity = Capacity::of(resource);
        let stretches = stretches(
            &pieces,
            |date| working_ranges(calendar, date),
            |date| capacity.units_on(date),
        );
        let until = pieces.iter().map(|piece| piece.end).max().unwrap_or(0);
        busy.insert(
            resource_uid,
            Busy {
                pieces,
                stretches,
                until,
            },
        );
    }
}
