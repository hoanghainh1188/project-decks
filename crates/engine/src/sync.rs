//! Three-way sync of a task with its issue (phase 19, phase 3).
//!
//! A pull used to make the source the authority: whatever it sent replaced the task, even
//! when the source had not changed since the last pull and the task had been edited in
//! Sheafly. A linked task now remembers, field by field, what the source and Sheafly each
//! held at the last sync (`SyncBase`), and a field moves only when the comparison says it
//! may:
//!
//! - the source did not change (`s = b_source`): leave the task, however it differs;
//! - only the source changed (`s ≠ b_source`, local `l = b_local`): take the source;
//! - both changed to the same value (`l = s`): only the base is updated;
//! - both changed to different values: a conflict, which the user resolves by keeping
//!   Sheafly or taking the source; nothing moves until then.
//!
//! A link with no base yet (made before this existed) records one from fields that agree;
//! where they disagree nobody knows who changed, so it is a conflict, except a start or due
//! day the source holds because a push wrote it (`pushed_start`, `pushed_due`).
//!
//! Values are compared as canonical text, so Sheafly's own forms (a "start no earlier than"
//! constraint, a deadline, a percent) compare with the source's (a start date, a due date,
//! a status).

use serde::{Deserialize, Serialize};

use crate::fields::FieldValue;
use crate::model::{ConstraintType, Project, Task};
use crate::source::{ExternalLink, StatusClass, SyncBase};
use crate::timeutil::{civil, date_key};

/// A field that syncs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncField {
    Name,
    Work,
    Start,
    Due,
    Status,
    Assignee,
    /// A custom field, by id.
    Field(u32),
}

impl SyncField {
    /// The key in a `SyncBase`, in a conflict and in a resolution.
    pub fn key(self) -> String {
        match self {
            Self::Name => "name".into(),
            Self::Work => "work".into(),
            Self::Start => "start".into(),
            Self::Due => "due".into(),
            Self::Status => "status".into(),
            Self::Assignee => "assignee".into(),
            Self::Field(id) => format!("field:{id}"),
        }
    }
}

/// Which side a resolution keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// Take what the source says.
    Source,
    /// Keep what the task has.
    Sheafly,
}

/// The user's choice for one field of one issue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    /// The issue's key.
    pub key: String,
    /// The field's key (`SyncField::key`).
    pub field: String,
    pub keep: Side,
}

/// What the comparison says to do with one field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Leave the task and the base as they are.
    Keep,
    /// Both sides already agree: record the base, change nothing. `real` is false when it
    /// only seeds a missing base, which alone is not worth a change.
    Agree { real: bool },
    /// Put the source's value on the task.
    Take,
    /// Both sides changed to different values.
    Conflict,
}

/// A day (`YYYY-MM-DD`) of an instant.
pub fn day_text(instant: i64) -> String {
    date_key(civil(instant).0)
}

pub fn status_text(class: StatusClass) -> &'static str {
    match class {
        StatusClass::NotStarted => "not_started",
        StatusClass::InProgress => "in_progress",
        StatusClass::Done => "done",
    }
}

/// The canonical text of a custom field value.
pub fn field_text(value: &FieldValue) -> String {
    match value {
        FieldValue::Text(text) => text.clone(),
        FieldValue::Number(number) => number.to_string(),
    }
}

/// What the task holds for `field`, in canonical text; `None` for no value.
pub fn local_value(project: &Project, task: &Task, field: SyncField) -> Option<String> {
    match field {
        SyncField::Name => Some(task.name.trim().to_string()),
        SyncField::Work => Some(task.work.to_string()),
        SyncField::Start => (task.constraint_type == ConstraintType::Snet)
            .then_some(task.constraint_date)
            .flatten()
            .map(day_text),
        SyncField::Due => task.deadline.map(day_text),
        SyncField::Status => Some(
            if task.percent_complete >= 100.0 {
                status_text(StatusClass::Done)
            } else if task.percent_complete > 0.0 {
                status_text(StatusClass::InProgress)
            } else {
                status_text(StatusClass::NotStarted)
            }
            .to_string(),
        ),
        SyncField::Assignee => {
            let people: Vec<u32> = project
                .assignments
                .iter()
                .filter(|assignment| assignment.task_uid == task.uid)
                .map(|assignment| assignment.resource_uid)
                .collect();
            match people.as_slice() {
                [] => None,
                [person] => Some(
                    project
                        .resources
                        .iter()
                        .find(|resource| resource.uid == *person)
                        .map(|resource| resource.email.trim().to_lowercase())
                        .filter(|email| !email.is_empty())
                        .unwrap_or_else(|| format!("#{person}")),
                ),
                _ => Some("*several".to_string()),
            }
        }
        SyncField::Field(id) => task.values.get(&id).map(field_text),
    }
}

/// Compare the three values of one field. `base` is the link's base (None before the first
/// sync under this scheme); `pushed` is the day a push wrote for a start or due field.
pub fn decide(
    base: Option<&SyncBase>,
    field: SyncField,
    source: &Option<String>,
    local: &Option<String>,
    pushed: Option<&str>,
) -> Verdict {
    let key = field.key();
    let Some(base) = base else {
        // No history: agreement seeds one; a day the source holds because we pushed it is
        // ours; anything else differs for a reason nobody can tell.
        return if source == local {
            Verdict::Agree { real: false }
        } else if pushed.is_some() && source.as_deref() == pushed {
            Verdict::Keep
        } else {
            Verdict::Conflict
        };
    };
    let was_source = base.source.get(&key).cloned();
    let was_local = base.local.get(&key).cloned();
    if *source == was_source {
        Verdict::Keep
    } else if local == source {
        Verdict::Agree { real: true }
    } else if *local == was_local {
        Verdict::Take
    } else {
        Verdict::Conflict
    }
}

/// The base of a link, made if it had none.
pub fn base_of(link: &mut ExternalLink) -> &mut SyncBase {
    link.base.get_or_insert_with(SyncBase::default)
}

/// Set (or clear, for `None`) one field of one side of a base.
pub fn set(side: &mut std::collections::BTreeMap<String, String>, key: &str, value: Option<String>) {
    match value {
        Some(value) => {
            side.insert(key.to_string(), value);
        }
        None => {
            side.remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(source: &[(&str, &str)], local: &[(&str, &str)]) -> SyncBase {
        let map = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        SyncBase { source: map(source), local: map(local) }
    }

    fn some(text: &str) -> Option<String> {
        Some(text.to_string())
    }

    #[test]
    fn the_source_unchanged_keeps_the_task_however_it_differs() {
        let b = base(&[("name", "A")], &[("name", "A")]);
        assert_eq!(decide(Some(&b), SyncField::Name, &some("A"), &some("B"), None), Verdict::Keep);
    }

    #[test]
    fn only_the_source_changed_takes_the_source() {
        let b = base(&[("name", "A")], &[("name", "A")]);
        assert_eq!(decide(Some(&b), SyncField::Name, &some("C"), &some("A"), None), Verdict::Take);
    }

    #[test]
    fn both_changed_to_the_same_value_agree() {
        let b = base(&[("name", "A")], &[("name", "A")]);
        assert_eq!(decide(Some(&b), SyncField::Name, &some("C"), &some("C"), None), Verdict::Agree { real: true });
    }

    #[test]
    fn both_changed_differently_is_a_conflict() {
        let b = base(&[("name", "A")], &[("name", "A")]);
        assert_eq!(decide(Some(&b), SyncField::Name, &some("C"), &some("B"), None), Verdict::Conflict);
    }

    #[test]
    fn a_missing_value_in_the_base_means_no_value() {
        let b = SyncBase::default();
        // The source gained a due day, the task has none: only the source changed.
        assert_eq!(decide(Some(&b), SyncField::Due, &some("2026-01-05"), &None, None), Verdict::Take);
        // The source's value went away, the task still has the old one: the task is the change.
        let b = base(&[("due", "2026-01-05")], &[("due", "2026-01-05")]);
        assert_eq!(decide(Some(&b), SyncField::Due, &None, &some("2026-01-05"), None), Verdict::Take);
    }

    #[test]
    fn without_a_base_agreement_seeds_one_and_difference_is_a_conflict() {
        assert_eq!(decide(None, SyncField::Name, &some("A"), &some("A"), None), Verdict::Agree { real: false });
        assert_eq!(decide(None, SyncField::Name, &some("A"), &some("B"), None), Verdict::Conflict);
        assert_eq!(decide(None, SyncField::Start, &some("2026-01-07"), &None, Some("2026-01-07")), Verdict::Keep);
        assert_eq!(decide(None, SyncField::Start, &some("2026-01-08"), &None, Some("2026-01-07")), Verdict::Conflict);
    }

    #[test]
    fn keys_are_stable() {
        assert_eq!(SyncField::Field(3).key(), "field:3");
        assert_eq!(SyncField::Status.key(), "status");
    }
}
