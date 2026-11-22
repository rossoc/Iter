//! A task's workspace: the worktree, branch, tmux name and the work-in-progress mark. From
//! `utils::workspace`, `commands/session.rs` and `agent/solve.rs` in isolation step 4; the
//! whole module is a stub until then.

use crate::db::Db;
use crate::error::Result;
use crate::models::{Project, SessionConfig, Task};

/// The tmux session name of a new session, `<project>/<id> <title>`, each half flattened (tmux
/// reads `/`, `:`, `.` in a target). Names already stored are read back, never recomputed.
// stub: I3 (`commands::session::tmux_session_name` still does the work)
#[allow(dead_code)]
pub fn tmux_name(project: &Project, task: &Task) -> String {
    let _ = (project, task);
    unimplemented!("stub: I3")
}

/// The worktree, branch and `session_configs` row of `task` (no tmux name yet), undone when the
/// row cannot be written. The slug carries the id, so equal titles never collide, and a worktree
/// folder that already exists is an error naming it.
// stub: I3 (`create_task_worktree` plus the two callers' row code still do the work)
#[allow(dead_code)]
pub fn open(
    db: &Db,
    project: &Project,
    task: &Task,
    branch: Option<&str>,
) -> Result<SessionConfig> {
    let _ = (db, project, task, branch);
    unimplemented!("stub: I3")
}

/// Marks a queued task in progress when work starts on it; a done task stays done.
// stub: I3 (`agent::solve::start_work` still does the work)
#[allow(dead_code)]
pub fn start_work(db: &Db, task: &mut Task) -> Result<()> {
    let _ = (db, task);
    unimplemented!("stub: I3")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::models::Table;
    use crate::models::TaskStatus;

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
    fn repo_world(tag: &str) -> Option<(Db, Project, Vec<Task>)> {
        let dir = fixtures::git_repo(tag)?;
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
        Some((db, project, tasks))
    }

    /// R3: branch and folder are `<id>-<slug>`, and the row has no tmux name yet.
    #[test]
    fn open_makes_the_worktree_the_branch_and_the_row_with_the_id_in_the_slug() {
        let Some((db, project, tasks)) = repo_world("ws-open") else {
            return;
        };
        let (first, second) = (&tasks[0], &tasks[1]);
        let one = open(&db, &project, first, None).unwrap();
        let two = open(&db, &project, second, None).unwrap();
        let slug = |task: &Task| format!("{}-fix-login", task.id());
        assert_eq!(
            one.github_branch.as_deref(),
            Some(format!("feat/{}", slug(first)).as_str())
        );
        assert_eq!(
            two.github_branch.as_deref(),
            Some(format!("feat/{}", slug(second)).as_str())
        );
        let path = one.worktree_path.clone().unwrap();
        assert!(
            path.ends_with(&format!(".iter-worktrees/{}", slug(first))),
            "{path}"
        );
        assert!(std::path::Path::new(&path).is_dir());
        assert_eq!(one.tmux_session_name, None);
        let stored = db.find_session_config_by_task(first.id()).unwrap().unwrap();
        assert_eq!(stored.worktree_path, one.worktree_path);
        let _ = std::fs::remove_dir_all(&project.base_path);
    }

    /// A stale folder of this task is an error naming it, and nothing is written.
    #[test]
    fn a_worktree_folder_that_exists_is_an_error_and_writes_no_row() {
        let Some((db, project, tasks)) = repo_world("ws-stale") else {
            return;
        };
        let task = &tasks[0];
        let folder = format!("{}-fix-login", task.id());
        std::fs::create_dir_all(
            std::path::Path::new(&project.base_path)
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
        let Some((db, project, _)) = repo_world("ws-undo") else {
            return;
        };
        let ghost = task(999, "Ghost");
        assert!(open(&db, &project, &ghost, None).is_err());
        let worktree = std::path::Path::new(&project.base_path).join(".iter-worktrees/999-ghost");
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
