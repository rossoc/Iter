//! The Details aside of a task: a `Task` and its tags mapped onto the
//! generic `ui::panel` rows (like `settings_panel.rs` for a `Settings`).

use super::task_rows::{chips, issue_label};
use super::ui::panel::{Row, panel};
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
    let rows = [
        Row::Status("Status", task.status),
        Row::Text("GitHub issue", issue_label(task.github_issue)),
        Row::Text("Branch prefix", task.branch_prefix.clone()),
        Row::Text("Start", task.start_text()),
        Row::Text("Duration", task.duration_text()),
        Row::Chips("Tags", chips(tags)),
    ];
    Ok(view! {
        panel(id: "details-title", title: "Details", rows: &rows)
    })
}
