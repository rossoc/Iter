//! Addressing: turning what the user typed (a [`TaskRef`], a name) or where they are (iter's
//! variables, the worktree, the tmux session) into the project, task or organization it names.

use crate::db::Db;
use crate::error::{IterError, Result};
use crate::models::{Project, ProjectGroup, Table, Task, TaskId, TaskRef};
use crate::{git, tmux};

/// The task's issue number, or the "no linked issue" error.
pub fn linked_issue(project: &Project, task: &Task) -> Result<i64> {
    task.github_issue
        .ok_or_else(|| IterError::NoLinkedIssue(task.label(project).to_string()))
}

/// The one resolver: the project and task a reference names. An id (`42`, `#42`) always
/// resolves; `<project>/<title>` resolves when the title is unique in the project, else
/// [`IterError::AmbiguousTask`]; in a label (`web/old title #42`) the id wins outright: the project
/// and title it carries are informational.
pub fn task(db: &Db, task_ref: &TaskRef) -> Result<(Project, Task)> {
    match task_ref {
        TaskRef::Id(id) => db
            .task_with_project(id.get())?
            .ok_or_else(|| IterError::NotFound {
                kind: "task",
                name: id.to_string(),
            }),
        TaskRef::Title { project, title } => {
            let project = db.resolve::<Project>(project)?;
            let task = titled(db, &project, title)?;
            Ok((project, task))
        }
        TaskRef::Label(label) => {
            db.task_with_project(label.id)?
                .ok_or_else(|| IterError::TaskNotFound {
                    project: label.project.clone(),
                    task: format!("{} #{}", label.title, label.id),
                })
        }
    }
}

/// A task of `project` by what `--task` of pull and push says: an id (`42`, `#42`) when a task
/// of *that* project has it, otherwise a title, which must be unique.
pub fn task_in(db: &Db, project: &Project, text: &str) -> Result<Task> {
    if let Ok(id) = text.parse::<TaskId>() {
        match db
            .get::<Task>(id.get())?
            .filter(|task| task.project_id == project.id())
        {
            Some(task) => return Ok(task),
            None if text.starts_with('#') => {
                return Err(IterError::TaskNotFound {
                    project: project.name.clone(),
                    task: text.to_string(),
                });
            }
            None => {}
        }
    }
    titled(db, project, text)
}

/// The one task of `project` titled `title`; the ids of the several are in the error.
fn titled(db: &Db, project: &Project, title: &str) -> Result<Task> {
    let mut found = db.tasks_titled(project.id(), title)?;
    match found.len() {
        0 => Err(IterError::TaskNotFound {
            project: project.name.clone(),
            task: title.to_string(),
        }),
        1 => Ok(found.remove(0)),
        _ => Err(IterError::AmbiguousTask {
            project: project.name.clone(),
            title: title.to_string(),
            ids: found.iter().map(Task::id).collect(),
        }),
    }
}

/// The task an agent iter started works on (its id), set on the agent's process.
pub const ENV_TASK: &str = "ITER_TASK";
/// The project an agent iter started works for (its id), when it has no task (a wiki ingest).
pub const ENV_PROJECT: &str = "ITER_PROJECT";

/// What says which task or project "here" is: iter's variables, the directory, and the tmux
/// session (asked only when the others do not answer).
pub struct Here {
    pub task: Option<String>,
    pub project: Option<String>,
    pub cwd: Option<std::path::PathBuf>,
}

impl Here {
    pub fn now() -> Here {
        let var = |name| {
            std::env::var(name)
                .ok()
                .filter(|v: &String| !v.trim().is_empty())
        };
        Here {
            task: var(ENV_TASK),
            project: var(ENV_PROJECT),
            cwd: std::env::current_dir().ok(),
        }
    }
}

/// The current task: `ITER_TASK`, else the task whose worktree holds the current directory, else
/// the task of the tmux session this process runs in. An agent iter started has the variable; a
/// person in a worktree or a tmux session needs nothing.
pub fn current_task(db: &Db) -> Result<(Project, Task)> {
    current_task_at(db, &Here::now(), tmux::current_session_name)
}

pub fn current_task_at(
    db: &Db,
    here: &Here,
    tmux_session: impl FnOnce() -> Option<String>,
) -> Result<(Project, Task)> {
    if let Some(typed) = &here.task {
        return task(db, &typed.trim().parse()?);
    }
    if let Some(cwd) = &here.cwd
        && let Some(task_id) = worktree_task(db, cwd)?
    {
        return task_of(db, task_id);
    }
    let name = tmux_session().ok_or(IterError::NoCurrentTask)?;
    let session_config = db
        .find_session_config_by_tmux_name(&name)?
        .ok_or_else(|| IterError::UntrackedTmuxSession(name.clone()))?;
    task_of(db, session_config.task_id)
}

/// The task a session record names, with its project.
fn task_of(db: &Db, id: i64) -> Result<(Project, Task)> {
    db.task_with_project(id)?.ok_or(IterError::Orphan {
        what: "task",
        of: "session",
    })
}

/// The task whose worktree is `dir` or holds it (the deepest, should worktrees nest).
fn worktree_task(db: &Db, dir: &std::path::Path) -> Result<Option<i64>> {
    let dir = canonical(dir);
    let configs = db.list::<crate::models::SessionConfig>()?;
    Ok(configs
        .iter()
        .filter_map(|c| Some((canonical(c.worktree_path.as_deref()?.as_ref()), c.task_id)))
        .filter(|(path, _)| dir.starts_with(path))
        .max_by_key(|(path, _)| path.components().count())
        .map(|(_, task)| task))
}

/// The project whose folder (`base_path`) is `dir` or holds it, the deepest.
fn folder_project(db: &Db, dir: &std::path::Path) -> Result<Option<Project>> {
    let dir = canonical(dir);
    Ok(db
        .list::<Project>()?
        .into_iter()
        .filter(|p| !p.base_path.trim().is_empty())
        .map(|p| (canonical(p.base_path.as_ref()), p))
        .filter(|(path, _)| dir.starts_with(path))
        .max_by_key(|(path, _)| path.components().count())
        .map(|(_, p)| p))
}

/// `path` with symlinks resolved (`/tmp` is `/private/tmp` on macOS), or as given if it is gone.
fn canonical(path: &std::path::Path) -> std::path::PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The current project: the current task's, else `ITER_PROJECT`, else the project whose folder
/// holds the current directory.
pub fn current_project(db: &Db) -> Result<Project> {
    current_project_at(db, &Here::now(), tmux::current_session_name)
}

pub fn current_project_at(
    db: &Db,
    here: &Here,
    tmux_session: impl FnOnce() -> Option<String>,
) -> Result<Project> {
    match current_task_at(db, here, tmux_session) {
        Ok((project, _)) => return Ok(project),
        // a tmux session iter does not track says nothing about the project either
        Err(IterError::NoCurrentTask | IterError::UntrackedTmuxSession(_)) => {}
        Err(e) => return Err(e),
    }
    if let Some(project) = &here.project {
        let project = project.trim();
        return match project.parse::<i64>() {
            Ok(id) => db.get::<Project>(id)?.ok_or_else(|| IterError::NotFound {
                kind: "project",
                name: project.to_string(),
            }),
            Err(_) => db.resolve(project),
        };
    }
    if let Some(cwd) = &here.cwd
        && let Some(project) = folder_project(db, cwd)?
    {
        return Ok(project);
    }
    Err(IterError::NoCurrentProject)
}

pub fn task_or_current(db: &Db, task_ref: Option<&str>) -> Result<(Project, Task)> {
    match task_ref {
        Some(r) => task(db, &r.parse()?),
        None => current_task(db),
    }
}

pub fn project_or_current(db: &Db, name: Option<&str>) -> Result<Project> {
    match name {
        Some(n) => db.resolve(n),
        None => current_project(db),
    }
}

/// An explicit board/organization name, else the one the tmux session's project is in. Erroring
/// when it is in none is deliberate: membership is optional, so there is no sensible "current" one.
pub fn group_or_current<G: ProjectGroup>(db: &Db, name: Option<&str>) -> Result<G> {
    if let Some(name) = name {
        return db.resolve(name);
    }
    let project = current_project(db)?;
    let id = G::group_of(&project).ok_or_else(|| IterError::NotAMember {
        project: project.name.clone(),
        relation: G::NOT_A_MEMBER,
    })?;
    db.require(id, G::KIND, "project")
}

/// The project a `task pull`/`push` runs against: resolved as usual, then held to `github` and a
/// real repo, which is how `gh` learns the repo.
pub fn github_project(db: &Db, name: Option<&str>) -> Result<Project> {
    let project = project_or_current(db, name)?;
    require_github(&project)?;
    require_git_repo(&project)?;
    Ok(project)
}

pub fn require_github(project: &Project) -> Result<()> {
    match project.github {
        true => Ok(()),
        false => Err(IterError::GithubDisabled(project.name.clone())),
    }
}

/// Holds `project`'s `base_path` to being a repo. Separate from [`require_github`]: `gh` infers the
/// repo from the directory's remote, while `iter session new` needs it for the worktree whether or
/// not `gh` is involved.
pub fn require_git_repo(project: &Project) -> Result<()> {
    match git::is_git_repo(&project.base_path) {
        true => Ok(()),
        false => Err(IterError::NotAGitRepo {
            name: project.name.clone(),
            path: project.base_path.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::IterError;
    use crate::fixtures;
    use crate::models::{SessionConfig, Table};
    use std::path::PathBuf;

    /// Project `web` with `login` (1), `login` (2), `logout` (3); project `api` with `login` (4)
    /// and `2024` (5).
    fn world() -> Db {
        let db = fixtures::db();
        let web = db.insert(&fixtures::project("web")).unwrap();
        let api = db.insert(&fixtures::project("api")).unwrap();
        for (project, title) in [
            (web, "login"),
            (web, "login"),
            (web, "logout"),
            (api, "login"),
            (api, "2024"),
        ] {
            fixtures::add_task(&db, project, title);
        }
        db
    }

    fn by_ref(db: &Db, text: &str) -> Result<i64> {
        task(db, &text.parse::<TaskRef>()?).map(|(_, task)| task.id())
    }

    fn in_web(db: &Db, text: &str) -> Result<i64> {
        let web = db.resolve::<Project>("web")?;
        task_in(db, &web, text).map(|task| task.id())
    }

    /// R2, R3: what `locate::task` resolves, one row per form of reference.
    #[test]
    fn a_reference_resolves_to_its_task_by_id_title_or_label() {
        let db = world();
        for (text, id) in [
            ("3", 3),
            ("#3", 3),
            ("web/logout", 3),
            ("api/login", 4),
            ("web/logout #3", 3),
            ("web/a stale title #3", 3),
            ("web/login #2", 2),
            // the id of the label wins outright, wherever the task is now
            ("web/logout #4", 4),
            ("api/login #1", 1),
        ] {
            assert_eq!(by_ref(&db, text).unwrap(), id, "{text}");
        }
    }

    #[test]
    fn a_reference_that_names_no_task_is_refused_with_its_own_error() {
        let db = world();
        for text in ["#99", "99"] {
            assert!(
                matches!(by_ref(&db, text), Err(IterError::NotFound { .. })),
                "{text}"
            );
        }
        for text in ["web/missing", "api/logout", "web/login #99"] {
            assert!(
                matches!(by_ref(&db, text), Err(IterError::TaskNotFound { .. })),
                "{text}"
            );
        }
        assert!(matches!(
            by_ref(&db, "nosuch/login"),
            Err(IterError::NotFound { .. })
        ));
    }

    #[test]
    fn a_repeated_title_is_ambiguous_and_lists_every_id() {
        let db = world();
        match by_ref(&db, "web/login") {
            Err(IterError::AmbiguousTask {
                project,
                title,
                ids,
            }) => {
                assert_eq!(
                    (project.as_str(), title.as_str(), ids),
                    ("web", "login", vec![1, 2])
                );
            }
            other => panic!("expected AmbiguousTask, got {other:?}"),
        }
    }

    /// R2, R4: `--task`: an id of the project, else a title; another project's id is not found.
    #[test]
    fn a_task_of_the_project_is_found_by_id_then_by_title() {
        let db = world();
        for (text, id) in [("3", 3), ("#3", 3), ("logout", 3), ("#1", 1)] {
            assert_eq!(in_web(&db, text).unwrap(), id, "{text}");
        }
        for text in ["4", "#4", "missing"] {
            assert!(
                matches!(in_web(&db, text), Err(IterError::TaskNotFound { .. })),
                "{text}: task 4 is in `api`, and `pull web --task 4` never acts on it"
            );
        }
        assert!(matches!(
            in_web(&db, "login"),
            Err(IterError::AmbiguousTask { .. })
        ));
    }

    /// With no task 2024 in the project, `2024` is the title of one (`api` has task 5, not `web`).
    #[test]
    fn an_all_digit_title_is_a_title_when_no_task_has_that_id() {
        let db = world();
        let web = db.resolve::<Project>("web").unwrap().id();
        let numeric = fixtures::add_task(&db, web, "2024");
        assert_eq!(in_web(&db, "2024").unwrap(), numeric);
    }

    /// Project `app` in `<tmp>/app` with tasks `a` (worktree `<tmp>/app/.iter-worktrees/a`, tmux
    /// `app/a`) and `b`.
    struct Place {
        db: Db,
        root: PathBuf,
        a: i64,
        b: i64,
    }

    fn place(tag: &str) -> Place {
        let root = fixtures::scratch(tag);
        let worktree = root.join("app").join(".iter-worktrees").join("a");
        std::fs::create_dir_all(worktree.join("src")).unwrap();
        let db = fixtures::db();
        let project = db
            .insert(&Project {
                base_path: root.join("app").display().to_string(),
                ..fixtures::project("app")
            })
            .unwrap();
        let a = fixtures::add_task(&db, project, "a");
        let b = fixtures::add_task(&db, project, "b");
        db.insert(&SessionConfig {
            id: None,
            task_id: a,
            tmux_session_name: Some("app/a".into()),
            github_branch: None,
            worktree_path: Some(worktree.display().to_string()),
        })
        .unwrap();
        Place { db, root, a, b }
    }

    fn here(task: Option<&str>, project: Option<&str>, cwd: Option<PathBuf>) -> Here {
        Here {
            task: task.map(String::from),
            project: project.map(String::from),
            cwd,
        }
    }

    fn no_tmux() -> Option<String> {
        None
    }

    #[test]
    fn the_variable_then_the_worktree_then_tmux_say_which_task() {
        let p = place("resolve-order");
        let in_worktree = Some(p.root.join("app/.iter-worktrees/a/src"));
        let task = |h: &Here, tmux: Option<&str>| {
            current_task_at(&p.db, h, || tmux.map(String::from)).map(|(_, t)| t.id())
        };
        // the variable wins over the directory
        let b = p.b.to_string();
        assert_eq!(
            task(&here(Some(&b), None, in_worktree.clone()), None).unwrap(),
            p.b
        );
        // a subdirectory of the worktree
        assert_eq!(task(&here(None, None, in_worktree), None).unwrap(), p.a);
        // elsewhere: the tmux session
        assert_eq!(
            task(&here(None, None, Some(p.root.clone())), Some("app/a")).unwrap(),
            p.a
        );
        assert!(matches!(
            task(&here(None, None, Some(p.root.clone())), None),
            Err(IterError::NoCurrentTask)
        ));
        std::fs::remove_dir_all(&p.root).unwrap();
    }

    #[test]
    fn without_a_task_the_project_comes_from_the_variable_or_its_folder() {
        let p = place("resolve-project");
        let project = |h: &Here| current_project_at(&p.db, h, no_tmux).map(|p| p.name);
        assert_eq!(project(&here(None, Some("app"), None)).unwrap(), "app");
        assert_eq!(project(&here(None, Some("1"), None)).unwrap(), "app");
        // the project's folder, outside any worktree
        assert_eq!(
            project(&here(None, None, Some(p.root.join("app")))).unwrap(),
            "app"
        );
        assert!(matches!(
            project(&here(None, None, Some(p.root.clone()))),
            Err(IterError::NoCurrentProject)
        ));
        // in a tmux session iter does not track, the folder still answers
        let in_folder = here(None, None, Some(p.root.join("app")));
        let found = current_project_at(&p.db, &in_folder, || Some("other".into())).unwrap();
        assert_eq!(found.name, "app");
        let _ = p.a;
        std::fs::remove_dir_all(&p.root).unwrap();
    }

    fn by_ref_full(db: &Db, text: &str) -> (Project, Task) {
        task(db, &text.parse::<TaskRef>().unwrap()).unwrap()
    }

    /// R3: a task is shown as `<project>/<title> #<id>`, and what is shown reads back.
    #[test]
    fn a_task_is_shown_with_its_id() {
        let db = world();
        let (project, task) = by_ref_full(&db, "3");
        assert_eq!(task.label(&project).to_string(), "web/logout #3");
        assert_eq!(by_ref(&db, &task.label(&project).to_string()).unwrap(), 3);
        let unlinked = IterError::NoLinkedIssue(task.label(&project).to_string());
        assert!(unlinked.to_string().contains("web/logout #3"), "{unlinked}");
    }
}
