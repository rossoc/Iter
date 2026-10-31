//! The Tasks tab of an organization or project page: the search box, the task
//! table (its first row opens the New task pop-up, when the page can add a
//! task); with no rows the table says "No tasks yet." / "No tasks match.".

use super::filter_bar::{Search, filter_bar};
use super::task_table::{TaskRow, task_table};
use topcoat::{
    Result,
    view::{View, component, view},
};

/// `rows` are the tasks that pass the search, out of `total`. `add` is where
/// the table's first row leads (the New task pop-up), if the page can add a
/// task.
#[component]
pub async fn tasks_tab(
    rows: &[TaskRow<'_>],
    caption: &str,
    total: usize,
    search: Search,
    #[default] add: Option<String>,
) -> Result<impl View> {
    let none = if total == 0 {
        "No tasks yet."
    } else {
        "No tasks match."
    };
    Ok(view! {
        if total > 0 {
            filter_bar(search: &search)
            if rows.len() != total {
                <p class="filter-count" role="status">(rows.len()) " of " (total) " tasks match"</p>
            }
        }
        task_table(rows: rows, caption: caption, add: add, empty: rows.is_empty().then_some(none))
    })
}
