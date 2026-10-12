//! Scheduling engine for ProjectDesk.
//!
//! Instants are Unix milliseconds holding project wall-clock time (formatted as UTC).
//! Durations and lags are working minutes. Calendars convert between the two.
//! The engine is the source of truth: the UI sends commands and renders the result.

pub mod calendar;
pub mod checkpoint;
pub mod command;
pub mod document;
pub mod error;
pub mod evm;
mod graph;
pub mod level;
pub mod model;
pub mod mspdi;
pub mod network;
pub mod recovery;
pub mod recurrence;
pub mod schedule;
pub mod split;
pub mod store;
pub mod timeutil;
pub mod work;

pub use calendar::Calendar;
pub use command::Command;
pub use document::{Document, Snapshot};
pub use error::EngineError;
pub use model::{
    Accrual, Assignment, ConstraintType, DepKind, Dependency, Overallocation, Project, Resource,
    ResourceKind, Task, TaskType, UsageDay, UsageRow,
};
pub use schedule::schedule;
