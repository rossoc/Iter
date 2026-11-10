//! The Agent tab of the task page: the plan, a step opened beside it, and the notes and sketches in the task's folder.

use super::task_compose::Compose;
use super::task_runs::{act, label_of, state_pill};
use super::ui::button::post_button;
use super::ui::prose::description;
use super::ui::table::{AddRow, cell, data_table, row};
use super::ui::{CIRCLE, CIRCLE_CHECK, HALF, PLAY, STOP, X, icon};
use super::url::{
    RunAction, agent_tab_url, agent_url, plan_step_url, run_action_url, task_step_url, task_url,
};
use super::{Id, open_db};
use crate::agent::instruction::{self, Layers};
use crate::agent::plan::{self, Item, Plan, State};
use crate::agent::run::{Harness, RunRecord};
use crate::agent::taskdir::{self, TaskDir};
use crate::app::App;
use crate::config::config;
use crate::db::Db;
use crate::error::{IterError, Result as Loaded};
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
    builder.route(mark_step)
}

const FILE_BYTES: usize = 64 * 1024;
const FILES: usize = 50;

pub struct Doc {
    pub name: String,
    pub text: String,
    pub cut: bool,
}

pub struct AgentTab {
    /// The folder is among the project's notes, else in iter's config directory (the project has no notes folder).
    pub in_notes: bool,
    pub plan: Plan,
    pub step: Option<String>,
    pub roster: Roster,
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

pub struct Files {
    pub notes: Vec<Doc>,
    pub sketches: Vec<Doc>,
}

pub fn files(db: &Db, project: &Project, task: &Task) -> Files {
    match taskdir::of(db, project, task) {
        Ok(dir) => Files {
            notes: docs(&dir.notes_dir()),
            sketches: docs(&dir.sketches_dir()),
        },
        Err(_) => Files {
            notes: Vec::new(),
            sketches: Vec::new(),
        },
    }
}

#[derive(Default)]
pub struct Open<'a> {
    pub step: Option<&'a str>,
    pub run: Option<&'a str>,
    pub new: Option<&'a str>,
    pub instruction: Option<&'a str>,
}

pub fn load(app: &App, project: &Project, task: &Task, open: &Open) -> Loaded<AgentTab> {
    let db = &app.db;
    let dir = taskdir::of(db, project, task).ok();
    let plan_text = match &dir {
        Some(dir) => dir.read_plan()?,
        None => None,
    };
    let name = task_display(project, task);
    let mut layers = instruction::layers_of(db, project);
    if let Some(dir) = &dir {
        layers.insert(0, (instruction::Source::Task, dir.root().to_path_buf()));
    }
    let parsed = plan::parse(plan_text.as_deref().unwrap_or(""));
    let roster = Roster::load(
        app,
        &name,
        dir.as_ref(),
        open,
        &layers,
        !parsed.items.is_empty(),
    );
    Ok(AgentTab {
        in_notes: taskdir::in_notes(db, project),
        roster,
        plan: parsed,
        step: open.step.map(String::from),
    })
}

fn glyph(state: State) -> &'static str {
    match state {
        State::Todo => CIRCLE,
        State::Doing => HALF,
        State::Done => CIRCLE_CHECK,
    }
}

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

pub fn plan_of(db: &Db, project: &Project, task: &Task) -> Plan {
    taskdir::of(db, project, task)
        .ok()
        .and_then(|dir| dir.read_plan().ok().flatten())
        .map(|text| plan::parse(&text))
        .unwrap_or_default()
}

#[component]
async fn checkpoint(task_id: i64, item: &Item) -> Result<impl View> {
    Ok(view! {
        <li class=(format!("{} point", item.state.label()))>
            (icon(glyph(item.state)))
            <span class="sr">(item.state.label()) ": "</span>
            if item.id.is_empty() {
                <span class="step-text">(item.text.as_str())</span>
            } else {
                <a class="step-text" href=(task_step_url(task_id, &item.id))>(item.text.as_str())</a>
            }
            if item.verify {
                <span class="step-tag">"check"</span>
            }
        </li>
    })
}

#[component]
pub async fn plan_block(task_id: i64, plan: Option<&Plan>) -> Result<impl View> {
    let plan = plan.cloned().unwrap_or_default();
    let any = !plan.items.is_empty() || !plan.goal.is_empty() || !plan.how.is_empty();
    Ok(view! {
        if any {
            <section class="plan-block" aria-labelledby="plan-title">
                <h2 id="plan-title" class="plan-title">"Plan"</h2>
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
                        <h3 class="label">"Checkpoints"</h3>
                        <ul class="points">
                            for item in plan.items.iter().filter(|i| i.phase.is_none()) {
                                checkpoint(task_id: task_id, item: item)
                            }
                        </ul>
                        for (n, name) in plan.phases.iter().enumerate() {
                            let (done, total) = plan.phase_progress(n);
                            <h4 class="point-head"><span class="name">(name.as_str())</span> <span class="state">(done.to_string()) "/" (total.to_string())</span></h4>
                            <ul class="points">
                                for item in plan.items.iter().filter(|i| i.phase == Some(n)) {
                                    checkpoint(task_id: task_id, item: item)
                                }
                            </ul>
                        }
                    </div>
                }
            </section>
        }
    })
}

#[component]
pub(super) async fn files_block(
    label: &str,
    noun: &str,
    docs: &[Doc],
    #[default] add: Option<AddRow<'_>>,
) -> Result<impl View> {
    let empty = format!("No {noun} yet.");
    let columns = [label];
    Ok(view! {
        data_table(caption: label, columns: &columns, class: "files", headed: false, add: add,
            empty: docs.is_empty().then_some(empty.as_str()),
            for doc in docs.iter() {
                row(
                    cell(
                        <details>
                            <summary>(doc.name.as_str())</summary>
                            description(text: &doc.text)
                            if doc.cut {
                                <p class="empty-line">"The rest of the file is not shown."</p>
                            }
                        </details>
                    )
                )
            }
        )
    })
}

#[component]
pub(super) async fn step_panel(task_id: i64, item: &Item) -> Result<impl View> {
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
            <a class="step-close" href=(agent_tab_url(&task_url(task_id)))>"Close"</a>
        </aside>
    })
}

/// A step being worked on is half way.
fn share(state: State) -> &'static str {
    match state {
        State::Todo => "width:0",
        State::Doing => "width:50%",
        State::Done => "width:100%",
    }
}

#[component]
async fn rail_item(
    task_id: i64,
    item: &Item,
    current: Option<&str>,
    bar: bool,
) -> Result<impl View> {
    Ok(view! {
        <li class=(item.state.label())>
            if item.id.is_empty() {
                // written by hand and not numbered yet: it cannot be opened
                <span class="loose">
                    <span class="name">(item.text.as_str())</span>
                    <span class="state">(item.state.label())</span>
                    if bar {
                        <span class="share" aria-hidden="true"><i style=(share(item.state))></i></span>
                    }
                </span>
            } else {
                <a href=(task_step_url(task_id, &item.id)) if current == Some(item.id.as_str()) { aria-current="step" }>
                    <span class="name">(item.text.as_str())</span>
                    <span class="state">(item.state.label())</span>
                    if bar {
                        <span class="share" aria-hidden="true"><i style=(share(item.state))></i></span>
                    }
                </a>
            }
        </li>
    })
}

#[component]
async fn plan_rail(task_id: i64, plan: &Plan, current: Option<&str>) -> Result<impl View> {
    let phased = !plan.phases.is_empty();
    Ok(view! {
        <nav class="plan-rail" aria-labelledby="rail-title">
            <h2 id="rail-title" class="label">"Plan"</h2>
            <ol>
                if phased {
                    for (n, name) in plan.phases.iter().enumerate() {
                        let (done, total) = plan.phase_progress(n);
                        <li class="phase">
                            <div class="phase-head">
                                <span class="name">(name.as_str())</span>
                                <span class="state">(done.to_string()) "/" (total.to_string())</span>
                                <span class="share" aria-hidden="true"><i style=(format!("width:{}%", (done * 100).checked_div(total).unwrap_or(0)))></i></span>
                            </div>
                        </li>
                    }
                } else {
                    for item in plan.items.iter() {
                        rail_item(task_id: task_id, item: item, current: current, bar: true)
                    }
                }
            </ol>
        </nav>
    })
}

#[component]
pub async fn agent_tab(task_id: i64, agent: &AgentTab) -> Result<impl View> {
    let open = agent.step.as_deref().and_then(|id| agent.plan.item(id));
    let missing = agent.step.as_deref().filter(|_| open.is_none());
    let rail = !agent.plan.items.is_empty();
    Ok(view! {
        <div class=(if rail { "agent-grid" } else { "agent-grid bare" })>
            if rail {
                plan_rail(task_id: task_id, plan: &agent.plan, current: agent.step.as_deref())
            }
            <div class="agent-main">
                if let Some(step) = missing {
                    <p class="empty-line">"There is no step " <code>(step)</code> " in the plan."</p>
                }
                roster_block(task_id: task_id, roster: &agent.roster, step: open,
                    note: if agent.in_notes { None } else { Some("The project has no notes folder (`notes_path`), so the task is kept in iter's config folder and the wiki does not see it.") })
            </div>
        </div>
    })
}

#[derive(Deserialize)]
struct MarkForm {
    state: String,
}

#[derive(Debug)]
pub(super) enum Refusal {
    State(String),
    Missing,
    Io(std::io::Error),
}

pub fn set_state(dir: &TaskDir, step: &str, state: &str) -> std::result::Result<(), Refusal> {
    let state = <State as clap::ValueEnum>::from_str(state, false)
        .map_err(|_| Refusal::State(format!("'{state}' is not a state")))?;
    // taking turns with whoever else is writing the plan (an agent ticking a step)
    dir.update_plan(|content| {
        let missing = || IterError::NotFound {
            kind: "plan step",
            name: step.to_string(),
        };
        let content = content.ok_or_else(missing)?;
        let marked = plan::mark(&content, step, state).map_err(|_| missing())?;
        Ok((Some(marked), ()))
    })
    .map_err(|e| match e {
        IterError::NotFound { .. } => Refusal::Missing,
        IterError::Io(io) => Refusal::Io(io),
        other => Refusal::Io(std::io::Error::other(other.to_string())),
    })
}

#[route(POST "/task/{id}/plan/{step}")]
async fn mark_step(cx: &Cx, Form(form): Form<MarkForm>) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let step = path_param::<Step>(cx)?.clone();
    let db = open_db()?;
    let (task, project) = super::load::task_with_project(&db, id)?;
    let dir = taskdir::of(&db, &project, &task).ok().ok_or_not_found()?;
    match set_state(&dir, &step, &form.state) {
        Ok(()) => {}
        Err(Refusal::State(message)) => return Err(bad_request(message).into()),
        Err(Refusal::Missing) => None::<()>.ok_or_not_found()?,
        Err(Refusal::Io(e)) => return Err(e.into()),
    }
    Err(see_other(task_step_url(id, &step)).into())
}

const RUNS: usize = 50;

#[derive(Default)]
pub struct Roster {
    pub runs: Vec<RunRecord>,
    pub compose: Option<Compose>,
    pub current: Option<u32>,
}

fn defaults() -> (String, String) {
    let models = Harness::Claude.models(&config().agent.models);
    (
        models.first().cloned().unwrap_or_default(),
        "medium".to_string(),
    )
}

fn offered(layers: &Layers, has_plan: bool) -> Vec<(String, String)> {
    instruction::list(layers)
        .into_iter()
        .filter(|i| has_plan || i.name == "plan")
        .map(|i| {
            let label = label_of(&i.name);
            (i.name, label)
        })
        .collect()
}

impl Roster {
    pub fn load(
        app: &App,
        name: &str,
        dir: Option<&TaskDir>,
        open: &Open,
        layers: &Layers,
        has_plan: bool,
    ) -> Roster {
        let mut roster = Roster::default();
        let Some(dir) = dir else {
            return roster;
        };
        roster.runs = dir.runs().into_iter().rev().take(RUNS).collect();
        let asked = open.run.and_then(|r| r.parse::<u32>().ok());
        let found = asked.and_then(|n| roster.runs.iter().find(|r| r.n == n).cloned());
        let (mut model, effort) = defaults();
        if open.new == Some("agent") {
            let chosen = open
                .instruction
                .unwrap_or(if has_plan { "develop" } else { "plan" });
            let inst = instruction::find(chosen, layers)
                .or_else(|| instruction::find("plan", layers))
                .expect("the built-in instruction is always there");
            let models = Harness::Claude.models(&config().agent.models);
            if let Some(m) = instruction::preferred_model(&inst.front, &models) {
                model = m;
            }
            let kind = inst.kind();
            let (prompt, error) = match crate::agent::task_prompt::prompt_for(
                app,
                Some(name),
                kind,
                inst.path.as_deref(),
                true,
                None,
                true,
            ) {
                Ok(text) => (text, None),
                Err(e) => (String::new(), Some(e.to_string())),
            };
            roster.compose = Some(Compose {
                instruction: inst.name.clone(),
                offered: offered(layers, has_plan),
                run: None,
                model,
                effort,
                prompt,
                error,
            });
        } else if let Some(run) = found {
            roster.current = Some(run.n);
            if run.is_queued() {
                roster.compose = Some(Compose {
                    instruction: run.instruction_name().to_string(),
                    offered: offered(layers, has_plan),
                    run: Some(run.n),
                    model: run.model.clone(),
                    effort: run.effort.clone(),
                    prompt: std::fs::read_to_string(dir.run_prompt_path(run.n)).unwrap_or_default(),
                    error: None,
                });
            }
        }
        roster
    }
}

#[component]
async fn agent_row(task_id: i64, run: &RunRecord, current: bool) -> Result<impl View> {
    let shown = run.shown_status();
    Ok(view! {
        row(class: if current { "current" } else { "" },
            cell(
                <a class="u" href=(agent_url(task_id, run.n)) if current { aria-current="true" }>
                    "#" (run.n.to_string()) " " (label_of(run.instruction_name()))
                </a>
                // what it was told to use: only on the row under the pointer
                <span class="hint">(run.model.as_str()) ", " (run.effort.as_str())</span>
            )
            cell(state_pill(run: run))
            cell(class: "acts",
                if shown == "queued" {
                    act(action: run_action_url(task_id, run.n, RunAction::Run), label: "Run now", svg: PLAY)
                    act(action: run_action_url(task_id, run.n, RunAction::Cancel), label: "Remove from the queue", svg: X)
                } else if shown == "running" {
                    act(action: run_action_url(task_id, run.n, RunAction::Cancel), label: "Stop", svg: STOP)
                }
            )
        )
    })
}

#[component]
async fn agents_table(task_id: i64, roster: &Roster) -> Result<impl View> {
    let new = format!("{}&new=agent", agent_tab_url(&task_url(task_id)));
    Ok(view! {
        data_table(caption: "Agents of this task, the latest first", columns: &["Agent", "Status"], actions: true,
            class: "tasks agents", add: Some(AddRow::link(&new, "New agent\u{2026}")),
            empty: roster.runs.is_empty().then_some("No agent has worked on this task yet."),
            for run in roster.runs.iter() {
                agent_row(task_id: task_id, run: run, current: roster.current == Some(run.n))
            }
        )
    })
}

#[component]
pub async fn roster_block(
    task_id: i64,
    roster: &Roster,
    step: Option<&Item>,
    note: Option<&str>,
) -> Result<impl View> {
    Ok(view! {
        <div class="roster">
            if let Some(item) = step {
                step_panel(task_id: task_id, item: item)
            }
            agents_table(task_id: task_id, roster: roster)
            if let Some(note) = note {
                <p class="empty-line note">(note)</p>
            }
        </div>
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    const PLAN: &str =
        "## Goal\nStop it.\n\n## Checklist\n- [x] {s1} one\n- [ ] {s2} (verify) two\n";

    #[test]
    fn the_tab_loads_the_plan_and_the_folder_its_notes_and_sketches() {
        let fixtures::World {
            db,
            project_id,
            task_id,
            ..
        } = fixtures::world("webagent-load");
        let (project, task) = (
            db.get::<Project>(project_id).unwrap().unwrap(),
            db.get::<Task>(task_id).unwrap().unwrap(),
        );

        let app = App {
            db,
            now: chrono::Local::now().naive_local(),
        };
        let db = &app.db;
        let bare = load(&app, &project, &task, &Open::default()).unwrap();
        assert!(bare.plan.items.is_empty());
        assert!(files(db, &project, &task).notes.is_empty());

        let dir = taskdir::of(db, &project, &task).unwrap();
        dir.write_plan(PLAN).unwrap();
        std::fs::write(dir.notes_dir().join("idea.md"), "# An idea").unwrap();
        std::fs::write(dir.sketches_dir().join("a.md"), "x".repeat(FILE_BYTES + 10)).unwrap();
        let open = Open {
            step: Some("s2"),
            ..Open::default()
        };
        let tab = load(&app, &project, &task, &open).unwrap();
        assert_eq!(tab.plan.items.len(), 2);
        assert_eq!(tab.step.as_deref(), Some("s2"));
        let kept = files(db, &project, &task);
        assert_eq!(kept.notes[0].name, "idea");
        assert!(!kept.notes[0].cut);
        // a long file is cut, on a character boundary
        assert!(kept.sketches[0].cut && kept.sketches[0].text.len() == FILE_BYTES);
    }

    #[test]
    fn a_project_with_nowhere_to_keep_notes_keeps_the_task_in_the_config_folder() {
        let db = fixtures::db();
        let project_id = db.insert(&fixtures::project("bare")).unwrap();
        let task_id = fixtures::add_task(&db, project_id, "Fix it");
        let (project, task) = (
            db.get::<Project>(project_id).unwrap().unwrap(),
            db.get::<Task>(task_id).unwrap().unwrap(),
        );
        let app = App {
            db,
            now: chrono::Local::now().naive_local(),
        };
        let tab = load(&app, &project, &task, &Open::default()).unwrap();
        assert!(!tab.in_notes && tab.plan.items.is_empty());
    }

    #[test]
    fn a_long_file_is_cut_on_a_character_boundary() {
        let dir = fixtures::scratch("webagent-cut");
        std::fs::create_dir_all(&dir).unwrap();
        // two-byte characters, so the byte limit falls inside one
        std::fs::write(dir.join("long.md"), "é".repeat(FILE_BYTES)).unwrap();
        let d = docs(&dir);
        assert!(d[0].cut);
        assert!(d[0].text.len() <= FILE_BYTES && d[0].text.chars().all(|c| c == 'é'));
    }

    #[test]
    fn a_step_is_moved_by_its_state_name_and_nothing_else_changes() {
        let dir = TaskDir::new(fixtures::scratch("webagent-mark").join("tasks/1"));
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
}
