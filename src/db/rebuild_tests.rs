//! The migration that lets two tasks of a project share a title (task 298, R1). Every test starts
//! from a database file in the shape of version 6, the last before it: `tasks` still has
//! `UNIQUE (project_id, name)`, and tags, session configs and sessions hang off it. The rebuild
//! drops and recreates `tasks`; with foreign keys on, that `DROP TABLE` would cascade-delete
//! everything that points at it, so each test counts what is left.
//!
//! The old shape is written out here, not made by `Db::open`, which builds the new one.

use super::*;
use crate::fixtures;
use std::path::PathBuf;

/// The tables of version 6, with every column a later step added.
fn schema(branch_prefix: bool) -> String {
    let prefix = if branch_prefix {
        ", branch_prefix TEXT NOT NULL DEFAULT ''"
    } else {
        ""
    };
    format!(
        "CREATE TABLE boards (
            id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL DEFAULT '');
         CREATE TABLE tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE,
            color TEXT NOT NULL, description TEXT NOT NULL DEFAULT '');
         CREATE TABLE organizations (
            id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL DEFAULT '', github INTEGER NOT NULL DEFAULT 0,
            tmux INTEGER NOT NULL DEFAULT 1, auto_branch INTEGER NOT NULL DEFAULT 1,
            branch_template TEXT NOT NULL DEFAULT 'feat/{{task}}',
            github_project TEXT NOT NULL DEFAULT '', default_branch TEXT NOT NULL DEFAULT 'main',
            notes_path TEXT NOT NULL DEFAULT '', platform_url TEXT NOT NULL DEFAULT '',
            platform_project TEXT NOT NULL DEFAULT '', platform_activity TEXT NOT NULL DEFAULT '');
         CREATE TABLE projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL DEFAULT '', base_path TEXT NOT NULL,
            github INTEGER NOT NULL DEFAULT 0, tmux INTEGER NOT NULL DEFAULT 1,
            auto_branch INTEGER NOT NULL DEFAULT 1,
            branch_template TEXT NOT NULL DEFAULT 'feat/{{task}}',
            organization_id INTEGER REFERENCES organizations(id) ON DELETE SET NULL,
            github_project TEXT NOT NULL DEFAULT '', default_branch TEXT NOT NULL DEFAULT 'main',
            board_id INTEGER REFERENCES boards(id) ON DELETE SET NULL,
            notes_path TEXT NOT NULL DEFAULT '');
         CREATE TABLE tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            name TEXT NOT NULL, description TEXT NOT NULL DEFAULT '', github_issue INTEGER,
            status TEXT NOT NULL DEFAULT 'queue' CHECK (status IN ('queue', 'wip', 'done')),
            matrix_placed INTEGER NOT NULL DEFAULT 0, start_time TEXT, duration INTEGER
            {prefix}, UNIQUE (project_id, name));
         CREATE TABLE task_tags (
            task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY (task_id, tag_id));
         CREATE TABLE session_configs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id INTEGER NOT NULL UNIQUE REFERENCES tasks(id) ON DELETE CASCADE,
            tmux_session_name TEXT, github_branch TEXT, worktree_path TEXT);
         CREATE TABLE sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            start TEXT NOT NULL, end TEXT, message TEXT);
         CREATE TABLE pushes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            organization_id INTEGER NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
            from_day TEXT NOT NULL, to_day TEXT NOT NULL, hash TEXT NOT NULL, at TEXT NOT NULL,
            results TEXT NOT NULL DEFAULT '');"
    )
}

/// What the old database holds: projects `web` (1) and `api` (2); tasks 1 and 2 of `web` and 5 of
/// `api`, where 3, 4 and 6 once lived (the sequence stands at 6); three tags, two of them on
/// task 1; session configs for tasks 1 and 2; three sessions; and an orphan `task_tags` row
/// whose task is gone, which the step must not choke on.
const DATA: &str = "INSERT INTO projects (id, name, base_path) VALUES (1, 'web', '/tmp/web'), (2, 'api', '/tmp/api');
     INSERT INTO tasks (id, project_id, name, status) VALUES
        (1, 1, 'login', 'wip'), (2, 1, 'logout', 'queue'), (3, 1, 'gone', 'queue'),
        (4, 1, 'gone too', 'queue'), (5, 2, 'login', 'done'), (6, 2, 'last', 'queue');
     DELETE FROM tasks WHERE id IN (3, 4, 6);
     INSERT INTO tags (id, name, color) VALUES (1, 'Urgent', '#f00'), (2, 'Important', '#00f'), (3, 'misc', '#0f0');
     INSERT INTO task_tags (task_id, tag_id) VALUES (1, 1), (1, 3), (2, 2);
     INSERT INTO session_configs (task_id, tmux_session_name, github_branch, worktree_path)
        VALUES (1, 'web/login', 'feat/login', '/tmp/web/.iter-worktrees/login'), (2, 'web/logout', NULL, NULL);
     INSERT INTO sessions (task_id, start, end, message) VALUES
        (1, '2026-01-01 09:00:00', '2026-01-01 10:00:00', 'a'),
        (1, '2026-01-02 09:00:00', '2026-01-02 10:00:00', NULL),
        (2, '2026-01-03 09:00:00', NULL, NULL);";

/// An old database file `<dir>/iter.db` (`dir` is made); `version` is its `user_version`.
fn old_database(dir: &Path, version: u32, branch_prefix: bool) -> PathBuf {
    std::fs::create_dir_all(dir).expect("scratch folder");
    let path = dir.join("iter.db");
    let conn = Connection::open(&path).expect("old database");
    // the bundled SQLite defaults foreign keys on; the orphan below needs them off
    conn.execute_batch("PRAGMA foreign_keys = OFF")
        .expect("pragma");
    conn.execute_batch(&schema(branch_prefix)).expect("schema");
    conn.execute_batch(DATA).expect("rows");
    // an orphan, written with foreign keys off as in a database an old build wrote
    conn.execute_batch("INSERT INTO task_tags (task_id, tag_id) VALUES (99, 3);")
        .expect("orphan");
    conn.execute_batch(&format!("PRAGMA user_version = {version}"))
        .expect("version");
    path
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .expect("count")
}

fn ids(conn: &Connection, table: &str) -> Vec<i64> {
    let mut stmt = conn
        .prepare(&format!("SELECT id FROM {table} ORDER BY id"))
        .expect("select");
    stmt.query_map([], |r| r.get(0))
        .expect("rows")
        .collect::<rusqlite::Result<_>>()
        .expect("ids")
}

fn violations(conn: &Connection) -> usize {
    conn.prepare("PRAGMA foreign_key_check")
        .expect("check")
        .query_map([], |_| Ok(()))
        .expect("rows")
        .count()
}

fn backup_of(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.before-v7", path.display()))
}

#[test]
fn the_rebuild_keeps_every_row_and_every_dependent_row() {
    let dir = fixtures::scratch("rebuild-keeps");
    let path = old_database(&dir, 6, true);
    let db = Db::open(&path).expect("migrates");
    // the old orphan is still the only violation: nothing was lost, nothing was made
    assert_eq!(violations(&db.conn), 1, "only the orphan that was there");
    db.conn
        .execute("DELETE FROM task_tags WHERE task_id = 99", [])
        .expect("orphan removed");
    assert_eq!(violations(&db.conn), 0);

    assert_eq!(ids(&db.conn, "tasks"), [1, 2, 5]);
    assert_eq!(count(&db.conn, "task_tags"), 3, "tags on tasks 1 and 2");
    assert_eq!(count(&db.conn, "session_configs"), 2);
    assert_eq!(count(&db.conn, "sessions"), 3);
    assert_eq!(count(&db.conn, "tags"), 3);
    let kept: (String, String) = db
        .conn
        .query_row(
            "SELECT t.name, c.tmux_session_name FROM tasks t JOIN session_configs c ON c.task_id = t.id WHERE t.id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("a task and its session config still meet");
    assert_eq!(kept, ("login".to_string(), "web/login".to_string()));
    assert_eq!(db.user_version().unwrap(), 7);
    let on: i64 = db
        .conn
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert_eq!(on, 1, "foreign keys are back on after the rebuild");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn two_tasks_of_a_project_can_share_a_title_and_the_ids_go_on() {
    let dir = fixtures::scratch("rebuild-dup");
    let db = Db::open(old_database(&dir, 6, true)).expect("migrates");
    let first = fixtures::add_task(&db, 1, "login");
    let second = fixtures::add_task(&db, 1, "login");
    // 6 was the highest id ever handed out: ids of deleted tasks are not handed out again
    assert_eq!((first, second), (7, 8));
    let titled = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE project_id = 1 AND name = 'login'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(titled, 3, "the old `login` and the two new ones");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deleting_a_task_or_a_project_still_cascades_after_the_rebuild() {
    let dir = fixtures::scratch("rebuild-cascade");
    let db = Db::open(old_database(&dir, 6, true)).expect("migrates");
    db.delete::<Task>(1).expect("task deleted");
    assert_eq!(
        count(&db.conn, "task_tags"),
        2,
        "its two links went, task 2's stayed (+ the orphan)"
    );
    assert_eq!(count(&db.conn, "session_configs"), 1);
    assert_eq!(count(&db.conn, "sessions"), 1);
    db.delete::<Project>(2).expect("project deleted");
    assert_eq!(ids(&db.conn, "tasks"), [2], "task 5 went with its project");
    // a bad status is still refused
    assert!(
        db.conn
            .execute(
                "INSERT INTO tasks (project_id, name, status) VALUES (1, 'x', 'bogus')",
                []
            )
            .is_err()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_database_missing_a_late_column_still_migrates() {
    let dir = fixtures::scratch("rebuild-column");
    let db = Db::open(old_database(&dir, 6, false)).expect("migrates");
    assert!(db.column_exists("tasks", "branch_prefix").unwrap());
    assert_eq!(ids(&db.conn, "tasks"), [1, 2, 5]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_copy_of_the_old_database_is_left_beside_it() {
    let dir = fixtures::scratch("rebuild-copy");
    let path = old_database(&dir, 6, true);
    let _db = Db::open(&path).expect("migrates");
    assert!(backup_of(&path).exists(), "a copy was made");
    let copy = Connection::open(backup_of(&path)).expect("the copy opens");
    assert_eq!(ids(&copy, "tasks"), [1, 2, 5]);
    assert_eq!(count(&copy, "sessions"), 3);
    let version: i64 = copy
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        version, 6,
        "the copy is the old schema, from before the rebuild"
    );
    let temporaries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("iter.db.before-v7.")
        })
        .collect();
    assert!(
        temporaries.is_empty(),
        "no half copy is left: {temporaries:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A version-0 file holding tasks is a legacy database with data: it is copied too.
#[test]
fn a_database_from_before_versioning_is_copied_as_well() {
    let dir = fixtures::scratch("rebuild-legacy");
    let path = old_database(&dir, 0, true);
    let _db = Db::open(&path).expect("migrates");
    assert!(backup_of(&path).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_database_leaves_no_copy() {
    let dir = fixtures::scratch("rebuild-new");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("iter.db");
    let db = Db::open(&path).expect("created");
    assert_eq!(db.user_version().unwrap(), 7);
    assert!(!backup_of(&path).exists(), "there was nothing to keep");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The copy is made with the path bound, not spliced into SQL.
#[test]
fn a_path_with_a_quote_in_it_migrates() {
    let dir = fixtures::scratch("rebuild-it's");
    let path = old_database(&dir, 6, true);
    let _db = Db::open(&path).expect("migrates");
    assert!(backup_of(&path).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn two_connections_migrating_one_file_leave_one_copy_and_version_7() {
    let dir = fixtures::scratch("rebuild-race");
    let path = old_database(&dir, 6, true);
    let opens: Vec<_> = (0..4)
        .map(|_| {
            let path = path.clone();
            std::thread::spawn(move || Db::open(&path).map(|db| db.user_version()))
        })
        .collect();
    for open in opens {
        assert_eq!(open.join().unwrap().unwrap().unwrap(), 7);
    }
    assert!(backup_of(&path).exists(), "a copy was made");
    let copy = Connection::open(backup_of(&path)).expect("one copy");
    assert_eq!(ids(&copy, "tasks"), [1, 2, 5], "it holds the old data");
    let _ = std::fs::remove_dir_all(&dir);
}

fn foreign_keys_on(db: &Db) -> bool {
    let on: i64 = db
        .conn
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    on == 1
}

fn noop_step(_: &Db) -> Result<()> {
    Ok(())
}

fn failing_step(db: &Db) -> Result<()> {
    // a half-done change, rolled back with its transaction
    db.conn.execute_batch("UPDATE tasks SET name = 'changed'")?;
    Err(crate::error::failed("boom"))
}

/// Round I2: the copy guards the rebuild (step 7). A later step on a database that already ran it
/// has nothing to guard, and must not leave a `.before-v7` file that says otherwise.
#[test]
fn a_step_after_the_rebuild_takes_no_copy() {
    let dir = fixtures::scratch("rebuild-later-step");
    let path = old_database(&dir, 6, true);
    let db = Db::open(&path).expect("migrates");
    std::fs::remove_file(backup_of(&path)).expect("the copy of step 7");
    let mut steps = Db::MIGRATIONS.to_vec();
    steps.push(noop_step);
    db.migrate_steps(&steps).expect("the extra step runs");
    assert_eq!(db.user_version().unwrap(), 8);
    assert!(
        !backup_of(&path).exists(),
        "step 7 was not pending: no copy"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A failing step is rolled back whole (name and version), the error reaches the caller, and
/// foreign keys are on again: the connection goes on being used by the process.
#[test]
fn a_failing_step_leaves_the_data_the_version_and_foreign_keys_alone() {
    let dir = fixtures::scratch("rebuild-failing");
    let path = old_database(&dir, 6, true);
    let db = Db::open(&path).expect("migrates");
    let mut steps = Db::MIGRATIONS.to_vec();
    steps.push(failing_step);
    let err = db.migrate_steps(&steps).expect_err("the step fails");
    assert!(err.to_string().contains("boom"), "{err}");
    assert_eq!(db.user_version().unwrap(), 7, "the version did not move");
    assert!(foreign_keys_on(&db), "foreign keys are on after a failure");
    let names: Vec<String> = db
        .conn
        .prepare("SELECT name FROM tasks ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(!names.contains(&"changed".to_string()), "rolled back");
    assert_eq!(count(&db.conn, "sessions"), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The rebuild itself failing (step 7 replaced by one that errors) loses nothing either: the
/// old database is still version 6 with every dependent row, and foreign keys are on.
#[test]
fn a_failing_rebuild_keeps_the_old_database_and_foreign_keys_on() {
    let dir = fixtures::scratch("rebuild-failing-v7");
    let path = old_database(&dir, 6, true);
    let (before, _) = Db::MIGRATIONS.split_at(6);
    let mut steps = before.to_vec();
    steps.push(failing_step);
    let conn = Connection::open(&path).expect("opens");
    conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
    let db = Db { conn };
    db.migrate_steps(&steps).expect_err("the rebuild fails");
    assert_eq!(db.user_version().unwrap(), 6);
    assert!(foreign_keys_on(&db));
    assert_eq!(ids(&db.conn, "tasks"), [1, 2, 5]);
    assert_eq!(count(&db.conn, "task_tags"), 4, "three and the orphan");
    assert_eq!(count(&db.conn, "session_configs"), 2);
    assert_eq!(count(&db.conn, "sessions"), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round I3: the copy is taken right before the rebuild step wherever it sits in the list (the
/// runner finds it by what it is, not by a hand-kept index). A step before it runs first, so the
/// copy holds the old tasks table at the version just before the rebuild.
#[test]
fn the_copy_is_taken_right_before_the_rebuild_wherever_it_sits() {
    let dir = fixtures::scratch("rebuild-found-by-identity");
    let path = old_database(&dir, 6, true);
    let (old, _) = Db::MIGRATIONS.split_at(6);
    let mut steps = old.to_vec();
    steps.push(noop_step);
    steps.push(Db::task_titles_repeat);
    let conn = Connection::open(&path).expect("opens");
    let db = Db { conn };
    db.migrate_steps(&steps).expect("migrates");
    assert_eq!(db.user_version().unwrap(), 8);
    let copy = Connection::open(backup_of(&path)).expect("a copy was made");
    let version: i64 = copy
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 7, "the noop step ran, the rebuild had not");
    assert_eq!(ids(&copy, "tasks"), [1, 2, 5]);
    let _ = std::fs::remove_dir_all(&dir);
}

fn step(run: fn(&Db) -> Result<()>, copy_first: bool) -> migrate::Step {
    migrate::Step { run, copy_first }
}

/// A version-6 file opened without migrating it, as `old_database` leaves it.
fn open_old(dir: &std::path::Path) -> (PathBuf, Db) {
    let path = old_database(dir, 6, true);
    let conn = Connection::open(&path).expect("opens");
    conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
    (path, Db { conn })
}

/// Round I4 (step 6): a step takes a copy because it says `copy_first`, not because it is the
/// rebuild. The rebuild without the flag takes none; a no-op step with it takes one named for the
/// version it brings, and the copy holds the version before.
#[test]
fn the_copy_is_taken_because_the_step_says_copy_first() {
    let dir = fixtures::scratch("rebuild-copy-first-flag");
    let (path, db) = open_old(&dir);
    let (old, _) = Db::MIGRATIONS.split_at(6);
    let mut steps: Vec<migrate::Step> = old.iter().map(|&run| step(run, false)).collect();
    steps.push(step(Db::task_titles_repeat, false));
    db.migrate_with(&steps).expect("migrates");
    assert_eq!(db.user_version().unwrap(), 7);
    assert!(!backup_of(&path).exists(), "the flag was off: no copy");
    let _ = std::fs::remove_dir_all(&dir);

    let dir = fixtures::scratch("rebuild-copy-first-noop");
    let (path, db) = open_old(&dir);
    let mut steps: Vec<migrate::Step> = old.iter().map(|&run| step(run, false)).collect();
    steps.push(step(noop_step, true));
    db.migrate_with(&steps).expect("migrates");
    let copy = Connection::open(backup_of(&path)).expect("a copy was made before step 7");
    let version: i64 = copy
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 6, "taken before the step ran");
    assert_eq!(ids(&copy, "tasks"), [1, 2, 5]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// With the flag the data survives the rebuild exactly as before (the runner still switches
/// foreign keys off around it and back on), and a failing flagged step leaves them on.
#[test]
fn a_flagged_rebuild_keeps_every_row_and_a_failing_flagged_step_keeps_foreign_keys_on() {
    let dir = fixtures::scratch("rebuild-copy-first-rows");
    let (path, db) = open_old(&dir);
    let (old, _) = Db::MIGRATIONS.split_at(6);
    let mut steps: Vec<migrate::Step> = old.iter().map(|&run| step(run, false)).collect();
    steps.push(step(Db::task_titles_repeat, true));
    db.migrate_with(&steps).expect("migrates");
    assert_eq!(ids(&db.conn, "tasks"), [1, 2, 5]);
    assert_eq!(count(&db.conn, "sessions"), 3);
    assert!(foreign_keys_on(&db));
    assert!(backup_of(&path).exists());
    steps.push(step(failing_step, true));
    db.migrate_with(&steps).expect_err("the step fails");
    assert!(foreign_keys_on(&db));
    assert_eq!(db.user_version().unwrap(), 7);
    let _ = std::fs::remove_dir_all(&dir);
}
