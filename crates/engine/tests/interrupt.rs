//! The interrupt flag (phase 18 P2, step 5a): an urgent task goes ahead of work in
//! progress that ranks below it on the same person; that work is split where the urgent
//! task starts and its remaining work resumes after it. Splits are made when leveling and
//! are the leveling's own: Clear Leveling removes them, user splits stay. Dates are worked
//! out by hand on the default calendar (Mon–Fri, 08–12 and 13–17).

use chrono::Datelike;

use sheafly_engine::command::Command;
use sheafly_engine::document::replay;
use sheafly_engine::level_order::{Criterion, SortKey};
use sheafly_engine::model::{ConstraintType, ResourceKind, Task};
use sheafly_engine::mspdi::{read_mspdi, write_mspdi};
use sheafly_engine::split::{Segment, Split};
use sheafly_engine::store::{load, save};
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::{Document, EngineError, Invalid, Project};

const DAY: i64 = 480;

/// 2026-01-05 is a Monday; `day` 0 is that Monday.
fn at(day: u32, hour: u32, minute: u32) -> i64 {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 5)
        .unwrap()
        .checked_add_signed(chrono::Duration::days(i64::from(day)))
        .unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, minute)
}

/// Tasks of these lengths (in days) from Monday and people 1..=`people`.
fn doc_with(days: &[i64], people: u32) -> Document {
    let mut project = Project::new("Chen ngang", at(0, 8, 0));
    for (index, days) in days.iter().enumerate() {
        let mut task = Task::new(index as u32 + 1, format!("Việc {}", index + 1));
        task.duration = days * DAY;
        task.work = days * DAY;
        task.remaining_duration = task.duration;
        task.remaining_work = task.work;
        project.tasks.push(task);
    }
    project.next_task_uid = days.len() as u32 + 1;
    let mut doc = Document::from_project(project);
    for person in 1..=people {
        let name = format!("Người {person}");
        doc.apply(Command::AddResource {
            name,
            kind: ResourceKind::Person,
        })
        .unwrap();
    }
    doc
}

fn apply(doc: &mut Document, command: Command) {
    doc.apply(command).unwrap();
}

fn assign(doc: &mut Document, task_uid: u32, resource_uid: u32, units: f64) {
    apply(
        doc,
        Command::AssignResource {
            task_uid,
            resource_uid,
            units,
        },
    );
}

fn snet(doc: &mut Document, uid: u32, date: i64) {
    apply(
        doc,
        Command::SetConstraint {
            uid,
            constraint_type: ConstraintType::Snet,
            date: Some(date),
        },
    );
}

fn urgent(doc: &mut Document, uid: u32) {
    apply(doc, Command::SetPriority { uid, priority: 900 });
    apply(
        doc,
        Command::SetInterrupts {
            uid,
            interrupts: true,
        },
    );
}

fn level(doc: &mut Document) {
    apply(doc, Command::LevelResources);
}

fn task(doc: &Document, uid: u32) -> &Task {
    doc.project.task(uid).unwrap()
}

fn dates(doc: &Document, uid: u32) -> (i64, i64) {
    (task(doc, uid).start, task(doc, uid).finish)
}

fn segments(doc: &Document, uid: u32) -> Vec<(i64, i64)> {
    task(doc, uid)
        .segments
        .iter()
        .map(|Segment { start, finish }| (*start, *finish))
        .collect()
}

fn leveling_splits(doc: &Document, uid: u32) -> Vec<Split> {
    task(doc, uid)
        .splits
        .iter()
        .filter(|split| split.leveling)
        .cloned()
        .collect()
}

/// Task 1 (three days) ready on Monday, task 2 (one day, urgent) able to start Tuesday,
/// both on one person.
fn urgent_on_tuesday() -> Document {
    let mut doc = doc_with(&[3, 1], 1);
    assign(&mut doc, 1, 1, 1.0);
    assign(&mut doc, 2, 1, 1.0);
    snet(&mut doc, 2, at(1, 8, 0));
    doc
}

#[test]
fn without_the_flag_the_urgent_task_waits_for_the_work_in_progress() {
    let mut doc = urgent_on_tuesday();
    apply(
        &mut doc,
        Command::SetPriority {
            uid: 2,
            priority: 900,
        },
    );
    level(&mut doc);
    assert_eq!(dates(&doc, 1), (at(0, 8, 0), at(2, 17, 0)));
    assert_eq!(dates(&doc, 2), (at(3, 8, 0), at(3, 17, 0)));
    assert!(task(&doc, 1).splits.is_empty());
}

#[test]
fn an_urgent_task_splits_planned_work_running_when_it_starts() {
    let mut doc = urgent_on_tuesday();
    urgent(&mut doc, 2);
    level(&mut doc);
    assert_eq!(dates(&doc, 2), (at(1, 8, 0), at(1, 17, 0)));
    assert_eq!(
        segments(&doc, 1),
        vec![(at(0, 8, 0), at(0, 17, 0)), (at(2, 8, 0), at(3, 17, 0))]
    );
    assert_eq!(
        leveling_splits(&doc, 1),
        vec![Split {
            offset: DAY,
            gap: DAY,
            leveling: true
        }]
    );
    assert!(doc.project.overallocations.is_empty());
}

#[test]
fn ordinary_work_placed_after_an_urgent_task_flows_around_it() {
    // Priority first: the urgent task is placed before task 1, which would otherwise wait
    // until Wednesday and leave Monday empty.
    let mut doc = urgent_on_tuesday();
    let order = vec![SortKey {
        criterion: Criterion::Priority,
        descending: true,
    }];
    apply(&mut doc, Command::SetLevelingOrder { order });
    apply(
        &mut doc,
        Command::SetPriority {
            uid: 2,
            priority: 900,
        },
    );
    level(&mut doc);
    assert_eq!(dates(&doc, 1), (at(2, 8, 0), at(4, 17, 0)));

    apply(
        &mut doc,
        Command::SetInterrupts {
            uid: 2,
            interrupts: true,
        },
    );
    level(&mut doc);
    assert_eq!(dates(&doc, 2), (at(1, 8, 0), at(1, 17, 0)));
    assert_eq!(
        segments(&doc, 1),
        vec![(at(0, 8, 0), at(0, 17, 0)), (at(2, 8, 0), at(3, 17, 0))]
    );
    assert!(doc.project.overallocations.is_empty());
}

#[test]
fn an_urgent_task_splits_started_work_after_the_part_done() {
    let mut doc = doc_with(&[3, 1], 1);
    assign(&mut doc, 1, 1, 1.0);
    assign(&mut doc, 2, 1, 1.0);
    apply(
        &mut doc,
        Command::SetActualStart {
            uid: 1,
            start: Some(at(0, 8, 0)),
        },
    );
    apply(
        &mut doc,
        Command::SetPercentComplete {
            uid: 1,
            percent: 100.0 / 3.0,
        },
    );
    urgent(&mut doc, 2);
    level(&mut doc);
    // The day done stays; the urgent task comes next; the rest of task 1 after it.
    assert_eq!(dates(&doc, 2), (at(1, 8, 0), at(1, 17, 0)));
    assert_eq!(
        segments(&doc, 1),
        vec![(at(0, 8, 0), at(0, 17, 0)), (at(2, 8, 0), at(3, 17, 0))]
    );
    assert_eq!(task(&doc, 1).actual_start, Some(at(0, 8, 0)));
}

#[test]
fn started_work_keeps_its_part_done_however_small() {
    // An actual start marks 1% done (14 minutes of three days): the urgent task comes
    // right after those minutes, never over them.
    let mut doc = doc_with(&[3, 1], 1);
    assign(&mut doc, 1, 1, 1.0);
    assign(&mut doc, 2, 1, 1.0);
    apply(
        &mut doc,
        Command::SetActualStart {
            uid: 1,
            start: Some(at(0, 8, 0)),
        },
    );
    urgent(&mut doc, 2);
    level(&mut doc);
    assert_eq!(task(&doc, 1).percent_complete, 1.0);
    assert_eq!(dates(&doc, 2).0, at(0, 8, 14));
    assert_eq!(segments(&doc, 1)[0], (at(0, 8, 0), at(0, 8, 14)));
    assert_eq!(
        leveling_splits(&doc, 1),
        vec![Split {
            offset: 14,
            gap: DAY,
            leveling: true
        }]
    );
}

/// A change made to a document before leveling.
type Setup = dyn Fn(&mut Document);

#[test]
fn only_work_ranked_below_and_free_to_move_is_interrupted() {
    let cases: Vec<(&str, Box<Setup>)> = vec![
        (
            "ranked above",
            Box::new(|doc| {
                apply(
                    doc,
                    Command::SetPriority {
                        uid: 1,
                        priority: 950,
                    },
                )
            }),
        ),
        (
            "ranked the same",
            Box::new(|doc| {
                apply(
                    doc,
                    Command::SetPriority {
                        uid: 1,
                        priority: 900,
                    },
                )
            }),
        ),
        (
            "urgent too",
            Box::new(|doc| {
                apply(
                    doc,
                    Command::SetInterrupts {
                        uid: 1,
                        interrupts: true,
                    },
                )
            }),
        ),
        (
            "not leveled",
            Box::new(|doc| {
                apply(
                    doc,
                    Command::SetPriority {
                        uid: 1,
                        priority: 1000,
                    },
                )
            }),
        ),
        (
            "must start on",
            Box::new(|doc| {
                let command = Command::SetConstraint {
                    uid: 1,
                    constraint_type: ConstraintType::Mso,
                    date: Some(at(0, 8, 0)),
                };
                apply(doc, command)
            }),
        ),
        (
            "manual",
            Box::new(|doc| {
                let command = Command::SetManual {
                    uid: 1,
                    manual: true,
                    start: Some(at(0, 8, 0)),
                    finish: Some(at(2, 17, 0)),
                };
                apply(doc, command)
            }),
        ),
    ];
    for (case, setup) in cases {
        let mut doc = urgent_on_tuesday();
        urgent(&mut doc, 2);
        setup(&mut doc);
        level(&mut doc);
        assert!(leveling_splits(&doc, 1).is_empty(), "{case}");
        assert_eq!(task(&doc, 1).segments.len(), 0, "{case}");
    }
}

#[test]
fn only_shared_people_who_run_out_of_room_are_interrupted() {
    // Task 1 on person 1 at half time, task 2 urgent on person 1 at half time: both fit.
    let mut doc = doc_with(&[3, 1], 2);
    assign(&mut doc, 1, 1, 0.5);
    assign(&mut doc, 2, 1, 0.5);
    snet(&mut doc, 2, at(1, 8, 0));
    urgent(&mut doc, 2);
    level(&mut doc);
    assert!(task(&doc, 1).splits.is_empty());
    assert_eq!(dates(&doc, 2).0, at(1, 8, 0));

    // Task 1 on two people, the urgent task on one of them: task 1 is split.
    let mut doc = doc_with(&[3, 1], 2);
    assign(&mut doc, 1, 1, 1.0);
    assign(&mut doc, 1, 2, 1.0);
    assign(&mut doc, 2, 2, 1.0);
    snet(&mut doc, 2, at(1, 8, 0));
    urgent(&mut doc, 2);
    level(&mut doc);
    assert_eq!(dates(&doc, 2).0, at(1, 8, 0));
    assert_eq!(leveling_splits(&doc, 1).len(), 1);
}

#[test]
fn leveling_twice_gives_the_same_result_and_clearing_keeps_user_splits() {
    let mut doc = urgent_on_tuesday();
    urgent(&mut doc, 2);
    // A split of the user's own, late in task 1's work.
    apply(
        &mut doc,
        Command::SplitTask {
            uid: 1,
            at: at(2, 13, 0),
            gap: 60,
        },
    );
    let user: Vec<Split> = task(&doc, 1).splits.clone();
    assert!(user.iter().all(|split| !split.leveling));
    level(&mut doc);
    let once = doc.project.clone();
    level(&mut doc);
    assert_eq!(doc.project.tasks, once.tasks);
    assert_eq!(
        task(&doc, 1)
            .splits
            .iter()
            .filter(|split| !split.leveling)
            .count(),
        1
    );

    apply(&mut doc, Command::ClearLeveling);
    assert_eq!(task(&doc, 1).splits, user);
    doc.undo().unwrap();
    assert_eq!(doc.project.tasks, once.tasks);
}

#[test]
fn leveling_splits_replay_from_the_log_and_survive_a_file() {
    let mut doc = urgent_on_tuesday();
    urgent(&mut doc, 2);
    level(&mut doc);
    let logged = doc.commands.last().unwrap().clone();
    let Command::ApplyLeveling { splits, .. } = &logged else {
        panic!("logged as {logged:?}");
    };
    assert_eq!(splits.len(), 1);
    assert_eq!(splits[0].uid, 1);
    let replayed = replay(&doc.base, &doc.commands).unwrap();
    assert_eq!(replayed.tasks, doc.project.tasks);

    let dir = std::env::temp_dir().join(format!("sheafly-interrupt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("interrupt.sheafly").to_string_lossy().to_string();
    save(&doc, &path).unwrap();
    let opened = load(&path).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(opened.project.tasks, doc.project.tasks);

    // Files from before the flag: no `interrupts`, no `leveling` on splits, no `splits`
    // on ApplyLeveling.
    let mut json = serde_json::to_value(&doc.project).unwrap();
    json["tasks"][1]
        .as_object_mut()
        .unwrap()
        .remove("interrupts");
    json["tasks"][0]["splits"][0]
        .as_object_mut()
        .unwrap()
        .remove("leveling");
    let old: Project = serde_json::from_value(json).unwrap();
    assert!(!old.tasks[1].interrupts);
    assert!(!old.tasks[0].splits[0].leveling);
    let command: Command =
        serde_json::from_str(r#"{"type":"apply_leveling","delays":[]}"#).unwrap();
    assert!(matches!(command, Command::ApplyLeveling { ref splits, .. } if splits.is_empty()));
    // A split of the user's is written as before, without the flag.
    let plain = serde_json::to_value(Split {
        offset: 60,
        gap: 60,
        leveling: false,
    })
    .unwrap();
    assert_eq!(plain, serde_json::json!({ "offset": 60, "gap": 60 }));
}

#[test]
fn mspdi_keeps_the_dates_of_split_work() {
    let mut doc = urgent_on_tuesday();
    urgent(&mut doc, 2);
    level(&mut doc);
    let xml = write_mspdi(&doc.project).unwrap();
    let back = read_mspdi(&xml).unwrap();
    let mut imported = Document::from_project(back);
    imported
        .apply(Command::RenameProject {
            name: "Nhập".into(),
        })
        .unwrap();
    let segments_back: Vec<(i64, i64)> = imported
        .project
        .task(1)
        .unwrap()
        .segments
        .iter()
        .map(|s| (s.start, s.finish))
        .collect();
    assert_eq!(segments_back, segments(&doc, 1));
    // The flag does not cross: Project has nothing like it.
    assert!(!imported.project.task(2).unwrap().interrupts);
}

#[test]
fn the_flag_is_for_leaf_tasks_and_detaches_an_occurrence() {
    let mut doc = doc_with(&[1, 1], 1);
    apply(&mut doc, Command::Indent { uid: 2 });
    let refused = doc
        .apply(Command::SetInterrupts {
            uid: 1,
            interrupts: true,
        })
        .map(|_| ());
    assert!(
        matches!(refused, Err(EngineError::Invalid(Invalid::InterruptsLeaf))),
        "{refused:?}"
    );
    apply(
        &mut doc,
        Command::AddMilestone {
            name: "Mốc".into(),
            after_uid: None,
        },
    );
    let milestone = doc.project.tasks.last().unwrap().uid;
    let refused = doc
        .apply(Command::SetInterrupts {
            uid: milestone,
            interrupts: true,
        })
        .map(|_| ());
    assert!(
        matches!(refused, Err(EngineError::Invalid(Invalid::InterruptsLeaf))),
        "{refused:?}"
    );
    apply(
        &mut doc,
        Command::SetInterrupts {
            uid: 2,
            interrupts: true,
        },
    );
    assert!(task(&doc, 2).interrupts);
    doc.undo().unwrap();
    assert!(!task(&doc, 2).interrupts);
    let sent = r#"{"type":"set_interrupts","uid":2,"interrupts":true}"#;
    assert!(serde_json::from_str::<Command>(sent).is_ok());
}

#[test]
fn a_copy_leaves_leveling_splits_behind() {
    let mut doc = urgent_on_tuesday();
    urgent(&mut doc, 2);
    level(&mut doc);
    assert_eq!(leveling_splits(&doc, 1).len(), 1);
    let copied = sheafly_engine::clipboard::copy_tasks(&doc.project, &[1, 2]);
    assert!(copied.tasks.iter().all(|task| task.splits.is_empty()));
    // The flag goes with the copy, like the priority.
    apply(
        &mut doc,
        Command::PasteTasks {
            after_uid: Some(2),
            clipboard: copied,
        },
    );
    let pasted = doc.project.tasks.last().unwrap();
    assert!(pasted.interrupts && pasted.splits.is_empty());
}

#[test]
fn work_pushed_by_a_split_is_placed_again_next_to_other_work() {
    // Ordered by priority. Task 1 (three days, person 1, a day done) is followed by task 2
    // (one day, person 2) on Thursday; task 3 (person 2) is fixed on Friday; both are
    // placed before task 4, urgent on person 1, which splits task 1 and pushes task 2 onto
    // Friday, where task 3 is.
    let mut doc = doc_with(&[3, 1, 1, 1], 2);
    assign(&mut doc, 1, 1, 1.0);
    assign(&mut doc, 2, 2, 1.0);
    assign(&mut doc, 3, 2, 1.0);
    assign(&mut doc, 4, 1, 1.0);
    let order = vec![SortKey {
        criterion: Criterion::Priority,
        descending: true,
    }];
    apply(&mut doc, Command::SetLevelingOrder { order });
    apply(
        &mut doc,
        Command::AddDependency {
            predecessor_uid: 1,
            successor_uid: 2,
            kind: sheafly_engine::model::DepKind::Fs,
            lag: 0,
        },
    );
    apply(
        &mut doc,
        Command::SetActualStart {
            uid: 1,
            start: Some(at(0, 8, 0)),
        },
    );
    apply(
        &mut doc,
        Command::SetPercentComplete {
            uid: 1,
            percent: 100.0 / 3.0,
        },
    );
    apply(
        &mut doc,
        Command::SetPriority {
            uid: 2,
            priority: 950,
        },
    );
    apply(
        &mut doc,
        Command::SetPriority {
            uid: 3,
            priority: 960,
        },
    );
    snet(&mut doc, 3, at(4, 8, 0));
    urgent(&mut doc, 4);
    level(&mut doc);
    assert_eq!(dates(&doc, 4), (at(1, 8, 0), at(1, 17, 0)));
    assert_eq!(
        segments(&doc, 1),
        vec![(at(0, 8, 0), at(0, 17, 0)), (at(2, 8, 0), at(3, 17, 0))]
    );
    assert_eq!(dates(&doc, 3), (at(4, 8, 0), at(4, 17, 0)));
    // Task 2 follows task 1 onto Friday, where task 3 is: it waits for Monday.
    assert_eq!(dates(&doc, 2), (at(7, 8, 0), at(7, 17, 0)));
    assert!(
        doc.project.overallocations.is_empty(),
        "{:?}",
        doc.project.overallocations
    );
}

#[test]
fn a_leveling_pause_dragged_by_hand_becomes_the_users() {
    let mut doc = urgent_on_tuesday();
    urgent(&mut doc, 2);
    level(&mut doc);
    apply(
        &mut doc,
        Command::MoveSegment {
            uid: 1,
            index: 1,
            start: at(3, 8, 0),
        },
    );
    assert_eq!(
        task(&doc, 1).splits,
        vec![Split {
            offset: DAY,
            gap: 2 * DAY,
            leveling: false
        }]
    );
    apply(&mut doc, Command::ClearLeveling);
    assert_eq!(task(&doc, 1).splits.len(), 1);
}
