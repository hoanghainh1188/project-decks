//! Network diagram (PERT) layout.
//!
//! Columns follow dependency depth (longest path from a task with no shown
//! predecessor). Rows come from a layered heuristic: edges spanning several
//! columns get waypoints, then barycenter sweeps reorder each column to reduce
//! crossings. Coordinates are pixels at zoom 1; the UI only draws them.
//!
//! A recurring series is one box, its summary, so a weekly meeting does not fill the
//! first column with every occurrence. With summaries hidden, a link to or from a
//! summary is drawn to the tasks inside it: from those that end the group (no
//! successor inside it) and to those that start it (no predecessor inside it).

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::model::{DepKind, Project};

pub const BOX_WIDTH: f64 = 220.0;
pub const BOX_HEIGHT: f64 = 104.0;
pub const COLUMN_GAP: f64 = 80.0;
pub const ROW_GAP: f64 = 28.0;
pub const MARGIN: f64 = 24.0;
/// Space kept between a waypoint and its neighbours in a column.
const WAYPOINT_GAP: f64 = 14.0;
const MAX_SWEEPS: usize = 16;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct NetworkOptions {
    #[serde(default)]
    pub include_summary: bool,
    /// When set, only these task UIDs are laid out (view filters).
    #[serde(default)]
    pub visible_uids: Option<Vec<u32>>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NetworkNode {
    pub uid: u32,
    pub column: usize,
    /// Rank of the box inside its column, top to bottom.
    pub row: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NetworkEdge {
    /// The link drawn. A link to a hidden summary can be drawn as several edges.
    pub uid: u32,
    /// The boxes the edge joins, which differ from the link's own tasks when it was
    /// moved down from a hidden summary or up to a recurring series.
    pub from_uid: u32,
    pub to_uid: u32,
    pub kind: DepKind,
    pub lag: i64,
    /// Polyline from the predecessor's right side to the successor's left side.
    pub points: Vec<Point>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NetworkLayout {
    pub nodes: Vec<NetworkNode>,
    pub edges: Vec<NetworkEdge>,
    pub columns: usize,
    pub width: f64,
    pub height: f64,
    pub crossings: usize,
}

pub fn layout(project: &Project, options: &NetworkOptions) -> NetworkLayout {
    let allowed: Option<HashSet<u32>> = options
        .visible_uids
        .as_ref()
        .map(|uids| uids.iter().copied().collect());
    let boxes = Boxes::new(project, options.include_summary);
    // A series box shows when its summary or any of its occurrences passes the filter.
    let shown: HashSet<u32> = project
        .tasks
        .iter()
        .filter(|task| allowed.as_ref().is_none_or(|set| set.contains(&task.uid)))
        .filter_map(|task| boxes.drawn_as(project, task.uid))
        .collect();
    let tasks: Vec<u32> = project
        .tasks
        .iter()
        .filter(|task| {
            boxes.drawn_as(project, task.uid) == Some(task.uid) && shown.contains(&task.uid)
        })
        .map(|task| task.uid)
        .collect();
    let index: HashMap<u32, usize> = tasks.iter().enumerate().map(|(i, uid)| (*uid, i)).collect();
    let n = tasks.len();

    let mut seen = HashSet::new();
    let mut deps: Vec<(usize, usize, usize)> = Vec::new();
    for (dep_index, dep) in project.dependencies.iter().enumerate() {
        let froms = boxes.ends(project, dep.predecessor_uid, Side::Exit, &index);
        let tos = boxes.ends(project, dep.successor_uid, Side::Entry, &index);
        for &from in &froms {
            for &to in &tos {
                if from != to && seen.insert((from, to)) {
                    deps.push((dep_index, from, to));
                }
            }
        }
    }

    let depth = depths(n, &deps);
    // Scheduling rejects cycles, so every kept edge points to a deeper column.
    let deps: Vec<(usize, usize, usize)> = deps
        .into_iter()
        .filter(|(_, from, to)| depth[*from] < depth[*to])
        .collect();
    let columns = depth.iter().copied().max().map_or(0, |max| max + 1);

    // Vertices 0..n are tasks; the rest are waypoints of long edges.
    let mut layer_of: Vec<usize> = depth.clone();
    let mut chains: Vec<Vec<usize>> = Vec::with_capacity(deps.len());
    for (_, from, to) in &deps {
        let mut chain = vec![*from];
        for layer in depth[*from] + 1..depth[*to] {
            layer_of.push(layer);
            chain.push(layer_of.len() - 1);
        }
        chain.push(*to);
        chains.push(chain);
    }
    let vertex_count = layer_of.len();
    let mut up: Vec<Vec<usize>> = vec![Vec::new(); vertex_count];
    let mut down: Vec<Vec<usize>> = vec![Vec::new(); vertex_count];
    for chain in &chains {
        for pair in chain.windows(2) {
            down[pair[0]].push(pair[1]);
            up[pair[1]].push(pair[0]);
        }
    }

    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); columns];
    for (vertex, layer) in layer_of.iter().enumerate() {
        layers[*layer].push(vertex);
    }
    let crossings = order_layers(&mut layers, &up, &down, vertex_count);

    // Vertical placement: follow the predecessors' centre when there is room,
    // otherwise stack below the previous item. This never overlaps.
    let height_of = |vertex: usize| if vertex < n { BOX_HEIGHT } else { 0.0 };
    let mut top = vec![0.0f64; vertex_count];
    for layer in &layers {
        let mut cursor = MARGIN;
        let mut previous: Option<usize> = None;
        for &vertex in layer {
            let h = height_of(vertex);
            if let Some(prev) = previous {
                let gap = if prev < n && vertex < n {
                    ROW_GAP
                } else {
                    WAYPOINT_GAP
                };
                cursor = top[prev] + height_of(prev) + gap;
            }
            let desired = if up[vertex].is_empty() {
                cursor
            } else {
                let sum: f64 = up[vertex]
                    .iter()
                    .map(|u| top[*u] + height_of(*u) / 2.0)
                    .sum();
                sum / up[vertex].len() as f64 - h / 2.0
            };
            top[vertex] = desired.max(cursor);
            previous = Some(vertex);
        }
    }
    let left = |layer: usize| MARGIN + layer as f64 * (BOX_WIDTH + COLUMN_GAP);

    let mut nodes: Vec<NetworkNode> = Vec::with_capacity(n);
    for layer in &layers {
        let mut row = 0;
        for &vertex in layer {
            if vertex >= n {
                continue;
            }
            nodes.push(NetworkNode {
                uid: tasks[vertex],
                column: layer_of[vertex],
                row,
                x: left(layer_of[vertex]),
                y: top[vertex],
                width: BOX_WIDTH,
                height: BOX_HEIGHT,
            });
            row += 1;
        }
    }

    let edges = deps
        .iter()
        .zip(&chains)
        .map(|((dep_index, from, to), chain)| {
            let dep = &project.dependencies[*dep_index];
            let mut points = Vec::with_capacity(chain.len() * 2);
            points.push(Point {
                x: left(layer_of[*from]) + BOX_WIDTH,
                y: top[*from] + BOX_HEIGHT / 2.0,
            });
            for &waypoint in &chain[1..chain.len() - 1] {
                let x = left(layer_of[waypoint]);
                points.push(Point {
                    x,
                    y: top[waypoint],
                });
                points.push(Point {
                    x: x + BOX_WIDTH,
                    y: top[waypoint],
                });
            }
            points.push(Point {
                x: left(layer_of[*to]),
                y: top[*to] + BOX_HEIGHT / 2.0,
            });
            NetworkEdge {
                uid: dep.uid,
                from_uid: tasks[*from],
                to_uid: tasks[*to],
                kind: dep.kind,
                lag: dep.lag,
                points,
            }
        })
        .collect();

    let width = if columns == 0 {
        0.0
    } else {
        left(columns - 1) + BOX_WIDTH + MARGIN
    };
    let height = (0..vertex_count)
        .map(|vertex| top[vertex] + height_of(vertex))
        .fold(0.0f64, f64::max)
        + if vertex_count == 0 { 0.0 } else { MARGIN };
    NetworkLayout {
        nodes,
        edges,
        columns,
        width,
        height,
        crossings,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    /// The predecessor end of a link: the tasks that end a group.
    Exit,
    /// The successor end of a link: the tasks that start a group.
    Entry,
}

/// Which box stands for each task.
struct Boxes {
    position: HashMap<u32, usize>,
    series: HashSet<u32>,
    include_summary: bool,
}

impl Boxes {
    fn new(project: &Project, include_summary: bool) -> Self {
        Self {
            position: project
                .tasks
                .iter()
                .enumerate()
                .map(|(i, task)| (task.uid, i))
                .collect(),
            series: project
                .tasks
                .iter()
                .filter(|task| task.recurrence.is_some())
                .map(|task| task.uid)
                .collect(),
            include_summary,
        }
    }

    /// The box a task is drawn as: itself, its recurring series, or none for a hidden
    /// summary.
    fn drawn_as(&self, project: &Project, uid: u32) -> Option<u32> {
        self.box_of(project, *self.position.get(&uid)?)
    }

    /// Box positions a link end is drawn at. A hidden summary gives the boxes inside it
    /// that end the group (`Exit`) or start it (`Entry`).
    fn ends(
        &self,
        project: &Project,
        uid: u32,
        side: Side,
        index: &HashMap<u32, usize>,
    ) -> Vec<usize> {
        let Some(&position) = self.position.get(&uid) else {
            return Vec::new();
        };
        if let Some(drawn) = self.box_of(project, position) {
            return index.get(&drawn).copied().into_iter().collect();
        }
        let level = project.tasks[position].outline_level;
        let mut inside: Vec<usize> = project.tasks[position + 1..]
            .iter()
            .take_while(|child| child.outline_level > level)
            .enumerate()
            .filter_map(|(offset, _)| self.box_of(project, position + 1 + offset))
            .filter_map(|drawn| index.get(&drawn).copied())
            .collect();
        let mut once = HashSet::new();
        inside.retain(|member| once.insert(*member));
        let members: HashSet<usize> = inside.iter().copied().collect();
        let linked: Vec<(usize, usize)> = project
            .dependencies
            .iter()
            .filter_map(|dep| {
                let from = self.box_of(project, *self.position.get(&dep.predecessor_uid)?)?;
                let to = self.box_of(project, *self.position.get(&dep.successor_uid)?)?;
                Some((*index.get(&from)?, *index.get(&to)?))
            })
            .filter(|(from, to)| from != to && members.contains(from) && members.contains(to))
            .collect();
        let ends: Vec<usize> = inside
            .iter()
            .copied()
            .filter(|member| match side {
                Side::Exit => !linked.iter().any(|(from, _)| from == member),
                Side::Entry => !linked.iter().any(|(_, to)| to == member),
            })
            .collect();
        if ends.is_empty() {
            inside
        } else {
            ends
        }
    }

    /// The box the task at `position` is drawn as.
    fn box_of(&self, project: &Project, position: usize) -> Option<u32> {
        let task = &project.tasks[position];
        if let Some(series) = task.series_uid.filter(|uid| self.series.contains(uid)) {
            return Some(series);
        }
        if task.summary && !self.include_summary && !self.series.contains(&task.uid) {
            return None;
        }
        Some(task.uid)
    }
}

/// Longest-path depth over a DAG, visiting roots in task order.
/// Nodes left over by a cycle (not reachable after scheduling) go one column past the deepest.
fn depths(n: usize, deps: &[(usize, usize, usize)]) -> Vec<usize> {
    let mut successors: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut indegree = vec![0usize; n];
    for (_, from, to) in deps {
        successors[*from].push(*to);
        indegree[*to] += 1;
    }
    let mut depth = vec![0usize; n];
    let mut done = vec![false; n];
    let mut queue: VecDeque<usize> = (0..n).filter(|i| indegree[*i] == 0).collect();
    while let Some(vertex) = queue.pop_front() {
        done[vertex] = true;
        for &next in &successors[vertex] {
            depth[next] = depth[next].max(depth[vertex] + 1);
            indegree[next] -= 1;
            if indegree[next] == 0 {
                queue.push_back(next);
            }
        }
    }
    if done.iter().any(|visited| !visited) {
        let base = depth.iter().copied().max().unwrap_or(0) + 1;
        for vertex in 0..n {
            if !done[vertex] {
                depth[vertex] = base;
            }
        }
    }
    depth
}

/// Barycenter sweeps (down then up), keeping the order with the fewest crossings.
fn order_layers(
    layers: &mut [Vec<usize>],
    up: &[Vec<usize>],
    down: &[Vec<usize>],
    vertex_count: usize,
) -> usize {
    let mut position = vec![0usize; vertex_count];
    let refresh = |layers: &[Vec<usize>], position: &mut [usize]| {
        for layer in layers {
            for (rank, vertex) in layer.iter().enumerate() {
                position[*vertex] = rank;
            }
        }
    };
    refresh(layers, &mut position);
    let mut best = layers.to_vec();
    let mut best_crossings = total_crossings(layers, down, &position);
    for _ in 0..MAX_SWEEPS {
        if best_crossings == 0 {
            break;
        }
        for layer in layers.iter_mut().skip(1) {
            sort_by_barycenter(layer, up, &mut position);
        }
        for layer in (0..layers.len().saturating_sub(1)).rev() {
            sort_by_barycenter(&mut layers[layer], down, &mut position);
        }
        let crossings = total_crossings(layers, down, &position);
        if crossings < best_crossings {
            best_crossings = crossings;
            best = layers.to_vec();
        } else {
            break;
        }
    }
    layers.clone_from_slice(&best);
    best_crossings
}

fn sort_by_barycenter(layer: &mut [usize], neighbours: &[Vec<usize>], position: &mut [usize]) {
    let mut keyed: Vec<(f64, usize, usize)> = layer
        .iter()
        .enumerate()
        .map(|(rank, vertex)| {
            let list = &neighbours[*vertex];
            let key = if list.is_empty() {
                rank as f64
            } else {
                list.iter().map(|u| position[*u] as f64).sum::<f64>() / list.len() as f64
            };
            (key, rank, *vertex)
        })
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    for (rank, (_, _, vertex)) in keyed.into_iter().enumerate() {
        layer[rank] = vertex;
        position[vertex] = rank;
    }
}

fn total_crossings(layers: &[Vec<usize>], down: &[Vec<usize>], position: &[usize]) -> usize {
    let mut total = 0;
    for layer in layers {
        let mut segments: Vec<(usize, usize)> = layer
            .iter()
            .flat_map(|u| down[*u].iter().map(move |v| (position[*u], position[*v])))
            .collect();
        segments.sort_unstable();
        for i in 0..segments.len() {
            for j in i + 1..segments.len() {
                if segments[i].0 < segments[j].0 && segments[i].1 > segments[j].1 {
                    total += 1;
                }
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Dependency, Task};

    fn project(tasks: &[(u32, bool)], deps: &[(u32, u32)]) -> Project {
        let mut project = Project::new("Mạng", 0);
        for (uid, summary) in tasks {
            let mut task = Task::new(*uid, format!("T{uid}"));
            task.summary = *summary;
            project.tasks.push(task);
        }
        for (index, (from, to)) in deps.iter().enumerate() {
            project.dependencies.push(Dependency {
                uid: index as u32 + 1,
                predecessor_uid: *from,
                successor_uid: *to,
                kind: DepKind::Fs,
                lag: 0,
            });
        }
        project
    }

    fn node(layout: &NetworkLayout, uid: u32) -> &NetworkNode {
        layout
            .nodes
            .iter()
            .find(|node| node.uid == uid)
            .expect("node")
    }

    #[test]
    fn columns_follow_longest_dependency_path() {
        let p = project(
            &[(1, false), (2, false), (3, false), (4, false), (5, false)],
            &[(1, 2), (2, 3), (1, 3), (4, 3)],
        );
        let layout = layout(&p, &NetworkOptions::default());
        assert_eq!(node(&layout, 1).column, 0);
        assert_eq!(node(&layout, 2).column, 1);
        assert_eq!(node(&layout, 3).column, 2);
        assert_eq!(node(&layout, 4).column, 0);
        assert_eq!(
            node(&layout, 5).column,
            0,
            "unlinked task starts the diagram"
        );
        assert_eq!(layout.columns, 3);
        for node in &layout.nodes {
            assert_eq!(
                node.x,
                MARGIN + node.column as f64 * (BOX_WIDTH + COLUMN_GAP)
            );
        }
        // 1 -> 3 skips column 1, so it passes a waypoint there.
        let long = layout
            .edges
            .iter()
            .find(|e| e.from_uid == 1 && e.to_uid == 3)
            .unwrap();
        assert_eq!(long.points.len(), 4);
    }

    #[test]
    fn barycenter_removes_simple_crossing() {
        // Task order alone would cross: 1 -> 4 and 2 -> 3.
        let p = project(
            &[(1, false), (2, false), (3, false), (4, false)],
            &[(1, 4), (2, 3)],
        );
        let layout = layout(&p, &NetworkOptions::default());
        assert_eq!(layout.crossings, 0);
        assert!(node(&layout, 4).y < node(&layout, 3).y);
    }

    #[test]
    fn summaries_hidden_unless_requested() {
        let p = project(&[(1, true), (2, false), (3, false)], &[(1, 3), (2, 3)]);
        let hidden = layout(&p, &NetworkOptions::default());
        assert!(hidden.nodes.iter().all(|node| node.uid != 1));
        assert!(hidden.edges.iter().all(|edge| edge.from_uid != 1));
        let shown = layout(
            &p,
            &NetworkOptions {
                include_summary: true,
                visible_uids: None,
            },
        );
        assert!(shown.nodes.iter().any(|node| node.uid == 1));
        assert_eq!(shown.edges.len(), 2);
    }

    /// Tasks given as (uid, outline level); a task is a summary when the next is deeper.
    fn outline(tasks: &[(u32, u32)], deps: &[(u32, u32)]) -> Project {
        let mut p = project(
            &tasks
                .iter()
                .map(|(uid, _)| (*uid, false))
                .collect::<Vec<_>>(),
            deps,
        );
        for (task, (_, level)) in p.tasks.iter_mut().zip(tasks) {
            task.outline_level = *level;
        }
        for index in 0..p.tasks.len() {
            let next = p.tasks.get(index + 1).map(|task| task.outline_level);
            p.tasks[index].summary = next.is_some_and(|level| level > p.tasks[index].outline_level);
        }
        p
    }

    fn links(layout: &NetworkLayout) -> Vec<(u32, u32)> {
        let mut links: Vec<(u32, u32)> = layout
            .edges
            .iter()
            .map(|e| (e.from_uid, e.to_uid))
            .collect();
        links.sort_unstable();
        links
    }

    #[test]
    fn links_of_a_hidden_summary_move_to_the_tasks_inside() {
        // Group 1 holds 2 -> 3; the group is linked to 4 and from 5.
        let p = outline(
            &[(5, 1), (1, 1), (2, 2), (3, 2), (4, 1)],
            &[(2, 3), (1, 4), (5, 1)],
        );
        let hidden = layout(&p, &NetworkOptions::default());
        assert_eq!(links(&hidden), vec![(2, 3), (3, 4), (5, 2)]);
        let shown = layout(
            &p,
            &NetworkOptions {
                include_summary: true,
                visible_uids: None,
            },
        );
        assert_eq!(links(&shown), vec![(1, 4), (2, 3), (5, 1)]);
    }

    #[test]
    fn a_recurring_series_is_one_box() {
        use crate::recurrence::{Recurrence, RecurrenceEnd, RecurrencePattern, RecurrenceSpec};
        // Series 10 with occurrences 11 and 12; the last meeting leads to task 1.
        let mut p = outline(&[(1, 1), (10, 1), (11, 2), (12, 2)], &[(12, 1)]);
        p.tasks[1].recurrence = Some(Recurrence {
            spec: RecurrenceSpec {
                name: "Họp".into(),
                duration: 60,
                start: 0,
                pattern: RecurrencePattern::Daily { every: 1 },
                end: RecurrenceEnd::Count { count: 2 },
                calendar_id: None,
                resource_uids: Vec::new(),
                resources: Vec::new(),
                background: false,
            },
            skipped: Vec::new(),
        });
        for task in &mut p.tasks[2..] {
            task.series_uid = Some(10);
        }
        let diagram = layout(&p, &NetworkOptions::default());
        let mut uids: Vec<u32> = diagram.nodes.iter().map(|node| node.uid).collect();
        uids.sort_unstable();
        assert_eq!(uids, vec![1, 10]);
        assert_eq!(links(&diagram), vec![(10, 1)]);
        // Filtered to one meeting, the series still shows.
        let filtered = layout(
            &p,
            &NetworkOptions {
                include_summary: false,
                visible_uids: Some(vec![11]),
            },
        );
        assert_eq!(
            filtered
                .nodes
                .iter()
                .map(|node| node.uid)
                .collect::<Vec<_>>(),
            vec![10]
        );
    }

    #[test]
    fn visible_uids_drop_hidden_tasks_and_their_edges() {
        let p = project(&[(1, false), (2, false), (3, false)], &[(1, 2), (2, 3)]);
        let options = NetworkOptions {
            include_summary: false,
            visible_uids: Some(vec![1, 3]),
        };
        let layout = layout(&p, &options);
        assert_eq!(layout.nodes.len(), 2);
        assert!(layout.edges.is_empty());
        assert_eq!(node(&layout, 3).column, 0);
    }

    #[test]
    fn empty_project_has_empty_layout() {
        let layout = layout(&project(&[], &[]), &NetworkOptions::default());
        assert!(layout.nodes.is_empty());
        assert_eq!(layout.columns, 0);
        assert_eq!(layout.width, 0.0);
        assert_eq!(layout.height, 0.0);
    }

    #[test]
    fn random_dags_have_valid_non_overlapping_layout() {
        let mut seed: u64 = 0x5eed;
        let mut next = move |bound: u64| {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) % bound
        };
        for _ in 0..20 {
            let count = 5 + next(60) as u32;
            let tasks: Vec<(u32, bool)> = (1..=count).map(|uid| (uid, false)).collect();
            let mut deps = Vec::new();
            for to in 2..=count {
                for _ in 0..next(3) {
                    // Edges always go from a lower to a higher UID, so the graph is acyclic.
                    let from = 1 + next((to - 1) as u64) as u32;
                    if !deps.contains(&(from, to)) {
                        deps.push((from, to));
                    }
                }
            }
            let p = project(&tasks, &deps);
            let layout = layout(&p, &NetworkOptions::default());
            assert_valid(&p, &layout, &deps);
        }
    }

    fn assert_valid(p: &Project, layout: &NetworkLayout, deps: &[(u32, u32)]) {
        assert_eq!(layout.nodes.len(), p.tasks.len());
        assert_eq!(layout.edges.len(), deps.len());
        let by_uid: HashMap<u32, &NetworkNode> = layout.nodes.iter().map(|n| (n.uid, n)).collect();
        assert_eq!(by_uid.len(), layout.nodes.len(), "each task appears once");
        for edge in &layout.edges {
            let from = by_uid.get(&edge.from_uid).expect("edge source is a node");
            let to = by_uid.get(&edge.to_uid).expect("edge target is a node");
            assert!(from.column < to.column, "edges run left to right");
            let first = edge.points.first().unwrap();
            let last = edge.points.last().unwrap();
            assert_eq!(first.x, from.x + from.width);
            assert_eq!(last.x, to.x);
            assert_eq!(edge.points.len(), 2 + 2 * (to.column - from.column - 1));
        }
        for (i, a) in layout.nodes.iter().enumerate() {
            assert!(a.x + a.width <= layout.width && a.y + a.height <= layout.height);
            for b in &layout.nodes[i + 1..] {
                let apart = a.x + a.width <= b.x
                    || b.x + b.width <= a.x
                    || a.y + a.height <= b.y
                    || b.y + b.height <= a.y;
                assert!(apart, "boxes {} and {} overlap", a.uid, b.uid);
            }
        }
        // Rows are the top-to-bottom rank inside a column.
        for node in &layout.nodes {
            for other in &layout.nodes {
                if node.column == other.column && node.row < other.row {
                    assert!(node.y < other.y);
                }
            }
        }
    }
}
