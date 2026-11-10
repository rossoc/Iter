//! The agents of a task at the top of its Agent tab: the table, the form that writes a new one, and the page of one that has begun.

use super::assets::json;
use super::errors::refused;
use super::sections::TabQuery;
use super::ui::icon;
use super::url::{agent_tab_url, agent_url, task_url};
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
