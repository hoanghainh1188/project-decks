//! Phase 16: overallocation by overlapping hours, and leveling. Dates are worked out by
//! hand on the default calendar (Mon–Fri, 08–12 and 13–17).

use chrono::Datelike;

use sheafly_engine::command::Command;
use sheafly_engine::document::replay;
use sheafly_engine::model::{ConstraintType, ResourceKind, Task};
use sheafly_engine::store::{load, save};
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::{Document, Project};

const DAY: i64 = 480;

/// 2026-01-05 is a Monday; `day` 0 is that Monday.
fn at(day: u32, hour: u32, minute: u32) -> i64 {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 5)
        .unwrap()
        .checked_add_signed(chrono::Duration::days(i64::from(day)))
        .unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, minute)
}

/// Tasks with these durations (working minutes) from Monday 08:00, and resources
/// `resources`, each with max units 1.
fn doc_with(minutes: &[i64], resources: &[&str]) -> Document {
    let mut project = Project::new("Cân bằng", at(0, 8, 0));
    for (index, minutes) in minutes.iter().enumerate() {
        let mut task = Task::new(index as u32 + 1, format!("Việc {}", index + 1));
        task.duration = *minutes;
        task.work = *minutes;
        project.tasks.push(task);
    }
    project.next_task_uid = minutes.len() as u32 + 1;
    let mut doc = Document::from_project(project);
    for name in resources {
        doc.apply(Command::AddResource {
            name: (*name).into(),
            kind: ResourceKind::Person,
        })
        .unwrap();
    }
    doc
}

fn assign(doc: &mut Document, task_uid: u32, resource_uid: u32, units: f64) {
    doc.apply(Command::AssignResource {
        task_uid,
        resource_uid,
        units,
    })
    .unwrap();
}

fn start_no_earlier(doc: &mut Document, uid: u32, date: i64) {
    doc.apply(Command::SetConstraint {
        uid,
        constraint_type: ConstraintType::Snet,
        date: Some(date),
    })
    .unwrap();
}

fn task(doc: &Document, uid: u32) -> &Task {
    doc.project.task(uid).unwrap()
}

fn json(project: &Project) -> serde_json::Value {
    serde_json::to_value(project).unwrap()
}

#[test]
fn tasks_at_different_hours_of_one_day_do_not_clash() {
    let mut doc = doc_with(&[240, 240], &["An"]);
    start_no_earlier(&mut doc, 2, at(0, 13, 0));
    assign(&mut doc, 1, 1, 1.0);
    assign(&mut doc, 2, 1, 1.0);
    assert_eq!(
        (task(&doc, 2).start, task(&doc, 2).finish),
        (at(0, 13, 0), at(0, 17, 0))
    );
    assert!(
        doc.project.overallocations.is_empty(),
        "{:?}",
        doc.project.overallocations
    );
}

#[test]
fn back_to_back_tasks_within_a_day_do_not_clash() {
    // As in the sample project: one task ends at 08:48, the next starts then.
    let mut doc = doc_with(&[48, DAY], &["An"]);
    doc.apply(Command::AddDependency {
        predecessor_uid: 1,
        successor_uid: 2,
        kind: sheafly_engine::model::DepKind::Fs,
        lag: 0,
    })
    .unwrap();
    assign(&mut doc, 1, 1, 1.0);
    assign(&mut doc, 2, 1, 1.0);
    assert_eq!(task(&doc, 2).start, at(0, 8, 48));
    assert!(
        doc.project.overallocations.is_empty(),
        "{:?}",
        doc.project.overallocations
    );
}

#[test]
fn overlapping_work_is_reported_with_its_peak() {
    let mut doc = doc_with(&[DAY, 240], &["An"]);
    start_no_earlier(&mut doc, 2, at(0, 13, 0));
    assign(&mut doc, 1, 1, 1.0);
    assign(&mut doc, 2, 1, 0.5);
    let rows = &doc.project.overallocations;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(
        (rows[0].resource_uid, rows[0].date.as_str()),
        (1, "2026-01-05")
    );
    assert!((rows[0].units - 1.5).abs() < 1e-9);
    assert!((rows[0].max_units - 1.0).abs() < 1e-9);

    // Half and half fits.
    doc.apply(Command::UpdateAssignment {
        uid: 1,
        units: Some(0.5),
        work: None,
        overtime_work: None,
    })
    .unwrap();
    assert!(
        doc.project.overallocations.is_empty(),
        "{:?}",
        doc.project.overallocations
    );
}

#[test]
fn work_on_a_day_off_of_the_resource_is_overallocated() {
    let mut doc = doc_with(&[DAY], &["An"]);
    doc.apply(Command::CreateCalendar {
        name: "Lịch của An".into(),
        base_id: None,
    })
    .unwrap();
    let calendar_id = doc.project.calendars.last().unwrap().id;
    doc.apply(Command::SetCalendarExceptions {
        calendar_id,
        from: "2026-01-05".into(),
        to: "2026-01-05".into(),
        ranges: Vec::new(),
    })
    .unwrap();
    doc.apply(Command::UpdateResource {
        uid: 1,
        name: "An".into(),
        kind: ResourceKind::Person,
        max_units: 1.0,
        calendar_id: Some(calendar_id),
        standard_rate: 0.0,
        overtime_rate: 0.0,
        cost_per_use: 0.0,
    })
    .unwrap();
    assign(&mut doc, 1, 1, 1.0);
    let rows = &doc.project.overallocations;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].max_units, 0.0);
}

#[test]
fn overallocation_is_found_beyond_ten_years() {
    let mut doc = doc_with(&[DAY, DAY], &["An"]);
    for uid in [1, 2] {
        start_no_earlier(&mut doc, uid, timestamp(2037, 3, 2, 8, 0));
        assign(&mut doc, uid, 1, 1.0);
    }
    let rows = &doc.project.overallocations;
    assert_eq!(
        rows.iter().map(|row| row.date.as_str()).collect::<Vec<_>>(),
        vec!["2037-03-02"]
    );
}

#[test]
fn leveling_moves_the_lower_priority_task_past_the_whole_clash() {
    let mut doc = doc_with(&[2 * DAY, 2 * DAY], &["An"]);
    assign(&mut doc, 1, 1, 1.0);
    assign(&mut doc, 2, 1, 1.0);
    doc.apply(Command::SetPriority {
        uid: 2,
        priority: 100,
    })
    .unwrap();
    doc.apply(Command::LevelResources).unwrap();
    assert_eq!(task(&doc, 1).start, at(0, 8, 0));
    assert_eq!(
        (task(&doc, 2).start, task(&doc, 2).finish),
        (at(2, 8, 0), at(3, 17, 0))
    );
    assert!(doc.project.overallocations.is_empty());
}

#[test]
fn leveling_moves_by_the_hour_not_to_the_next_day() {
    // A takes the morning; B (lower priority, a full day) fits from 13:00.
    let mut doc = doc_with(&[240, DAY], &["An"]);
    assign(&mut doc, 1, 1, 1.0);
    assign(&mut doc, 2, 1, 1.0);
    doc.apply(Command::SetPriority {
        uid: 2,
        priority: 100,
    })
    .unwrap();
    doc.apply(Command::LevelResources).unwrap();
    assert_eq!(
        (task(&doc, 2).start, task(&doc, 2).finish),
        (at(0, 13, 0), at(1, 12, 0))
    );
    assert!(doc.project.overallocations.is_empty());
}

#[test]
fn an_overload_nothing_can_leave_does_not_stop_the_others() {
    // An's two tasks are pinned; Bình's two can move.
    let mut doc = doc_with(&[DAY, DAY, DAY, DAY], &["An", "Bình"]);
    for uid in [1, 2] {
        doc.apply(Command::SetConstraint {
            uid,
            constraint_type: ConstraintType::Mso,
            date: Some(at(0, 8, 0)),
        })
        .unwrap();
        assign(&mut doc, uid, 1, 1.0);
    }
    for uid in [3, 4] {
        start_no_earlier(&mut doc, uid, at(1, 8, 0));
        assign(&mut doc, uid, 2, 1.0);
    }
    doc.apply(Command::LevelResources).unwrap();
    let rows = &doc.project.overallocations;
    assert_eq!(
        rows.iter().map(|row| row.resource_uid).collect::<Vec<_>>(),
        vec![1],
        "{rows:?}"
    );
    let moved = [3, 4]
        .iter()
        .filter(|uid| task(&doc, **uid).leveling_delay > 0)
        .count();
    assert_eq!(moved, 1);
}

#[test]
fn leveling_is_logged_as_the_delays_it_chose() {
    let mut doc = doc_with(&[DAY, DAY], &["An"]);
    assign(&mut doc, 1, 1, 1.0);
    assign(&mut doc, 2, 1, 1.0);
    doc.apply(Command::LevelResources).unwrap();
    let Some(Command::ApplyLeveling { delays }) = doc.commands.last() else {
        panic!("{:?}", doc.commands.last());
    };
    assert_eq!(delays.len(), 1);
    assert_eq!(delays[0].minutes, DAY);
    assert_eq!(
        json(&doc.project),
        json(&replay(&doc.base, &doc.commands).unwrap())
    );
    doc.undo().unwrap();
    assert!(doc
        .project
        .tasks
        .iter()
        .all(|task| task.leveling_delay == 0));
}

#[test]
fn files_that_logged_the_heuristic_still_replay_it() {
    let mut doc = doc_with(&[DAY, DAY], &["An"]);
    assign(&mut doc, 1, 1, 1.0);
    assign(&mut doc, 2, 1, 1.0);
    let leveled = {
        let mut copy = doc.clone();
        copy.apply(Command::LevelResources).unwrap();
        copy.project
    };
    // An older app logged the request itself.
    doc.commands.push(Command::LevelResources);
    doc.cursor = doc.commands.len();
    let path = std::env::temp_dir()
        .join(format!(
            "sheafly-leveling-{}.pdproj",
            std::process::id()
        ))
        .to_string_lossy()
        .to_string();
    save(&doc, &path).unwrap();
    // Its checkpoint would hold the stale project; drop it so the log replays.
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute("DELETE FROM checkpoint", [])
        .unwrap();
    let opened = load(&path).unwrap();
    assert_eq!(json(&opened.project), json(&leveled));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_do_not_level_task_keeps_its_place_and_the_others_fit_around_it() {
    // Task 2 must not be leveled; task 1 starts at the same time and gives way.
    let mut doc = doc_with(&[DAY, DAY], &["An"]);
    assign(&mut doc, 1, 1, 1.0);
    assign(&mut doc, 2, 1, 1.0);
    doc.apply(Command::SetPriority {
        uid: 2,
        priority: 1000,
    })
    .unwrap();
    doc.apply(Command::LevelResources).unwrap();
    assert_eq!(task(&doc, 2).start, at(0, 8, 0));
    assert_eq!(task(&doc, 1).start, at(1, 8, 0));
    assert!(doc.project.overallocations.is_empty());
}

#[test]
fn start_to_start_links_level_without_leftover_clashes() {
    // B starts with A (SS) and has the higher priority, yet both need An.
    let mut doc = doc_with(&[DAY, DAY, DAY], &["An"]);
    doc.apply(Command::AddDependency {
        predecessor_uid: 1,
        successor_uid: 2,
        kind: sheafly_engine::model::DepKind::Ss,
        lag: 0,
    })
    .unwrap();
    for uid in [1, 2, 3] {
        assign(&mut doc, uid, 1, 1.0);
    }
    doc.apply(Command::SetPriority {
        uid: 2,
        priority: 900,
    })
    .unwrap();
    doc.apply(Command::LevelResources).unwrap();
    assert!(
        doc.project.overallocations.is_empty(),
        "{:?}",
        doc.project.overallocations
    );
    let mut starts: Vec<i64> = [1, 2, 3].iter().map(|uid| task(&doc, *uid).start).collect();
    starts.sort_unstable();
    assert_eq!(starts, vec![at(0, 8, 0), at(1, 8, 0), at(2, 8, 0)]);
}

#[test]
fn a_resource_that_never_works_when_the_task_does_is_left_overloaded_quickly() {
    // Bình only works nights; the tasks run on the day calendar.
    let mut doc = doc_with(&[DAY, DAY], &["Bình"]);
    doc.apply(Command::CreateCalendar {
        name: "Ca đêm".into(),
        base_id: None,
    })
    .unwrap();
    let calendar_id = doc.project.calendars.last().unwrap().id;
    let night = vec![sheafly_engine::calendar::TimeRange::new(
        20 * 60,
        23 * 60,
    )];
    doc.apply(Command::SetCalendarWeek {
        calendar_id,
        week: vec![night; 7],
    })
    .unwrap();
    doc.apply(Command::UpdateResource {
        uid: 1,
        name: "Bình".into(),
        kind: ResourceKind::Person,
        max_units: 1.0,
        calendar_id: Some(calendar_id),
        standard_rate: 0.0,
        overtime_rate: 0.0,
        cost_per_use: 0.0,
    })
    .unwrap();
    assign(&mut doc, 1, 1, 1.0);
    assign(&mut doc, 2, 1, 1.0);
    let started = std::time::Instant::now();
    doc.apply(Command::LevelResources).unwrap();
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert!(!doc.project.overallocations.is_empty());
    assert!(doc
        .project
        .tasks
        .iter()
        .all(|task| task.leveling_delay == 0));
}

/// A calendar working Monday to Saturday, 07:00–16:00, for the tasks in `uids`.
fn on_long_days(doc: &mut Document, uids: &[u32]) {
    doc.apply(Command::CreateCalendar {
        name: "Ca dài".into(),
        base_id: None,
    })
    .unwrap();
    let calendar_id = doc.project.calendars.last().unwrap().id;
    let day = vec![sheafly_engine::calendar::TimeRange::new(7 * 60, 16 * 60)];
    let mut week = vec![day; 6];
    week.push(Vec::new());
    doc.apply(Command::SetCalendarWeek { calendar_id, week })
        .unwrap();
    for uid in uids {
        doc.apply(Command::SetTaskCalendar {
            uid: *uid,
            calendar_id: Some(calendar_id),
        })
        .unwrap();
    }
}

#[test]
fn a_resource_that_only_partly_covers_the_task_calendar_is_left_overloaded_quickly() {
    // The tasks work Mon–Sat 07–16; An keeps the standard hours, so some of each task
    // (07–08, lunch, Saturday) always falls outside them, however long it waits.
    // Every task used to slide an hour at a time for four years before giving up.
    let uids: Vec<u32> = (1..=12).collect();
    let mut doc = doc_with(&[2 * DAY; 12], &["An"]);
    on_long_days(&mut doc, &uids);
    for uid in &uids {
        assign(&mut doc, *uid, 1, 1.0);
    }
    let before: Vec<i64> = uids.iter().map(|uid| task(&doc, *uid).start).collect();
    let started = std::time::Instant::now();
    doc.apply(Command::LevelResources).unwrap();
    assert!(
        started.elapsed() < std::time::Duration::from_millis(200),
        "{:?}",
        started.elapsed()
    );
    assert!(!doc.project.overallocations.is_empty());
    let after: Vec<i64> = uids.iter().map(|uid| task(&doc, *uid).start).collect();
    assert_eq!(after, before);
    assert!(doc
        .project
        .tasks
        .iter()
        .all(|task| task.leveling_delay == 0));
}

#[test]
fn a_short_task_on_a_partly_covered_calendar_still_waits_for_the_resource() {
    // Two hours from 11:00 run into An's lunch; from 13:00 they fit.
    let mut doc = doc_with(&[120], &["An"]);
    on_long_days(&mut doc, &[1]);
    start_no_earlier(&mut doc, 1, at(0, 11, 0));
    assign(&mut doc, 1, 1, 1.0);
    assert_eq!(task(&doc, 1).start, at(0, 11, 0));
    doc.apply(Command::LevelResources).unwrap();
    assert_eq!(task(&doc, 1).start, at(0, 13, 0));
    assert!(doc.project.overallocations.is_empty());
}

#[test]
fn a_work_week_months_ahead_still_ends_the_wait() {
    // An works the tasks' long days only through March, so the task waits until then.
    let mut doc = doc_with(&[2 * DAY], &["An"]);
    on_long_days(&mut doc, &[1]);
    let calendar_id = doc.project.resources[0]
        .calendar_id
        .unwrap_or(doc.project.calendar_id);
    let day = vec![sheafly_engine::calendar::TimeRange::new(7 * 60, 16 * 60)];
    let mut week = vec![day; 6];
    week.push(Vec::new());
    doc.apply(Command::SetCalendarWorkWeek {
        calendar_id,
        name: "Tháng ba".into(),
        from: "2026-03-02".into(),
        to: "2026-03-31".into(),
        week,
    })
    .unwrap();
    assign(&mut doc, 1, 1, 1.0);
    doc.apply(Command::LevelResources).unwrap();
    assert_eq!(task(&doc, 1).start, at(56, 7, 0));
    assert!(doc.project.overallocations.is_empty());
}
