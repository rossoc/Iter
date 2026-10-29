//! The daily notes of an organization or a project: the Notes tab (the
//! list), a note's page (`/{org,project}/{id}/notes/{date}`) and the button
//! that opens today's note. The files and their reading are `crate::notes`;
//! this is the routes and the view.

use super::Id;
use super::crumbs::trail;
use super::load::{Parents, parents_of};
use super::ui::breadcrumb::Crumb;
use super::ui::empty_line::empty_line;
use super::ui::frame::frame;
use super::ui::notes::{Month, NoteItem, NoteLink, note_nav, notes_tab};
use super::ui::page_header::{Kicker, page_header};
use super::ui::prose::description;
use super::url::{edit_url, note_url, notes_tab_url, notes_today_url, org_url, project_url};
use super::{now, open_db};
use crate::db::Db;
use crate::models::Organization;
use crate::notes::{self as journal, Note};
use chrono::{Datelike, NaiveDate};
use std::path::PathBuf;
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        error::{RouterErrorExt, see_other},
        page, path_param,
        response::Response,
        route,
    },
    view::{View, component, view},
};

path_param!(date: String, error = bad_request);

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder
        .page(project_note)
        .route(project_today)
        .page(org_note)
        .route(org_today)
}

/// What a page knows of its notes: the folder (none when no path is set),
/// and the notes themselves only on the Notes tab -- the other tabs need
/// the count, for the tab's badge.
#[derive(Default)]
pub struct NotesTab {
    pub dir: Option<PathBuf>,
    pub notes: Vec<Note>,
    pub count: usize,
}

impl NotesTab {
    /// Reads the notes in `dir` when `full` (the Notes tab), else counts
    /// them.
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

    /// The tab's badge: none when the page has no folder.
    pub fn badge(tab: Option<&NotesTab>) -> Option<usize> {
        tab.filter(|t| t.dir.is_some()).map(|t| t.count)
    }
}

/// `clip` as a one-line preview.
fn preview(note: &Note) -> String {
    const MAX: usize = 140;
    let line = super::notes::first_line(&note.body);
    match line.char_indices().nth(MAX) {
        Some((at, _)) => format!("{}\u{2026}", &line[..at]),
        None => line.to_string(),
    }
}

/// The rows of `notes` (latest first) for the page at `base`, a month each.
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
                iso: crate::reporting::fmt_date(note.date),
                title: own.then_some(note.title.as_str()),
                preview: preview(note),
            });
        }
    }
    months.into_iter().map(|(_, _, m)| m).collect()
}

/// The Notes tab of the page at `base`.
#[component]
pub async fn notes_section(base: &str, notes: &NotesTab) -> Result<impl View> {
    let months = months(base, &notes.notes);
    Ok(view! {
        notes_tab(months: &months, configured: notes.dir.is_some(), today: notes_today_url(base), set_path: edit_url(base))
    })
}

/// A note's page, ready to show.
pub struct NotePage {
    /// The name of the organization or project the notes are of.
    owner: String,
    crumbs: Vec<Crumb>,
    note: Note,
    prev: Option<NoteLink>,
    next: Option<NoteLink>,
}

fn dir_or_404(dir: Option<PathBuf>) -> topcoat::Result<PathBuf> {
    Ok(dir.ok_or_not_found()?)
}

fn day_of(cx: &Cx) -> topcoat::Result<NaiveDate> {
    let text = path_param::<Date>(cx)?;
    Ok(journal::parse_date(text).ok_or_not_found()?)
}

fn project_dir(db: &Db, id: i64) -> topcoat::Result<(Parents, PathBuf)> {
    let parents = parents_of(db, id)?;
    let dir = dir_or_404(journal::project_dir(&parents.project, &parents.org))?;
    Ok((parents, dir))
}

fn org_dir(db: &Db, id: i64) -> topcoat::Result<(Organization, PathBuf)> {
    let org = db.get::<Organization>(id)?.ok_or_not_found()?;
    let dir = dir_or_404(journal::org_dir(&org))?;
    Ok((org, dir))
}

fn day_label(day: NaiveDate) -> String {
    day.format("%-d %B %Y").to_string()
}

fn link(base: &str, note: &Note) -> NoteLink {
    NoteLink {
        href: note_url(base, note.date),
        day: note.date.format("%-d %b %Y").to_string(),
        title: note.title.clone(),
    }
}

/// The note of `day` in `dir` under the page at `base`; `crumbs` lead to the
/// Notes tab, the page's own crumb is added. No such note is a 404.
fn load_note(
    dir: &std::path::Path,
    day: NaiveDate,
    base: &str,
    owner: &str,
    mut crumbs: Vec<Crumb>,
) -> topcoat::Result<NotePage> {
    let notes = journal::list(dir);
    let (prev, next) = journal::neighbours(&notes, day);
    let (prev, next) = (prev.map(|n| link(base, n)), next.map(|n| link(base, n)));
    let note = notes
        .into_iter()
        .find(|n| n.date == day)
        .ok_or_not_found()?;
    crumbs.push(Crumb::here(day_label(day)));
    Ok(NotePage {
        owner: owner.to_string(),
        crumbs,
        note,
        prev,
        next,
    })
}

#[page("/project/{id}/notes/{date}")]
async fn project_note(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let day = day_of(cx)?;
    let (Parents { project, org }, dir) = project_dir(&open_db()?, id)?;
    let base = project_url(id);
    let crumbs = trail(
        &org,
        Some(&project),
        Crumb::link("Notes", notes_tab_url(&base)),
    );
    let page = load_note(&dir, day, &base, &project.name, crumbs)?;
    Ok(view! { note_screen(page: &page) })
}

#[page("/org/{id}/notes/{date}")]
async fn org_note(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let day = day_of(cx)?;
    let (org, dir) = org_dir(&open_db()?, id)?;
    let base = org_url(id);
    let crumbs = trail(
        &Some(org.clone()),
        None,
        Crumb::link("Notes", notes_tab_url(&base)),
    );
    let page = load_note(&dir, day, &base, &org.name, crumbs)?;
    Ok(view! { note_screen(page: &page) })
}

/// `POST /project/{id}/notes/today`: makes today's note, then shows it.
#[route(POST "/project/{id}/notes/today")]
async fn project_today(cx: &Cx) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let (_, dir) = project_dir(&open_db()?, id)?;
    open_today(&dir, &project_url(id))
}

/// `POST /org/{id}/notes/today`: as the project's.
#[route(POST "/org/{id}/notes/today")]
async fn org_today(cx: &Cx) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let (_, dir) = org_dir(&open_db()?, id)?;
    open_today(&dir, &org_url(id))
}

/// Makes sure today's note exists, then redirects to it: always an `Err`,
/// as every redirect here is.
fn open_today(dir: &std::path::Path, base: &str) -> Result<Response> {
    let today = now().date();
    journal::create(dir, today)?;
    Err(see_other(note_url(base, today)).into())
}

#[component]
async fn note_screen(page: &NotePage) -> Result<impl View> {
    let NotePage {
        owner,
        crumbs,
        note,
        prev,
        next,
    } = page;
    let when = note.date.format("%A %-d %B %Y").to_string();
    let iso = crate::reporting::fmt_date(note.date);
    let parts = [note.title.as_str(), "Notes", owner.as_str()];
    Ok(view! {
        frame(
            title: &parts,
            page_header(title: note.title.as_str(), kicker: Kicker::Crumbs(crumbs))
            <p class="note-date"><time datetime=(iso.as_str())>(when.as_str())</time></p>
            if note.body.is_empty() {
                empty_line("Nothing written on this day.")
            } else {
                description(text: &note.body)
            }
            note_nav(prev: prev.as_ref(), next: next.as_ref())
        )
    })
}
