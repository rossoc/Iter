//! A table of work sessions: when each ran, how long, and its note. The
//! Organization report and the task page show the same table, with
//! different columns.

use super::button::add_link;
use super::table::{cell, col_head};
use std::borrow::Cow;
use topcoat::{
    Result,
    view::{View, component, view},
};

/// A point in time as shown, with its machine-readable form for `<time>`
/// when the source has one.
pub struct Moment<'a> {
    pub text: Cow<'a, str>,
    /// `2025-03-03T09:00`.
    pub at: Option<String>,
}

/// One row, ready to show (see `web/session_lines.rs` for the mapping from
/// the models and the report).
pub struct SessionLine<'a> {
    /// Only for a table spanning several days.
    pub date: Option<Cow<'a, str>>,
    pub start: Moment<'a>,
    /// `None`: the session is still open.
    pub end: Option<Moment<'a>>,
    pub duration: Cow<'a, str>,
    pub note: Option<Cow<'a, str>>,
}

/// One row: the cells the table's columns ask for.
#[component]
async fn session_row(l: &SessionLine<'_>, dated: bool, show_end: bool) -> Result<impl View> {
    Ok(view! {
        <tr role="row">
            if dated {
                cell(class: "mono", (l.date.as_deref().unwrap_or_default()))
            }
            cell(class: "mono", moment(m: &l.start))
            if show_end {
                cell(class: "mono end",
                    if let Some(end) = &l.end {
                        moment(m: end)
                    } else {
                        <span class="live" aria-hidden="true"></span><span class="ongoing">"ongoing"</span>
                    }
                )
            }
            cell(class: "mono",
                (l.duration.as_ref())
                // Still open: its duration counts up to now.
                if l.end.is_none() && !show_end {
                    <span class="live"><span class="sr">" (still running)"</span></span>
                }
            )
            cell(class: "note", (l.note.as_deref().unwrap_or_default()))
        </tr>
    })
}

/// A moment: a `<time datetime>` when its machine form is known.
#[component]
async fn moment(m: &Moment<'_>) -> Result<impl View> {
    Ok(view! {
        if let Some(at) = &m.at {
            <time datetime=(at.as_str())>(m.text.as_ref())</time>
        } else {
            (m.text.as_ref())
        }
    })
}

/// `caption` is read by screen readers only. `show_end` adds the End column
/// (an open session reads "ongoing" there); without it the open session is
/// marked next to its duration. The `role`s repeat the native table
/// semantics (see `table.rs`). `add` is where the first row leads, when the
/// table has one ("New session…"). The caller shows its own empty state
/// instead of an empty table, unless the table has that row.
#[component]
pub async fn sessions_table(
    lines: &[SessionLine<'_>],
    caption: &str,
    #[default] show_end: bool,
    #[default] add: Option<String>,
) -> Result<impl View> {
    let dated = lines.iter().any(|l| l.date.is_some());
    let columns = (3 + usize::from(dated) + usize::from(show_end)).to_string();
    Ok(view! {
        <table class="sessions" role="table">
            <caption class="sr">(caption)</caption>
            <thead role="rowgroup">
                <tr role="row">
                    if dated {
                        col_head("Date")
                    }
                    col_head("Start")
                    if show_end {
                        col_head("End")
                    }
                    col_head("Duration")
                    col_head("Note")
                </tr>
            </thead>
            <tbody role="rowgroup">
                if let Some(href) = &add {
                    <tr class="add" role="row">
                        cell(colspan: columns.as_str(), add_link(href: href, label: "New session\u{2026}"))
                    </tr>
                }
                for l in lines.iter() {
                    session_row(l: l, dated: dated, show_end: show_end)
                }
            </tbody>
        </table>
    })
}
