//! The live version (`crate::live`, R2a of task 302): one number that changes when iter.db gets a
//! commit from another connection, or when a `.md`/`.yaml` file under an organization's or a
//! project's notes folder is made, changed or removed, and stays the same otherwise.
//!
//! The API these tests assume: `live::Version::open(&App) -> Result<Version>` and
//! `Version::current() -> u64`. Each test runs in a scratch config folder
//! ([`in_scratch_config`]) so the database is a scratch file a second connection can open.

use crate::app::App;
use crate::config::config;
use crate::db::Db;
use crate::fixtures::{self, in_scratch_config as isolated};
use crate::live::Version;
use crate::models::{Organization, Project};
use std::path::{Path, PathBuf};

/// The folders a world's version watches: an organization with notes (and its project `app`
/// keeping its notes inside them) and a project `solo` with a folder of its own.
struct World {
    org: PathBuf,
    app: PathBuf,
    solo: PathBuf,
}

/// The scratch world: the rows in the scratch database, the folders made, task 1's folder
/// (`app/tasks/1/{notes,sketches,runs}` and `plan.md`) filled.
fn world() -> World {
    let home = PathBuf::from(std::env::var_os("HOME").expect("a scratch home"));
    let (org, solo) = (home.join("org"), home.join("solo"));
    let app = org.join("app");
    let db = App::open().expect("opens").db;
    let org_id = db
        .insert(&Organization {
            name: "acme".into(),
            notes_path: org.display().to_string(),
            ..Organization::default()
        })
        .expect("org inserts");
    let app_id = db
        .insert(&Project {
            organization_id: Some(org_id),
            ..fixtures::project("app")
        })
        .expect("project inserts");
    db.insert(&Project {
        notes_path: solo.display().to_string(),
        ..fixtures::project("solo")
    })
    .expect("project inserts");
    assert_eq!(fixtures::add_task(&db, app_id, "Fix it"), 1);
    let task = app.join("tasks").join("1");
    for dir in ["notes", "sketches", "runs"] {
        std::fs::create_dir_all(task.join(dir)).expect("task folder");
    }
    std::fs::create_dir_all(&solo).expect("solo folder");
    write(&task.join("plan.md"), "# Plan\n");
    write(&task.join("notes").join("a.md"), "a");
    write(&org.join("2026-01-05.md"), "# Monday\n");
    World { org, app, solo }
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

fn version() -> Version {
    Version::open(&App::open().expect("opens")).expect("the version opens")
}

/// `change` moves the version, and once moved it holds still.
fn moves(version: &mut Version, what: &str, change: impl FnOnce()) {
    let before = version.current();
    change();
    let after = version.current();
    assert_ne!(before, after, "{what} changes the version");
    assert_eq!(after, version.current(), "{what}: steady once changed");
}

/// `change` leaves the version as it was.
fn holds(version: &mut Version, what: &str, change: impl FnOnce()) {
    let before = version.current();
    change();
    assert_eq!(before, version.current(), "{what} leaves the version");
}

/// With nothing written, asking again (and again, a moment later) gives the same number.
#[test]
fn the_version_holds_without_writes() {
    if isolated(module_path!(), "the_version_holds_without_writes", None) {
        return;
    }
    world();
    #[allow(unused_mut)]
    let mut version = version();
    let first = version.current();
    assert_eq!(first, version.current());
    std::thread::sleep(std::time::Duration::from_millis(1100));
    assert_eq!(first, version.current(), "time alone changes nothing");
    assert_eq!(
        first,
        self::version().current(),
        "a second version over the same data agrees"
    );
}

/// A commit through another connection (the CLI, while the server runs) moves the version;
/// a read does not.
#[test]
fn a_commit_from_another_connection_changes_it() {
    if isolated(
        module_path!(),
        "a_commit_from_another_connection_changes_it",
        None,
    ) {
        return;
    }
    world();
    let mut version = version();
    let other = Db::open(config().db_path().expect("the scratch database")).expect("opens");
    holds(&mut version, "a read", || {
        let _: Option<Project> = other.get(1).expect("reads");
    });
    moves(&mut version, "an insert", || {
        fixtures::add_task(&other, 1, "Another");
    });
    moves(&mut version, "a second insert", || {
        fixtures::add_task(&other, 1, "And another");
    });
}

/// Making, changing and removing a `.md` or `.yaml` file anywhere the notes live moves the
/// version: the task's notes, sketches, runs and plan, the organization's daily notes and a
/// project's own folder.
#[test]
fn a_notes_file_made_changed_or_removed_changes_it() {
    if isolated(
        module_path!(),
        "a_notes_file_made_changed_or_removed_changes_it",
        None,
    ) {
        return;
    }
    let World { org, app, solo } = world();
    let task = app.join("tasks").join("1");
    let mut version = version();
    let note = task.join("notes").join("b.md");
    moves(&mut version, "a new task note", || write(&note, "b"));
    moves(&mut version, "a changed task note", || {
        write(&note, "b, longer")
    });
    moves(&mut version, "a removed task note", || {
        std::fs::remove_file(&note).expect("removed")
    });
    moves(&mut version, "a new sketch", || {
        write(&task.join("sketches").join("plan.md"), "# Sketch\n")
    });
    moves(&mut version, "a run's yaml", || {
        write(&task.join("runs").join("1.yaml"), "state: working\n")
    });
    moves(&mut version, "a run's log", || {
        write(&task.join("runs").join("1.md"), "started\n")
    });
    moves(&mut version, "the plan", || {
        write(&task.join("plan.md"), "# Plan\n- [ ] {s1} a step\n")
    });
    moves(&mut version, "an organization's daily note", || {
        write(&org.join("2026-01-05.md"), "# Monday\nmore\n")
    });
    moves(&mut version, "a project's own folder", || {
        write(&solo.join("2026-01-06.md"), "# Tuesday\n")
    });
    moves(&mut version, "a project page", || {
        write(&app.join("page.md"), "x")
    });
}

/// R9 of task 303 (#244): the page of a running agent follows its log, so a run's log
/// (`runs/<n>.log`) that is made or grows moves the version, and a log nobody writes to leaves it.
/// Fails today: only `.md` and `.yaml` files are watched.
#[test]
fn a_runs_log_made_or_grown_changes_it() {
    if isolated(module_path!(), "a_runs_log_made_or_grown_changes_it", None) {
        return;
    }
    let World { app, .. } = world();
    let runs = app.join("tasks").join("1").join("runs");
    let log = runs.join("1.log");
    let mut version = version();
    moves(&mut version, "a new run log", || write(&log, "started\n"));
    moves(&mut version, "a run log that grows", || {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&log)
            .expect("the log opens");
        file.write_all(b"a line the agent printed\n")
            .expect("appended");
    });
    holds(&mut version, "nothing written to the log", || {});
}

/// Files that are neither `.md` nor `.yaml` (an editor's swap file, a lock) leave it alone, and
/// so does a file outside every notes folder.
#[test]
fn other_files_leave_it() {
    if isolated(module_path!(), "other_files_leave_it", None) {
        return;
    }
    let World { app, .. } = world();
    let task = app.join("tasks").join("1");
    let mut version = version();
    holds(&mut version, "a .txt file", || {
        write(&task.join("notes").join("scratch.txt"), "x")
    });
    holds(&mut version, "a swap file", || {
        write(&task.join("notes").join(".a.md.swp"), "x")
    });
    let home = PathBuf::from(std::env::var_os("HOME").expect("a scratch home"));
    let elsewhere = home.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("a folder");
    holds(&mut version, "a note outside the notes folders", || {
        write(&elsewhere.join("x.md"), "x")
    });
}
