//! What opening a file found besides the project: damage in the command log, and tasks
//! whose dates differ from what the engine that saved the file computed.
//!
//! A damaged log never stops a file from opening. The app opens an unsaved copy at the
//! last state it can rebuild and leaves the file itself alone.

use chrono::Datelike;

use crate::model::Project;
use crate::timeutil::civil;

/// How many moved tasks the message names before summing up the rest.
const MOVED_LISTED: usize = 10;

#[derive(Clone, Debug, PartialEq)]
pub enum Damage {
    /// The stored command does not parse.
    Unreadable,
    /// The command is one this build does not know, likely from a newer app.
    Newer,
    /// The command parses but fails to apply.
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Recovery {
    pub damage: Damage,
    /// 0-based position of the first command that could not be used.
    pub at: usize,
    /// Commands in the file's log, including the damaged ones.
    pub total: usize,
    /// Commands the file had applied when it was saved (its cursor).
    pub saved_cursor: usize,
    /// The copy shows the state after this many commands.
    pub shown_after: usize,
    /// Whether undo still reaches back through the commands before the damage.
    pub history_kept: bool,
    /// Whether the app opened an unsaved copy. Damage only in the redo commands after
    /// the cursor leaves the saved state whole, so the file itself opens.
    pub copy: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MovedTask {
    pub uid: u32,
    pub name: String,
    pub before: (i64, i64),
    pub after: (i64, i64),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpenReport {
    pub recovery: Option<Recovery>,
    /// Tasks this engine schedules on other dates than the engine that saved the file.
    pub moved: Vec<MovedTask>,
}

impl OpenReport {
    /// The warning to show after opening, if there is anything to say.
    pub fn message(&self) -> Option<String> {
        let parts: Vec<String> = self
            .recovery
            .iter()
            .map(recovery_message)
            .chain((!self.moved.is_empty()).then(|| moved_message(&self.moved)))
            .collect();
        (!parts.is_empty()).then(|| parts.join("\n\n"))
    }
}

/// Leaf tasks whose start or finish differs between `before` and `after`, in outline order.
pub fn moved_tasks(before: &Project, after: &Project) -> Vec<MovedTask> {
    after
        .tasks
        .iter()
        .filter(|task| !task.summary)
        .filter_map(|task| {
            let old = before.task(task.uid)?;
            let (old_dates, new_dates) = ((old.start, old.finish), (task.start, task.finish));
            (old_dates != new_dates).then(|| MovedTask {
                uid: task.uid,
                name: task.name.clone(),
                before: old_dates,
                after: new_dates,
            })
        })
        .collect()
}

fn recovery_message(recovery: &Recovery) -> String {
    let position = format!("thao tác {}/{}", recovery.at + 1, recovery.total);
    let cause = match &recovery.damage {
        Damage::Unreadable => format!("Tệp bị hỏng ở {position}."),
        Damage::Newer => {
            format!("Tệp có {position} do phiên bản mới hơn của ProjectDesk tạo ra.")
        }
        Damage::Failed(error) => format!("Không áp dụng lại được {position}: {error}."),
    };
    if !recovery.copy {
        return format!(
            "{cause} Trạng thái đã lưu mở đầy đủ; các thao tác làm lại từ thao tác {} trở đi \
             không dùng được và sẽ bị bỏ khi lưu.",
            recovery.at + 1
        );
    }
    let state = if recovery.shown_after == recovery.saved_cursor {
        "Đã mở một bản sao đúng ở trạng thái đã lưu.".to_string()
    } else {
        format!(
            "Đã mở một bản sao ở trạng thái sau thao tác {} (lúc lưu là {}).",
            recovery.shown_after, recovery.saved_cursor
        )
    };
    let history = if recovery.history_kept {
        ""
    } else {
        " Bản sao không có lịch sử hoàn tác."
    };
    format!("{cause} {state}{history} Tệp gốc giữ nguyên; hãy lưu bản sao thành tệp mới.")
}

fn moved_message(moved: &[MovedTask]) -> String {
    let mut lines = vec![format!(
        "Phiên bản ProjectDesk này tính lịch khác phiên bản đã lưu tệp: {} công việc đổi ngày. \
         Hãy kiểm tra trước khi lưu.",
        moved.len()
    )];
    lines.extend(moved.iter().take(MOVED_LISTED).map(|task| {
        format!(
            "• {}: {} → {}",
            task.name,
            span(task.before),
            span(task.after)
        )
    }));
    if moved.len() > MOVED_LISTED {
        lines.push(format!(
            "… và {} công việc khác.",
            moved.len() - MOVED_LISTED
        ));
    }
    lines.join("\n")
}

fn span((start, finish): (i64, i64)) -> String {
    format!("{}–{}", day(start), day(finish))
}

fn day(ms: i64) -> String {
    let (date, _) = civil(ms);
    format!("{:02}/{:02}/{}", date.day(), date.month(), date.year())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeutil::timestamp;

    fn moved(count: usize) -> Vec<MovedTask> {
        (0..count)
            .map(|index| MovedTask {
                uid: index as u32 + 1,
                name: format!("Việc {}", index + 1),
                before: (timestamp(2026, 1, 5, 8, 0), timestamp(2026, 1, 6, 17, 0)),
                after: (timestamp(2026, 1, 7, 8, 0), timestamp(2026, 1, 8, 17, 0)),
            })
            .collect()
    }

    #[test]
    fn a_clean_open_has_no_message() {
        assert_eq!(OpenReport::default().message(), None);
    }

    #[test]
    fn moved_tasks_are_listed_with_old_and_new_dates() {
        let report = OpenReport {
            recovery: None,
            moved: moved(12),
        };
        let message = report.message().unwrap();
        assert!(message.contains("12 công việc đổi ngày"), "{message}");
        assert!(
            message.contains("• Việc 1: 05/01/2026–06/01/2026 → 07/01/2026–08/01/2026"),
            "{message}"
        );
        assert!(message.contains("Việc 10"), "{message}");
        assert!(!message.contains("Việc 11:"), "{message}");
        assert!(message.contains("… và 2 công việc khác."), "{message}");
    }

    #[test]
    fn recovery_explains_the_cause_and_the_copy() {
        let recovery = Recovery {
            damage: Damage::Newer,
            at: 4,
            total: 9,
            saved_cursor: 9,
            shown_after: 4,
            history_kept: true,
            copy: true,
        };
        let message = OpenReport {
            recovery: Some(recovery),
            moved: Vec::new(),
        }
        .message()
        .unwrap();
        assert!(
            message.contains("thao tác 5/9 do phiên bản mới hơn"),
            "{message}"
        );
        assert!(
            message.contains("sau thao tác 4 (lúc lưu là 9)"),
            "{message}"
        );
        assert!(message.contains("Tệp gốc giữ nguyên"), "{message}");
        assert!(!message.contains("không có lịch sử"), "{message}");
    }
}
