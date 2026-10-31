//! The daily notes of an organization or a project: the Notes tab (the
//! list), a note's page (`/{org,project}/{id}/notes/{date}`) and the button
//! that opens today's note. The files and their reading are `crate::notes`;
//! this is the routes and the view.

use super::Id;
use super::crumbs::trail;
use super::load::{Parents, parents_of};
use super::ui::breadcrumb::Crumb;
use super::forms;
use super::sections::NoteTab;
use super::ui::empty_line::empty_line;
use super::ui::field::textarea;
use super::ui::form::form_shell;
use super::ui::form_error::FormError;
use super::ui::frame::frame;
use super::ui::tabs::{Tab, tabs};
use super::ui::notes::{Month, NoteItem, NoteLink, note_nav, notes_tab};
use super::ui::page_header::{Kicker, page_header};
use super::ui::prose::description;
use super::url::{edit_url, note_tab_url, note_url, notes_tab_url, notes_today_url, org_url, project_url};
use super::{now, open_db};
use crate::db::Db;
use crate::models::{Organization, Project};
use crate::notes::{self as journal, Note};
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;
use std::path::PathBuf;
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        content::Form,
        error::{RouterErrorExt, see_other},
        page, path_param, query_params,
        response::Response,
        route,
    },
    view::{View, component, view},
};

path_param!(date: String, error = bad_request);

/// The note page's query: which tab is open (`?tab=`, see [`NoteTab`]).
#[query_params(error = bad_request)]
pub struct NoteQuery {
    pub tab: Option<String>,
}

/// The editor's submission: the note's whole file text.
#[derive(Deserialize)]
pub struct NoteForm {
    text: String,
}

/// The name of the editor's textarea (and of the submitted field).
const TEXT: &str = "text";

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder
        .page(project_note)
        .page(project_note_save)
        .route(project_today)
        .page(org_note)
        .page(org_note_save)
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
    /// The page's own URL base (the owner's), for the tabs and the form.
    base: String,
    tab: NoteTab,
    /// The file's text, as the editor starts out (or as typed, if refused).
    text: String,
    error: Option<FormError>,
}

fn dir_or_404(dir: Option<PathBuf>) -> topcoat::Result<PathBuf> {
    Ok(dir.ok_or_not_found()?)
}

fn day_of(cx: &Cx) -> topcoat::Result<NaiveDate> {
    let text = path_param::<Date>(cx)?;
    Ok(journal::parse_date(text).ok_or_not_found()?)
}

/// Whose notes a page shows: a project (with the organization it is in) or
/// an organization, with the folder they are in and the page they belong to.
struct Owner {
    name: String,
    /// The owner's own page, which the notes' URLs hang from.
    base: String,
    dir: PathBuf,
    org: Option<Organization>,
    project: Option<Project>,
}

impl Owner {
    /// Project `id`. No such project, or one with no notes folder, is a 404.
    fn project(db: &Db, id: i64) -> topcoat::Result<Owner> {
        let Parents { project, org } = parents_of(db, id)?;
        let dir = dir_or_404(journal::project_dir(&project, &org))?;
        Ok(Owner {
            name: project.name.clone(),
            base: project_url(id),
            dir,
            org,
            project: Some(project),
        })
    }

    /// Organization `id`, with the same 404s.
    fn org(db: &Db, id: i64) -> topcoat::Result<Owner> {
        let org = db.get::<Organization>(id)?.ok_or_not_found()?;
        let dir = dir_or_404(journal::org_dir(&org))?;
        Ok(Owner {
            name: org.name.clone(),
            base: org_url(id),
            dir,
            org: Some(org),
            project: None,
        })
    }

    /// The crumbs up to the Notes tab.
    fn crumbs(&self) -> Vec<Crumb> {
        trail(
            &self.org,
            self.project.as_ref(),
            Crumb::link("Notes", notes_tab_url(&self.base)),
        )
    }

    /// The note of `day`, with its neighbours. No such note is a 404.
    fn note_page(&self, day: NaiveDate, tab: NoteTab) -> topcoat::Result<NotePage> {
        let notes = journal::list(&self.dir);
        let (prev, next) = journal::neighbours(&notes, day);
        let (prev, next) = (
            prev.map(|n| link(&self.base, n)),
            next.map(|n| link(&self.base, n)),
        );
        let note = notes
            .into_iter()
            .find(|n| n.date == day)
            .ok_or_not_found()?;
        let mut crumbs = self.crumbs();
        crumbs.push(Crumb::here(day_label(day)));
        Ok(NotePage {
            owner: self.name.clone(),
            crumbs,
            note,
            prev,
            next,
            base: self.base.clone(),
            tab,
            text: journal::raw(&self.dir, day).unwrap_or_default(),
            error: None,
        })
    }

    /// Saves the editor's `text` as the file of `day`, then goes back to
    /// the Info tab (an `Err`, as every redirect here is). A text that will
    /// not do comes back as the editor again, filled in, with the error.
    fn save(&self, day: NaiveDate, text: &str) -> topcoat::Result<NotePage> {
        let text = forms::text(text);
        match journal::save(&self.dir, day, &text) {
            Ok(()) => Err(see_other(note_tab_url(&self.base, day, NoteTab::Info)).into()),
            Err(e) => {
                let mut page = self.note_page(day, NoteTab::Edit)?;
                page.error = Some(FormError::new(&e, Some(TEXT)));
                page.text = text;
                Ok(page)
            }
        }
    }

    /// Makes sure today's note exists, then redirects to it: always an
    /// `Err`, as every redirect here is.
    fn open_today(&self) -> Result<Response> {
        let today = now().date();
        journal::create(&self.dir, today)?;
        Err(see_other(note_url(&self.base, today)).into())
    }
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

#[page("/project/{id}/notes/{date}")]
async fn project_note(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let day = day_of(cx)?;
    let tab = NoteTab::of(query_params::<NoteQuery>(cx)?.tab.as_deref());
    let page = Owner::project(&open_db()?, id)?.note_page(day, tab)?;
    Ok(view! { note_screen(page: &page) })
}

#[page("/org/{id}/notes/{date}")]
async fn org_note(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let day = day_of(cx)?;
    let tab = NoteTab::of(query_params::<NoteQuery>(cx)?.tab.as_deref());
    let page = Owner::org(&open_db()?, id)?.note_page(day, tab)?;
    Ok(view! { note_screen(page: &page) })
}

/// `POST /project/{id}/notes/{date}`: saves the editor.
#[page(POST "/project/{id}/notes/{date}")]
async fn project_note_save(cx: &Cx, Form(form): Form<NoteForm>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let day = day_of(cx)?;
    let page = Owner::project(&open_db()?, id)?.save(day, &form.text)?;
    Ok(view! { note_screen(page: &page) })
}

/// `POST /org/{id}/notes/{date}`: as the project's.
#[page(POST "/org/{id}/notes/{date}")]
async fn org_note_save(cx: &Cx, Form(form): Form<NoteForm>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let day = day_of(cx)?;
    let page = Owner::org(&open_db()?, id)?.save(day, &form.text)?;
    Ok(view! { note_screen(page: &page) })
}

/// `POST /project/{id}/notes/today`: makes today's note, then shows it.
#[route(POST "/project/{id}/notes/today")]
async fn project_today(cx: &Cx) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    Owner::project(&open_db()?, id)?.open_today()
}

/// `POST /org/{id}/notes/today`: as the project's.
#[route(POST "/org/{id}/notes/today")]
async fn org_today(cx: &Cx) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    Owner::org(&open_db()?, id)?.open_today()
}

/// The Info and Edit tabs of the note of `day` on the page at `base`.
fn note_tabs(base: &str, day: NaiveDate, current: NoteTab) -> Vec<Tab> {
    super::sections::tabs_for(
        current,
        NoteTab::ALL
            .map(|t| (t, Tab::link(t.label(), note_tab_url(base, day, t)))),
    )
}

#[component]
async fn note_screen(page: &NotePage) -> Result<impl View> {
    let NotePage {
        owner,
        crumbs,
        note,
        prev,
        next,
        base,
        tab,
        text,
        error,
    } = page;
    let when = note.date.format("%A %-d %B %Y").to_string();
    let iso = crate::reporting::fmt_date(note.date);
    let parts = [note.title.as_str(), "Notes", owner.as_str()];
    let items = note_tabs(base, note.date, *tab);
    let here = note_tab_url(base, note.date, NoteTab::Info);
    Ok(view! {
        frame(
            title: &parts,
            page_header(title: note.title.as_str(), kicker: Kicker::Crumbs(crumbs))
            tabs(label: "Note sections", items: &items)
            <p class="note-date"><time datetime=(iso.as_str())>(when.as_str())</time></p>
            if *tab == NoteTab::Edit {
                form_shell(action: here.as_str(), error: error.as_ref(), cancel: here.as_str(),
                    textarea(name: TEXT, label: "Note (markdown, with its front matter)", value: text.as_str())
                )
            } else {
                if note.body.is_empty() {
                    empty_line("Nothing written on this day.")
                } else {
                    description(text: &note.body)
                }
                note_nav(prev: prev.as_ref(), next: next.as_ref())
            }
        )
    })
}
