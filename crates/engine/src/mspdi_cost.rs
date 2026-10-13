//! MSPDI cost fields: `Cost`, `ActualCost` and `RemainingCost` on tasks, assignments and
//! resources, and the project's `EditableActualCosts` switch.
//!
//! The engine computes every one of these (`work.rs`), so they are written for other
//! programs to read. On import only a task's `ActualCost` is used, and only when the file
//! says actual costs are editable: it becomes the task's hand-entered actual cost. A file
//! that lets Project calculate them gets the engine's own numbers.

use crate::model::{Assignment, Project, Resource, Task};

/// `Cost`, `ActualCost`, `RemainingCost` of a task.
pub(crate) fn write_task_costs(xml: &mut String, task: &Task) {
    write_costs(xml, task.cost, task.actual_cost, task.remaining_cost);
}

/// `ActualCost` and `RemainingCost` of an assignment (its `Cost` is written with it).
pub(crate) fn write_assignment_costs(xml: &mut String, assignment: &Assignment) {
    el(xml, "ActualCost", &number(assignment.actual_cost));
    el(
        xml,
        "RemainingCost",
        &number((assignment.cost - assignment.actual_cost).max(0.0)),
    );
}

/// A resource's costs are the sum over its assignments.
pub(crate) fn write_resource_costs(xml: &mut String, project: &Project, resource: &Resource) {
    let own = project
        .assignments
        .iter()
        .filter(|assignment| assignment.resource_uid == resource.uid);
    let (cost, actual) = own.fold((0.0, 0.0), |(cost, actual), assignment| {
        (cost + assignment.cost, actual + assignment.actual_cost)
    });
    write_costs(xml, cost, actual, (cost - actual).max(0.0));
}

/// The project-level switch, written near the start of the file.
pub(crate) fn write_editable(xml: &mut String, project: &Project) {
    el(
        xml,
        "EditableActualCosts",
        if project.editable_actual_costs {
            "1"
        } else {
            "0"
        },
    );
}

fn write_costs(xml: &mut String, cost: f64, actual: f64, remaining: f64) {
    el(xml, "Cost", &number(cost));
    el(xml, "ActualCost", &number(actual));
    el(xml, "RemainingCost", &number(remaining));
}

/// Money in the file has at most two decimals.
fn number(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded}")
    }
}

fn el(xml: &mut String, name: &str, value: &str) {
    xml.push('<');
    xml.push_str(name);
    xml.push('>');
    xml.push_str(value);
    xml.push_str("</");
    xml.push_str(name);
    xml.push_str(">\n");
}
