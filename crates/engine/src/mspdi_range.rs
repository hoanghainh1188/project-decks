//! Effort ranges (`Task::work_low`, `work_high`) in MSPDI, as two task duration fields.
//!
//! Microsoft Project has no range estimate. Export declares task Duration1 (`FieldID`
//! 188743783) with the alias "Effort thấp" and Duration2 (188743784) with "Effort cao"
//! when a task has a range, and writes each amount as an ISO duration of working time.
//! Import reads any task duration field whose alias is one of `LOW_ALIASES` or
//! `HIGH_ALIASES` (letter case aside). A task gets a range only when both amounts read,
//! `0 <= low <= high`, and it is a leaf that is not a milestone, as `SetWorkRange` requires;
//! other values are dropped.

use crate::model::Project;
use crate::timeutil::{format_duration_iso, parse_duration_iso};

/// `FieldID` of task Duration1 and Duration2, where the range is written.
const LOW_FIELD_ID: u32 = 188_743_783;
const HIGH_FIELD_ID: u32 = 188_743_784;
/// `FieldID` of task Duration1–Duration10.
const TASK_DURATION_IDS: [u32; 10] = [
    188743783, 188743784, 188743785, 188743955, 188743956, 188743957, 188743958, 188743959,
    188743960, 188743961,
];
/// MSPDI `CFType` of a duration field.
const CF_DURATION: &str = "2";
const LOW_ALIAS: &str = "Effort thấp";
const HIGH_ALIAS: &str = "Effort cao";
const LOW_ALIASES: [&str; 3] = ["effort thấp", "low effort", "effort low"];
const HIGH_ALIASES: [&str; 3] = ["effort cao", "high effort", "effort high"];

/// Whether any task has a range: only then are the fields declared and written.
pub(crate) fn any(project: &Project) -> bool {
    project.tasks.iter().any(|task| task.work_low.is_some())
}

/// The two declarations, inside `<ExtendedAttributes>`.
pub(crate) fn write_definitions(xml: &mut String, project: &Project) {
    if !any(project) {
        return;
    }
    for (field_id, name, alias) in [
        (LOW_FIELD_ID, "Duration1", LOW_ALIAS),
        (HIGH_FIELD_ID, "Duration2", HIGH_ALIAS),
    ] {
        xml.push_str("<ExtendedAttribute>\n");
        el(xml, "FieldID", &field_id.to_string());
        el(xml, "FieldName", name);
        el(xml, "CFType", CF_DURATION);
        el(xml, "ElemType", "20");
        el(xml, "Alias", alias);
        xml.push_str("</ExtendedAttribute>\n");
    }
}

/// The task's range, as two `<ExtendedAttribute>` values.
pub(crate) fn write_values(xml: &mut String, task: &crate::model::Task) {
    let (Some(low), Some(high)) = (task.work_low, task.work_high) else {
        return;
    };
    for (field_id, minutes) in [(LOW_FIELD_ID, low), (HIGH_FIELD_ID, high)] {
        xml.push_str("<ExtendedAttribute>\n");
        el(xml, "FieldID", &field_id.to_string());
        el(xml, "Value", &format_duration_iso(minutes));
        el(xml, "DurationFormat", "7");
        xml.push_str("</ExtendedAttribute>\n");
    }
}

/// Collects the range fields' `FieldID`s and tasks' values while the file is read.
#[derive(Default)]
pub(crate) struct RangeReader {
    /// `FieldID` and alias of the declaration being read.
    declaring: Option<(u32, String)>,
    low_fields: Vec<u32>,
    high_fields: Vec<u32>,
    /// `FieldID` and value of a task's `<ExtendedAttribute>` being read.
    open_value: Option<(u32, String)>,
    task_values: Vec<(u32, String)>,
    /// Values by task uid, in file order.
    values: Vec<(u32, u32, String)>,
}

impl RangeReader {
    pub fn start(&mut self, name: &str, parent: Option<&str>) {
        match (parent, name) {
            (Some("ExtendedAttributes"), "ExtendedAttribute") => {
                self.declaring = Some((0, String::new()))
            }
            (Some("Task"), "ExtendedAttribute") => self.open_value = Some((0, String::new())),
            (Some("Tasks"), "Task") => self.task_values.clear(),
            _ => {}
        }
    }

    pub fn text(&mut self, name: &str, parent: Option<&str>, text: &str) {
        if parent != Some("ExtendedAttribute") {
            return;
        }
        let text = text.trim();
        if let Some((field_id, alias)) = self.declaring.as_mut() {
            match name {
                "FieldID" => *field_id = text.parse().unwrap_or(0),
                "Alias" => *alias = text.into(),
                _ => {}
            }
        } else if let Some((field_id, value)) = self.open_value.as_mut() {
            match name {
                "FieldID" => *field_id = text.parse().unwrap_or(0),
                "Value" => *value = text.into(),
                _ => {}
            }
        }
    }

    pub fn end(&mut self, name: &str, parent: Option<&str>) {
        match (parent, name) {
            (Some("ExtendedAttributes"), "ExtendedAttribute") => {
                if let Some((field_id, alias)) = self.declaring.take() {
                    if !TASK_DURATION_IDS.contains(&field_id) {
                        return;
                    }
                    let alias = alias.to_lowercase();
                    if LOW_ALIASES.contains(&alias.as_str()) {
                        self.low_fields.push(field_id);
                    } else if HIGH_ALIASES.contains(&alias.as_str()) {
                        self.high_fields.push(field_id);
                    }
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

    /// The `FieldID`s read as a range, which custom fields leave alone.
    pub fn taken(&self) -> Vec<u32> {
        self.low_fields
            .iter()
            .chain(&self.high_fields)
            .copied()
            .collect()
    }

    /// Give each leaf its range. Summaries follow from outline levels, since the file's
    /// `Summary` flag is set again by scheduling.
    pub fn finish(self, project: &mut Project) {
        let mut ranges: std::collections::BTreeMap<u32, (Option<i64>, Option<i64>)> =
            Default::default();
        for (uid, field_id, value) in self.values {
            let Some(minutes) = parse_duration_iso(&value) else {
                continue;
            };
            let entry = ranges.entry(uid).or_default();
            if self.low_fields.contains(&field_id) {
                entry.0.get_or_insert(minutes);
            } else if self.high_fields.contains(&field_id) {
                entry.1.get_or_insert(minutes);
            }
        }
        let levels: Vec<u32> = project
            .tasks
            .iter()
            .map(|task| task.outline_level)
            .collect();
        for (index, task) in project.tasks.iter_mut().enumerate() {
            let Some(&(Some(low), Some(high))) = ranges.get(&task.uid) else {
                continue;
            };
            let summary = levels
                .get(index + 1)
                .is_some_and(|&next| next > levels[index]);
            if summary || task.milestone || low < 0 || low > high {
                continue;
            }
            task.work_low = Some(low);
            task.work_high = Some(high);
        }
    }
}

fn el(xml: &mut String, name: &str, value: &str) {
    xml.push_str(&format!("<{name}>{value}</{name}>\n"));
}
