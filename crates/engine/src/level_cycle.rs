//! Ending a leveling search that can never succeed: the task's calendar works hours its
//! resources never do (or never have the units for), so some of its work clashes
//! wherever it starts.
//!
//! Past the placed work of every checked resource (`horizon`), a round sees no load:
//! the clash, the room after it and the slide come from the calendars and capacities
//! alone. Past the last exception and work week of those calendars and the last change
//! of those capacities (`steady`), they repeat every week, so a round starting a whole
//! number of weeks later is the same round moved by those weeks, and so is every round
//! after it. When a start from there on lands on the same point of the week as an
//! earlier clashing start from there on, the search has entered a loop of clashing
//! starts and no later start can fit. Every search that does fit ends exactly as
//! before.

use std::collections::BTreeSet;

use chrono::NaiveDate;

use crate::calendar::Calendar;
#[cfg(test)]
use crate::level::{CAME_ROUND, SEARCH_IN_FULL};
use crate::timeutil::{compose, parse_date_key};

/// Wall-clock time has no daylight saving, so every week is this long.
const WEEK: i64 = 7 * 86_400_000;

pub(crate) struct Cycle {
    horizon: i64,
    steady: Option<i64>,
    seen: BTreeSet<i64>,
}

impl Cycle {
    /// `horizon` is where the placed work of every checked resource ends.
    pub(crate) fn new(horizon: i64) -> Self {
        Self {
            horizon,
            steady: None,
            seen: BTreeSet::new(),
        }
    }

    /// Whether `start`, which clashes, repeats an earlier clashing start. `steady` gives,
    /// once asked, the instant from which the calendars and capacities repeat weekly.
    pub(crate) fn came_round(&mut self, start: i64, steady: impl FnOnce() -> i64) -> bool {
        if start < self.horizon {
            return false;
        }
        let horizon = self.horizon;
        let steady = *self.steady.get_or_insert_with(|| steady().max(horizon));
        let came_round = start >= steady && !self.seen.insert(start.rem_euclid(WEEK));
        #[cfg(test)]
        if came_round {
            CAME_ROUND.with(|count| count.set(count.get() + 1));
            // Searching on: the same start of the week is not counted twice.
            self.seen.clear();
            self.steady = Some(i64::MAX);
            return !SEARCH_IN_FULL.with(std::cell::Cell::get);
        }
        came_round
    }
}

/// Midnight after the last day a calendar sets apart from its weekly pattern (an
/// exception or a work week), from which on it repeats every week.
pub(crate) fn weekly_from(calendar: &Calendar) -> i64 {
    let last_exception = calendar
        .exceptions
        .keys()
        .filter_map(|key| parse_date_key(key))
        .max();
    let last_work_week = calendar.work_weeks.iter().map(|week| week.to).max();
    midnight_after(last_exception.max(last_work_week))
}

/// Midnight starting `date`, or the earliest instant without one.
pub(crate) fn midnight_of(date: Option<NaiveDate>) -> i64 {
    date.map_or(i64::MIN, |date| compose(date, 0))
}

fn midnight_after(date: Option<NaiveDate>) -> i64 {
    match date {
        Some(date) => date.succ_opt().map_or(i64::MAX, |next| compose(next, 0)),
        None => i64::MIN,
    }
}
