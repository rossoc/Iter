//! A table of tasks: name, optionally project, status, issue.

use super::status::status;
use super::table::{AddRow, cell, data_table, row};
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
/// New task pop-up (`modal.rs`). `empty` is the line shown when there are
/// no rows. It is a `data_table` (see `table.rs`).
#[component]
pub async fn task_table(
    rows: &[TaskRow<'_>],
    caption: &str,
    #[default] add: Option<String>,
    #[default] empty: Option<&str>,
) -> Result<impl View> {
    let project_column = rows.iter().any(|r| r.project.is_some());
    let mut columns = vec!["Task"];
    if project_column {
        columns.push("Project");
    }
    columns.extend(["Status", "Issue"]);
    let add = add.map(|href| AddRow::link(href, "New task\u{2026}"));
    Ok(view! {
        data_table(caption: caption, columns: &columns, class: "tasks", add: add, empty: empty,
            for r in rows.iter() {
                row(
                    cell(<a class="u" href=(r.href.as_str())>(r.name)</a>)
                    if project_column {
                        cell(class: "project",
                            if let Some(p) = &r.project {
                                <a class="u muted" href=(p.href.as_str())>(p.name)</a>
                            }
                        )
                    }
                    cell(status(state: r.status))
                    // Empty issue: "No issue" for screen readers; the
                    // cell stays, so the row keeps its columns.
                    cell(class: "mono",
                        if r.issue.is_empty() {
                            <span class="sr">"No issue"</span>
                        } else {
                            (r.issue.as_str())
                        }
                    )
                )
            }
        )
    })
}
