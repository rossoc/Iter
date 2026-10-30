//! The Agent tab of the task page: the task's plan (goal, how, the checklist
//! with the state of every step), a step opened beside it, and the notes and
//! sketches kept in the task's folder.
//!
//! The files are `crate::agent`'s; this is what loads them for the page, the
//! view, and the one route (`POST /task/{id}/plan/{step}`) that moves a step
//! between todo, doing and done. A step is a link (`?tab=agent&step=s3`), so
//! opening one needs no script.

use super::ui::button::post_button;
use super::ui::empty_line::empty_line;
use super::ui::field::{field, select};
use super::ui::labelled::labelled_section;
use super::ui::layout::info_columns;
use super::ui::panel::{Row, panel};
use super::ui::prose::description;
use super::ui::{CIRCLE, CIRCLE_CHECK, HALF, icon};
use super::url::{agent_tab_url, plan_step_url, solve_url, task_step_url, task_url};
use super::{Id, open_db};
use crate::agent::plan::{self, Item, Plan, Progress, State};
use crate::agent::prompt::{Kind, shell_word};
use crate::agent::run::{Harness, Run};
use crate::agent::taskdir::{self, TaskDir};
use crate::commands::solve::{self, Spec};
use crate::db::{Db, Table};
use crate::error::Result as Loaded;
use crate::models::{Project, Task};
use crate::utils::resolve::task_display;
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        content::Form,
        error::{RouterErrorExt, bad_request, see_other},
        path_param,
        response::Response,
        route,
    },
    view::{View, component, view},
};

path_param!(step: String, error = bad_request);

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.route(mark_step).route(start_run)
}

/// The most of a file the page reads, and of how many files per list.
const FILE_BYTES: usize = 64 * 1024;
const FILES: usize = 50;

/// A note or a sketch, as the page shows it.
pub struct Doc {
    pub name: String,
    pub text: String,
    /// The file is longer than what is shown.
    pub cut: bool,
}

/// A run and the end of its log.
pub struct RunView {
    pub run: Run,
    pub log: String,
    /// The log is longer than what is shown.
    pub cut: bool,
}

/// What the Agent tab shows.
pub struct AgentTab {
    pub task_id: i64,
    /// `<project>/<task>`, as the commands take it.
    pub task: String,
    /// `None` when neither the project nor its organization has a notes
    /// folder to keep the task in.
    pub dir: Option<TaskDir>,
    /// The plan's text, when there is a plan.
    pub plan_text: Option<String>,
    pub plan: Plan,
    /// The step open beside the plan.
    pub step: Option<String>,
    pub notes: Vec<Doc>,
    pub sketches: Vec<Doc>,
    /// Latest first.
    pub runs: Vec<RunView>,
}

fn docs(dir: &std::path::Path) -> Vec<Doc> {
    TaskDir::files(dir)
        .into_iter()
        .take(FILES)
        .map(|file| {
            let text = std::fs::read_to_string(&file.path).unwrap_or_default();
            let cut = text.len() > FILE_BYTES;
            let end = (0..=FILE_BYTES.min(text.len()))
                .rev()
                .find(|&i| text.is_char_boundary(i))
                .unwrap_or(0);
            Doc {
                name: file.name,
                text: text[..end].to_string(),
                cut,
            }
        })
        .collect()
}

/// The most of a log the page shows, from its end, and how many runs.
const LOG_BYTES: usize = 16 * 1024;
const RUNS: usize = 10;

fn run_views(dir: &TaskDir) -> Vec<RunView> {
    dir.runs()
        .into_iter()
        .rev()
        .take(RUNS)
        .map(|run| {
            let text = std::fs::read_to_string(dir.run_log_path(run.n)).unwrap_or_default();
            let cut = text.len() > LOG_BYTES;
            let start = (text.len().saturating_sub(LOG_BYTES)..=text.len())
                .find(|&i| text.is_char_boundary(i))
                .unwrap_or(text.len());
            RunView {
                log: text[start..].to_string(),
                cut,
                run,
            }
        })
        .collect()
}

/// Reads what the tab shows. `step` is the `?step=` of the request.
pub fn load(db: &Db, project: &Project, task: &Task, step: Option<&str>) -> Loaded<AgentTab> {
    let dir = taskdir::of(db, project, task).ok();
    let plan_text = match &dir {
        Some(dir) => dir.read_plan()?,
        None => None,
    };
    let (notes, sketches) = match &dir {
        Some(dir) => (docs(&dir.notes_dir()), docs(&dir.sketches_dir())),
        None => (Vec::new(), Vec::new()),
    };
    let runs = dir.as_ref().map(run_views).unwrap_or_default();
    Ok(AgentTab {
        task_id: task.id(),
        runs,
        task: task_display(project, task),
        plan: plan::parse(plan_text.as_deref().unwrap_or("")),
        dir,
        plan_text,
        step: step.map(String::from),
        notes,
        sketches,
    })
}

/// "3 of 7 done, 1 in progress. Checks 1 of 2."
fn summary(p: &Progress) -> String {
    let mut text = format!("{} of {} done", p.done, p.total);
    if p.doing > 0 {
        text.push_str(&format!(", {} in progress", p.doing));
    }
    if p.verify_total > 0 {
        text.push_str(&format!(". Checks {} of {}", p.verify_done, p.verify_total));
    }
    text
}

fn glyph(state: State) -> &'static str {
    match state {
        State::Todo => CIRCLE,
        State::Doing => HALF,
        State::Done => CIRCLE_CHECK,
    }
}

/// `status.css`'s pill classes, which a step shares with a task.
fn pill_class(state: State) -> &'static str {
    match state {
        State::Todo => "status queue",
        State::Doing => "status wip",
        State::Done => "status done",
    }
}

fn verb(state: State) -> &'static str {
    match state {
        State::Todo => "Back to todo",
        State::Doing => "Start",
        State::Done => "Mark done",
    }
}

#[component]
async fn step_pill(state: State) -> Result<impl View> {
    Ok(view! {
        <span class=(pill_class(state))>(icon(glyph(state))) (state.label())</span>
    })
}

#[component]
async fn step_row(task_id: i64, item: &Item, current: bool) -> Result<impl View> {
    let class = format!(
        "{}{}",
        item.state.label(),
        if current { " current" } else { "" }
    );
    Ok(view! {
        <li class=(class)>
            if item.id.is_empty() {
                // a step written by hand and not numbered yet: it cannot be opened
                <span class="loose">
                    (icon(glyph(item.state)))
                    <span class="sr">(item.state.label()) ": "</span>
                    <span class="step-text">(item.text.as_str())</span>
                </span>
            } else {
                <a href=(task_step_url(task_id, &item.id)) if current { aria-current="step" }>
                    (icon(glyph(item.state)))
                    <span class="sr">(item.state.label()) ": "</span>
                    <span class="step-text">(item.text.as_str())</span>
                    if item.verify {
                        <span class="step-tag">"check"</span>
                    }
                </a>
            }
        </li>
    })
}

#[component]
async fn checklist(task_id: i64, plan: &Plan, current: Option<&str>) -> Result<impl View> {
    Ok(view! {
        <ol class="steps">
            for item in plan.items.iter() {
                step_row(task_id: task_id, item: item, current: current == Some(item.id.as_str()) && !item.id.is_empty())
            }
        </ol>
    })
}

#[component]
async fn plan_block(task_id: i64, agent: &AgentTab) -> Result<impl View> {
    let progress = agent.plan.progress();
    let said = summary(&progress);
    let plan = &agent.plan;
    Ok(view! {
        labelled_section(id: "plan-title", label: "Plan", count: Some(progress.total), noun: "steps",
            if agent.dir.is_none() {
                empty_line("This project has no notes folder to keep a plan in. Set `notes_path` on the project or its organization.")
            } else if agent.plan_text.is_none() {
                empty_line("No plan yet.")
            } else {
                if progress.total > 0 {
                    <div class="plan-progress">
                        <progress value=(progress.done.to_string()) max=(progress.total.to_string()) aria-label="Steps done">(said.as_str())</progress>
                        <span>(said.as_str())</span>
                    </div>
                }
                if !plan.goal.is_empty() {
                    <div class="plan-part">
                        <h3 class="label">"Goal"</h3>
                        description(text: &plan.goal)
                    </div>
                }
                if !plan.how.is_empty() {
                    <div class="plan-part">
                        <h3 class="label">"How"</h3>
                        description(text: &plan.how)
                    </div>
                }
                if !plan.items.is_empty() {
                    <div class="plan-part">
                        <h3 class="label">"Checklist"</h3>
                        checklist(task_id: task_id, plan: plan, current: agent.step.as_deref())
                    </div>
                }
            }
        )
    })
}

#[component]
async fn files_block(id: &str, label: &str, noun: &str, docs: &[Doc]) -> Result<impl View> {
    Ok(view! {
        labelled_section(id: id, label: label, count: Some(docs.len()), noun: noun,
            if docs.is_empty() {
                empty_line("Nothing here yet.")
            } else {
                <div class="files">
                    for doc in docs.iter() {
                        <details>
                            <summary>(doc.name.as_str())</summary>
                            description(text: &doc.text)
                            if doc.cut {
                                <p class="empty-line">"The rest of the file is not shown."</p>
                            }
                        </details>
                    }
                </div>
            }
        )
    })
}

#[component]
async fn runs_block(runs: &[RunView]) -> Result<impl View> {
    Ok(view! {
        labelled_section(id: "runs-title", label: "Runs", count: Some(runs.len()), noun: "runs",
            if runs.is_empty() {
                empty_line("No agent has worked on this task yet.")
            } else {
                <div class="files runs">
                    for r in runs.iter() {
                        <details>
                            <summary>
                                "#" (r.run.n.to_string()) " "
                                <span class=(format!("run-state {}", r.run.shown_status()))>(r.run.shown_status())</span>
                                " " (r.run.kind.as_str()) ", " (r.run.agent.key()) " " (r.run.model.as_str())
                                " - " (r.run.started.as_str())
                            </summary>
                            if let Some(branch) = &r.run.branch {
                                <p class="empty-line">"Branch " <code>(branch.as_str())</code></p>
                            }
                            if r.log.is_empty() {
                                <p class="empty-line">"The log is empty."</p>
                            } else {
                                if r.cut {
                                    <p class="empty-line">"The start of the log is not shown."</p>
                                }
                                <pre class="log" tabindex="0">(r.log.as_str())</pre>
                            }
                        </details>
                    }
                </div>
            }
        )
    })
}

/// The form that hands the task to an agent.
#[component]
async fn launch_form(task_id: i64) -> Result<impl View> {
    let kinds: Vec<(String, String)> = [
        (Kind::Execute, "Work through the plan"),
        (Kind::Fix, "Fix a defect"),
        (Kind::Investigate, "Investigate (no code changes)"),
        (Kind::Plan, "Write the plan (no code changes)"),
    ]
    .into_iter()
    .map(|(k, label)| (k.key().to_string(), label.to_string()))
    .collect();
    let agents = vec![(Harness::Claude.key().to_string(), "Claude".to_string())];
    let efforts: Vec<(String, String)> = ["low", "medium", "high"]
        .into_iter()
        .map(|e| (e.to_string(), e.to_string()))
        .collect();
    Ok(view! {
        <section class="panel launch" aria-labelledby="launch-title">
            <h2 id="launch-title" class="label">"Hand to an agent"</h2>
            <form method="post" action=(solve_url(task_id))>
                select(name: "kind", label: "What to do", options: &kinds, current: "execute")
                select(name: "agent", label: "Agent", options: &agents, current: "claude")
                field(name: "model", label: "Model", value: "haiku", identifier: true)
                select(name: "effort", label: "Effort", options: &efforts, current: "medium")
                <button class="button" type="submit">"Start"</button>
            </form>
        </section>
    })
}

/// The open step: what it says, its state, and the buttons that change it.
#[component]
async fn step_panel(task_id: i64, item: &Item) -> Result<impl View> {
    let others: Vec<State> = [State::Todo, State::Doing, State::Done]
        .into_iter()
        .filter(|s| *s != item.state)
        .collect();
    Ok(view! {
        <aside class="panel step-panel" aria-labelledby="step-title">
            <h2 id="step-title" class="label">"Step " (item.id.as_str())</h2>
            <p>
                step_pill(state: item.state)
                if item.verify {
                    <span class="step-tag">"check"</span>
                }
            </p>
            description(text: &item.text)
            if !item.detail.is_empty() {
                description(text: &item.detail)
            }
            <div class="step-actions">
                for s in others.iter() {
                    post_button(action: plan_step_url(task_id, &item.id), fields: vec![("state", s.label().to_string())], small: true, (verb(*s)))
                }
            </div>
            <a class="step-close" href=(format!("/task/{task_id}?tab=agent"))>"Close"</a>
        </aside>
    })
}

/// The aside when no step is open (or the one asked for is not in the plan):
/// how to send an agent at the task, until the page can do it itself.
#[component]
async fn agent_panel(agent: &AgentTab, missing: Option<&str>) -> Result<impl View> {
    let word = shell_word(&agent.task);
    let command = |kind: Kind| format!("iter task prompt {word} -k {}", kind.key());
    let mut rows = vec![
        Row::Text("Plan", command(Kind::Plan)),
        Row::Text("Execute", command(Kind::Execute)),
        Row::Text("Fix", command(Kind::Fix)),
        Row::Text("Investigate", command(Kind::Investigate)),
    ];
    if let Some(dir) = &agent.dir {
        rows.push(Row::Text("Task folder", dir.root().display().to_string()));
    }
    Ok(view! {
        if let Some(step) = missing {
            <p class="empty-line">"There is no step " <code>(step)</code> " in the plan."</p>
        }
        launch_form(task_id: agent.task_id)
        panel(id: "agent-title", title: "Prompts for an agent", rows: &rows)
    })
}

/// The tab's body.
#[component]
pub async fn agent_tab(task_id: i64, agent: &AgentTab) -> Result<impl View> {
    let open = agent.step.as_deref().and_then(|id| agent.plan.item(id));
    let missing = agent.step.as_deref().filter(|_| open.is_none());
    Ok(view! {
        info_columns(
            <div>
                plan_block(task_id: task_id, agent: agent)
                runs_block(runs: &agent.runs)
                files_block(id: "notes-title", label: "Notes", noun: "notes", docs: &agent.notes)
                files_block(id: "sketches-title", label: "Sketches", noun: "sketches", docs: &agent.sketches)
            </div>
            if let Some(item) = open {
                step_panel(task_id: task_id, item: item)
            } else {
                agent_panel(agent: agent, missing: missing)
            }
        )
    })
}

#[derive(Deserialize)]
struct MarkForm {
    state: String,
}

/// Why a step could not be moved.
#[derive(Debug)]
pub enum Refusal {
    /// Not `todo`, `doing` or `done`.
    State(String),
    /// No plan, or no such step in it.
    Missing,
    Io(std::io::Error),
}

/// Puts `step` of the plan in `dir` into the state `state` names.
pub fn set_state(dir: &TaskDir, step: &str, state: &str) -> std::result::Result<(), Refusal> {
    let state = match state {
        "todo" => State::Todo,
        "doing" => State::Doing,
        "done" => State::Done,
        other => return Err(Refusal::State(format!("'{other}' is not a state"))),
    };
    let content = dir
        .read_plan()
        .map_err(Refusal::Io)?
        .ok_or(Refusal::Missing)?;
    let marked = plan::mark(&content, step, state).map_err(|_| Refusal::Missing)?;
    dir.write_plan(&marked).map_err(Refusal::Io)
}

/// `POST /task/{id}/plan/{step}`: moves a step, then shows the plan with that
/// step open.
#[route(POST "/task/{id}/plan/{step}")]
async fn mark_step(cx: &Cx, Form(form): Form<MarkForm>) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let step = path_param::<Step>(cx)?.clone();
    let db = open_db()?;
    let task = db.get::<Task>(id)?.ok_or_not_found()?;
    let project = db.get::<Project>(task.project_id)?.ok_or_not_found()?;
    let dir = taskdir::of(&db, &project, &task).ok().ok_or_not_found()?;
    match set_state(&dir, &step, &form.state) {
        Ok(()) => {}
        Err(Refusal::State(message)) => return Err(bad_request(message).into()),
        Err(Refusal::Missing) => None::<()>.ok_or_not_found()?,
        Err(Refusal::Io(e)) => return Err(e.into()),
    }
    Err(see_other(task_step_url(id, &step)).into())
}

#[derive(Deserialize)]
struct SolveForm {
    kind: String,
    agent: String,
    model: String,
    effort: String,
}

/// The run a form asks for, or why not.
fn parse_solve(form: &SolveForm) -> std::result::Result<(Kind, Spec), String> {
    use clap::ValueEnum;
    let kind =
        Kind::from_str(&form.kind, true).map_err(|_| format!("'{}' is not a kind", form.kind))?;
    let agent = Harness::from_str(&form.agent, true)
        .map_err(|_| format!("'{}' is not an agent", form.agent))?;
    let model = form.model.trim();
    // the model is an argument of the harness: not a flag, not a novel
    if model.starts_with('-')
        || model.len() > 64
        || model.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(format!("'{model}' is not a model name"));
    }
    if !["low", "medium", "high"].contains(&form.effort.as_str()) {
        return Err(format!("'{}' is not an effort", form.effort));
    }
    Ok((
        kind,
        Spec {
            agent,
            model: model.to_string(),
            effort: form.effort.clone(),
        },
    ))
}

/// `POST /task/{id}/solve`: starts an agent on the task, then shows the tab,
/// where the run is listed.
#[route(POST "/task/{id}/solve")]
async fn start_run(cx: &Cx, Form(form): Form<SolveForm>) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let (kind, spec) = parse_solve(&form).map_err(bad_request)?;
    let db = open_db()?;
    let task = db.get::<Task>(id)?.ok_or_not_found()?;
    let project = db.get::<Project>(task.project_id)?.ok_or_not_found()?;
    let dir = taskdir::of(&db, &project, &task).ok().ok_or_not_found()?;
    if dir.runs().iter().any(|r| r.shown_status() == "running") {
        return Err(bad_request("an agent is already working on this task").into());
    }
    let app = crate::app::App {
        db,
        now: chrono::Local::now().naive_local(),
    };
    solve::start(
        &app,
        &task_display(&project, &task),
        kind,
        &spec,
        &task.description,
        false,
        &[],
    )?;
    Err(see_other(agent_tab_url(&task_url(id))).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::models::TaskStatus;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("iter-webagent-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn task(project_id: i64) -> Task {
        Task {
            id: None,
            project_id,
            name: "Fix it".into(),
            description: String::new(),
            github_issue: None,
            status: TaskStatus::Queue,
            branch_prefix: String::new(),
            matrix_placed: false,
            start_time: None,
            duration: None,
        }
    }

    const PLAN: &str =
        "## Goal\nStop it.\n\n## Checklist\n- [x] {s1} one\n- [ ] {s2} (verify) two\n";

    #[test]
    fn the_tab_loads_the_plan_the_notes_and_the_sketches() {
        let notes = scratch("load");
        let db = fixtures::db();
        let mut p = fixtures::project("app");
        p.notes_path = notes.display().to_string();
        let project_id = db.insert(&p).unwrap();
        let task_id = db.insert(&task(project_id)).unwrap();
        let (project, task) = (
            db.get::<Project>(project_id).unwrap().unwrap(),
            db.get::<Task>(task_id).unwrap().unwrap(),
        );

        let bare = load(&db, &project, &task, None).unwrap();
        assert!(bare.dir.is_some() && bare.plan_text.is_none() && bare.notes.is_empty());
        assert_eq!(bare.task, "app/Fix it");

        let dir = taskdir::of(&db, &project, &task).unwrap();
        dir.write_plan(PLAN).unwrap();
        std::fs::write(dir.notes_dir().join("idea.md"), "# An idea").unwrap();
        std::fs::write(dir.sketches_dir().join("a.md"), "x".repeat(FILE_BYTES + 10)).unwrap();
        let tab = load(&db, &project, &task, Some("s2")).unwrap();
        assert_eq!(tab.plan.items.len(), 2);
        assert_eq!(tab.step.as_deref(), Some("s2"));
        assert_eq!(tab.notes[0].name, "idea");
        assert!(!tab.notes[0].cut);
        // a long file is cut, on a character boundary
        assert!(tab.sketches[0].cut && tab.sketches[0].text.len() == FILE_BYTES);
    }

    #[test]
    fn a_project_with_nowhere_to_keep_notes_has_an_empty_tab() {
        let db = fixtures::db();
        let project_id = db.insert(&fixtures::project("bare")).unwrap();
        let task_id = db.insert(&task(project_id)).unwrap();
        let (project, task) = (
            db.get::<Project>(project_id).unwrap().unwrap(),
            db.get::<Task>(task_id).unwrap().unwrap(),
        );
        let tab = load(&db, &project, &task, None).unwrap();
        assert!(tab.dir.is_none() && tab.plan_text.is_none());
    }

    #[test]
    fn a_long_file_is_cut_on_a_character_boundary() {
        let dir = scratch("cut");
        std::fs::create_dir_all(&dir).unwrap();
        // two-byte characters, so the byte limit falls inside one
        std::fs::write(dir.join("long.md"), "é".repeat(FILE_BYTES)).unwrap();
        let d = docs(&dir);
        assert!(d[0].cut);
        assert!(d[0].text.len() <= FILE_BYTES && d[0].text.chars().all(|c| c == 'é'));
    }

    #[test]
    fn a_step_is_moved_by_its_state_name_and_nothing_else_changes() {
        let dir = TaskDir::new(scratch("mark").join("tasks/1"));
        assert!(matches!(
            set_state(&dir, "s1", "done"),
            Err(Refusal::Missing)
        ));
        dir.write_plan(PLAN).unwrap();
        set_state(&dir, "s2", "doing").unwrap();
        assert_eq!(
            dir.read_plan().unwrap().unwrap(),
            PLAN.replace("- [ ] {s2}", "- [~] {s2}")
        );
        set_state(&dir, "s1", "todo").unwrap();
        assert!(dir.read_plan().unwrap().unwrap().contains("- [ ] {s1} one"));
        assert!(matches!(
            set_state(&dir, "s9", "done"),
            Err(Refusal::Missing)
        ));
        assert!(matches!(
            set_state(&dir, "s1", "finished"),
            Err(Refusal::State(_))
        ));
        // a refusal leaves the plan as it was
        assert!(dir.read_plan().unwrap().unwrap().contains("- [~] {s2}"));
    }

    fn form(kind: &str, model: &str, effort: &str) -> SolveForm {
        SolveForm {
            kind: kind.into(),
            agent: "claude".into(),
            model: model.into(),
            effort: effort.into(),
        }
    }

    #[test]
    fn the_launch_form_is_checked_before_anything_starts() {
        let (kind, spec) = parse_solve(&form("fix", " sonnet ", "high")).unwrap();
        assert_eq!(
            (kind, spec.model.as_str(), spec.effort.as_str()),
            (Kind::Fix, "sonnet", "high")
        );
        assert!(parse_solve(&form("dance", "x", "low")).is_err());
        assert!(parse_solve(&form("fix", "--dangerous", "low")).is_err());
        assert!(parse_solve(&form("fix", "a b", "low")).is_err());
        assert!(parse_solve(&form("fix", "x", "extreme")).is_err());
        let mut other = form("fix", "x", "low");
        other.agent = "nobody".into();
        assert!(parse_solve(&other).is_err());
    }

    #[test]
    fn a_log_is_shown_from_its_end_on_a_character_boundary() {
        let dir = TaskDir::new(scratch("log").join("tasks/1"));
        assert!(run_views(&dir).is_empty());
        let mut run = Run {
            n: 1,
            agent: Harness::Claude,
            model: "haiku".into(),
            effort: "medium".into(),
            kind: "execute".into(),
            status: crate::agent::run::Status::Done,
            started: "2026-10-30 10:00:00".into(),
            ended: None,
            exit_code: Some(0),
            branch: None,
            worktree: None,
            pid: None,
        };
        dir.write_run(&run).unwrap();
        run.n = 2;
        dir.write_run(&run).unwrap();
        std::fs::write(
            dir.run_log_path(2),
            format!("start{}end", "é".repeat(LOG_BYTES)),
        )
        .unwrap();
        let views = run_views(&dir);
        // latest first; the first run has no log
        assert_eq!((views[0].run.n, views[1].run.n), (2, 1));
        assert!(views[0].cut && views[0].log.ends_with("end") && !views[0].log.contains("start"));
        assert!(!views[1].cut && views[1].log.is_empty());
    }

    #[test]
    fn the_summary_says_where_the_plan_stands() {
        let p = plan::parse(PLAN).progress();
        assert_eq!(summary(&p), "1 of 2 done. Checks 0 of 1");
        let doing = plan::parse("## Checklist\n- [~] {s1} a\n- [ ] {s2} b\n").progress();
        assert_eq!(summary(&doing), "0 of 2 done, 1 in progress");
    }
}
