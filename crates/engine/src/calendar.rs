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

    pub fn ranges_on(&self, date: NaiveDate) -> &[TimeRange] {
        if let Some(ranges) = self.exception_on(date) {
            return ranges;
        }
        let index = date.weekday().num_days_from_monday() as usize;
        &self.week[index]
    }

    /// The scheduler asks for every civil day it walks, so the lookup must not allocate.
    fn exception_on(&self, date: NaiveDate) -> Option<&Vec<TimeRange>> {
        if self.exceptions.is_empty() {
            return None;
        }
        match key_bytes(date) {
            Some(bytes) => std::str::from_utf8(&bytes)
                .ok()
                .and_then(|key| self.exceptions.get(key)),
            None => self.exceptions.get(&date_key(date)),
        }
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
        if start_date == end_date {
            return minutes_within(self.ranges_on(start_date), start_minute, end_minute);
        }
        let Some(first_full) = start_date.succ_opt() else {
            return 0;
        };
        minutes_within(self.ranges_on(start_date), start_minute, 24 * 60)
            + self.full_days_between(first_full, end_date)
            + minutes_within(self.ranges_on(end_date), 0, end_minute)
    }

    /// Working minutes on the whole civil days in `[from, to)`: whole weeks by
    /// arithmetic, then the exceptions that fall inside the span.
    fn full_days_between(&self, from: NaiveDate, to: NaiveDate) -> i64 {
        let days = (to - from).num_days();
        if days <= 0 {
            return 0;
        }
        let weekday_minutes: Vec<i64> = self.week.iter().map(|ranges| day_minutes(ranges)).collect();
        let first = from.weekday().num_days_from_monday() as i64;
        let mut total = days / 7 * weekday_minutes.iter().sum::<i64>();
        for offset in 0..days % 7 {
            total += weekday_minutes[((first + offset) % 7) as usize];
        }
        if self.exceptions.is_empty() {
            return total;
        }
        for (key, ranges) in self.exceptions.range(date_key(from)..date_key(to)) {
            // Only canonical keys ever match a day; `ranges_on` ignores the rest too.
            let Some(date) = parse_date_key(key).filter(|date| date_key(*date) == *key) else {
                continue;
            };
            if date < from || date >= to {
                continue;
            }
            total += day_minutes(ranges)
                - weekday_minutes[date.weekday().num_days_from_monday() as usize];
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

/// Minutes of `ranges` that fall inside `[from, to)` of one civil day.
fn minutes_within(ranges: &[TimeRange], from: u16, to: u16) -> i64 {
    ranges
        .iter()
        .map(|range| {
            let low = from.max(range.start_minute);
            let high = to.min(range.end_minute);
            i64::from(high.saturating_sub(low))
        })
        .sum()
}

fn day_minutes(ranges: &[TimeRange]) -> i64 {
    minutes_within(ranges, 0, 24 * 60)
}

/// `date_key` written into a stack buffer, for years 0..=9999.
fn key_bytes(date: NaiveDate) -> Option<[u8; 10]> {
    let year = u32::try_from(date.year()).ok().filter(|year| *year <= 9999)?;
    let digit = |value: u32| b'0' + (value % 10) as u8;
    let (month, day) = (date.month(), date.day());
    Some([
        digit(year / 1000),
        digit(year / 100),
        digit(year / 10),
        digit(year),
        b'-',
        digit(month / 10),
        digit(month),
        b'-',
        digit(day / 10),
        digit(day),
    ])
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

    /// Day-by-day count, the way `working_between` worked before it skipped whole weeks.
    fn walk_between(cal: &Calendar, start: i64, end: i64) -> i64 {
        let (start_date, start_minute) = civil(start);
        let (end_date, end_minute) = civil(end);
        let mut total = 0;
        let mut date = start_date;
        loop {
            let from = if date == start_date { start_minute } else { 0 };
            let to = if date == end_date { end_minute } else { 24 * 60 };
            total += minutes_within(cal.ranges_on(date), from, to);
            if date >= end_date {
                return total;
            }
            date = date.succ_opt().unwrap();
        }
    }

    #[test]
    fn working_between_matches_a_day_walk() {
        let mut cal = Calendar::standard();
        cal.week[5] = vec![TimeRange::new(9 * 60, 11 * 60)];
        let base = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        cal.set_day_off(base + chrono::Days::new(10));
        cal.set_day_off(base + chrono::Days::new(400));
        cal.exceptions.insert(
            date_key(base + chrono::Days::new(13)),
            vec![TimeRange::new(10 * 60, 12 * 60)],
        );
        // Keys that never match a day must not count either.
        cal.exceptions.insert("2026-1-20".into(), Vec::new());
        let mut seed: u64 = 7;
        let mut next = |limit: i64| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as i64).rem_euclid(limit)
        };
        let origin = timestamp(2026, 1, 1, 0, 0);
        for _ in 0..2000 {
            let start = origin + next(700 * 1440) * 60_000;
            let end = start + next(120 * 1440) * 60_000;
            assert_eq!(
                cal.working_between(start, end),
                walk_between(&cal, start, end),
                "{start} → {end}"
            );
        }
    }

    #[test]
    fn exception_lookup_matches_date_key() {
        for (year, month, day) in [(2026, 1, 5), (987, 12, 31), (9999, 2, 9)] {
            let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
            let bytes = super::key_bytes(date).unwrap();
            assert_eq!(std::str::from_utf8(&bytes).unwrap(), date_key(date));
        }
        let mut cal = Calendar::standard();
        let holiday = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        cal.set_day_off(holiday);
        assert!(cal.ranges_on(holiday).is_empty());
        assert_eq!(cal.ranges_on(holiday.succ_opt().unwrap()).len(), 2);
    }
}
