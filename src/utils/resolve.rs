//! Addressing: turning what the user typed, or where they are (iter's variables, the worktree,
//! the tmux session), into the project, task or organization it names.

use crate::db::{Db, Table};
use crate::error::{IterError, Result};
use crate::models::{Project, ProjectGroup, Task, TaskId, TaskRef, split_label};
use crate::{git, tmux};

/// The task's issue number, or the "no linked issue" error.
pub(crate) fn linked_issue(project: &Project, task: &Task) -> Result<i64> {
    task.github_issue
        .ok_or_else(|| IterError::NoLinkedIssue(task.label(project).to_string()))
}

/// A task by its id (`12` or `#12`: what a command run by an agent says, with no space to quote),
/// or by `<project>/<title>` (refused when the title repeats) or `<project>/<title> #<id>` (the id
/// wins over a stale title).
pub(crate) fn resolve_task(db: &Db, text: &str) -> Result<(Project, Task)> {
    let (project, task) = match text.parse()? {
        TaskRef::Id(TaskId(id)) => {
            let task: Task = db.get(id)?.ok_or_else(|| IterError::NotFound {
                kind: "task",
                name: text.to_string(),
            })?;
            (db.require(task.project_id, "project", "task")?, task)
        }
        TaskRef::Title { project, title } => {
            let project = db.resolve::<Project>(&project)?;
            let task = find_task_or_err(db, &project, &title)?;
            (project, task)
        }
        TaskRef::Label(label) => {
            let project = db.resolve::<Project>(&label.project)?;
            let task = task_in(db, &project, label.id)?.ok_or_else(|| IterError::TaskNotFound {
                project: project.name.clone(),
                task: format!("{} {}", label.title, TaskId(label.id)),
            })?;
            (project, task)
        }
    };
    Ok((project, task))
}

/// The task `id` when it belongs to `project`.
fn task_in(db: &Db, project: &Project, id: i64) -> Result<Option<Task>> {
    Ok(db
        .get::<Task>(id)?
        .filter(|task| task.project_id == project.id()))
}

/// The task an agent iter started works on (its id), set on the agent's process.
pub(crate) const ENV_TASK: &str = "ITER_TASK";
/// The project an agent iter started works for (its id), when it has no task (a wiki ingest).
pub(crate) const ENV_PROJECT: &str = "ITER_PROJECT";

/// What says which task or project "here" is: iter's variables, the directory, and the tmux
/// session (asked only when the others do not answer).
pub(crate) struct Here {
    pub task: Option<String>,
    pub project: Option<String>,
    pub cwd: Option<std::path::PathBuf>,
}

impl Here {
    pub(crate) fn now() -> Here {
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
pub(crate) fn current_task(db: &Db) -> Result<(Project, Task)> {
    current_task_at(db, &Here::now(), tmux::current_session_name)
}

pub(crate) fn current_task_at(
    db: &Db,
    here: &Here,
    tmux_session: impl FnOnce() -> Option<String>,
) -> Result<(Project, Task)> {
    if let Some(task) = &here.task {
        return resolve_task(db, task.trim());
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
pub(crate) fn current_project(db: &Db) -> Result<Project> {
    current_project_at(db, &Here::now(), tmux::current_session_name)
}

pub(crate) fn current_project_at(
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

pub(crate) fn resolve_task_or_current(db: &Db, task_ref: Option<&str>) -> Result<(Project, Task)> {
    match task_ref {
        Some(r) => resolve_task(db, r),
        None => current_task(db),
    }
}

pub(crate) fn resolve_project_or_current(db: &Db, name: Option<&str>) -> Result<Project> {
    match name {
        Some(n) => db.resolve(n),
        None => current_project(db),
    }
}

/// An explicit board/organization name, else the one the tmux session's project is in. Erroring
/// when it is in none is deliberate: membership is optional, so there is no sensible "current" one.
pub(crate) fn resolve_group_or_current<G: ProjectGroup>(db: &Db, name: Option<&str>) -> Result<G> {
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

/// A task of `project` by what `--task` or a path says: `#12`, `<title> #12`, a plain `12` (the id
/// when the project has that task, else a title) or a title, which must be unique.
pub(crate) fn find_task_or_err(db: &Db, project: &Project, text: &str) -> Result<Task> {
    let id = text.parse::<TaskId>().ok().map(|id| id.0);
    let labelled = match text.starts_with('#') {
        true => id,
        false => split_label(text).map(|(_, id)| id),
    };
    if let Some(id) = labelled {
        return task_in(db, project, id)?.ok_or_else(|| IterError::TaskNotFound {
            project: project.name.clone(),
            task: text.to_string(),
        });
    }
    match id.map(|id| task_in(db, project, id)).transpose()?.flatten() {
        Some(task) => Ok(task),
        None => one_titled(db, project, text),
    }
}

/// The one task of `project` titled `title`; the ids of the several are in the error.
fn one_titled(db: &Db, project: &Project, title: &str) -> Result<Task> {
    let mut found = db.find_tasks(project.id(), title)?;
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

/// The project a `task pull`/`push` runs against: resolved as usual, then held to `github` and a
/// real repo, which is how `gh` learns the repo.
pub(crate) fn github_project(db: &Db, name: Option<&str>) -> Result<Project> {
    let project = resolve_project_or_current(db, name)?;
    require_github(&project)?;
    require_git_repo(&project)?;
    Ok(project)
}

pub(crate) fn require_github(project: &Project) -> Result<()> {
    match project.github {
        true => Ok(()),
        false => Err(IterError::GithubDisabled(project.name.clone())),
    }
}

/// Holds `project`'s `base_path` to being a repo. Separate from [`require_github`]: `gh` infers the
/// repo from the directory's remote, while `iter session new` needs it for the worktree whether or
/// not `gh` is involved.
pub(crate) fn require_git_repo(project: &Project) -> Result<()> {
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
    use crate::fixtures;
    use crate::models::SessionConfig;
    use std::path::PathBuf;

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

    /// Project `web` with tasks `login` (1) and `logout` (2); project `api` with `login` (3).
    fn distinct_titles() -> Db {
        let db = fixtures::db();
        let web = db.insert(&fixtures::project("web")).unwrap();
        let api = db.insert(&fixtures::project("api")).unwrap();
        for (project, title) in [(web, "login"), (web, "logout"), (api, "login")] {
            fixtures::add_task(&db, project, title);
        }
        db
    }

    /// Project `web` with two tasks titled `login` (1, 2) and one titled `logout` (3); project
    /// `api` with a task titled `login` (4).
    fn repeated_titles() -> Db {
        let db = fixtures::db();
        let web = db.insert(&fixtures::project("web")).unwrap();
        let api = db.insert(&fixtures::project("api")).unwrap();
        for (project, title) in [
            (web, "login"),
            (web, "login"),
            (web, "logout"),
            (api, "login"),
        ] {
            fixtures::add_task(&db, project, title);
        }
        db
    }

    fn found(db: &Db, text: &str) -> std::result::Result<i64, IterError> {
        resolve_task(db, text).map(|(_, task)| task.id())
    }

    /// R2: a title that names one task of the project resolves; one that names several is
    /// refused with an error that lists the `#<id>` of each.
    #[test]
    fn a_repeated_title_is_ambiguous_and_names_the_ids() {
        let db = repeated_titles();
        assert_eq!(found(&db, "web/logout").unwrap(), 3);
        assert_eq!(
            found(&db, "api/login").unwrap(),
            4,
            "unique in its own project"
        );
        match found(&db, "web/login") {
            Err(IterError::AmbiguousTask {
                project,
                title,
                ids,
            }) => {
                assert_eq!((project.as_str(), title.as_str()), ("web", "login"));
                assert_eq!(ids, [1, 2]);
            }
            other => panic!("expected AmbiguousTask, got {other:?}"),
        }
        assert!(matches!(
            found(&db, "web/missing"),
            Err(IterError::TaskNotFound { .. })
        ));
    }

    /// R2: `42` and `#42` always resolve, repeated title or not.
    #[test]
    fn an_id_with_or_without_the_hash_resolves() {
        let db = distinct_titles();
        for text in ["2", "#2"] {
            assert_eq!(found(&db, text).unwrap(), 2, "{text}");
        }
        assert!(matches!(found(&db, "#99"), Err(IterError::NotFound { .. })));
    }

    /// R3: in a pasted label the id wins: the title may be stale, the project must match.
    #[test]
    fn in_a_label_the_id_wins_over_the_title() {
        let db = distinct_titles();
        assert_eq!(found(&db, "web/login #1").unwrap(), 1);
        assert_eq!(
            found(&db, "web/an old title #1").unwrap(),
            1,
            "renamed since"
        );
        assert!(
            matches!(
                found(&db, "api/login #1"),
                Err(IterError::TaskNotFound { .. })
            ),
            "task 1 is not in `api`"
        );
        // a trailing `#<id>` is always the id, even where a task is titled `fix #12`
        let web = db.resolve::<Project>("web").unwrap().id();
        let odd = fixtures::add_task(&db, web, "fix #12");
        assert_eq!(odd, 4);
        assert!(matches!(
            found(&db, "web/fix #12"),
            Err(IterError::TaskNotFound { .. })
        ));
        assert_eq!(found(&db, "#4").unwrap(), 4, "reachable by its own id");
    }

    /// R3: a task is shown as `<project>/<title> #<id>`, and what is shown reads back.
    #[test]
    fn a_task_is_shown_with_its_id() {
        let db = distinct_titles();
        let (project, task) = resolve_task(&db, "2").unwrap();
        assert_eq!(task.label(&project).to_string(), "web/logout #2");
        assert_eq!(found(&db, &task.label(&project).to_string()).unwrap(), 2);
        let unlinked = IterError::NoLinkedIssue(task.label(&project).to_string());
        assert!(unlinked.to_string().contains("web/logout #2"), "{unlinked}");
    }

    /// R2, R4: `--task` of pull and push names a task of *that* project, by id or by title, and an
    /// all-digit title still works as a title.
    #[test]
    fn a_task_of_the_project_is_found_by_id_or_by_title() {
        let db = distinct_titles();
        let web = db.resolve::<Project>("web").unwrap();
        let id = |text: &str| find_task_or_err(&db, &web, text).map(|t| t.id());
        assert_eq!(id("2").unwrap(), 2);
        assert_eq!(id("#2").unwrap(), 2);
        assert_eq!(id("logout").unwrap(), 2);
        assert!(
            matches!(id("3"), Err(IterError::TaskNotFound { .. })),
            "task 3 is in `api`: `pull web --task 3` never acts on it"
        );
        let numeric = fixtures::add_task(&db, web.id(), "2024");
        assert_eq!(id("2024").unwrap(), numeric, "no task 2024: the title");
    }

    /// R2: `--task <title>` of a repeated title is the same ambiguity.
    #[test]
    fn a_repeated_title_given_to_task_in_a_project_is_ambiguous() {
        let db = repeated_titles();
        let web = db.resolve::<Project>("web").unwrap();
        assert!(matches!(
            find_task_or_err(&db, &web, "login"),
            Err(IterError::AmbiguousTask { .. })
        ));
    }
}
