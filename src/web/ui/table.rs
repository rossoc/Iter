//! The one table every list of the app is built from, modelled on the tasks
//! table: a header row of small-caps labels (optional), a tinted full-width
//! "add" row under it, then rows separated by thin lines, optionally in
//! sections (the Notes tab's months). A list is `data_table` with its
//! columns, its rows (`row` and `cell`) as the child nodes, and, when it
//! has none, its own `empty` line as the last row. The variants (tasks,
//! sessions, projects, notes, agents, instructions) differ only in their
//! cells and in a class that tunes them (`class`).
//!
//! Each element repeats its native table semantics as a `role`: on narrow
//! screens the rows are laid out as boxes, and browsers then drop a table's
//! semantics. A table without a header (`headed: false`) is still named by
//! its caption.

use super::button::{add_link, add_post};
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// The first row, which adds an entity: a link to the New pop-up
/// (`modal.rs`), or a button that posts (Today's note).
pub struct AddRow<'a> {
    pub label: &'a str,
    pub target: String,
    pub post: bool,
}

impl<'a> AddRow<'a> {
    /// A row that opens `href` (a `?new=` pop-up).
    pub fn link(href: impl Into<String>, label: &'a str) -> Self {
        AddRow {
            label,
            target: href.into(),
            post: false,
        }
    }

    /// A row that posts to `action`.
    pub fn post(action: impl Into<String>, label: &'a str) -> Self {
        AddRow {
            label,
            target: action.into(),
            post: true,
        }
    }
}

/// `caption` is read by screen readers only. `columns` are the headers (and
/// the count of columns the add, section and empty rows span); `actions`
/// adds a last column of buttons, whose header only screen readers get; `headed`
/// false leaves the header row out. `class` tunes the variant (`tasks`,
/// `sessions`, ...; `inline` keeps the first cell beside the others on
/// narrow screens). `empty` is the line that takes the place of the rows
/// when there are none. The child nodes are the rows.
#[component]
pub async fn data_table(
    caption: &str,
    columns: &[&str],
    #[default] actions: bool,
    #[default] class: &str,
    #[default(true)] headed: bool,
    #[default] add: Option<AddRow<'_>>,
    #[default] empty: Option<&str>,
    child: Child<'_>,
) -> Result<impl View> {
    let span = (columns.len() + usize::from(actions)).to_string();
    let classes = format!("tbl {class}");
    Ok(view! {
        <table class=(classes.as_str()) role="table">
            <caption class="sr">(caption)</caption>
            if headed {
                <thead role="rowgroup">
                    <tr role="row">
                        for c in columns.iter() {
                            col_head((*c))
                        }
                        if actions {
                            col_head(<span class="sr">"Actions"</span>)
                        }
                    </tr>
                </thead>
            }
            <tbody role="rowgroup">
                if let Some(a) = &add {
                    <tr class="add" role="row">
                        cell(colspan: span.as_str(),
                            if a.post {
                                add_post(action: a.target.as_str(), label: a.label)
                            } else {
                                add_link(href: a.target.as_str(), label: a.label)
                            }
                        )
                    </tr>
                }
                (child)
                if let Some(text) = empty {
                    <tr class="empty" role="row">
                        cell(colspan: span.as_str(), (text))
                    </tr>
                }
            </tbody>
        </table>
    })
}

/// A row; `class` marks it (`current`).
#[component]
pub async fn row(#[default] class: &str, child: Child<'_>) -> Result<impl View> {
    Ok(view! {
        <tr if !class.is_empty() { class=(class) } role="row">(child)</tr>
    })
}

/// A heading between groups of rows (a month); `span` is the table's
/// column count.
#[component]
pub async fn section_row(label: &str, span: usize) -> Result<impl View> {
    let span = span.to_string();
    Ok(view! {
        <tr class="section" role="row">
            cell(colspan: span.as_str(), <h2 class="label">(label)</h2>)
        </tr>
    })
}

/// A column's header; the child nodes are its name.
#[component]
pub async fn col_head(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <th scope="col" role="columnheader">(child)</th> })
}

/// A cell; `class` is its look (`mono`), `colspan` the columns it spans
/// when more than one.
#[component]
pub async fn cell(
    #[default] class: &str,
    #[default] colspan: &str,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <td if !class.is_empty() { class=(class) } if !colspan.is_empty() { colspan=(colspan) } role="cell">(child)</td>
    })
}
