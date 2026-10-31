//! The table every list is built from: optional small-caps header, a tinted
//! "add" row, then rows (optionally in sections), with its own `empty` line when
//! there are none. Variants differ in cells and a `class`. Each element repeats
//! its native semantics as a `role`, because browsers drop them when narrow
//! screens lay rows out as boxes; a headerless table is named by its caption.

use super::button::{add_link, add_post};
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// The first row, which adds an entity: a link to the New pop-up or a posting button.
pub struct AddRow<'a> {
    pub label: &'a str,
    pub target: String,
    pub post: bool,
}

impl<'a> AddRow<'a> {
    pub fn link(href: impl Into<String>, label: &'a str) -> Self {
        AddRow {
            label,
            target: href.into(),
            post: false,
        }
    }

    pub fn post(action: impl Into<String>, label: &'a str) -> Self {
        AddRow {
            label,
            target: action.into(),
            post: true,
        }
    }
}

/// `caption` is for screen readers only. `columns` are the headers (and the span of
/// the add, section and empty rows); `actions` adds a last button column; `headed`
/// false drops the header row. `class` tunes the variant (`inline` keeps the first
/// cell beside the others when narrow). `empty` replaces the rows when none.
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
                    <tr class="no-rows" role="row">
                        cell(colspan: span.as_str(), (text))
                    </tr>
                }
            </tbody>
        </table>
    })
}

#[component]
pub async fn row(#[default] class: &str, child: Child<'_>) -> Result<impl View> {
    Ok(view! {
        <tr if !class.is_empty() { class=(class) } role="row">(child)</tr>
    })
}

/// A heading between groups of rows; `span` is the column count.
#[component]
pub async fn section_row(label: &str, span: usize) -> Result<impl View> {
    let span = span.to_string();
    Ok(view! {
        <tr class="section" role="row">
            cell(colspan: span.as_str(), <h2 class="label">(label)</h2>)
        </tr>
    })
}

#[component]
pub async fn col_head(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <th scope="col" role="columnheader">(child)</th> })
}

/// A cell; `class` is its look (`mono`), `colspan` the columns it spans.
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
