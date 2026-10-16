//! MSPDI custom fields. `<ExtendedAttributes>` under `<Project>` declares them; each
//! `<Task>` holds an `<ExtendedAttribute>` (`FieldID`, `Value`) per value.
//!
//! Microsoft Project names a task's local fields by slot: Text1–Text30 and
//! Number1–Number20, each with a fixed `FieldID` (the `PjCustomField` enumeration). Sheafly
//! fields take the slots in the order they were added: text and choice fields the text
//! slots, number fields the number slots; the user's name goes in `Alias`. A choice field
//! is a text field with a `ValueList`. Reading accepts text and number fields, with or
//! without a value list, and leaves the other kinds (cost, date, duration, flag) out. The
//! slot decides between text and number: `CFType` (5 number, 7 text) only tells another
//! kind apart.

use std::collections::BTreeMap;

use crate::fields::{self, FieldDef, FieldKind, FieldValue, MAX_NAME_CHARS, MAX_TEXT_CHARS};
use crate::mspdi::escape_text;
use crate::{mspdi_range, mspdi_skills};
use crate::mspdi_report::ImportNote;
use crate::model::Project;

/// `FieldID` of Text1–Text30 for tasks.
const TEXT_IDS: [u32; 30] = [
    188743731, 188743734, 188743737, 188743740, 188743743, 188743746, 188743747, 188743748,
    188743749, 188743750, 188743997, 188743998, 188743999, 188744000, 188744001, 188744002,
    188744003, 188744004, 188744005, 188744006, 188744007, 188744008, 188744009, 188744010,
    188744011, 188744012, 188744013, 188744014, 188744015, 188744016,
];
/// `FieldID` of Number1–Number20 for tasks.
const NUMBER_IDS: [u32; 20] = [
    188743767, 188743768, 188743769, 188743770, 188743771, 188743982, 188743983, 188743984,
    188743985, 188743986, 188743987, 188743988, 188743989, 188743990, 188743991, 188743992,
    188743993, 188743994, 188743995, 188743996,
];
/// MSPDI `CFType` of a number and of a text field.
const CF_NUMBER: u32 = 5;
const CF_TEXT: u32 = 7;

/// A task field slot: text or number, and its number (Text3 is `(false, 3)`).
fn slot_of(field_id: u32) -> Option<(bool, usize)> {
    if let Some(index) = TEXT_IDS.iter().position(|id| *id == field_id) {
        return Some((false, index + 1));
    }
    NUMBER_IDS
        .iter()
        .position(|id| *id == field_id)
        .map(|index| (true, index + 1))
}

fn slot_name(field_id: u32) -> Option<String> {
    let (number, index) = slot_of(field_id)?;
    Some(format!("{}{index}", if number { "Number" } else { "Text" }))
}

/// The MSPDI `FieldID` of each field, by field id. The limits on fields keep every field
/// inside its slots.
fn slots(project: &Project) -> BTreeMap<u32, u32> {
    let (mut text, mut number) = (TEXT_IDS.iter(), NUMBER_IDS.iter());
    project
        .fields
        .iter()
        .filter_map(|field| {
            let slot = match field.kind {
                FieldKind::Number => number.next(),
                FieldKind::Text | FieldKind::Choice { .. } => text.next(),
            }?;
            Some((field.id, *slot))
        })
        .collect()
}

/// `<ExtendedAttributes>`, which goes before `<Calendars>`; nothing without fields or
/// skills or effort ranges (`mspdi_skills.rs` and `mspdi_range.rs` declare theirs here too).
pub(crate) fn write_definitions(xml: &mut String, project: &Project) {
    if project.fields.is_empty() && !mspdi_skills::any(project) && !mspdi_range::any(project)
    {
        return;
    }
    let slots = slots(project);
    xml.push_str("<ExtendedAttributes>\n");
    for field in &project.fields {
        let Some(field_id) = slots.get(&field.id) else {
            continue;
        };
        xml.push_str("<ExtendedAttribute>\n");
        el(xml, "FieldID", &field_id.to_string());
        el(xml, "FieldName", &slot_name(*field_id).unwrap_or_default());
        let cf_type = if field.kind == FieldKind::Number { CF_NUMBER } else { CF_TEXT };
        el(xml, "CFType", &cf_type.to_string());
        el(xml, "ElemType", "20");
        el(xml, "Alias", &escape_text(&field.name));
        if let FieldKind::Choice { options } = &field.kind {
            el(xml, "RestrictValues", "1");
            xml.push_str("<ValueList>\n");
            for (index, option) in options.iter().enumerate() {
                xml.push_str("<Value>\n");
                el(xml, "ID", &(index + 1).to_string());
                el(xml, "Value", &escape_text(option));
                xml.push_str("</Value>\n");
            }
            xml.push_str("</ValueList>\n");
        }
        xml.push_str("</ExtendedAttribute>\n");
    }
    mspdi_skills::write_definition(xml, project);
    mspdi_range::write_definitions(xml, project);
    xml.push_str("</ExtendedAttributes>\n");
}

/// The task's `<ExtendedAttribute>` values, in field order.
pub(crate) fn write_values(xml: &mut String, project: &Project, task: &crate::model::Task) {
    if task.values.is_empty() {
        return;
    }
    let slots = slots(project);
    for field in &project.fields {
        let (Some(value), Some(field_id)) = (task.values.get(&field.id), slots.get(&field.id))
        else {
            continue;
        };
        let text = match value {
            FieldValue::Number(number) => fields::number_text(*number),
            FieldValue::Text(text) => escape_text(text),
        };
        xml.push_str("<ExtendedAttribute>\n");
        el(xml, "FieldID", &field_id.to_string());
        el(xml, "Value", &text);
        xml.push_str("</ExtendedAttribute>\n");
    }
}

/// A field declaration being read.
#[derive(Default)]
struct Declared {
    field_id: u32,
    field_name: String,
    alias: String,
    cf_type: Option<u32>,
    choices: Vec<String>,
}

/// Collects declarations and task values while the file is read; `finish` turns them
/// into fields once the tasks exist.
#[derive(Default)]
pub(crate) struct FieldReader {
    declared: Vec<Declared>,
    open_declaration: Option<Declared>,
    /// `FieldID` and value of a task's `<ExtendedAttribute>` being read.
    open_value: Option<(u32, String)>,
    /// Values of the task being read.
    task_values: Vec<(u32, String)>,
    /// Values by task uid.
    values: Vec<(u32, u32, String)>,
}

impl FieldReader {
    /// An element opened inside `parent`.
    pub fn start(&mut self, name: &str, parent: Option<&str>) {
        match (parent, name) {
            (Some("ExtendedAttributes"), "ExtendedAttribute") => {
                self.open_declaration = Some(Declared::default());
            }
            (Some("Task"), "ExtendedAttribute") => self.open_value = Some((0, String::new())),
            (Some("Tasks"), "Task") => self.task_values.clear(),
            _ => {}
        }
    }

    /// The text of an element that closed inside `parent`.
    pub fn text(&mut self, name: &str, parent: Option<&str>, text: &str) {
        let text = text.trim();
        if let Some(declared) = self.open_declaration.as_mut() {
            match (parent, name) {
                (Some("ExtendedAttribute"), "FieldID") => {
                    declared.field_id = text.parse().unwrap_or(0)
                }
                (Some("ExtendedAttribute"), "FieldName") => declared.field_name = text.into(),
                (Some("ExtendedAttribute"), "Alias") => declared.alias = text.into(),
                (Some("ExtendedAttribute"), "CFType") => declared.cf_type = text.parse().ok(),
                // `<ValueList><Value><Value>choice</Value></Value></ValueList>`
                (Some("Value"), "Value") if !text.is_empty() => {
                    declared.choices.push(text.into())
                }
                _ => {}
            }
        } else if let Some((field_id, value)) = self.open_value.as_mut() {
            match (parent, name) {
                (Some("ExtendedAttribute"), "FieldID") => *field_id = text.parse().unwrap_or(0),
                (Some("ExtendedAttribute"), "Value") => *value = text.into(),
                _ => {}
            }
        }
    }

    /// An element closed inside `parent`.
    pub fn end(&mut self, name: &str, parent: Option<&str>) {
        match (parent, name) {
            (Some("ExtendedAttributes"), "ExtendedAttribute") => {
                if let Some(declared) = self.open_declaration.take() {
                    self.declared.push(declared);
                }
            }
            (Some("Task"), "ExtendedAttribute") => {
                if let Some(value) = self.open_value.take() {
                    self.task_values.push(value);
                }
            }
            _ => {}
        }
    }

    /// The task with `uid` was read: its values belong to it.
    pub fn end_task(&mut self, uid: u32) {
        for (field_id, value) in self.task_values.drain(..) {
            self.values.push((uid, field_id, value));
        }
    }

    /// Declare the fields the tasks use and give the tasks their values. Values of a kind
    /// Sheafly has no field for are left out and noted; `taken` slots are read elsewhere.
    pub fn finish(self, project: &mut Project, notes: &mut Vec<ImportNote>, taken: &[u32]) {
        let declared: BTreeMap<u32, &Declared> =
            self.declared.iter().map(|declared| (declared.field_id, declared)).collect();
        let mut field_of: BTreeMap<u32, u32> = BTreeMap::new();
        let mut skipped: BTreeMap<String, usize> = BTreeMap::new();
        let mut unread: BTreeMap<String, usize> = BTreeMap::new();
        // Declared fields first, in the file's order, then slots used without a declaration.
        let used: Vec<u32> = self
            .declared
            .iter()
            .map(|declared| declared.field_id)
            .chain(self.values.iter().map(|(_, field_id, _)| *field_id))
            .collect();
        for field_id in used {
            if field_of.contains_key(&field_id) {
                continue;
            }
            let file = declared.get(&field_id).copied();
            let Some(kind) = kind_of(field_id, file) else {
                continue;
            };
            let name = unique_name(project, field_id, file);
            let id = fields::alloc_field_id(project);
            project.fields.push(FieldDef { id, name, kind });
            field_of.insert(field_id, id);
        }
        for (uid, field_id, text) in self.values {
            if taken.contains(&field_id) {
                continue;
            }
            let Some(&id) = field_of.get(&field_id) else {
                if !text.trim().is_empty() {
                    let name = declared
                        .get(&field_id)
                        .map(|file| display_name(field_id, file))
                        .unwrap_or_else(|| field_id.to_string());
                    *skipped.entry(name).or_insert(0) += 1;
                }
                continue;
            };
            if !give_value(project, uid, id, &text) {
                let name = project.fields.iter().find(|field| field.id == id).map(|f| f.name.clone());
                *unread.entry(name.unwrap_or_default()).or_insert(0) += 1;
            }
        }
        notes.extend(
            skipped
                .into_iter()
                .map(|(name, values)| ImportNote::FieldSkipped { name, values }),
        );
        notes.extend(
            unread
                .into_iter()
                .map(|(name, values)| ImportNote::FieldValuesUnread { name, values }),
        );
    }
}

/// The kind of field `field_id` becomes, or `None` for a kind Sheafly does not keep: a
/// slot other than Text or Number, or a `CFType` that is neither.
fn kind_of(field_id: u32, file: Option<&Declared>) -> Option<FieldKind> {
    let (number, _) = slot_of(field_id)?;
    let cf_type = file.and_then(|file| file.cf_type);
    if cf_type.is_some_and(|cf_type| cf_type != CF_NUMBER && cf_type != CF_TEXT) {
        return None;
    }
    if number {
        return Some(FieldKind::Number);
    }
    let mut options: Vec<String> = Vec::new();
    for choice in file.map(|file| file.choices.as_slice()).unwrap_or_default() {
        let choice: String = choice.trim().chars().take(MAX_TEXT_CHARS).collect();
        if !choice.is_empty() && !options.contains(&choice) {
            options.push(choice);
        }
    }
    Some(if options.is_empty() {
        FieldKind::Text
    } else {
        FieldKind::Choice { options }
    })
}

/// The field's alias, else its slot name.
fn display_name(field_id: u32, file: &Declared) -> String {
    [file.alias.trim(), file.field_name.trim()]
        .into_iter()
        .find(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| slot_name(field_id))
        .unwrap_or_else(|| field_id.to_string())
}

/// A name for the new field no other field has: the alias, else the slot name, with
/// the slot name added when the alias is taken.
fn unique_name(project: &Project, field_id: u32, file: Option<&Declared>) -> String {
    let slot = slot_name(field_id).unwrap_or_else(|| field_id.to_string());
    let base: String = file
        .map(|file| display_name(field_id, file))
        .unwrap_or_else(|| slot.clone())
        .chars()
        .take(MAX_NAME_CHARS)
        .collect();
    let taken = |name: &str| {
        project
            .fields
            .iter()
            .any(|field| field.name.to_lowercase() == name.to_lowercase())
    };
    if !taken(&base) {
        return base;
    }
    let suffix = format!(" ({slot})");
    let keep = MAX_NAME_CHARS.saturating_sub(suffix.chars().count());
    let named: String = base.chars().take(keep).collect::<String>() + &suffix;
    if !taken(&named) {
        return named;
    }
    (2..)
        .map(|n| format!("{slot} {n}"))
        .find(|name| !taken(name))
        .unwrap_or(slot)
}

/// Give the task `uid` the value `text` for field `id`; false when a number field's text
/// is no number. A choice the list lacks is added to it, so no value from the file is
/// lost.
fn give_value(project: &mut Project, uid: u32, id: u32, text: &str) -> bool {
    let text: String = text.trim().chars().take(MAX_TEXT_CHARS).collect();
    if text.is_empty() {
        return true;
    }
    let Some(field) = project.fields.iter_mut().find(|field| field.id == id) else {
        return true;
    };
    let value = match &mut field.kind {
        // XML numbers always use a dot: "1,234" is no number here.
        FieldKind::Number => match text.parse::<f64>().ok().filter(|number| number.is_finite()) {
            Some(number) => FieldValue::Number(number),
            None => return false,
        },
        FieldKind::Text => FieldValue::Text(text),
        FieldKind::Choice { options } => {
            if !options.contains(&text) {
                options.push(text.clone());
            }
            FieldValue::Text(text)
        }
    };
    if let Some(task) = project.task_mut(uid) {
        task.values.insert(id, value);
    }
    true
}

fn el(xml: &mut String, name: &str, value: &str) {
    xml.push_str(&format!("<{name}>{value}</{name}>\n"));
}
