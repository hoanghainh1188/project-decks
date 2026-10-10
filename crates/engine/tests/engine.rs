use chrono::Datelike;

use projectdesk_engine::command::{parse_predecessor_text, Command};
use projectdesk_engine::level::level_project;
use projectdesk_engine::model::{
    Accrual, ConstraintType, DepKind, Dependency, ResourceKind, Task, TaskType,
};
use projectdesk_engine::mspdi::{read_mspdi, write_mspdi};
use projectdesk_engine::schedule::schedule;
use projectdesk_engine::store::{load, save};
use projectdesk_engine::timeutil::timestamp;
use projectdesk_engine::work::{self, Changed};
use projectdesk_engine::{Document, Project};

fn monday() -> i64 {
    timestamp(2026, 1, 5, 8, 0)
}

fn at(day_offset: u32, hour: u32, minute: u32) -> i64 {
    // 2026-01-05 is Monday. day_offset 0 = Monday.
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 5)
        .unwrap()
        .checked_add_signed(chrono::Duration::days(day_offset as i64))
        .unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, minute)
}

fn project_with(tasks: Vec<(&str, i64)>, deps: Vec<(u32, u32, DepKind, i64)>) -> Project {
    let mut project = Project::new("Kiểm thử", monday());
    for (index, (name, duration)) in tasks.into_iter().enumerate() {
        let mut task = Task::new((index as u32) + 1, name);
        task.duration = duration;
        task.work = duration;
        task.milestone = duration == 0;
        task.remaining_duration = duration;
        task.remaining_work = duration;
        project.tasks.push(task);
    }
    project.next_task_uid = project.tasks.len() as u32 + 1;
    for (index, (pred, succ, kind, lag)) in deps.into_iter().enumerate() {
        project.dependencies.push(Dependency {
            uid: (index as u32) + 1,
            predecessor_uid: pred,
            successor_uid: succ,
            kind,
            lag,
        });
    }
    project.next_dependency_uid = project.dependencies.len() as u32 + 1;
    schedule(&mut project).expect("schedule");
    project
}

#[test]
fn fs_chain_is_critical() {
    let project = project_with(
        vec![("A", 960), ("B", 480), ("C", 480)],
        vec![(1, 2, DepKind::Fs, 0), (2, 3, DepKind::Fs, 0)],
    );
    let a = &project.tasks[0];
    let b = &project.tasks[1];
    let c = &project.tasks[2];
    assert_eq!(a.start, at(0, 8, 0));
    assert_eq!(a.finish, at(1, 17, 0));
    assert_eq!(b.start, at(2, 8, 0));
    assert_eq!(b.finish, at(2, 17, 0));
    assert_eq!(c.start, at(3, 8, 0));
    assert_eq!(c.finish, at(3, 17, 0));
    assert!(a.critical && b.critical && c.critical);
    assert_eq!(a.total_slack, 0);
    assert_eq!(b.predecessors, "1");
    assert_eq!(c.predecessors, "2");
}

#[test]
fn lag_crosses_a_holiday() {
    let mut project = Project::new("Nghỉ", monday());
    project
        .calendar_mut()
        .set_day_off(chrono::NaiveDate::from_ymd_opt(2026, 1, 6).unwrap());
    let mut a = Task::new(1, "A");
    a.duration = 480;
    a.work = 480;
    let mut b = Task::new(2, "B");
    b.duration = 480;
    b.work = 480;
    project.tasks = vec![a, b];
    project.next_task_uid = 3;
    project.dependencies.push(Dependency {
        uid: 1,
        predecessor_uid: 1,
        successor_uid: 2,
        kind: DepKind::Fs,
        lag: 480,
    });
    schedule(&mut project).unwrap();
    assert_eq!(project.tasks[0].finish, at(0, 17, 0));
    // Tuesday is a holiday, so the one-day lag lands on Wednesday and B starts Thursday.
    assert_eq!(project.tasks[1].start, at(3, 8, 0));
}

#[test]
fn ss_ff_and_sf() {
    // A is held until Wednesday so SS, FF, and SF are each the binding constraint.
    let mut project = Project::new("Liên kết", monday());
    let mut anchor = Task::new(1, "A");
    anchor.duration = 480;
    anchor.work = 480;
    anchor.constraint_type = ConstraintType::Snet;
    anchor.constraint_date = Some(at(2, 8, 0));
    let mut ss = Task::new(2, "SS");
    ss.duration = 480;
    ss.work = 480;
    let mut ff = Task::new(3, "FF");
    ff.duration = 480;
    ff.work = 480;
    let mut sf = Task::new(4, "SF");
    sf.duration = 480;
    sf.work = 480;
    project.tasks = vec![anchor, ss, ff, sf];
    project.next_task_uid = 5;
    project.dependencies = vec![
        Dependency {
            uid: 1,
            predecessor_uid: 1,
            successor_uid: 2,
            kind: DepKind::Ss,
            lag: 480,
        },
        Dependency {
            uid: 2,
            predecessor_uid: 1,
            successor_uid: 3,
            kind: DepKind::Ff,
            lag: 0,
        },
        Dependency {
            uid: 3,
            predecessor_uid: 1,
            successor_uid: 4,
            kind: DepKind::Sf,
            lag: 0,
        },
    ];
    schedule(&mut project).unwrap();
    assert_eq!(project.tasks[0].start, at(2, 8, 0));
    assert_eq!(project.tasks[0].finish, at(2, 17, 0));
    assert_eq!(project.tasks[1].start, at(3, 8, 0), "SS + 1 ngày");
    assert_eq!(project.tasks[2].finish, at(2, 17, 0), "FF");
    assert_eq!(project.tasks[2].start, at(2, 8, 0));
    assert_eq!(project.tasks[3].start, at(1, 8, 0), "SF");
    assert_eq!(project.tasks[3].finish, at(2, 8, 0));
}

#[test]
fn summary_rolls_up_children() {
    let mut project = Project::new("WBS", monday());
    let mut summary = Task::new(1, "Tóm tắt");
    summary.outline_level = 1;
    let mut a = Task::new(2, "A");
    a.outline_level = 2;
    a.duration = 960;
    a.work = 960;
    let mut b = Task::new(3, "B");
    b.outline_level = 2;
    b.duration = 480;
    b.work = 480;
    project.tasks = vec![summary, a, b];
    project.next_task_uid = 4;
    project.dependencies.push(Dependency {
        uid: 1,
        predecessor_uid: 2,
        successor_uid: 3,
        kind: DepKind::Fs,
        lag: 0,
    });
    schedule(&mut project).unwrap();
    let summary = &project.tasks[0];
    assert!(summary.summary);
    assert_eq!(summary.wbs, "1");
    assert_eq!(project.tasks[1].wbs, "1.1");
    assert_eq!(project.tasks[2].wbs, "1.2");
    assert_eq!(summary.start, project.tasks[1].start);
    assert_eq!(summary.finish, project.tasks[2].finish);
    assert_eq!(summary.duration, 1440);
    assert!(summary.critical);
}

#[test]
fn dependency_cycle_is_an_error() {
    let mut project = project_with(
        vec![("A", 480), ("B", 480)],
        vec![(1, 2, DepKind::Fs, 0)],
    );
    project.dependencies.push(Dependency {
        uid: 2,
        predecessor_uid: 2,
        successor_uid: 1,
        kind: DepKind::Fs,
        lag: 0,
    });
    let error = schedule(&mut project).unwrap_err();
    assert!(matches!(error, projectdesk_engine::EngineError::DependencyCycle));
}

#[test]
fn milestone_has_zero_duration() {
    let project = project_with(
        vec![("Mốc", 0), ("Sau", 480)],
        vec![(1, 2, DepKind::Fs, 0)],
    );
    assert!(project.tasks[0].milestone);
    assert_eq!(project.tasks[0].start, project.tasks[0].finish);
    assert_eq!(project.tasks[0].start, at(0, 8, 0));
    assert_eq!(project.tasks[1].start, at(0, 8, 0));
}

#[test]
fn snet_constraint_holds_the_start() {
    let mut project = project_with(vec![("A", 480)], vec![]);
    project.tasks[0].constraint_type = ConstraintType::Snet;
    project.tasks[0].constraint_date = Some(at(2, 8, 0));
    schedule(&mut project).unwrap();
    assert_eq!(project.tasks[0].start, at(2, 8, 0));
    assert_eq!(project.tasks[0].finish, at(2, 17, 0));
}

#[test]
fn remaining_constraints_are_honored() {
    let asap = project_with(vec![("A", 480)], vec![]);
    assert_eq!(asap.tasks[0].start, at(0, 8, 0));

    let mut alap = project_with(vec![("Dài", 1440), ("Muộn", 480)], vec![]);
    alap.tasks[1].constraint_type = ConstraintType::Alap;
    schedule(&mut alap).unwrap();
    assert_eq!(alap.tasks[1].start, at(2, 8, 0));
    assert_eq!(alap.tasks[1].finish, at(2, 17, 0));

    let mut mso = project_with(vec![("Ghim", 480)], vec![]);
    mso.tasks[0].constraint_type = ConstraintType::Mso;
    mso.tasks[0].constraint_date = Some(at(2, 10, 0));
    schedule(&mut mso).unwrap();
    assert_eq!(mso.tasks[0].start, at(2, 10, 0));

    let mut mfo = project_with(vec![("Xong", 480)], vec![]);
    mfo.tasks[0].constraint_type = ConstraintType::Mfo;
    mfo.tasks[0].constraint_date = Some(at(2, 17, 0));
    schedule(&mut mfo).unwrap();
    assert_eq!(mfo.tasks[0].finish, at(2, 17, 0));
    assert_eq!(mfo.tasks[0].start, at(2, 8, 0));

    let mut fnet = project_with(vec![("F", 480)], vec![]);
    fnet.tasks[0].constraint_type = ConstraintType::Fnet;
    fnet.tasks[0].constraint_date = Some(at(3, 17, 0));
    schedule(&mut fnet).unwrap();
    assert!(fnet.tasks[0].finish >= at(3, 17, 0));

    let mut snlt = project_with(
        vec![("A", 1440), ("B", 480)],
        vec![(1, 2, DepKind::Fs, 0)],
    );
    snlt.tasks[1].constraint_type = ConstraintType::Snlt;
    snlt.tasks[1].constraint_date = Some(at(0, 8, 0));
    schedule(&mut snlt).unwrap();
    assert!(snlt.tasks[1].total_slack < 0 || snlt.tasks[1].conflict);

    let mut fnlt = project_with(vec![("A", 1440)], vec![]);
    fnlt.tasks[0].constraint_type = ConstraintType::Fnlt;
    fnlt.tasks[0].constraint_date = Some(at(0, 12, 0));
    schedule(&mut fnlt).unwrap();
    assert!(fnlt.tasks[0].conflict || fnlt.tasks[0].total_slack < 0);
}

#[test]
fn alap_with_flexible_successor_stays_early() {
    let mut project = project_with(
        vec![("Muộn", 480), ("Kế", 480), ("Dài", 2400)],
        vec![(1, 2, DepKind::Fs, 0)],
    );
    project.tasks[0].constraint_type = ConstraintType::Alap;
    schedule(&mut project).unwrap();
    assert_eq!(project.tasks[0].start, at(0, 8, 0));
    assert_eq!(project.tasks[0].finish, at(0, 17, 0));
    assert_eq!(project.tasks[1].start, at(1, 8, 0));
    assert_eq!(project.tasks[2].finish, at(4, 17, 0));
}

#[test]
fn alap_without_successor_sits_at_project_end() {
    let mut project = project_with(vec![("Dài", 1440), ("Muộn", 480)], vec![]);
    project.tasks[1].constraint_type = ConstraintType::Alap;
    schedule(&mut project).unwrap();
    assert_eq!(project.tasks[1].start, at(2, 8, 0));
    assert_eq!(project.tasks[1].finish, at(2, 17, 0));
    assert_eq!(project.tasks[0].start, at(0, 8, 0));
}

#[test]
fn alap_chain_slides_together() {
    let mut project = project_with(
        vec![("Muộn", 480), ("Cũng muộn", 480), ("Dài", 2400)],
        vec![(1, 2, DepKind::Fs, 0)],
    );
    project.tasks[0].constraint_type = ConstraintType::Alap;
    project.tasks[1].constraint_type = ConstraintType::Alap;
    schedule(&mut project).unwrap();
    assert_eq!(project.tasks[2].start, at(0, 8, 0));
    assert_eq!(project.tasks[2].finish, at(4, 17, 0));
    assert_eq!(project.tasks[1].start, at(4, 8, 0));
    assert_eq!(project.tasks[1].finish, at(4, 17, 0));
    assert_eq!(project.tasks[0].start, at(3, 8, 0));
    assert_eq!(project.tasks[0].finish, at(3, 17, 0));
}

#[test]
fn successor_constraint_limits_alap_predecessor() {
    let mut snet = project_with(
        vec![("Muộn", 480), ("Ghim", 480), ("Dài", 2400)],
        vec![(1, 2, DepKind::Fs, 0)],
    );
    snet.tasks[0].constraint_type = ConstraintType::Alap;
    snet.tasks[1].constraint_type = ConstraintType::Snet;
    snet.tasks[1].constraint_date = Some(at(2, 8, 0));
    schedule(&mut snet).unwrap();
    assert_eq!(snet.tasks[1].start, at(2, 8, 0));
    assert_eq!(snet.tasks[0].start, at(1, 8, 0));
    assert_eq!(snet.tasks[0].finish, at(1, 17, 0));

    let mut mso = project_with(
        vec![("Muộn", 480), ("Ghim", 480), ("Dài", 2400)],
        vec![(1, 2, DepKind::Fs, 0)],
    );
    mso.tasks[0].constraint_type = ConstraintType::Alap;
    mso.tasks[1].constraint_type = ConstraintType::Mso;
    mso.tasks[1].constraint_date = Some(at(2, 10, 0));
    schedule(&mut mso).unwrap();
    assert_eq!(mso.tasks[1].start, at(2, 10, 0));
    assert_eq!(mso.tasks[0].finish, at(2, 10, 0));
    assert!(mso.tasks[0].start < mso.tasks[1].start);
    assert!(mso.tasks[0].start > at(0, 8, 0));
}

#[test]
fn tracking_edits_undo() {
    let mut doc = Document::from_project(Project::new("Theo dõi", monday()));
    let uid = doc.project.tasks[0].uid;
    doc.apply(Command::SetActualStart {
        uid,
        start: Some(at(1, 9, 0)),
    })
    .unwrap();
    assert_eq!(doc.project.tasks[0].actual_start, Some(at(1, 9, 0)));
    doc.apply(Command::SetActualFinish {
        uid,
        finish: Some(at(1, 17, 0)),
    })
    .unwrap();
    assert_eq!(doc.project.tasks[0].actual_finish, Some(at(1, 17, 0)));
    doc.apply(Command::SetRemainingWork { uid, minutes: 120 }).unwrap();
    doc.undo().unwrap();
    doc.undo().unwrap();
    doc.undo().unwrap();
    assert_eq!(doc.project.tasks[0].actual_start, None);
    assert_eq!(doc.project.tasks[0].actual_finish, None);

    let calendar_id = doc.project.calendar_id;
    doc.apply(Command::SetTaskCalendar {
        uid,
        calendar_id: Some(calendar_id),
    })
    .unwrap();
    assert_eq!(doc.project.tasks[0].calendar_id, Some(calendar_id));
    doc.undo().unwrap();
    assert_eq!(doc.project.tasks[0].calendar_id, None);

    doc.apply(Command::AddResource {
        name: "An".into(),
        kind: ResourceKind::Person,
    })
    .unwrap();
    let resource_uid = doc.project.resources[0].uid;
    doc.apply(Command::AssignResource {
        task_uid: uid,
        resource_uid,
        units: 1.0,
    })
    .unwrap();
    doc.apply(Command::SetOvertimeWork { uid, minutes: 120 }).unwrap();
    assert_eq!(doc.project.assignments[0].overtime_work, 120);
    doc.undo().unwrap();
    assert_eq!(doc.project.assignments[0].overtime_work, 0);
    doc.redo().unwrap();
    assert_eq!(doc.project.assignments[0].overtime_work, 120);
}

#[test]
fn manual_task_stays_pinned() {
    let mut project = project_with(vec![("Tay", 480), ("Sau", 480)], vec![(1, 2, DepKind::Fs, 0)]);
    project.tasks[0].manual = true;
    project.tasks[0].start = at(1, 8, 0);
    project.tasks[0].finish = at(1, 17, 0);
    schedule(&mut project).unwrap();
    assert_eq!(project.tasks[0].start, at(1, 8, 0));
    assert_eq!(project.tasks[1].start, at(2, 8, 0));
}

#[test]
fn task_type_recomputes_duration_units_and_work() {
    let mut project = project_with(vec![("A", 480)], vec![]);
    assert_eq!(project.tasks[0].work, 480);

    project.tasks[0].duration = 960;
    work::apply_change(&mut project, 1, Changed::Duration);
    assert_eq!(project.tasks[0].work, 960);
    assert!((project.tasks[0].units - 1.0).abs() < 0.001);

    project.tasks[0].work = 480;
    work::apply_change(&mut project, 1, Changed::Work);
    assert_eq!(project.tasks[0].duration, 480);

    project.tasks[0].units = 2.0;
    project.tasks[0].effort_driven = true;
    project.tasks[0].task_type = TaskType::FixedUnits;
    work::apply_change(&mut project, 1, Changed::Units);
    assert_eq!(project.tasks[0].duration, 240);
    assert_eq!(project.tasks[0].work, 480);

    project.tasks[0].task_type = TaskType::FixedDuration;
    project.tasks[0].duration = 480;
    project.tasks[0].units = 2.0;
    work::apply_change(&mut project, 1, Changed::Units);
    assert_eq!(project.tasks[0].duration, 480);
    assert_eq!(project.tasks[0].work, 960);

    project.tasks[0].work = 480;
    work::apply_change(&mut project, 1, Changed::Work);
    assert_eq!(project.tasks[0].duration, 480);
    assert!((project.tasks[0].units - 1.0).abs() < 0.001);

    project.tasks[0].task_type = TaskType::FixedWork;
    project.tasks[0].work = 480;
    project.tasks[0].units = 1.0;
    project.tasks[0].duration = 960;
    work::apply_change(&mut project, 1, Changed::Duration);
    assert!((project.tasks[0].units - 0.5).abs() < 0.001);
    assert_eq!(project.tasks[0].work, 480);

    project.tasks[0].units = 2.0;
    work::apply_change(&mut project, 1, Changed::Units);
    assert_eq!(project.tasks[0].duration, 240);
}

#[test]
fn tracking_baseline_and_cost() {
    let mut project = project_with(vec![("A", 480)], vec![]);
    let uid = project.alloc_resource_uid();
    let mut resource = projectdesk_engine::Resource::new(uid, "An", ResourceKind::Person);
    resource.standard_rate = 10.0;
    resource.overtime_rate = 20.0;
    resource.cost_per_use = 5.0;
    project.resources.push(resource);
    work::add_assignment(&mut project, 1, uid, 1.0);
    project.tasks[0].fixed_cost = 20.0;
    project.tasks[0].fixed_cost_accrual = Accrual::Prorated;
    project.tasks[0].percent_complete = 50.0;
    schedule(&mut project).unwrap();
    assert_eq!(project.tasks[0].remaining_duration, 240);
    assert_eq!(project.tasks[0].work, 480);
    // 8h * 10 + 5 per use + 20 fixed = 105. Half is actual.
    assert!((project.tasks[0].cost - 105.0).abs() < 0.01);
    assert!((project.tasks[0].actual_cost - 52.5).abs() < 0.01);

    project.tasks[0].fixed_cost_accrual = Accrual::Start;
    schedule(&mut project).unwrap();
    assert!((project.tasks[0].actual_cost - 105.0).abs() < 0.01);
    project.tasks[0].fixed_cost_accrual = Accrual::End;
    project.tasks[0].percent_complete = 50.0;
    schedule(&mut project).unwrap();
    assert!(project.tasks[0].actual_cost.abs() < 0.01);

    project.tasks[0].percent_complete = 100.0;
    schedule(&mut project).unwrap();
    let start = project.tasks[0].start;
    let finish = project.tasks[0].finish;
    let mut doc = Document::from_project(project);
    doc.apply(Command::SetBaseline { number: 0 }).unwrap();
    doc.apply(Command::SetBaseline { number: 10 }).unwrap();
    let saved = doc.project.tasks[0]
        .baselines
        .iter()
        .find(|baseline| baseline.number == 0)
        .unwrap();
    assert_eq!(saved.start, start);
    assert_eq!(saved.finish, finish);
    assert!(doc.project.tasks[0].baselines.iter().any(|b| b.number == 10));
}

#[test]
fn overallocation_and_leveling_delay() {
    let mut project = project_with(vec![("A", 960), ("B", 960)], vec![]);
    project.tasks[1].priority = 100;
    let resource = project.alloc_resource_uid();
    project.resources.push(projectdesk_engine::Resource::new(
        resource,
        "Máy cắt",
        ResourceKind::Machine,
    ));
    work::add_assignment(&mut project, 1, resource, 1.0);
    work::add_assignment(&mut project, 2, resource, 1.0);
    schedule(&mut project).unwrap();
    assert!(!project.overallocations.is_empty());
    level_project(&mut project).unwrap();
    let delayed = project.tasks.iter().find(|task| task.uid == 2).unwrap();
    assert!(delayed.leveling_delay > 0);
    assert!(delayed.start > project.tasks[0].start);
    assert!(project.overallocations.is_empty());
}

#[test]
fn undo_redo_and_sqlite_roundtrip() {
    let mut doc = Document::from_project(Project::new("Tệp", monday()));
    let before = doc.project.tasks.len();
    doc.apply(Command::AddTask {
        name: "Thêm".into(),
        after_uid: None,
    })
    .unwrap();
    assert_eq!(doc.project.tasks.len(), before + 1);
    doc.undo().unwrap();
    assert_eq!(doc.project.tasks.len(), before);
    doc.redo().unwrap();
    assert_eq!(doc.project.tasks.len(), before + 1);

    let path = std::env::temp_dir().join(format!("projectdesk-test-{}.pdproj", std::process::id()));
    let path = path.to_string_lossy().to_string();
    save(&doc, &path).unwrap();
    let loaded = load(&path).unwrap();
    assert_eq!(loaded.project.tasks.len(), doc.project.tasks.len());
    assert_eq!(loaded.project.tasks.last().unwrap().name, "Thêm");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn mspdi_roundtrip_covers_phase_fields() {
    let mut project = project_with(
        vec![("Thiết kế", 960), ("Xây", 480)],
        vec![(1, 2, DepKind::Fs, 480)],
    );
    project
        .calendar_mut()
        .set_day_off(chrono::NaiveDate::from_ymd_opt(2026, 1, 7).unwrap());
    project.tasks[0].notes = "Ghi chú <A&B>".to_string();
    project.tasks[0].priority = 700;
    project.tasks[0].constraint_type = ConstraintType::Snet;
    project.tasks[0].constraint_date = Some(at(0, 8, 0));
    project.tasks[1].task_type = TaskType::FixedDuration;
    project.tasks[1].percent_complete = 25.0;
    let resource_uid = project.alloc_resource_uid();
    let mut resource = projectdesk_engine::Resource::new(resource_uid, "Bình", ResourceKind::Person);
    resource.standard_rate = 12.0;
    resource.max_units = 1.0;
    project.resources.push(resource);
    work::add_assignment(&mut project, 2, resource_uid, 1.0);
    schedule(&mut project).unwrap();
    let baseline = {
        let task = &project.tasks[0];
        projectdesk_engine::model::Baseline {
            number: 0,
            start: task.start,
            finish: task.finish,
            work: task.work,
            cost: task.cost,
        }
    };
    project.tasks[0].baselines.push(baseline);
    let xml = write_mspdi(&project).unwrap();
    assert!(xml.contains("http://schemas.microsoft.com/project"));
    let imported = read_mspdi(&xml).unwrap();
    assert_eq!(imported.tasks.len(), 2);
    assert_eq!(imported.tasks[0].name, "Thiết kế");
    assert_eq!(imported.tasks[1].predecessors.contains("1FS"), true);
    assert!(imported.calendar().exceptions.contains_key("2026-01-07"));
    assert_eq!(imported.resources.len(), 1);
    assert_eq!(imported.resources[0].name, "Bình");
    assert_eq!(imported.assignments.len(), 1);
    assert!(imported.tasks[0].baselines.iter().any(|b| b.number == 0));
    assert_eq!(imported.tasks[0].notes, "Ghi chú <A&B>");
}

#[test]
fn predecessor_text_parses_lag_units() {
    let links = parse_predecessor_text("1FS+1d, 2SS-4h, 3").unwrap();
    assert_eq!(links[0], (1, DepKind::Fs, 480));
    assert_eq!(links[1], (2, DepKind::Ss, -240));
    assert_eq!(links[2], (3, DepKind::Fs, 0));
}
