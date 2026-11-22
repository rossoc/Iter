//! A task's workspace: the git worktree and branch, the tmux session name, the clock that times
//! the work, and the teardown of all of them.
//!
//! Kill-last invariant: `kill-session` takes down every pane, including this process when run from
//! inside that session, so nothing after it is guaranteed to run. Hence (1) write the database
//! before tearing down, and (2) kill last: [`teardown`] returns the tmux session to kill instead of
//! killing it, so `project delete` can kill after its loop; [`teardown_and_kill`] is for callers
//! where nothing follows.

pub mod clock;

use crate::db::{Db, Table};
use crate::error::{IterError, Result};
use crate::git;
use crate::models::{Project, SessionConfig, Task, TaskStatus};
use crate::tmux::Multiplexer;
use std::path::Path;

/// The tmux session name of a new session, `<project>/<id> <title>`, each half flattened (tmux
/// reads `/`, `:`, `.` in a target). The id keeps it unique when titles repeat. A tmux identifier,
/// not a task reference; names already stored are read back from the database, never recomputed.
pub fn tmux_name(project: &Project, task: &Task) -> String {
    let part = |name: &str| name.replace('/', "-").replace([':', '.', '*', '?'], "_");
    format!("{}/{} {}", part(&project.name), task.id(), part(&task.name))
}

/// The worktree, branch and `session_configs` row of `task` (no tmux name yet), undone when the
/// row cannot be written. The slug carries the id, so equal titles never collide, and a worktree
/// folder that already exists (a stale one of this task) is an error naming it. `session new` and
/// `task solve` both start from here.
pub fn open(
    db: &Db,
    project: &Project,
    task: &Task,
    branch: Option<&str>,
) -> Result<SessionConfig> {
    // An empty prefix (blank, or a task from before the field) falls back to the project's template.
    let prefix = match task.branch_prefix.trim() {
        "" => git::branch_prefix(&project.branch_template),
        prefix => prefix.to_string(),
    };
    let slug = task.slug();
    let path = Path::new(&project.base_path)
        .join(".iter-worktrees")
        .join(&slug);
    if path.exists() {
        return Err(IterError::CommandFailed(format!(
            "the worktree folder '{slug}' already exists"
        )));
    }
    let path = path.to_string_lossy().to_string();
    let branch = branch.map_or_else(|| format!("{prefix}{slug}"), str::to_string);
    git::create_worktree(&project.base_path, &branch, &path)?;
    let mut config = SessionConfig {
        id: None,
        task_id: task.id(),
        tmux_session_name: None,
        github_branch: Some(branch),
        worktree_path: Some(path),
    };
    match db.insert(&config) {
        Ok(id) => {
            config.id = Some(id);
            Ok(config)
        }
        Err(error) => {
            remove_worktree_and_branch(
                project,
                config.worktree_path.as_deref(),
                config.github_branch.as_deref(),
            );
            Err(error)
        }
    }
}

/// Marks a queued task in progress when work starts on it; a done task stays done.
pub fn start_work(db: &Db, task: &mut Task) -> Result<()> {
    if task.status == TaskStatus::Queue {
        task.status = TaskStatus::Wip;
        db.update(task.id(), task)?;
    }
    Ok(())
}

/// Removes the worktree, branch and session-config row, for `task done`/`task delete`/`project
/// delete`. Does not kill the tmux session; it returns it (see the module doc).
#[must_use = "the tmux session still needs killing"]
pub fn teardown<'a>(
    db: &Db,
    project: &Project,
    session_config: &'a SessionConfig,
) -> Option<&'a str> {
    remove_worktree_and_branch(
        project,
        session_config.worktree_path.as_deref(),
        session_config.github_branch.as_deref(),
    );
    if let Some(id) = session_config.id
        && let Err(e) = db.delete::<SessionConfig>(id)
    {
        eprintln!("warning: failed to delete session-config row: {e}");
    }
    session_config.tmux_session_name.as_deref()
}

/// The on-disk half of teardown, shared with [`unwind`] and [`open`]'s undo (no row yet). Failures
/// warn and carry on. The branch goes after the worktree, as `git::delete_branch` requires, and is
/// kept (with a warning) while it has unmerged commits.
pub(crate) fn remove_worktree_and_branch(
    project: &Project,
    worktree: Option<&str>,
    branch: Option<&str>,
) {
    if let Some(worktree) = worktree
        && let Err(e) = git::remove_worktree(&project.base_path, worktree)
    {
        eprintln!("warning: {e}");
    }
    if let Some(branch) = branch {
        match git::delete_branch(&project.base_path, branch) {
            Ok(true) => {}
            Ok(false) => eprintln!(
                "warning: kept branch '{branch}': it has commits that are not merged \
                 (delete it with `git branch -D {branch}`)"
            ),
            Err(e) => eprintln!("warning: {e}"),
        }
    }
}

/// [`teardown`] for a lone session, killing at once; fine where nothing runs after.
pub fn teardown_and_kill(
    db: &Db,
    mux: &dyn Multiplexer,
    project: &Project,
    session_config: &SessionConfig,
) {
    if let Some(name) = teardown(db, project, session_config) {
        mux.kill_session(name);
    }
}

/// Undoes a session set up half way: kills `tmux_session` first (it is detached and unattached,
/// so this cannot take the running process with it), then removes the worktree, branch and row.
pub fn unwind(
    db: &Db,
    mux: &dyn Multiplexer,
    project: &Project,
    config: &SessionConfig,
    tmux_session: Option<&str>,
) {
    if let Some(name) = tmux_session {
        mux.kill_session(name);
    }
    let _ = teardown(db, project, config);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    fn task(id: i64, name: &str) -> Task {
        Task {
            id: Some(id),
            ..fixtures::task(1, name)
        }
    }

    /// R3: `<project>/<id> <title>`, each half flattened; equal titles give different names.
    #[test]
    fn a_new_session_is_named_with_the_task_id() {
        let web = fixtures::project("web");
        for (id, title, name) in [
            (42, "login", "web/42 login"),
            (7, "a: b.c/d", "web/7 a_ b_c-d"),
            (3, "what*?", "web/3 what__"),
        ] {
            assert_eq!(tmux_name(&web, &task(id, title)), name);
        }
        let odd = fixtures::project("my/app");
        assert_eq!(tmux_name(&odd, &task(5, "x")), "my-app/5 x");
        // every target separator of tmux is flattened in the project half too
        let target = fixtures::project("web: a.b/c");
        assert_eq!(tmux_name(&target, &task(1, "a*b?")), "web_ a_b-c/1 a_b_");
        assert_ne!(
            tmux_name(&web, &task(1, "login")),
            tmux_name(&web, &task(2, "login"))
        );
    }

    #[test]
    fn work_starting_marks_a_queued_task_in_progress_and_leaves_a_done_one() {
        for (before, after) in [
            (TaskStatus::Queue, TaskStatus::Wip),
            (TaskStatus::Wip, TaskStatus::Wip),
            (TaskStatus::Done, TaskStatus::Done),
        ] {
            let db = fixtures::db();
            let project = db.insert(&fixtures::project("web")).unwrap();
            let id = fixtures::add_task(&db, project, "login");
            let mut task: Task = db.get(id).unwrap().unwrap();
            task.status = before;
            db.update(id, &task).unwrap();
            start_work(&db, &mut task).unwrap();
            let stored: Task = db.get(id).unwrap().unwrap();
            assert_eq!((task.status, stored.status), (after, after), "{before:?}");
        }
    }

    /// A github project with a real repository in a scratch folder, and two tasks of one title.
    fn repo_world(tag: &str) -> (Db, Project, Vec<Task>) {
        let dir = fixtures::git_repo(tag);
        let db = fixtures::db();
        let mut project = Project {
            base_path: dir.display().to_string(),
            github: true,
            ..fixtures::project("web")
        };
        project.id = Some(db.insert(&project).unwrap());
        let tasks = (0..2)
            .map(|_| {
                let id = fixtures::add_task(&db, project.id.unwrap(), "Fix Login");
                db.get::<Task>(id).unwrap().unwrap()
            })
            .collect();
        (db, project, tasks)
    }

    /// R3: branch and folder are `<id>-<slug>`, and the row has no tmux name yet.
    #[test]
    fn open_makes_the_worktree_the_branch_and_the_row_with_the_id_in_the_slug() {
        let (db, project, tasks) = repo_world("ws-open");
        let slug = |task: &Task| format!("{}-fix-login", task.id());
        let opened: Vec<_> = tasks
            .iter()
            .map(|task| open(&db, &project, task, None).unwrap())
            .collect();
        for (task, config) in tasks.iter().zip(&opened) {
            assert_eq!(
                config.github_branch.as_deref(),
                Some(format!("feat/{}", slug(task)).as_str())
            );
            assert_eq!(config.tmux_session_name, None);
        }
        let path = opened[0].worktree_path.clone().unwrap();
        assert!(
            path.ends_with(&format!(".iter-worktrees/{}", slug(&tasks[0]))),
            "{path}"
        );
        assert!(Path::new(&path).is_dir());
        let stored = db
            .find_session_config_by_task(tasks[0].id())
            .unwrap()
            .unwrap();
        assert_eq!(stored.worktree_path, opened[0].worktree_path);
        let _ = std::fs::remove_dir_all(&project.base_path);
    }

    /// A stale folder of this task is an error naming it, and nothing is written.
    #[test]
    fn a_worktree_folder_that_exists_is_an_error_and_writes_no_row() {
        let (db, project, tasks) = repo_world("ws-stale");
        let task = &tasks[0];
        let folder = format!("{}-fix-login", task.id());
        std::fs::create_dir_all(
            Path::new(&project.base_path)
                .join(".iter-worktrees")
                .join(&folder),
        )
        .unwrap();
        let error = open(&db, &project, task, None).unwrap_err().to_string();
        assert!(error.contains(&folder), "{error}");
        assert!(db.find_session_config_by_task(task.id()).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&project.base_path);
    }

    /// When the row cannot be written (here: the task is not in the database) the worktree and
    /// the branch are taken down again, so a retry does not meet "branch already exists".
    #[test]
    fn a_row_that_cannot_be_written_undoes_the_worktree_and_the_branch() {
        let (db, project, _) = repo_world("ws-undo");
        let ghost = task(999, "Ghost");
        assert!(open(&db, &project, &ghost, None).is_err());
        let worktree = Path::new(&project.base_path).join(".iter-worktrees/999-ghost");
        assert!(!worktree.exists(), "{}", worktree.display());
        let branch = std::process::Command::new("git")
            .args(["rev-parse", "--verify", "-q", "feat/999-ghost"])
            .current_dir(&project.base_path)
            .output()
            .unwrap();
        assert!(!branch.status.success(), "the branch is gone");
        let _ = std::fs::remove_dir_all(&project.base_path);
    }
}
