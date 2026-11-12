//! The form that writes an agent's run, as a pop-up over the task's Agent tab and as the Settings
//! tab of a queued run (`agents.rs`). The routes that receive it are `task_runs.rs`.

use super::task_runs::act;
use super::ui::field::{field, select, textarea};
use super::ui::modal::modal;
use super::ui::{PLAY, X, icon};
use super::url::{RunAction, agent_tab_url, run_action_url, run_post_url, runs_url, task_url};
use crate::agent::run::Harness;
use crate::config::config;
use topcoat::{
    Result,
    view::{View, component, view},
};

pub struct Compose {
    pub instruction: String,
    pub offered: Vec<(String, String)>,
    pub run: Option<u32>,
    pub model: String,
    pub effort: String,
    pub prompt: String,
    pub error: Option<String>,
}

fn options(values: &[String]) -> Vec<(String, String)> {
    values.iter().map(|v| (v.clone(), v.clone())).collect()
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
