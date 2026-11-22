//! The prompt that sends an agent to work on a task; shared by the CLI and the web page.

use crate::agent::instruction::{self, Instruction};
use crate::agent::prompt::{self, Context};
use crate::agent::taskdir::{self, TaskDir};
use crate::app::App;
use crate::db::Table;
use crate::error::AgentError;
use crate::error::Result;
use crate::locate::task_or_current;
use crate::models::{Project, Task};
use std::path::Path;

pub fn locate(app: &App, task: Option<&str>) -> Result<(Project, Task, TaskDir)> {
    let (project, task) = task_or_current(&app.db, task)?;
    let dir = taskdir::of(&app.db, &project, &task)?;
    Ok((project, task, dir))
}

/// What the project's wiki knows about the task, for the prompt. The only place the agent side reaches for the wiki; never fails: any problem gives no text.
fn onboarding(app: &App, task: &Task) -> String {
    use crate::wiki::{retrieve, scope};
    let spec = scope::Spec {
        task: Some(task.id().to_string()),
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
    let mut words: Vec<&str> = Vec::new();
    let text = format!("{name} {start}").to_lowercase();
    for word in text.split(|c: char| !c.is_alphanumeric()) {
        if word.chars().count() >= 4 && !words.contains(&word) {
            words.push(word);
        }
    }
    words.truncate(12);
    words.join(" ")
}

/// Whether `template` has the placeholder `key` (`{{key}}`, spaces allowed inside the braces).
fn uses(template: &str, key: &str) -> bool {
    template
        .split("{{")
        .skip(1)
        .any(|rest| rest.split("}}").next().is_some_and(|k| k.trim() == key))
}

/// The instruction `name` for `task`: the nearest `instructions/<name>.md` in the task's folder,
/// the project's notes, the organization's notes or iter's config folder; an error if none holds it.
pub fn instruction_for(
    app: &App,
    project: &Project,
    task: &Task,
    name: &str,
) -> Result<Instruction> {
    let layers = instruction::layers_for_task(&app.db, project, task)?;
    Ok(instruction::resolve(name, &layers)?)
}

/// The prompt of the instruction `name`: `template` if given, else the nearest instruction file, filled from the task and plan.
pub fn prompt_text(
    app: &App,
    task: Option<&str>,
    name: &str,
    template: Option<&Path>,
    onboarding_wanted: bool,
) -> Result<String> {
    prompt_for(app, task, name, template, onboarding_wanted, None, false)
}

/// As [`prompt_text`] with `description` in place of the task's; `defer_worktree` keeps `{{worktree}}` when there is no worktree yet.
pub fn prompt_for(
    app: &App,
    task: Option<&str>,
    name: &str,
    template: Option<&Path>,
    onboarding_wanted: bool,
    description: Option<&str>,
    defer_worktree: bool,
) -> Result<String> {
    let (project, task, dir) = locate(app, task)?;
    let text = match template {
        Some(path) => std::fs::read_to_string(path)
            .map(|t| instruction::split(&t).1.to_string())
            .map_err(|source| AgentError::TemplateUnreadable {
                path: path.to_path_buf(),
                source,
            })?,
        None => instruction_for(app, &project, &task, name)?.body,
    };
    fill(
        app,
        (&project, &task, &dir),
        name,
        &text,
        onboarding_wanted,
        description,
        defer_worktree,
    )
}

/// `text`, the prompt of the instruction `name`, filled from the task and plan.
pub fn fill(
    app: &App,
    (project, task, dir): (&Project, &Task, &TaskDir),
    name: &str,
    text: &str,
    onboarding_wanted: bool,
    description: Option<&str>,
    defer_worktree: bool,
) -> Result<String> {
    // the folder the agent writes sketches in must exist
    dir.ensure()?;
    let plan_text = dir.read_plan()?;
    let worktree = app
        .db
        .find_session_config_by_task(task.id())?
        .and_then(|s| s.worktree_path);
    let known = if onboarding_wanted {
        onboarding(app, task)
    } else {
        String::new()
    };
    // computed only when asked for: it reads the whole wiki
    let ingest = uses(text, "ingest")
        .then(|| super::ingest_job::task_ingest_text(&app.db, project, task))
        .flatten()
        .unwrap_or_default();
    let shown = task.id().to_string();
    let plan_path = dir.plan_path();
    let ctx = Context {
        task: &shown,
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
        branch_main: &project.default_branch,
        ingest: &ingest,
    };
    Ok(prompt::render(text, name, &ctx)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::taskdir::save_plan;
    use crate::fixtures;
    use std::path::PathBuf;

    /// The agent test world, its notes holding the shipped instructions (the project's layer).
    fn world(name: &str) -> (App, PathBuf) {
        let world = fixtures::world(&format!("agentcmd-{name}"));
        fixtures::install_instructions(&world.notes);
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
        let text = prompt_text(&app, Some("app/Fix it"), "develop", None, true).unwrap();
        assert!(text.contains("Fix it") && text.contains("Retries hammer the api."));
        assert!(text.contains("(there is no plan yet)"));
        // sketches go through the CLI, which knows the folder: the agent needs no path
        assert!(text.contains("iter note sketch new"));
        assert!(!text.contains(&notes.join("tasks/1/sketches").display().to_string()));
        assert!(!text.contains("{{"));
        assert!(notes.join("tasks/1/sketches").is_dir());

        save_plan(
            &TaskDir::new(notes.join("tasks/1")),
            "## Goal\nStop it.\n## Checklist\n- [ ] one\n",
        )
        .unwrap();
        let text = prompt_text(&app, Some("app/Fix it"), "plan", None, true).unwrap();
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
        let with = prompt_text(&app, Some("app/Fix it"), "plan", None, true).unwrap();
        assert!(
            with.contains("`wiki/concepts/backoff` [Concept]: Retries need exponential backoff")
        );
        assert!(with.contains("iter wiki show <id>"));
        let without = prompt_text(&app, Some("app/Fix it"), "plan", None, false).unwrap();
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
            prompt_text(&app, Some("app/Fix it"), "plan", None, false).unwrap(),
            "Plan Fix it as a haiku."
        );
        assert!(
            prompt_text(&app, Some("app/Fix it"), "fix", None, false)
                .unwrap()
                .contains("Reproduce it first")
        );
        std::fs::write(dir.join("plan.md"), "{{task.nmae}}").unwrap();
        let e = prompt_text(&app, Some("app/Fix it"), "plan", None, false).unwrap_err();
        assert!(
            e.to_string().contains("unknown placeholder {{task.nmae}}"),
            "{e}"
        );
        let file = notes.join("mine.md");
        std::fs::write(&file, "mine: {{task}}").unwrap();
        assert_eq!(
            prompt_text(&app, Some("app/Fix it"), "plan", Some(&file), false).unwrap(),
            "mine: 1"
        );
    }

    /// `{{task}}` is the bare id: agents address a task by it, and a title may repeat. The
    /// title is `{{task.name}}`.
    #[test]
    fn a_prompt_names_a_task_whose_title_repeats_by_its_id_only() {
        let (app, notes) = world("repeat");
        let twin = fixtures::add_task(&app.db, 1, "Fix it");
        assert_eq!(twin, 2);
        let file = notes.join("twin.md");
        std::fs::write(&file, "{{task}}|{{task.id}}|{{task.name}}").unwrap();
        for (given, id) in [("1", 1), ("2", 2), ("#2", 2)] {
            assert_eq!(
                prompt_text(&app, Some(given), "plan", Some(&file), false).unwrap(),
                format!("{id}|{id}|Fix it"),
                "{given}"
            );
        }
        let e = prompt_text(&app, Some("app/Fix it"), "plan", Some(&file), false).unwrap_err();
        assert!(e.to_string().contains("#1, #2"), "{e}");
    }

    /// `-i ingest` carries the wiki ingest text for the task: the note waiting, the
    /// rules, and the task's sketches named by command, not by folder.
    #[test]
    fn the_ingest_prompt_lists_the_waiting_notes_and_reads_through_iter() {
        let (app, notes) = world("ingest");
        std::fs::create_dir_all(notes.join("wiki")).unwrap();
        std::fs::write(notes.join("2026-10-27.md"), "Retries need a budget.\n").unwrap();
        let text = prompt_text(&app, Some("app/Fix it"), "ingest", None, false).unwrap();
        assert!(!text.contains("{{"), "{text}");
        assert!(text.contains("Fix it"), "{text}");
        assert!(text.contains("2026-10-27"), "the waiting note: {text}");
        assert!(
            text.contains("## Workflow") && text.contains("## Rules"),
            "{text}"
        );
        assert!(text.contains("iter note sketch list --task"), "{text}");
        assert!(text.contains("git show --stat"), "{text}");
        let sketches = notes.join("tasks/1/sketches").display().to_string();
        assert!(!text.contains(&sketches), "names the folder: {text}");
        assert!(!text.contains("(the project has no wiki)"), "{text}");
    }

    /// A project without a notes folder has no wiki; the prompt says so and does not fail.
    #[test]
    fn the_ingest_prompt_of_a_project_without_notes_says_there_is_no_wiki() {
        let db = fixtures::db();
        let project = db.insert(&fixtures::project("bare")).unwrap();
        fixtures::add_task(&db, project, "x");
        let app = App::with_db(db);
        let ingest = fixtures::shipped_instructions().join("ingest.md");
        let text = prompt_text(&app, Some("bare/x"), "ingest", Some(&ingest), false).unwrap();
        assert!(text.contains("(the project has no wiki)"), "{text}");
        assert!(!text.contains("{{"), "{text}");
    }

    /// `{{branch.main}}` is the project's default branch, `{{ingest}}` works in any template.
    #[test]
    fn a_template_can_name_the_main_branch_and_the_ingest_text() {
        let (app, notes) = world("branch");
        let mut project = app.db.resolve::<Project>("app").unwrap();
        project.default_branch = "trunk".into();
        app.db.update(project.id(), &project).unwrap();
        std::fs::create_dir_all(notes.join("wiki")).unwrap();
        let file = notes.join("close.md");
        std::fs::write(&file, "merge {{branch.main}}\n{{ingest}}").unwrap();
        let text = prompt_text(&app, Some("app/Fix it"), "plan", Some(&file), false).unwrap();
        assert!(text.starts_with("merge trunk\n"), "{text}");
        assert!(!text.contains("{{"), "{text}");
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
