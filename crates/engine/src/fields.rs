//! Custom task fields: the project declares them, each task holds a value per field.
//!
//! A field is plain text, a number, or a choice from a list. Values are data the user
//! typed, so scheduling never reads them; they group and filter work, and leveling can
//! place tasks by them (`level_order.rs`). The limits follow what Microsoft Project can
//! hold for a task: 30 text fields (a choice is a text field with a value list) and 20
//! number fields, so every field survives MSPDI.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{EngineError, Invalid};
use crate::level_order;
use crate::model::Project;

/// Text and choice fields a project can have (Microsoft Project's Text1–Text30).
pub const MAX_TEXT_FIELDS: usize = 30;
/// Number fields a project can have (Number1–Number20).
pub const MAX_NUMBER_FIELDS: usize = 20;
/// Longest field name, as MSPDI's `Alias`.
pub const MAX_NAME_CHARS: usize = 50;
/// Longest text value or choice, as a Microsoft Project text field.
pub const MAX_TEXT_CHARS: usize = 255;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FieldDef {
    pub id: u32,
    pub name: String,
    pub kind: FieldKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Number,
    /// One of `options`, in the order the user listed them.
    Choice { options: Vec<String> },
}

impl FieldKind {
    /// Text and choice fields share Microsoft Project's text slots.
    fn uses_text_slot(&self) -> bool {
        !matches!(self, FieldKind::Number)
    }
}

/// A task's value for one field: a number for number fields, text for the others.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
    Number(f64),
    Text(String),
}

/// A choice renamed while editing a field, so tasks that had it keep it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionRename {
    pub from: String,
    pub to: String,
}

pub(crate) fn add_field(project: &mut Project, name: &str, kind: &FieldKind) -> Result<u32, EngineError> {
    let name = checked_name(project, None, name)?;
    let kind = checked_kind(kind)?;
    check_room(project, None, &kind)?;
    let id = alloc_field_id(project);
    project.fields.push(FieldDef { id, name, kind });
    Ok(id)
}

/// A field id no field has had: past `next_field_id` and every id in use.
pub(crate) fn alloc_field_id(project: &mut Project) -> u32 {
    let largest = project.fields.iter().map(|field| field.id).max().unwrap_or(0);
    let id = project.next_field_id.max(largest + 1);
    project.next_field_id = id + 1;
    id
}

/// Rename a field or change its kind or choices. Values are kept where they still fit:
/// a number field keeps text that reads as a number, a choice field keeps values that are
/// one of its choices, and the rest are cleared. `renamed` maps old choices to new ones
/// when both kinds are choice lists; each new name must be one of the new choices.
pub(crate) fn update_field(
    project: &mut Project,
    id: u32,
    name: &str,
    kind: &FieldKind,
    renamed: &[OptionRename],
) -> Result<(), EngineError> {
    let position = position_of(project, id)?;
    let name = checked_name(project, Some(id), name)?;
    let kind = checked_kind(kind)?;
    check_room(project, Some(id), &kind)?;
    let renames = checked_renames(&project.fields[position].kind, &kind, renamed)?;
    for task in &mut project.tasks {
        let Some(old) = task.values.remove(&id) else {
            continue;
        };
        let old = match old {
            FieldValue::Text(text) => FieldValue::Text(renames.get(&text).cloned().unwrap_or(text)),
            number => number,
        };
        if let Some(value) = converted(old, &kind) {
            task.values.insert(id, value);
        }
    }
    project.fields[position] = FieldDef { id, name, kind };
    Ok(())
}

/// Remove a field, every task's value for it, and its place in the leveling order.
pub(crate) fn delete_field(project: &mut Project, id: u32) -> Result<(), EngineError> {
    let position = position_of(project, id)?;
    project.fields.remove(position);
    level_order::drop_field(project, id);
    for task in &mut project.tasks {
        task.values.remove(&id);
    }
    Ok(())
}

/// Set or clear one task's value. Text is trimmed and empty text clears the value.
pub(crate) fn set_value(
    project: &mut Project,
    uid: u32,
    id: u32,
    value: Option<&FieldValue>,
) -> Result<(), EngineError> {
    let position = position_of(project, id)?;
    let value = match value {
        Some(value) => checked_value(&project.fields[position].kind, value)?,
        None => None,
    };
    let task = project.task_mut(uid).ok_or(EngineError::TaskNotFound)?;
    match value {
        Some(value) => task.values.insert(id, value),
        None => task.values.remove(&id),
    };
    Ok(())
}

/// Old choice to new choice, trimmed. Empty unless both kinds are choice lists; a rename
/// from something that was not a choice is ignored, and one to a name the new list lacks
/// is refused.
fn checked_renames(
    old: &FieldKind,
    new: &FieldKind,
    renamed: &[OptionRename],
) -> Result<BTreeMap<String, String>, EngineError> {
    let (FieldKind::Choice { options: before }, FieldKind::Choice { options: after }) = (old, new)
    else {
        return Ok(BTreeMap::new());
    };
    let mut renames = BTreeMap::new();
    for rename in renamed {
        let (from, to) = (rename.from.trim(), rename.to.trim());
        if !after.iter().any(|option| option == to) {
            return Err(Invalid::ChoiceOptions.into());
        }
        if before.iter().any(|option| option == from) {
            renames.insert(from.to_string(), to.to_string());
        }
    }
    Ok(renames)
}

/// The value `value` becomes in a field of `kind`, or `None` when it does not fit there.
/// Used where a value comes from elsewhere: another field kind, another project. It is
/// held to what `SetFieldValue` accepts: trimmed text of at most 255 characters, finite
/// numbers.
pub(crate) fn converted(value: FieldValue, kind: &FieldKind) -> Option<FieldValue> {
    let value = match value {
        FieldValue::Text(text) => {
            let text = text.trim();
            if text.is_empty() || text.chars().count() > MAX_TEXT_CHARS {
                return None;
            }
            FieldValue::Text(text.to_string())
        }
        FieldValue::Number(number) if !number.is_finite() => return None,
        number => number,
    };
    match (kind, value) {
        (FieldKind::Number, FieldValue::Number(number)) => Some(FieldValue::Number(number)),
        (FieldKind::Number, FieldValue::Text(text)) => parse_number(&text).map(FieldValue::Number),
        (FieldKind::Text, FieldValue::Text(text)) => Some(FieldValue::Text(text)),
        (FieldKind::Text, FieldValue::Number(number)) => Some(FieldValue::Text(number_text(number))),
        (FieldKind::Choice { options }, value) => {
            let text = match value {
                FieldValue::Text(text) => text,
                FieldValue::Number(number) => number_text(number),
            };
            options.contains(&text).then_some(FieldValue::Text(text))
        }
    }
}

/// A number as plain decimal text, exact and as short as that allows (no exponent);
/// -0 is written 0.
pub(crate) fn number_text(number: f64) -> String {
    if number == 0.0 {
        return "0".into();
    }
    format!("{number}")
}

/// A number written with a dot or a comma as the decimal mark.
fn parse_number(text: &str) -> Option<f64> {
    let text = text.trim();
    let number = text.parse::<f64>().or_else(|_| text.replace(',', ".").parse::<f64>()).ok()?;
    number.is_finite().then_some(number)
}

fn position_of(project: &Project, id: u32) -> Result<usize, EngineError> {
    project
        .fields
        .iter()
        .position(|field| field.id == id)
        .ok_or_else(|| Invalid::NoSuchField.into())
}

/// A trimmed name no other field has (letter case aside).
fn checked_name(project: &Project, own: Option<u32>, name: &str) -> Result<String, EngineError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err(Invalid::FieldName.into());
    }
    let taken = project
        .fields
        .iter()
        .any(|field| Some(field.id) != own && field.name.to_lowercase() == name.to_lowercase());
    if taken {
        return Err(Invalid::FieldNameTaken.into());
    }
    Ok(name.to_string())
}

/// The kind with its choices trimmed; a choice list must be non-empty and have no
/// empty, too long, or repeated choice.
fn checked_kind(kind: &FieldKind) -> Result<FieldKind, EngineError> {
    let FieldKind::Choice { options } = kind else {
        return Ok(kind.clone());
    };
    let options: Vec<String> = options.iter().map(|option| option.trim().to_string()).collect();
    let bad = options.is_empty()
        || options
            .iter()
            .enumerate()
            .any(|(index, option)| {
                option.is_empty()
                    || option.chars().count() > MAX_TEXT_CHARS
                    || options[..index].contains(option)
            });
    if bad {
        return Err(Invalid::ChoiceOptions.into());
    }
    Ok(FieldKind::Choice { options })
}

/// Room for one more field of `kind`, not counting the field `own` being edited.
fn check_room(project: &Project, own: Option<u32>, kind: &FieldKind) -> Result<(), EngineError> {
    let others = project.fields.iter().filter(|field| Some(field.id) != own);
    let same: usize = others
        .filter(|field| field.kind.uses_text_slot() == kind.uses_text_slot())
        .count();
    let (limit, reason) = if kind.uses_text_slot() {
        (MAX_TEXT_FIELDS, Invalid::TooManyTextFields)
    } else {
        (MAX_NUMBER_FIELDS, Invalid::TooManyNumberFields)
    };
    if same >= limit {
        return Err(reason.into());
    }
    Ok(())
}

/// The value as stored, or `None` for empty text; an error when it does not fit `kind`.
fn checked_value(kind: &FieldKind, value: &FieldValue) -> Result<Option<FieldValue>, EngineError> {
    let value = match value {
        FieldValue::Text(text) if text.trim().is_empty() => return Ok(None),
        FieldValue::Text(text) => {
            let text = text.trim();
            if text.chars().count() > MAX_TEXT_CHARS {
                return Err(Invalid::FieldValue.into());
            }
            FieldValue::Text(text.to_string())
        }
        FieldValue::Number(number) if !number.is_finite() => return Err(Invalid::FieldValue.into()),
        number => number.clone(),
    };
    let fits = match (kind, &value) {
        (FieldKind::Number, FieldValue::Number(_)) | (FieldKind::Text, FieldValue::Text(_)) => true,
        (FieldKind::Choice { options }, FieldValue::Text(text)) => options.contains(text),
        _ => false,
    };
    if fits {
        Ok(Some(value))
    } else {
        Err(Invalid::FieldValue.into())
    }
}
