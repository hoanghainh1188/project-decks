use std::collections::{HashMap, VecDeque};

use crate::error::EngineError;
use crate::level::publish_overallocations;
use crate::model::{ConstraintType, DepKind, Project, Task};
use crate::work::publish_costs_and_usage;

/// Recompute WBS, early/late dates, slack, critical path, costs, and usage.
pub fn schedule(project: &mut Project) -> Result<(), EngineError> {
    normalize_outline(project);
    if project.calendar().week.iter().all(|d| d.is_empty())
        && project
            .calendar()
            .exceptions
            .values()
            .all(|ranges| ranges.is_empty())
    {
        return Err(EngineError::NoWorkingTime);
    }
    detect_cycles(project)?;

    let project_start = project.calendar().next_work_start(project.start);
    project.start = project_start;

    let mut shift: HashMap<u32, i64> = HashMap::new();
    // A handful of passes covers summary finish constraints that push a group later.
    let passes = project.tasks.len().max(1) + 2;
    for _ in 0..passes {
        forward(project, &shift)?;
        if !push_summary_finish(project, &mut shift) {
            break;
        }
    }
    forward(project, &shift)?;
    backward(project);
    rollup(project);
    free_slack(project);
    publish_costs_and_usage(project);
    publish_overallocations(project);
    publish_predecessor_text(project);
    Ok(())
}

fn publish_predecessor_text(project: &mut Project) {
    let deps = project.dependencies.clone();
    let ids: HashMap<u32, u32> = project.tasks.iter().map(|task| (task.uid, task.id)).collect();
    for task in &mut project.tasks {
        let mut parts = Vec::new();
        for dep in deps.iter().filter(|dep| dep.successor_uid == task.uid) {
            let Some(id) = ids.get(&dep.predecessor_uid) else {
                continue;
            };
            parts.push(format_predecessor(*id, dep.kind, dep.lag));
        }
        task.predecessors = parts.join(",");
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

fn detect_cycles(project: &Project) -> Result<(), EngineError> {
    let nodes = leaf_uids(project);
    let edges = expanded_edges(project)?;
    if topo(&nodes, &edges).is_none() {
        return Err(EngineError::DependencyCycle);
    }
    Ok(())
}

fn leaf_uids(project: &Project) -> Vec<u32> {
    project
        .tasks
        .iter()
        .filter(|t| !t.summary)
        .map(|t| t.uid)
        .collect()
}

fn expanded_edges(project: &Project) -> Result<Vec<(u32, u32)>, EngineError> {
    let mut edges = Vec::new();
    for dep in &project.dependencies {
        let pred_nodes = endpoint_leaves(project, dep.predecessor_uid);
        let succ_nodes = endpoint_leaves(project, dep.successor_uid);
        for pred in &pred_nodes {
            for succ in &succ_nodes {
                if pred == succ {
                    return Err(EngineError::DependencyCycle);
                }
                edges.push((*pred, *succ));
            }
        }
    }
    Ok(edges)
}

fn endpoint_leaves(project: &Project, uid: u32) -> Vec<u32> {
    let Some(index) = project.index_of(uid) else {
        return Vec::new();
    };
    if !project.tasks[index].summary {
        return vec![uid];
    }
    descendant_leaves(project, index)
        .into_iter()
        .map(|i| project.tasks[i].uid)
        .collect()
}

fn descendant_leaves(project: &Project, summary: usize) -> Vec<usize> {
    let level = project.tasks[summary].outline_level;
    let mut out = Vec::new();
    for j in summary + 1..project.tasks.len() {
        if project.tasks[j].outline_level <= level {
            break;
        }
        if !project.tasks[j].summary {
            out.push(j);
        }
    }
    out
}

fn topo(nodes: &[u32], edges: &[(u32, u32)]) -> Option<Vec<u32>> {
    let mut indeg: HashMap<u32, usize> = nodes.iter().map(|n| (*n, 0)).collect();
    let mut adj: HashMap<u32, Vec<u32>> = HashMap::new();
    for &(pred, succ) in edges {
        if !indeg.contains_key(&pred) || !indeg.contains_key(&succ) {
            continue;
        }
        adj.entry(pred).or_default().push(succ);
        *indeg.entry(succ).or_insert(0) += 1;
    }
    let mut queue: VecDeque<u32> = nodes
        .iter()
        .copied()
        .filter(|n| indeg.get(n).copied().unwrap_or(0) == 0)
        .collect();
    let mut order = Vec::with_capacity(nodes.len());
    while let Some(node) = queue.pop_front() {
        order.push(node);
        if let Some(nexts) = adj.get(&node) {
            for next in nexts {
                if let Some(degree) = indeg.get_mut(next) {
                    *degree = degree.saturating_sub(1);
                    if *degree == 0 {
                        queue.push_back(*next);
                    }
                }
            }
        }
    }
    if order.len() == nodes.len() {
        Some(order)
    } else {
        None
    }
}

fn forward(project: &mut Project, shift: &HashMap<u32, i64>) -> Result<(), EngineError> {
    let nodes = leaf_uids(project);
    let edges = expanded_edges(project)?;
    let order = topo(&nodes, &edges).ok_or(EngineError::DependencyCycle)?;
    let mut early: HashMap<u32, (i64, i64)> = HashMap::new();

    for uid in order {
        let Some(index) = project.index_of(uid) else {
            continue;
        };
        let (es, ef, conflict) = schedule_leaf(project, index, &early, shift);
        early.insert(uid, (es, ef));
        let task = &mut project.tasks[index];
        task.early_start = es;
        task.early_finish = ef;
        task.start = es;
        task.finish = ef;
        task.conflict = conflict;
        if task.milestone {
            task.duration = 0;
        }
    }
    rollup_partial(project);
    Ok(())
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
        let start = children.iter().map(|j| project.tasks[*j].start).min().unwrap();
        let finish = children.iter().map(|j| project.tasks[*j].finish).max().unwrap();
        let task = &mut project.tasks[index];
        task.start = start;
        task.finish = finish;
        task.early_start = start;
        task.early_finish = finish;
    }
}

fn schedule_leaf(
    project: &Project,
    index: usize,
    early: &HashMap<u32, (i64, i64)>,
    shift: &HashMap<u32, i64>,
) -> (i64, i64, bool) {
    let task = &project.tasks[index];
    let cal = project.task_calendar(task);
    if task.manual {
        let start = task.start;
        let finish = task.finish.max(start);
        return (start, finish, false);
    }

    let (bound_start, bound_finish) = collect_bounds(project, index, early);
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

    let duration = if task.milestone {
        0
    } else {
        task.duration.max(0)
    };
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
    index: usize,
    early: &HashMap<u32, (i64, i64)>,
) -> (i64, Option<i64>) {
    let task = &project.tasks[index];
    let cal = project.task_calendar(task);
    let mut min_start = cal.next_work_start(project.start);
    let mut min_finish: Option<i64> = None;

    let mut chain = vec![task.uid];
    let mut parent = task.parent_uid;
    while let Some(uid) = parent {
        chain.push(uid);
        parent = project.task(uid).and_then(|t| t.parent_uid);
    }

    for uid in chain {
        for dep in project
            .dependencies
            .iter()
            .filter(|d| d.successor_uid == uid)
        {
            let Some((pred_es, pred_ef)) = predecessor_dates(project, dep.predecessor_uid, early)
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
        if let Some(node) = project.task(uid) {
            if matches!(node.constraint_type, ConstraintType::Snet | ConstraintType::Mso) {
                if let Some(date) = node.constraint_date {
                    // MSO on an ancestor pins the group's start; on the leaf it is applied later.
                    if node.constraint_type == ConstraintType::Snet || uid != task.uid {
                        min_start = min_start.max(cal.next_work_start(date));
                    }
                }
            }
        }
    }
    (min_start, min_finish)
}

fn predecessor_dates(
    project: &Project,
    uid: u32,
    early: &HashMap<u32, (i64, i64)>,
) -> Option<(i64, i64)> {
    let index = project.index_of(uid)?;
    if project.tasks[index].summary {
        summary_span(project, index, early)
    } else {
        early.get(&uid).copied()
    }
}

fn summary_span(
    project: &Project,
    summary: usize,
    early: &HashMap<u32, (i64, i64)>,
) -> Option<(i64, i64)> {
    let mut start = None;
    let mut finish = None;
    for index in descendant_leaves(project, summary) {
        let uid = project.tasks[index].uid;
        let Some((es, ef)) = early.get(&uid).copied() else {
            continue;
        };
        start = Some(start.map_or(es, |cur: i64| cur.min(es)));
        finish = Some(finish.map_or(ef, |cur: i64| cur.max(ef)));
    }
    Some((start?, finish?))
}

/// Shift descendant leaves when a summary must finish later (FF/SF/FNET/MFO).
fn push_summary_finish(project: &Project, shift: &mut HashMap<u32, i64>) -> bool {
    let mut changed = false;
    for index in (0..project.tasks.len()).rev() {
        if !project.tasks[index].summary {
            continue;
        }
        let Some(required) = summary_required_finish(project, index) else {
            continue;
        };
        let current = project.tasks[index].finish;
        if current >= required {
            continue;
        }
        let cal = project.calendar();
        let duration = cal.working_between(project.tasks[index].start, current).max(0);
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
        for leaf in descendant_leaves(project, index) {
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

fn summary_required_finish(project: &Project, index: usize) -> Option<i64> {
    let task = &project.tasks[index];
    let cal = project.calendar();
    let mut required: Option<i64> = None;
    for dep in project
        .dependencies
        .iter()
        .filter(|d| d.successor_uid == task.uid)
    {
        let pred = project.task(dep.predecessor_uid)?;
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

fn backward(project: &mut Project) {
    let horizon = project
        .tasks
        .iter()
        .map(|t| t.early_finish)
        .max()
        .unwrap_or(project.start);
    let nodes = leaf_uids(project);
    let edges = expanded_edges(project).unwrap_or_default();
    let Some(order) = topo(&nodes, &edges) else {
        return;
    };
    let mut late: HashMap<u32, (i64, i64)> = HashMap::new();

    for uid in order.into_iter().rev() {
        let Some(index) = project.index_of(uid) else {
            continue;
        };
        let (ls, lf) = late_leaf(project, index, horizon, &late);
        late.insert(uid, (ls, lf));
        let cal = project.task_calendar(&project.tasks[index]).clone();
        let early_start = project.tasks[index].early_start;
        let slack = cal.working_between(early_start, ls);
        let task = &mut project.tasks[index];
        task.late_start = ls;
        task.late_finish = lf;
        task.total_slack = slack;
        task.critical = slack <= 0;
    }
    // ALAP consumes only the slack that does not push a successor which is
    // neither ALAP nor held by its own date constraint.
    schedule_alap(project);
}

fn late_leaf(
    project: &Project,
    index: usize,
    horizon: i64,
    late: &HashMap<u32, (i64, i64)>,
) -> (i64, i64) {
    let task = &project.tasks[index];
    let cal = project.task_calendar(task);
    let duration = if task.milestone {
        0
    } else {
        task.duration.max(0)
    };
    if task.manual {
        return (task.early_start, task.early_finish);
    }

    let mut lf_cap = horizon;
    let mut ls_cap: Option<i64> = None;
    for dep in project
        .dependencies
        .iter()
        .filter(|d| d.predecessor_uid == task.uid)
    {
        let Some((succ_ls, succ_lf)) = successor_late(project, dep.successor_uid, late) else {
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
    uid: u32,
    late: &HashMap<u32, (i64, i64)>,
) -> Option<(i64, i64)> {
    let index = project.index_of(uid)?;
    if !project.tasks[index].summary {
        return late.get(&uid).copied();
    }
    let mut ls = None;
    let mut lf = None;
    for leaf in descendant_leaves(project, index) {
        let uid = project.tasks[leaf].uid;
        let Some((leaf_ls, leaf_lf)) = late.get(&uid).copied() else {
            continue;
        };
        ls = Some(ls.map_or(leaf_ls, |cur: i64| cur.min(leaf_ls)));
        lf = Some(lf.map_or(leaf_lf, |cur: i64| cur.max(leaf_lf)));
    }
    Some((ls?, lf?))
}

/// Place ALAP tasks on the late dates they can actually use.
///
/// A flexible successor (not ALAP, and not pinned by SNET/MSO) stays on its
/// early dates, so the predecessor cannot slide into slack that would move it.
/// An ALAP successor is placed first and the predecessor follows it. A successor
/// SNET or MSO date is a ceiling: the predecessor may wait until that pin, and
/// no later. Tasks with no successor use the project early finish.
fn schedule_alap(project: &mut Project) {
    let nodes = leaf_uids(project);
    let edges = expanded_edges(project).unwrap_or_default();
    let Some(order) = topo(&nodes, &edges) else {
        return;
    };
    let mut placed: HashMap<u32, (i64, i64)> = HashMap::new();
    for uid in order.into_iter().rev() {
        let Some(index) = project.index_of(uid) else {
            continue;
        };
        let (skip, duration, early_start, early_finish, calendar_id) = {
            let task = &project.tasks[index];
            (
                task.summary || task.manual || task.constraint_type != ConstraintType::Alap,
                if task.milestone { 0 } else { task.duration.max(0) },
                task.early_start,
                task.early_finish,
                task.calendar_id,
            )
        };
        if skip {
            continue;
        }
        let cal = calendar_for(project, calendar_id).clone();
        let (ls, lf) = alap_dates(project, uid, duration, early_start, early_finish, &placed);
        placed.insert(uid, (ls, lf));
        let slack = cal.working_between(early_start, ls).max(0);
        if let Some(task) = project.task_mut(uid) {
            task.start = ls;
            task.finish = lf;
            task.late_start = ls;
            task.late_finish = lf;
            task.total_slack = slack;
            task.critical = slack <= 0;
        }
    }
}

fn alap_dates(
    project: &Project,
    uid: u32,
    duration: i64,
    early_start: i64,
    early_finish: i64,
    placed: &HashMap<u32, (i64, i64)>,
) -> (i64, i64) {
    let task = project.task(uid).unwrap();
    let cal = project.task_calendar(task);
    let horizon = project
        .tasks
        .iter()
        .filter(|t| !t.summary)
        .map(|t| t.early_finish)
        .max()
        .unwrap_or(project.start);
    let mut lf_cap = horizon;
    let mut ls_cap: Option<i64> = None;
    for dep in project.dependencies.iter().filter(|d| d.predecessor_uid == uid) {
        let Some((succ_es, succ_ef)) = successor_anchor(project, dep.successor_uid, placed) else {
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
    uid: u32,
    placed: &HashMap<u32, (i64, i64)>,
) -> Option<(i64, i64)> {
    let index = project.index_of(uid)?;
    if project.tasks[index].summary {
        let mut start = None;
        let mut finish = None;
        for leaf in descendant_leaves(project, index) {
            let (es, ef) = leaf_anchor(project, leaf, placed);
            start = Some(start.map_or(es, |cur: i64| cur.min(es)));
            finish = Some(finish.map_or(ef, |cur: i64| cur.max(ef)));
        }
        return Some((start?, finish?));
    }
    Some(leaf_anchor(project, index, placed))
}

fn leaf_anchor(project: &Project, index: usize, placed: &HashMap<u32, (i64, i64)>) -> (i64, i64) {
    let (uid, manual, constraint, date, early_start, early_finish, calendar_id) = {
        let task = &project.tasks[index];
        (
            task.uid,
            task.manual,
            task.constraint_type,
            task.constraint_date,
            task.early_start,
            task.early_finish,
            task.calendar_id,
        )
    };
    if !manual && constraint == ConstraintType::Alap {
        if let Some(dates) = placed.get(&uid) {
            return *dates;
        }
    }
    let mut start = early_start;
    if matches!(constraint, ConstraintType::Snet | ConstraintType::Mso) {
        if let Some(date) = date {
            let pinned = calendar_for(project, calendar_id).next_work_start(date);
            if constraint == ConstraintType::Mso && start > pinned {
                // Must-start-on is a hard ceiling even if the forward pass drifted later.
                start = pinned;
            } else if constraint == ConstraintType::Snet && start < pinned {
                // Start-no-earlier-than holds the successor, which caps this predecessor.
                start = pinned;
            }
        }
    }
    (start, early_finish)
}

fn calendar_for(project: &Project, calendar_id: Option<u32>) -> &crate::calendar::Calendar {
    calendar_id
        .and_then(|id| project.calendar_by_id(id))
        .unwrap_or_else(|| project.calendar())
}

fn rollup(project: &mut Project) {
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
        let late_start = child_indexes
            .iter()
            .map(|j| project.tasks[*j].late_start)
            .min()
            .unwrap();
        let late_finish = child_indexes
            .iter()
            .map(|j| project.tasks[*j].late_finish)
            .max()
            .unwrap();
        let critical = child_indexes
            .iter()
            .any(|j| project.tasks[*j].critical);
        let cal = project.calendar().clone();
        let task = &mut project.tasks[index];
        task.start = start;
        task.finish = finish;
        task.early_start = early_start;
        task.early_finish = early_finish;
        task.late_start = late_start;
        task.late_finish = late_finish;
        task.duration = cal.working_between(start, finish).max(0);
        task.total_slack = cal.working_between(early_start, late_start);
        task.critical = critical || task.total_slack <= 0;
        task.milestone = false;
    }
}

fn free_slack(project: &mut Project) {
    let snapshot: Vec<Task> = project.tasks.clone();
    let deps = project.dependencies.clone();
    let mut leaf_slack = Vec::new();
    for task in snapshot.iter().filter(|task| !task.summary) {
        let cal = project.task_calendar(task).clone();
        let mut gap = task.total_slack;
        let mut saw = false;
        for dep in deps.iter().filter(|dep| dep.predecessor_uid == task.uid) {
            let Some(succ) = snapshot.iter().find(|item| item.uid == dep.successor_uid) else {
                continue;
            };
            saw = true;
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
        if !saw {
            gap = task.total_slack;
        }
        leaf_slack.push((task.uid, gap.max(0)));
    }
    for (uid, gap) in leaf_slack {
        if let Some(task) = project.task_mut(uid) {
            task.free_slack = gap;
        }
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
