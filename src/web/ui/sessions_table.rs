//! A table of work sessions: when each ran, how long, and its note. The
//! Organization report and the task page show the same table, with
//! different columns.

use super::table::{AddRow, cell, data_table, row};
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
        row(
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
        )
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
/// marked next to its duration. `add` is where the first row leads, when the
/// table has one ("New session…": the pop-up). `empty` is the line shown
/// when there are no sessions. It is a `data_table` (see `table.rs`).
#[component]
pub async fn sessions_table(
    lines: &[SessionLine<'_>],
    caption: &str,
    #[default] show_end: bool,
    #[default] add: Option<String>,
    #[default] empty: Option<&str>,
) -> Result<impl View> {
    let dated = lines.iter().any(|l| l.date.is_some());
    let mut columns = Vec::new();
    if dated {
        columns.push("Date");
    }
    columns.push("Start");
    if show_end {
        columns.push("End");
    }
    columns.extend(["Duration", "Note"]);
    let add = add.map(|href| AddRow::link(href, "New session\u{2026}"));
    Ok(view! {
        data_table(caption: caption, columns: &columns, class: "sessions inline", add: add, empty: empty,
            for l in lines.iter() {
                session_row(l: l, dated: dated, show_end: show_end)
            }
        )
    })
}
