use serde::{Deserialize, Serialize};

use crate::command::{self, Command};
use crate::error::EngineError;
use crate::evm::{self, BurndownSnapshot, ProjectMetrics};
use crate::model::{Project, Task};
use crate::schedule::{mark_late, schedule};
use crate::timeutil::{civil, compose};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub project: Project,
    pub path: Option<String>,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Clone, Debug)]
pub struct Document {
    pub base: Project,
    pub project: Project,
    pub commands: Vec<Command>,
    pub cursor: usize,
    pub path: Option<String>,
    pub dirty: bool,
    /// Burn-down samples, oldest first. Kept outside the command log so undo does not drop them.
    pub burndown: Vec<BurndownSnapshot>,
}

impl Document {
    pub fn new_default() -> Self {
        Self::from_project(Project::new("Dự án mới", default_start(now_millis())))
    }

    pub fn from_project(mut project: Project) -> Self {
        if project.tasks.is_empty() {
            let uid = project.alloc_task_uid();
            let mut task = Task::new(uid, "Công việc 1");
            task.start = project.start;
            project.tasks.push(task);
        }
        schedule(&mut project).ok();
        Self {
            base: project.clone(),
            project,
            commands: Vec::new(),
            cursor: 0,
            path: None,
            dirty: false,
            burndown: Vec::new(),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            project: self.project.clone(),
            path: self.path.clone(),
            dirty: self.dirty,
            can_undo: self.cursor > 0,
            can_redo: self.cursor < self.commands.len(),
        }
    }

    pub fn apply(&mut self, command: Command) -> Result<Snapshot, EngineError> {
        if self.cursor < self.commands.len() {
            self.commands.truncate(self.cursor);
        }
        let mut trial = self.project.clone();
        command::apply_one(&mut trial, &command)?;
        schedule(&mut trial)?;
        let status_changed = trial.status_date != self.project.status_date;
        self.commands.push(command);
        self.cursor = self.commands.len();
        self.project = trial;
        self.dirty = true;
        if let Some(status) = self.project.status_date.filter(|_| status_changed) {
            self.record_burndown(status);
        }
        Ok(self.snapshot())
    }

    /// Sample burn-down for the status date day, or for `now` when there is no status date.
    pub fn record_burndown(&mut self, now: i64) {
        let date = self.project.status_date.unwrap_or(now);
        let snapshot = evm::take_snapshot(&self.project, date);
        evm::record_snapshot(&mut self.burndown, snapshot);
    }

    pub fn metrics(&self) -> ProjectMetrics {
        evm::metrics(&self.project, &self.burndown)
    }

    pub fn undo(&mut self) -> Result<Snapshot, EngineError> {
        if self.cursor == 0 {
            return Ok(self.snapshot());
        }
        self.cursor -= 1;
        self.project = replay(&self.base, &self.commands[..self.cursor])?;
        self.dirty = true;
        Ok(self.snapshot())
    }

    pub fn redo(&mut self) -> Result<Snapshot, EngineError> {
        if self.cursor >= self.commands.len() {
            return Ok(self.snapshot());
        }
        self.cursor += 1;
        self.project = replay(&self.base, &self.commands[..self.cursor])?;
        self.dirty = true;
        Ok(self.snapshot())
    }

    pub fn replace_project(&mut self, project: Project, path: Option<String>) -> Result<Snapshot, EngineError> {
        let mut project = project;
        schedule(&mut project)?;
        self.base = project.clone();
        self.project = project;
        self.commands.clear();
        self.cursor = 0;
        self.path = path;
        self.dirty = false;
        self.burndown.clear();
        Ok(self.snapshot())
    }
}

pub fn replay(base: &Project, commands: &[Command]) -> Result<Project, EngineError> {
    let mut project = base.clone();
    for command in commands {
        command::apply_one(&mut project, command)?;
        schedule(&mut project)?;
    }
    // A saved base with no replayed commands still needs the derived late flag.
    if commands.is_empty() {
        mark_late(&mut project);
    }
    Ok(project)
}

pub fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// 08:00 on the current working day, or the next working morning.
pub fn default_start(now: i64) -> i64 {
    let (date, _minute) = civil(now);
    let morning = compose(date, 8 * 60);
    let calendar = crate::calendar::Calendar::standard();
    if calendar.ranges_on(date).is_empty() {
        calendar.next_work_start(morning)
    } else {
        morning
    }
}
