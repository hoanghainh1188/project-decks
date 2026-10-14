//! MSPDI `<AvailabilityPeriods>` of a resource: `AvailableFrom`, `AvailableTo` and
//! `AvailableUnits` (a percentage) per `<AvailabilityPeriod>`.
//!
//! Project has a resource away outside its availability table, while Sheafly gives it its
//! max units there. Reading, the days a file's table leaves out become periods of zero
//! units; a table of one period with no dates is just the resource's units. Writing,
//! the table covers all time, the days outside Sheafly's periods at the max units. A
//! missing date, or one as far out as Project's "NA" (assumed: a start on 1984-01-01 or
//! before, an end on 2049-12-31 or after; not checked against a file Project wrote), is
//! no limit.

use chrono::NaiveDate;

use crate::availability::{self, AvailabilityPeriod};
use crate::model::Resource;
use crate::mspdi_report::ImportNote;
use crate::timeutil::{civil, compose, format_iso, parse_date_key, parse_iso};

/// An `<AvailabilityPeriod>` being read.
#[derive(Default)]
pub(crate) struct PeriodBuilder {
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    percent: Option<f64>,
}

impl PeriodBuilder {
    /// A text node inside the period.
    pub fn read(&mut self, parent: Option<&str>, name: &str, value: &str) {
        if parent != Some("AvailabilityPeriod") {
            return;
        }
        match name {
            "AvailableFrom" => self.from = date_of(value).filter(|date| Some(*date) > ymd(1984, 1, 1)),
            "AvailableTo" => self.to = date_of(value).filter(|date| Some(*date) < ymd(2049, 12, 31)),
            "AvailableUnits" => self.percent = value.trim().parse().ok(),
            _ => {}
        }
    }

    pub fn finish(self) -> AvailabilityPeriod {
        // Rounded so 30 % reads back as 0.3, not 0.30000000000000004.
        let units = (self.percent.unwrap_or(100.0) * 1e4).round() / 1e6;
        AvailabilityPeriod {
            from: self.from,
            to: self.to,
            units: if units.is_finite() { units.max(0.0) } else { 0.0 },
        }
    }
}

/// A date with or without a time of day.
fn date_of(value: &str) -> Option<NaiveDate> {
    let value = value.trim();
    if value.contains('T') {
        parse_iso(value).map(|ms| civil(ms).0)
    } else {
        parse_date_key(value)
    }
}

fn ymd(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(year, month, day)
}

/// Give `resource` the periods a file listed for it, read the way Project reads them.
/// A period that overlaps an earlier one or ends before it starts is left out and noted;
/// a table with nothing left leaves the resource its max units.
pub(crate) fn apply(resource: &mut Resource, periods: Vec<AvailabilityPeriod>, notes: &mut Vec<ImportNote>) {
    if periods.is_empty() {
        return;
    }
    let mut kept: Vec<AvailabilityPeriod> = Vec::new();
    for period in availability::sorted(periods) {
        let bad_range = matches!((period.from, period.to), (Some(from), Some(to)) if from > to);
        if bad_range || kept.last().is_some_and(|last| availability::overlap(last, &period)) {
            notes.push(ImportNote::AvailabilityDropped { resource: resource.name.clone() });
        } else {
            kept.push(period);
        }
    }
    if kept.is_empty() {
        return;
    }
    if let [only] = kept.as_slice() {
        if only.from.is_none() && only.to.is_none() {
            resource.max_units = only.units;
            return;
        }
    }
    // Project has the resource away on the days the table leaves out.
    let away = |from, to| AvailabilityPeriod { from, to, units: 0.0 };
    let mut filled = Vec::with_capacity(kept.len() * 2 + 1);
    let mut cursor: Option<Option<NaiveDate>> = Some(None);
    for period in kept {
        if let (Some(day), Some(from)) = (cursor, period.from) {
            if day.is_none_or(|day| day < from) {
                filled.push(away(day, from.pred_opt()));
            }
        }
        cursor = period.to.map(|to| to.succ_opt());
        filled.push(period);
    }
    if let Some(day) = cursor {
        filled.push(away(day, None));
    }
    resource.availability = filled;
}

/// The resource's availability as Project reads it: every day covered, those outside
/// Sheafly's periods at the max units. Nothing without periods.
pub(crate) fn write(xml: &mut String, resource: &Resource) {
    if resource.availability.is_empty() {
        return;
    }
    let normal = |from, to| AvailabilityPeriod { from, to, units: resource.max_units };
    let mut table = Vec::with_capacity(resource.availability.len() * 2 + 1);
    // The first day not covered yet; `Some(None)` is the start of time, `None` the end.
    let mut cursor: Option<Option<NaiveDate>> = Some(None);
    for period in &resource.availability {
        if let (Some(day), Some(from)) = (cursor, period.from) {
            if day.is_none_or(|day| day < from) {
                table.push(normal(day, from.pred_opt()));
            }
        }
        cursor = period.to.map(|to| to.succ_opt());
        table.push(period.clone());
    }
    if let Some(day) = cursor {
        table.push(normal(day, None));
    }
    xml.push_str("<AvailabilityPeriods>\n");
    for period in table {
        xml.push_str("<AvailabilityPeriod>\n");
        if let Some(from) = period.from {
            el(xml, "AvailableFrom", &format_iso(compose(from, 0)));
        }
        if let Some(to) = period.to {
            el(xml, "AvailableTo", &format_iso(compose(to, 23 * 60 + 59)));
        }
        el(xml, "AvailableUnits", &crate::mspdi::format_number(period.units * 100.0));
        xml.push_str("</AvailabilityPeriod>\n");
    }
    xml.push_str("</AvailabilityPeriods>\n");
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
