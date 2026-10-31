use super::*;
use crate::fixtures::{self, board, day, db, dt};
use crate::models::Organization;
use crate::models::Settings;

/// Runs every migration again, as on a database from before versioning
/// -- what the tests that hand-build an old schema need.
fn remigrate(db: &Db) -> Result<()> {
    db.conn.execute_batch("PRAGMA user_version = 0")?;
    db.migrate()
}

/// Several processes opening one fresh file at once -- a tmux hook and
/// a web request, say -- all get a migrated database: the step
/// transactions queue on the write lock rather than failing, and each
/// re-reads the version under it.
#[test]
fn concurrent_opens_migrate_once_without_failing() {
    let path = std::env::temp_dir().join(format!(
        "iter-migrate-race-{}-{:?}.db",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_file(&path);
    let opens: Vec<_> = (0..4)
        .map(|_| {
            let path = path.clone();
            std::thread::spawn(move || Db::open(&path).map(|db| db.user_version()))
        })
        .collect();
    for open in opens {
        let version = open
            .join()
            .expect("no panic")
            .expect("opens")
            .expect("reads");
        assert_eq!(version, Db::MIGRATIONS.len());
    }
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

fn project(name: &str) -> Project {
    Project {
        description: "notes".to_string(),
        github: true,
        tmux: false,
        branch_template: "fix/{task}".to_string(),
        github_project: "Roadmap".to_string(),
        notes_path: "/tmp/notes".to_string(),
        ..fixtures::project(name)
    }
}

fn insert_project(db: &Db, name: &str) -> i64 {
    db.insert(&project(name)).expect("project inserts")
}

fn insert_task(db: &Db, project_id: i64, name: &str) -> i64 {
    let task = Task {
        description: "task notes".to_string(),
        github_issue: Some(7),
        status: TaskStatus::Wip,
        branch_prefix: "fix/".to_string(),
        ..fixtures::task(project_id, name)
    };
    db.insert(&task).expect("task inserts")
}

/// A database from before urgent/important were tags carries them as
/// two columns; migrating turns each set flag into the tag and drops
/// the columns, leaving tasks with neither flag untagged.
#[test]
fn migrating_moves_the_flag_columns_onto_the_builtin_tags() {
    let db = db();
    let project = insert_project(&db, "p");
    let (both, urgent, neither) = (
        insert_task(&db, project, "both"),
        insert_task(&db, project, "urgent"),
        insert_task(&db, project, "neither"),
    );
    db.conn
        .execute_batch(&format!(
            "ALTER TABLE tasks ADD COLUMN urgency INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE tasks ADD COLUMN importance INTEGER NOT NULL DEFAULT 0;
             UPDATE tasks SET urgency = 1 WHERE id IN ({both}, {urgent});
             UPDATE tasks SET importance = 1 WHERE id = {both};"
        ))
        .expect("legacy columns");

    remigrate(&db).expect("migrates");

    assert!(!db.column_exists("tasks", "urgency").expect("check"));
    assert!(!db.column_exists("tasks", "importance").expect("check"));
    let of = |id| {
        let tags = db.tags_for_task(id).expect("tags");
        let has = |name| tags.iter().any(|t| t.name == name);
        Priority {
            urgent: has(URGENT_TAG),
            important: has(IMPORTANT_TAG),
        }
    };
    assert_eq!(
        of(both),
        Priority {
            urgent: true,
            important: true
        }
    );
    assert_eq!(
        of(urgent),
        Priority {
            urgent: true,
            important: false
        }
    );
    assert_eq!(of(neither), Priority::default());
}

/// Setting a priority adds and removes only the two built-in tags; a
/// task's other tags are left alone, and they show up on its cards.
#[test]
fn set_priority_toggles_only_the_builtin_tags() {
    let db = db();
    let work = db.insert(&board("work")).expect("board");
    let project = insert_project(&db, "p");
    db.set_projects::<Board>(work, &[project]).expect("roster");
    let task = insert_task(&db, project, "t");
    let other = db
        .insert(&Tag {
            name: "home".to_string(),
            ..Tag::template()
        })
        .expect("tag");
    db.set_task_tags(task, &[other]).expect("tags");

    let flags = Priority {
        urgent: true,
        important: false,
    };
    db.set_priority(task, flags).expect("set");
    let cards = db.cards(work).expect("cards");
    assert_eq!((cards[0].label.as_str(), cards[0].priority), ("p/t", flags));

    db.set_priority(task, Priority::default()).expect("clear");
    let names: Vec<String> = db
        .tags_for_task(task)
        .expect("tags")
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert_eq!(names, ["home"]);
}

/// A drop stores the task and its new flags together; with no flags
/// (an unplace) only the task.
#[test]
fn a_placement_stores_the_task_and_its_flags() {
    let db = db();
    let work = db.insert(&board("work")).expect("board");
    let project = insert_project(&db, "p");
    db.set_projects::<Board>(work, &[project]).expect("roster");
    let id = insert_task(&db, project, "t");
    let mut task = db.get::<Task>(id).expect("get").expect("task");
    task.matrix_placed = true;
    let flags = Priority {
        urgent: true,
        important: true,
    };
    db.update_placed(id, &task, Some(flags)).expect("place");
    let card = db.cards(work).expect("cards").remove(0);
    assert_eq!(card.priority, flags);
    assert!(card.task.matrix_placed);
    task.matrix_placed = false;
    db.update_placed(id, &task, None).expect("unplace");
    let card = db.cards(work).expect("cards").remove(0);
    assert_eq!(card.priority, flags, "flags untouched");
    assert!(!card.task.matrix_placed);
}

/// The unfinished cards leave finished tasks (and their flags) out in the
/// SQL, and the colors of both priority tags come from one query.
#[cfg(feature = "web")]
#[test]
fn unfinished_cards_leave_out_done_tasks() {
    let db = db();
    let work = db.insert(&board("work")).expect("board");
    let project = insert_project(&db, "p");
    db.set_projects::<Board>(work, &[project]).expect("roster");
    let open = insert_task(&db, project, "open");
    let done = insert_task(&db, project, "done");
    let mut finished = db.get::<Task>(done).expect("get").expect("exists");
    finished.status = TaskStatus::Done;
    db.update(done, &finished).expect("update");
    let urgent = Priority {
        urgent: true,
        important: false,
    };
    db.set_priority(open, urgent).expect("flag");
    db.set_priority(done, urgent).expect("flag");

    let cards = db.cards_unfinished(work).expect("cards");
    assert_eq!(
        cards.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(),
        ["p/open"]
    );
    assert_eq!(cards[0].priority, urgent);
    // the CLI's list still has both, in project then task order
    let all = db.cards(work).expect("cards");
    assert_eq!(all.len(), 2);
    assert!(
        db.priorities_of::<Board>(work, false)
            .expect("map")
            .contains_key(&done)
    );

    assert_eq!(
        db.priority_colors().expect("seeded"),
        ("#facc15".to_string(), "#3b82f6".to_string())
    );
}

/// A migrated database records its version, and opening it again runs
/// nothing: a step that would fail if re-run (here, a table the baseline
/// would recreate is gone) shows it was skipped.
#[test]
fn an_up_to_date_database_skips_every_migration() {
    let db = db();
    let version: i64 = db
        .conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("version");
    assert_eq!(version, Db::MIGRATIONS.len() as i64);

    db.conn.execute_batch("DROP TABLE boards").expect("drop");
    db.migrate().expect("nothing to do");
    assert!(!db.table_exists("boards").expect("check"));
    remigrate(&db).expect("from scratch");
    assert!(db.table_exists("boards").expect("check"));
}

/// The date window is half-open on the upper end, so the boundaries
/// are worth pinning: a session starting at 00:00:00 on `from` is in,
/// one at 23:59:59 on `to` is in, and the first instant of the day
/// after `to` is out. This is the filter every report applies, and it
/// is now SQL's job rather than a Rust-side pass.
#[test]
fn a_range_query_includes_both_boundary_days_whole() {
    let db = db();
    let project = insert_project(&db, "proj");
    let task = insert_task(&db, project, "t");
    for start in [
        "2025-03-02 23:59:59", // day before `from` -- out
        "2025-03-03 00:00:00", // first instant of `from` -- in
        "2025-03-04 12:00:00", // inside -- in
        "2025-03-05 23:59:59", // last instant of `to` -- in
        "2025-03-06 00:00:00", // first instant after `to` -- out
    ] {
        let session = fixtures::session(task, dt(&start[..16]), None);
        db.insert(&session).expect("session inserts");
    }

    let starts: Vec<String> = db
        .sessions_for_task_in_range(task, Some(day("2025-03-03")), day("2025-03-05"))
        .expect("range query")
        .iter()
        .map(|s| s.start.to_string())
        .collect();
    assert_eq!(
        starts,
        vec![
            "2025-03-03 00:00:00".to_string(),
            "2025-03-04 12:00:00".to_string(),
            "2025-03-05 23:59:00".to_string(),
        ]
    );
}

/// A stored timestamp that won't parse fails the read with an error
/// naming the value and column, instead of panicking. Reads that don't
/// touch the bad row still work.
#[test]
fn a_malformed_stored_timestamp_is_an_error_not_a_panic() {
    let db = db();
    let project = insert_project(&db, "proj");
    let task = insert_task(&db, project, "t");
    let session = fixtures::session(task, dt("2025-03-04 12:00"), None);
    db.insert(&session).expect("session inserts");
    db.conn
        .execute(
            "UPDATE sessions SET start = 'not a date' WHERE task_id = ?1",
            params![task],
        )
        .expect("corrupts the stored start");

    let error = db
        .sessions_for_task(task)
        .expect_err("a malformed start is an error");
    let message = error.to_string();
    assert!(
        message.contains("bad datetime 'not a date' in column 'start'"),
        "{message}"
    );

    let projects = db.list::<Project>().expect("other reads still work");
    assert_eq!(projects.len(), 1);
}

/// An absent `from` reaches back to the beginning of time rather than
/// to some arbitrary epoch.
#[test]
fn an_open_start_reaches_back_past_any_stored_session() {
    let db = db();
    let project = insert_project(&db, "proj");
    let task = insert_task(&db, project, "t");
    let session = fixtures::session(task, dt("1970-01-02 03:04"), None);
    db.insert(&session).expect("session inserts");

    assert_eq!(
        db.sessions_for_task_in_range(task, None, day("2025-03-05"))
            .expect("range query")
            .len(),
        1
    );
}

/// The project-wide query is the same window, across every task, and
/// it is what a project or organization report is built from.
#[test]
fn a_project_range_query_spans_its_tasks_in_time_order() {
    let db = db();
    let project = insert_project(&db, "proj");
    let other = insert_project(&db, "other");
    let a = insert_task(&db, project, "a");
    let b = insert_task(&db, project, "b");
    let elsewhere = insert_task(&db, other, "c");
    for (task, start) in [
        (b, "2025-03-04 09:00"),
        (a, "2025-03-03 09:00"),
        (elsewhere, "2025-03-03 10:00"), // another project -- excluded
        (a, "2025-03-09 09:00"),         // outside the range -- excluded
    ] {
        let session = fixtures::session(task, dt(start), None);
        db.insert(&session).expect("session inserts");
    }

    let found = db
        .sessions_for_project_in_range(project, Some(day("2025-03-03")), day("2025-03-05"))
        .expect("range query");
    assert_eq!(
        found
            .iter()
            .map(|s| (s.task_id, s.start.to_string()))
            .collect::<Vec<_>>(),
        vec![
            (a, "2025-03-03 09:00:00".to_string()),
            (b, "2025-03-04 09:00:00".to_string()),
        ]
    );
}

/// `task list` narrows by project and by a set of statuses at once;
/// both filters reach SQL, and an empty set means "every status".
#[test]
fn task_refs_narrows_by_project_and_status_set() {
    let db = db();
    let alpha = insert_project(&db, "alpha");
    let beta = insert_project(&db, "beta");
    insert_task(&db, alpha, "wip one"); // inserted as `wip`
    insert_task(&db, beta, "wip two");
    for (project, name, status) in [
        (alpha, "queued", TaskStatus::Queue),
        (alpha, "finished", TaskStatus::Done),
    ] {
        let task = Task { status, ..fixtures::task(project, name) };
        db.insert(&task).expect("task inserts");
    }

    assert_eq!(
        db.task_refs(Some("alpha"), &[]).expect("refs"),
        vec!["alpha/finished", "alpha/queued", "alpha/wip one",]
    );
    assert_eq!(
        db.task_refs(None, &[TaskStatus::Queue, TaskStatus::Wip])
            .expect("refs"),
        vec!["alpha/queued", "alpha/wip one", "beta/wip two",]
    );
    assert_eq!(
        db.task_refs(Some("beta"), &[TaskStatus::Queue])
            .expect("refs"),
        Vec::<String>::new()
    );
}

/// The completion lookups are one join rather than a query per
/// project, so what they return is worth pinning: every task, paired
/// with its own project, ordered the way completion offers it.
#[test]
fn task_refs_pairs_every_task_with_its_project() {
    let db = db();
    let beta = insert_project(&db, "beta");
    let alpha = insert_project(&db, "alpha");
    insert_task(&db, beta, "ship it");
    insert_task(&db, alpha, "write docs");
    insert_task(&db, alpha, "another");

    assert_eq!(
        db.task_refs(None, &[]).expect("task refs"),
        vec!["alpha/another", "alpha/write docs", "beta/ship it",]
    );
}

/// `session new` only completes tasks that haven't been started, so the
/// status filter has to reach SQL rather than being applied after every
/// task in the database was loaded.
#[test]
fn task_refs_can_narrow_to_one_status() {
    let db = db();
    let project = insert_project(&db, "proj");
    insert_task(&db, project, "in progress"); // inserted as `wip`
    let queued = fixtures::task(project, "queued");
    db.insert(&queued).expect("task inserts");

    assert_eq!(
        db.task_refs(None, &[TaskStatus::Queue]).expect("task refs"),
        vec!["proj/queued"]
    );
    assert_eq!(
        db.task_refs(None, &[TaskStatus::Done]).expect("task refs"),
        Vec::<String>::new()
    );
}

/// `--task` takes a bare name, so the same name under two projects is
/// one candidate, not two -- deduped and ordered by SQL.
#[test]
fn task_names_are_distinct_and_sorted() {
    let db = db();
    let one = insert_project(&db, "one");
    let two = insert_project(&db, "two");
    insert_task(&db, one, "shared");
    insert_task(&db, two, "shared");
    insert_task(&db, one, "alone");

    assert_eq!(
        db.task_names().expect("task names"),
        vec!["alone".to_string(), "shared".to_string()]
    );
}

#[test]
fn generated_statements_match_the_table_declaration() {
    assert_eq!(
        select_sql::<Session>("WHERE task_id = ?1"),
        "SELECT id, task_id, start, end, message FROM sessions WHERE task_id = ?1"
    );
    assert_eq!(
        insert_sql::<Session>(),
        "INSERT INTO sessions (task_id, start, end, message) VALUES (?1, ?2, ?3, ?4)"
    );
    assert_eq!(
        update_sql::<Session>(),
        "UPDATE sessions SET task_id = ?1, start = ?2, end = ?3, message = ?4 WHERE id = ?5"
    );
    assert_eq!(
        delete_sql::<Session>(),
        "DELETE FROM sessions WHERE id = ?1"
    );
}

#[test]
fn organization_statements_carry_the_downstream_defaults() {
    assert_eq!(
        insert_sql::<Organization>(),
        "INSERT INTO organizations (name, description, github, tmux, auto_branch, \
         branch_template, default_branch, github_project, notes_path, platform_url, \
         platform_project, platform_activity) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
    );
    assert_eq!(
        select_sql::<Organization>("WHERE name = ?1"),
        "SELECT id, name, description, github, tmux, auto_branch, branch_template, \
         default_branch, github_project, notes_path, platform_url, platform_project, \
         platform_activity FROM organizations WHERE name = ?1"
    );
}

/// Every column a struct declares must actually exist in the migrated
/// schema. This is the guard that `#[derive(Table)]` needs: because the
/// derive takes the columns straight off the struct's fields, adding a
/// field silently adds a column, and forgetting the matching change to
/// `migrate` would otherwise surface as a runtime "no such column".
#[test]
fn every_declared_column_exists_in_the_migrated_schema() {
    let db = db();

    fn columns_of(db: &Db, table: &str) -> Vec<String> {
        let mut stmt = db
            .conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("table_info prepares");
        let names = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("table_info runs")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("table_info rows read");
        assert!(!names.is_empty(), "table `{table}` does not exist");
        names
    }

    fn check<T: Table>(db: &Db) {
        let actual = columns_of(db, T::NAME);
        assert!(
            actual.iter().any(|c| c == "id"),
            "table `{}` has no `id` column, but every row is read as `id` first",
            T::NAME
        );
        for column in T::COLUMNS {
            assert!(
                actual.iter().any(|c| c == column),
                "`{}` declares column `{column}`, which the schema doesn't have (got {actual:?})",
                T::NAME
            );
        }
    }

    check::<Organization>(&db);
    check::<Project>(&db);
    check::<Task>(&db);
    check::<SessionConfig>(&db);
    check::<Session>(&db);
    check::<Push>(&db);
}

/// Every table binds exactly as many values as it declares columns.
/// `#[derive(Table)]` makes this true by construction; the test stays
/// as the guard for any `Table` impl written by hand, where `COLUMNS`
/// and `values` would again be two lists that could disagree.
#[test]
fn every_table_binds_one_value_per_column() {
    fn check<T: Table>(item: &T) {
        assert_eq!(
            item.values().len(),
            T::COLUMNS.len(),
            "{} binds a different number of values than it declares columns",
            T::NAME
        );
    }
    check(&project("alpha"));
    check(&Organization::template(&Settings::default()));
    check(&fixtures::task(1, "t"));
    check(&SessionConfig {
        id: None,
        task_id: 1,
        tmux_session_name: None,
        github_branch: None,
        worktree_path: None,
    });
    check(&fixtures::session(1, dt("2026-09-01 09:00"), None));
}

#[test]
fn project_round_trips_every_field() {
    let db = db();
    let id = insert_project(&db, "alpha");
    let loaded = db
        .get::<Project>(id)
        .expect("get succeeds")
        .expect("the row just inserted exists");

    assert_eq!(loaded.id, Some(id));
    assert_eq!(loaded.name, "alpha");
    assert_eq!(loaded.description, "notes");
    assert_eq!(loaded.base_path, "/tmp/alpha");
    // The bools matter most: they cross a bool -> INTEGER -> bool round
    // trip, and `tmux` is deliberately the non-default `false` here.
    assert!(loaded.github);
    assert!(!loaded.tmux);
    assert!(loaded.auto_branch);
    assert_eq!(loaded.branch_template, "fix/{task}");
    assert_eq!(loaded.github_project, "Roadmap");
}

/// Databases in the wild carry columns this build no longer knows
/// about -- `projects.auto_issue`, left behind by a removed feature,
/// is one. The generated statements name their columns explicitly, so
/// an extra column with a default is simply not written; this pins
/// that, since a `SELECT *`/positional binding would break on it.
#[test]
fn an_unknown_extra_column_does_not_break_writes() {
    let db = db();
    db.conn
        .execute_batch(
            "DROP TABLE projects;
             CREATE TABLE projects (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                organization_id INTEGER REFERENCES organizations(id) ON DELETE SET NULL,
                board_id        INTEGER REFERENCES boards(id) ON DELETE SET NULL,
                name            TEXT NOT NULL UNIQUE,
                description     TEXT NOT NULL DEFAULT '',
                base_path       TEXT NOT NULL,
                github          INTEGER NOT NULL DEFAULT 0,
                tmux            INTEGER NOT NULL DEFAULT 1,
                auto_branch     INTEGER NOT NULL DEFAULT 1,
                branch_template TEXT NOT NULL DEFAULT 'feat/{task}',
                default_branch  TEXT NOT NULL DEFAULT 'main',
                github_project  TEXT NOT NULL DEFAULT '',
                notes_path      TEXT NOT NULL DEFAULT '',
                auto_issue      INTEGER NOT NULL DEFAULT 0
             );",
        )
        .expect("drifted schema is created");

    let id = insert_project(&db, "alpha");
    db.update(id, &project("renamed")).expect("update succeeds");

    let loaded = db
        .get::<Project>(id)
        .expect("get succeeds")
        .expect("the row exists");
    assert_eq!(loaded.name, "renamed");
}

/// The mirror of `an_unknown_extra_column_does_not_break_writes`: a
/// database created before `branch_prefix` existed is *missing* a
/// column the struct declares, which no amount of explicit naming
/// survives -- `migrate` has to add it.
#[test]
fn a_pre_existing_tasks_table_gains_the_branch_prefix_column() {
    let db = db();
    db.conn
        .execute_batch(
            "DROP TABLE tasks;
             CREATE TABLE tasks (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id   INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                name         TEXT NOT NULL,
                description  TEXT NOT NULL DEFAULT '',
                github_issue INTEGER,
                status       TEXT NOT NULL DEFAULT 'queue',
                UNIQUE (project_id, name)
             );",
        )
        .expect("the old schema is created");
    remigrate(&db).expect("migrating adds the column");

    let project_id = insert_project(&db, "alpha");
    let id = insert_task(&db, project_id, "build it");
    let loaded = db
        .get::<Task>(id)
        .expect("get succeeds")
        .expect("the row just inserted exists");
    assert_eq!(loaded.branch_prefix, "fix/");
}

/// The same guarantee for the settings that land on two tables at once
/// -- `github_project` and `default_branch`. A database that predates
/// one has to gain the column on `projects` *and* on `organizations`,
/// or the setting a project inherits has nowhere to be read from.
#[test]
fn pre_existing_tables_gain_the_columns_added_since() {
    let db = db();
    db.conn
        .execute_batch(
            "DROP TABLE projects;
             DROP TABLE organizations;
             CREATE TABLE organizations (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                name            TEXT NOT NULL UNIQUE,
                description     TEXT NOT NULL DEFAULT '',
                github          INTEGER NOT NULL DEFAULT 0,
                tmux            INTEGER NOT NULL DEFAULT 1,
                auto_branch     INTEGER NOT NULL DEFAULT 1,
                branch_template TEXT NOT NULL DEFAULT 'feat/{task}'
             );
             CREATE TABLE projects (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                organization_id INTEGER REFERENCES organizations(id) ON DELETE SET NULL,
                board_id        INTEGER REFERENCES boards(id) ON DELETE SET NULL,
                name            TEXT NOT NULL UNIQUE,
                description     TEXT NOT NULL DEFAULT '',
                base_path       TEXT NOT NULL,
                github          INTEGER NOT NULL DEFAULT 0,
                tmux            INTEGER NOT NULL DEFAULT 1,
                auto_branch     INTEGER NOT NULL DEFAULT 1,
                branch_template TEXT NOT NULL DEFAULT 'feat/{task}'
             );",
        )
        .expect("the old schema is created");
    remigrate(&db).expect("migrating adds the columns");

    let id = insert_project(&db, "alpha");
    let loaded = db
        .get::<Project>(id)
        .expect("get succeeds")
        .expect("the row just inserted exists");
    assert_eq!(loaded.github_project, "Roadmap");
    assert_eq!(loaded.default_branch, "main");

    let organization_id = insert_organization(&db, "acme");
    let loaded = db
        .get::<Organization>(organization_id)
        .expect("get succeeds")
        .expect("the row just inserted exists");
    assert_eq!(loaded.github_project, "Acme Roadmap");
    assert_eq!(loaded.default_branch, "dev");
    assert_eq!(loaded.platform_url, "");
    assert_eq!(loaded.platform_project, "");
    assert_eq!(loaded.platform_activity, "");
}

#[test]
fn project_update_rewrites_every_column() {
    let db = db();
    let id = insert_project(&db, "alpha");

    let mut changed = project("renamed");
    changed.github = false;
    changed.tmux = true;
    changed.auto_branch = false;
    changed.branch_template = "chore/{task}".to_string();
    db.update(id, &changed).expect("update succeeds");

    let loaded = db
        .get::<Project>(id)
        .expect("get succeeds")
        .expect("the updated row exists");
    assert_eq!(loaded.name, "renamed");
    assert!(!loaded.github);
    assert!(loaded.tmux);
    assert!(!loaded.auto_branch);
    assert_eq!(loaded.branch_template, "chore/{task}");
}

#[test]
fn project_list_is_ordered_by_name() {
    let db = db();
    insert_project(&db, "charlie");
    insert_project(&db, "alpha");
    insert_project(&db, "bravo");

    let names: Vec<String> = db
        .list::<Project>()
        .expect("list succeeds")
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, ["alpha", "bravo", "charlie"]);
}

#[test]
fn find_project_by_name_matches_and_misses() {
    let db = db();
    insert_project(&db, "alpha");
    assert!(
        db.find_by_name::<Project>("alpha")
            .expect("lookup succeeds")
            .is_some()
    );
    assert!(
        db.find_by_name::<Project>("nope")
            .expect("lookup succeeds")
            .is_none()
    );
}

#[test]
fn task_round_trips_and_is_found_by_project_and_name() {
    let db = db();
    let project_id = insert_project(&db, "alpha");
    let task_id = insert_task(&db, project_id, "build it");

    let found = db
        .find_task(project_id, "build it")
        .expect("lookup succeeds")
        .expect("the task just inserted exists");
    assert_eq!(found.id, Some(task_id));
    assert_eq!(found.project_id, project_id);
    assert_eq!(found.description, "task notes");
    assert_eq!(found.github_issue, Some(7));
    assert_eq!(found.status, TaskStatus::Wip);
    assert_eq!(found.branch_prefix, "fix/");
}

#[test]
fn task_github_issue_round_trips_as_null_when_absent() {
    let db = db();
    let project_id = insert_project(&db, "alpha");
    let task = fixtures::task(project_id, "no issue");
    let id = db.insert(&task).expect("task inserts");
    let loaded = db
        .get::<Task>(id)
        .expect("get succeeds")
        .expect("the row just inserted exists");
    assert_eq!(loaded.github_issue, None);
    assert_eq!(loaded.status, TaskStatus::Queue);
}

#[test]
fn deleting_a_project_cascades_to_its_tasks() {
    let db = db();
    let project_id = insert_project(&db, "alpha");
    insert_task(&db, project_id, "build it");

    db.delete::<Project>(project_id).expect("delete succeeds");
    assert!(
        db.tasks_for_project(project_id)
            .expect("lookup succeeds")
            .is_empty()
    );
}

#[test]
fn session_config_round_trips_its_optional_columns() {
    let db = db();
    let project_id = insert_project(&db, "alpha");
    let task_id = insert_task(&db, project_id, "build it");

    let config = SessionConfig {
        id: None,
        task_id,
        tmux_session_name: Some("alpha/build-it".to_string()),
        github_branch: None,
        worktree_path: Some("/tmp/wt".to_string()),
    };
    db.insert(&config).expect("config inserts");

    let by_task = db
        .find_session_config_by_task(task_id)
        .expect("lookup succeeds")
        .expect("the config just inserted exists");
    assert_eq!(by_task.tmux_session_name.as_deref(), Some("alpha/build-it"));
    assert_eq!(by_task.github_branch, None);
    assert_eq!(by_task.worktree_path.as_deref(), Some("/tmp/wt"));

    let by_name = db
        .find_session_config_by_tmux_name("alpha/build-it")
        .expect("lookup succeeds")
        .expect("the config is findable by its tmux name");
    assert_eq!(by_name.id, by_task.id);
}

#[test]
fn open_session_is_the_one_without_an_end() {
    let db = db();
    let project_id = insert_project(&db, "alpha");
    let task_id = insert_task(&db, project_id, "build it");

    let closed = Session {
        message: Some("did a thing".to_string()),
        ..fixtures::session(
            task_id,
            dt("2026-09-01 09:00"),
            Some(dt("2026-09-01 10:00")),
        )
    };
    let open = fixtures::session(task_id, dt("2026-09-01 11:00"), None);
    db.insert(&closed).expect("closed session inserts");
    let open_id = db.insert(&open).expect("open session inserts");

    let found = db
        .open_session_for_task(task_id)
        .expect("lookup succeeds")
        .expect("there is an open session");
    assert_eq!(found.id, Some(open_id));
    assert!(found.is_ongoing());
    assert_eq!(found.start, dt("2026-09-01 11:00"));
}

/// The migration drops closed sessions under a minute -- 59 seconds
/// goes, exactly 60 stays -- and never an open one, however recent.
#[test]
fn short_closed_sessions_are_dropped_by_the_migration() {
    let db = db();
    let project_id = insert_project(&db, "alpha");
    let task_id = insert_task(&db, project_id, "build it");
    let at = |h, m, s| day("2026-09-01").and_hms_opt(h, m, s).expect("valid time");
    let session = |start, end| fixtures::session(task_id, start, end);
    db.insert(&session(at(9, 0, 0), Some(at(9, 0, 59))))
        .expect("59s");
    db.insert(&session(at(10, 0, 0), Some(at(10, 0, 0))))
        .expect("0s");
    let minute = db
        .insert(&session(at(11, 0, 0), Some(at(11, 1, 0))))
        .expect("60s");
    let open = db.insert(&session(at(12, 0, 0), None)).expect("open");

    db.drop_short_sessions().expect("migration runs");

    let left: Vec<_> = db
        .sessions_for_task(task_id)
        .expect("lookup")
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(left, [Some(minute), Some(open)]);
}

#[test]
fn closing_a_session_persists_its_end_and_message() {
    let db = db();
    let project_id = insert_project(&db, "alpha");
    let task_id = insert_task(&db, project_id, "build it");

    let session = fixtures::session(task_id, dt("2026-09-01 09:00"), None);
    let id = db.insert(&session).expect("session inserts");

    let mut closed = session;
    closed.end = Some(dt("2026-09-01 10:30"));
    closed.message = Some("wrapped up".to_string());
    db.update(id, &closed).expect("update succeeds");

    let loaded = db
        .get::<Session>(id)
        .expect("get succeeds")
        .expect("the updated row exists");
    assert_eq!(loaded.end, Some(dt("2026-09-01 10:30")));
    assert_eq!(loaded.message.as_deref(), Some("wrapped up"));
    assert!(
        db.open_session_for_task(task_id)
            .expect("lookup succeeds")
            .is_none()
    );
}

/// The union a project-level report sums is built by walking
/// `tasks_for_project` and asking `sessions_for_task` for each -- so
/// what has to hold is that neither leaks across a project boundary.
#[test]
fn a_projects_sessions_are_its_own_tasks_and_no_others() {
    let db = db();
    let project_id = insert_project(&db, "alpha");
    let one = insert_task(&db, project_id, "one");
    let two = insert_task(&db, project_id, "two");

    // A second project's session must not leak into the union.
    let other_project = insert_project(&db, "beta");
    let other_task = insert_task(&db, other_project, "elsewhere");

    for task_id in [one, two, other_task] {
        let (start, end) = (dt("2026-09-01 09:00"), dt("2026-09-01 10:00"));
        fixtures::add_session(&db, task_id, start, Some(end));
    }

    let mut task_ids: Vec<i64> = db
        .tasks_for_project(project_id)
        .expect("lookup succeeds")
        .into_iter()
        .flat_map(|t| {
            db.sessions_for_task(t.id.expect("an inserted task has an id"))
                .expect("lookup succeeds")
        })
        .map(|s| s.task_id)
        .collect();
    task_ids.sort_unstable();
    assert_eq!(task_ids, [one, two]);

    assert_eq!(db.sessions_for_task(one).expect("lookup succeeds").len(), 1);
}

fn insert_organization(db: &Db, name: &str) -> i64 {
    let mut organization = Organization::template(&Settings::default());
    organization.name = name.to_string();
    organization.github = true;
    organization.tmux = false;
    organization.branch_template = "chore/{task}".to_string();
    organization.github_project = "Acme Roadmap".to_string();
    organization.default_branch = "dev".to_string();
    db.insert(&organization).expect("organization inserts")
}

#[test]
fn organization_round_trips_its_downstream_defaults() {
    let db = db();
    let id = insert_organization(&db, "acme");
    let loaded = db
        .get::<Organization>(id)
        .expect("get succeeds")
        .expect("the row just inserted exists");
    assert_eq!(loaded.name, "acme");
    assert!(loaded.github);
    assert!(!loaded.tmux);
    assert!(loaded.auto_branch);
    assert_eq!(loaded.branch_template, "chore/{task}");
    assert_eq!(loaded.github_project, "Acme Roadmap");
}

/// Setting the roster moves listed projects in -- even out of another
/// organization -- and drops unlisted ones to no organization, without
/// touching anyone else's members.
#[test]
fn setting_an_organizations_projects_moves_them_in_and_out() {
    let db = db();
    let acme = insert_organization(&db, "acme");
    let other = insert_organization(&db, "other");
    let kept = insert_project(&db, "kept");
    let dropped = insert_project(&db, "dropped");
    let stolen = insert_project(&db, "stolen");
    let bystander = insert_project(&db, "bystander");
    db.set_projects::<Organization>(acme, &[kept, dropped])
        .unwrap();
    db.set_projects::<Organization>(other, &[stolen, bystander])
        .unwrap();

    db.set_projects::<Organization>(acme, &[kept, stolen])
        .unwrap();

    let names = |org| -> Vec<String> {
        db.projects_in::<Organization>(org)
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect()
    };
    assert_eq!(names(acme), ["kept", "stolen"]);
    assert_eq!(names(other), ["bystander"]);
    assert_eq!(
        db.get::<Project>(dropped).unwrap().unwrap().organization_id,
        None
    );
}

#[test]
fn a_project_may_belong_to_an_organization_or_to_none() {
    let db = db();
    let organization = insert_organization(&db, "acme");

    let mut member = project("member");
    member.organization_id = Some(organization);
    let member_id = db.insert(&member).expect("project inserts");
    let loner_id = insert_project(&db, "loner");

    assert_eq!(
        db.get::<Project>(member_id)
            .expect("get succeeds")
            .expect("row exists")
            .organization_id,
        Some(organization)
    );
    assert_eq!(
        db.get::<Project>(loner_id)
            .expect("get succeeds")
            .expect("row exists")
            .organization_id,
        None,
        "a project without an organization must round-trip as NULL"
    );

    let roster: Vec<String> = db
        .projects_in::<Organization>(organization)
        .expect("roster loads")
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(roster, ["member"], "only members are in the roster");
}

/// The guarantee that makes organizations safe to delete: the projects
/// (and so every task and session under them) survive, merely stopping
/// being members.
#[test]
fn deleting_an_organization_keeps_its_projects() {
    let db = db();
    let organization = insert_organization(&db, "acme");
    let mut member = project("member");
    member.organization_id = Some(organization);
    let project_id = db.insert(&member).expect("project inserts");
    let task_id = insert_task(&db, project_id, "build it");

    db.delete::<Organization>(organization)
        .expect("delete succeeds");

    let loaded = db
        .get::<Project>(project_id)
        .expect("get succeeds")
        .expect("the project outlives its organization");
    assert_eq!(loaded.organization_id, None);
    assert_eq!(
        db.tasks_for_project(project_id)
            .expect("tasks load")
            .first()
            .map(|t| t.id),
        Some(Some(task_id)),
        "the project's tasks must survive too"
    );
}

/// A database written before organizations existed has a `projects`
/// table with no `organization_id`, and `CREATE TABLE IF NOT EXISTS`
/// won't add it -- so `migrate` has to.
#[test]
fn an_older_database_gains_the_organization_column() {
    let db = db();
    db.conn
        .execute_batch(
            "DROP TABLE projects;
             CREATE TABLE projects (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                name            TEXT NOT NULL UNIQUE,
                description     TEXT NOT NULL DEFAULT '',
                base_path       TEXT NOT NULL,
                github          INTEGER NOT NULL DEFAULT 0,
                tmux            INTEGER NOT NULL DEFAULT 1,
                auto_branch     INTEGER NOT NULL DEFAULT 1,
                branch_template TEXT NOT NULL DEFAULT 'feat/{task}'
             );",
        )
        .expect("pre-organization schema is created");
    assert!(!db.column_exists("projects", "organization_id").unwrap());

    remigrate(&db).expect("migration succeeds");

    assert!(db.column_exists("projects", "organization_id").unwrap());
    // ...and the column is usable, defaulting to "no organization".
    let id = insert_project(&db, "legacy");
    assert_eq!(
        db.get::<Project>(id)
            .expect("get succeeds")
            .expect("row exists")
            .organization_id,
        None
    );
}

#[test]
fn a_fresh_database_migrates_without_legacy_tables() {
    let db = db();
    assert!(db.table_exists("projects").expect("check succeeds"));
    assert!(db.table_exists("tasks").expect("check succeeds"));
    assert!(db.table_exists("session_configs").expect("check succeeds"));
    assert!(db.table_exists("sessions").expect("check succeeds"));
    assert!(!db.table_exists("records").expect("check succeeds"));
}

#[test]
fn legacy_tables_are_renamed_in_the_right_order() {
    // The pre-rename schema: `sessions` held what is now
    // `session_configs`, and `records` held what are now `sessions`.
    let db = db();
    db.conn
        .execute_batch(
            "DROP TABLE sessions;
             DROP TABLE session_configs;
             CREATE TABLE sessions (id INTEGER PRIMARY KEY, task_id INTEGER);
             CREATE TABLE records (id INTEGER PRIMARY KEY, task_id INTEGER);",
        )
        .expect("legacy schema is created");

    remigrate(&db).expect("migration succeeds");

    assert!(db.table_exists("session_configs").expect("check succeeds"));
    assert!(db.table_exists("sessions").expect("check succeeds"));
    assert!(!db.table_exists("records").expect("check succeeds"));
}

#[test]
fn resolve_names_the_kind_it_could_not_find() {
    let db = db();
    insert_project(&db, "alpha");
    assert_eq!(db.resolve::<Project>("alpha").expect("found").name, "alpha");
    let missing = db.resolve::<Board>("nope").expect_err("no such board");
    assert_eq!(missing.to_string(), "no such board 'nope'");
}

/// One unknown name fails the whole lookup, so a roster is never
/// half-applied.
#[test]
fn ids_by_name_is_all_or_nothing() {
    let db = db();
    let a = insert_project(&db, "a");
    let b = insert_project(&db, "b");
    let names = |ns: &[&str]| ns.iter().map(|n| n.to_string()).collect::<Vec<_>>();
    assert_eq!(
        db.ids_by_name::<Project>(&names(&["b", "a"])).expect("ids"),
        [b, a]
    );
    assert!(db.ids_by_name::<Project>(&names(&["a", "typo"])).is_err());
    // Repeats collapse, and nothing is nothing.
    assert_eq!(
        db.ids_by_name::<Project>(&names(&["b", "a", "b"]))
            .expect("ids"),
        [b, a]
    );
    assert!(db.ids_by_name::<Project>(&[]).expect("none").is_empty());
    assert_eq!(
        db.ids_by_name::<Tag>(&names(&["Urgent"]))
            .expect("seeded")
            .len(),
        1
    );
}

/// Board and organization membership are independent columns driven by
/// the same generic code: moving a project on one leaves the other be.
#[test]
fn set_projects_touches_only_its_own_group_column() {
    let db = db();
    let work = db.insert(&board("work")).expect("insert board");
    let acme = insert_organization(&db, "acme");
    let (a, b) = (insert_project(&db, "a"), insert_project(&db, "b"));
    db.set_projects::<Organization>(acme, &[a, b])
        .expect("org roster");
    db.set_projects::<Board>(work, &[a]).expect("board roster");
    db.set_projects::<Board>(work, &[b]).expect("board roster");

    let names = |ps: Vec<Project>| ps.into_iter().map(|p| p.name).collect::<Vec<_>>();
    assert_eq!(names(db.projects_in::<Board>(work).expect("board")), ["b"]);
    assert_eq!(
        names(db.projects_in::<Organization>(acme).expect("org")),
        ["a", "b"]
    );
}

/// The group row and its projects are written together, for a board and
/// for an organization: a taken name leaves the roster as it was.
#[test]
fn save_group_writes_the_row_and_its_projects_together() {
    let db = db();
    let work = db.insert(&board("work")).expect("insert board");
    let home = db.insert(&board("home")).expect("insert board");
    let (a, b) = (insert_project(&db, "a"), insert_project(&db, "b"));
    db.set_projects::<Board>(work, &[a]).expect("roster");
    let mut renamed = db.get::<Board>(work).expect("get").expect("board");
    renamed.name = "job".into();
    db.save_group(work, &renamed, &[b]).expect("saved");
    let names = |ps: Vec<Project>| ps.into_iter().map(|p| p.name).collect::<Vec<_>>();
    assert_eq!(names(db.projects_in::<Board>(work).expect("work")), ["b"]);
    renamed.name = "home".into();
    assert!(matches!(
        db.save_group(work, &renamed, &[a]),
        Err(IterError::NameTaken { kind: "board" })
    ));
    assert_eq!(names(db.projects_in::<Board>(work).expect("work")), ["b"]);
    assert!(db.projects_in::<Board>(home).expect("home").is_empty());

    let acme = insert_organization(&db, "acme");
    let beta = insert_organization(&db, "beta");
    let mut org = db.get::<Organization>(acme).expect("get").expect("org");
    db.save_group(acme, &org, &[a, b]).expect("org saved");
    assert_eq!(
        names(db.projects_in::<Organization>(acme).expect("acme")),
        ["a", "b"]
    );
    org.name = "beta".into();
    assert!(db.save_group(acme, &org, &[a]).is_err());
    assert_eq!(
        names(db.projects_in::<Organization>(acme).expect("acme")),
        ["a", "b"]
    );
    assert!(
        db.projects_in::<Organization>(beta)
            .expect("beta")
            .is_empty()
    );
}

/// The roster write moves projects between boards, ignores unknown and
/// repeated ids, leaves the rest alone, and takes a list longer than one
/// statement could bind.
#[test]
fn assign_projects_moves_only_what_changes() {
    let db = db();
    let (work, home) = (
        db.insert(&board("work")).expect("board"),
        db.insert(&board("home")).expect("board"),
    );
    let (a, b, c) = (
        insert_project(&db, "a"),
        insert_project(&db, "b"),
        insert_project(&db, "c"),
    );
    db.set_projects::<Board>(work, &[a, b]).expect("roster");
    db.set_projects::<Board>(home, &[c]).expect("roster");
    db.set_projects::<Board>(work, &[b, c, c, 9999])
        .expect("moved");
    let names = |id| {
        db.projects_in::<Board>(id)
            .expect("in")
            .into_iter()
            .map(|p| p.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(names(work), ["b", "c"]);
    assert!(names(home).is_empty());
    db.set_projects::<Board>(work, &[]).expect("emptied");
    assert!(names(work).is_empty());

    let all: Vec<i64> = (0..1200)
        .map(|i| insert_project(&db, &format!("p{i}")))
        .collect();
    db.set_projects::<Board>(home, &all).expect("long roster");
    assert_eq!(names(home).len(), 1200);
}

/// The narrow project listing pairs each project with its board's id.
#[cfg(feature = "web")]
#[test]
fn project_owners_lists_id_name_and_group() {
    let db = db();
    let work = db.insert(&board("work")).expect("board");
    let (b, a) = (insert_project(&db, "b"), insert_project(&db, "a"));
    db.set_projects::<Board>(work, &[b]).expect("roster");
    assert_eq!(
        db.project_owners::<Board>().expect("owners"),
        [(a, "a".to_string(), None), (b, "b".to_string(), Some(work))]
    );
}

/// An error inside `atomically` rolls its writes back.
#[test]
fn atomically_rolls_back_on_error() {
    let db = db();
    let out: Result<()> = db.atomically(|| {
        db.insert(&board("gone"))?;
        Err(IterError::CommandFailed("stop".into()))
    });
    assert!(out.is_err());
    assert!(db.list::<Board>().expect("list").is_empty());
}

/// The tasks of a group come back with their project's name, ordered by
/// project then task, and only from that group's projects.
#[test]
fn tasks_in_a_group_pair_each_task_with_its_project() {
    let db = db();
    let work = db.insert(&board("work")).expect("insert board");
    let (b, a, off) = (
        insert_project(&db, "b"),
        insert_project(&db, "a"),
        insert_project(&db, "off"),
    );
    db.set_projects::<Board>(work, &[a, b]).expect("roster");
    for (project, name) in [(b, "y"), (a, "z"), (a, "x"), (off, "w")] {
        insert_task(&db, project, name);
    }
    let got: Vec<(String, String)> = db
        .tasks_in::<Board>(work)
        .expect("query")
        .into_iter()
        .map(|(project, task)| (project, task.name))
        .collect();
    let want =
        [("a", "x"), ("a", "z"), ("b", "y")].map(|(p, t)| (p.to_string(), t.to_string()));
    assert_eq!(got, want);
    assert!(db.tasks_in::<Organization>(1).expect("query").is_empty());
}

/// A long list of names stays under SQLite's limit on bound variables,
/// and an unknown name at the far end is still found out.
#[test]
fn ids_by_name_handles_more_names_than_one_query_binds() {
    let db = db();
    let all: Vec<String> = (0..1200).map(|i| format!("p{i}")).collect();
    let ids: Vec<i64> = all.iter().map(|n| insert_project(&db, n)).collect();
    assert_eq!(db.ids_by_name::<Project>(&all).expect("ids"), ids);
    let mut typo = all.clone();
    typo.push("nope".into());
    assert!(db.ids_by_name::<Project>(&typo).is_err());
}

/// One call creates or rewrites a task with its tags; a task without
/// tags gets none.
#[cfg(feature = "web")]
#[test]
fn save_task_writes_the_row_and_its_tags_together() {
    let db = db();
    let project = insert_project(&db, "a");
    let urgent = db.ids_by_name::<Tag>(&["Urgent".to_string()]).expect("tag");
    let mut task = Task::template(project, String::new(), TaskStatus::Queue);
    task.name = "x".into();
    let id = db.save_task(None, &task, &urgent).expect("create");
    assert_eq!(db.tags_for_task(id).expect("tags").len(), 1);
    task.name = "y".into();
    db.save_task(Some(id), &task, &[]).expect("edit");
    assert!(db.tags_for_task(id).expect("tags").is_empty());
    assert_eq!(db.get::<Task>(id).expect("get").expect("row").name, "y");
    task.name = "z".into();
    let bare = db.save_task(None, &task, &[]).expect("bare");
    assert!(db.tags_for_task(bare).expect("tags").is_empty());
    // A duplicate name is refused and leaves nothing behind.
    assert!(db.save_task(None, &task, &urgent).is_err());
}

/// The badge count matches the rows `tasks_for_project` reads.
#[cfg(feature = "web")]
#[test]
fn a_project_counts_its_tasks_without_reading_them() {
    let db = db();
    let (a, b) = (insert_project(&db, "a"), insert_project(&db, "b"));
    assert_eq!(db.count_tasks_for_project(a).expect("count"), 0);
    for (project, name) in [(a, "x"), (a, "y"), (b, "z")] {
        insert_task(&db, project, name);
    }
    assert_eq!(db.count_tasks_for_project(a).expect("count"), 2);
    assert_eq!(db.tasks_for_project(a).expect("rows").len(), 2);
}

/// A taken name is a typed error, from the constraint's result code:
/// on insert and on update, per entity; a task's name only clashes
/// within its project.
#[test]
fn a_taken_name_is_a_typed_error() {
    let db = db();
    let (a, b) = (insert_project(&db, "a"), insert_project(&db, "b"));
    assert!(matches!(
        db.insert(&project("a")),
        Err(IterError::NameTaken { kind: "project" })
    ));
    assert!(matches!(
        db.update(b, &project("a")),
        Err(IterError::NameTaken { kind: "project" })
    ));
    insert_task(&db, a, "t");
    insert_task(&db, b, "t");
    let mut same = Task::template(a, String::new(), TaskStatus::Queue);
    same.name = "t".into();
    assert!(matches!(
        db.insert(&same),
        Err(IterError::NameTaken { kind: "task" })
    ));
}

/// Binding is the whole point of a board: a project put on one shows
/// up in `projects_in::<Board>`, and a project on another board or none
/// does not.
#[test]
fn projects_in_a_board_are_only_the_projects_bound_to_it() {
    let db = db();
    let work = db.insert(&board("work")).expect("insert board");
    let home = db.insert(&board("home")).expect("insert board");
    let (a, b, c) = (
        insert_project(&db, "a"),
        insert_project(&db, "b"),
        insert_project(&db, "c"),
    );
    for (id, board_id) in [(a, Some(work)), (b, Some(home)), (c, None)] {
        let mut p = db.get::<Project>(id).expect("get").expect("exists");
        p.board_id = board_id;
        db.update(id, &p).expect("update");
    }

    let names: Vec<String> = db
        .projects_in::<Board>(work)
        .expect("query")
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, ["a"]);
}

/// Every database starts with the two tags the board views colour the
/// Eisenhower flags with, and re-running the migration neither
/// duplicates them nor undoes a recolouring.
#[test]
fn urgent_and_important_tags_are_seeded_once() {
    let db = db();
    let mut urgent = db
        .find_by_name::<Tag>("Urgent")
        .expect("find")
        .expect("seeded");
    let important = db
        .find_by_name::<Tag>("Important")
        .expect("find")
        .expect("seeded");
    assert_eq!(
        (urgent.color.as_str(), important.color.as_str()),
        ("#facc15", "#3b82f6")
    );

    urgent.color = "#ff0000".into();
    db.update(urgent.id(), &urgent).expect("recolor");
    db.seed_tags().expect("reseed");
    assert_eq!(db.list::<Tag>().expect("list").len(), 2);
    let urgent = db
        .find_by_name::<Tag>("Urgent")
        .expect("find")
        .expect("seeded");
    assert_eq!(urgent.color, "#ff0000");
}

/// Tags are shared: setting replaces, and deleting a task or a tag
/// drops only the link.
#[cfg(feature = "web")]
#[test]
fn task_tags_replace_and_cascade() {
    let db = db();
    let project = insert_project(&db, "a");
    let task = insert_task(&db, project, "t");
    let urgent = db
        .find_by_name::<Tag>("Urgent")
        .expect("find")
        .expect("seeded")
        .id();
    let important = db
        .find_by_name::<Tag>("Important")
        .expect("find")
        .expect("seeded")
        .id();

    db.set_task_tags(task, &[urgent, important]).expect("set");
    assert_eq!(db.tags_for_task(task).expect("tags").len(), 2);
    db.set_task_tags(task, &[important]).expect("replace");
    let tags = db.tags_for_task(task).expect("tags");
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].name, "Important");

    db.delete::<Tag>(important).expect("delete tag");
    assert!(db.tags_for_task(task).expect("tags").is_empty());
    assert!(db.find_by_name::<Tag>("Urgent").expect("find").is_some());
}

/// A board is a container, not an owner: deleting one must unbind its
/// projects rather than delete them.
#[test]
fn deleting_a_board_keeps_its_projects_and_unbinds_them() {
    let db = db();
    let board_id = db.insert(&board("work")).expect("insert board");
    let id = insert_project(&db, "a");
    let mut p = db.get::<Project>(id).expect("get").expect("exists");
    p.board_id = Some(board_id);
    db.update(id, &p).expect("update");

    db.delete::<crate::models::Board>(board_id).expect("delete");

    let p = db.get::<Project>(id).expect("get").expect("project kept");
    assert_eq!(p.board_id, None);
}

/// Databases created before boards existed have a `projects` table
/// without `board_id`; opening one must add it.
#[test]
fn migration_adds_board_id_to_a_projects_table_that_lacks_it() {
    let dir = fixtures::scratch("board-mig");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("iter.db");
    let _ = std::fs::remove_file(&path);
    {
        let conn = Connection::open(&path).expect("open");
        conn.execute_batch(
            "CREATE TABLE projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                base_path TEXT NOT NULL
             );
             INSERT INTO projects (name, base_path) VALUES ('old', '/tmp/old');",
        )
        .expect("legacy schema");
    }
    let db = Db::open(path.to_str().expect("utf8 path")).expect("migrates");
    assert!(db.column_exists("projects", "board_id").expect("check"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn task_schedule_round_trips_and_migrates_onto_an_old_tasks_table() {
    let db = db();
    let project_id = insert_project(&db, "alpha");
    let mut task = Task::template(project_id, String::new(), TaskStatus::Queue);
    task.name = "scheduled".to_string();
    task.start_time = Some(dt("2026-09-28 09:30"));
    task.duration = Some(Duration(90));
    db.insert(&task).expect("insert");
    let found = db
        .find_task(project_id, "scheduled")
        .expect("lookup")
        .expect("exists");
    assert_eq!(found.start_time, Some(dt("2026-09-28 09:30")));
    assert_eq!(found.duration, Some(Duration(90)));

    let dir = fixtures::scratch("task-mig");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("iter.db");
    let _ = std::fs::remove_file(&path);
    {
        let conn = Connection::open(&path).expect("open");
        conn.execute_batch(
            "CREATE TABLE tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL,
                name TEXT NOT NULL
             );",
        )
        .expect("legacy schema");
    }
    let db = Db::open(path.to_str().expect("utf8 path")).expect("migrates");
    for column in ["start_time", "duration"] {
        assert!(
            db.column_exists("tasks", column).expect("check"),
            "{column}"
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}
