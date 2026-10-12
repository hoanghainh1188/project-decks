//! Timing for large projects: apply, undo, redo, save, and load.
//!
//! `cargo bench -p projectdesk-engine` builds a project of grouped tasks with chained
//! and summary-to-summary links, then pads the log with edits. Pass `-- <tasks> <commands>`
//! to change the size (default 1000 tasks, 10000 commands).

use std::time::{Duration, Instant};

use projectdesk_engine::command::Command;
use projectdesk_engine::model::{DepKind, ResourceKind};
use projectdesk_engine::store::{load, save};
use projectdesk_engine::timeutil::timestamp;
use projectdesk_engine::{Document, Project};

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

fn main() {
    if std::env::args().any(|arg| arg == "leveling") {
        leveling(500);
        return;
    }
    let args: Vec<usize> = std::env::args()
        .skip(1)
        .filter_map(|arg| arg.parse().ok())
        .collect();
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

    let dir = std::env::temp_dir().join(format!("projectdesk-bench-{}", std::process::id()));
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
