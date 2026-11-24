//! A task's note or sketch on its own page (`/task/{id}/notes/{name}`, `/task/{id}/sketches/{name}`),
//! built like an organization's or project's daily note. A note has an Edit tab that saves it;
//! a sketch is the agents' and read-only. Only a name among the folder's own files
//! (`TaskDir::files`) opens: anything else, `..` and `/` included, is a 404.

use crate::pages::site::daily_notes::{NoteForm, NoteQuery, TEXT, note_editor};
use crate::{parts::all::*, prelude::*};
use iter_core::agent::taskdir::{self, TaskDir};
use std::path::PathBuf;

path_param!(name: String, error = bad_request);

pub(crate) struct FilePage {
    task_id: i64,
    task: String,
    kind: TaskFile,
    name: String,
    crumbs: Vec<Crumb>,
    tab: NoteTab,
    text: String,
    error: Option<FormError>,
}

/// The file `name` of the task's `kind` folder, found only among the folder's own files.
fn find(dir: &TaskDir, kind: TaskFile, name: &str) -> Option<PathBuf> {
    let folder = match kind {
        TaskFile::Note => dir.notes_dir(),
        TaskFile::Sketch => dir.sketches_dir(),
    };
    TaskDir::files(&folder)
        .into_iter()
        .find(|f| f.name == name)
        .map(|f| f.path)
        // a link in the folder could lead outside it: only a plain file is opened
        .filter(|path| std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_file()))
}

/// The task of the request's `{id}` and the path of its file `{name}`; either missing is a 404.
fn open(cx: &Cx, kind: TaskFile) -> Result<(FilePage, PathBuf)> {
    let id = path_id(cx)?;
    let name = path_param::<Name>(cx)?.clone();
    let db = fresh_db()?;
    let (task, project) = crate::load::task_with_project(&db, id)?;
    let dir = taskdir::of(&db, &project, &task).ok().ok_or_not_found()?;
    let path = find(&dir, kind, &name).ok_or_not_found()?;
    let org = db.org_of(&project)?;
    let mut crumbs = trail(&org, Some(&project), Crumb::link("Task", task_url(id)));
    crumbs.push(Crumb::link("Notes", notes_tab_url(&task_url(id))));
    crumbs.push(Crumb::here(name.clone()));
    let tab = match kind {
        TaskFile::Note => NoteTab::of(query_params::<NoteQuery>(cx)?.tab.as_deref()),
        TaskFile::Sketch => NoteTab::Info,
    };
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let page = FilePage {
        task_id: id,
        task: task.name.clone(),
        kind,
        name,
        crumbs,
        tab,
        text,
        error: None,
    };
    Ok((page, path))
}

#[page("/task/{id}/notes/{name}")]
pub(super) async fn note(cx: &Cx) -> Result<impl View> {
    let (page, _) = open(cx, TaskFile::Note)?;
    Ok(view! { file_screen(page: &page) })
}

#[page("/task/{id}/sketches/{name}")]
pub(super) async fn sketch(cx: &Cx) -> Result<impl View> {
    let (page, _) = open(cx, TaskFile::Sketch)?;
    Ok(view! { file_screen(page: &page) })
}

/// Saves the note and goes back to it; a refused write comes back as the editor with the error.
/// It never makes a note: that is `POST /task/{id}/notes`.
#[page(POST "/task/{id}/notes/{name}")]
pub(super) async fn note_save(cx: &Cx, Form(form): Form<NoteForm>) -> Result<impl View> {
    let (mut page, path) = open(cx, TaskFile::Note)?;
    let text = crate::forms::text(&form.text);
    // no `create`: a note removed since it was found is not made again
    let written = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, text.as_bytes()));
    match written {
        Ok(()) => Err(see_other(task_file_url(page.task_id, page.kind, &page.name)).into()),
        Err(e) => {
            page.error = Some(FormError::new(&e.into(), Some(TEXT)));
            page.tab = NoteTab::Edit;
            page.text = text;
            Ok(view! { file_screen(page: &page) })
        }
    }
}

#[component]
async fn file_screen(page: &FilePage) -> Result<impl View> {
    let FilePage {
        task_id,
        task,
        kind,
        name,
        crumbs,
        tab,
        text,
        error,
    } = page;
    let title = first_line(text);
    let title = if title.is_empty() {
        name.as_str()
    } else {
        title
    };
    let parts = [name.as_str(), "Notes", task.as_str()];
    let items: Vec<Tab> = match kind {
        TaskFile::Note => crate::parts::sections::tabs_for(
            *tab,
            NoteTab::ALL.map(|t| {
                (
                    t,
                    Tab::link(t.label(), task_file_tab_url(*task_id, *kind, name, t)),
                )
            }),
        ),
        TaskFile::Sketch => Vec::new(),
    };
    let here = task_file_url(*task_id, *kind, name);
    Ok(view! {
        frame(
            title: &parts,
            error: error.as_ref(),
            page_header(title: title, kicker: Kicker::Crumbs(crumbs))
            if !items.is_empty() {
                tabs(label: "Note sections", items: &items)
            }
            if *tab == NoteTab::Edit {
                note_editor(here: here.as_str(), error: error.as_ref(), text: text.as_str())
            } else if text.trim().is_empty() {
                empty_line("Nothing written yet.")
            } else {
                prose_text(text: text)
            }
        )
    })
}
