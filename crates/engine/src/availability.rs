//! What a resource can give on each day: its max units, except on the days of an
//! availability period ("week 3 at half time", "on leave 1–5 Nov"), which sets the
//! units for those days.
//!
//! Like Project, availability does not change durations or work: only overallocation
//! and leveling read it. Unlike Project, days outside every period keep the max units
//! (Project has the resource away outside its availability table); MSPDI import and
//! export translate between the two (`mspdi_availability.rs`).

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::error::{EngineError, Invalid};
use crate::model::{Project, Resource};

/// Most periods one resource can have.
pub const MAX_PERIODS: usize = 500;

/// Units below this are rounding noise.
const EPSILON: f64 = 1e-6;

/// Both ends count as part of the period; `None` is no limit on that side.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AvailabilityPeriod {
    #[serde(default)]
    pub from: Option<NaiveDate>,
    #[serde(default)]
    pub to: Option<NaiveDate>,
    pub units: f64,
}

/// Replace a resource's periods. They must not overlap, each must end on or after its
/// start, units must be finite and not negative, and there are at most `MAX_PERIODS`.
/// They are kept in date order, whatever order they came in.
pub(crate) fn set_availability(
    project: &mut Project,
    uid: u32,
    periods: &[AvailabilityPeriod],
) -> Result<(), EngineError> {
    let position = project
        .resources
        .iter()
        .position(|resource| resource.uid == uid)
        .ok_or(EngineError::ResourceNotFound)?;
    project.resources[position].availability = checked(periods)?;
    Ok(())
}

fn checked(periods: &[AvailabilityPeriod]) -> Result<Vec<AvailabilityPeriod>, EngineError> {
    if periods.len() > MAX_PERIODS {
        return Err(Invalid::TooManyAvailabilityPeriods.into());
    }
    for period in periods {
        if !period.units.is_finite() || period.units < 0.0 {
            return Err(Invalid::AvailabilityUnits.into());
        }
        if let (Some(from), Some(to)) = (period.from, period.to) {
            if from > to {
                return Err(Invalid::AvailabilityRange.into());
            }
        }
        // The first and last dates there are stand for no limit: use an open end.
        let at_the_edge = period.from.is_some_and(|from| from.pred_opt().is_none())
            || period.to.is_some_and(|to| to.succ_opt().is_none());
        if at_the_edge {
            return Err(Invalid::AvailabilityRange.into());
        }
    }
    let sorted = sorted(periods.to_vec());
    if sorted.windows(2).any(|pair| overlap(&pair[0], &pair[1])) {
        return Err(Invalid::AvailabilityOverlaps.into());
    }
    Ok(sorted)
}

/// In date order, an open start first; -0 units are written 0.
pub(crate) fn sorted(mut periods: Vec<AvailabilityPeriod>) -> Vec<AvailabilityPeriod> {
    for period in &mut periods {
        period.units += 0.0;
    }
    periods.sort_by_key(|period| (period.from.is_some(), period.from, period.to.is_none(), period.to));
    periods
}

/// Whether `later`, which does not start before `earlier`, starts before it ends.
pub(crate) fn overlap(earlier: &AvailabilityPeriod, later: &AvailabilityPeriod) -> bool {
    match (earlier.to, later.from) {
        (None, _) | (_, None) => true,
        (Some(to), Some(from)) => from <= to,
    }
}

/// The units a resource has on `date`.
pub fn units_on(resource: &Resource, date: NaiveDate) -> f64 {
    Capacity::of(resource).units_on(date)
}

/// A resource's units by date, for the leveling sweep.
#[derive(Clone, Copy)]
pub(crate) struct Capacity<'a> {
    max_units: f64,
    /// Sorted and apart, as `set_availability` keeps them.
    periods: &'a [AvailabilityPeriod],
}

impl<'a> Capacity<'a> {
    pub(crate) fn of(resource: &'a Resource) -> Self {
        Self {
            max_units: resource.max_units,
            periods: &resource.availability,
        }
    }

    /// The period that covers `date`.
    fn period_on(&self, date: NaiveDate) -> Option<&'a AvailabilityPeriod> {
        let after = self
            .periods
            .partition_point(|period| period.from.is_none_or(|from| from <= date));
        let period = self.periods.get(after.checked_sub(1)?)?;
        period.to.is_none_or(|to| date <= to).then_some(period)
    }

    pub(crate) fn has_periods(&self) -> bool {
        !self.periods.is_empty()
    }

    pub(crate) fn units_on(&self, date: NaiveDate) -> f64 {
        self.period_on(date).map_or(self.max_units, |period| period.units)
    }

    /// The most the resource has on any day from `date` on.
    pub(crate) fn most_from(&self, date: NaiveDate) -> f64 {
        let mut most: f64 = 0.0;
        // The first day not yet known to be inside a period.
        let mut cursor = Some(date);
        for period in &self.periods[self.first_ending_from(date)..] {
            let Some(day) = cursor else { break };
            if period.from.is_some_and(|from| from > day) {
                most = most.max(self.max_units);
            }
            most = most.max(period.units);
            cursor = period.to.and_then(|to| to.succ_opt());
        }
        if cursor.is_some() {
            most = most.max(self.max_units);
        }
        most
    }

    /// The first period that ends on or after `date`.
    fn first_ending_from(&self, date: NaiveDate) -> usize {
        self.periods
            .partition_point(|period| period.to.is_some_and(|to| to < date))
    }

    /// The first day from `date` on with at least `units` (and some units at all), or
    /// `None` when no such day comes.
    pub(crate) fn next_with(&self, date: NaiveDate, units: f64) -> Option<NaiveDate> {
        let enough = |have: f64| have > EPSILON && units <= have + EPSILON;
        let mut day = date;
        // Each round leaves a period or a stretch between periods behind.
        for _ in 0..=2 * self.periods.len() + 1 {
            match self.period_on(day) {
                Some(period) if enough(period.units) => return Some(day),
                Some(period) => day = period.to?.succ_opt()?,
                None if enough(self.max_units) => return Some(day),
                None => {
                    let next = self.periods[self.first_ending_from(day)..]
                        .iter()
                        .find_map(|period| period.from.filter(|from| *from > day))?;
                    day = next;
                }
            }
        }
        None
    }

    /// The first day from which the units never change again: past the end of the last
    /// period that ends and the start of one that never does. `None` without periods.
    pub(crate) fn steady_from(&self) -> Option<NaiveDate> {
        self.periods
            .iter()
            .filter_map(|period| match (period.from, period.to) {
                (_, Some(to)) => to.succ_opt(),
                (from, None) => from,
            })
            .max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ResourceKind;

    fn day(n: i64) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 1, 5).unwrap() + chrono::Duration::days(n)
    }

    fn resource(periods: Vec<(Option<i64>, Option<i64>, f64)>) -> Resource {
        let mut resource = Resource::new(1, "An", ResourceKind::Person);
        resource.availability = periods
            .into_iter()
            .map(|(from, to, units)| AvailabilityPeriod { from: from.map(day), to: to.map(day), units })
            .collect();
        resource
    }

    #[test]
    fn the_most_counts_the_days_between_periods() {
        let apart = resource(vec![(None, Some(2), 0.0), (Some(5), Some(6), 0.5), (Some(7), None, 0.25)]);
        let capacity = Capacity::of(&apart);
        // Days 3 and 4 have the max units.
        assert_eq!(capacity.most_from(day(0)), 1.0);
        assert_eq!(capacity.most_from(day(5)), 0.5);
        assert_eq!(capacity.most_from(day(9)), 0.25);
        assert_eq!(capacity.steady_from(), Some(day(7)));
    }

    #[test]
    fn the_next_day_with_room_skips_whole_periods() {
        let apart = resource(vec![(Some(0), Some(90), 0.0), (Some(91), Some(95), 0.5), (Some(100), None, 0.0)]);
        let capacity = Capacity::of(&apart);
        assert_eq!(capacity.next_with(day(0), 0.5), Some(day(91)));
        assert_eq!(capacity.next_with(day(0), 1.0), Some(day(96)));
        assert_eq!(capacity.next_with(day(97), 0.5), Some(day(97)));
        assert_eq!(capacity.next_with(day(98), 2.0), None);
        assert_eq!(capacity.next_with(day(100), 0.1), None);
    }

    #[test]
    fn a_day_between_two_leaves_has_the_max_units() {
        let leaves = resource(vec![(Some(0), Some(2), 0.0), (Some(4), Some(6), 0.0)]);
        let capacity = Capacity::of(&leaves);
        assert_eq!(capacity.next_with(day(0), 1.0), Some(day(3)));
        assert_eq!(capacity.next_with(day(4), 1.0), Some(day(7)));
        assert_eq!(capacity.most_from(day(4)), 1.0);
        assert_eq!(capacity.units_on(day(3)), 1.0);
    }
}
