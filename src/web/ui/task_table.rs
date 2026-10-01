//! A table of tasks: name, optionally project, status, issue.

use super::status::status;
use super::{PLUS, icon};
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
/// The `role`s repeat the native table semantics: on narrow screens the rows
/// are laid out as a grid, and browsers then drop a table's semantics.
#[component]
pub async fn task_table(
    rows: &[TaskRow<'_>],
    caption: &str,
    #[default] add: Option<String>,
) -> Result<impl View> {
    let project_column = rows.iter().any(|r| r.project.is_some());
    let columns = if project_column { 4 } else { 3 };
    Ok(view! {
        <table class="tasks" role="table">
            <caption class="sr">(caption)</caption>
            <thead role="rowgroup">
                <tr role="row">
                    <th scope="col" role="columnheader">"Task"</th>
                    if project_column {
                        <th scope="col" role="columnheader">"Project"</th>
                    }
                    <th scope="col" role="columnheader">"Status"</th>
                    <th scope="col" role="columnheader">"Issue"</th>
                </tr>
            </thead>
            <tbody role="rowgroup">
                if let Some(href) = &add {
                    <tr class="add" role="row">
                        <td role="cell" colspan=(columns.to_string())>
                            <a class="add-link" href=(href.as_str())>(icon(PLUS)) "New task\u{2026}"</a>
                        </td>
                    </tr>
                }
                for row in rows.iter() {
                    <tr role="row">
                        <td role="cell"><a class="u" href=(row.href.as_str())>(row.name)</a></td>
                        if project_column {
                            <td class="project" role="cell">
                                if let Some(p) = &row.project {
                                    <a class="u muted" href=(p.href.as_str())>(p.name)</a>
                                }
                            </td>
                        }
                        <td role="cell">status(state: row.status)</td>
                        // Empty issue: "No issue" for screen readers; the
                        // cell stays, so the row keeps its columns.
                        <td class="mono" role="cell">
                            if row.issue.is_empty() {
                                <span class="sr">"No issue"</span>
                            } else {
                                (row.issue.as_str())
                            }
                        </td>
                    </tr>
                }
            </tbody>
        </table>
    })
}
