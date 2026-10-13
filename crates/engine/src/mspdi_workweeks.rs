//! MSPDI `<WorkWeeks>` of a calendar: a name, a `TimePeriod` and seven `WeekDay`s per
//! `<WorkWeek>`, the same shape as the calendar's own `<WeekDays>`.
//!
//! The reader gives the builder the text of the elements inside a `<WorkWeek>` and the
//! finished `WeekDay`s; the writer is called from `write_calendar`.

use chrono::NaiveDate;

use crate::calendar::{Calendar, TimeRange, WorkWeek};
use crate::mspdi_report::ImportNote;
use crate::timeutil::{civil, format_iso, parse_iso};

/// A `<WorkWeek>` being read.
#[derive(Default)]
pub(crate) struct WorkWeekBuilder {
    name: String,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    /// Days the file lists; the others take the calendar's default week.
    pub week: [Option<Vec<TimeRange>>; 7],
}

impl WorkWeekBuilder {
    /// A text node inside the work week: its name and its period.
    pub fn read(&mut self, parent: Option<&str>, name: &str, value: &str) {
        match (parent, name) {
            (Some("WorkWeek"), "Name") => self.name = value.to_string(),
            (Some("TimePeriod"), "FromDate") => self.from = date_of(value),
            (Some("TimePeriod"), "ToDate") => self.to = date_of(value),
            _ => {}
        }
    }

    /// The work week, or `None` without a usable period. A day the file does not list
    /// keeps the hours of `default_week`.
    pub fn finish(self, default_week: &[Vec<TimeRange>; 7]) -> Option<WorkWeek> {
        let from = self.from?;
        let mut week: [Vec<TimeRange>; 7] = Default::default();
        for (index, day) in self.week.into_iter().enumerate() {
            week[index] = day.unwrap_or_else(|| default_week[index].clone());
        }
        Some(WorkWeek {
            name: self.name,
            from,
            to: self.to.unwrap_or(from).max(from),
            week,
        })
    }
}

/// A date with or without a time of day.
fn date_of(value: &str) -> Option<NaiveDate> {
    let value = value.trim();
    if value.contains('T') {
        parse_iso(value).map(|ms| civil(ms).0)
    } else {
        crate::timeutil::parse_date_key(value)
    }
}

/// Add the work weeks of a file to `calendar`; one that overlaps an earlier one is
/// dropped, since the engine keeps them apart, and noted.
pub(crate) fn add_all(calendar: &mut Calendar, work_weeks: Vec<WorkWeek>, notes: &mut Vec<ImportNote>) {
    for work_week in work_weeks {
        let name = work_week.name.clone();
        if calendar.set_work_week(work_week).is_err() {
            notes.push(ImportNote::WorkWeekOverlaps { calendar: calendar.name.clone(), name });
        }
    }
}

pub(crate) fn write_work_weeks(xml: &mut String, work_weeks: &[WorkWeek]) {
    if work_weeks.is_empty() {
        return;
    }
    xml.push_str("<WorkWeeks>\n");
    for work_week in work_weeks {
        xml.push_str("<WorkWeek>\n<TimePeriod>\n");
        el(xml, "FromDate", &format_iso(day_start(work_week.from)));
        el(xml, "ToDate", &format_iso(day_start(work_week.to) + 23 * 3_600_000 + 59 * 60_000));
        xml.push_str("</TimePeriod>\n");
        el(xml, "Name", &crate::mspdi::escape_text(&work_week.name));
        xml.push_str("<WeekDays>\n");
        crate::mspdi::write_week_days(xml, &work_week.week);
        xml.push_str("</WeekDays>\n</WorkWeek>\n");
    }
    xml.push_str("</WorkWeeks>\n");
}

fn day_start(date: NaiveDate) -> i64 {
    crate::timeutil::compose(date, 0)
}

fn el(xml: &mut String, name: &str, value: &str) {
    xml.push('<');
    xml.push_str(name);
    xml.push('>');
    xml.push_str(value);
    xml.push_str("</");
    xml.push_str(name);
    xml.push_str(">\n");
}
