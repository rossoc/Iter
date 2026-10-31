//! The daily notes of an organization or project: the Notes tab, a note's page (`/{org,project}/{id}/notes/{date}`) and the today's-note button.

use super::Id;
use super::crumbs::trail;
use super::forms;
use super::load::{Parents, parents_of};
use super::sections::NoteTab;
use super::ui::breadcrumb::Crumb;
use super::ui::empty_line::empty_line;
use super::ui::field::textarea;
use super::ui::form::form_shell;
use super::ui::form_error::FormError;
use super::ui::frame::frame;
use super::ui::notes::{Month, NoteItem, NoteLink, note_nav, notes_tab};
use super::ui::page_header::{Kicker, page_header};
use super::ui::prose::description;
use super::ui::tabs::{Tab, tabs};
use super::url::{
    edit_url, note_tab_url, note_url, notes_tab_url, notes_today_url, org_url, project_url,
};
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

#[query_params(error = bad_request)]
pub struct NoteQuery {
    pub tab: Option<String>,
}

#[derive(Deserialize)]
pub struct NoteForm {
    text: String,
}

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
    let line = super::notes::first_line(&note.body);
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
                iso: crate::reporting::fmt_date(note.date),
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

pub struct NotePage {
    owner: String,
    crumbs: Vec<Crumb>,
    note: Note,
    prev: Option<NoteLink>,
    next: Option<NoteLink>,
    base: String,
    tab: NoteTab,
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

struct Owner {
    name: String,
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

    /// Saves `text`, then goes back to the Info tab; a refused text comes back as the editor with the error.
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

    /// Redirects (an `Err`, as every redirect here) to today's note, creating it first.
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

#[page(POST "/project/{id}/notes/{date}")]
async fn project_note_save(cx: &Cx, Form(form): Form<NoteForm>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let day = day_of(cx)?;
    let page = Owner::project(&open_db()?, id)?.save(day, &form.text)?;
    Ok(view! { note_screen(page: &page) })
}

#[page(POST "/org/{id}/notes/{date}")]
async fn org_note_save(cx: &Cx, Form(form): Form<NoteForm>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let day = day_of(cx)?;
    let page = Owner::org(&open_db()?, id)?.save(day, &form.text)?;
    Ok(view! { note_screen(page: &page) })
}

#[route(POST "/project/{id}/notes/today")]
async fn project_today(cx: &Cx) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    Owner::project(&open_db()?, id)?.open_today()
}

#[route(POST "/org/{id}/notes/today")]
async fn org_today(cx: &Cx) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    Owner::org(&open_db()?, id)?.open_today()
}

fn note_tabs(base: &str, day: NaiveDate, current: NoteTab) -> Vec<Tab> {
    super::sections::tabs_for(
        current,
        NoteTab::ALL.map(|t| (t, Tab::link(t.label(), note_tab_url(base, day, t)))),
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
                    textarea(name: TEXT, label: "Note", value: text.as_str(), hide_label: true)
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
