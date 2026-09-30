//! The inputs of a task, read back by `TaskForm`: shared by the new-task
//! form and the task edit form (the write-side twin of the task page).
//!
//! Name and Description are `edit_page`'s. Status and Tags are loose; Start
//! and Duration are grouped as "Schedule", the issue number and branch
//! prefix as "GitHub". Hints: what it is, then "Leave empty for X".

use super::forms::TaskForm;
use super::ui::field::{field, select};
use super::ui::form_error::FormError;
use super::ui::group::group;
use crate::models::TaskStatus;
use topcoat::{
    Result,
    view::{View, component, view},
};

/// `form` holds what each input shows, as text: the task's own values, or
/// what was typed after a refused submit (`TaskForm::of`). `error` is the
/// form's refusal, if any.
#[component]
pub async fn task_fields(
    form: &TaskForm,
    #[default] error: Option<&FormError>,
) -> Result<impl View> {
    let statuses = TaskStatus::options();
    Ok(view! {
        select(name: TaskForm::STATUS, label: "Status", options: &statuses, current: &form.status, error: error)
        field(name: TaskForm::TAGS, label: "Tags", value: &form.tags, identifier: true, error: error,
            hint: "The task's tags, comma-separated. Urgent and Important flag the task. Leave empty for none.")
        group(
            title: "Schedule",
            field(name: TaskForm::START_TIME, label: "Start", value: &form.start_time, identifier: true, error: error,
                hint: "When the task starts, as year-month-day hours:minutes, for example 2026-09-29 14:30. Leave empty for none.")
            field(name: TaskForm::DURATION, label: "Duration", value: &form.duration, identifier: true, error: error,
                hint: "How long the task takes, as hours:minutes, for example 01:30. Leave empty for none.")
        )
        group(
            title: "GitHub",
            field(name: TaskForm::GITHUB_ISSUE, label: "GitHub issue", value: &form.github_issue, identifier: true, numeric: true, error: error,
                hint: "The GitHub issue this task tracks, such as 42. Leave empty for none.")
            field(name: TaskForm::BRANCH_PREFIX, label: "Branch prefix", value: &form.branch_prefix, identifier: true,
                hint: "Put in front of the task's name to make its branch, such as feat/. Leave empty to follow the project's template.")
        )
    })
}
