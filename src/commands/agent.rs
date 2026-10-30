//! `iter task plan` and `iter task prompt`: the plan of a task and the prompts
//! that send an agent to work on it. The files are `agent::taskdir`'s; this is
//! only what the command line says about them.

use crate::agent::plan::{self, Plan, Progress, State};
use crate::agent::prompt::{self, Context, Kind};
use crate::agent::taskdir::{self, TaskDir};
use crate::app::App;
use crate::db::Table;
use crate::error::{IterError, Result};
use crate::md_edit::edit_text;
use crate::models::{Organization, Project, Task};
use crate::notes;
use crate::utils::output::{Format as DataFormat, emit};
use crate::utils::resolve::{resolve_task_or_current, task_display};
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};

fn failed(message: impl Into<String>) -> IterError {
    IterError::CommandFailed(message.into())
}

/// The task a command is about, with its project and its folder.
pub(super) fn locate(app: &App, task: Option<&str>) -> Result<(Project, Task, TaskDir)> {
    let (project, task) = resolve_task_or_current(&app.db, task)?;
    let dir = taskdir::of(&app.db, &project, &task)?;
    Ok((project, task, dir))
}

#[derive(Serialize)]
struct PlanView {
    task: String,
    path: String,
    exists: bool,
    plan: Plan,
    progress: Progress,
    problems: Vec<String>,
}

fn view(task: &str, dir: &TaskDir, content: Option<&str>) -> PlanView {
    let text = content.unwrap_or("");
    PlanView {
        task: task.to_string(),
        path: dir.plan_path().display().to_string(),
        exists: content.is_some(),
        plan: plan::parse(text),
        progress: plan::parse(text).progress(),
        problems: if content.is_some() {
            plan::problems(text)
        } else {
            Vec::new()
        },
    }
}

fn progress_line(p: &Progress) -> String {
    let mut line = format!("{}/{} steps done", p.done, p.total);
    if p.doing > 0 {
        line.push_str(&format!(", {} in progress", p.doing));
    }
    if p.verify_total > 0 {
        line.push_str(&format!(" (checks {}/{})", p.verify_done, p.verify_total));
    }
    line
}

/// What saving a plan did: how many steps the plan has, and what it is missing.
pub struct Saved {
    pub steps: usize,
    pub problems: Vec<String>,
}

/// Saves `text` as the plan of `dir`: every step numbered, the rest as it was
/// written. A plan with nothing in it is refused; one that lacks a Goal or a
/// Checklist is saved, and says so.
pub fn save_plan(dir: &TaskDir, text: &str) -> Result<Saved> {
    if text.trim().is_empty() {
        return Err(failed("the plan is empty: nothing was saved"));
    }
    let mut text = plan::normalize(text);
    if !text.ends_with('\n') {
        text.push('\n');
    }
    // one level of undo: a plan replaced by a worse one is not lost
    if let Some(old) = dir.read_plan()?
        && old != text
    {
        std::fs::write(dir.previous_plan_path(), old)?;
    }
    dir.write_plan(&text)?;
    Ok(Saved {
        steps: plan::parse(&text).items.len(),
        problems: plan::problems(&text),
    })
}

/// Puts step `id` of the plan of `dir` in `state`; the plan's progress after.
pub fn mark_step(dir: &TaskDir, id: &str, state: State) -> Result<Progress> {
    let content = dir
        .read_plan()?
        .ok_or_else(|| failed("there is no plan yet: write one with `iter task plan`"))?;
    let marked = plan::mark(&content, id, state).map_err(|e| failed(e.to_string()))?;
    dir.write_plan(&marked)?;
    Ok(plan::parse(&marked).progress())
}

/// What `iter task plan` was asked: alone it edits, `--show` prints,
/// `--stdin` replaces, `--mark` ticks a step.
pub struct PlanRequest<'a> {
    pub task: Option<&'a str>,
    pub show: bool,
    pub stdin: bool,
    /// `[step, state]`, or empty.
    pub mark: &'a [String],
    pub format: DataFormat,
}

pub fn plan(app: &App, req: &PlanRequest) -> Result<()> {
    let (project, task, dir) = locate(app, req.task)?;
    let name = task_display(&project, &task);
    let word = prompt::shell_word(&name);
    if let [step, state] = req.mark {
        let state = <State as clap::ValueEnum>::from_str(state, true)
            .map_err(|_| failed(format!("`{state}` is not a state: todo, doing or done")))?;
        let p = mark_step(&dir, step, state)?;
        println!("{step}: {} -- {}", state.label(), progress_line(&p));
        return Ok(());
    }
    if req.show {
        let content = dir.read_plan()?;
        let v = view(&name, &dir, content.as_deref());
        return emit(req.format, &v, || match &content {
            None => format!(
                "No plan yet. Write one with `iter task plan {word}`, or send an agent: `iter task prompt {word} --kind plan`.\n"
            ),
            Some(text) => format!("{}\n\n{}\n", text.trim_end(), progress_line(&v.progress)),
        });
    }
    let text = if req.stdin {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Some(buf)
    } else {
        let start = match dir.read_plan()? {
            Some(existing) => existing,
            None => plan::skeleton(&task.description),
        };
        edit_text(&start)?
    };
    let Some(text) = text else {
        println!("no changes -- plan not saved");
        return Ok(());
    };
    let saved = save_plan(&dir, &text)?;
    for problem in &saved.problems {
        eprintln!("warning: {problem}");
    }
    println!("saved the plan of {name}: {} steps", saved.steps);
    Ok(())
}

/// What the project's wiki knows that bears on the task, as a few lines for
/// the prompt. This is the only place the agent side reaches for the wiki:
/// to use another source of knowledge, change this function. It never fails
/// the prompt: no wiki, nothing found or any error all give no text.
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
        .map(|h| match &h.abstract_ {
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

/// The words of a task worth looking up: its name and the start of its
/// description, lowercased, short words dropped, at most a dozen.
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

/// The folders that may hold a `prompts/<kind>.md` for this task, nearest
/// first: the task's, its project's notes, its organization's notes.
fn override_dirs(app: &App, project: &Project, dir: &TaskDir) -> Result<Vec<PathBuf>> {
    let org = match project.organization_id {
        Some(id) => app.db.get::<Organization>(id)?,
        None => None,
    };
    let mut dirs = vec![dir.root().to_path_buf()];
    dirs.extend(notes::project_dir(project, &org));
    dirs.extend(org.as_ref().and_then(notes::org_dir));
    Ok(dirs)
}

/// The prompt of `kind` for the task: the template (`template` if given,
/// else the nearest override, else the built-in one) filled from the task and
/// the plan, with `onboarding` from the wiki if asked.
pub fn prompt_text(
    app: &App,
    task: Option<&str>,
    kind: Kind,
    template: Option<&Path>,
    onboarding_wanted: bool,
) -> Result<String> {
    prompt_for(app, task, kind, template, onboarding_wanted, None)
}

/// As [`prompt_text`], with `description` told to the agent in place of the
/// task's own (`solve` lets it be edited for one run).
pub fn prompt_for(
    app: &App,
    task: Option<&str>,
    kind: Kind,
    template: Option<&Path>,
    onboarding_wanted: bool,
    description: Option<&str>,
) -> Result<String> {
    let (project, task, dir) = locate(app, task)?;
    // an agent following the prompt writes sketches here: have the folder
    dir.ensure()?;
    let plan_text = dir.read_plan()?;
    let worktree = app
        .db
        .find_session_config_by_task(task.id())?
        .and_then(|s| s.worktree_path);
    let text = match template {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|e| failed(format!("cannot read {}: {e}", path.display())))?,
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
        worktree: worktree.as_deref(),
        plan_path: &plan_path,
        plan: plan_text.as_deref(),
        onboarding: &known,
    };
    prompt::render(&text, kind, &ctx).map_err(failed)
}

pub fn prompt_cmd(
    app: &App,
    task: Option<&str>,
    kind: Kind,
    template: Option<&Path>,
    onboarding_wanted: bool,
) -> Result<()> {
    print!(
        "{}",
        prompt_text(app, task, kind, template, onboarding_wanted)?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::models::TaskStatus;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("iter-agentcmd-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// An app with the project `app` (notes in a scratch folder) and its task `Fix it`.
    fn world(name: &str) -> (App, PathBuf) {
        let notes = scratch(name);
        let db = fixtures::db();
        let mut p = fixtures::project("app");
        p.notes_path = notes.display().to_string();
        let project = db.insert(&p).expect("project");
        db.insert(&Task {
            id: None,
            project_id: project,
            name: "Fix it".into(),
            description: "Retries hammer the api.".into(),
            github_issue: None,
            status: TaskStatus::Queue,
            branch_prefix: String::new(),
            matrix_placed: false,
            start_time: None,
            duration: None,
        })
        .expect("task");
        (App::with_db(db), notes)
    }

    #[test]
    fn saving_numbers_the_steps_and_says_what_is_missing() {
        let dir = TaskDir::new(scratch("save").join("tasks/1"));
        let saved = save_plan(
            &dir,
            "## Goal\nx\n\n## Checklist\n- [ ] a\n- [ ] (verify) b",
        )
        .unwrap();
        assert_eq!((saved.steps, saved.problems.len()), (2, 0));
        assert_eq!(
            dir.read_plan().unwrap().unwrap(),
            "## Goal\nx\n\n## Checklist\n- [ ] {s1} a\n- [ ] {s2} (verify) b\n"
        );
        let saved = save_plan(&dir, "only prose").unwrap();
        assert_eq!(saved.steps, 0);
        assert_eq!(saved.problems.len(), 2);
        assert!(save_plan(&dir, "  \n").is_err());
        // the empty plan did not replace the last one
        assert_eq!(dir.read_plan().unwrap().unwrap(), "only prose\n");
        // the plan before the last one is kept, once
        let previous = std::fs::read_to_string(dir.previous_plan_path()).unwrap();
        assert!(previous.contains("{s1} a"), "{previous}");
        save_plan(&dir, "only prose").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.previous_plan_path()).unwrap(),
            previous
        );
    }

    #[test]
    fn marking_a_step_edits_only_its_checkbox_and_reports_progress() {
        let dir = TaskDir::new(scratch("mark").join("tasks/1"));
        assert!(
            mark_step(&dir, "s1", State::Done)
                .unwrap_err()
                .to_string()
                .contains("no plan yet")
        );
        save_plan(&dir, "## Goal\ng\n## Checklist\n- [ ] a\n- [ ] b\n  more\n").unwrap();
        let p = mark_step(&dir, "s2", State::Doing).unwrap();
        assert_eq!((p.done, p.doing, p.total), (0, 1, 2));
        let p = mark_step(&dir, "s1", State::Done).unwrap();
        assert_eq!((p.done, p.doing, p.total), (1, 1, 2));
        assert_eq!(
            dir.read_plan().unwrap().unwrap(),
            "## Goal\ng\n## Checklist\n- [x] {s1} a\n- [~] {s2} b\n  more\n"
        );
        let e = mark_step(&dir, "s9", State::Done).unwrap_err().to_string();
        assert_eq!(e, "no step 's9' (the steps are s1, s2)");
    }

    #[test]
    fn the_plan_view_is_the_plan_and_its_progress() {
        let dir = TaskDir::new(PathBuf::from("/t"));
        let v = view(
            "app/Fix it",
            &dir,
            Some("## Goal\ng\n## Checklist\n- [x] {s1} a\n- [ ] {s2} b\n"),
        );
        assert!(v.exists && v.problems.is_empty());
        assert_eq!(progress_line(&v.progress), "1/2 steps done");
        let none = view("app/Fix it", &dir, None);
        assert!(!none.exists && none.plan.items.is_empty() && none.problems.is_empty());
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
        let text = prompt_text(&app, Some("app/Fix it"), Kind::Execute, None, true).unwrap();
        assert!(text.contains("Fix it") && text.contains("Retries hammer the api."));
        assert!(text.contains("(there is no plan yet)"));
        assert!(text.contains(&notes.join("tasks/1/sketches").display().to_string()));
        assert!(!text.contains("{{"));
        // the folder the agent is told to write sketches in exists
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
            "---\nabstract: Retries need exponential backoff\ntags: [net]\n---\n# Backoff\n",
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
        let dir = notes.join("tasks/1/prompts");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("plan.md"), "Plan {{task.name}} as a haiku.").unwrap();
        assert_eq!(
            prompt_text(&app, Some("app/Fix it"), Kind::Plan, None, false).unwrap(),
            "Plan Fix it as a haiku."
        );
        // the other kinds are still built in
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
        // an explicit template beats the override
        let file = notes.join("mine.md");
        std::fs::write(&file, "mine: {{task}}").unwrap();
        assert_eq!(
            prompt_text(&app, Some("app/Fix it"), Kind::Plan, Some(&file), false).unwrap(),
            r"mine: app/Fix\ it"
        );
    }

    #[test]
    fn no_notes_folder_is_an_error_that_says_what_to_set() {
        let db = fixtures::db();
        let project = db.insert(&fixtures::project("bare")).unwrap();
        db.insert(&Task {
            id: None,
            project_id: project,
            name: "x".into(),
            description: String::new(),
            github_issue: None,
            status: TaskStatus::Queue,
            branch_prefix: String::new(),
            matrix_placed: false,
            start_time: None,
            duration: None,
        })
        .unwrap();
        let app = App::with_db(db);
        let e = prompt_text(&app, Some("bare/x"), Kind::Plan, None, false).unwrap_err();
        assert!(e.to_string().contains("set `notes_path`"), "{e}");
        let e = locate(&app, Some("bare/missing")).unwrap_err();
        assert!(e.to_string().contains("missing"), "{e}");
    }
}
