//! The Notes tab of an organization or project: the folder's notes by month, and the one-line
//! preview of a note.

use crate::prelude::*;
use chrono::Datelike;
use iter_core::notes::{self as journal, Note};
use std::path::PathBuf;

pub fn first_line(text: &str) -> &str {
    text.lines()
        .map(|l| l.trim().trim_start_matches('#').trim())
        .find(|l| !l.is_empty())
        .unwrap_or_default()
}


/// Notes are read only on the Notes tab; the other tabs just count them (the badge).
#[derive(Default)]
pub struct NotesTab {
    pub dir: Option<PathBuf>,
    pub notes: Vec<Note>,
    pub count: usize,
}

impl NotesTab {
    pub fn load(dir: Option<PathBuf>, full: bool) -> NotesTab {
        let (notes, count) = match &dir {
            Some(dir) if full => {
                let notes = journal::list(dir);
                let count = notes.len();
                (notes, count)
            }
            Some(dir) => (Vec::new(), journal::count(dir)),
            None => (Vec::new(), 0),
        };
        NotesTab { dir, notes, count }
    }

    pub fn badge(tab: Option<&NotesTab>) -> Option<usize> {
        tab.filter(|t| t.dir.is_some()).map(|t| t.count)
    }
}

fn preview(note: &Note) -> String {
    const MAX: usize = 140;
    let line = first_line(&note.body);
    match line.char_indices().nth(MAX) {
        Some((at, _)) => format!("{}\u{2026}", &line[..at]),
        None => line.to_string(),
    }
}

fn months<'a>(base: &str, notes: &'a [Note]) -> Vec<Month<'a>> {
    let mut months: Vec<(i32, u32, Month<'a>)> = Vec::new();
    for note in notes {
        let (year, month) = (note.date.year(), note.date.month());
        if months
            .last()
            .is_none_or(|(y, m, _)| (*y, *m) != (year, month))
        {
            months.push((
                year,
                month,
                Month {
                    label: note.date.format("%B %Y").to_string(),
                    items: Vec::new(),
                },
            ));
        }
        let own = note.title != journal::default_title(note.date);
        if let Some((_, _, last)) = months.last_mut() {
            last.items.push(NoteItem {
                href: note_url(base, note.date),
                day: note.date.format("%a %-d %b").to_string(),
                iso: iter_core::reporting::fmt_date(note.date),
                title: own.then_some(note.title.as_str()),
                preview: preview(note),
            });
        }
    }
    months.into_iter().map(|(_, _, m)| m).collect()
}

#[component]
pub async fn notes_section(base: &str, notes: &NotesTab) -> Result<impl View> {
    let months = months(base, &notes.notes);
    let today = notes_today_url(base);
    let set_path = edit_url(base);
    Ok(view! {
        notes_tab(months: &months, configured: notes.dir.is_some(), today: &today, set_path: &set_path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_line_with_text_is_taken() {
        assert_eq!(
            first_line(
                "
## Plan
more"
            ),
            "Plan"
        );
        assert_eq!(
            first_line(
                "  
"
            ),
            ""
        );
    }
}
