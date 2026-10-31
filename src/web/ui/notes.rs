//! Daily notes: the Notes tab list (by month, latest day first) and links between days.

use super::empty_line::empty_line;
use super::table::{AddRow, cell, data_table, row, section_row};
use topcoat::{
    Result,
    view::{View, component, view},
};

/// One row of the list; `title` only when the note has its own.
pub struct NoteItem<'a> {
    pub href: String,
    pub day: String,
    pub iso: String,
    pub title: Option<&'a str>,
    pub preview: String,
}

pub struct Month<'a> {
    pub label: String,
    pub items: Vec<NoteItem<'a>>,
}

/// The Notes tab: a headerless `data_table` whose first row adds today's note
/// (`today` is where it posts); `set_path` leads to naming the folder, shown when none.
#[component]
pub async fn notes_tab(
    months: &[Month<'_>],
    configured: bool,
    today: &str,
    set_path: &str,
) -> Result<impl View> {
    let add = AddRow::post(today, "Today\u{2019}s note\u{2026}");
    Ok(view! {
        if !configured {
            empty_line(<span>"No notes folder yet. " <a class="u" href=(set_path)>"Set a notes path"</a> "."</span>)
        } else {
            data_table(caption: "Notes", columns: &["Day", "Note"], class: "notes inline", headed: false,
                add: Some(add), empty: months.is_empty().then_some("No notes yet."),
                for month in months.iter() {
                    section_row(label: month.label.as_str(), span: 2)
                    for n in month.items.iter() {
                        row(
                            cell(class: "note-day",
                                <a class="cover" href=(n.href.as_str())>
                                    <time datetime=(n.iso.as_str())>(n.day.as_str())</time>
                                </a>
                            )
                            cell(class: "note-text",
                                if let Some(title) = n.title {
                                    <span class="note-title">(title)</span>
                                }
                                <span class="note-preview">(n.preview.as_str())</span>
                            )
                        )
                    }
                }
            )
        }
    })
}

pub struct NoteLink {
    pub href: String,
    pub day: String,
    pub title: String,
}

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
