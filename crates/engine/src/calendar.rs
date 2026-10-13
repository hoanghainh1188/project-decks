use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::error::Invalid;
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

/// A pattern of working hours that replaces the default week on every day of
/// `[from, to]` (both inclusive), like a "work week" of Microsoft Project.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkWeek {
    pub name: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub week: [Vec<TimeRange>; 7],
}

/// Week index 0 is Monday. Empty ranges mean a non-working day.
/// For a civil date the hours come from, in order of priority: its exception, the
/// work week that covers it, the default week. Work weeks never overlap and are
/// kept ordered by `from`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Calendar {
    pub id: u32,
    pub name: String,
    pub week: [Vec<TimeRange>; 7],
    pub exceptions: BTreeMap<String, Vec<TimeRange>>,
    #[serde(default)]
    pub work_weeks: Vec<WorkWeek>,
    /// Set when the calendar is based on another one; `week`, `exceptions` and
    /// `work_weeks` then hold the merged result (see `calendar_base`).
    #[serde(default)]
    pub base: Option<CalendarBase>,
}

/// What a calendar based on calendar `id` sets itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CalendarBase {
    pub id: u32,
    /// Weekdays set on this calendar; `None` takes the base calendar's day.
    pub week: [Option<Vec<TimeRange>>; 7],
    pub exceptions: BTreeMap<String, Vec<TimeRange>>,
    pub work_weeks: Vec<WorkWeek>,
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
            work_weeks: Vec::new(),
            base: None,
        }
    }

    pub fn ranges_on(&self, date: NaiveDate) -> &[TimeRange] {
        if let Some(ranges) = self.exception_on(date) {
            return ranges;
        }
        let index = date.weekday().num_days_from_monday() as usize;
        &self.week_for(date)[index]
    }

    /// The weekly pattern in force on `date`, ignoring exceptions.
    pub fn week_for(&self, date: NaiveDate) -> &[Vec<TimeRange>; 7] {
        self.work_weeks
            .iter()
            .find(|work_week| work_week.from <= date && date <= work_week.to)
            .map_or(&self.week, |work_week| &work_week.week)
    }

    /// Add a work week, or replace the one that starts on the same day. Refuses an
    /// inverted span and a span that overlaps another work week.
    pub fn set_work_week(&mut self, work_week: WorkWeek) -> Result<(), Invalid> {
        if work_week.to < work_week.from {
            return Err(Invalid::BadDate);
        }
        let overlaps = |other: &WorkWeek| {
            other.from != work_week.from
                && other.from <= work_week.to
                && work_week.from <= other.to
        };
        if self.work_weeks.iter().any(overlaps) {
            return Err(Invalid::WorkWeekOverlaps);
        }
        self.work_weeks.retain(|other| other.from != work_week.from);
        self.work_weeks.push(work_week);
        self.work_weeks.sort_by_key(|work_week| work_week.from);
        Ok(())
    }

    /// Remove the work week that starts on `from`; false when there is none.
    pub fn remove_work_week(&mut self, from: NaiveDate) -> bool {
        let before = self.work_weeks.len();
        self.work_weeks.retain(|work_week| work_week.from != from);
        self.work_weeks.len() != before
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
        self.week.iter().any(day_works)
            || self.exceptions.values().any(day_works)
            || self
                .work_weeks
                .iter()
                .any(|work_week| work_week.week.iter().any(day_works))
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
        let week = self.plain_week_minutes();
        for _ in 0..20000 {
            let (date, minute) = civil(cursor);
            // At the first working minute of a day, a whole week without exceptions can be
            // skipped in one step: long leveling delays would otherwise walk every range.
            if let Some(week) = week.filter(|week| remaining > *week) {
                if minute == self.first_minute(date) && self.no_exceptions_in_week(date) {
                    if let Some(next) = date.checked_add_days(chrono::Days::new(7)) {
                        remaining -= week;
                        cursor = self.next_work_start(compose(next, 0));
                        continue;
                    }
                }
            }
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

    /// Working minutes of a week without exceptions, when every weekday's ranges are in
    /// order and apart, so consuming them one by one adds up to exactly this.
    fn plain_week_minutes(&self) -> Option<i64> {
        if !self.work_weeks.is_empty() {
            return None;
        }
        let mut total = 0;
        for ranges in &self.week {
            let mut end = 0u16;
            for range in ranges {
                if range.start_minute < end || range.end_minute <= range.start_minute {
                    return None;
                }
                end = range.end_minute;
            }
            total += day_minutes(ranges);
        }
        (total > 0).then_some(total)
    }

    /// The minute the first working range of `date` starts, or none on a day off.
    fn first_minute(&self, date: NaiveDate) -> u16 {
        self.ranges_on(date).first().map_or(u16::MAX, |range| range.start_minute)
    }

    fn no_exceptions_in_week(&self, date: NaiveDate) -> bool {
        if self.exceptions.is_empty() {
            return true;
        }
        let Some(end) = date.checked_add_days(chrono::Days::new(7)) else {
            return false;
        };
        self.exceptions.range(date_key(date)..date_key(end)).next().is_none()
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
        let mut total = 0;
        let mut cursor = from;
        for work_week in &self.work_weeks {
            let low = work_week.from.max(cursor);
            let high = work_week.to.succ_opt().unwrap_or(work_week.to).min(to);
            if high <= low {
                continue;
            }
            total += pattern_minutes(&self.week, cursor, low);
            total += pattern_minutes(&work_week.week, low, high);
            cursor = high;
        }
        total += pattern_minutes(&self.week, cursor, to);
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
                - day_minutes(&self.week_for(date)[date.weekday().num_days_from_monday() as usize]);
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

/// Working minutes of the civil days in `[from, to)` under one weekly pattern, no
/// exceptions: whole weeks by arithmetic, then the days left over.
fn pattern_minutes(week: &[Vec<TimeRange>; 7], from: NaiveDate, to: NaiveDate) -> i64 {
    let days = (to - from).num_days();
    if days <= 0 {
        return 0;
    }
    let weekday_minutes: Vec<i64> = week.iter().map(|ranges| day_minutes(ranges)).collect();
    let first = from.weekday().num_days_from_monday() as i64;
    let mut total = days / 7 * weekday_minutes.iter().sum::<i64>();
    for offset in 0..days % 7 {
        total += weekday_minutes[((first + offset) % 7) as usize];
    }
    total
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

    /// `add_working` as it was before it skipped whole weeks.
    fn walk_add(cal: &Calendar, start: i64, minutes: i64) -> i64 {
        let mut cursor = cal.next_work_start(start);
        let mut remaining = minutes;
        loop {
            let (date, minute) = civil(cursor);
            let ranges = cal.ranges_on(date);
            let Some(range) = ranges.iter().find(|r| minute >= r.start_minute && minute < r.end_minute) else {
                cursor = cal.next_work_start(cursor);
                continue;
            };
            let available = i64::from(range.end_minute - minute);
            if remaining <= available {
                return compose(date, minute + remaining as u16);
            }
            remaining -= available;
            cursor = cal.next_work_start(compose(date, range.end_minute));
        }
    }

    #[test]
    fn add_working_matches_a_range_walk() {
        let mut cal = Calendar::standard();
        cal.week[5] = vec![TimeRange::new(9 * 60, 11 * 60)];
        let base = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        cal.set_day_off(base + chrono::Days::new(40));
        cal.set_day_off(base + chrono::Days::new(300));
        cal.exceptions.insert(
            date_key(base + chrono::Days::new(90)),
            vec![TimeRange::new(10 * 60, 12 * 60)],
        );
        let mut seed: u64 = 11;
        let mut next = |limit: i64| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as i64).rem_euclid(limit)
        };
        let origin = timestamp(2026, 1, 1, 0, 0);
        for _ in 0..2000 {
            let start = origin + next(400 * 1440) * 60_000;
            let minutes = 1 + next(200 * 480);
            assert_eq!(cal.add_working(start, minutes), walk_add(&cal, start, minutes), "{start} + {minutes}");
        }
    }

    fn summer_hours() -> WorkWeek {
        // Mon–Thu 09:00–13:00 (four hours), no Fridays.
        let mut week: [Vec<TimeRange>; 7] = Default::default();
        for day in week.iter_mut().take(4) {
            *day = vec![TimeRange::new(9 * 60, 13 * 60)];
        }
        WorkWeek {
            name: "Mùa hè".into(),
            from: NaiveDate::from_ymd_opt(2026, 1, 12).unwrap(),
            to: NaiveDate::from_ymd_opt(2026, 2, 8).unwrap(),
            week,
        }
    }

    #[test]
    fn a_work_week_replaces_the_default_week_and_an_exception_replaces_it() {
        let mut cal = Calendar::standard();
        cal.set_work_week(summer_hours()).unwrap();
        let day = |d: u32, m: u32| NaiveDate::from_ymd_opt(2026, m, d).unwrap();
        assert_eq!(cal.ranges_on(day(9, 1)).len(), 2, "before the work week");
        assert_eq!(cal.ranges_on(day(12, 1)), &[TimeRange::new(9 * 60, 13 * 60)]);
        assert!(cal.ranges_on(day(16, 1)).is_empty(), "Friday is off in the work week");
        assert!(cal.ranges_on(day(8, 2)).is_empty(), "last day, a Sunday");
        assert_eq!(cal.ranges_on(day(9, 2)).len(), 2, "after the work week");
        cal.exceptions.insert(date_key(day(13, 1)), vec![TimeRange::new(10 * 60, 11 * 60)]);
        assert_eq!(cal.ranges_on(day(13, 1)), &[TimeRange::new(10 * 60, 11 * 60)]);
        // A day's work now takes the work week's four hours.
        assert_eq!(cal.add_working(timestamp(2026, 1, 12, 9, 0), 240), timestamp(2026, 1, 12, 13, 0));
        assert_eq!(cal.add_working(timestamp(2026, 1, 12, 9, 0), 480), timestamp(2026, 1, 14, 12, 0));
    }

    #[test]
    fn work_weeks_stay_apart_and_ordered() {
        let mut cal = Calendar::standard();
        let mut later = summer_hours();
        later.name = "Sau".into();
        later.from = NaiveDate::from_ymd_opt(2026, 3, 2).unwrap();
        later.to = NaiveDate::from_ymd_opt(2026, 3, 8).unwrap();
        cal.set_work_week(later).unwrap();
        cal.set_work_week(summer_hours()).unwrap();
        assert_eq!(cal.work_weeks[0].name, "Mùa hè");
        let mut overlapping = summer_hours();
        overlapping.from = NaiveDate::from_ymd_opt(2026, 2, 8).unwrap();
        overlapping.to = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        assert_eq!(cal.set_work_week(overlapping), Err(Invalid::WorkWeekOverlaps));
        let mut inverted = summer_hours();
        inverted.to = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        assert_eq!(cal.set_work_week(inverted), Err(Invalid::BadDate));
        // The same start replaces.
        let mut renamed = summer_hours();
        renamed.name = "Mới".into();
        cal.set_work_week(renamed).unwrap();
        assert_eq!(cal.work_weeks.len(), 2);
        assert_eq!(cal.work_weeks[0].name, "Mới");
        assert!(cal.remove_work_week(NaiveDate::from_ymd_opt(2026, 1, 12).unwrap()));
        assert!(!cal.remove_work_week(NaiveDate::from_ymd_opt(2026, 1, 12).unwrap()));
    }

    #[test]
    fn working_time_and_add_working_with_work_weeks_match_a_day_walk() {
        let mut cal = Calendar::standard();
        cal.week[5] = vec![TimeRange::new(9 * 60, 11 * 60)];
        cal.set_work_week(summer_hours()).unwrap();
        let mut second = summer_hours();
        second.from = NaiveDate::from_ymd_opt(2026, 4, 6).unwrap();
        second.to = NaiveDate::from_ymd_opt(2026, 5, 31).unwrap();
        cal.set_work_week(second).unwrap();
        let base = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        cal.set_day_off(base + chrono::Days::new(20));
        cal.set_day_off(base + chrono::Days::new(100));
        let mut seed: u64 = 3;
        let mut next = |limit: i64| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as i64).rem_euclid(limit)
        };
        let origin = timestamp(2026, 1, 1, 0, 0);
        for _ in 0..2000 {
            let start = origin + next(300 * 1440) * 60_000;
            let end = start + next(150 * 1440) * 60_000;
            assert_eq!(cal.working_between(start, end), walk_between(&cal, start, end), "{start} → {end}");
            let minutes = 1 + next(90 * 480);
            assert_eq!(cal.add_working(start, minutes), walk_add(&cal, start, minutes), "{start} + {minutes}");
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
