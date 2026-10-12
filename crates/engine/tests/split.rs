use chrono::Datelike;

use projectdesk_engine::command::Command;
use projectdesk_engine::evm::{burndown, evm_at, s_curve, BurndownPoint};
use projectdesk_engine::model::{Accrual, DepKind, Dependency, ResourceKind, Task};
use projectdesk_engine::mspdi::{read_mspdi, write_mspdi};
use projectdesk_engine::recurrence::{RecurrenceEnd, RecurrencePattern, RecurrenceSpec};
use projectdesk_engine::schedule::schedule;
use projectdesk_engine::split::{Segment, Split};
use projectdesk_engine::store::{load, save};
use projectdesk_engine::timeutil::timestamp;
use projectdesk_engine::{Document, Project, Resource};

/// 2026-01-05 is a Monday; `day` 0 is that Monday.
fn at(day: u32, hour: u32, minute: u32) -> i64 {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 5)
        .unwrap()
        .checked_add_signed(chrono::Duration::days(i64::from(day)))
        .unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, minute)
}

fn project_with(tasks: Vec<(&str, i64)>, deps: Vec<(u32, u32)>) -> Project {
    let mut project = Project::new("Tách", at(0, 8, 0));
    for (index, (name, duration)) in tasks.into_iter().enumerate() {
        let mut task = Task::new(index as u32 + 1, name);
        task.duration = duration;
        task.work = duration;
        task.remaining_duration = duration;
        task.remaining_work = duration;
        task.milestone = duration == 0;
        project.tasks.push(task);
    }
    project.next_task_uid = project.tasks.len() as u32 + 1;
    for (index, (pred, succ)) in deps.into_iter().enumerate() {
        project.dependencies.push(Dependency {
            uid: index as u32 + 1,
            predecessor_uid: pred,
            successor_uid: succ,
            kind: DepKind::Fs,
            lag: 0,
        });
    }
    project.next_dependency_uid = project.dependencies.len() as u32 + 1;
    schedule(&mut project).unwrap();
    project
}

fn doc_with(tasks: Vec<(&str, i64)>, deps: Vec<(u32, u32)>) -> Document {
    Document::from_project(project_with(tasks, deps))
}

fn task(doc: &Document, uid: u32) -> &Task {
    doc.project.task(uid).unwrap()
}

fn seg(start: i64, finish: i64) -> Segment {
    Segment { start, finish }
}

/// Two-day task split at Monday 13:00 for one working day.
fn split_two_day_task() -> Document {
    let mut doc = doc_with(vec![("A", 960), ("B", 480)], vec![(1, 2)]);
    doc.apply(Command::SplitTask {
        uid: 1,
        at: at(0, 13, 0),
        gap: 480,
    })
    .unwrap();
    doc
}

#[test]
fn split_keeps_duration_and_moves_finish_by_the_gap() {
    let doc = split_two_day_task();
    let a = task(&doc, 1);
    assert_eq!(a.duration, 960);
    assert_eq!(
        a.splits,
        vec![Split {
            offset: 240,
            gap: 480
        }]
    );
    assert_eq!(
        a.segments,
        vec![
            seg(at(0, 8, 0), at(0, 12, 0)),
            seg(at(1, 13, 0), at(2, 17, 0))
        ]
    );
    assert_eq!(a.start, at(0, 8, 0));
    // Unsplit it would finish Tuesday 17:00; the one-day gap pushes it to Wednesday.
    assert_eq!(a.finish, at(2, 17, 0));
    assert_eq!(a.work, 960);
    // The successor keys off the last segment finish.
    let b = task(&doc, 2);
    assert_eq!(b.start, at(3, 8, 0));
    assert!(a.critical && b.critical);
}

#[test]
fn split_changes_slack_and_critical_path() {
    // A (2d) and C (4d) both feed D.
    let mut doc = doc_with(
        vec![("A", 960), ("C", 1920), ("D", 480)],
        vec![(1, 3), (2, 3)],
    );
    assert_eq!(task(&doc, 1).total_slack, 960);
    assert!(!task(&doc, 1).critical);

    doc.apply(Command::SplitTask {
        uid: 1,
        at: at(0, 13, 0),
        gap: 480,
    })
    .unwrap();
    assert_eq!(task(&doc, 1).total_slack, 480);
    assert_eq!(task(&doc, 1).free_slack, 480);
    assert!(!task(&doc, 1).critical);
    assert_eq!(task(&doc, 3).start, at(4, 8, 0));

    // Wednesday 08:00 is 480 minutes into the work; a two-day gap there makes A longest.
    doc.apply(Command::SplitTask {
        uid: 1,
        at: at(2, 8, 0),
        gap: 960,
    })
    .unwrap();
    let a = task(&doc, 1);
    assert_eq!(
        a.splits,
        vec![
            Split {
                offset: 240,
                gap: 480
            },
            Split {
                offset: 480,
                gap: 960
            }
        ]
    );
    assert_eq!(a.finish, at(4, 17, 0));
    assert_eq!(a.total_slack, 0);
    assert!(a.critical);
    assert_eq!(task(&doc, 2).total_slack, 480);
    assert!(!task(&doc, 2).critical);
    assert_eq!(task(&doc, 3).start, at(7, 8, 0));
}

#[test]
fn remove_split_joins_segments() {
    let mut doc = split_two_day_task();
    doc.apply(Command::SplitTask {
        uid: 1,
        at: at(2, 8, 0),
        gap: 480,
    })
    .unwrap();
    assert_eq!(task(&doc, 1).segments.len(), 3);

    doc.apply(Command::RemoveSplit {
        uid: 1,
        index: Some(0),
    })
    .unwrap();
    let a = task(&doc, 1);
    assert_eq!(
        a.splits,
        vec![Split {
            offset: 480,
            gap: 480
        }]
    );
    assert_eq!(a.finish, at(2, 17, 0));

    doc.apply(Command::RemoveSplit {
        uid: 1,
        index: None,
    })
    .unwrap();
    let a = task(&doc, 1);
    assert!(a.splits.is_empty() && a.segments.is_empty());
    assert_eq!(a.finish, at(1, 17, 0));
    assert_eq!(task(&doc, 2).start, at(2, 8, 0));
    assert!(doc
        .apply(Command::RemoveSplit {
            uid: 1,
            index: None
        })
        .is_err());
}

#[test]
fn split_inside_a_gap_lengthens_it() {
    let mut doc = split_two_day_task();
    doc.apply(Command::SplitTask {
        uid: 1,
        at: at(1, 9, 0),
        gap: 480,
    })
    .unwrap();
    let a = task(&doc, 1);
    assert_eq!(
        a.splits,
        vec![Split {
            offset: 240,
            gap: 960
        }]
    );
    assert_eq!(a.finish, at(3, 17, 0));
}

#[test]
fn invalid_splits_are_rejected() {
    let mut doc = doc_with(vec![("A", 960), ("M", 0)], vec![]);
    // Before the start, at the finish, and with no gap.
    assert!(doc
        .apply(Command::SplitTask {
            uid: 1,
            at: at(0, 7, 0),
            gap: 480
        })
        .is_err());
    assert!(doc
        .apply(Command::SplitTask {
            uid: 1,
            at: at(0, 8, 0),
            gap: 480
        })
        .is_err());
    assert!(doc
        .apply(Command::SplitTask {
            uid: 1,
            at: at(1, 17, 0),
            gap: 480
        })
        .is_err());
    assert!(doc
        .apply(Command::SplitTask {
            uid: 1,
            at: at(0, 13, 0),
            gap: 0
        })
        .is_err());
    // Milestones have no work to split.
    assert!(doc
        .apply(Command::SplitTask {
            uid: 2,
            at: at(0, 13, 0),
            gap: 480
        })
        .is_err());
    assert!(!doc.snapshot().can_undo);
}

#[test]
fn summary_tasks_cannot_be_split() {
    let mut doc = doc_with(vec![("Nhóm", 480), ("Con", 960)], vec![]);
    doc.apply(Command::Indent { uid: 2 }).unwrap();
    assert!(task(&doc, 1).summary);
    assert!(doc
        .apply(Command::SplitTask {
            uid: 1,
            at: at(0, 13, 0),
            gap: 480
        })
        .is_err());

    // A split leaf that becomes a summary drops its splits.
    let mut doc = doc_with(vec![("X", 960), ("Y", 480)], vec![]);
    doc.apply(Command::SplitTask {
        uid: 1,
        at: at(0, 13, 0),
        gap: 480,
    })
    .unwrap();
    doc.apply(Command::Indent { uid: 2 }).unwrap();
    let x = task(&doc, 1);
    assert!(x.summary && x.splits.is_empty() && x.segments.is_empty());
}

#[test]
fn moving_the_start_keeps_segment_positions() {
    let mut doc = split_two_day_task();
    doc.apply(Command::MoveTask {
        uid: 1,
        start: at(1, 8, 0),
    })
    .unwrap();
    let a = task(&doc, 1);
    assert_eq!(
        a.splits,
        vec![Split {
            offset: 240,
            gap: 480
        }]
    );
    assert_eq!(
        a.segments,
        vec![
            seg(at(1, 8, 0), at(1, 12, 0)),
            seg(at(2, 13, 0), at(3, 17, 0))
        ]
    );
}

#[test]
fn resizing_extends_the_last_segment() {
    let mut doc = split_two_day_task();
    doc.apply(Command::ResizeTask {
        uid: 1,
        finish: at(3, 17, 0),
    })
    .unwrap();
    let a = task(&doc, 1);
    assert_eq!(a.duration, 1440);
    assert_eq!(
        a.splits,
        vec![Split {
            offset: 240,
            gap: 480
        }]
    );
    assert_eq!(a.segments[0], seg(at(0, 8, 0), at(0, 12, 0)));
    assert_eq!(a.segments[1], seg(at(1, 13, 0), at(3, 17, 0)));

    doc.apply(Command::SetDuration {
        uid: 1,
        minutes: 480,
    })
    .unwrap();
    let a = task(&doc, 1);
    assert_eq!(a.splits.len(), 1);
    assert_eq!(a.segments[1], seg(at(1, 13, 0), at(1, 17, 0)));

    // A finish inside the first segment drops the gap that no longer fits.
    doc.apply(Command::ResizeTask {
        uid: 1,
        finish: at(0, 11, 0),
    })
    .unwrap();
    let a = task(&doc, 1);
    assert!(a.splits.is_empty());
    assert_eq!(a.duration, 180);
    assert_eq!(a.finish, at(0, 11, 0));
}

#[test]
fn manual_task_finish_moves_with_the_gap() {
    let mut doc = doc_with(vec![("A", 960)], vec![]);
    doc.apply(Command::SetManual {
        uid: 1,
        manual: true,
        start: Some(at(0, 8, 0)),
        finish: Some(at(1, 17, 0)),
    })
    .unwrap();
    doc.apply(Command::SplitTask {
        uid: 1,
        at: at(0, 13, 0),
        gap: 480,
    })
    .unwrap();
    let a = task(&doc, 1);
    assert_eq!(a.finish, at(2, 17, 0));
    assert_eq!(a.segments.last().unwrap().finish, at(2, 17, 0));
    doc.apply(Command::RemoveSplit {
        uid: 1,
        index: None,
    })
    .unwrap();
    assert_eq!(task(&doc, 1).finish, at(1, 17, 0));
}

#[test]
fn split_and_join_undo_redo() {
    let mut doc = split_two_day_task();
    doc.apply(Command::RemoveSplit {
        uid: 1,
        index: Some(0),
    })
    .unwrap();
    assert!(task(&doc, 1).splits.is_empty());

    doc.undo().unwrap();
    assert_eq!(
        task(&doc, 1).splits,
        vec![Split {
            offset: 240,
            gap: 480
        }]
    );
    assert_eq!(task(&doc, 1).finish, at(2, 17, 0));
    doc.undo().unwrap();
    assert!(task(&doc, 1).splits.is_empty());
    assert_eq!(task(&doc, 1).finish, at(1, 17, 0));
    assert_eq!(task(&doc, 2).start, at(2, 8, 0));
    doc.redo().unwrap();
    assert_eq!(task(&doc, 1).finish, at(2, 17, 0));
    assert_eq!(task(&doc, 2).start, at(3, 8, 0));
    doc.redo().unwrap();
    assert!(task(&doc, 1).splits.is_empty());
}

fn temp_path(name: &str) -> String {
    std::env::temp_dir()
        .join(format!(
            "projectdesk-split-{name}-{}.pdproj",
            std::process::id()
        ))
        .to_string_lossy()
        .to_string()
}

#[test]
fn splits_survive_save_and_load() {
    let mut doc = split_two_day_task();
    doc.apply(Command::SetBaseline { number: 0 }).unwrap();
    let path = temp_path("roundtrip");
    save(&doc, &path).unwrap();
    let loaded = load(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(loaded.project.tasks, doc.project.tasks);
    let a = loaded.project.task(1).unwrap();
    assert_eq!(
        a.splits,
        vec![Split {
            offset: 240,
            gap: 480
        }]
    );
    assert_eq!(a.baselines[0].segments, a.segments);
    assert!(loaded
        .commands
        .iter()
        .any(|command| matches!(command, Command::SplitTask { .. })));
}

#[test]
fn file_without_split_fields_still_loads() {
    let mut doc = doc_with(vec![("A", 960), ("B", 480)], vec![(1, 2)]);
    doc.apply(Command::SetBaseline { number: 0 }).unwrap();
    let path = temp_path("old");
    save(&doc, &path).unwrap();

    // Rewrite the stored base and log the way a file from before splits looks.
    let connection = rusqlite::Connection::open(&path).unwrap();
    let strip = |text: String| -> String {
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        fn walk(value: &mut serde_json::Value) {
            match value {
                serde_json::Value::Object(map) => {
                    map.remove("splits");
                    map.remove("segments");
                    map.remove("planned");
                    map.values_mut().for_each(walk);
                }
                serde_json::Value::Array(items) => items.iter_mut().for_each(walk),
                _ => {}
            }
        }
        walk(&mut value);
        value.to_string()
    };
    let base: String = connection
        .query_row("SELECT value FROM meta WHERE key = 'base'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(base.contains("\"splits\""));
    let old_base = strip(base);
    assert!(!old_base.contains("\"splits\"") && !old_base.contains("\"segments\""));
    connection
        .execute("UPDATE meta SET value = ?1 WHERE key = 'base'", [old_base])
        .unwrap();
    drop(connection);

    let mut loaded = load(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let a = loaded.project.task(1).unwrap();
    assert!(a.splits.is_empty() && a.segments.is_empty());
    assert!(a.baselines[0].segments.is_empty());
    assert_eq!(a.finish, at(1, 17, 0));
    loaded
        .apply(Command::SplitTask {
            uid: 1,
            at: at(0, 13, 0),
            gap: 480,
        })
        .unwrap();
    assert_eq!(loaded.project.task(1).unwrap().finish, at(2, 17, 0));
}

fn hours_on(project: &Project, task_uid: u32, assignment_uid: Option<u32>) -> Vec<(String, f64)> {
    project
        .usage
        .iter()
        .find(|row| row.task_uid == task_uid && row.assignment_uid == assignment_uid)
        .unwrap()
        .days
        .iter()
        .map(|day| (day.date.clone(), day.hours))
        .collect()
}

#[test]
fn work_distribution_skips_the_gap() {
    let mut project = project_with(vec![("A", 960), ("B", 480)], vec![]);
    project
        .resources
        .push(Resource::new(1, "An", ResourceKind::Person));
    project.next_resource_uid = 2;
    let mut doc = Document::from_project(project);
    doc.apply(Command::AssignResource {
        task_uid: 1,
        resource_uid: 1,
        units: 1.0,
    })
    .unwrap();
    doc.apply(Command::AssignResource {
        task_uid: 2,
        resource_uid: 1,
        units: 1.0,
    })
    .unwrap();
    // B is held on Tuesday, where unsplit A also works: An is overallocated.
    doc.apply(Command::MoveTask {
        uid: 2,
        start: at(1, 8, 0),
    })
    .unwrap();
    assert!(doc
        .project
        .overallocations
        .iter()
        .any(|row| row.date == "2026-01-06"));

    // Split A at noon Monday for two days: no work on Tuesday.
    doc.apply(Command::SplitTask {
        uid: 1,
        at: at(0, 12, 0),
        gap: 960,
    })
    .unwrap();
    let a = task(&doc, 1);
    assert_eq!(
        a.segments,
        vec![
            seg(at(0, 8, 0), at(0, 12, 0)),
            seg(at(2, 13, 0), at(3, 17, 0))
        ]
    );
    let expected = vec![
        ("2026-01-05".to_string(), 4.0),
        ("2026-01-07".to_string(), 4.0),
        ("2026-01-08".to_string(), 8.0),
    ];
    assert_eq!(hours_on(&doc.project, 1, None), expected);
    let assignment = doc
        .project
        .assignments
        .iter()
        .find(|item| item.task_uid == 1)
        .unwrap()
        .uid;
    assert_eq!(hours_on(&doc.project, 1, Some(assignment)), expected);
    assert!(doc.project.overallocations.is_empty());
}

#[test]
fn planned_value_and_burndown_skip_the_gap() {
    let mut doc = doc_with(vec![("A", 960)], vec![]);
    doc.apply(Command::SplitTask {
        uid: 1,
        at: at(0, 12, 0),
        gap: 960,
    })
    .unwrap();
    doc.apply(Command::SetFixedCost {
        uid: 1,
        cost: 960.0,
        accrual: Accrual::Prorated,
    })
    .unwrap();
    doc.apply(Command::SetBaseline { number: 0 }).unwrap();
    let project = &doc.project;
    assert_eq!(project.task(1).unwrap().baselines[0].segments.len(), 2);

    // By Wednesday 17:00 the plan has 240 + 240 of 960 working minutes, not the
    // 1440 of 1920 a start-to-finish line would give.
    let pv = evm_at(project, at(2, 17, 0)).pv;
    assert!((pv - 480.0).abs() < 1e-9, "pv = {pv}");
    // Nothing more is planned during the gap.
    let monday_end = evm_at(project, at(0, 17, 0)).pv;
    let tuesday_end = evm_at(project, at(1, 17, 0)).pv;
    assert!((monday_end - 240.0).abs() < 1e-9 && (tuesday_end - 240.0).abs() < 1e-9);

    let report = burndown(project, &[]);
    assert_eq!(
        report.planned,
        vec![
            BurndownPoint {
                date: at(0, 8, 0),
                remaining_work: 960
            },
            BurndownPoint {
                date: at(1, 0, 0),
                remaining_work: 720
            },
            BurndownPoint {
                date: at(2, 0, 0),
                remaining_work: 720
            },
            BurndownPoint {
                date: at(3, 0, 0),
                remaining_work: 480
            },
            BurndownPoint {
                date: at(3, 17, 0),
                remaining_work: 0
            },
        ]
    );
}

#[test]
fn s_curve_planned_value_is_flat_across_the_gap() {
    let mut doc = doc_with(vec![("A", 960)], vec![]);
    doc.apply(Command::SplitTask {
        uid: 1,
        at: at(0, 12, 0),
        gap: 960,
    })
    .unwrap();
    doc.apply(Command::SetFixedCost {
        uid: 1,
        cost: 960.0,
        accrual: Accrual::Prorated,
    })
    .unwrap();
    doc.apply(Command::SetBaseline { number: 0 }).unwrap();

    let report = s_curve(&doc.project);
    let pv_on = |date: i64| -> f64 {
        report
            .points
            .iter()
            .find(|point| point.date == date)
            .unwrap_or_else(|| panic!("no S-curve point at {date}"))
            .pv
    };
    // Same phasing as evm_at: 240 before the gap, nothing during it, then the rest.
    assert!((pv_on(at(1, 0, 0)) - 240.0).abs() < 1e-9);
    assert!((pv_on(at(2, 0, 0)) - 240.0).abs() < 1e-9);
    assert!((pv_on(at(3, 0, 0)) - 480.0).abs() < 1e-9);
    assert!((pv_on(at(3, 17, 0)) - 960.0).abs() < 1e-9);
}

#[test]
fn splitting_an_occurrence_detaches_it() {
    let spec = RecurrenceSpec {
        name: "Họp".into(),
        duration: 120,
        start: at(0, 8, 0),
        pattern: RecurrencePattern::Daily { every: 1 },
        end: RecurrenceEnd::Count { count: 3 },
        calendar_id: None,
        resource_uids: Vec::new(),
        resources: Vec::new(),
        background: false,
    };
    let mut doc = doc_with(vec![("Đầu", 480)], vec![]);
    doc.apply(Command::CreateRecurringTask {
        after_uid: Some(1),
        spec: spec.clone(),
    })
    .unwrap();
    let summary = doc
        .project
        .tasks
        .iter()
        .find(|item| item.recurrence.is_some())
        .unwrap()
        .uid;
    let occurrence = doc
        .project
        .tasks
        .iter()
        .find(|item| item.series_uid == Some(summary))
        .unwrap()
        .uid;
    assert!(doc
        .apply(Command::SplitTask {
            uid: summary,
            at: at(0, 9, 0),
            gap: 60
        })
        .is_err());

    doc.apply(Command::SplitTask {
        uid: occurrence,
        at: at(0, 9, 0),
        gap: 60,
    })
    .unwrap();
    let split = task(&doc, occurrence);
    assert!(split.detached);
    assert_eq!(split.finish, at(0, 11, 0));

    // Regenerating the series keeps the split occurrence as it is.
    let mut renamed = spec;
    renamed.name = "Giao ban".into();
    doc.apply(Command::UpdateRecurringTask {
        uid: summary,
        spec: renamed,
    })
    .unwrap();
    let kept = task(&doc, occurrence);
    assert!(kept.detached);
    assert_eq!(
        kept.splits,
        vec![Split {
            offset: 60,
            gap: 60
        }]
    );
    assert_eq!(
        doc.project
            .tasks
            .iter()
            .filter(|item| item.series_uid == Some(summary))
            .count(),
        3
    );
}

#[test]
fn mspdi_exports_and_imports_split_segments() {
    let mut project = project_with(vec![("A", 960), ("B", 960), ("C", 480)], vec![(1, 3)]);
    project
        .resources
        .push(Resource::new(1, "An", ResourceKind::Person));
    project.next_resource_uid = 2;
    let mut doc = Document::from_project(project);
    doc.apply(Command::AssignResource {
        task_uid: 2,
        resource_uid: 1,
        units: 1.0,
    })
    .unwrap();
    doc.apply(Command::SplitTask {
        uid: 1,
        at: at(0, 13, 0),
        gap: 480,
    })
    .unwrap();
    doc.apply(Command::SplitTask {
        uid: 2,
        at: at(1, 8, 0),
        gap: 240,
    })
    .unwrap();

    let xml = write_mspdi(&doc.project).unwrap();
    assert!(xml.contains("<TimephasedData>"));
    // Unassigned A carries its segments on an unassigned-resource assignment.
    assert!(xml.contains("<ResourceUID>-65535</ResourceUID>"));
    assert!(xml.contains("<Value>PT0H0M0S</Value>"));

    let imported = read_mspdi(&xml).unwrap();
    for uid in [1, 2, 3] {
        let original = doc.project.task(uid).unwrap();
        let restored = imported.task(uid).unwrap();
        assert_eq!(restored.splits, original.splits, "task {uid}");
        assert_eq!(restored.segments, original.segments, "task {uid}");
        assert_eq!(restored.finish, original.finish, "task {uid}");
    }
    assert_eq!(imported.assignments.len(), 1);
    assert_eq!(imported.task(3).unwrap().start, at(3, 8, 0));
}
