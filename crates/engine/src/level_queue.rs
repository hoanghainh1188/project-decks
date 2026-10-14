//! The order leveling places leaves in: a heap of keys. A child of `level.rs`.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

use super::levelable;
use crate::graph::Graph;
use crate::level_order::{self, Parts};
use crate::model::Project;

/// What decides which leaf is placed next, smallest first: tasks leveling will not
/// delay come first, since their place is fixed and the others must fit around them;
/// then tasks whose leaf predecessors are all placed, so a predecessor delayed later
/// cannot push a placed task; then the project's leveling order (by default the
/// earliest start, higher priority, less slack); then the lower uid. Slack is the one
/// before leveling: it follows the links, not the order leveling happens to place tasks
/// in.
#[derive(Clone, Debug)]
pub(super) struct Key {
    pub(super) levelable: bool,
    pub(super) waiting: bool,
    /// The task's start when the key was made: the one criterion leveling moves.
    pub(super) start: i64,
    pub(super) parts: Parts,
    pub(super) uid: u32,
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.levelable
            .cmp(&other.levelable)
            .then(self.waiting.cmp(&other.waiting))
            .then_with(|| self.parts.cmp(self.start, &other.parts, other.start))
            .then(self.uid.cmp(&other.uid))
    }
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Key {}

/// The leaves still to place, in a heap keyed by `Key`. A leaf whose key changes (its
/// start moved, or its last predecessor was placed) is pushed again with a new stamp;
/// an entry whose stamp is old is skipped when it comes out.
pub(super) struct Order {
    heap: BinaryHeap<Reverse<(Key, usize, u64)>>,
    /// The stamp of each leaf's current entry, or `None` once placed (and for summaries).
    stamp: Vec<Option<u64>>,
    next_stamp: u64,
    /// The key parts that do not change while leveling, built once.
    parts: Vec<Option<Parts>>,
    /// Links from leaf predecessors not placed yet.
    waiting_on: Vec<usize>,
    /// Leaf successors through links between leaves, once per link.
    successors: Vec<Vec<usize>>,
}

impl Order {
    pub(super) fn new(project: &Project, graph: &Graph) -> Self {
        let count = project.tasks.len();
        let mut waiting_on = vec![0; count];
        let mut successors = vec![Vec::new(); count];
        for dep in &project.dependencies {
            let (Some(pred), Some(succ)) = (
                graph.position(dep.predecessor_uid),
                graph.position(dep.successor_uid),
            ) else {
                continue;
            };
            if !project.tasks[pred].summary && !project.tasks[succ].summary {
                waiting_on[succ] += 1;
                successors[pred].push(succ);
            }
        }
        let keys = level_order::effective(project);
        // Read after the delays of movable tasks were cleared; a task leveling may not
        // move keeps its delay, and the slack that comes with it.
        let parts = project
            .tasks
            .iter()
            .map(|task| (!task.summary).then(|| Parts::of(project, &keys, task, task.total_slack)))
            .collect();
        let mut order = Self {
            heap: BinaryHeap::with_capacity(count),
            stamp: vec![None; count],
            next_stamp: 0,
            parts,
            waiting_on,
            successors,
        };
        for index in 0..count {
            if !project.tasks[index].summary {
                order.refresh(project, index);
            }
        }
        order
    }

    fn refresh(&mut self, project: &Project, index: usize) {
        let task = &project.tasks[index];
        let Some(parts) = self.parts[index].clone() else {
            return;
        };
        let key = Key {
            levelable: levelable(task),
            waiting: self.waiting_on[index] > 0,
            start: task.start,
            parts,
            uid: task.uid,
        };
        let stamp = self.next_stamp;
        self.next_stamp += 1;
        self.stamp[index] = Some(stamp);
        self.heap.push(Reverse((key, index, stamp)));
    }

    /// Whether the leaf at `a` is more urgent than the one at `b` by the leveling order,
    /// start aside: the order's criteria other than the start rank them, a tie does not.
    pub(super) fn ranks_above(&self, a: usize, b: usize) -> bool {
        match (&self.parts[a], &self.parts[b]) {
            (Some(a), Some(b)) => a.cmp(0, b, 0) == Ordering::Less,
            _ => false,
        }
    }

    /// The next leaf to place, taken out of the order.
    pub(super) fn next(&mut self, project: &Project) -> Option<usize> {
        while let Some(Reverse((_, index, stamp))) = self.heap.pop() {
            if self.stamp[index] == Some(stamp) {
                self.stamp[index] = None;
                debug_assert!(!project.tasks[index].summary);
                return Some(index);
            }
        }
        None
    }

    /// Note that the leaf at `index` is placed and that the leaves in `moved` have new
    /// dates.
    pub(super) fn placed(
        &mut self,
        project: &Project,
        index: usize,
        moved: impl Iterator<Item = usize>,
    ) {
        let mut stale: Vec<usize> = moved.collect();
        for position in 0..self.successors[index].len() {
            let successor = self.successors[index][position];
            self.waiting_on[successor] -= 1;
            stale.push(successor);
        }
        for position in stale {
            if self.stamp[position].is_some() {
                self.refresh(project, position);
            }
        }
    }
}
