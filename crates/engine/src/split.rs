//! Task splits, like Microsoft Project's "Split Task".
//!
//! A split leaf task keeps its working duration; its work is divided into segments
//! separated by gaps. Splits are stored as working-minute offsets into the work plus
//! gap lengths in working minutes on the task calendar, so the segments keep their
//! relative positions when the start moves. The scheduler treats the task as one span
//! of `duration + gaps` working minutes; successors key off the last segment finish.

use serde::{Deserialize, Serialize};

use crate::calendar::Calendar;
use crate::error::{EngineError, Invalid};
use crate::model::{Project, Task};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Split {
    /// Working minutes of task work done before the gap; strictly between 0 and the duration.
    pub offset: i64,
    /// Gap length in working minutes on the task calendar.
    pub gap: i64,
}

/// One scheduled piece of work, as instants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub start: i64,
    pub finish: i64,
}

pub fn total_gap(splits: &[Split]) -> i64 {
    splits.iter().map(|split| split.gap.max(0)).sum()
}

/// Working minutes from the first segment start to the last segment finish.
pub fn span(task: &Task) -> i64 {
    if task.milestone {
        return 0;
    }
    task.duration.max(0) + total_gap(&task.splits)
}

/// Sort, merge equal offsets, and drop splits outside the work or with no gap.
pub fn normalize(splits: &mut Vec<Split>, duration: i64) {
    splits.retain(|split| split.gap > 0 && split.offset > 0 && split.offset < duration);
    splits.sort_by_key(|split| split.offset);
    let mut merged: Vec<Split> = Vec::with_capacity(splits.len());
    for split in splits.drain(..) {
        match merged.last_mut() {
            Some(last) if last.offset == split.offset => last.gap += split.gap,
            _ => merged.push(split),
        }
    }
    *splits = merged;
}

/// Splits only exist on leaf tasks that have work to divide.
pub fn normalize_task(task: &mut Task) {
    if task.summary || task.milestone || task.duration <= 0 {
        task.splits.clear();
    } else {
        normalize(&mut task.splits, task.duration);
    }
}

/// Lay the segments out from `start`. Returns one segment per piece of work.
pub fn layout(cal: &Calendar, start: i64, duration: i64, splits: &[Split]) -> Vec<Segment> {
    let mut segments = Vec::with_capacity(splits.len() + 1);
    let mut cursor = cal.next_work_start(start);
    let mut done = 0i64;
    for split in splits {
        let finish = cal.add_working(cursor, split.offset - done);
        segments.push(Segment {
            start: cursor,
            finish,
        });
        cursor = cal.next_work_start(cal.add_working(finish, split.gap));
        done = split.offset;
    }
    let finish = cal.add_working(cursor, (duration - done).max(0));
    segments.push(Segment {
        start: cursor,
        finish,
    });
    segments
}

/// When the work after the first `offset` working minutes goes on: inside a segment,
/// the next working instant; at a split, the start of the next segment.
pub fn resume_at(cal: &Calendar, task: &Task, offset: i64) -> i64 {
    let segments = layout(cal, task.start, task.duration.max(0), &task.splits);
    let mut done = 0i64;
    for (index, segment) in segments.iter().enumerate() {
        let length = cal.working_between(segment.start, segment.finish).max(0);
        if offset < done + length {
            return cal.next_work_start(cal.add_working(segment.start, offset - done));
        }
        done += length;
        if offset == done {
            return segments.get(index + 1).map_or(segment.finish, |next| next.start);
        }
    }
    segments.last().map_or(task.start, |segment| segment.finish)
}

/// Time windows in which the task does work: its segments, or the whole bar when unsplit.
pub fn work_intervals(task: &Task) -> Vec<(i64, i64)> {
    if task.segments.is_empty() {
        vec![(task.start, task.finish)]
    } else {
        task.segments
            .iter()
            .map(|segment| (segment.start, segment.finish))
            .collect()
    }
}

/// Working minutes of `intervals` that fall inside `[from, to)`.
pub fn working_within(cal: &Calendar, intervals: &[(i64, i64)], from: i64, to: i64) -> i64 {
    intervals
        .iter()
        .map(|(start, finish)| {
            let low = (*start).max(from);
            let high = (*finish).min(to);
            if high > low {
                cal.working_between(low, high).max(0)
            } else {
                0
            }
        })
        .sum()
}

/// Recompute the derived segments of every leaf after its start and finish are final.
pub fn publish_segments(project: &mut Project) {
    let layouts: Vec<(usize, Vec<Segment>)> = project
        .tasks
        .iter()
        .enumerate()
        .map(|(index, task)| {
            if task.summary || task.splits.is_empty() {
                return (index, Vec::new());
            }
            let cal = project.task_calendar(task);
            let mut segments = layout(cal, task.start, task.duration.max(0), &task.splits);
            // A manual task keeps the finish the user pinned; it bounds the last segment.
            if task.manual {
                if let Some(last) = segments.last_mut() {
                    if task.finish > last.start {
                        last.finish = task.finish;
                    }
                }
            }
            (index, segments)
        })
        .collect();
    for (index, segments) in layouts {
        project.tasks[index].segments = segments;
    }
}

fn outside_work() -> EngineError {
    Invalid::SplitOutsideWork.into()
}

/// Work offset (working minutes) at the instant `at`, reading the current segments.
/// An instant inside an existing gap maps to that gap's offset.
fn offset_at(cal: &Calendar, task: &Task, at: i64) -> Option<i64> {
    let intervals = work_intervals(task);
    let mut done = 0i64;
    for (index, (start, finish)) in intervals.iter().enumerate() {
        if at < *start {
            return (index > 0).then_some(done);
        }
        if at < *finish {
            return Some(done + cal.working_between(*start, at).max(0));
        }
        done += cal.working_between(*start, *finish).max(0);
    }
    None
}

fn refresh_manual_finish(project: &mut Project, uid: u32) {
    let Some(task) = project.task(uid) else {
        return;
    };
    if !task.manual {
        return;
    }
    let cal = project.task_calendar(task).clone();
    let finish = cal.add_working(task.start, span(task));
    if let Some(task) = project.task_mut(uid) {
        task.finish = finish.max(task.start);
    }
}

/// Interrupt the work at `at` for `gap` working minutes. The duration is unchanged.
pub fn split_task(project: &mut Project, uid: u32, at: i64, gap: i64) -> Result<(), EngineError> {
    let task = project.task(uid).ok_or(EngineError::TaskNotFound)?;
    if task.summary {
        return Err(Invalid::SplitSummary.into());
    }
    if task.milestone || task.duration <= 0 {
        return Err(Invalid::SplitNoDuration.into());
    }
    if gap <= 0 {
        return Err(Invalid::SplitGapNotPositive.into());
    }
    let cal = project.task_calendar(task);
    let offset = offset_at(cal, task, at).ok_or_else(outside_work)?;
    if offset <= 0 || offset >= task.duration {
        return Err(outside_work());
    }
    let task = project.task_mut(uid).ok_or(EngineError::TaskNotFound)?;
    task.splits.push(Split { offset, gap });
    normalize(&mut task.splits, task.duration);
    refresh_manual_finish(project, uid);
    Ok(())
}

/// Join the segments on both sides of split `index`, or every segment when `index` is `None`.
pub fn remove_split(
    project: &mut Project,
    uid: u32,
    index: Option<u32>,
) -> Result<(), EngineError> {
    let task = project.task_mut(uid).ok_or(EngineError::TaskNotFound)?;
    if task.splits.is_empty() {
        return Err(Invalid::NotSplit.into());
    }
    match index {
        None => task.splits.clear(),
        Some(index) => {
            let index = index as usize;
            if index >= task.splits.len() {
                return Err(Invalid::NoSuchSplit.into());
            }
            task.splits.remove(index);
        }
    }
    refresh_manual_finish(project, uid);
    Ok(())
}

/// Duration for a new finish, keeping the gaps that still fit before it.
/// Resizing changes the last segment; splits the new duration no longer reaches are dropped.
pub fn resize_duration(task: &mut Task, working_span: i64) -> i64 {
    loop {
        let duration = (working_span - total_gap(&task.splits)).max(0);
        match task.splits.last() {
            Some(last) if last.offset >= duration => {
                task.splits.pop();
            }
            _ => return duration,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_merges_and_drops() {
        let mut splits = vec![
            Split {
                offset: 240,
                gap: 60,
            },
            Split { offset: 0, gap: 60 },
            Split {
                offset: 240,
                gap: 120,
            },
            Split {
                offset: 900,
                gap: 60,
            },
            Split {
                offset: 120,
                gap: 0,
            },
        ];
        normalize(&mut splits, 480);
        assert_eq!(
            splits,
            vec![Split {
                offset: 240,
                gap: 180
            }]
        );
    }
}
