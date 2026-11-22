//! The routes that write a task's agent runs (create, amend, run, cancel, and the prompt an instruction makes), and the parts of a run the agent pages share: its label, state pill and action buttons.

use crate::{assets::json, layers::errors::refused, parts::all::*, prelude::*};
use iter_core::{
    agent::{
        instruction::{self, Instruction, Layers},
        run::{Harness, RunRecord},
        solve::{self, Launch, Spec},
        taskdir,
    },
    app::App,
    config::config,
    models::{Project, Task},
};
use serde::Deserialize;

path_param!(n: u32, error = bad_request);

pub(crate) fn label_of(name: &str) -> String {
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
pub(crate) async fn act(
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

/// What can be done to a run now: start or drop a queued one, stop a running one.
#[component]
pub(crate) async fn run_acts(task_id: i64, run: &RunRecord) -> Result<impl View> {
    let shown = run.shown_status();
    Ok(view! {
        if shown == "queued" {
            act(action: run_action_url(task_id, run.n, RunAction::Run), label: "Run now", svg: PLAY)
            act(action: run_action_url(task_id, run.n, RunAction::Cancel), label: "Remove from the queue", svg: X)
        } else if shown == "running" {
            act(action: run_action_url(task_id, run.n, RunAction::Cancel), label: "Stop the agent", svg: STOP)
        }
    })
}

#[component]
pub(crate) async fn state_pill(run: &RunRecord) -> Result<impl View> {
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
    let found = instruction::find(&form.instruction, layers)
        .ok_or_else(|| format!("'{}' is not an instruction", form.instruction))?;
    let agent =
        Harness::parse(&form.agent).ok_or_else(|| format!("'{}' is not an agent", form.agent))?;
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

/// The task's app, name, row and project; a task without a place for its folder has no runs to act on.
fn world(id: i64) -> Result<(App, String, Task, Project)> {
    let db = fresh_db()?;
    let (task, project) = crate::load::task_with_project(&db, id)?;
    taskdir::of(&db, &project, &task)?;
    let name = task.label(&project).to_string();
    let app = App::over(db, chrono::Local::now().naive_local());
    Ok((app, name, task, project))
}

/// The instruction and spec the form asks for, among the task's layers.
fn checked(
    app: &App,
    project: &Project,
    task: &Task,
    form: &RunForm,
) -> Result<(Instruction, Spec)> {
    let layers = instruction::layers_for_task(&app.db, project, task)?;
    Ok(parse_run(form, &layers).map_err(bad_request)?)
}

#[route(POST "/task/{id}/runs")]
pub(super) async fn create_run(cx: &Cx, Form(form): Form<RunForm>) -> Result<Response> {
    let id = path_id(cx)?;
    let (app, name, task, project) = world(id)?;
    let (found, spec) = checked(&app, &project, &task, &form)?;
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

/// `GET /task/{id}/prompt?instruction=NAME`: the prompt that instruction makes for the task, and the model it prefers (if the harness offers it), as JSON.
#[route(GET "/task/{id}/prompt")]
pub(super) async fn prompt_of(cx: &Cx) -> Result<Response> {
    let id = path_id(cx)?;
    let query = query_params::<TabQuery>(cx)?;
    let (app, name, task, project) = world(id)?;
    let layers = instruction::layers_for_task(&app.db, &project, &task)?;
    let chosen = query.instruction.as_deref().unwrap_or_default();
    let inst = instruction::find(chosen, &layers).ok_or_not_found()?;
    let prompt = iter_core::agent::task_prompt::prompt_for(
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
pub(super) async fn edit_run(cx: &Cx, Form(form): Form<RunForm>) -> Result<Response> {
    let id = path_id(cx)?;
    let n = *path_param::<N>(cx)?;
    let (app, name, task, project) = world(id)?;
    let (found, spec) = checked(&app, &project, &task, &form)?;
    solve::amend(&app, &name, n, &found.name, &spec, &form.prompt).map_err(refused)?;
    follow_up(&app, &name, n, form.action)?;
    Err(see_other(agent_url(id, n)).into())
}

#[route(POST "/task/{id}/runs/{n}/run")]
pub(super) async fn run_now(cx: &Cx) -> Result<Response> {
    let id = path_id(cx)?;
    let n = *path_param::<N>(cx)?;
    let (app, name, _, _) = world(id)?;
    solve::begin(&app, &name, n, &Launch::detached()).map_err(refused)?;
    Err(see_other(agent_url(id, n)).into())
}

#[route(POST "/task/{id}/runs/{n}/cancel")]
pub(super) async fn cancel_run(cx: &Cx) -> Result<Response> {
    let id = path_id(cx)?;
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
        let w = iter_core::fixtures::world("runs-task-layer");
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
            iter_core::fixtures::shipped_instructions()
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
