//! The daily notes of an organization or project: the Notes tab, a note's page (`/{org,project}/{id}/notes/{date}`) and the today's-note button.

use crate::{
    forms,
    load::{Parents, parents_of},
    parts::all::*,
    prelude::*,
};
use chrono::NaiveDate;
use iter_core::{
    db::Db,
    models::{Organization, Project},
    notes::{self as journal, Note},
};
use serde::Deserialize;
use std::path::PathBuf;

path_param!(date: String, error = bad_request);

#[query_params(error = bad_request)]
pub(crate) struct NoteQuery {
    pub tab: Option<String>,
}

#[derive(Deserialize)]
pub struct NoteForm {
    text: String,
}

const TEXT: &str = "text";

pub(crate) struct NotePage {
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

/// How a route finds its owner: [`Owner::project`] or [`Owner::org`].
type Find = fn(&Db, i64) -> topcoat::Result<Owner>;

/// The owner named by the request's `{id}`.
fn owner_of(cx: &Cx, find: Find) -> Result<Owner> {
    find(&fresh_db()?, path_id(cx)?)
}

fn show(cx: &Cx, find: Find) -> Result<NotePage> {
    let day = day_of(cx)?;
    let tab = NoteTab::of(query_params::<NoteQuery>(cx)?.tab.as_deref());
    owner_of(cx, find)?.note_page(day, tab)
}

fn save_text(cx: &Cx, find: Find, text: &str) -> Result<NotePage> {
    let day = day_of(cx)?;
    owner_of(cx, find)?.save(day, text)
}

#[page("/project/{id}/notes/{date}")]
pub(super) async fn project_note(cx: &Cx) -> Result<impl View> {
    let page = show(cx, Owner::project)?;
    Ok(view! { note_screen(page: &page) })
}

#[page("/org/{id}/notes/{date}")]
pub(super) async fn org_note(cx: &Cx) -> Result<impl View> {
    let page = show(cx, Owner::org)?;
    Ok(view! { note_screen(page: &page) })
}

#[page(POST "/project/{id}/notes/{date}")]
pub(super) async fn project_note_save(cx: &Cx, Form(form): Form<NoteForm>) -> Result<impl View> {
    let page = save_text(cx, Owner::project, &form.text)?;
    Ok(view! { note_screen(page: &page) })
}

#[page(POST "/org/{id}/notes/{date}")]
pub(super) async fn org_note_save(cx: &Cx, Form(form): Form<NoteForm>) -> Result<impl View> {
    let page = save_text(cx, Owner::org, &form.text)?;
    Ok(view! { note_screen(page: &page) })
}

#[route(POST "/project/{id}/notes/today")]
pub(super) async fn project_today(cx: &Cx) -> Result<Response> {
    owner_of(cx, Owner::project)?.open_today()
}

#[route(POST "/org/{id}/notes/today")]
pub(super) async fn org_today(cx: &Cx) -> Result<Response> {
    owner_of(cx, Owner::org)?.open_today()
}

fn note_tabs(base: &str, day: NaiveDate, current: NoteTab) -> Vec<Tab> {
    crate::parts::sections::tabs_for(
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
    let iso = iter_core::reporting::fmt_date(note.date);
    let parts = [note.title.as_str(), "Notes", owner.as_str()];
    let items = note_tabs(base, note.date, *tab);
    let here = note_tab_url(base, note.date, NoteTab::Info);
    Ok(view! {
        frame(
            title: &parts,
            error: error.as_ref(),
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
                    prose_text(text: &note.body)
                }
                note_nav(prev: prev.as_ref(), next: next.as_ref())
            }
        )
    })
}
