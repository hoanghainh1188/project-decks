//! Calendar exceptions in MSPDI, from either place a file can put them.
//!
//! Newer files list them under `<Exceptions><Exception>`. Project 2003 and some other
//! tools write them as a `<WeekDay>` whose `<DayType>` is 0 (the schema's "exception"),
//! with a `<TimePeriod>`, `<DayWorking>` and optional `<WorkingTimes>`. Both end up as
//! entries of `Calendar.exceptions`, one per civil day.

use std::collections::BTreeMap;

use crate::calendar::TimeRange;
use crate::timeutil::{civil, date_key, parse_iso};

/// The longest span written out day by day.
const MAX_SPAN_DAYS: usize = 3660;

/// Put one exception, `from` to `to` inclusive, into `exceptions`: no ranges for a day
/// off, `ranges` for a working day. A working exception with no hours is ignored, since
/// that is not a day off. `to` defaults to `from`.
///
/// Returns `false` when `from` is missing or not a date, so nothing could be stored.
pub(crate) fn add_exception(
    exceptions: &mut BTreeMap<String, Vec<TimeRange>>,
    from: Option<&str>,
    to: Option<&str>,
    working: bool,
    ranges: &[TimeRange],
) -> bool {
    let Some(start_ms) = from.and_then(parse_iso) else {
        return false;
    };
    if working && ranges.is_empty() {
        return true;
    }
    let end_ms = to.and_then(parse_iso).unwrap_or(start_ms);
    let day = if working { ranges.to_vec() } else { Vec::new() };
    let mut date = civil(start_ms).0;
    let end = civil(end_ms).0;
    for _ in 0..MAX_SPAN_DAYS {
        exceptions.insert(date_key(date), day.clone());
        if date >= end {
            break;
        }
        date = match date.succ_opt() {
            Some(next) => next,
            None => break,
        };
    }
    true
}
