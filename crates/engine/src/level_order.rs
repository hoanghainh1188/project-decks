//! The order in which leveling places tasks: a list of criteria the project keeps, each
//! ascending or descending, like Microsoft Project's leveling order but with any number
//! of keys and custom fields among them.
//!
//! Two things always come before the list: tasks leveling cannot move, and tasks with a
//! leaf predecessor still to place. The task's uid breaks every tie after it. An empty
//! list is the order leveling has always had (start, higher priority, less slack), so
//! files written before the list existed level exactly as they did.

use std::cmp::Ordering;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::error::{EngineError, Invalid};
use crate::fields::{FieldKind, FieldValue};
use crate::model::{Project, Task};

/// Most criteria one order can have.
pub const MAX_SORT_KEYS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortKey {
    pub criterion: Criterion,
    #[serde(default)]
    pub descending: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Criterion {
    /// The current start, which moves as leveling delays the task's predecessors.
    Start,
    Priority,
    /// Total slack before leveling.
    Slack,
    Deadline,
    Duration,
    /// Position in the outline (Project's "ID Only").
    Id,
    /// A custom field: numbers by value, choices by their list order, text by code point.
    Field { id: u32 },
}

/// The order an empty list stands for.
pub fn default_order() -> Vec<SortKey> {
    vec![
        SortKey { criterion: Criterion::Start, descending: false },
        SortKey { criterion: Criterion::Priority, descending: true },
        SortKey { criterion: Criterion::Slack, descending: false },
    ]
}

/// Replace the project's leveling order. Every field must exist, no criterion may come
/// twice (whatever its direction), and there are at most `MAX_SORT_KEYS`. Dates do not
/// change until the project is leveled again.
pub(crate) fn set_order(project: &mut Project, order: &[SortKey]) -> Result<(), EngineError> {
    if order.len() > MAX_SORT_KEYS {
        return Err(Invalid::TooManySortKeys.into());
    }
    for (index, key) in order.iter().enumerate() {
        if let Criterion::Field { id } = key.criterion {
            if !project.fields.iter().any(|field| field.id == id) {
                return Err(Invalid::NoSuchField.into());
            }
        }
        if order[..index].iter().any(|earlier| earlier.criterion == key.criterion) {
            return Err(Invalid::DuplicateSortKey.into());
        }
    }
    project.leveling_order = order.to_vec();
    Ok(())
}

/// The project's order, or the default one when its list is empty.
pub(crate) fn effective(project: &Project) -> Vec<SortKey> {
    if project.leveling_order.is_empty() {
        default_order()
    } else {
        project.leveling_order.clone()
    }
}

/// Forget the criterion of a deleted field.
pub(crate) fn drop_field(project: &mut Project, id: u32) {
    project
        .leveling_order
        .retain(|key| key.criterion != Criterion::Field { id });
}

/// One criterion's value for one task.
#[derive(Clone, Debug)]
enum Value {
    /// The key's own start, read when it is built: the one part that changes.
    Start,
    Int(i64),
    Num(f64),
    Text(String),
    Choice(usize),
    /// No value: always after every value, ascending or descending.
    Missing,
}

impl Value {
    /// Values of one criterion are all of one kind; this only keeps `Ord` total.
    fn rank(&self) -> u8 {
        match self {
            Value::Start | Value::Int(_) => 0,
            Value::Num(_) => 1,
            Value::Text(_) => 2,
            Value::Choice(_) => 3,
            Value::Missing => 4,
        }
    }
}

#[derive(Clone, Debug)]
struct Part {
    value: Value,
    descending: bool,
}

/// The parts of a task's key that stay put while leveling, in the order's order.
#[derive(Clone, Debug)]
pub(crate) struct Parts(Rc<[Part]>);

impl Parts {
    /// `slack` is the task's total slack before leveling.
    pub(crate) fn of(project: &Project, order: &[SortKey], task: &Task, slack: i64) -> Self {
        let parts: Vec<Part> = order
            .iter()
            .map(|key| Part {
                value: value_of(project, key.criterion, task, slack),
                descending: key.descending,
            })
            .collect();
        Self(parts.into())
    }

    /// Compare two tasks' parts, `start` standing in for each one's `Start`.
    pub(crate) fn cmp(&self, start: i64, other: &Self, other_start: i64) -> Ordering {
        for (mine, theirs) in self.0.iter().zip(other.0.iter()) {
            let ordering = match (&mine.value, &theirs.value) {
                (Value::Missing, Value::Missing) => Ordering::Equal,
                (Value::Missing, _) => Ordering::Greater,
                (_, Value::Missing) => Ordering::Less,
                (a, b) => {
                    let ordering = compare(a, start, b, other_start);
                    if mine.descending {
                        ordering.reverse()
                    } else {
                        ordering
                    }
                }
            };
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        Ordering::Equal
    }
}

fn compare(a: &Value, start: i64, b: &Value, other_start: i64) -> Ordering {
    match (a, b) {
        (Value::Start, Value::Start) => start.cmp(&other_start),
        (Value::Int(a), Value::Int(b)) => a.cmp(b),
        (Value::Num(a), Value::Num(b)) => a.total_cmp(b),
        // `str` compares bytes, and UTF-8 bytes sort as their code points.
        (Value::Text(a), Value::Text(b)) => a.cmp(b),
        (Value::Choice(a), Value::Choice(b)) => a.cmp(b),
        _ => a.rank().cmp(&b.rank()),
    }
}

fn value_of(project: &Project, criterion: Criterion, task: &Task, slack: i64) -> Value {
    match criterion {
        Criterion::Start => Value::Start,
        Criterion::Priority => Value::Int(i64::from(task.priority)),
        Criterion::Slack => Value::Int(slack),
        Criterion::Deadline => task.deadline.map_or(Value::Missing, Value::Int),
        Criterion::Duration => Value::Int(task.duration),
        Criterion::Id => Value::Int(i64::from(task.id)),
        Criterion::Field { id } => field_value(project, id, task),
    }
}

/// A custom field's value as its kind sorts it; a value that does not fit the field
/// (a choice the list lacks, say) counts as none.
fn field_value(project: &Project, id: u32, task: &Task) -> Value {
    let Some(field) = project.fields.iter().find(|field| field.id == id) else {
        return Value::Missing;
    };
    match (&field.kind, task.values.get(&id)) {
        // Adding 0 turns -0 into 0, so the two tie as they read.
        (FieldKind::Number, Some(FieldValue::Number(number))) => Value::Num(number + 0.0),
        (FieldKind::Text, Some(FieldValue::Text(text))) => Value::Text(text.clone()),
        (FieldKind::Choice { options }, Some(FieldValue::Text(text))) => options
            .iter()
            .position(|option| option == text)
            .map_or(Value::Missing, Value::Choice),
        _ => Value::Missing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minus_zero_ties_with_zero() {
        let mut project = Project::new("Số", 0);
        let field = crate::fields::FieldDef { id: 1, name: "Điểm".into(), kind: FieldKind::Number };
        project.fields.push(field);
        let order = [SortKey { criterion: Criterion::Field { id: 1 }, descending: false }];
        let parts: Vec<Parts> = [-0.0, 0.0]
            .iter()
            .map(|&number| {
                let mut task = Task::new(1, "Việc");
                task.values.insert(1, FieldValue::Number(number));
                Parts::of(&project, &order, &task, 0)
            })
            .collect();
        assert_eq!(parts[0].cmp(0, &parts[1], 0), Ordering::Equal);
    }
}
