//! Daily notes: the list of a Notes tab (grouped by month, the latest day
//! first) and the way between the days of a note's page.

use super::empty_line::empty_line;
use super::{PLUS, icon};
use topcoat::{
    Result,
    view::{View, component, view},
};

/// One row of the list, ready to show (see `web/daily_notes.rs`). `title` is
/// there only when the note has one of its own, as the day is shown anyway.
pub struct NoteItem<'a> {
    pub href: String,
    pub day: String,
    pub iso: String,
    pub title: Option<&'a str>,
    pub preview: String,
}

/// The rows of one month.
pub struct Month<'a> {
    pub label: String,
    pub items: Vec<NoteItem<'a>>,
}

/// The Notes tab. `today` is where the button that opens today's note posts;
/// `set_path` is where to go to name the folder, shown when there is none.
#[component]
pub async fn notes_tab(
    months: &[Month<'_>],
    configured: bool,
    #[into] today: String,
    #[into] set_path: String,
) -> Result<impl View> {
    Ok(view! {
        if !configured {
            empty_line(<span>"No notes folder yet. " <a class="u" href=(set_path.as_str())>"Set a notes path"</a> "."</span>)
        } else {
            // the first row, as of any list: it adds today's note (or opens it)
            <ul class="notes new">
                <li class="add">
                    <form method="post" action=(today.as_str())>
                        <button type="submit" class="add-link">(icon(PLUS)) "Today\u{2019}s note\u{2026}"</button>
                    </form>
                </li>
            </ul>
            if months.is_empty() {
                empty_line("No notes yet.")
            }
            for month in months.iter() {
                <section class="notes-month">
                    <h2 class="label">(month.label.as_str())</h2>
                    <ul class="notes">
                        for n in month.items.iter() {
                            <li>
                                <a href=(n.href.as_str())>
                                    <time class="note-day" datetime=(n.iso.as_str())>(n.day.as_str())</time>
                                    if let Some(title) = n.title {
                                        <span class="note-title">(title)</span>
                                    }
                                    <span class="note-preview">(n.preview.as_str())</span>
                                </a>
                            </li>
                        }
                    </ul>
                </section>
            }
        }
    })
}

/// A neighbouring day of a note: where it is, and what to call it.
pub struct NoteLink {
    pub href: String,
    pub day: String,
    pub title: String,
}

/// The links to the previous (older) and the next (newer) note.
#[component]
pub async fn note_nav(prev: Option<&NoteLink>, next: Option<&NoteLink>) -> Result<impl View> {
    Ok(view! {
        <nav class="note-nav" aria-label="Notes">
            if let Some(p) = prev {
                <a class="prev" href=(p.href.as_str()) rel="prev">
                    <span class="label">"\u{2190} Previous"</span>
                    <span>(p.day.as_str()) " \u{b7} " (p.title.as_str())</span>
                </a>
            } else {
                <span></span>
            }
            if let Some(n) = next {
                <a class="next" href=(n.href.as_str()) rel="next">
                    <span class="label">"Next \u{2192}"</span>
                    <span>(n.day.as_str()) " \u{b7} " (n.title.as_str())</span>
                </a>
            }
        </nav>
    })
}
