//! What the tests start from: an empty database, and the rows most of them
//! need, with the defaults a test does not care about.

use std::path::PathBuf;

use chrono::{NaiveDate, NaiveDateTime};

use crate::db::Db;
use crate::models::{Board, Project, Session, Settings, Task, TaskStatus};

pub fn db() -> Db {
    Db::open(":memory:").expect("in-memory database opens")
}

/// A project with default settings in `/tmp/{name}`; not inserted.
pub fn project(name: &str) -> Project {
    let mut p = Project::template(&Settings::default());
    p.name = name.into();
    p.base_path = format!("/tmp/{name}");
    p
}

/// [`project`] inserted into `db`: its id. (Only the web's tests use it.)
#[cfg_attr(not(feature = "web"), allow(dead_code))]
pub fn add_project(db: &Db, name: &str) -> i64 {
    db.insert(&project(name)).expect("project inserts")
}

pub fn board(name: &str) -> Board {
    Board {
        name: name.into(),
        ..Board::template()
    }
}

/// A queued task with nothing else set; not inserted. Override with struct update.
pub fn task(project_id: i64, name: &str) -> Task {
    Task {
        name: name.into(),
        ..Task::template(project_id, String::new(), TaskStatus::Queue)
    }
}

pub fn add_task(db: &Db, project_id: i64, name: &str) -> i64 {
    db.insert(&task(project_id, name)).expect("task inserts")
}

pub fn session(task_id: i64, start: NaiveDateTime, end: Option<NaiveDateTime>) -> Session {
    Session {
        id: None,
        task_id,
        start,
        end,
        message: None,
    }
}

pub fn add_session(db: &Db, task_id: i64, start: NaiveDateTime, end: Option<NaiveDateTime>) -> i64 {
    db.insert(&session(task_id, start, end))
        .expect("session inserts")
}

pub fn dt(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M")
        .unwrap_or_else(|e| panic!("bad fixture datetime {s:?}: {e}"))
}

pub fn day(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .unwrap_or_else(|e| panic!("bad fixture date {s:?}: {e}"))
}

/// A fresh scratch directory path, unique to this process and to `tag`
/// (keep each test's tag distinct so parallel tests never share one):
/// removed if it was there, not created.
pub fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("iter-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// What the agent tests start from: the project `app` keeping its notes in a
/// scratch folder, with one queued task `Fix it` whose description is
/// "Retries hammer the api.".
#[cfg_attr(not(feature = "web"), allow(dead_code))]
pub struct World {
    pub db: Db,
    pub project_id: i64,
    pub task_id: i64,
    pub notes: PathBuf,
}

pub fn world(tag: &str) -> World {
    let notes = scratch(tag);
    let db = db();
    let project_id = db
        .insert(&Project {
            notes_path: notes.display().to_string(),
            ..project("app")
        })
        .expect("project inserts");
    let task = Task {
        description: "Retries hammer the api.".into(),
        ..task(project_id, "Fix it")
    };
    let task_id = db.insert(&task).expect("task inserts");
    World {
        db,
        project_id,
        task_id,
        notes,
    }
}

/// The repo's `instructions/` folder: the instructions iter ships, read at run time (never
/// compiled in), as a config folder holds them once installed.
pub fn shipped_instructions() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/instructions"))
}

/// The text of the shipped instruction `name`.
pub fn shipped(name: &str) -> String {
    let path = shipped_instructions().join(format!("{name}.md"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The names of the shipped instructions.
pub const SHIPPED: [&str; 7] = [
    "plan", "develop", "test", "check", "fix", "complete", "ingest",
];

/// Copies the shipped instructions into `<dir>/instructions/`, as if `dir` were a layer that
/// holds them (the project's notes, say).
pub fn install_instructions(dir: &std::path::Path) {
    let to = dir.join("instructions");
    std::fs::create_dir_all(&to).expect("instructions folder");
    for name in SHIPPED {
        std::fs::write(to.join(format!("{name}.md")), shipped(name)).expect("instruction copied");
    }
}

/// Set in the child of [`in_scratch_config`].
const CHILD: &str = "ITER_SCRATCH_CONFIG_CHILD";

/// For a test that reads or opens the process-wide config. In the parent: runs test `name` of
/// the module `module` (`module_path!()`) again in a child of the test binary whose
/// `$XDG_CONFIG_HOME` and `$HOME` are a scratch folder holding `yaml` as `iter.yaml` (none if
/// `None`), asserts it passed and returns `true`: the caller returns. In the child: `false`, the
/// caller runs its body. The user's own config and database are never read.
pub fn in_scratch_config(module: &str, name: &str, yaml: Option<&str>) -> bool {
    if std::env::var_os(CHILD).is_some() {
        return false;
    }
    let home = scratch(&format!("cfg-{}", name.replace("::", "-")));
    let config_home = home.join("config");
    let dir = config_home.join("iter");
    std::fs::create_dir_all(&dir).expect("scratch config folder");
    if let Some(yaml) = yaml {
        std::fs::write(dir.join("iter.yaml"), yaml).expect("scratch config");
    }
    let module = module.split_once("::").map_or("", |(_, m)| m);
    let test = format!("{module}::{name}");
    let out = std::process::Command::new(std::env::current_exe().expect("the test binary"))
        .args([test.as_str(), "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .env("XDG_CONFIG_HOME", &config_home)
        .env("HOME", &home)
        .output()
        .expect("the test binary runs");
    let _ = std::fs::remove_dir_all(&home);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success() && stdout.contains("1 passed"),
        "{test} in a scratch config folder failed:\n--- stdout\n{stdout}\n--- stderr\n{stderr}"
    );
    true
}

/// A scratch git repository on `main` with one empty commit, or `None` where `git` is missing.
pub fn git_repo(tag: &str) -> Option<PathBuf> {
    let dir = scratch(tag);
    std::fs::create_dir_all(&dir).ok()?;
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(&dir)
            .output()
            .is_ok_and(|o| o.status.success())
    };
    (git(&["init", "-q", "-b", "main"]) && git(&["commit", "-q", "--allow-empty", "-m", "a"]))
        .then_some(dir)
}
