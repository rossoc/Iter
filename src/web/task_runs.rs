//! The agents of a task at the top of its Agent tab: the table, the form that writes a new one, and the page of one that has begun.

use super::assets::json;
use super::errors::refused;
use super::sections::TabQuery;
use super::ui::icon;
use super::url::{agent_tab_url, agent_url, task_url};
use super::{Id, fresh_db};
use crate::agent::instruction::{self, Instruction, Layers};
use crate::agent::run::{Harness, RunRecord};
use crate::agent::solve::{self, Launch, Spec};
use crate::agent::taskdir::{self};
use crate::app::App;
use crate::config::config;
use crate::models::{Project, Task};
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

/// The instruction must be one the layers hold, and the model and the effort ones the agent
/// takes: the form offers only those.
fn parse_run(form: &RunForm, layers: &Layers) -> std::result::Result<(Instruction, Spec), String> {
    use clap::ValueEnum;
    let found = instruction::find(&form.instruction, layers)
        .ok_or_else(|| format!("'{}' is not an instruction", form.instruction))?;
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
        found,
        Spec {
            agent,
            model: model.to_string(),
            effort: form.effort.clone(),
        },
    ))
}

fn world(id: i64) -> Result<(App, String, Task, Project)> {
    let db = fresh_db()?;
    let (task, project) = super::load::task_with_project(&db, id)?;
    // a task without a place for its folder has no runs to act on
    taskdir::of(&db, &project, &task)?;
    let name = task.label(&project).to_string();
    let app = App::over(db, chrono::Local::now().naive_local());
    Ok((app, name, task, project))
}

#[route(POST "/task/{id}/runs")]
async fn create_run(cx: &Cx, Form(form): Form<RunForm>) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let (app, name, task, project) = world(id)?;
    let layers = instruction::layers_for_task(&app.db, &project, &task)?;
    let (found, spec) = parse_run(&form, &layers).map_err(bad_request)?;
    let n = solve::create(
        &app,
        &name,
        &found.name,
        &spec,
        &task.description,
        Some(&form.prompt),
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
    let layers = instruction::layers_for_task(&app.db, &project, &task)?;
    let chosen = query.instruction.as_deref().unwrap_or_default();
    let inst = instruction::find(chosen, &layers).ok_or_not_found()?;
    let prompt = crate::agent::task_prompt::prompt_for(
        &app,
        Some(&name),
        &inst.name,
        None,
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
    let (app, name, task, project) = world(id)?;
    let layers = instruction::layers_for_task(&app.db, &project, &task)?;
    let (found, spec) = parse_run(&form, &layers).map_err(bad_request)?;
    solve::amend(&app, &name, n, &found.name, &spec, &form.prompt).map_err(refused)?;
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

    /// The compose form offers the task folder's own instructions; the POST must take them too.
    #[test]
    fn a_task_folder_instruction_is_one_a_run_can_be_given() {
        let w = crate::fixtures::world("runs-task-layer");
        let project = w.db.get::<Project>(w.project_id).unwrap().unwrap();
        let task = w.db.get::<Task>(w.task_id).unwrap().unwrap();
        let dir = taskdir::of(&w.db, &project, &task).unwrap();
        std::fs::create_dir_all(dir.root().join("instructions")).unwrap();
        std::fs::write(
            dir.root().join("instructions").join("review.md"),
            "---\nworktree: true\n---\nLook it over.\n",
        )
        .unwrap();

        let layers = instruction::layers_for_task(&w.db, &project, &task).unwrap();
        let (found, _) = parse_run(&form("review", "sonnet", "high"), &layers).unwrap();
        assert_eq!(found.name, "review");
        assert!(
            found.front.worktree,
            "the front matter's worktree is picked up"
        );
        // without the task's layer, as the POST used to check it
        let project_only = instruction::layers_of(&w.db, &project).unwrap();
        assert!(parse_run(&form("review", "sonnet", "high"), &project_only).is_err());

        std::fs::remove_dir_all(&w.notes).unwrap();
    }

    #[test]
    fn the_form_is_checked_before_anything_starts() {
        let shipped: Layers = vec![(
            instruction::Source::Global,
            crate::fixtures::shipped_instructions()
                .parent()
                .unwrap()
                .to_path_buf(),
        )];
        let (found, spec) = parse_run(&form("fix", " sonnet ", "high"), &shipped).unwrap();
        assert_eq!(
            (
                found.name.as_str(),
                found.front.worktree,
                spec.model.as_str(),
                spec.effort.as_str()
            ),
            ("fix", true, "sonnet", "high")
        );
        let (plan, _) = parse_run(&form("plan", "haiku", "low"), &shipped).unwrap();
        assert!(!plan.front.worktree);
        assert!(parse_run(&form("dance", "haiku", "low"), &shipped).is_err());
        // an instruction found nowhere is not one: nothing is built in
        assert!(parse_run(&form("fix", "haiku", "low"), &Layers::new()).is_err());
        // only the models the agent takes, and the efforts it knows
        assert!(parse_run(&form("fix", "gpt-4", "low"), &shipped).is_err());
        assert!(parse_run(&form("fix", "--dangerous", "low"), &shipped).is_err());
        assert!(parse_run(&form("fix", "haiku", "extreme"), &shipped).is_err());
        let mut other = form("fix", "haiku", "low");
        other.agent = "nobody".into();
        assert!(parse_run(&other, &shipped).is_err());
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
