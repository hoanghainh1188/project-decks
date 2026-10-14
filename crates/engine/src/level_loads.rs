//! The work each resource already has, for placing new work next to it the way leveling
//! does (`suggest.rs`). A child of `level.rs`, so it shares its sweep and search.

use std::collections::BTreeMap;

use super::{
    first_clash, fit_from, free_from, stretches, working_pieces, working_ranges, Busy, Checked,
    Lookup, EPSILON, NO_OWNER,
};
use crate::availability::Capacity;
use crate::model::{Project, Task};

/// Every resource's assigned work at the project's current dates, plus work added since.
pub(crate) struct Loads {
    lookup: Lookup,
    busy: BTreeMap<u32, Busy>,
}

impl Loads {
    pub(crate) fn of(project: &Project) -> Self {
        let lookup = Lookup::new(project);
        let mut busy: BTreeMap<u32, Busy> = BTreeMap::new();
        for (index, task) in project.tasks.iter().enumerate() {
            if task.summary {
                continue;
            }
            for &(resource_uid, units) in &lookup.units[index] {
                let entry = busy.entry(resource_uid).or_default();
                working_pieces(project, task, units, NO_OWNER, &mut entry.pieces);
            }
        }
        let mut loads = Self { lookup, busy };
        let uids: Vec<u32> = loads.busy.keys().copied().collect();
        for uid in uids {
            loads.sweep(project, uid);
        }
        loads
    }

    /// The first start from `task.start` at which the task's work, at its own duration and
    /// `units` of the resource, fits next to that resource's work; `None` when the resource
    /// will never have the units or the work does not fit in four years.
    pub(crate) fn fit(
        &self,
        project: &Project,
        task: &Task,
        resource_uid: u32,
        units: f64,
    ) -> Option<i64> {
        let resource = self.lookup.resource(project, resource_uid)?;
        let (first_day, _) = crate::timeutil::civil(task.start);
        if units > Capacity::of(resource).most_from(first_day) + EPSILON {
            return None;
        }
        fit_from(
            project,
            &self.lookup,
            task,
            &[(resource_uid, units)],
            &self.busy,
        )
    }

    /// Whether `task`'s work, at its own dates and `units` of the resource, fits next to
    /// that resource's work without moving: for tasks leveling would not move either.
    pub(crate) fn fits_at(
        &self,
        project: &Project,
        task: &Task,
        resource_uid: u32,
        units: f64,
    ) -> bool {
        let Some(resource) = self.lookup.resource(project, resource_uid) else {
            return false;
        };
        let empty = Busy::default();
        let checked = Checked {
            units,
            placed: self.busy.get(&resource_uid).unwrap_or(&empty),
            calendar: project.resource_calendar(resource),
            capacity: Capacity::of(resource),
        };
        let mut pieces = Vec::new();
        working_pieces(project, task, 0.0, NO_OWNER, &mut pieces);
        first_clash(&checked, &pieces).is_none()
    }

    /// No start from `at` on can fit before this: the first instant at which the resource
    /// works and has room for `units` more, next to its load.
    pub(crate) fn earliest_room(
        &self,
        project: &Project,
        resource_uid: u32,
        units: f64,
        at: i64,
    ) -> i64 {
        let Some(resource) = self.lookup.resource(project, resource_uid) else {
            return at;
        };
        let empty = Busy::default();
        let placed = self.busy.get(&resource_uid).unwrap_or(&empty);
        let checked = Checked {
            units,
            placed,
            calendar: project.resource_calendar(resource),
            capacity: Capacity::of(resource),
        };
        let stretches = &placed.stretches;
        // Before the load there is none: the room is where the calendar and capacity allow.
        let loaded_from = stretches.first().map_or(i64::MAX, |stretch| stretch.start);
        if at < loaded_from {
            let free = free_from(&checked, at);
            if free < loaded_from {
                return free;
            }
        }
        // The stretches run on with no gap, from the one holding `at`.
        let from = stretches
            .partition_point(|stretch| stretch.start <= at)
            .saturating_sub(1);
        let room = stretches[from..].iter().find(|stretch| {
            stretch.available > EPSILON && stretch.load + units <= stretch.available + EPSILON
        });
        match room {
            Some(stretch) => stretch.start.max(at),
            None => free_from(&checked, placed.until.max(at)),
        }
    }

    /// Add `task`'s work, at its dates and `units`, to the resource's load.
    pub(crate) fn add(&mut self, project: &Project, task: &Task, resource_uid: u32, units: f64) {
        if self.lookup.resource(project, resource_uid).is_none() {
            return;
        }
        let entry = self.busy.entry(resource_uid).or_default();
        working_pieces(project, task, units, NO_OWNER, &mut entry.pieces);
        self.sweep(project, resource_uid);
    }

    /// Rebuild the swept load of one resource from its pieces.
    fn sweep(&mut self, project: &Project, resource_uid: u32) {
        let Some(resource) = self.lookup.resource(project, resource_uid) else {
            return;
        };
        let Some(entry) = self.busy.get_mut(&resource_uid) else {
            return;
        };
        let calendar = project.resource_calendar(resource);
        let capacity = Capacity::of(resource);
        entry.stretches = stretches(
            &entry.pieces,
            |date| working_ranges(calendar, date),
            |date| capacity.units_on(date),
        );
        entry.until = entry
            .pieces
            .iter()
            .map(|piece| piece.end)
            .max()
            .unwrap_or(0);
    }
}
