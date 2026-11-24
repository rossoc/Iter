//! `iter note`: the daily notes of a project or an organization, a task's notes and sketches, and
//! the wiki's pages, from the command line, by kind or by address (`note_ref`). The web has the
//! same files under the Notes and Wiki tabs.

use crate::Run;
use crate::args::{DocCommand, NoteCommand, NotesScope, TaskScope};
use iter_core::app::App;
use iter_core::error::{IterError, Result};
use iter_core::locate::project_or_current;
use iter_core::models::{Organization, TaskRef};
use iter_core::note_ref::{self, Address, Doc};
use iter_core::notes;
use iter_core::text::{opens_front, split_front};
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

fn dir(app: &App, scope: &NotesScope) -> Result<PathBuf> {
    let db = &app.db;
    let found = match &scope.org {
        Some(name) => notes::org_dir(&db.resolve::<Organization>(name)?),
        None => {
            let project = project_or_current(db, scope.project.as_deref())?;
            notes::project_dir(&project, &db.org_of(&project)?)
        }
    };
    found.ok_or_else(|| {
        IterError::CommandFailed(
            "no notes folder: set `notes_path` on it or its organization".into(),
        )
    })
}

fn day(text: &str) -> Result<chrono::NaiveDate> {
    notes::parse_date(text)
        .ok_or_else(|| IterError::CommandFailed(format!("`{text}` is not a day (YYYY-MM-DD)")))
}

/// `iter note today` (and `iter note` alone): makes today's note if it is not there, prints its path.
pub(crate) fn today(app: &App, scope: &NotesScope) -> Result<()> {
    let today = app.now.date();
    let dir = dir(app, scope)?;
    notes::create(&dir, today)?;
    println!(
        "{}",
        dir.join(format!("{}.md", today.format("%Y-%m-%d")))
            .display()
    );
    Ok(())
}

impl Run for NoteCommand {
    fn run(&self, app: &App) -> Result<()> {
        let today = app.now.date();
        match self {
            Self::Today { scope } => self::today(app, scope),
            Self::List { scope } => {
                for note in notes::list(&dir(app, scope)?) {
                    println!("{}  {}", note.date.format("%Y-%m-%d"), note.title);
                }
                Ok(())
            }
            Self::Show { date, scope } => {
                let date = match date {
                    Some(d) => day(d)?,
                    None => today,
                };
                let note = notes::get(&dir(app, scope)?, date)
                    .ok_or_else(|| IterError::CommandFailed(format!("no note of {date}")))?;
                println!("{}", note.body);
                Ok(())
            }
            Self::Sketch { task, action } => in_task(app, Doc::Sketch, task, action),
            Self::Task { task, action } => in_task(app, Doc::TaskNote, task, action),
            Self::Idea { scope, action } => page(app, "idea", scope, action),
            Self::Concept { scope, action } => page(app, "concept", scope, action),
            Self::Entity { scope, action } => page(app, "entity", scope, action),
            Self::Synthesis { scope, action } => page(app, "synthesis", scope, action),
            Self::Read { address } => {
                let found = note_ref::locate(&app.db, &note_ref::parse(address)?, today)?;
                let file = found.file.ok_or_else(|| missing(address))?;
                print!(
                    "{}",
                    std::fs::read_to_string(&file).map_err(|_| missing(address))?
                );
                Ok(())
            }
            Self::Path { address } => {
                let found = note_ref::locate(&app.db, &note_ref::parse(address)?, today)?;
                println!("{}", found.file.unwrap_or(found.dir).display());
                Ok(())
            }
        }
    }
}

fn missing(what: &str) -> IterError {
    IterError::CommandFailed(format!("no document '{what}'"))
}

/// A task's sketches or notes: `--task`, else the current task.
fn in_task(app: &App, doc: Doc, scope: &TaskScope, action: &DocCommand) -> Result<()> {
    let mut address = Address::of(doc, None, None, None);
    if let Some(task) = &scope.task {
        match task.parse() {
            // `<project>/<task>`
            Ok(TaskRef::Title { project, title }) => {
                address.owners = vec![project];
                address.task = Some(title);
            }
            // `<project>/<task> #<id>`: the id keeps the title unambiguous
            Ok(TaskRef::Label(label)) => {
                address.task = Some(format!("{} #{}", label.title, label.id));
                address.owners = vec![label.project];
            }
            // an id
            Ok(TaskRef::Id(_)) | Err(_) => address.task = Some(task.clone()),
        }
    }
    act(app, address, action)
}

/// Wiki pages of `class` in the project's or organization's wiki.
fn page(app: &App, class: &str, scope: &NotesScope, action: &DocCommand) -> Result<()> {
    let address = Address::of(
        Doc::Page(class.to_string()),
        None,
        scope.project.as_deref(),
        scope.org.as_deref(),
    );
    act(app, address, action)
}

fn noun(doc: &Doc) -> String {
    match doc {
        Doc::Daily(_) => "note".into(),
        Doc::TaskNote => "task note".into(),
        Doc::Sketch => "sketch".into(),
        Doc::Page(class) => class.clone(),
    }
}

fn act(app: &App, mut address: Address, action: &DocCommand) -> Result<()> {
    let today = app.now.date();
    let doc = address.doc.clone().expect("kind commands name the kind");
    match action {
        DocCommand::List => {
            let found = note_ref::locate(&app.db, &address, today)?;
            for file in iter_core::agent::taskdir::TaskDir::files(&found.dir) {
                let text = std::fs::read_to_string(&file.path).unwrap_or_default();
                println!("{}  {}", file.name, summary(&text));
            }
            Ok(())
        }
        DocCommand::Read { name } | DocCommand::Path { name } => {
            address.name = Some(name.clone());
            let file = note_ref::locate(&app.db, &address, today)?
                .file
                .expect("a named document has a file");
            if matches!(action, DocCommand::Path { .. }) {
                println!("{}", file.display());
                return Ok(());
            }
            let text = std::fs::read_to_string(&file)
                .map_err(|_| IterError::CommandFailed(format!("no {} '{name}'", noun(&doc))))?;
            print!("{text}");
            Ok(())
        }
        DocCommand::New {
            name,
            message,
            description,
            tags,
            append,
            replace,
        } => {
            notes::stem(name)
                .ok_or_else(|| IterError::CommandFailed(format!("'{name}' is not a name")))?;
            address.name = Some(name.clone());
            let file = note_ref::locate(&app.db, &address, today)?
                .file
                .expect("a named document has a file");
            let how = match (append, replace) {
                (true, _) => How::Append,
                (_, true) => How::Replace,
                _ => How::New,
            };
            let Some(text) = text_of(message.as_deref())? else {
                println!("nothing written");
                return Ok(());
            };
            let exists = file.is_file();
            let content = match (&how, exists) {
                (How::Append, true) => text,
                _ => compose(&doc, name, &text, description.as_deref(), tags, app)?,
            };
            write(&file, &content, how)?;
            println!("{}", file.display());
            Ok(())
        }
    }
}

/// The text of a new document: `-m`, else standard input when something is piped (an agent's
/// case: its stdin is never a terminal), else the editor. `None`: the editor wrote nothing.
fn text_of(message: Option<&str>) -> Result<Option<String>> {
    if let Some(m) = message {
        return Ok(Some(m.to_string()));
    }
    let mut stdin = std::io::stdin();
    if !stdin.is_terminal() {
        let mut text = String::new();
        stdin.read_to_string(&mut text)?;
        return Ok(Some(text));
    }
    crate::editor::edit_text("")
}

/// The whole file for a new document. A wiki page gets the front matter `iter wiki check` wants
/// (`description`, `tags`, `updated`) unless the text brings its own; the rest is the text, or a
/// title when there is none.
fn compose(
    doc: &Doc,
    name: &str,
    text: &str,
    description: Option<&str>,
    tags: &[String],
    app: &App,
) -> Result<String> {
    let text = text.trim_end();
    let title = format!("# {}\n", name.trim());
    let body = match text.is_empty() {
        true => title.clone(),
        false => format!("{text}\n"),
    };
    let Doc::Page(_) = doc else {
        // a title, so `list` has something to say about it
        return Ok(match text.is_empty() || text.starts_with("# ") {
            true => body,
            false => format!("{title}\n{body}"),
        });
    };
    if opens_front(text) && split_front(text).front.is_some() {
        return Ok(body);
    }
    let (Some(description), false) = (
        description.filter(|d| !d.trim().is_empty()),
        tags.is_empty(),
    ) else {
        return Err(IterError::CommandFailed(
            "a wiki page needs --description and --tags (or front matter in its text)".into(),
        ));
    };
    let mut front = serde_yaml::Mapping::new();
    front.insert("description".into(), description.trim().into());
    let tags: Vec<serde_yaml::Value> = tags.iter().map(|t| t.trim().into()).collect();
    front.insert("tags".into(), serde_yaml::Value::Sequence(tags));
    front.insert(
        "updated".into(),
        app.now.format("%Y-%m-%dT%H:%M:%S").to_string().into(),
    );
    let front = serde_yaml::to_string(&front)?;
    let body = match text.starts_with("# ") {
        true => body,
        false => format!("# {}\n\n{body}", name.trim()),
    };
    Ok(format!("---\n{front}---\n{body}"))
}

enum How {
    New,
    Append,
    Replace,
}

fn write(file: &Path, content: &str, how: How) -> Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    match how {
        How::New => {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(file)
                .map_err(|e| match e.kind() {
                    std::io::ErrorKind::AlreadyExists => IterError::CommandFailed(format!(
                        "{} exists: --append or --replace",
                        file.display()
                    )),
                    _ => e.into(),
                })?;
            f.write_all(content.as_bytes())?;
        }
        How::Replace => iter_core::files::write_atomic(file, content)?,
        How::Append => {
            let old = std::fs::read_to_string(file).unwrap_or_default();
            let joined = match old.trim_end().is_empty() {
                true => content.to_string(),
                false => format!("{}\n\n{}\n", old.trim_end(), content.trim_end()),
            };
            iter_core::files::write_atomic(file, &joined)?;
        }
    }
    Ok(())
}

/// One line about a document: its front matter's `description`, else its first heading.
fn summary(text: &str) -> String {
    let split = split_front(text);
    let described = split
        .front
        .filter(|_| opens_front(text))
        .and_then(|front| serde_yaml::from_str::<serde_yaml::Mapping>(front).ok())
        .and_then(|m| m.get("description")?.as_str().map(str::to_string));
    described
        .or_else(|| {
            split
                .body
                .lines()
                .find_map(|l| l.strip_prefix("# ").map(|t| t.trim().to_string()))
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::over(
            iter_core::fixtures::db(),
            iter_core::fixtures::dt("2026-11-18 10:00"),
        )
    }

    #[test]
    fn a_page_gets_the_front_matter_the_wiki_checks() {
        let page = Doc::Page("concept".into());
        let text = compose(
            &page,
            "Backoff",
            "Wait longer each time.",
            Some("Retry waits grow"),
            &["net".into(), "retry".into()],
            &app(),
        )
        .unwrap();
        assert_eq!(
            text,
            "---\ndescription: Retry waits grow\ntags:\n- net\n- retry\nupdated: 2026-11-18T10:00:00\n---\n# Backoff\n\nWait longer each time.\n"
        );
        // its own front matter is kept as written
        let own = "---\ndescription: x\ntags: [a]\n---\n# X\n";
        assert_eq!(compose(&page, "x", own, None, &[], &app()).unwrap(), own);
        // without either, refused
        assert!(compose(&page, "x", "body", Some("d"), &[], &app()).is_err());
        // a sketch is the text, or a title
        assert_eq!(
            compose(&Doc::Sketch, "Cause", "", None, &[], &app()).unwrap(),
            "# Cause\n"
        );
        assert_eq!(
            compose(&Doc::Sketch, "Cause", "It loops.", None, &[], &app()).unwrap(),
            "# Cause\n\nIt loops.\n"
        );
        assert_eq!(
            compose(&Doc::Sketch, "Cause", "# Why\nIt loops.", None, &[], &app()).unwrap(),
            "# Why\nIt loops.\n"
        );
    }

    #[test]
    fn new_refuses_an_existing_file_and_append_adds_to_it() {
        let dir = iter_core::fixtures::scratch("note-write");
        let file = dir.join("sketches").join("a.md");
        write(&file, "one\n", How::New).unwrap();
        assert!(write(&file, "two\n", How::New).is_err());
        write(&file, "two", How::Append).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "one\n\ntwo\n");
        write(&file, "three\n", How::Replace).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "three\n");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_summary_is_the_description_else_the_title() {
        assert_eq!(summary("---\ndescription: Says x\n---\n# X\n"), "Says x");
        assert_eq!(summary("# Root cause\n\ntext"), "Root cause");
        assert_eq!(summary("plain"), "");
    }

    /// R1 of task 303: `iter note` opens the editor only for a person at a terminal: standard
    /// input and output both terminals, and no `ITER_TASK` (an agent's session, or a run, sets
    /// it). Anything else (a pipe, an agent) gets the path printed, as before.
    #[test]
    fn the_editor_opens_only_at_a_terminal_outside_a_task() {
        let cases = [
            // (stdin, stdout, ITER_TASK) -> opens
            ((true, true, None), true),
            ((true, true, Some("7")), false),
            ((false, true, None), false),
            ((true, false, None), false),
            ((false, false, None), false),
            ((false, false, Some("7")), false),
            ((false, true, Some("app/Fix it")), false),
            ((true, false, Some("app/Fix it")), false),
        ];
        for ((stdin, stdout, task), want) in cases {
            assert_eq!(
                opens_editor(stdin, stdout, task),
                want,
                "stdin tty {stdin}, stdout tty {stdout}, ITER_TASK {task:?}"
            );
        }
    }

    /// R1 of task 303: opening today's note makes it first when it is missing, then runs the
    /// configured editor (`editor:` in iter.yaml, here a script that writes down its arguments)
    /// on its path. Runs in a scratch config folder: the user's config and database are not read.
    #[test]
    fn opening_today_runs_the_editor_on_the_note_it_made() {
        let name = "opening_today_runs_the_editor_on_the_note_it_made";
        // the editor: a script that writes its arguments to `<script>.args`
        let bin = iter_core::fixtures::scratch("note-editor-script");
        std::fs::create_dir_all(&bin).expect("a folder for the script");
        let script = bin.join("editor.sh");
        std::fs::write(&script, "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args\"\n")
            .expect("the script");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                .expect("executable");
        }
        let yaml = format!("editor: {}\n", script.display());
        if iter_core::fixtures::in_scratch_config(module_path!(), name, Some(&yaml)) {
            let _ = std::fs::remove_dir_all(&bin);
            return;
        }
        // in the child: the script named by the config is the parent's
        let app = App::open().expect("the scratch app opens");
        let editor = iter_core::config::config().editor.clone();
        let home = PathBuf::from(std::env::var_os("HOME").expect("a scratch home"));
        let notes = home.join("notes");
        app.db
            .insert(&iter_core::models::Project {
                notes_path: notes.display().to_string(),
                ..iter_core::fixtures::project("app")
            })
            .expect("project inserts");
        let note = notes.join(format!("{}.md", app.now.date().format("%Y-%m-%d")));
        assert!(!note.exists(), "no note yet");
        let scope = NotesScope {
            project: Some("app".into()),
            org: None,
        };
        let _ = open_today(&app, &scope).expect("today's note opens");
        assert!(note.is_file(), "today's note is made: {}", note.display());
        let args = std::fs::read_to_string(format!("{editor}.args"))
            .expect("the editor ran and wrote down its arguments");
        assert_eq!(
            args.lines().collect::<Vec<_>>(),
            [note.display().to_string()],
            "the editor gets the note's path, and only that"
        );
        // a note that is there is opened as it is, not made again
        std::fs::write(&note, "kept\n").expect("written");
        let _ = open_today(&app, &scope).expect("opens again");
        assert_eq!(std::fs::read_to_string(&note).unwrap(), "kept\n");
    }
}
