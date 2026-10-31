//! The prompt that sends an agent to work on a task; shared by the CLI and the web page.

use crate::agent::error::AgentError;
use crate::agent::prompt::{self, Context, Kind};
use crate::agent::taskdir::{self, TaskDir};
use crate::app::App;
use crate::db::Table;
use crate::error::Result;
use crate::models::{Organization, Project, Task};
use crate::notes;
use crate::utils::resolve::{resolve_task_or_current, task_display};
use std::path::{Path, PathBuf};

pub fn locate(app: &App, task: Option<&str>) -> Result<(Project, Task, TaskDir)> {
    let (project, task) = resolve_task_or_current(&app.db, task)?;
    let dir = taskdir::of(&app.db, &project, &task)?;
    Ok((project, task, dir))
}

/// What the project's wiki knows about the task, for the prompt. The only place the agent side reaches for the wiki; never fails: any problem gives no text.
fn onboarding(app: &App, project: &Project, task: &Task) -> String {
    use crate::wiki::{retrieve, scope};
    let spec = scope::Spec {
        task: Some(task_display(project, task)),
        ..scope::Spec::default()
    };
    let Ok(wiki) = scope::wiki(&app.db, &spec) else {
        return String::new();
    };
    let query = keywords(&task.name, &task.description);
    if query.is_empty() {
        return String::new();
    }
    let found = retrieve::find(&wiki, 0, &query, 600);
    let mut lines: Vec<String> = found
        .hits
        .iter()
        .filter(|h| h.class != "Plan")
        .map(|h| match &h.description {
            Some(a) => format!("- `{}` [{}]: {a}", h.id, h.class),
            None => format!("- `{}` [{}]: {}", h.id, h.class, h.title),
        })
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    lines.push(
        "\nOpen one with `iter wiki show <id>`; look for more with `iter wiki find <words>`."
            .to_string(),
    );
    lines.join("\n")
}

/// Words worth looking up: name and start of description, lowercased, short words dropped, at most a dozen.
fn keywords(name: &str, description: &str) -> String {
    let start: String = description.chars().take(300).collect();
    let mut words: Vec<String> = Vec::new();
    for word in format!("{name} {start}")
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
    {
        if word.chars().count() >= 4 && !words.iter().any(|w| w == word) {
            words.push(word.to_string());
        }
    }
    words.truncate(12);
    words.join(" ")
}

/// Folders that may hold `instructions/<kind>.md`, nearest first.
fn override_dirs(app: &App, project: &Project, dir: &TaskDir) -> Result<Vec<PathBuf>> {
    let org = match project.organization_id {
        Some(id) => app.db.get::<Organization>(id)?,
        None => None,
    };
    let mut dirs = vec![dir.root().to_path_buf()];
    dirs.extend(notes::project_dir(project, &org));
    dirs.extend(org.as_ref().and_then(notes::org_dir));
    dirs.extend(crate::config::config_dir().ok());
    Ok(dirs)
}

/// The prompt of `kind`: `template`, else the nearest override, else the built-in, filled from the task and plan.
pub fn prompt_text(
    app: &App,
    task: Option<&str>,
    kind: Kind,
    template: Option<&Path>,
    onboarding_wanted: bool,
) -> Result<String> {
    prompt_for(app, task, kind, template, onboarding_wanted, None, false)
}

/// As [`prompt_text`] with `description` in place of the task's; `defer_worktree` keeps `{{worktree}}` when there is no worktree yet.
pub fn prompt_for(
    app: &App,
    task: Option<&str>,
    kind: Kind,
    template: Option<&Path>,
    onboarding_wanted: bool,
    description: Option<&str>,
    defer_worktree: bool,
) -> Result<String> {
    let (project, task, dir) = locate(app, task)?;
    // the folder the agent writes sketches in must exist
    dir.ensure()?;
    let plan_text = dir.read_plan()?;
    let worktree = app
        .db
        .find_session_config_by_task(task.id())?
        .and_then(|s| s.worktree_path);
    let text = match template {
        Some(path) => std::fs::read_to_string(path)
            .map(|t| crate::agent::instruction::split(&t).1.to_string())
            .map_err(|source| AgentError::TemplateUnreadable {
                path: path.to_path_buf(),
                source,
            })?,
        None => prompt::template(kind, &override_dirs(app, &project, &dir)?).1,
    };
    let known = match onboarding_wanted {
        true => onboarding(app, &project, &task),
        false => String::new(),
    };
    let name = task_display(&project, &task);
    let plan_path = dir.plan_path();
    let ctx = Context {
        task: &name,
        task_name: &task.name,
        task_description: description.unwrap_or(&task.description),
        task_id: task.id(),
        task_dir: dir.root(),
        project_name: &project.name,
        project_path: &project.base_path,
        worktree: worktree
            .as_deref()
            .or(defer_worktree.then_some("{{worktree}}")),
        plan_path: &plan_path,
        plan: plan_text.as_deref(),
        onboarding: &known,
    };
    Ok(prompt::render(&text, kind, &ctx)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::taskdir::save_plan;
    use crate::fixtures;
    use std::path::PathBuf;

    fn world(name: &str) -> (App, PathBuf) {
        let world = fixtures::world(&format!("agentcmd-{name}"));
        (App::with_db(world.db), world.notes)
    }

    #[test]
    fn keywords_are_the_longer_distinct_words() {
        assert_eq!(
            keywords("Fix the retry budget", "Retries hammer the api. Retries!"),
            "retry budget retries hammer"
        );
        assert_eq!(keywords("a", "to be"), "");
    }

    #[test]
    fn the_prompt_names_the_task_the_plan_and_its_folder() {
        let (app, notes) = world("prompt");
        let text = prompt_text(&app, Some("app/Fix it"), Kind::Develop, None, true).unwrap();
        assert!(text.contains("Fix it") && text.contains("Retries hammer the api."));
        assert!(text.contains("(there is no plan yet)"));
        assert!(text.contains(&notes.join("tasks/1/sketches").display().to_string()));
        assert!(!text.contains("{{"));
        assert!(notes.join("tasks/1/sketches").is_dir());

        save_plan(
            &TaskDir::new(notes.join("tasks/1")),
            "## Goal\nStop it.\n## Checklist\n- [ ] one\n",
        )
        .unwrap();
        let text = prompt_text(&app, Some("app/Fix it"), Kind::Plan, None, true).unwrap();
        assert!(text.contains("Stop it.") && text.contains("- [ ] {s1} one"));
    }

    #[test]
    fn the_prompt_brings_in_what_the_wiki_knows() {
        let (app, notes) = world("onboard");
        std::fs::create_dir_all(notes.join("wiki/concepts")).unwrap();
        std::fs::write(
            notes.join("wiki/concepts/backoff.md"),
            "---\ndescription: Retries need exponential backoff\ntags: [net]\n---\n# Backoff\n",
        )
        .unwrap();
        let with = prompt_text(&app, Some("app/Fix it"), Kind::Plan, None, true).unwrap();
        assert!(
            with.contains("`wiki/concepts/backoff` [Concept]: Retries need exponential backoff")
        );
        assert!(with.contains("iter wiki show <id>"));
        let without = prompt_text(&app, Some("app/Fix it"), Kind::Plan, None, false).unwrap();
        assert!(!without.contains("backoff"));
        assert!(without.contains("(nothing was found for this task)"));
    }

    #[test]
    fn a_task_can_bring_its_own_template_and_a_typo_is_refused() {
        let (app, notes) = world("template");
        let dir = notes.join("tasks/1/instructions");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("plan.md"), "Plan {{task.name}} as a haiku.").unwrap();
        assert_eq!(
            prompt_text(&app, Some("app/Fix it"), Kind::Plan, None, false).unwrap(),
            "Plan Fix it as a haiku."
        );
        assert!(
            prompt_text(&app, Some("app/Fix it"), Kind::Fix, None, false)
                .unwrap()
                .contains("Reproduce it first")
        );
        std::fs::write(dir.join("plan.md"), "{{task.nmae}}").unwrap();
        let e = prompt_text(&app, Some("app/Fix it"), Kind::Plan, None, false).unwrap_err();
        assert!(
            e.to_string().contains("unknown placeholder {{task.nmae}}"),
            "{e}"
        );
        let file = notes.join("mine.md");
        std::fs::write(&file, "mine: {{task}}").unwrap();
        assert_eq!(
            prompt_text(&app, Some("app/Fix it"), Kind::Plan, Some(&file), false).unwrap(),
            r"mine: app/Fix\ it"
        );
    }

    #[test]
    fn no_notes_folder_means_the_task_is_kept_in_the_config_folder() {
        let db = fixtures::db();
        let project = db.insert(&fixtures::project("bare")).unwrap();
        fixtures::add_task(&db, project, "x");
        let app = App::with_db(db);
        let (_, _, dir) = locate(&app, Some("bare/x")).unwrap();
        assert!(dir.root().starts_with(crate::config::config_dir().unwrap()));
        assert!(dir.root().ends_with("tasks/1"));
        assert_eq!(locate(&app, Some("1")).unwrap().1.name, "x");
        assert!(locate(&app, Some("99")).is_err());
        let e = locate(&app, Some("bare/missing")).unwrap_err();
        assert!(e.to_string().contains("missing"), "{e}");
    }
}
