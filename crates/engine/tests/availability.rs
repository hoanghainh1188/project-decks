//! Resource capacity by date (phase 18 P2, step 3): availability periods override a
//! resource's max units on their days, for overallocation and leveling only. Dates are
//! worked out by hand on the default calendar (Mon–Fri, 08–12 and 13–17).

use chrono::{Datelike, NaiveDate};

use sheafly_engine::availability::{units_on, AvailabilityPeriod, MAX_PERIODS};
use sheafly_engine::command::Command;
use sheafly_engine::lang::Lang;
use sheafly_engine::model::{ResourceKind, Task};
use sheafly_engine::mspdi::{read_mspdi_with_report, write_mspdi};
use sheafly_engine::store::{load, save};
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::{Document, EngineError, Invalid, Project};

const DAY: i64 = 480;
const FIXTURE: &str = include_str!("fixtures/availability.xml");

/// 2026-01-05 is a Monday; `day` 0 is that Monday.
fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 1, 5).unwrap() + chrono::Duration::days(i64::from(day))
}

fn at(day: u32, hour: u32, minute: u32) -> i64 {
    let date = date(day);
    timestamp(date.year(), date.month(), date.day(), hour, minute)
}

/// Tasks of these durations from Monday 08:00, and one person with max units 1.
fn doc_with(minutes: &[i64]) -> Document {
    let mut project = Project::new("Năng lực", at(0, 8, 0));
    for (index, minutes) in minutes.iter().enumerate() {
        let mut task = Task::new(index as u32 + 1, format!("Việc {}", index + 1));
        task.duration = *minutes;
        task.work = *minutes;
        task.remaining_duration = *minutes;
        task.remaining_work = *minutes;
        project.tasks.push(task);
    }
    project.next_task_uid = minutes.len() as u32 + 1;
    let mut doc = Document::from_project(project);
    let name = "An".to_string();
    doc.apply(Command::AddResource { name, kind: ResourceKind::Person }).unwrap();
    doc
}

fn assign(doc: &mut Document, task_uid: u32, units: f64) {
    doc.apply(Command::AssignResource { task_uid, resource_uid: 1, units }).unwrap();
}

fn period(from: Option<u32>, to: Option<u32>, units: f64) -> AvailabilityPeriod {
    AvailabilityPeriod { from: from.map(date), to: to.map(date), units }
}

fn set(doc: &mut Document, periods: Vec<AvailabilityPeriod>) -> Result<(), EngineError> {
    doc.apply(Command::SetResourceAvailability { uid: 1, periods }).map(|_| ())
}

fn level(doc: &mut Document) {
    doc.apply(Command::LevelResources).unwrap();
}

fn start(doc: &Document, uid: u32) -> i64 {
    doc.project.task(uid).unwrap().start
}

fn invalid(result: Result<(), EngineError>) -> Invalid {
    match result {
        Err(EngineError::Invalid(reason)) => reason,
        other => panic!("expected a validation error, got {other:?}"),
    }
}

#[test]
fn a_half_time_stretch_overallocates_only_on_its_days() {
    let mut doc = doc_with(&[5 * DAY]);
    assign(&mut doc, 1, 1.0);
    assert!(doc.project.overallocations.is_empty());
    set(&mut doc, vec![period(Some(2), Some(3), 0.5)]).unwrap();
    let days: Vec<(String, f64, f64)> = doc
        .project
        .overallocations
        .iter()
        .map(|over| (over.date.clone(), over.units, over.max_units))
        .collect();
    assert_eq!(
        days,
        vec![("2026-01-07".to_string(), 1.0, 0.5), ("2026-01-08".to_string(), 1.0, 0.5)]
    );
}

#[test]
fn availability_does_not_change_the_schedule() {
    let mut doc = doc_with(&[2 * DAY]);
    assign(&mut doc, 1, 1.0);
    let before: Vec<(i64, i64, i64)> = doc.project.tasks.iter().map(|t| (t.start, t.finish, t.work)).collect();
    set(&mut doc, vec![period(Some(0), Some(9), 0.0)]).unwrap();
    let after: Vec<(i64, i64, i64)> = doc.project.tasks.iter().map(|t| (t.start, t.finish, t.work)).collect();
    assert_eq!(before, after);
}

#[test]
fn leveling_moves_a_task_past_a_leave() {
    let mut doc = doc_with(&[DAY]);
    assign(&mut doc, 1, 1.0);
    set(&mut doc, vec![period(Some(0), Some(4), 0.0)]).unwrap();
    level(&mut doc);
    assert_eq!(start(&doc, 1), at(7, 8, 0));
    assert!(doc.project.overallocations.is_empty());
}

#[test]
fn leveling_jumps_a_long_leave() {
    let mut doc = doc_with(&[DAY]);
    assign(&mut doc, 1, 1.0);
    // Off until Friday 2026-04-03; back the Monday after.
    set(&mut doc, vec![period(Some(0), Some(88), 0.0)]).unwrap();
    level(&mut doc);
    assert_eq!(start(&doc, 1), at(91, 8, 0));
}

#[test]
fn a_task_placed_after_a_leave_can_go_before_work_placed_later() {
    // Task 1 must start on Thursday and is placed first; task 2 starts on Monday, but the
    // person is off Monday and Tuesday. Wednesday is free.
    let mut doc = doc_with(&[DAY, DAY]);
    assign(&mut doc, 1, 1.0);
    assign(&mut doc, 2, 1.0);
    let mso = Command::SetConstraint {
        uid: 1,
        constraint_type: sheafly_engine::model::ConstraintType::Mso,
        date: Some(at(3, 8, 0)),
    };
    doc.apply(mso).unwrap();
    set(&mut doc, vec![period(Some(0), Some(1), 0.0)]).unwrap();
    level(&mut doc);
    assert_eq!(start(&doc, 1), at(3, 8, 0));
    assert_eq!(start(&doc, 2), at(2, 8, 0));
}

#[test]
fn a_half_unit_task_fits_a_half_time_week_and_a_full_one_waits() {
    let mut doc = doc_with(&[DAY, DAY]);
    assign(&mut doc, 1, 0.5);
    assign(&mut doc, 2, 1.0);
    set(&mut doc, vec![period(Some(0), Some(4), 0.5)]).unwrap();
    level(&mut doc);
    assert_eq!(start(&doc, 1), at(0, 8, 0));
    assert_eq!(start(&doc, 2), at(7, 8, 0));
    assert!(doc.project.overallocations.is_empty());
}

#[test]
fn a_task_waits_for_a_stretch_with_more_units_but_not_for_one_that_never_comes() {
    let mut doc = doc_with(&[DAY, DAY]);
    assign(&mut doc, 1, 2.0);
    assign(&mut doc, 2, 3.0);
    set(&mut doc, vec![period(Some(7), Some(11), 2.0)]).unwrap();
    level(&mut doc);
    assert_eq!(start(&doc, 1), at(7, 8, 0), "two units come in the second week");
    assert_eq!(start(&doc, 2), at(0, 8, 0), "three units never come: left where it was");
}

#[test]
fn periods_can_be_open_at_either_end() {
    let mut doc = doc_with(&[DAY]);
    assign(&mut doc, 1, 1.0);
    set(&mut doc, vec![period(None, Some(2), 0.0)]).unwrap();
    level(&mut doc);
    assert_eq!(start(&doc, 1), at(3, 8, 0));

    // From Thursday on, half time for good: a full-time task can never fit after the
    // leave, so it stays where it was.
    set(&mut doc, vec![period(None, Some(2), 0.0), period(Some(3), None, 0.5)]).unwrap();
    level(&mut doc);
    assert_eq!(start(&doc, 1), at(0, 8, 0));
    let resource = &doc.project.resources[0];
    assert_eq!(units_on(resource, date(1)), 0.0);
    assert_eq!(units_on(resource, date(400)), 0.5);
}

#[test]
fn a_holiday_and_a_leave_add_up() {
    let mut doc = doc_with(&[DAY]);
    assign(&mut doc, 1, 1.0);
    doc.apply(Command::AddHoliday { date: "2026-01-08".into() }).unwrap();
    set(&mut doc, vec![period(Some(0), Some(2), 0.0)]).unwrap();
    level(&mut doc);
    assert_eq!(start(&doc, 1), at(4, 8, 0));
}

#[test]
fn periods_that_cannot_be_kept_are_refused_and_the_rest_are_sorted() {
    let mut doc = doc_with(&[DAY]);
    let overlapping = vec![period(Some(0), Some(4), 0.5), period(Some(4), Some(8), 0.0)];
    assert_eq!(invalid(set(&mut doc, overlapping)), Invalid::AvailabilityOverlaps);
    let open = vec![period(Some(3), None, 0.5), period(Some(10), Some(12), 0.0)];
    assert_eq!(invalid(set(&mut doc, open)), Invalid::AvailabilityOverlaps);
    assert_eq!(invalid(set(&mut doc, vec![period(Some(5), Some(4), 1.0)])), Invalid::AvailabilityRange);
    assert_eq!(invalid(set(&mut doc, vec![period(Some(0), Some(1), -0.5)])), Invalid::AvailabilityUnits);
    assert_eq!(invalid(set(&mut doc, vec![period(Some(0), Some(1), f64::NAN)])), Invalid::AvailabilityUnits);
    let many: Vec<AvailabilityPeriod> =
        (0..=MAX_PERIODS as u32).map(|day| period(Some(day), Some(day), 0.5)).collect();
    assert_eq!(invalid(set(&mut doc, many)), Invalid::TooManyAvailabilityPeriods);
    let edge = vec![AvailabilityPeriod { from: Some(date(0)), to: Some(NaiveDate::MAX), units: 0.5 }];
    assert_eq!(invalid(set(&mut doc, edge)), Invalid::AvailabilityRange, "use an open end");
    let missing = doc.apply(Command::SetResourceAvailability { uid: 9, periods: Vec::new() });
    assert!(matches!(missing, Err(EngineError::ResourceNotFound)), "{missing:?}");

    let unsorted = vec![period(Some(10), Some(12), 0.0), period(None, Some(2), 0.5), period(Some(3), Some(3), 2.0)];
    set(&mut doc, unsorted).unwrap();
    let stored = &doc.project.resources[0].availability;
    assert_eq!(
        stored,
        &vec![period(None, Some(2), 0.5), period(Some(3), Some(3), 2.0), period(Some(10), Some(12), 0.0)]
    );
    // Editing the resource keeps its periods.
    let update = Command::UpdateResource {
        uid: 1,
        name: "An".into(),
        kind: ResourceKind::Person,
        max_units: 0.8,
        calendar_id: None,
        standard_rate: 0.0,
        overtime_rate: 0.0,
        cost_per_use: 0.0,
    };
    doc.apply(update).unwrap();
    assert_eq!(doc.project.resources[0].availability.len(), 3);
    assert_eq!(units_on(&doc.project.resources[0], date(5)), 0.8);
}

#[test]
fn periods_undo_and_survive_a_file_and_old_projects_have_none() {
    let mut doc = doc_with(&[DAY]);
    let periods = vec![period(None, Some(2), 0.0), period(Some(7), None, 0.5)];
    set(&mut doc, periods.clone()).unwrap();
    doc.undo().unwrap();
    assert!(doc.project.resources[0].availability.is_empty());
    doc.redo().unwrap();
    assert_eq!(doc.project.resources[0].availability, periods);

    let dir = std::env::temp_dir().join(format!("sheafly-availability-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("availability.sheafly").to_string_lossy().to_string();
    save(&doc, &path).unwrap();
    let opened = load(&path).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(opened.project.resources, doc.project.resources);

    let mut json = serde_json::to_value(&doc.project).unwrap();
    json["resources"][0].as_object_mut().unwrap().remove("availability");
    let old: Project = serde_json::from_value(json).unwrap();
    assert!(old.resources[0].availability.is_empty());
}

#[test]
fn the_command_reads_the_shape_the_app_sends() {
    let sent = r#"{"type":"set_resource_availability","uid":1,"periods":[
        {"from":null,"to":"2026-01-07","units":0},
        {"from":"2026-01-12","to":null,"units":0.5}]}"#;
    let command: Command = serde_json::from_str(sent).unwrap();
    let Command::SetResourceAvailability { uid, periods } = command else {
        panic!("read as {command:?}");
    };
    assert_eq!(uid, 1);
    assert_eq!(periods, vec![period(None, Some(2), 0.0), period(Some(7), None, 0.5)]);
}

#[test]
fn mspdi_periods_are_read_as_project_means_them() {
    let (project, report) = read_mspdi_with_report(FIXTURE).unwrap();
    let ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    let span = |from: Option<NaiveDate>, to: Option<NaiveDate>, units| AvailabilityPeriod { from, to, units };

    // Outside its table Project has the resource away: the gaps come in as zero.
    let gaps = &project.resources[0];
    assert_eq!(gaps.max_units, 1.0);
    assert_eq!(
        gaps.availability,
        vec![
            span(None, Some(ymd(2026, 1, 11)), 0.0),
            span(Some(ymd(2026, 1, 12)), Some(ymd(2026, 1, 16)), 0.5),
            span(Some(ymd(2026, 1, 17)), Some(ymd(2026, 1, 25)), 0.0),
            span(Some(ymd(2026, 1, 26)), Some(ymd(2026, 1, 30)), 1.0),
            span(Some(ymd(2026, 1, 31)), None, 0.0),
        ]
    );

    // One period with no dates is the resource's units for all time.
    let always = &project.resources[1];
    assert_eq!((always.max_units, always.availability.len()), (0.75, 0));

    // 1984 and 2049 stand for "no limit"; the period that overlaps is left out and noted.
    let overlapping = &project.resources[2];
    assert_eq!(
        overlapping.availability,
        vec![
            span(None, Some(ymd(2026, 1, 9)), 2.0),
            span(Some(ymd(2026, 1, 10)), None, 1.0),
        ]
    );
    let message = report.message(Lang::En).unwrap();
    assert!(
        message.contains("Resource “Overlapping”: an availability period overlaps an earlier one or ends before it starts"),
        "{message}"
    );
    let message = report.message(Lang::Vi).unwrap();
    assert!(message.contains("Nguồn lực “Overlapping”: một khoảng khả dụng"), "{message}");
}

/// A resource with the given `<AvailabilityPeriods>` inside an otherwise empty file.
fn file_with_periods(periods: &str) -> String {
    let start = FIXTURE.find("<Resources>").unwrap();
    let end = FIXTURE.find("</Resources>").unwrap();
    let resource = format!(
        "<Resources><Resource><UID>1</UID><Name>R</Name><Type>1</Type><MaxUnits>1</MaxUnits>\
         <AvailabilityPeriods>{periods}</AvailabilityPeriods></Resource>"
    );
    format!("{}{}{}", &FIXTURE[..start], resource, &FIXTURE[end..])
}

#[test]
fn mspdi_tables_with_nothing_usable_leave_the_max_units() {
    let backwards = "<AvailabilityPeriod><AvailableFrom>2026-01-20T08:00:00</AvailableFrom>\
        <AvailableTo>2026-01-10T17:00:00</AvailableTo><AvailableUnits>50</AvailableUnits></AvailabilityPeriod>";
    let (project, report) = read_mspdi_with_report(&file_with_periods(backwards)).unwrap();
    assert!(project.resources[0].availability.is_empty());
    assert_eq!(project.resources[0].max_units, 1.0);
    assert!(report.message(Lang::En).unwrap().contains("Resource “R”"));
}

#[test]
fn mspdi_far_dates_are_open_only_on_their_own_side() {
    // A start in 2050 and an end in 1980 are dates, not "no limit"; a start in 1970 and an
    // end in 2050 are.
    let periods = "<AvailabilityPeriod><AvailableFrom>1970-01-01T00:00:00</AvailableFrom>\
        <AvailableTo>1980-06-30T17:00:00</AvailableTo><AvailableUnits>50</AvailableUnits></AvailabilityPeriod>\
        <AvailabilityPeriod><AvailableFrom>2050-01-03T08:00:00</AvailableFrom>\
        <AvailableTo>2050-12-31T17:00:00</AvailableTo><AvailableUnits>30</AvailableUnits></AvailabilityPeriod>";
    let (project, _) = read_mspdi_with_report(&file_with_periods(periods)).unwrap();
    let ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    let resource = &project.resources[0];
    assert_eq!(units_on(resource, ymd(1975, 1, 1)), 0.5);
    assert_eq!(units_on(resource, ymd(1981, 1, 1)), 0.0);
    assert_eq!(units_on(resource, ymd(2050, 6, 1)), 0.3);
    assert_eq!(units_on(resource, ymd(2051, 6, 1)), 0.3);
    assert_eq!(units_on(resource, ymd(1900, 1, 1)), 0.5);
    assert_eq!(units_on(resource, ymd(2050, 1, 2)), 0.0, "the gap before the start in 2050");
}

#[test]
fn mspdi_round_trip_keeps_the_capacity_of_every_day() {
    let mut doc = doc_with(&[DAY]);
    let periods = vec![period(None, Some(2), 0.0), period(Some(7), Some(11), 0.3), period(Some(20), None, 0.07)];
    set(&mut doc, periods).unwrap();
    let mut no_periods = doc.project.clone();
    no_periods.resources[0].availability.clear();
    for project in [&doc.project, &no_periods] {
        let xml = write_mspdi(project).unwrap();
        let (back, _) = read_mspdi_with_report(&xml).unwrap();
        let (before, after) = (&project.resources[0], &back.resources[0]);
        for day in 0..60 {
            assert_eq!(units_on(before, date(day)), units_on(after, date(day)), "day {day}");
        }
        assert_eq!(units_on(before, date(5000)), units_on(after, date(5000)));
    }
    let xml = write_mspdi(&no_periods).unwrap();
    assert!(!xml.contains("AvailabilityPeriods"), "nothing to write without periods");
}
