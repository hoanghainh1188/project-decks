//! Lookups that one `schedule()` run shares: UID to position, outline subtrees,
//! links per task, and the leaf order for the CPM passes.
//!
//! The outline and the links do not change while a schedule runs, so this is built
//! once after `normalize_outline` instead of scanning the task and link lists for
//! every leaf.

use std::collections::{HashMap, VecDeque};

use crate::error::EngineError;
use crate::model::Project;

pub(crate) struct Graph {
    index: HashMap<u32, usize>,
    /// Exclusive end of each task's subtree in outline order.
    subtree_end: Vec<usize>,
    /// Link indexes where the task is the successor, in link order.
    preds: Vec<Vec<usize>>,
    /// Link indexes where the task is the predecessor, in link order.
    succs: Vec<Vec<usize>>,
    /// Leaf task positions, every predecessor before its successors.
    order: Vec<usize>,
}

impl Graph {
    /// Fails with `DependencyCycle` when the links loop, including a link between a
    /// summary and one of its own leaves.
    pub(crate) fn build(project: &Project) -> Result<Self, EngineError> {
        let count = project.tasks.len();
        let index: HashMap<u32, usize> = project
            .tasks
            .iter()
            .enumerate()
            .map(|(position, task)| (task.uid, position))
            .collect();
        let subtree_end = subtree_ends(project);
        let mut preds = vec![Vec::new(); count];
        let mut succs = vec![Vec::new(); count];
        for (link, dep) in project.dependencies.iter().enumerate() {
            let (Some(&pred), Some(&succ)) = (
                index.get(&dep.predecessor_uid),
                index.get(&dep.successor_uid),
            ) else {
                continue;
            };
            succs[pred].push(link);
            preds[succ].push(link);
        }
        let mut graph = Self {
            index,
            subtree_end,
            preds,
            succs,
            order: Vec::new(),
        };
        graph.order = graph.leaf_order(project)?;
        Ok(graph)
    }

    pub(crate) fn position(&self, uid: u32) -> Option<usize> {
        self.index.get(&uid).copied()
    }

    pub(crate) fn order(&self) -> &[usize] {
        &self.order
    }

    pub(crate) fn preds(&self, position: usize) -> &[usize] {
        &self.preds[position]
    }

    pub(crate) fn succs(&self, position: usize) -> &[usize] {
        &self.succs[position]
    }

    /// Positions of the leaves under a summary, in outline order.
    pub(crate) fn leaves_under<'a>(
        &self,
        project: &'a Project,
        summary: usize,
    ) -> impl Iterator<Item = usize> + 'a {
        (summary + 1..self.subtree_end[summary])
            .filter(|&position| !project.tasks[position].summary)
    }

    /// Kahn's order over leaves plus two virtual nodes per summary: every leaf of a
    /// summary feeds its "out" node, and its "in" node feeds every leaf. A link then
    /// costs one edge however large the groups are.
    fn leaf_order(&self, project: &Project) -> Result<Vec<usize>, EngineError> {
        let count = project.tasks.len();
        let out_node = |position: usize| count + position;
        let in_node = |position: usize| 2 * count + position;
        let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); 3 * count];
        for (position, task) in project.tasks.iter().enumerate() {
            if !task.summary {
                continue;
            }
            for leaf in self.leaves_under(project, position) {
                adjacency[leaf].push(out_node(position));
                adjacency[in_node(position)].push(leaf);
            }
        }
        for dep in &project.dependencies {
            let (Some(pred), Some(succ)) = (
                self.position(dep.predecessor_uid),
                self.position(dep.successor_uid),
            ) else {
                continue;
            };
            let from = if project.tasks[pred].summary {
                out_node(pred)
            } else {
                pred
            };
            let to = if project.tasks[succ].summary {
                in_node(succ)
            } else {
                succ
            };
            adjacency[from].push(to);
        }

        let mut indegree = vec![0usize; 3 * count];
        for targets in &adjacency {
            for &target in targets {
                indegree[target] += 1;
            }
        }
        let mut queue: VecDeque<usize> =
            (0..3 * count).filter(|&node| indegree[node] == 0).collect();
        let mut visited = 0;
        let mut order = Vec::with_capacity(count);
        while let Some(node) = queue.pop_front() {
            visited += 1;
            if node < count && !project.tasks[node].summary {
                order.push(node);
            }
            for &next in &adjacency[node] {
                indegree[next] -= 1;
                if indegree[next] == 0 {
                    queue.push_back(next);
                }
            }
        }
        if visited == 3 * count {
            Ok(order)
        } else {
            Err(EngineError::DependencyCycle)
        }
    }
}

fn subtree_ends(project: &Project) -> Vec<usize> {
    let count = project.tasks.len();
    let mut ends = vec![count; count];
    let mut open: Vec<usize> = Vec::new();
    for (position, task) in project.tasks.iter().enumerate() {
        while let Some(&top) = open.last() {
            if project.tasks[top].outline_level >= task.outline_level {
                ends[top] = position;
                open.pop();
            } else {
                break;
            }
        }
        open.push(position);
    }
    ends
}
