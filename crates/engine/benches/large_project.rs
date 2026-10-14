//! Timing for large projects: apply, undo, redo, save, and load.
//!
//! `cargo bench -p sheafly-engine` builds a project of grouped tasks with chained
//! and summary-to-summary links, then pads the log with edits. Pass `-- <tasks> <commands>`
//! to change the size (default 1000 tasks, 10000 commands).
//!
//! `-- leveling` levels 500 tasks on ten people. `-- capacity [<people> <tasks>]` levels a
//! sprint-sized backlog (default 100 people, 1000 tasks): unlinked tasks of one to sixteen
//! hours with random priorities, one person each, built straight into the model so the
//! setup does not replay thousands of commands. Add `field` to level the backlog by a
//! custom choice field first (`[Field asc, Priority desc, Start asc]`), most tasks having
//! a value, and `leave` to give each person up to three periods of other units (a week
//! off, weeks at half time).
//!
//! `-- suggest [<people> <tasks>]` times the assignment suggestions for an unassigned
//! backlog (default 500 people, 5000 tasks): twenty skills, one to three per person, and
//! twenty rules from the values of a list field to the skills.

use std::time::{Duration, Instant};

use chrono::NaiveDate;

use sheafly_engine::availability::AvailabilityPeriod;
use sheafly_engine::command::Command;
use sheafly_engine::fields::{FieldDef, FieldKind, FieldValue};
use sheafly_engine::level::level_project;
use sheafly_engine::level_order::{Criterion, SortKey};
use sheafly_engine::model::{Assignment, DepKind, Resource, ResourceKind, Task, TaskType};
use sheafly_engine::skills::{AssignmentRule, Condition, RuleTarget, SkillDef};
use sheafly_engine::suggest::suggest;
use sheafly_engine::store::{load, save};
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::{schedule, Document, Project};

const GROUP: u32 = 10;
const DAY: i64 = 480;

/// Commands for `tasks` tasks in groups of ten: the first task of a group is its summary,
/// leaves are chained FS, and each summary follows the previous one. Edits pad the log.
fn build_commands(tasks: u32, total: usize) -> Vec<Command> {
    let mut commands = Vec::new();
    // Task 1 already exists in a new document.
    for uid in 2..=tasks {
        commands.push(Command::AddTask {
            name: format!("Việc {uid}"),
            after_uid: Some(uid - 1),
        });
    }
    for uid in 1..=tasks {
        if !(uid - 1).is_multiple_of(GROUP) {
            commands.push(Command::Indent { uid });
        }
    }
    for uid in 1..=tasks {
        if !(uid - 1).is_multiple_of(GROUP) {
            commands.push(Command::SetDuration {
                uid,
                minutes: DAY * i64::from(1 + uid % 4),
            });
        }
    }
    for uid in 1..=tasks {
        let position = (uid - 1) % GROUP;
        if position >= 2 {
            commands.push(dependency(uid - 1, uid));
        } else if position == 0 && uid > GROUP {
            commands.push(dependency(uid - GROUP, uid));
        }
    }
    let mut edit = 0u32;
    while commands.len() < total {
        let uid = 2 + edit % (tasks - 1);
        if !(uid - 1).is_multiple_of(GROUP) {
            commands.push(Command::SetDuration {
                uid,
                minutes: DAY * i64::from(1 + edit % 5),
            });
        } else {
            commands.push(Command::RenameTask {
                uid,
                name: format!("Nhóm {uid}.{edit}"),
            });
        }
        edit += 1;
    }
    commands
}

fn dependency(predecessor_uid: u32, successor_uid: u32) -> Command {
    Command::AddDependency {
        predecessor_uid,
        successor_uid,
        kind: DepKind::Fs,
        lag: 0,
    }
}

fn timed<T>(label: &str, f: impl FnOnce() -> T) -> (T, Duration) {
    let started = Instant::now();
    let value = f();
    let elapsed = started.elapsed();
    println!("{label:<28} {:>10.1} ms", elapsed.as_secs_f64() * 1000.0);
    (value, elapsed)
}

/// 500 unlinked tasks of one to three days, all from the project start, shared by ten
/// people: fifty-way clashes for leveling to resolve.
fn leveling(tasks: u32) {
    let mut doc = Document::from_project(Project::new("Cân bằng", timestamp(2026, 1, 5, 8, 0)));
    let mut commands = Vec::new();
    for uid in 2..=tasks {
        commands.push(Command::AddTask { name: format!("Việc {uid}"), after_uid: Some(uid - 1) });
    }
    for uid in 1..=tasks {
        commands.push(Command::SetDuration { uid, minutes: DAY * i64::from(1 + uid % 3) });
    }
    for person in 0..10 {
        commands.push(Command::AddResource { name: format!("Người {person}"), kind: ResourceKind::Person });
    }
    for uid in 1..=tasks {
        commands.push(Command::AssignResource { task_uid: uid, resource_uid: 1 + uid % 10, units: 1.0 });
    }
    for command in commands {
        doc.apply(command).expect("command applies");
    }
    println!("{tasks} công việc, {} khoảng quá tải", doc.project.overallocations.len());
    timed("cân bằng nguồn lực", || doc.apply(Command::LevelResources).expect("level"));
    assert!(doc.project.overallocations.is_empty(), "{:?}", doc.project.overallocations.first());
    // No gaps would finish when the busiest person's work runs out.
    let busiest = (1..=10)
        .map(|person| (1..=tasks).filter(|uid| 1 + uid % 10 == person).map(|uid| 1 + uid % 3).sum::<u32>())
        .max()
        .unwrap_or(0);
    let finish = doc.project.tasks.iter().map(|task| task.finish).max().unwrap_or(0);
    let calendar = doc.project.calendar().clone();
    let days = calendar.working_between(doc.project.start, finish) as f64 / DAY as f64;
    println!("{:<28} {:>10.1} ngày (tối thiểu {busiest})", "độ dài sau cân bằng", days);
}

/// A backlog of `tasks` effort-only tasks spread over `people`, the way a sprint plan
/// arrives: no links, every task free to start at the project start, priority deciding
/// the order. Prints the leveling time and the idle hours left between a person's tasks.
fn capacity(people: u32, tasks: u32, by_field: bool, leave: bool) {
    // A fixed linear congruential generator, so every run builds the same backlog.
    let mut seed: u64 = 42;
    let mut random = |below: u64| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (seed >> 33) % below
    };
    let mut project = Project::new("Năng lực", timestamp(2026, 1, 5, 8, 0));
    let monday = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
    for uid in 1..=people {
        let mut resource = Resource::new(uid, format!("Người {uid}"), ResourceKind::Person);
        if leave {
            let mut day = random(10) as i64;
            for _ in 0..random(4) {
                let length = if random(2) == 0 { 4 } else { 11 };
                let from = monday + chrono::Duration::days(day);
                let to = from + chrono::Duration::days(length);
                let units = if random(2) == 0 { 0.0 } else { 0.5 };
                resource.availability.push(AvailabilityPeriod { from: Some(from), to: Some(to), units });
                day += length + 1 + random(14) as i64;
            }
        }
        project.resources.push(resource);
    }
    for uid in 1..=tasks {
        let work = 60 * (1 + random(16) as i64);
        let mut task = Task::new(uid, format!("Việc {uid}"));
        task.task_type = TaskType::FixedWork;
        task.duration = work;
        task.remaining_duration = work;
        task.work = work;
        task.remaining_work = work;
        task.priority = 100 + random(800) as i32;
        task.start = project.start;
        if by_field && random(10) > 0 {
            let option = ["Gấp", "Cao", "Vừa", "Thấp"][random(4) as usize];
            task.values.insert(1, FieldValue::Text(option.into()));
        }
        project.tasks.push(task);
        project.assignments.push(Assignment {
            uid,
            task_uid: uid,
            resource_uid: 1 + random(u64::from(people)) as u32,
            units: 1.0,
            work,
            overtime_work: 0,
            actual_work: 0,
            remaining_work: work,
            cost: 0.0,
            actual_cost: 0.0,
            manual_actual_cost: None,
        });
    }
    project.next_task_uid = tasks + 1;
    project.next_resource_uid = people + 1;
    project.next_assignment_uid = tasks + 1;
    if by_field {
        let options = ["Gấp", "Cao", "Vừa", "Thấp"].iter().map(|o| o.to_string()).collect();
        let kind = FieldKind::Choice { options };
        project.fields.push(FieldDef { id: 1, name: "Mức".into(), kind });
        project.next_field_id = 2;
        project.leveling_order = vec![
            SortKey { criterion: Criterion::Field { id: 1 }, descending: false },
            SortKey { criterion: Criterion::Priority, descending: true },
            SortKey { criterion: Criterion::Start, descending: false },
        ];
    }

    timed("lập lịch", || schedule(&mut project).expect("schedule"));
    println!("{people} người, {tasks} công việc, {} khoảng quá tải", project.overallocations.len());
    timed("cân bằng nguồn lực", || level_project(&mut project).expect("level"));
    assert!(project.overallocations.is_empty(), "{:?}", project.overallocations.first());

    // Working time from the project start to a person's last finish, less their work:
    // zero when leveling packs each person's tasks back to back (with `leave`, the days
    // off and the half days count too).
    let calendar = project.calendar().clone();
    let idle: i64 = (1..=people)
        .filter_map(|person| {
            let mine: Vec<&Task> = project
                .assignments
                .iter()
                .filter(|assignment| assignment.resource_uid == person)
                .filter_map(|assignment| project.task(assignment.task_uid))
                .collect();
            let finish = mine.iter().map(|task| task.finish).max()?;
            let work: i64 = mine.iter().map(|task| task.work).sum();
            Some(calendar.working_between(project.start, finish) - work)
        })
        .sum();
    println!("{:<28} {:>10.1} giờ", "giờ rảnh xen giữa", idle as f64 / 60.0);
}

/// An unassigned backlog of `tasks` tasks, each with one of twenty values of a list field
/// that a rule maps to one of twenty skills; `people` have one to three skills each.
/// Prints how long the suggestions take and how evenly the top picks spread.
fn suggestions(people: u32, tasks: u32) {
    let mut seed: u64 = 7;
    let mut random = |below: u64| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (seed >> 33) % below
    };
    const SKILLS: u32 = 20;
    let mut project = Project::new("Gợi ý", timestamp(2026, 1, 5, 8, 0));
    project.skills = (1..=SKILLS).map(|id| SkillDef { id, name: format!("Kỹ năng {id}") }).collect();
    project.next_skill_id = SKILLS + 1;
    for uid in 1..=people {
        let mut resource = Resource::new(uid, format!("Người {uid}"), ResourceKind::Person);
        for _ in 0..=random(3) {
            resource.skills.push(1 + random(u64::from(SKILLS)) as u32);
        }
        resource.skills.sort_unstable();
        resource.skills.dedup();
        project.resources.push(resource);
    }
    let options: Vec<String> = (1..=SKILLS).map(|value| format!("Mảng {value}")).collect();
    project.fields.push(FieldDef { id: 1, name: "Mảng".into(), kind: FieldKind::Choice { options: options.clone() } });
    project.next_field_id = 2;
    project.assignment_rules = (1..=SKILLS)
        .map(|id| AssignmentRule {
            conditions: vec![Condition { field: 1, values: vec![FieldValue::Text(options[id as usize - 1].clone())] }],
            target: RuleTarget::Skill { id },
            units: 1.0,
        })
        .collect();
    for uid in 1..=tasks {
        let work = 60 * (1 + random(16) as i64);
        let mut task = Task::new(uid, format!("Việc {uid}"));
        task.task_type = TaskType::FixedWork;
        task.duration = work;
        task.remaining_duration = work;
        task.work = work;
        task.remaining_work = work;
        task.priority = 100 + random(800) as i32;
        let option = options[random(u64::from(SKILLS)) as usize].clone();
        task.values.insert(1, FieldValue::Text(option));
        project.tasks.push(task);
    }
    project.next_task_uid = tasks + 1;
    project.next_resource_uid = people + 1;
    timed("lập lịch", || schedule(&mut project).expect("schedule"));
    println!("{people} người, {tasks} công việc, {SKILLS} kỹ năng");
    let (suggested, _) = timed("gợi ý giao việc", || suggest(&project));
    let picked: Vec<u32> = suggested.iter().filter_map(|s| s.picks.first().map(|pick| pick.resource_uid)).collect();
    let mut per_person = vec![0u32; people as usize + 1];
    for uid in &picked {
        per_person[*uid as usize] += 1;
    }
    let busiest = per_person.iter().max().copied().unwrap_or(0);
    let finish = suggested.iter().filter_map(|s| s.picks.first().map(|pick| pick.finish)).max().unwrap_or(0);
    let days = project.calendar().working_between(project.start, finish) as f64 / DAY as f64;
    println!("{:<28} {:>10} / {tasks}", "có gợi ý", picked.len());
    println!("{:<28} {:>10} việc", "nhiều nhất một người", busiest);
    println!("{:<28} {:>10.1} ngày", "xong muộn nhất (ước tính)", days);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let numbers: Vec<usize> = args.iter().filter_map(|arg| arg.parse().ok()).collect();
    if args.iter().any(|arg| arg == "leveling") {
        leveling(500);
        return;
    }
    if args.iter().any(|arg| arg == "suggest") {
        let people = numbers.first().copied().unwrap_or(500).max(1) as u32;
        let tasks = numbers.get(1).copied().unwrap_or(5000) as u32;
        suggestions(people, tasks);
        return;
    }
    if args.iter().any(|arg| arg == "capacity") {
        let people = numbers.first().copied().unwrap_or(100).max(1) as u32;
        let tasks = numbers.get(1).copied().unwrap_or(1000) as u32;
        let has = |name: &str| args.iter().any(|arg| arg == name);
        capacity(people, tasks, has("field"), has("leave"));
        return;
    }
    let args = numbers;
    let tasks = args.first().copied().unwrap_or(1000) as u32;
    let total = args.get(1).copied().unwrap_or(10_000);
    let commands = build_commands(tasks, total);
    println!("{tasks} công việc, {} lệnh", commands.len());

    let mut doc = Document::from_project(Project::new("Bench", timestamp(2026, 1, 5, 8, 0)));
    let started = Instant::now();
    let mut slowest = Duration::ZERO;
    for command in commands {
        let one = Instant::now();
        doc.apply(command).expect("command applies");
        slowest = slowest.max(one.elapsed());
    }
    println!(
        "{:<28} {:>10.1} ms",
        "apply (tổng)",
        started.elapsed().as_secs_f64() * 1000.0
    );
    println!(
        "{:<28} {:>10.1} ms",
        "apply (chậm nhất)",
        slowest.as_secs_f64() * 1000.0
    );

    let json = serde_json::to_string(&doc.project).expect("json");
    println!(
        "{:<28} {:>10.1} KB",
        "Project JSON",
        json.len() as f64 / 1024.0
    );
    timed("undo", || doc.undo().expect("undo"));
    timed("undo lần 2", || doc.undo().expect("undo"));
    timed("redo", || doc.redo().expect("redo"));
    timed("apply sau undo", || {
        doc.apply(Command::RenameTask {
            uid: 2,
            name: "Sau undo".into(),
        })
        .expect("apply")
    });

    let dir = std::env::temp_dir().join(format!("sheafly-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("bench.pdproj").to_string_lossy().to_string();
    timed("save", || save(&doc, &path).expect("save"));
    let (mut loaded, _) = timed("load", || load(&path).expect("load"));
    assert_eq!(loaded.project.tasks.len(), doc.project.tasks.len());
    let mut slowest = Duration::ZERO;
    for _ in 0..50 {
        let one = Instant::now();
        loaded.undo().expect("undo");
        slowest = slowest.max(one.elapsed());
    }
    println!(
        "{:<28} {:>10.1} ms",
        "undo sau khi mở (chậm nhất/50)",
        slowest.as_secs_f64() * 1000.0
    );
    let _ = std::fs::remove_dir_all(&dir);
}
