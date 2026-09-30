//! The Tasks tab of an organization or project page: the task table, or
//! "No tasks yet." when there are none, with the "New task" button when the
//! page can add one.

use super::button::add_button;
use super::empty_line::empty_line;
use super::task_table::{TaskRow, task_table};
use topcoat::{
    Result,
    view::{View, component, view},
};

/// The "New task" button, when the page can add a task.
#[component]
async fn new_task(add: &Option<String>) -> Result<impl View> {
    Ok(view! {
        if let Some(href) = add {
            <p class="tab-actions">add_button(href: href.as_str(), label: "New task")</p>
        }
    })
}

/// `add` is where "New task" leads, if the page can add a task (a project
/// can, an organization can't).
#[component]
pub async fn tasks_tab(
    rows: &[TaskRow<'_>],
    caption: &str,
    #[default] add: Option<String>,
) -> Result<impl View> {
    Ok(view! {
        if rows.is_empty() {
            empty_line("No tasks yet.")
            new_task(add: &add)
        } else {
            new_task(add: &add)
            task_table(rows: rows, caption: caption)
        }
    })
}
