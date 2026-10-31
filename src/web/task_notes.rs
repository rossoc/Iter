//! Writing a note into a task's folder by hand: the pop-up over the Notes tab
//! (`?tab=notes&new=note`) and the route that stores it. The agents write the
//! folder's other files; this is how a person adds one.

use super::ui::field::{field, textarea};
use super::ui::form::form_shell;
use super::ui::modal::modal;
use super::url::{notes_tab_url, task_url};
use super::{Id, open_db};
use crate::agent::taskdir;
use serde::Deserialize;
use std::io::Write;
use std::path::Path;
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        content::Form,
        error::{bad_request, see_other},
        path_param,
        response::Response,
        route,
    },
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.route(add_note)
}

/// The New note pop-up: a name (it becomes the file's) and the text. The first
/// row of the Notes tab opens it.
#[component]
pub async fn note_modal(task_id: i64) -> Result<impl View> {
    let close = notes_tab_url(&task_url(task_id));
    let action = format!("{}/notes", task_url(task_id));
    Ok(view! {
        modal(id: "note-title", title: "New note", close: close.as_str(),
            form_shell(action: action.as_str(), error: None, cancel: close.as_str(), submit_label: "Add note",
                field(name: "name", label: "Name", value: "", required: true, autofocus: true, identifier: true)
                textarea(name: "text", label: "Note", value: "")
            )
        )
    })
}

#[derive(Deserialize)]
struct NoteForm {
    name: String,
    #[serde(default)]
    text: String,
}

/// The file stem a note's name makes: lowercase letters and digits, any run of
/// anything else one `-`. Never a path, so a name cannot leave the folder.
fn stem(name: &str) -> std::result::Result<String, String> {
    let mut out = String::new();
    for c in name.trim().chars().take(64) {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    match out.is_empty() {
        true => Err("a note needs a name".into()),
        false => Ok(out),
    }
}

/// Writes the note into `dir` as `<stem>.md`, refusing one that exists.
fn write(dir: &Path, form: &NoteForm) -> std::result::Result<(), String> {
    let stem = stem(&form.name)?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let text = super::forms::text(&form.text);
    let body = match text.trim().is_empty() {
        true => format!("# {}\n", form.name.trim()),
        false => text,
    };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(format!("{stem}.md")))
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => format!("a note called '{stem}' exists already"),
            _ => e.to_string(),
        })?;
    file.write_all(body.as_bytes()).map_err(|e| e.to_string())
}

/// `POST /task/{id}/notes`: stores the note and shows the tab.
#[route(POST "/task/{id}/notes")]
async fn add_note(cx: &Cx, Form(form): Form<NoteForm>) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let db = open_db()?;
    let (task, project) = super::load::task_with_project(&db, id)?;
    let dir = taskdir::of(&db, &project, &task)?;
    write(&dir.notes_dir(), &form).map_err(bad_request)?;
    Err(see_other(notes_tab_url(&task_url(id))).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_makes_a_safe_file_stem() {
        assert_eq!(stem("  Ideas for the API! ").unwrap(), "ideas-for-the-api");
        assert_eq!(stem("../../etc/passwd").unwrap(), "etc-passwd");
        assert_eq!(stem("a/b\\c").unwrap(), "a-b-c");
        assert!(stem("   ").is_err());
        assert!(stem("../..").is_err());
    }

    #[test]
    fn a_note_is_written_once() {
        let dir = crate::fixtures::scratch("note");
        let form = |name: &str, text: &str| NoteForm {
            name: name.into(),
            text: text.into(),
        };
        write(&dir, &form("Idea", "some\r\ntext")).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("idea.md")).unwrap(),
            "some\ntext"
        );
        assert!(
            write(&dir, &form("idea", "again"))
                .unwrap_err()
                .contains("exists already")
        );
        write(&dir, &form("Empty one", "")).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("empty-one.md")).unwrap(),
            "# Empty one\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
