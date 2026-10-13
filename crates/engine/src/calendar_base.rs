//! Calendars based on another calendar, as in Microsoft Project.
//!
//! A based calendar stores only what it sets itself (`CalendarBase`: the weekdays it
//! sets, its exceptions and its work weeks) and takes everything else from its base.
//! The scheduler never looks at the base: `refresh` writes the merged result into the
//! calendar's own `week`, `exceptions` and `work_weeks` before every schedule, so a
//! holiday added to the project calendar reaches every resource calendar based on it.
//!
//! Edits go through the merged fields like on any calendar; `capture` then keeps, as
//! the calendar's own part, whatever differs from the base. A day equal to the base's
//! follows the base from then on. An exception of the base cannot be removed on the
//! based calendar, only overridden with other hours.

use std::collections::{BTreeMap, HashSet};

use chrono::NaiveDate;

use crate::calendar::{Calendar, CalendarBase, TimeRange, WorkWeek};
use crate::timeutil::date_key;
use crate::error::{EngineError, Invalid};
use crate::model::Project;

/// The calendar's own part, as it differs from `base`.
pub fn capture(calendar: &Calendar, base: &Calendar) -> CalendarBase {
    let mut week: [Option<Vec<TimeRange>>; 7] = Default::default();
    for (index, day) in calendar.week.iter().enumerate() {
        if *day != base.week[index] {
            week[index] = Some(day.clone());
        }
    }
    let exceptions: BTreeMap<String, Vec<TimeRange>> = calendar
        .exceptions
        .iter()
        .filter(|(key, ranges)| base.exceptions.get(*key) != Some(*ranges))
        .map(|(key, ranges)| (key.clone(), ranges.clone()))
        .collect();
    let work_weeks: Vec<WorkWeek> = calendar
        .work_weeks
        .iter()
        .filter(|week| !base.work_weeks.contains(week))
        .cloned()
        .collect();
    CalendarBase {
        id: base.id,
        week,
        exceptions,
        work_weeks,
    }
}

/// Write `own` over `base` into `calendar`'s merged fields.
fn merge(calendar: &mut Calendar, base: &Calendar, own: &CalendarBase) {
    for index in 0..7 {
        calendar.week[index] = own.week[index]
            .clone()
            .unwrap_or_else(|| base.week[index].clone());
    }
    let mut exceptions = base.exceptions.clone();
    exceptions.extend(
        own.exceptions
            .iter()
            .map(|(key, ranges)| (key.clone(), ranges.clone())),
    );
    calendar.exceptions = exceptions;
    let mut work_weeks = own.work_weeks.clone();
    work_weeks.extend(
        base.work_weeks
            .iter()
            .filter(|week| {
                !own.work_weeks
                    .iter()
                    .any(|mine| mine.from <= week.to && week.from <= mine.to)
            })
            .cloned(),
    );
    work_weeks.sort_by_key(|week| week.from);
    calendar.work_weeks = work_weeks;
}

/// Recompute every based calendar from its base, bases first. A calendar whose base no
/// longer exists keeps its merged hours and stands alone.
pub fn refresh(project: &mut Project) {
    if project
        .calendars
        .iter()
        .all(|calendar| calendar.base.is_none())
    {
        return;
    }
    let mut done: HashSet<u32> = project
        .calendars
        .iter()
        .filter(|calendar| calendar.base.is_none())
        .map(|calendar| calendar.id)
        .collect();
    // Each pass settles the calendars whose base is settled; a chain of n needs n passes.
    for _ in 0..project.calendars.len() {
        let mut progressed = false;
        for index in 0..project.calendars.len() {
            let Some(own) = project.calendars[index].base.clone() else {
                continue;
            };
            let id = project.calendars[index].id;
            if done.contains(&id) || !done.contains(&own.id) {
                if !done.contains(&id) && project.calendar_by_id(own.id).is_none() {
                    project.calendars[index].base = None;
                    done.insert(id);
                    progressed = true;
                }
                continue;
            }
            let base = project
                .calendar_by_id(own.id)
                .cloned()
                .expect("settled base exists");
            merge(&mut project.calendars[index], &base, &own);
            done.insert(id);
            progressed = true;
        }
        if !progressed {
            break;
        }
    }
    // Calendars left are in a loop of bases (only a file can make one): they stand
    // alone with what they set themselves.
    for calendar in &mut project.calendars {
        if !done.contains(&calendar.id) {
            calendar.base = None;
        }
    }
}

/// Whether a day of `[from, to]` has an exception that `calendar_id` only inherits:
/// removing it there would not stick, since the base brings it back.
pub fn inherits_exception(project: &Project, calendar_id: u32, from: NaiveDate, to: NaiveDate) -> bool {
    let Some(calendar) = project.calendar_by_id(calendar_id) else {
        return false;
    };
    let Some(own) = &calendar.base else {
        return false;
    };
    let (low, high) = if to < from { (to, from) } else { (from, to) };
    let Some(after) = high.succ_opt() else {
        return false;
    };
    calendar
        .exceptions
        .range(date_key(low)..date_key(after))
        .any(|(key, _)| !own.exceptions.contains_key(key))
}

/// Whether the work week starting on `from` is one `calendar_id` only inherits.
pub fn inherits_work_week(project: &Project, calendar_id: u32, from: NaiveDate) -> bool {
    project.calendar_by_id(calendar_id).is_some_and(|calendar| {
        calendar.base.as_ref().is_some_and(|own| {
            calendar.work_weeks.iter().any(|week| week.from == from)
                && !own.work_weeks.iter().any(|week| week.from == from)
        })
    })
}

/// Before a based calendar sets a work week over `[from, to]`, drop the inherited
/// ones it overlaps: its own week replaces them there, as `merge` does.
pub fn make_room_for_work_week(project: &mut Project, calendar_id: u32, from: NaiveDate, to: NaiveDate) {
    let Some(calendar) = project.calendar_mut_by_id(calendar_id) else {
        return;
    };
    let Some(own) = calendar.base.clone() else {
        return;
    };
    calendar.work_weeks.retain(|week| {
        let inherited = !own.work_weeks.contains(week);
        !(inherited && week.from <= to && from <= week.to)
    });
}

/// Base `calendar_id` on `base_id`, or make it stand alone with `None`. The calendar
/// keeps its hours: what differs from the base becomes its own part.
pub fn set_base(
    project: &mut Project,
    calendar_id: u32,
    base_id: Option<u32>,
) -> Result<(), EngineError> {
    project
        .calendar_by_id(calendar_id)
        .ok_or_else(|| EngineError::from(Invalid::NoSuchCalendar))?;
    let Some(base_id) = base_id else {
        if let Some(calendar) = project.calendar_mut_by_id(calendar_id) {
            calendar.base = None;
        }
        return Ok(());
    };
    let base = project
        .calendar_by_id(base_id)
        .cloned()
        .ok_or_else(|| EngineError::from(Invalid::NoSuchCalendar))?;
    if leads_to(project, base_id, calendar_id) {
        return Err(Invalid::CalendarBaseLoop.into());
    }
    let calendar = project
        .calendar_mut_by_id(calendar_id)
        .expect("checked above");
    calendar.base = Some(capture(calendar, &base));
    Ok(())
}

/// Whether following bases from `from` reaches `target` (itself included).
fn leads_to(project: &Project, from: u32, target: u32) -> bool {
    let mut at = Some(from);
    for _ in 0..=project.calendars.len() {
        match at {
            Some(id) if id == target => return true,
            Some(id) => {
                at = project
                    .calendar_by_id(id)
                    .and_then(|calendar| calendar.base.as_ref().map(|base| base.id))
            }
            None => return false,
        }
    }
    false
}

/// After an edit of `calendar_id`'s merged fields, keep what differs from its base.
pub fn after_edit(project: &mut Project, calendar_id: u32) {
    let Some(base_id) = project
        .calendar_by_id(calendar_id)
        .and_then(|calendar| calendar.base.as_ref().map(|base| base.id))
    else {
        return;
    };
    let Some(base) = project.calendar_by_id(base_id).cloned() else {
        return;
    };
    if let Some(calendar) = project.calendar_mut_by_id(calendar_id) {
        calendar.base = Some(capture(calendar, &base));
    }
}
