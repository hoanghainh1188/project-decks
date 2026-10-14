//! The load map (phase 18 P3, step 3d): work and capacity by person and by day or week,
//! for the heatmap.

use chrono::{Datelike, NaiveDate};

use sheafly_engine::availability::AvailabilityPeriod;
use sheafly_engine::command::Command;
use sheafly_engine::load_map::{load_map, Bucket, LoadCell, MAX_BUCKETS};
use sheafly_engine::model::{ResourceKind, Task};
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::{Document, EngineError, Invalid, Project};

const DAY: i64 = 480;

fn at(day: u32, hour: u32, minute: u32) -> i64 {
    let date = monday().checked_add_signed(chrono::Duration::days(i64::from(day))).unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, minute)
}

fn monday() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()
}

fn day(offset: i64) -> NaiveDate {
    monday() + chrono::Duration::days(offset)
}

/// Two people; task 1 (two days) on An full time, task 2 (one day) on Bình at half time.
fn doc() -> Document {
    let mut project = Project::new("Tải", at(0, 8, 0));
    for (uid, minutes) in [(1, 2 * DAY), (2, DAY)] {
        let mut task = Task::new(uid, format!("Việc {uid}"));
        task.duration = minutes;
        task.remaining_duration = minutes;
        task.work = minutes;
        task.remaining_work = minutes;
        project.tasks.push(task);
    }
    project.next_task_uid = 3;
    let mut doc = Document::from_project(project);
    for name in ["An", "Bình"] {
        doc.apply(Command::AddResource { name: name.into(), kind: ResourceKind::Person }).unwrap();
    }
    doc.apply(Command::AssignResource { task_uid: 1, resource_uid: 1, units: 1.0 }).unwrap();
    doc.apply(Command::AssignResource { task_uid: 2, resource_uid: 2, units: 0.5 }).unwrap();
    doc
}

fn cell(work: i64, capacity: i64) -> LoadCell {
    LoadCell { work, capacity }
}

#[test]
fn gives_work_and_capacity_by_person_and_day() {
    let doc = doc();
    let map = load_map(&doc.project, day(0), day(6), Bucket::Day).unwrap();
    assert_eq!(map.buckets, ["2026-01-05", "2026-01-06", "2026-01-07", "2026-01-08", "2026-01-09", "2026-01-10", "2026-01-11"]);
    let an = &map.rows[0];
    assert_eq!(an.resource_uid, 1);
    assert_eq!(an.cells[..3], [cell(DAY, DAY), cell(DAY, DAY), cell(0, DAY)]);
    // The weekend has no working time.
    assert_eq!(an.cells[5], cell(0, 0));
    assert_eq!(map.rows[1].cells[0], cell(DAY / 2, DAY));
}

#[test]
fn weeks_start_on_monday_and_sum_their_days() {
    let doc = doc();
    let map = load_map(&doc.project, day(2), day(9), Bucket::Week).unwrap();
    assert_eq!(map.buckets, ["2026-01-05", "2026-01-12"]);
    assert_eq!(map.rows[0].cells, [cell(2 * DAY, 5 * DAY), cell(0, 5 * DAY)]);
}

#[test]
fn capacity_follows_availability_and_the_resource_calendar() {
    let mut doc = doc();
    let periods = vec![AvailabilityPeriod { from: Some(day(1)), to: Some(day(1)), units: 0.0 }];
    doc.apply(Command::SetResourceAvailability { uid: 1, periods }).unwrap();
    doc.apply(Command::AddHoliday { date: "2026-01-07".into() }).unwrap();
    let map = load_map(&doc.project, day(0), day(2), Bucket::Day).unwrap();
    // Leave on Tuesday; the project holiday on Wednesday reaches everyone's calendar.
    let capacity: Vec<i64> = map.rows[0].cells.iter().map(|cell| cell.capacity).collect();
    assert_eq!(capacity, [DAY, 0, 0]);
    assert_eq!(map.rows[1].cells[2].capacity, 0);
}

#[test]
fn refuses_a_backwards_or_too_long_span() {
    let doc = doc();
    let invalid = |from, to, bucket| match load_map(&doc.project, from, to, bucket) {
        Err(EngineError::Invalid(reason)) => reason,
        other => panic!("expected an error, got {other:?}"),
    };
    assert_eq!(invalid(day(3), day(1), Bucket::Day), Invalid::BadDate);
    assert_eq!(invalid(day(0), day(MAX_BUCKETS as i64), Bucket::Day), Invalid::DateRangeTooLong);
    assert!(load_map(&doc.project, day(0), day(7 * (MAX_BUCKETS as i64 - 1)), Bucket::Week).is_ok());
}

#[test]
fn five_hundred_people_over_a_year_of_weeks_is_quick() {
    let mut project = Project::new("Lớn", at(0, 8, 0));
    for uid in 1..=500 {
        project.resources.push(sheafly_engine::model::Resource::new(uid, format!("Người {uid}"), ResourceKind::Person));
    }
    let started = std::time::Instant::now();
    let map = load_map(&project, day(0), day(364), Bucket::Week).unwrap();
    assert_eq!(map.rows.len(), 500);
    assert_eq!(map.buckets.len(), 53);
    // Debug builds are slow; the bench in release is far below this.
    assert!(started.elapsed().as_millis() < 2000);
}
