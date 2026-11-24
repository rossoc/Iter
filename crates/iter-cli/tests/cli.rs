//! The `iter` binary run as a person or an agent runs it, for the commands of task 303: `iter
//! note` (R1), `iter task new --name` (R2), `iter task edit --tag/--untag` (R3) and `iter
//! instruction list/show` (R8).
//!
//! Each test makes a scratch home of its own: `$HOME` and `$XDG_CONFIG_HOME` point into it, so
//! the binary reads a scratch `iter.yaml` and database, never the user's. `ITER_TASK`,
//! `ITER_PROJECT` and `TMUX` are taken out of the binary's environment and every command names
//! its project or task, so nothing asks tmux which session it runs in. Standard input is empty
//! and output is a pipe: the binary never sees a terminal here.

use iter_core::db::Db;
use iter_core::fixtures;
use iter_core::models::{IMPORTANT_TAG, Project, Tag, Task, TaskStatus, URGENT_TAG};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// A scratch home: the config folder, the database in it, and a project `app` keeping its notes
/// in `<home>/notes`, with one queued task `Fix it` ("Retries hammer the api.").
struct Home {
    root: PathBuf,
    config: PathBuf,
    notes: PathBuf,
    db: PathBuf,
}

impl Home {
    fn new(tag: &str, yaml: Option<&str>) -> Home {
        let root = fixtures::scratch(&format!("cli303-{tag}"));
        let config = root.join("config").join("iter");
        std::fs::create_dir_all(&config).expect("a scratch config folder");
        if let Some(yaml) = yaml {
            std::fs::write(config.join("iter.yaml"), yaml).expect("a scratch iter.yaml");
        }
        let notes = root.join("notes");
        std::fs::create_dir_all(&notes).expect("a notes folder");
        let db_path = config.join("iter.db");
        iter_core::config::load_defaults();
        let db = Db::open(&db_path).expect("the scratch database opens");
        let project_id = db
            .insert(&Project {
                tmux: false,
                auto_branch: false,
                notes_path: notes.display().to_string(),
                base_path: root.join("code").display().to_string(),
                ..fixtures::project("app")
            })
            .expect("project inserts");
        db.insert(&Task {
            description: "Retries hammer the api.".into(),
            ..fixtures::task(project_id, "Fix it")
        })
        .expect("task inserts");
        Home {
            root,
            config,
            notes,
            db: db_path,
        }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_iter"));
        command
            .args(args)
            .env("HOME", &self.root)
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env_remove("ITER_TASK")
            .env_remove("ITER_PROJECT")
            .env_remove("TMUX")
            .env_remove("TMUX_PANE")
            .current_dir(&self.root)
            .stdin(Stdio::null());
        command
    }

    /// Runs `iter <args>`.
    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().expect("the iter binary runs")
    }

    /// Runs `iter <args>` with `input` on standard input.
    fn run_with_input(&self, args: &[&str], input: &str) -> Output {
        use std::io::Write;
        let mut child = self
            .command(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the iter binary runs");
        child
            .stdin
            .take()
            .expect("a pipe")
            .write_all(input.as_bytes())
            .expect("the input is written");
        child.wait_with_output().expect("the iter binary ends")
    }

    fn db(&self) -> Db {
        Db::open(&self.db).expect("the scratch database opens")
    }

    fn tasks_named(&self, name: &str) -> Vec<Task> {
        self.db()
            .list::<Task>()
            .expect("tasks")
            .into_iter()
            .filter(|t| t.name == name)
            .collect()
    }

    fn task(&self, name: &str) -> Task {
        let mut found = self.tasks_named(name);
        assert_eq!(found.len(), 1, "one task `{name}`");
        found.remove(0)
    }

    fn tags_of(&self, task: &Task) -> Vec<String> {
        let mut names: Vec<String> = self
            .db()
            .tags_for_task(task.id.expect("a stored task"))
            .expect("tags")
            .into_iter()
            .map(|t| t.name)
            .collect();
        names.sort();
        names
    }

    /// The tags there are, the two seeded ones (`Urgent`, `Important`) left out.
    fn tag_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .db()
            .list::<Tag>()
            .expect("tags")
            .into_iter()
            .map(|t| t.name)
            .filter(|n| n != URGENT_TAG && n != IMPORTANT_TAG)
            .collect();
        names.sort();
        names
    }

    fn today_note(&self) -> PathBuf {
        let today = chrono::Local::now().date_naive();
        self.notes.join(format!("{}.md", today.format("%Y-%m-%d")))
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Asserts `out` succeeded and returns its standard output.
fn ok(out: &Output, what: &str) -> String {
    assert!(
        out.status.success(),
        "{what} failed:\n--- stdout\n{}\n--- stderr\n{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    text(&out.stdout)
}

/// A home whose `editor:` is a script that writes down its arguments (`<home>/editor.ran`) and
/// changes nothing.
fn home_with_editor(tag: &str) -> Home {
    let home = Home::new(tag, None);
    let script = home.root.join("editor.sh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}'\n",
            home.root.join("editor.ran").display()
        ),
    )
    .expect("the editor script");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("executable");
    std::fs::write(
        home.config.join("iter.yaml"),
        format!("editor: {}\n", script.display()),
    )
    .expect("a scratch iter.yaml");
    home
}

// ---- R1: `iter note` ----

/// R1: `--print` always prints the path of today's note (made if missing) and never opens the
/// editor. Fails today: `--print` is not an argument of `iter note today`.
#[test]
fn note_today_print_prints_the_path_and_opens_no_editor() {
    let home = home_with_editor("note-print");
    let out = home.run(&["note", "today", "--print", "--project", "app"]);
    let stdout = ok(&out, "iter note today --print");
    assert_eq!(stdout.trim(), home.today_note().display().to_string());
    assert!(home.today_note().is_file(), "today's note is made");
    assert!(
        !home.root.join("editor.ran").exists(),
        "--print opened the editor"
    );
}

/// R1: with no terminal (a pipe, as an agent or a script runs it) `iter note today` prints the
/// path as before and opens no editor, so `Bash(iter note *)` stays safe for agents. Passes
/// today; it guards the change.
#[test]
fn note_today_through_a_pipe_prints_the_path() {
    let home = home_with_editor("note-pipe");
    let out = home.run(&["note", "today", "--project", "app"]);
    let stdout = ok(&out, "iter note today");
    assert_eq!(stdout.trim(), home.today_note().display().to_string());
    assert!(home.today_note().is_file());
    assert!(
        !home.root.join("editor.ran").exists(),
        "a pipe got the editor"
    );
}

/// R1: inside a task's session (`ITER_TASK` set, as for an agent) the path is printed too.
/// Passes today; it guards the change.
#[test]
fn note_today_inside_a_task_prints_the_path() {
    let home = home_with_editor("note-task");
    let out = home
        .command(&["note", "today", "--project", "app"])
        .env("ITER_TASK", "app/Fix it")
        .output()
        .expect("the iter binary runs");
    let stdout = ok(&out, "iter note today with ITER_TASK");
    assert_eq!(stdout.trim(), home.today_note().display().to_string());
    assert!(
        !home.root.join("editor.ran").exists(),
        "an agent got the editor"
    );
}

// ---- R2: `iter task new --name` ----

/// R2: `--name`, `--description` and `--tag` make the task with no editor, print its label
/// (`app/<name> #<id>`), and make the tags that do not exist. Fails today: the flags do not parse.
#[test]
fn task_new_with_a_name_makes_the_task_and_its_tags_without_an_editor() {
    let home = home_with_editor("task-new");
    home.db()
        .insert(&Tag {
            name: "bug".into(),
            ..Tag::template()
        })
        .expect("an existing tag");
    let out = home.run(&[
        "task",
        "new",
        "app",
        "--name",
        "Retry per route",
        "--description",
        "Backoff should be per route.",
        "--tag",
        "bug",
        "--tag",
        "network",
    ]);
    let stdout = ok(&out, "iter task new --name");
    let task = home.task("Retry per route");
    let id = task.id.expect("stored");
    assert!(
        stdout.contains(&format!("app/Retry per route #{id}")),
        "the label is printed: {stdout}"
    );
    assert_eq!(task.description, "Backoff should be per route.");
    assert_eq!(task.status, TaskStatus::Queue, "the configured default");
    assert_eq!(home.tags_of(&task), ["bug", "network"]);
    assert_eq!(
        home.tag_names(),
        ["bug", "network"],
        "the missing tag is made once, the existing one reused"
    );
    assert!(
        !home.root.join("editor.ran").exists(),
        "--name opened the editor"
    );
}

/// R2: with `--name` and no `--description`, a piped standard input is the description.
#[test]
fn task_new_reads_a_piped_description() {
    let home = home_with_editor("task-new-stdin");
    let out = home.run_with_input(
        &["task", "new", "app", "--name", "Cache the token"],
        "The token is fetched on every call.\n",
    );
    ok(&out, "iter task new --name with stdin");
    let task = home.task("Cache the token");
    assert_eq!(
        task.description.trim(),
        "The token is fetched on every call."
    );
    assert!(!home.root.join("editor.ran").exists());
}

/// R2: `--status wip` starts the task in progress.
#[test]
fn task_new_takes_a_status() {
    let home = home_with_editor("task-new-status");
    let out = home.run(&["task", "new", "app", "--name", "Now", "--status", "wip"]);
    ok(&out, "iter task new --status wip");
    assert_eq!(home.task("Now").status, TaskStatus::Wip);
}

/// R2: an empty or blank name is an error, and nothing is written: no task, no tag.
#[test]
fn task_new_with_an_empty_name_writes_nothing() {
    let home = home_with_editor("task-new-empty");
    let before = home.db().list::<Task>().expect("tasks").len();
    for name in ["", "   "] {
        let out = home.run(&["task", "new", "app", "--name", name, "--tag", "stray"]);
        assert!(
            !out.status.success(),
            "an empty name {name:?} was accepted: {}",
            text(&out.stdout)
        );
        let stderr = text(&out.stderr);
        // refused by iter (exit 1), not by the argument parser (exit 2, a usage line)
        assert_eq!(
            out.status.code(),
            Some(1),
            "refused as an empty name, not as an unknown flag: {stderr}"
        );
        assert!(
            !stderr.contains("unexpected argument") && !stderr.contains("Usage:"),
            "{stderr}"
        );
        assert!(
            stderr.contains("name"),
            "the error says the name is missing: {stderr}"
        );
    }
    assert_eq!(home.db().list::<Task>().expect("tasks").len(), before);
    assert!(home.tag_names().is_empty(), "a tag was made for nothing");
    assert!(!home.root.join("editor.ran").exists());
}

/// R2: without `--name`, `iter task new` opens the editor as before (here one that saves
/// nothing, so no task is made). Passes today; it guards the change.
#[test]
fn task_new_without_a_name_still_opens_the_editor() {
    let home = home_with_editor("task-new-editor");
    let before = home.db().list::<Task>().expect("tasks").len();
    let out = home.run(&["task", "new", "app"]);
    let stdout = ok(&out, "iter task new");
    assert!(stdout.contains("no changes"), "{stdout}");
    assert!(home.root.join("editor.ran").exists(), "the editor ran");
    assert_eq!(home.db().list::<Task>().expect("tasks").len(), before);
}

// ---- R3: `iter task edit --tag/--untag` ----

/// R3: `--tag` adds tags (making a missing one) and `--untag` takes one off, with no editor; the
/// rest of the task is left as it was. Fails today: the flags do not parse.
#[test]
fn task_edit_tags_and_untags_without_an_editor() {
    let home = home_with_editor("task-edit");
    let before = home.task("Fix it");
    let out = home.run(&[
        "task",
        "edit",
        "app/Fix it",
        "--tag",
        "bug",
        "--tag",
        "later",
    ]);
    ok(&out, "iter task edit --tag");
    assert_eq!(home.tags_of(&home.task("Fix it")), ["bug", "later"]);

    let out = home.run(&["task", "edit", "app/Fix it", "--untag", "later"]);
    ok(&out, "iter task edit --untag");
    let after = home.task("Fix it");
    assert_eq!(home.tags_of(&after), ["bug"]);
    assert_eq!(
        home.tag_names(),
        ["bug", "later"],
        "untagging leaves the tag itself"
    );
    assert_eq!(
        (
            &after.name,
            &after.description,
            after.status,
            after.project_id
        ),
        (
            &before.name,
            &before.description,
            before.status,
            before.project_id
        ),
        "only the tags changed"
    );
    assert!(
        !home.root.join("editor.ran").exists(),
        "--tag/--untag opened the editor"
    );
}

/// R3: both at once; a tag the task already has is not added twice.
#[test]
fn task_edit_tag_and_untag_together() {
    let home = home_with_editor("task-edit-both");
    ok(
        &home.run(&["task", "edit", "app/Fix it", "--tag", "idea"]),
        "iter task edit --tag idea",
    );
    let out = home.run(&[
        "task",
        "edit",
        "app/Fix it",
        "--tag",
        "bug",
        "--tag",
        "bug",
        "--untag",
        "idea",
    ]);
    ok(&out, "iter task edit --tag --untag");
    assert_eq!(home.tags_of(&home.task("Fix it")), ["bug"]);
}

// ---- R8: `iter instruction list/show` ----

fn instruction(dir: &Path, name: &str, text: &str) {
    let folder = dir.join("instructions");
    std::fs::create_dir_all(&folder).expect("an instructions folder");
    std::fs::write(folder.join(format!("{name}.md")), text).expect("an instruction");
}

/// R8 (#239): `iter instruction list` names each instruction with where it comes from and its
/// description; the nearest layer wins. Fails today: there is no `instruction` command.
#[test]
fn instruction_list_names_each_with_its_origin_and_description() {
    let home = Home::new("instr-list", None);
    instruction(
        &home.notes,
        "review",
        "---\ndescription: Reviews the diff\n---\nReview it carefully.\n",
    );
    instruction(
        &home.config,
        "review",
        "---\ndescription: The config's review\n---\nConfig review.\n",
    );
    instruction(
        &home.config,
        "plan",
        "---\ndescription: Plans the task\n---\nPlan it.\n",
    );
    let out = home
        .command(&["instruction", "list"])
        .env("ITER_TASK", "app/Fix it")
        .output()
        .expect("the iter binary runs");
    let stdout = ok(&out, "iter instruction list");
    let line = |name: &str| {
        stdout
            .lines()
            .find(|l| l.split_whitespace().next() == Some(name))
            .unwrap_or_else(|| panic!("no line for {name}: {stdout}"))
            .to_string()
    };
    let review = line("review");
    assert!(
        review.contains("project") && review.contains("Reviews the diff"),
        "{review}"
    );
    assert!(!stdout.contains("The config's review"), "{stdout}");
    let plan = line("plan");
    assert!(
        plan.contains("config") && plan.contains("Plans the task"),
        "{plan}"
    );
    assert_eq!(
        stdout.lines().filter(|l| l.contains("review")).count(),
        1,
        "one line per name: {stdout}"
    );
}

/// R8: `iter instruction show <name>` prints the nearest file's instruction; an unknown name is
/// an error.
#[test]
fn instruction_show_prints_the_nearest_one() {
    let home = Home::new("instr-show", None);
    instruction(
        &home.notes,
        "review",
        "---\ndescription: Reviews the diff\n---\nReview it carefully.\n",
    );
    instruction(&home.config, "review", "Config review.\n");
    let show = |name: &str| {
        home.command(&["instruction", "show", name])
            .env("ITER_TASK", "app/Fix it")
            .output()
            .expect("the iter binary runs")
    };
    let stdout = ok(&show("review"), "iter instruction show review");
    assert!(stdout.contains("Review it carefully."), "{stdout}");
    assert!(!stdout.contains("Config review."), "{stdout}");
    assert!(
        !show("nope").status.success(),
        "an unknown name is an error"
    );
}
