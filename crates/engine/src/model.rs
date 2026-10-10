use serde::{Deserialize, Serialize};

use crate::calendar::Calendar;
use crate::timeutil::MINUTES_PER_DAY;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintType {
    Asap,
    Alap,
    Snet,
    Snlt,
    Fnet,
    Fnlt,
    Mso,
    Mfo,
}

impl Default for ConstraintType {
    fn default() -> Self {
        Self::Asap
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepKind {
    Fs,
    Ss,
    Ff,
    Sf,
}

impl DepKind {
    pub fn parse(text: &str) -> Option<Self> {
        match text.to_ascii_uppercase().as_str() {
            "FS" => Some(Self::Fs),
            "SS" => Some(Self::Ss),
            "FF" => Some(Self::Ff),
            "SF" => Some(Self::Sf),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fs => "FS",
            Self::Ss => "SS",
            Self::Ff => "FF",
            Self::Sf => "SF",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    FixedUnits,
    FixedDuration,
    FixedWork,
}

impl Default for TaskType {
    fn default() -> Self {
        Self::FixedUnits
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accrual {
    Start,
    Prorated,
    End,
}

impl Default for Accrual {
    fn default() -> Self {
        Self::Prorated
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Person,
    Machine,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    pub number: u8,
    pub start: i64,
    pub finish: i64,
    pub work: i64,
    pub cost: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub uid: u32,
    pub id: u32,
    pub name: String,
    pub wbs: String,
    pub outline_level: u32,
    pub parent_uid: Option<u32>,
    pub notes: String,
    pub priority: i32,
    pub milestone: bool,
    pub summary: bool,
    pub manual: bool,
    pub duration: i64,
    pub constraint_type: ConstraintType,
    pub constraint_date: Option<i64>,
    pub calendar_id: Option<u32>,
    pub task_type: TaskType,
    pub effort_driven: bool,
    pub percent_complete: f64,
    pub actual_start: Option<i64>,
    pub actual_finish: Option<i64>,
    pub remaining_duration: i64,
    pub remaining_work: i64,
    pub fixed_cost: f64,
    pub fixed_cost_accrual: Accrual,
    pub leveling_delay: i64,
    pub start: i64,
    pub finish: i64,
    pub early_start: i64,
    pub early_finish: i64,
    pub late_start: i64,
    pub late_finish: i64,
    pub total_slack: i64,
    pub free_slack: i64,
    pub critical: bool,
    pub units: f64,
    pub work: i64,
    pub actual_work: i64,
    pub cost: f64,
    pub actual_cost: f64,
    pub remaining_cost: f64,
    pub conflict: bool,
    pub predecessors: String,
    pub baselines: Vec<Baseline>,
}

impl Task {
    pub fn new(uid: u32, name: impl Into<String>) -> Self {
        Self {
            uid,
            id: 0,
            name: name.into(),
            wbs: String::new(),
            outline_level: 1,
            parent_uid: None,
            notes: String::new(),
            priority: 500,
            milestone: false,
            summary: false,
            manual: false,
            duration: MINUTES_PER_DAY,
            constraint_type: ConstraintType::Asap,
            constraint_date: None,
            calendar_id: None,
            task_type: TaskType::FixedUnits,
            effort_driven: true,
            percent_complete: 0.0,
            actual_start: None,
            actual_finish: None,
            remaining_duration: MINUTES_PER_DAY,
            remaining_work: MINUTES_PER_DAY,
            fixed_cost: 0.0,
            fixed_cost_accrual: Accrual::Prorated,
            leveling_delay: 0,
            start: 0,
            finish: 0,
            early_start: 0,
            early_finish: 0,
            late_start: 0,
            late_finish: 0,
            total_slack: 0,
            free_slack: 0,
            critical: false,
            units: 1.0,
            work: MINUTES_PER_DAY,
            actual_work: 0,
            cost: 0.0,
            actual_cost: 0.0,
            remaining_cost: 0.0,
            conflict: false,
            predecessors: String::new(),
            baselines: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dependency {
    pub uid: u32,
    pub predecessor_uid: u32,
    pub successor_uid: u32,
    pub kind: DepKind,
    pub lag: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Resource {
    pub uid: u32,
    pub name: String,
    pub kind: ResourceKind,
    pub max_units: f64,
    pub calendar_id: Option<u32>,
    pub standard_rate: f64,
    pub overtime_rate: f64,
    pub cost_per_use: f64,
}

impl Resource {
    pub fn new(uid: u32, name: impl Into<String>, kind: ResourceKind) -> Self {
        Self {
            uid,
            name: name.into(),
            kind,
            max_units: 1.0,
            calendar_id: None,
            standard_rate: 0.0,
            overtime_rate: 0.0,
            cost_per_use: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assignment {
    pub uid: u32,
    pub task_uid: u32,
    pub resource_uid: u32,
    pub units: f64,
    pub work: i64,
    pub overtime_work: i64,
    pub actual_work: i64,
    pub remaining_work: i64,
    pub cost: f64,
    pub actual_cost: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Overallocation {
    pub resource_uid: u32,
    pub date: String,
    pub units: f64,
    pub max_units: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UsageDay {
    pub date: String,
    pub hours: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UsageRow {
    pub task_uid: u32,
    pub assignment_uid: Option<u32>,
    pub resource_uid: Option<u32>,
    pub label: String,
    pub days: Vec<UsageDay>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub start: i64,
    pub status_date: Option<i64>,
    pub calendar_id: u32,
    pub calendars: Vec<Calendar>,
    pub tasks: Vec<Task>,
    pub dependencies: Vec<Dependency>,
    pub resources: Vec<Resource>,
    pub assignments: Vec<Assignment>,
    pub next_task_uid: u32,
    pub next_dependency_uid: u32,
    pub next_resource_uid: u32,
    pub next_assignment_uid: u32,
    pub next_calendar_uid: u32,
    pub overallocations: Vec<Overallocation>,
    pub usage: Vec<UsageRow>,
}

impl Project {
    pub fn new(name: impl Into<String>, start: i64) -> Self {
        let calendar = Calendar::standard();
        let calendar_id = calendar.id;
        let start = calendar.next_work_start(start);
        Self {
            name: name.into(),
            start,
            status_date: None,
            calendar_id,
            calendars: vec![calendar],
            tasks: Vec::new(),
            dependencies: Vec::new(),
            resources: Vec::new(),
            assignments: Vec::new(),
            next_task_uid: 1,
            next_dependency_uid: 1,
            next_resource_uid: 1,
            next_assignment_uid: 1,
            next_calendar_uid: 2,
            overallocations: Vec::new(),
            usage: Vec::new(),
        }
    }

    pub fn calendar(&self) -> &Calendar {
        self.calendars
            .iter()
            .find(|c| c.id == self.calendar_id)
            .or_else(|| self.calendars.first())
            .expect("project has a calendar")
    }

    pub fn calendar_mut(&mut self) -> &mut Calendar {
        let id = self.calendar_id;
        if let Some(index) = self.calendars.iter().position(|c| c.id == id) {
            return &mut self.calendars[index];
        }
        &mut self.calendars[0]
    }

    pub fn calendar_by_id(&self, id: u32) -> Option<&Calendar> {
        self.calendars.iter().find(|c| c.id == id)
    }

    pub fn calendar_mut_by_id(&mut self, id: u32) -> Option<&mut Calendar> {
        self.calendars.iter_mut().find(|calendar| calendar.id == id)
    }

    pub fn task_calendar(&self, task: &Task) -> &Calendar {
        task.calendar_id
            .and_then(|id| self.calendar_by_id(id))
            .unwrap_or_else(|| self.calendar())
    }

    pub fn resource_calendar(&self, resource: &Resource) -> &Calendar {
        resource
            .calendar_id
            .and_then(|id| self.calendar_by_id(id))
            .unwrap_or_else(|| self.calendar())
    }

    pub fn index_of(&self, uid: u32) -> Option<usize> {
        self.tasks.iter().position(|t| t.uid == uid)
    }

    pub fn task(&self, uid: u32) -> Option<&Task> {
        self.tasks.iter().find(|t| t.uid == uid)
    }

    pub fn task_mut(&mut self, uid: u32) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.uid == uid)
    }

    pub fn alloc_task_uid(&mut self) -> u32 {
        let uid = self.next_task_uid;
        self.next_task_uid += 1;
        uid
    }

    pub fn alloc_dependency_uid(&mut self) -> u32 {
        let uid = self.next_dependency_uid;
        self.next_dependency_uid += 1;
        uid
    }

    pub fn alloc_resource_uid(&mut self) -> u32 {
        let uid = self.next_resource_uid;
        self.next_resource_uid += 1;
        uid
    }

    pub fn alloc_assignment_uid(&mut self) -> u32 {
        let uid = self.next_assignment_uid;
        self.next_assignment_uid += 1;
        uid
    }

    pub fn alloc_calendar_uid(&mut self) -> u32 {
        let uid = self.next_calendar_uid;
        self.next_calendar_uid += 1;
        uid
    }
}
