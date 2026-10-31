//! The agents of a task, at the top of its Agent tab: a table with one entry
//! per run, the form that writes a new one, and the page of one that has
//! begun (what it was told, what it printed, a button to stop it).
//!
//! A run is a queued, running or finished record in the task's folder
//! (`crate::agent::run`); the routes here write them through
//! `commands::solve`, the one place that knows how a run begins.
//!
//! Everything is a link or a form: the kind of agent is chosen with links
//! (`?new=agent&kind=fix`) that load the prompt of that kind, so no script
//! is needed to fill the prompt in.

use super::errors::refused;
use super::sections::TabQuery;
use super::task_agent::{Open, step_panel};
use super::ui::button::add_link;
use super::ui::empty_line::empty_line;
use super::ui::field::{field, select, textarea};
use super::ui::modal::modal;
use super::ui::table::{cell, col_head};
use super::ui::{PLAY, STOP, X, icon};
use super::url::{agent_tab_url, agent_url, run_action_url, run_post_url, runs_url, task_url};
use super::{Id, open_db};
use crate::agent::instruction::{self, Layers};
use crate::agent::plan::Item;
use crate::agent::prompt::Kind;
use crate::agent::run::{Harness, RunRecord};
use crate::agent::taskdir::{self, TaskDir};
use crate::app::App;
use crate::commands::solve::{self, Spec};
use crate::config::config;
use crate::models::{Project, Task};
use crate::utils::resolve::task_display;
use serde::Deserialize;
use topcoat::router::{HeaderValue, header::CONTENT_TYPE, response::IntoResponse};
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        content::Form,
        error::{RouterErrorExt, bad_request, see_other},
        path_param, query_params,
        response::Response,
        route,
    },
    view::{View, component, view},
};

path_param!(n: u32, error = bad_request);

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder
        .route(create_run)
        .route(edit_run)
        .route(run_now)
        .route(cancel_run)
        .route(prompt_of)
}

/// How many agents the table lists.
const RUNS: usize = 50;

/// The form for a run that has not begun: a new one, or a queued one being
/// edited.
pub struct Compose {
    /// The instruction the prompt comes from.
    pub instruction: String,
    /// Every instruction that applies, with what it is for: the form's links.
    pub offered: Vec<(String, String)>,
    /// The queued run being edited; `None` for a new one.
    pub run: Option<u32>,
    pub model: String,
    pub effort: String,
    pub prompt: String,
    /// Why the prompt could not be made.
    pub error: Option<String>,
}

/// What the top of the tab shows.
#[derive(Default)]
pub struct Roster {
    /// Latest first.
    pub runs: Vec<RunRecord>,
    pub compose: Option<Compose>,
    /// The run the table marks (the one being edited).
    pub current: Option<u32>,
}

/// The default model and effort of a form.
fn defaults() -> (String, String) {
    let models = Harness::Claude.models(&config().agent.models);
    (
        models.first().cloned().unwrap_or_default(),
        "medium".to_string(),
    )
}

/// The instructions a form offers, as `(name, label)`.
/// At the start only `plan`: the plan says what the next big steps are. Once
/// there is a plan, every instruction that applies (the built-in ones, and
/// those the task, its project or its organization keep).
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
            let (prompt, error) = match crate::commands::agent::prompt_for(
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

/// What an instruction is for, as the page says it: the built-in ones in
/// words, the others by name.
pub(super) fn label_of(name: &str) -> String {
    match name {
        "plan" => "Plan",
        "develop" | "execute" => "Develop",
        "test" => "Test",
        "check" | "investigate" => "Check",
        "fix" => "Fix",
        other => other,
    }
    .to_string()
}

fn options(values: &[String]) -> Vec<(String, String)> {
    values.iter().map(|v| (v.clone(), v.clone())).collect()
}

/// A small icon-only button that posts to `action`: it is named for
/// assistive technology and as a tooltip.
#[component]
pub(super) async fn act(
    #[into] action: String,
    label: &str,
    svg: &'static str,
) -> Result<impl View> {
    Ok(view! {
        <form class="post-form" method="post" action=(action.as_str())>
            <button class="button small icon-only" type="submit" title=(label) aria-label=(label)>(icon(svg))</button>
        </form>
    })
}

#[component]
pub(super) async fn state_pill(run: &RunRecord) -> Result<impl View> {
    let shown = run.shown_status();
    Ok(view! {
        <span class=(format!("run-state {shown}"))>
            if shown == "running" {
                <span class="live" aria-hidden="true"></span>
            }
            (shown)
        </span>
    })
}

#[component]
async fn agent_row(task_id: i64, run: &RunRecord, current: bool) -> Result<impl View> {
    let shown = run.shown_status();
    Ok(view! {
        <tr role="row" if current { class="current" }>
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
                    act(action: run_action_url(task_id, run.n, "run"), label: "Run now", svg: PLAY)
                    act(action: run_action_url(task_id, run.n, "cancel"), label: "Remove from the queue", svg: X)
                } else if shown == "running" {
                    act(action: run_action_url(task_id, run.n, "cancel"), label: "Stop", svg: STOP)
                }
            )
        </tr>
    })
}

/// The agents, in the look of the tasks table: the first row writes a new
/// one, the rest are the runs, the latest first.
#[component]
async fn agents_table(task_id: i64, roster: &Roster) -> Result<impl View> {
    let new = format!("{}&new=agent", agent_tab_url(&task_url(task_id)));
    Ok(view! {
        <table class="tasks agents" role="table">
            <caption class="sr">"Agents of this task, the latest first"</caption>
            <thead role="rowgroup">
                <tr role="row">
                    col_head("Agent")
                    col_head("Status")
                    col_head(<span class="sr">"Actions"</span>)
                </tr>
            </thead>
            <tbody role="rowgroup">
                <tr class="add" role="row">
                    cell(colspan: "3", add_link(href: &new, label: "New agent\u{2026}"))
                </tr>
                for run in roster.runs.iter() {
                    agent_row(task_id: task_id, run: run, current: roster.current == Some(run.n))
                }
            </tbody>
        </table>
    })
}

/// The form of a new agent, or of a queued one.
#[component]
pub async fn compose_modal(task_id: i64, c: &Compose) -> Result<impl View> {
    let harness = Harness::Claude;
    let agents = vec![(harness.key().to_string(), "Claude".to_string())];
    let mut models = harness.models(&config().agent.models);
    // a queued run keeps its model even if the list changed since
    if !models.contains(&c.model) {
        models.insert(0, c.model.clone());
    }
    let efforts: Vec<String> = harness.efforts().iter().map(|e| e.to_string()).collect();
    let mut efforts = efforts;
    if !efforts.contains(&c.effort) {
        efforts.insert(0, c.effort.clone());
    }
    let efforts = options(&efforts);
    let action = match c.run {
        Some(n) => run_post_url(task_id, n),
        None => runs_url(task_id),
    };
    let editing = c.run.is_some();
    let close = agent_tab_url(&task_url(task_id));
    let tab = close.split('?').next().unwrap_or_default().to_string();
    let title = match c.run {
        Some(n) => format!("Agent #{n}, waiting"),
        None => "New agent".to_string(),
    };
    Ok(view! {
        modal(id: "compose-title", title: &title, close: close.clone(),
          <div class="compose">
            if !editing {
                // the prompt depends on the instruction: choosing one reloads the form
                <form id="pick-instruction" method="get" action=(tab.as_str())>
                    <input type="hidden" name="tab" value="agent">
                    <input type="hidden" name="new" value="agent">
                    // pressing Enter in the instruction field submits this form: without script it is how the prompt is loaded
                </form>
                <datalist id="instruction-list">
                    for (name, label) in c.offered.iter() {
                        <option value=(name.as_str())>(label.as_str())</option>
                    }
                </datalist>
            }
            <datalist id="model-list">
                for m in models.iter() {
                    <option value=(m.as_str())></option>
                }
            </datalist>
            if let Some(e) = &c.error {
                <p class="empty-line">"The prompt could not be made: " (e.as_str())</p>
            }
            <form method="post" action=(action.as_str())>
                <input type="hidden" name="instruction" value=(c.instruction.as_str())>
                if !editing {
                    <div class="compose-instruction">
                        field(name: "instruction", label: "Instruction", value: &c.instruction, identifier: true,
                            list: "instruction-list", form: "pick-instruction", autoload: true)
                    </div>
                }
                <div class="compose-pick">
                    select(name: "agent", label: "Agent", options: &agents, current: harness.key())
                    field(name: "model", label: "Model", value: &c.model, identifier: true, list: "model-list")
                    select(name: "effort", label: "Effort", options: &efforts, current: &c.effort)
                </div>
                textarea(name: "prompt", label: "Prompt", value: &c.prompt, loaded: &c.prompt,
                    hint: "What the agent is told. It starts as the instruction's prompt, filled in for the task; edit it freely.")
                <div class="compose-actions">
                    <button class="button primary" type="submit" name="do" value="run" title="Run now" aria-label="Run now">(icon(PLAY)) <span class="sr">"Run now"</span></button>
                    if editing {
                        <button class="button" type="submit" name="do" value="save">"Save"</button>
                    }
                    <button class="button" type="submit" name="do" value="queue">"Queue"</button>
                    <a class="button" href=(close.as_str())>"Close"</a>
                </div>
            </form>
            if let Some(n) = c.run {
                <div class="compose-drop">
                    act(action: run_action_url(task_id, n, "cancel"), label: "Remove from the queue", svg: X)
                </div>
            }
          </div>
        )
    })
}

/// The agents: the open step, if any, and the table, whose first row
/// writes a new one (a pop-up form, `compose_modal`).
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
            if roster.runs.is_empty() {
                empty_line("No agent has worked on this task yet.")
            }
            if let Some(note) = note {
                <p class="empty-line note">(note)</p>
            }
        </div>
    })
}

#[derive(Deserialize)]
struct RunForm {
    instruction: String,
    agent: String,
    model: String,
    effort: String,
    prompt: String,
    /// `run`, `queue` or `save`: the button that was pressed.
    #[serde(rename = "do", default)]
    action: String,
}

/// The run a form asks for, or why not. The model and the effort must be
/// ones the agent takes: the form offers only those.
fn parse_run(form: &RunForm, layers: &Layers) -> std::result::Result<(Kind, Spec), String> {
    use clap::ValueEnum;
    let kind = instruction::find(&form.instruction, layers)
        .ok_or_else(|| format!("'{}' is not an instruction", form.instruction))?
        .kind();
    let agent = Harness::from_str(&form.agent, true)
        .map_err(|_| format!("'{}' is not an agent", form.agent))?;
    let model = form.model.trim();
    if !agent
        .models(&config().agent.models)
        .iter()
        .any(|m| m == model)
    {
        return Err(format!("'{model}' is not a model of {}", agent.key()));
    }
    if !agent.efforts().contains(&form.effort.as_str()) {
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

/// The task a route is about: its name for the commands, and the app to run
/// them with.
fn world(id: i64) -> Result<(App, String, Task, Project)> {
    let db = open_db()?;
    let (task, project) = super::load::task_with_project(&db, id)?;
    // a task without a place for its folder has no runs to act on
    taskdir::of(&db, &project, &task)?;
    let name = task_display(&project, &task);
    let app = App {
        db,
        now: chrono::Local::now().naive_local(),
    };
    Ok((app, name, task, project))
}

/// `POST /task/{id}/runs`: writes an agent down and, if asked, starts it or
/// puts it in line.
#[route(POST "/task/{id}/runs")]
async fn create_run(cx: &Cx, Form(form): Form<RunForm>) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let (app, name, task, project) = world(id)?;
    let (kind, spec) =
        parse_run(&form, &instruction::layers_of(&app.db, &project)).map_err(bad_request)?;
    let n = solve::create(
        &app,
        &name,
        kind,
        &spec,
        &task.description,
        Some(&form.prompt),
        Some(&form.instruction),
    )
    .map_err(refused)?;
    follow_up(&app, &name, n, &form.action)?;
    Err(see_other(agent_url(id, n)).into())
}

/// `GET /task/{id}/prompt?instruction=NAME`: the prompt that instruction makes
/// for the task, and the model it prefers (if the harness offers it), as JSON.
/// The New agent pop-up asks for it when another instruction is picked.
#[route(GET "/task/{id}/prompt")]
async fn prompt_of(cx: &Cx) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let query = query_params::<TabQuery>(cx)?;
    let (app, name, task, project) = world(id)?;
    let mut layers = instruction::layers_of(&app.db, &project);
    if let Ok(dir) = taskdir::of(&app.db, &project, &task) {
        layers.insert(0, (instruction::Source::Task, dir.root().to_path_buf()));
    }
    let chosen = query.instruction.as_deref().unwrap_or_default();
    let inst = instruction::find(chosen, &layers).ok_or_not_found()?;
    let prompt = crate::commands::agent::prompt_for(
        &app,
        Some(&name),
        inst.kind(),
        inst.path.as_deref(),
        true,
        None,
        true,
    )
    .map_err(refused)?;
    let models = Harness::Claude.models(&config().agent.models);
    let body = serde_json::json!({
        "prompt": prompt,
        "model": instruction::preferred_model(&inst.front, &models),
    });
    (
        [(CONTENT_TYPE, HeaderValue::from_static("application/json"))],
        body.to_string(),
    )
        .into_response(cx)
}

/// What the button pressed asks for, once the run is written: `run` begins
/// it now; `queue` and `save` leave it waiting (written down, not started).
fn follow_up(app: &App, name: &str, n: u32, action: &str) -> Result<()> {
    if action == "run" {
        solve::begin(app, name, n, false, &[]).map_err(refused)?;
    }
    Ok(())
}

/// `POST /task/{id}/runs/{n}`: saves the changes to a queued agent, then
/// does what the button pressed asks.
#[route(POST "/task/{id}/runs/{n}")]
async fn edit_run(cx: &Cx, Form(form): Form<RunForm>) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let n = *path_param::<N>(cx)?;
    let (app, name, _, project) = world(id)?;
    let (kind, spec) =
        parse_run(&form, &instruction::layers_of(&app.db, &project)).map_err(bad_request)?;
    solve::amend(&app, &name, n, kind, &form.instruction, &spec, &form.prompt).map_err(refused)?;
    follow_up(&app, &name, n, &form.action)?;
    Err(see_other(agent_url(id, n)).into())
}

/// `POST /task/{id}/runs/{n}/run`: begins a queued agent now.
#[route(POST "/task/{id}/runs/{n}/run")]
async fn run_now(cx: &Cx) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let n = *path_param::<N>(cx)?;
    let (app, name, _, _) = world(id)?;
    solve::begin(&app, &name, n, false, &[]).map_err(refused)?;
    Err(see_other(agent_url(id, n)).into())
}

/// `POST /task/{id}/runs/{n}/cancel`: stops a working agent, or takes a
/// queued one out of the line.
#[route(POST "/task/{id}/runs/{n}/cancel")]
async fn cancel_run(cx: &Cx) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let n = *path_param::<N>(cx)?;
    let (app, name, _, _) = world(id)?;
    solve::cancel(&app, &name, n).map_err(refused)?;
    Err(see_other(agent_tab_url(&task_url(id))).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(instruction: &str, model: &str, effort: &str) -> RunForm {
        RunForm {
            instruction: instruction.into(),
            agent: "claude".into(),
            model: model.into(),
            effort: effort.into(),
            prompt: "do it".into(),
            action: "run".into(),
        }
    }

    #[test]
    fn the_form_is_checked_before_anything_starts() {
        let (kind, spec) = parse_run(&form("fix", " sonnet ", "high"), &Layers::new()).unwrap();
        assert_eq!(
            (kind, spec.model.as_str(), spec.effort.as_str()),
            (Kind::Fix, "sonnet", "high")
        );
        assert!(parse_run(&form("dance", "haiku", "low"), &Layers::new()).is_err());
        // only the models the agent takes, and the efforts it knows
        assert!(parse_run(&form("fix", "gpt-4", "low"), &Layers::new()).is_err());
        assert!(parse_run(&form("fix", "--dangerous", "low"), &Layers::new()).is_err());
        assert!(parse_run(&form("fix", "haiku", "extreme"), &Layers::new()).is_err());
        let mut other = form("fix", "haiku", "low");
        other.agent = "nobody".into();
        assert!(parse_run(&other, &Layers::new()).is_err());
    }

    #[test]
    fn a_custom_instruction_is_labelled_by_its_name() {
        assert_eq!(label_of("review"), "review");
    }

    #[test]
    fn a_kind_has_a_label_and_a_model_list_is_never_empty() {
        for k in ["develop", "test", "check", "fix", "plan"] {
            assert_ne!(label_of(k), k);
        }
        assert!(!Harness::Claude.models(&[]).is_empty());
        assert_eq!(Harness::Claude.models(&["x".into()]), ["x"]);
    }
}
