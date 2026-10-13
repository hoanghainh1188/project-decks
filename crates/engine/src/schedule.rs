use std::collections::{HashMap, HashSet};

use crate::error::EngineError;
use crate::graph::Graph;
use crate::level::publish_overallocations;
use crate::model::{ConstraintType, DepKind, Project};
use crate::split;
use crate::tracking;
use crate::work::publish_costs_and_usage;

/// Recompute WBS, early/late dates, slack, critical path, costs, usage, and the late flag.
pub fn schedule(project: &mut Project) -> Result<(), EngineError> {
    let graph = prepare(project)?;
    schedule_dates(project, &graph);
    publish_derived(project, &graph);
    Ok(())
}

/// Normalize the outline and splits, check the calendar, and build the link graph.
/// The graph stays valid while only dates and leveling delays change.
pub(crate) fn prepare(project: &mut Project) -> Result<Graph, EngineError> {
    normalize_outline(project);
    for task in &mut project.tasks {
        split::normalize_task(task);
    }
    crate::calendar_base::refresh(project);
    if !project.calendar().has_working_time() {
        return Err(EngineError::NoWorkingTime);
    }
    Graph::build(project)
}

/// Early and late dates, slack, the critical path, summary rollups, and split segments:
/// everything leveling needs to find the next overload.
pub(crate) fn schedule_dates(project: &mut Project, graph: &Graph) {
    let project_start = project.calendar().next_work_start(project.start);
    project.start = project_start;

    let mut shift: HashMap<u32, i64> = HashMap::new();
    // A handful of passes covers summary finish constraints that push a group later.
    let passes = project.tasks.len().max(1) + 2;
    let mut settled = false;
    for _ in 0..passes {
        forward(project, graph, &shift);
        if !push_summary_finish(project, graph, &mut shift) {
            settled = true;
            break;
        }
    }
    if !settled {
        forward(project, graph, &shift);
    }
    let background = background_tasks(project);
    backward(project, graph, &background);
    rollup(project, &background);
    split::publish_segments(project);
}

/// Free slack, costs and usage, overallocations, predecessor text, and the late flag.
pub(crate) fn publish_derived(project: &mut Project, graph: &Graph) {
    free_slack(project, graph);
    publish_costs_and_usage(project);
    publish_overallocations(project);
    publish_predecessor_text(project, graph);
    mark_late(project);
}

/// Recompute the derived late flag from the status date, baseline 0, and deadline.
pub fn mark_late(project: &mut Project) {
    let mut behind: Vec<bool> = match project.status_date {
        Some(status) => project
            .tasks
            .iter()
            .map(|task| tracking::behind_schedule(task, project.task_calendar(task), status))
            .collect(),
        None => vec![false; project.tasks.len()],
    };
    // A group is behind when any task in it is; children come after their summary.
    for index in (0..project.tasks.len()).rev() {
        if project.tasks[index].summary {
            let level = project.tasks[index].outline_level;
            behind[index] = (index + 1..project.tasks.len())
                .take_while(|&child| project.tasks[child].outline_level > level)
                .any(|child| behind[child]);
        }
    }
    for (task, behind) in project.tasks.iter_mut().zip(behind) {
        task.late = behind || task.misses_planned_dates();
    }
}

fn publish_predecessor_text(project: &mut Project, graph: &Graph) {
    let texts: Vec<String> = (0..project.tasks.len())
        .map(|position| {
            graph
                .preds(position)
                .iter()
                .filter_map(|&link| {
                    let dep = &project.dependencies[link];
                    let pred = graph.position(dep.predecessor_uid)?;
                    Some(format_predecessor(
                        project.tasks[pred].id,
                        dep.kind,
                        dep.lag,
                    ))
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect();
    for (task, text) in project.tasks.iter_mut().zip(texts) {
        task.predecessors = text;
    }
}

fn format_predecessor(id: u32, kind: DepKind, lag: i64) -> String {
    if kind == DepKind::Fs && lag == 0 {
        return id.to_string();
    }
    if lag == 0 {
        return format!("{id}{}", kind.as_str());
    }
    format!("{id}{}{}", kind.as_str(), format_lag(lag))
}

fn format_lag(minutes: i64) -> String {
    let sign = if minutes < 0 { "-" } else { "+" };
    let abs = minutes.abs();
    if abs % (8 * 60) == 0 {
        format!("{sign}{}d", abs / (8 * 60))
    } else if abs % 60 == 0 {
        format!("{sign}{}h", abs / 60)
    } else {
        format!("{sign}{abs}m")
    }
}

pub fn normalize_outline(project: &mut Project) {
    if project.tasks.is_empty() {
        return;
    }
    project.tasks[0].outline_level = 1;
    for i in 1..project.tasks.len() {
        let prev = project.tasks[i - 1].outline_level;
        if project.tasks[i].outline_level < 1 {
            project.tasks[i].outline_level = 1;
        }
        if project.tasks[i].outline_level > prev + 1 {
            project.tasks[i].outline_level = prev + 1;
        }
    }

    let mut counters: Vec<u32> = Vec::new();
    let mut stack: Vec<u32> = Vec::new();
    for i in 0..project.tasks.len() {
        let level = project.tasks[i].outline_level.max(1) as usize;
        counters.resize(level, 0);
        counters.truncate(level);
        counters[level - 1] += 1;
        project.tasks[i].wbs = counters
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(".");
        project.tasks[i].id = (i as u32) + 1;
        while stack.len() >= level {
            stack.pop();
        }
        project.tasks[i].parent_uid = stack.last().copied();
        stack.push(project.tasks[i].uid);
        project.tasks[i].summary = false;
    }
    for i in 0..project.tasks.len() {
        let level = project.tasks[i].outline_level;
        let is_summary = project
            .tasks
            .get(i + 1)
            .is_some_and(|next| next.outline_level > level);
        project.tasks[i].summary = is_summary;
        if is_summary {
            project.tasks[i].milestone = false;
        }
    }
}

/// Leaf dates by task position, filled as a pass reaches each leaf.
pub(crate) type Dates = Vec<Option<(i64, i64)>>;

fn forward(project: &mut Project, graph: &Graph, shift: &HashMap<u32, i64>) {
    let mut early: Dates = vec![None; project.tasks.len()];
    for &index in graph.order() {
        let (es, ef, conflict) = schedule_leaf(project, graph, index, &early, shift);
        early[index] = Some((es, ef));
        write_early(&mut project.tasks[index], es, ef, conflict);
    }
    rollup_partial(project);
}

/// What the forward pass writes on a leaf it has scheduled.
pub(crate) fn write_early(task: &mut crate::model::Task, es: i64, ef: i64, conflict: bool) {
    task.early_start = es;
    task.early_finish = ef;
    task.start = es;
    task.finish = ef;
    task.conflict = conflict;
    if task.milestone {
        task.duration = 0;
    }
}

fn rollup_partial(project: &mut Project) {
    for index in (0..project.tasks.len()).rev() {
        if !project.tasks[index].summary {
            continue;
        }
        let level = project.tasks[index].outline_level;
        let mut children = Vec::new();
        for j in index + 1..project.tasks.len() {
            if project.tasks[j].outline_level <= level {
                break;
            }
            if project.tasks[j].outline_level == level + 1 {
                children.push(j);
            }
        }
        if children.is_empty() {
            continue;
        }
        let start = children
            .iter()
            .map(|j| project.tasks[*j].start)
            .min()
            .unwrap();
        let finish = children
            .iter()
            .map(|j| project.tasks[*j].finish)
            .max()
            .unwrap();
        let task = &mut project.tasks[index];
        task.start = start;
        task.finish = finish;
        task.early_start = start;
        task.early_finish = finish;
    }
}

pub(crate) fn schedule_leaf(
    project: &Project,
    graph: &Graph,
    index: usize,
    early: &Dates,
    shift: &HashMap<u32, i64>,
) -> (i64, i64, bool) {
    let task = &project.tasks[index];
    let cal = project.task_calendar(task);
    // Actual dates beat links, constraints, leveling, and manual dates.
    if let Some((start, finish)) = tracking::pinned_dates(task, cal) {
        return (start, finish, false);
    }
    if task.manual {
        let start = task.start;
        let finish = task.finish.max(start);
        return (start, finish, false);
    }

    let (bound_start, bound_finish) = collect_bounds(project, graph, index, early);
    let extra = task.leveling_delay.max(0) + shift.get(&task.uid).copied().unwrap_or(0).max(0);
    let mut es = if extra > 0 {
        cal.add_lag(bound_start, extra)
    } else {
        cal.next_work_start(bound_start)
    };
    let mut conflict = false;

    if task.constraint_type == ConstraintType::Mso {
        if let Some(date) = task.constraint_date {
            let pinned = cal.next_work_start(date);
            if cal.working_between(pinned, bound_start) > 0 && pinned < bound_start {
                conflict = true;
            }
            es = pinned;
        }
    }

    // A split task occupies its work plus its gaps.
    let duration = split::span(task);
    let (mut es, mut ef) = span_from_start(cal, es, duration);

    if task.constraint_type != ConstraintType::Mso {
        if let Some(min_finish) = bound_finish {
            if ef < min_finish {
                let (start, finish) = cal.place_by_finish(min_finish, duration);
                if start >= es {
                    es = start;
                    ef = finish;
                } else if finish > ef {
                    ef = min_finish;
                }
            }
        }
        match task.constraint_type {
            ConstraintType::Fnet => {
                if let Some(date) = task.constraint_date {
                    if ef < date {
                        let (start, finish) = cal.place_by_finish(date, duration);
                        if start >= es {
                            es = start;
                            ef = finish;
                        }
                    }
                }
            }
            ConstraintType::Mfo => {
                if let Some(date) = task.constraint_date {
                    let (start, finish) = cal.place_by_finish(date, duration);
                    if bound_start > start && cal.working_between(start, bound_start) > 0 {
                        conflict = true;
                    }
                    es = start;
                    ef = finish;
                }
            }
            ConstraintType::Snlt => {
                if let Some(date) = task.constraint_date {
                    if es > date {
                        conflict = true;
                    }
                }
            }
            ConstraintType::Fnlt => {
                if let Some(date) = task.constraint_date {
                    if ef > date {
                        conflict = true;
                    }
                }
            }
            _ => {}
        }
    }

    (es, ef, conflict)
}

fn span_from_start(cal: &crate::calendar::Calendar, start: i64, duration: i64) -> (i64, i64) {
    let start = cal.next_work_start(start);
    if duration <= 0 {
        (start, start)
    } else {
        (start, cal.add_working(start, duration))
    }
}

fn collect_bounds(
    project: &Project,
    graph: &Graph,
    index: usize,
    early: &Dates,
) -> (i64, Option<i64>) {
    let task = &project.tasks[index];
    let cal = project.task_calendar(task);
    let mut min_start = cal.next_work_start(project.start);
    let mut min_finish: Option<i64> = None;

    let mut chain = vec![index];
    let mut parent = task.parent_uid;
    while let Some(position) = parent.and_then(|uid| graph.position(uid)) {
        chain.push(position);
        parent = project.tasks[position].parent_uid;
    }

    for node_index in chain {
        for &link in graph.preds(node_index) {
            let dep = &project.dependencies[link];
            let Some((pred_es, pred_ef)) = graph
                .position(dep.predecessor_uid)
                .and_then(|pred| predecessor_dates(project, graph, pred, early))
            else {
                continue;
            };
            match dep.kind {
                DepKind::Fs => min_start = min_start.max(cal.add_lag(pred_ef, dep.lag)),
                DepKind::Ss => min_start = min_start.max(cal.add_lag(pred_es, dep.lag)),
                DepKind::Ff => {
                    let finish = cal.finish_not_before(pred_ef, dep.lag);
                    min_finish = Some(min_finish.map_or(finish, |cur| cur.max(finish)));
                }
                DepKind::Sf => {
                    let finish = cal.finish_not_before(pred_es, dep.lag);
                    min_finish = Some(min_finish.map_or(finish, |cur| cur.max(finish)));
                }
            }
        }
        let node = &project.tasks[node_index];
        if matches!(
            node.constraint_type,
            ConstraintType::Snet | ConstraintType::Mso
        ) {
            if let Some(date) = node.constraint_date {
                // MSO on an ancestor pins the group's start; on the leaf it is applied later.
                if node.constraint_type == ConstraintType::Snet || node_index != index {
                    min_start = min_start.max(cal.next_work_start(date));
                }
            }
        }
    }
    (min_start, min_finish)
}

fn predecessor_dates(
    project: &Project,
    graph: &Graph,
    index: usize,
    early: &Dates,
) -> Option<(i64, i64)> {
    if project.tasks[index].summary {
        span_of(
            graph
                .leaves_under(project, index)
                .filter_map(|leaf| early[leaf]),
        )
    } else {
        early[index]
    }
}

/// Earliest start and latest finish of a set of dates.
fn span_of(dates: impl Iterator<Item = (i64, i64)>) -> Option<(i64, i64)> {
    dates.fold(None, |span, (start, finish)| match span {
        None => Some((start, finish)),
        Some((low, high)) => Some((low.min(start), high.max(finish))),
    })
}

/// Shift descendant leaves when a summary must finish later (FF/SF/FNET/MFO).
fn push_summary_finish(project: &Project, graph: &Graph, shift: &mut HashMap<u32, i64>) -> bool {
    let mut changed = false;
    for index in (0..project.tasks.len()).rev() {
        if !project.tasks[index].summary {
            continue;
        }
        let Some(required) = summary_required_finish(project, graph, index) else {
            continue;
        };
        let current = project.tasks[index].finish;
        if current >= required {
            continue;
        }
        let cal = project.calendar();
        let duration = cal
            .working_between(project.tasks[index].start, current)
            .max(0);
        let (target_start, _) = if duration > 0 {
            cal.place_by_finish(required, duration)
        } else {
            let snapped = cal.next_work_start(required);
            (snapped, snapped)
        };
        let bump = cal
            .working_between(project.tasks[index].start, target_start)
            .max(0);
        if bump <= 0 {
            continue;
        }
        for leaf in graph.leaves_under(project, index) {
            let uid = project.tasks[leaf].uid;
            let entry = shift.entry(uid).or_insert(0);
            *entry += bump;
            changed = true;
        }
        // One summary per pass so nested groups do not double-count.
        break;
    }
    changed
}

fn summary_required_finish(project: &Project, graph: &Graph, index: usize) -> Option<i64> {
    let task = &project.tasks[index];
    let cal = project.calendar();
    let mut required: Option<i64> = None;
    for &link in graph.preds(index) {
        let dep = &project.dependencies[link];
        let pred = &project.tasks[graph.position(dep.predecessor_uid)?];
        let (pes, pef) = (pred.early_start, pred.early_finish);
        let finish = match dep.kind {
            DepKind::Ff => Some(cal.finish_not_before(pef, dep.lag)),
            DepKind::Sf => Some(cal.finish_not_before(pes, dep.lag)),
            _ => None,
        };
        if let Some(finish) = finish {
            required = Some(required.map_or(finish, |cur| cur.max(finish)));
        }
    }
    match task.constraint_type {
        ConstraintType::Fnet | ConstraintType::Mfo => {
            if let Some(date) = task.constraint_date {
                required = Some(required.map_or(date, |cur| cur.max(date)));
            }
        }
        _ => {}
    }
    required
}

/// The summary and occurrences of every background recurring series.
pub(crate) fn background_tasks(project: &Project) -> Vec<bool> {
    let series: HashSet<u32> = project
        .tasks
        .iter()
        .filter(|task| task.recurrence.as_ref().is_some_and(|series| series.spec.background))
        .map(|task| task.uid)
        .collect();
    project
        .tasks
        .iter()
        .map(|task| {
            series.contains(&task.uid) || task.series_uid.is_some_and(|uid| series.contains(&uid))
        })
        .collect()
}

/// Latest early finish of the work that sets the project finish. Leaves are enough: a
/// summary finishes with its last leaf. Background series are left out unless nothing
/// else is left.
fn horizon(project: &Project, background: &[bool]) -> i64 {
    let finish_of = |skip_background: bool| {
        project
            .tasks
            .iter()
            .zip(background)
            .filter(|(task, background)| !task.summary && !(skip_background && **background))
            .map(|(task, _)| task.early_finish)
            .max()
    };
    finish_of(true).or_else(|| finish_of(false)).unwrap_or(project.start)
}

fn backward(project: &mut Project, graph: &Graph, background: &[bool]) {
    let horizon = horizon(project, background);
    let mut late: Dates = vec![None; project.tasks.len()];

    for &index in graph.order().iter().rev() {
        // A background occurrence sits where it is: no slack against a horizon it is
        // outside of, and never critical.
        let (ls, lf) = if background[index] {
            (project.tasks[index].early_start, project.tasks[index].early_finish)
        } else {
            late_leaf(project, graph, index, horizon, &late)
        };
        late[index] = Some((ls, lf));
        let task = &project.tasks[index];
        let cal = project.task_calendar(task);
        // A started task cannot move its start; only its finish has room to slip.
        let slack = if task.actual_start.is_some() {
            cal.working_between(task.early_finish, lf)
        } else {
            cal.working_between(task.early_start, ls)
        };
        let complete = tracking::is_complete(task);
        let task = &mut project.tasks[index];
        task.late_start = ls;
        task.late_finish = lf;
        task.total_slack = slack;
        task.critical = slack <= 0 && !complete && !background[index];
    }
    // ALAP consumes only the slack that does not push a successor which is
    // neither ALAP nor held by its own date constraint.
    schedule_alap(project, graph, background);
}

fn late_leaf(
    project: &Project,
    graph: &Graph,
    index: usize,
    horizon: i64,
    late: &Dates,
) -> (i64, i64) {
    let task = &project.tasks[index];
    let cal = project.task_calendar(task);
    let duration = split::span(task);
    if task.manual || tracking::is_complete(task) {
        return (task.early_start, task.early_finish);
    }

    let mut lf_cap = horizon;
    let mut ls_cap: Option<i64> = None;
    for &link in graph.succs(index) {
        let dep = &project.dependencies[link];
        let Some((succ_ls, succ_lf)) = graph
            .position(dep.successor_uid)
            .and_then(|succ| successor_late(project, graph, succ, late))
        else {
            continue;
        };
        match dep.kind {
            DepKind::Fs => {
                lf_cap = lf_cap.min(cal.latest_ref_for_start(succ_ls, dep.lag));
            }
            DepKind::Ff => {
                lf_cap = lf_cap.min(cal.latest_ref_for_finish(succ_lf, dep.lag));
            }
            DepKind::Ss => {
                let cap = cal.latest_ref_for_start(succ_ls, dep.lag);
                ls_cap = Some(ls_cap.map_or(cap, |cur| cur.min(cap)));
            }
            DepKind::Sf => {
                let cap = cal.latest_ref_for_finish(succ_lf, dep.lag);
                ls_cap = Some(ls_cap.map_or(cap, |cur| cur.min(cap)));
            }
        }
    }

    match task.constraint_type {
        ConstraintType::Fnlt | ConstraintType::Mfo => {
            if let Some(date) = task.constraint_date {
                lf_cap = lf_cap.min(date);
            }
        }
        ConstraintType::Snlt | ConstraintType::Mso => {
            if let Some(date) = task.constraint_date {
                let cap = cal.next_work_start(date);
                ls_cap = Some(ls_cap.map_or(cap, |cur| cur.min(cap)));
            }
        }
        _ => {}
    }

    let mut ls = if duration <= 0 {
        lf_cap
    } else {
        cal.subtract_working(lf_cap, duration)
    };
    if let Some(cap) = ls_cap {
        if cap < ls {
            ls = cap;
        }
    }
    ls = cal.next_work_start(ls);
    let mut lf = if duration <= 0 {
        ls
    } else {
        cal.add_working(ls, duration)
    };
    if lf > lf_cap && cal.working_between(lf_cap, lf) == 0 {
        lf = lf_cap;
    }
    (ls, lf)
}

fn successor_late(
    project: &Project,
    graph: &Graph,
    index: usize,
    late: &Dates,
) -> Option<(i64, i64)> {
    if project.tasks[index].summary {
        span_of(
            graph
                .leaves_under(project, index)
                .filter_map(|leaf| late[leaf]),
        )
    } else {
        late[index]
    }
}

/// Place ALAP tasks on the late dates they can actually use.
///
/// A flexible successor (not ALAP, and not pinned by SNET/MSO) stays on its
/// early dates, so the predecessor cannot slide into slack that would move it.
/// An ALAP successor is placed first and the predecessor follows it. A successor
/// SNET or MSO date is a ceiling: the predecessor may wait until that pin, and
/// no later. Tasks with no successor use the project early finish.
fn schedule_alap(project: &mut Project, graph: &Graph, background: &[bool]) {
    let horizon = horizon(project, background);
    let mut placed: Dates = vec![None; project.tasks.len()];
    for &index in graph.order().iter().rev() {
        let task = &project.tasks[index];
        if task.summary
            || task.manual
            || task.actual_start.is_some()
            || task.constraint_type != ConstraintType::Alap
        {
            continue;
        }
        let (ls, lf) = alap_dates(project, graph, index, horizon, &placed);
        placed[index] = Some((ls, lf));
        let task = &project.tasks[index];
        let slack = project
            .task_calendar(task)
            .working_between(task.early_start, ls)
            .max(0);
        let task = &mut project.tasks[index];
        task.start = ls;
        task.finish = lf;
        task.late_start = ls;
        task.late_finish = lf;
        task.total_slack = slack;
        task.critical = slack <= 0;
    }
}

fn alap_dates(
    project: &Project,
    graph: &Graph,
    index: usize,
    horizon: i64,
    placed: &Dates,
) -> (i64, i64) {
    let task = &project.tasks[index];
    let cal = project.task_calendar(task);
    let duration = split::span(task);
    let (early_start, early_finish) = (task.early_start, task.early_finish);
    let mut lf_cap = horizon;
    let mut ls_cap: Option<i64> = None;
    for &link in graph.succs(index) {
        let dep = &project.dependencies[link];
        let Some((succ_es, succ_ef)) = graph
            .position(dep.successor_uid)
            .and_then(|succ| successor_anchor(project, graph, succ, placed))
        else {
            continue;
        };
        match dep.kind {
            DepKind::Fs => lf_cap = lf_cap.min(cal.latest_ref_for_start(succ_es, dep.lag)),
            DepKind::Ff => lf_cap = lf_cap.min(cal.latest_ref_for_finish(succ_ef, dep.lag)),
            DepKind::Ss => {
                let cap = cal.latest_ref_for_start(succ_es, dep.lag);
                ls_cap = Some(ls_cap.map_or(cap, |cur| cur.min(cap)));
            }
            DepKind::Sf => {
                let cap = cal.latest_ref_for_finish(succ_ef, dep.lag);
                ls_cap = Some(ls_cap.map_or(cap, |cur| cur.min(cap)));
            }
        }
    }
    let mut ls = if duration <= 0 {
        lf_cap
    } else {
        cal.subtract_working(lf_cap, duration)
    };
    if let Some(cap) = ls_cap {
        ls = ls.min(cap);
    }
    // Never schedule ALAP earlier than the forward pass.
    if ls < early_start {
        ls = early_start;
    }
    ls = cal.next_work_start(ls);
    let lf = if duration <= 0 {
        ls
    } else {
        cal.add_working(ls, duration)
    };
    if lf < early_finish && cal.working_between(lf, early_finish) > 0 {
        return (early_start, early_finish);
    }
    (ls, lf)
}

/// Dates an ALAP predecessor must respect for this successor.
///
/// ALAP successors contribute the late position already chosen (successors are
/// placed first). Every other successor, including SNET and MSO, stays on the
/// forward-pass dates so this task cannot push it. SNET/MSO early dates already
/// include the constraint, and MSO is also clamped to the pinned instant.
fn successor_anchor(
    project: &Project,
    graph: &Graph,
    index: usize,
    placed: &Dates,
) -> Option<(i64, i64)> {
    if project.tasks[index].summary {
        return span_of(
            graph
                .leaves_under(project, index)
                .map(|leaf| leaf_anchor(project, leaf, placed)),
        );
    }
    Some(leaf_anchor(project, index, placed))
}

fn leaf_anchor(project: &Project, index: usize, placed: &Dates) -> (i64, i64) {
    let task = &project.tasks[index];
    if !task.manual && task.constraint_type == ConstraintType::Alap {
        if let Some(dates) = placed[index] {
            return dates;
        }
    }
    let mut start = task.early_start;
    if matches!(
        task.constraint_type,
        ConstraintType::Snet | ConstraintType::Mso
    ) {
        if let Some(date) = task.constraint_date {
            let pinned = calendar_for(project, task.calendar_id).next_work_start(date);
            if task.constraint_type == ConstraintType::Mso && start > pinned {
                // Must-start-on is a hard ceiling even if the forward pass drifted later.
                start = pinned;
            } else if task.constraint_type == ConstraintType::Snet && start < pinned {
                // Start-no-earlier-than holds the successor, which caps this predecessor.
                start = pinned;
            }
        }
    }
    (start, task.early_finish)
}

fn calendar_for(project: &Project, calendar_id: Option<u32>) -> &crate::calendar::Calendar {
    calendar_id
        .and_then(|id| project.calendar_by_id(id))
        .unwrap_or_else(|| project.calendar())
}

/// Summary dates, slack, and the critical flag from the children. Late dates and the
/// critical flag skip background series, so a meeting series inside a phase does not
/// make the phase critical; a group of only background work is never critical.
fn rollup(project: &mut Project, background: &[bool]) {
    let cal = project.calendar().clone();
    // A summary is done when every child is; a done group is never critical.
    let mut done: Vec<bool> = project.tasks.iter().map(tracking::is_complete).collect();
    for index in (0..project.tasks.len()).rev() {
        if !project.tasks[index].summary {
            continue;
        }
        let level = project.tasks[index].outline_level;
        let mut child_indexes = Vec::new();
        for j in index + 1..project.tasks.len() {
            if project.tasks[j].outline_level <= level {
                break;
            }
            if project.tasks[j].outline_level == level + 1 {
                child_indexes.push(j);
            }
        }
        if child_indexes.is_empty() {
            continue;
        }
        let driving: Vec<usize> = child_indexes.iter().copied().filter(|j| !background[*j]).collect();
        let driving = if driving.is_empty() { child_indexes.clone() } else { driving };
        let start = child_indexes
            .iter()
            .map(|j| project.tasks[*j].start)
            .min()
            .unwrap();
        let finish = child_indexes
            .iter()
            .map(|j| project.tasks[*j].finish)
            .max()
            .unwrap();
        let early_start = child_indexes
            .iter()
            .map(|j| project.tasks[*j].early_start)
            .min()
            .unwrap();
        let early_finish = child_indexes
            .iter()
            .map(|j| project.tasks[*j].early_finish)
            .max()
            .unwrap();
        let late_start = driving
            .iter()
            .map(|j| project.tasks[*j].late_start)
            .min()
            .unwrap();
        let late_finish = driving
            .iter()
            .map(|j| project.tasks[*j].late_finish)
            .max()
            .unwrap();
        let critical = driving.iter().any(|j| project.tasks[*j].critical);
        done[index] = child_indexes.iter().all(|j| done[*j]);
        let task = &mut project.tasks[index];
        task.start = start;
        task.finish = finish;
        task.early_start = early_start;
        task.early_finish = early_finish;
        task.late_start = late_start;
        task.late_finish = late_finish;
        task.duration = cal.working_between(start, finish).max(0);
        task.total_slack = cal.working_between(early_start, late_start);
        task.critical = (critical || task.total_slack <= 0) && !done[index] && !background[index];
        task.milestone = false;
    }
}

fn free_slack(project: &mut Project, graph: &Graph) {
    let mut leaf_slack = Vec::new();
    for (index, task) in project
        .tasks
        .iter()
        .enumerate()
        .filter(|(_, task)| !task.summary)
    {
        let cal = project.task_calendar(task);
        let mut gap = task.total_slack;
        for &link in graph.succs(index) {
            let dep = &project.dependencies[link];
            let Some(succ) = graph
                .position(dep.successor_uid)
                .map(|succ| &project.tasks[succ])
            else {
                continue;
            };
            let piece = match dep.kind {
                DepKind::Fs => {
                    let imposed = cal.add_lag(task.early_finish, dep.lag);
                    cal.working_between(imposed, succ.early_start).max(0)
                }
                DepKind::Ss => {
                    let imposed = cal.add_lag(task.early_start, dep.lag);
                    cal.working_between(imposed, succ.early_start).max(0)
                }
                DepKind::Ff => {
                    let imposed = cal.finish_not_before(task.early_finish, dep.lag);
                    cal.working_between(imposed, succ.early_finish).max(0)
                }
                DepKind::Sf => {
                    let imposed = cal.finish_not_before(task.early_start, dep.lag);
                    cal.working_between(imposed, succ.early_finish).max(0)
                }
            };
            gap = gap.min(piece);
        }
        leaf_slack.push((index, gap.max(0)));
    }
    for (index, gap) in leaf_slack {
        project.tasks[index].free_slack = gap;
    }
    for index in (0..project.tasks.len()).rev() {
        if !project.tasks[index].summary {
            continue;
        }
        let level = project.tasks[index].outline_level;
        let own = project.tasks[index].total_slack;
        let min_child = project
            .tasks
            .iter()
            .skip(index + 1)
            .take_while(|task| task.outline_level > level)
            .filter(|task| task.outline_level == level + 1)
            .map(|task| task.free_slack)
            .min();
        project.tasks[index].free_slack = min_child.unwrap_or(own).min(own);
    }
}
