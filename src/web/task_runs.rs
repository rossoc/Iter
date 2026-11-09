//! The agents of a task at the top of its Agent tab: the table, the form that writes a new one, and the page of one that has begun.

use super::assets::json;
use super::errors::refused;
use super::sections::TabQuery;
use super::ui::field::{field, select, textarea};
use super::ui::modal::modal;
use super::ui::{PLAY, X, icon};
use super::url::{
    RunAction, agent_tab_url, agent_url, run_action_url, run_post_url, runs_url, task_url,
};
use super::{Id, open_db};
use crate::agent::instruction::{self, Layers};
use crate::agent::prompt::Kind;
use crate::agent::run::{Harness, RunRecord};
use crate::agent::solve::{self, Launch, Spec};
use crate::agent::taskdir::{self};
use crate::app::App;
use crate::config::config;
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

pub struct Compose {
    pub instruction: String,
    pub offered: Vec<(String, String)>,
    pub run: Option<u32>,
    pub model: String,
    pub effort: String,
    pub prompt: String,
    pub error: Option<String>,
}

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

/// Named for assistive technology and as a tooltip.
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

/// The form that writes an agent's run: new (`c.run` is `None`) or a queued one being amended.
/// `close` adds a Close link (the pop-up has one; the settings tab has nowhere to go).
#[component]
pub async fn compose_form(
    task_id: i64,
    c: &Compose,
    harness: Harness,
    close: Option<&str>,
) -> Result<impl View> {
    let agents = vec![(harness.key().to_string(), "Claude".to_string())];
    let mut models = harness.models(&config().agent.models);
    // a queued run keeps its model even if the list changed since
    if !models.contains(&c.model) {
        models.insert(0, c.model.clone());
    }
    let mut efforts: Vec<String> = harness.efforts().iter().map(|e| e.to_string()).collect();
    if !efforts.contains(&c.effort) {
        efforts.insert(0, c.effort.clone());
    }
    let efforts = options(&efforts);
    let action = match c.run {
        Some(n) => run_post_url(task_id, n),
        None => runs_url(task_id),
    };
    let editing = c.run.is_some();
    let tab = agent_tab_url(&task_url(task_id))
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    let hint = if editing {
        "What the agent is told; edit it freely until it runs."
    } else {
        "What the agent is told. It starts as the instruction's prompt, filled in for the task; edit it freely."
    };
    Ok(view! {
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
        <form class="settings-form" method="post" action=(action.as_str())>
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
            textarea(name: "prompt", label: "Prompt", value: &c.prompt, loaded: &c.prompt, hint: hint)
            <div class="compose-actions">
                <button class="button primary" type="submit" name="do" value="run" title="Run now" aria-label="Run now">(icon(PLAY)) <span class="sr">"Run now"</span></button>
                if editing {
                    <button class="button" type="submit" name="do" value="save">"Save"</button>
                }
                <button class="button" type="submit" name="do" value="queue">"Queue"</button>
                if let Some(close) = close {
                    <a class="button" href=(close)>"Close"</a>
                }
            </div>
        </form>
    })
}

#[component]
pub async fn compose_modal(task_id: i64, c: &Compose) -> Result<impl View> {
    let close = agent_tab_url(&task_url(task_id));
    let title = match c.run {
        Some(n) => format!("Agent #{n}, waiting"),
        None => "New agent".to_string(),
    };
    Ok(view! {
        modal(id: "compose-title", title: &title, close: close.clone(),
          <div class="compose">
            compose_form(task_id: task_id, c: c, harness: Harness::Claude, close: Some(close.as_str()))
            if let Some(n) = c.run {
                <div class="compose-drop">
                    act(action: run_action_url(task_id, n, RunAction::Cancel), label: "Remove from the queue", svg: X)
                </div>
            }
          </div>
        )
    })
}

#[derive(Deserialize)]
struct RunForm {
    instruction: String,
    agent: String,
    model: String,
    effort: String,
    prompt: String,
    #[serde(rename = "do", default)]
    action: RunButton,
}

#[derive(Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum RunButton {
    Run,
    #[default]
    Queue,
    Save,
}

/// The model and the effort must be ones the agent takes: the form offers only those.
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
    follow_up(&app, &name, n, form.action)?;
    Err(see_other(agent_url(id, n)).into())
}

/// `GET /task/{id}/prompt?instruction=NAME`: the prompt that instruction makes
/// for the task, and the model it prefers (if the harness offers it), as JSON.
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
    let prompt = crate::agent::task_prompt::prompt_for(
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
    json(cx, body.to_string(), false)
}

fn follow_up(app: &App, name: &str, n: u32, action: RunButton) -> Result<()> {
    if action == RunButton::Run {
        solve::begin(app, name, n, &Launch::detached()).map_err(refused)?;
    }
    Ok(())
}

#[route(POST "/task/{id}/runs/{n}")]
async fn edit_run(cx: &Cx, Form(form): Form<RunForm>) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let n = *path_param::<N>(cx)?;
    let (app, name, _, project) = world(id)?;
    let (kind, spec) =
        parse_run(&form, &instruction::layers_of(&app.db, &project)).map_err(bad_request)?;
    solve::amend(&app, &name, n, kind, &form.instruction, &spec, &form.prompt).map_err(refused)?;
    follow_up(&app, &name, n, form.action)?;
    Err(see_other(agent_url(id, n)).into())
}

#[route(POST "/task/{id}/runs/{n}/run")]
async fn run_now(cx: &Cx) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let n = *path_param::<N>(cx)?;
    let (app, name, _, _) = world(id)?;
    solve::begin(&app, &name, n, &Launch::detached()).map_err(refused)?;
    Err(see_other(agent_url(id, n)).into())
}

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
            action: RunButton::Run,
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
