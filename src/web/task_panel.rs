//! The Details aside of a task: a `Task` and its tags mapped onto the
//! generic `ui::panel` rows (like `settings_panel.rs` for a `Settings`).

use super::task_rows::{chips, issue_label};
use super::ui::panel::{chips_row, panel, text_row, value_row};
use super::ui::status::status;
use crate::models::{Tag, Task};
use topcoat::{
    Result,
    view::{View, component, view},
};

/// Start and Duration are the slot the task is booked for (the edit form
/// groups them as "Schedule"), not what its sessions add up to (those are in
/// the Sessions table).
#[component]
pub async fn task_panel(task: &Task, tags: &[Tag]) -> Result<impl View> {
    let issue = issue_label(task.github_issue);
    let (start, duration) = (task.start_text(), task.duration_text());
    let chips = chips(tags);
    Ok(view! {
        panel(
            id: "details-title",
            title: "Details",
            value_row(label: "Status", status(state: task.status))
            text_row(label: "GitHub issue", value: &issue)
            text_row(label: "Branch prefix", value: &task.branch_prefix)
            text_row(label: "Start", value: &start)
            text_row(label: "Duration", value: &duration)
            chips_row(label: "Tags", chips: &chips)
        )
    })
}
