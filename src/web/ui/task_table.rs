//! A table of tasks: name, optionally project, status, issue.

use super::button::add_link;
use super::status::status;
use super::table::{cell, col_head};
use crate::models::TaskStatus;
use topcoat::{
    Result,
    view::{View, component, view},
};

/// The project a row belongs to, as a link.
pub struct ProjectLink<'a> {
    pub name: &'a str,
    pub href: String,
}

/// One row, ready to show: plain display fields, the links already built
/// (see `web/task_rows.rs` for the mapping from the models).
pub struct TaskRow<'a> {
    pub name: &'a str,
    pub href: String,
    /// Shown in the Project column, which the table has when any row
    /// carries one.
    pub project: Option<ProjectLink<'a>>,
    pub status: TaskStatus,
    /// `#12`, or empty.
    pub issue: String,
}

/// `caption` is read by screen readers only. The Project column appears
/// when the rows carry a project. `add` is where the first row leads, when
/// the table has one: the whole row is a link ("New task…") that opens the
/// New task pop-up (`modal.rs`). The caller shows its own empty state
/// instead of an empty table (see `tasks_tab`).
///
/// The `role`s repeat the native table semantics (see `table.rs`).
#[component]
pub async fn task_table(
    rows: &[TaskRow<'_>],
    caption: &str,
    #[default] add: Option<String>,
) -> Result<impl View> {
    let project_column = rows.iter().any(|r| r.project.is_some());
    let columns = if project_column { "4" } else { "3" };
    Ok(view! {
        <table class="tasks" role="table">
            <caption class="sr">(caption)</caption>
            <thead role="rowgroup">
                <tr role="row">
                    col_head("Task")
                    if project_column {
                        col_head("Project")
                    }
                    col_head("Status")
                    col_head("Issue")
                </tr>
            </thead>
            <tbody role="rowgroup">
                if let Some(href) = &add {
                    <tr class="add" role="row">
                        cell(colspan: columns, add_link(href: href, label: "New task\u{2026}"))
                    </tr>
                }
                for row in rows.iter() {
                    <tr role="row">
                        cell(<a class="u" href=(row.href.as_str())>(row.name)</a>)
                        if project_column {
                            cell(class: "project",
                                if let Some(p) = &row.project {
                                    <a class="u muted" href=(p.href.as_str())>(p.name)</a>
                                }
                            )
                        }
                        cell(status(state: row.status))
                        // Empty issue: "No issue" for screen readers; the
                        // cell stays, so the row keeps its columns.
                        cell(class: "mono",
                            if row.issue.is_empty() {
                                <span class="sr">"No issue"</span>
                            } else {
                                (row.issue.as_str())
                            }
                        )
                    </tr>
                }
            </tbody>
        </table>
    })
}
