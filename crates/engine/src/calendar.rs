use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::timeutil::{civil, compose, date_key, parse_date_key};

/// A half-open working window `[start_minute, end_minute)` from local midnight.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start_minute: u16,
    pub end_minute: u16,
}

impl TimeRange {
    pub fn new(start_minute: u16, end_minute: u16) -> Self {
        Self {
            start_minute,
            end_minute,
        }
    }
}

/// Week index 0 is Monday. Empty ranges mean a non-working day.
/// Exception values replace the weekday pattern for that civil date.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Calendar {
    pub id: u32,
    pub name: String,
    pub week: [Vec<TimeRange>; 7],
    pub exceptions: BTreeMap<String, Vec<TimeRange>>,
}

impl Calendar {
    pub fn standard() -> Self {
        let work = vec![
            TimeRange::new(8 * 60, 12 * 60),
            TimeRange::new(13 * 60, 17 * 60),
        ];
        let mut week: [Vec<TimeRange>; 7] = Default::default();
        for day in week.iter_mut().take(5) {
            *day = work.clone();
        }
        Self {
            id: 1,
            name: "Chuẩn".into(),
            week,
            exceptions: BTreeMap::new(),
        }
    }

    pub fn ranges_on(&self, date: NaiveDate) -> Vec<TimeRange> {
        let key = date_key(date);
        if let Some(ranges) = self.exceptions.get(&key) {
            return ranges.clone();
        }
        let index = date.weekday().num_days_from_monday() as usize;
        self.week[index].clone()
    }

    pub fn set_day_off(&mut self, date: NaiveDate) {
        self.exceptions.insert(date_key(date), Vec::new());
    }

    pub fn clear_exception(&mut self, date: NaiveDate) {
        self.exceptions.remove(&date_key(date));
    }

    /// Write the same exception onto every civil day in the inclusive span.
    /// Returns false when the span is longer than the scheduling horizon.
    pub fn set_exception_span(
        &mut self,
        from: NaiveDate,
        to: NaiveDate,
        ranges: Vec<TimeRange>,
    ) -> bool {
        each_civil_day(from, to, |date| {
            self.exceptions.insert(date_key(date), ranges.clone());
        })
    }

    /// Drop exceptions on every civil day in the inclusive span.
    pub fn clear_exception_span(&mut self, from: NaiveDate, to: NaiveDate) -> bool {
        each_civil_day(from, to, |date| {
            self.exceptions.remove(&date_key(date));
        })
    }

    pub fn has_working_time(&self) -> bool {
        let day_works = |ranges: &Vec<TimeRange>| {
            ranges
                .iter()
                .any(|range| range.start_minute < range.end_minute)
        };
        self.week.iter().any(day_works) || self.exceptions.values().any(day_works)
    }

    pub fn is_working_instant(&self, ms: i64) -> bool {
        let (date, minute) = civil(ms);
        self.ranges_on(date)
            .iter()
            .any(|r| minute >= r.start_minute && minute < r.end_minute)
    }

    /// First instant at which work may start, at or after `ms`.
    /// An instant exactly on an interval end rolls forward.
    pub fn next_work_start(&self, ms: i64) -> i64 {
        let (mut date, mut minute) = civil(ms);
        for _ in 0..3660 {
            for range in self.ranges_on(date) {
                if minute < range.end_minute {
                    let start = minute.max(range.start_minute);
                    if start < range.end_minute {
                        return compose(date, start);
                    }
                }
            }
            date = match date.succ_opt() {
                Some(next) => next,
                None => return ms,
            };
            minute = 0;
        }
        ms
    }

    /// Finish instant after consuming `minutes` of working time starting at `start`.
    /// Zero minutes snaps to the next working instant (a milestone).
    pub fn add_working(&self, start: i64, minutes: i64) -> i64 {
        if minutes < 0 {
            return self.subtract_working(start, -minutes);
        }
        let mut cursor = self.next_work_start(start);
        if minutes == 0 {
            return cursor;
        }
        let mut remaining = minutes;
        for _ in 0..20000 {
            let (date, minute) = civil(cursor);
            let ranges = self.ranges_on(date);
            if let Some(range) = ranges
                .iter()
                .find(|r| minute >= r.start_minute && minute < r.end_minute)
            {
                let available = i64::from(range.end_minute - minute);
                if remaining <= available {
                    return compose(date, minute + remaining as u16);
                }
                remaining -= available;
                let boundary = compose(date, range.end_minute);
                let next = self.next_work_start(boundary);
                if next == cursor {
                    return cursor;
                }
                cursor = next;
            } else {
                let next = self.next_work_start(cursor);
                if next == cursor {
                    return cursor;
                }
                cursor = next;
            }
        }
        cursor
    }

    /// Move backward by working minutes. The result is a valid start instant.
    pub fn subtract_working(&self, instant: i64, minutes: i64) -> i64 {
        if minutes < 0 {
            return self.add_working(instant, -minutes);
        }
        if minutes == 0 {
            return self.next_work_start(instant);
        }
        let mut remaining = minutes;
        let (mut date, mut minute) = civil(instant);
        for _ in 0..3660 {
            let ranges = self.ranges_on(date);
            for range in ranges.iter().rev() {
                let cursor_min = if minute > range.end_minute {
                    range.end_minute
                } else if minute > range.start_minute {
                    minute
                } else {
                    continue;
                };
                let available = i64::from(cursor_min - range.start_minute);
                if available <= 0 {
                    continue;
                }
                if remaining <= available {
                    return compose(date, cursor_min - remaining as u16);
                }
                remaining -= available;
                minute = range.start_minute;
            }
            date = match date.pred_opt() {
                Some(prev) => prev,
                None => return compose(date, 0),
            };
            minute = 24 * 60;
        }
        compose(date, 0)
    }

    /// Working minutes in `[start, end)`. Negative when `end < start`.
    pub fn working_between(&self, start: i64, end: i64) -> i64 {
        if start == end {
            return 0;
        }
        if end < start {
            return -self.working_between(end, start);
        }
        let (start_date, start_minute) = civil(start);
        let (end_date, end_minute) = civil(end);
        let mut total = 0i64;
        let mut date = start_date;
        for _ in 0..3660 {
            for range in self.ranges_on(date) {
                let from = if date == start_date {
                    start_minute.max(range.start_minute)
                } else {
                    range.start_minute
                };
                let to_limit = if date == end_date {
                    end_minute
                } else {
                    24 * 60
                };
                let to = to_limit.min(range.end_minute);
                if from < to && from < range.end_minute && to > range.start_minute {
                    total += i64::from(to - from);
                }
            }
            if date >= end_date {
                break;
            }
            date = match date.succ_opt() {
                Some(next) => next,
                None => break,
            };
        }
        total
    }

    /// Start instant that is `lag` working minutes after `reference`.
    ///
    /// Lag 0 yields the next working start at or after `reference`, so a finish
    /// on 17:00 starts the successor the next morning. Positive lag consumes
    /// working time first, then snaps to a start. Negative lag is lead.
    pub fn add_lag(&self, reference: i64, lag: i64) -> i64 {
        if lag == 0 {
            self.next_work_start(reference)
        } else if lag > 0 {
            self.next_work_start(self.add_working(reference, lag))
        } else {
            self.subtract_working(reference, -lag)
        }
    }

    /// Earliest finish instant that is `lag` working minutes after `reference`.
    pub fn finish_not_before(&self, reference: i64, lag: i64) -> i64 {
        if lag == 0 {
            reference
        } else if lag > 0 {
            self.add_working(reference, lag)
        } else {
            self.subtract_working(reference, -lag)
        }
    }

    /// Latest reference instant whose [`add_lag`](Self::add_lag) is still `<= target_start`.
    pub fn latest_ref_for_start(&self, target_start: i64, lag: i64) -> i64 {
        if lag > 0 {
            self.subtract_working(target_start, lag)
        } else if lag < 0 {
            self.add_working(target_start, -lag)
        } else {
            target_start
        }
    }

    pub fn latest_ref_for_finish(&self, target_finish: i64, lag: i64) -> i64 {
        self.latest_ref_for_start(target_finish, lag)
    }

    /// Place a fixed duration so the finish is at least `finish_target`.
    /// A target that falls in a non-working gap (Monday 08:00 after Friday 17:00)
    /// is kept as the finish when no working time sits between them.
    pub fn place_by_finish(&self, finish_target: i64, duration: i64) -> (i64, i64) {
        if duration <= 0 {
            if self.is_working_instant(finish_target) || self.is_interval_end(finish_target) {
                return (finish_target, finish_target);
            }
            let snapped = self.next_work_start(finish_target);
            return (snapped, snapped);
        }
        let start = self.subtract_working(finish_target, duration);
        let natural = self.add_working(start, duration);
        if natural >= finish_target {
            (start, natural)
        } else if self.working_between(natural, finish_target) == 0 {
            (start, finish_target)
        } else {
            (start, natural)
        }
    }

    fn is_interval_end(&self, ms: i64) -> bool {
        let (date, minute) = civil(ms);
        self.ranges_on(date).iter().any(|r| r.end_minute == minute)
    }

    pub fn day_off_keys(&self) -> Vec<String> {
        self.exceptions
            .iter()
            .filter(|(_, ranges)| ranges.is_empty())
            .map(|(k, _)| k.clone())
            .collect()
    }

    pub fn exception_date(key: &str) -> Option<NaiveDate> {
        parse_date_key(key)
    }
}

fn each_civil_day(from: NaiveDate, to: NaiveDate, mut visit: impl FnMut(NaiveDate)) -> bool {
    let (mut date, end) = if to < from { (to, from) } else { (from, to) };
    for _ in 0..3660 {
        visit(date);
        if date >= end {
            return true;
        }
        date = match date.succ_opt() {
            Some(next) => next,
            None => return false,
        };
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeutil::timestamp;

    fn monday() -> i64 {
        timestamp(2026, 1, 5, 8, 0)
    }

    #[test]
    fn one_day_finishes_at_1700() {
        let cal = Calendar::standard();
        let finish = cal.add_working(monday(), 480);
        assert_eq!(finish, timestamp(2026, 1, 5, 17, 0));
    }

    #[test]
    fn lag_zero_rolls_past_end_of_day() {
        let cal = Calendar::standard();
        let finish = timestamp(2026, 1, 5, 17, 0);
        assert_eq!(cal.add_lag(finish, 0), timestamp(2026, 1, 6, 8, 0));
    }

    #[test]
    fn one_day_lag_skips_a_holiday() {
        let mut cal = Calendar::standard();
        cal.set_day_off(NaiveDate::from_ymd_opt(2026, 1, 6).unwrap());
        let finish = timestamp(2026, 1, 5, 17, 0);
        // Tuesday is off, so the lag day is Wednesday and the successor starts Thursday.
        assert_eq!(cal.add_lag(finish, 480), timestamp(2026, 1, 8, 8, 0));
    }

    #[test]
    fn subtract_one_day_from_monday_morning_lands_friday() {
        let cal = Calendar::standard();
        let start = cal.subtract_working(monday(), 480);
        assert_eq!(start, timestamp(2026, 1, 2, 8, 0));
    }

    #[test]
    fn working_between_weekend_is_zero() {
        let cal = Calendar::standard();
        let friday = timestamp(2026, 1, 2, 17, 0);
        assert_eq!(cal.working_between(friday, monday()), 0);
        assert_eq!(cal.working_between(timestamp(2026, 1, 2, 8, 0), monday()), 480);
    }
}
