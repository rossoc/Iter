//! A table of work sessions: when each ran, how long, and its note. The
//! Organization report and the task page show the same table, with
//! different columns.

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
                <td class="mono" role="cell">(l.date.as_deref().unwrap_or_default())</td>
            }
            <td class="mono" role="cell">moment(m: &l.start)</td>
            if show_end {
                <td class="mono end" role="cell">
                    if let Some(end) = &l.end {
                        moment(m: end)
                    } else {
                        <span class="live" aria-hidden="true"></span><span class="ongoing">"ongoing"</span>
                    }
                </td>
            }
            <td class="mono" role="cell">
                (l.duration.as_ref())
                // Still open: its duration counts up to now.
                if l.end.is_none() && !show_end {
                    <span class="live"><span class="sr">" (still running)"</span></span>
                }
            </td>
            <td class="note" role="cell">(l.note.as_deref().unwrap_or_default())</td>
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
/// semantics: on narrow screens the rows are laid out as boxes, and browsers
/// then drop a table's semantics. The caller shows its own empty state
/// instead of an empty table.
#[component]
pub async fn sessions_table(
    lines: &[SessionLine<'_>],
    caption: &str,
    #[default] show_end: bool,
) -> Result<impl View> {
    let dated = lines.iter().any(|l| l.date.is_some());
    Ok(view! {
        <table class="sessions" role="table">
            <caption class="sr">(caption)</caption>
            <thead role="rowgroup">
                <tr role="row">
                    if dated {
                        <th scope="col" role="columnheader">"Date"</th>
                    }
                    <th scope="col" role="columnheader">"Start"</th>
                    if show_end {
                        <th scope="col" role="columnheader">"End"</th>
                    }
                    <th scope="col" role="columnheader">"Duration"</th>
                    <th scope="col" role="columnheader">"Note"</th>
                </tr>
            </thead>
            <tbody role="rowgroup">
                for l in lines.iter() {
                    session_row(l: l, dated: dated, show_end: show_end)
                }
            </tbody>
        </table>
    })
}
