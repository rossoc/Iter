//! Agents as pages: the list of every agent of every task (`/agents`) and the
//! page of one (`/agent/{id}/{n}`, `id` the task's): the facts in a box on the
//! right, and the tabs Prompt, Output and (while it waits) Settings. The task's Agents tab links here; the runs are
//! `agent::run`'s files and the buttons post to `task_runs`' routes.

use super::instructions::{Use, usage};
use super::sections::TabQuery;
use super::task_runs::{act, label_of, state_pill};
use super::ui::breadcrumb::Crumb;
use super::ui::empty_line::empty_line;
use super::ui::field::{field, select, textarea};
use super::ui::frame::frame;
use super::ui::layout::info_columns;
use super::ui::page_header::{Kicker, page_header};
use super::ui::prose::description;
use super::ui::table::{cell, col_head};
use super::ui::tabs::{Tab, tabs};
use super::ui::{PLAY, STOP, X, icon};
use super::url::{
    agent_tab_url, agent_url, agents_url, page_url, run_action_url, run_post_url, task_url,
};
use super::{Id, open_db};
use crate::agent::run::RunRecord;
use crate::agent::taskdir;
use crate::config::config;
use crate::utils::resolve::task_display;
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, error::RouterErrorExt, page, path_param, query_params},
    view::{View, component, view},
};

path_param!(n: u32, error = bad_request);

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(list_page).page(detail_page)
}

/// The most of a log the page shows, from its end.
const LOG_BYTES: usize = 16 * 1024;

/// The end of `log` (at most [`LOG_BYTES`], on a character boundary) and
/// whether the start was cut off.
fn tail(log: &str) -> (String, bool) {
    let cut = log.len() > LOG_BYTES;
    let start = (log.len().saturating_sub(LOG_BYTES)..=log.len())
        .find(|&i| log.is_char_boundary(i))
        .unwrap_or(log.len());
    (log[start..].to_string(), cut)
}

/// The title of an agent's page: `#3 Develop`.
fn heading(run: &RunRecord) -> String {
    format!("#{} {}", run.n, label_of(run.instruction_name()))
}

#[component]
async fn use_row(u: &Use) -> Result<impl View> {
    let run = &u.run;
    Ok(view! {
        <tr role="row">
            cell(
                <a class="u" href=(agent_url(u.task_id, run.n))>(heading(run))</a>
                // what it was told to use: only on the row under the pointer
                <span class="hint">(run.model.as_str()) ", " (run.effort.as_str())</span>
            )
            cell(<a class="u muted" href=(task_url(u.task_id))>(u.task.as_str())</a>)
            cell(state_pill(run: run))
        </tr>
    })
}

#[page("/agents")]
async fn list_page(cx: &Cx) -> Result<impl View> {
    let _ = cx;
    let db = open_db()?;
    let uses = usage(&db, None);
    Ok(view! {
        list_screen(uses: &uses)
    })
}

#[component]
async fn list_screen(uses: &[Use]) -> Result<impl View> {
    Ok(view! {
        frame(title: &["Agents"], current: "/agents",
            page_header(title: "Agents", kicker: Kicker::Eyebrow("Global"))
            if uses.is_empty() {
                empty_line("No agent has worked on any task yet.")
            } else {
                <table class="tasks agents" role="table">
                    <caption class="sr">"Agents of every task, the latest first"</caption>
                    <thead role="rowgroup">
                        <tr role="row">
                            col_head("Agent")
                            col_head("Task")
                            col_head("Status")
                        </tr>
                    </thead>
                    <tbody role="rowgroup">
                        for u in uses.iter() {
                            use_row(u: u)
                        }
                    </tbody>
                </table>
            }
        )
    })
}

/// An agent as its page shows it.
struct Loaded {
    task_id: i64,
    task: String,
    run: RunRecord,
    prompt: String,
    log: String,
    cut: bool,
}

fn load(id: i64, n: u32) -> Result<Loaded> {
    let db = open_db()?;
    let (task, project) = super::load::task_with_project(&db, id)?;
    let dir = taskdir::of(&db, &project, &task)?;
    let run = dir.read_run(n)?.ok_or_not_found()?;
    let (log, cut) = tail(&std::fs::read_to_string(dir.run_log_path(n)).unwrap_or_default());
    Ok(Loaded {
        task_id: id,
        task: task_display(&project, &task),
        prompt: std::fs::read_to_string(dir.run_prompt_path(n)).unwrap_or_default(),
        run,
        log,
        cut,
    })
}

/// The tabs of an agent's page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part {
    Prompt,
    Output,
    Settings,
}

/// The tab a request asks for. Settings exists only while the agent waits
/// (once it has begun nothing can change); without a usable `tab` a waiting
/// agent opens on its prompt and one that has begun on its output.
fn part(tab: Option<&str>, waiting: bool) -> Part {
    match tab {
        Some("prompt") => Part::Prompt,
        Some("output") => Part::Output,
        Some("settings") if waiting => Part::Settings,
        _ if waiting => Part::Prompt,
        _ => Part::Output,
    }
}

#[page("/agent/{id}/{n}")]
async fn detail_page(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let n = *path_param::<N>(cx)?;
    let query = query_params::<TabQuery>(cx)?;
    let loaded = load(id, n)?;
    let current = part(query.tab.as_deref(), loaded.run.is_queued());
    let raw = query.raw.is_some();
    Ok(view! {
        detail_screen(a: &loaded, part: current, raw: raw)
    })
}

/// The buttons that fit the agent's state: a waiting one is run or removed,
/// a working one is stopped.
#[component]
async fn actions(task_id: i64, run: &RunRecord) -> Result<impl View> {
    let shown = run.shown_status();
    Ok(view! {
        <div class="compose-actions">
            if shown == "queued" {
                act(action: run_action_url(task_id, run.n, "run"), label: "Run now", svg: PLAY)
                act(action: run_action_url(task_id, run.n, "cancel"), label: "Remove from the queue", svg: X)
            } else if shown == "running" {
                act(action: run_action_url(task_id, run.n, "cancel"), label: "Stop the agent", svg: STOP)
            }
        </div>
    })
}

/// The small box on the right: the facts, and the buttons.
#[component]
async fn facts(a: &Loaded) -> Result<impl View> {
    let run = &a.run;
    Ok(view! {
        <aside class="panel run-box" aria-labelledby="run-title">
            <h2 id="run-title" class="label">"Agent"</h2>
            <dl>
                <dt>"Status"</dt><dd>state_pill(run: run)</dd>
                <dt>"Instruction"</dt><dd>(label_of(run.instruction_name()))</dd>
                <dt>"Agent"</dt><dd>(run.agent.key())</dd>
                <dt>"Model"</dt><dd>(run.model.as_str())</dd>
                <dt>"Effort"</dt><dd>(run.effort.as_str())</dd>
                <dt>"Task"</dt><dd><a class="u" href=(task_url(a.task_id))>(a.task.as_str())</a></dd>
                if !run.is_queued() {
                    <dt>"Began"</dt><dd class="mono">(run.started.as_str())</dd>
                }
                if let Some(ended) = &run.ended {
                    <dt>"Ended"</dt><dd class="mono">(ended.as_str())</dd>
                }
                if let Some(code) = run.exit_code {
                    <dt>"Exit code"</dt><dd class="mono">(code.to_string())</dd>
                }
                if let Some(branch) = &run.branch {
                    <dt>"Branch"</dt><dd class="mono">(branch.as_str())</dd>
                }
            </dl>
            actions(task_id: a.task_id, run: run)
        </aside>
    })
}

/// What the agent was told, as the markdown it is.
#[component]
async fn prompt_part(a: &Loaded) -> Result<impl View> {
    Ok(view! {
        if a.prompt.trim().is_empty() {
            empty_line("No prompt was written.")
        } else {
            description(text: &a.prompt)
        }
    })
}

/// What the agent printed: its answer is markdown, so it is shown as such;
/// `raw` shows the text as it is.
#[component]
async fn output_part(a: &Loaded, raw: bool) -> Result<impl View> {
    let base = agent_url(a.task_id, a.run.n);
    let running = a.run.is_running();
    let other = if raw {
        page_url(&base, &[("tab", Some("output"))])
    } else {
        page_url(&base, &[("tab", Some("output")), ("raw", Some("1"))])
    };
    Ok(view! {
        if a.log.is_empty() {
            empty_line("Nothing printed yet.")
        } else {
            if a.cut {
                <p class="empty-line">"The start of the log is not shown."</p>
            }
            if raw {
                <pre class="log" tabindex="0">(a.log.as_str())</pre>
            } else {
                description(text: &a.log)
            }
        }
        <p class="output-links">
            if !a.log.is_empty() {
                <a class="u" href=(other.as_str())>(if raw { "Show formatted" } else { "Show raw" })</a>
            }
            if running {
                <a class="u" href=(page_url(&base, &[("tab", Some("output"))]))>"Refresh"</a>
            }
        </p>
    })
}

/// Settings of a waiting agent: model, effort and prompt, saved, run or put
/// in line. (The instruction is fixed: a different one is a new agent.)
#[component]
async fn settings_part(a: &Loaded) -> Result<impl View> {
    let run = &a.run;
    let harness = run.agent;
    let agents = vec![(harness.key().to_string(), "Claude".to_string())];
    let mut models = harness.models(&config().agent.models);
    if !models.contains(&run.model) {
        models.insert(0, run.model.clone());
    }
    let mut efforts: Vec<String> = harness.efforts().iter().map(|e| e.to_string()).collect();
    if !efforts.contains(&run.effort) {
        efforts.insert(0, run.effort.clone());
    }
    let efforts: Vec<(String, String)> = efforts.into_iter().map(|e| (e.clone(), e)).collect();
    let action = run_post_url(a.task_id, run.n);
    Ok(view! {
        <datalist id="model-list">
            for m in models.iter() {
                <option value=(m.as_str())></option>
            }
        </datalist>
        <form class="settings-form" method="post" action=(action.as_str())>
            <input type="hidden" name="instruction" value=(run.instruction_name())>
            <div class="compose-pick">
                select(name: "agent", label: "Agent", options: &agents, current: harness.key())
                field(name: "model", label: "Model", value: &run.model, identifier: true, list: "model-list")
                select(name: "effort", label: "Effort", options: &efforts, current: &run.effort)
            </div>
            textarea(name: "prompt", label: "Prompt", value: &a.prompt,
                hint: "What the agent is told; edit it freely until it runs.")
            <div class="compose-actions">
                <button class="button primary" type="submit" name="do" value="run" title="Run now" aria-label="Run now">(icon(PLAY)) <span class="sr">"Run now"</span></button>
                <button class="button" type="submit" name="do" value="save">"Save"</button>
                <button class="button" type="submit" name="do" value="queue">"Queue"</button>
            </div>
        </form>
    })
}

#[component]
async fn detail_screen(a: &Loaded, part: Part, raw: bool) -> Result<impl View> {
    let run = &a.run;
    let base = agent_url(a.task_id, run.n);
    let tab =
        |label: &'static str, key: &str| Tab::link(label, page_url(&base, &[("tab", Some(key))]));
    let mut items = vec![
        tab("Prompt", "prompt").marked(part == Part::Prompt),
        tab("Output", "output").marked(part == Part::Output),
    ];
    if run.is_queued() {
        items.push(tab("Settings", "settings").marked(part == Part::Settings));
    }
    let title = heading(run);
    let crumbs = [
        Crumb::link("Agents", agents_url()),
        Crumb::link(a.task.as_str(), agent_tab_url(&task_url(a.task_id))),
        Crumb::label(format!("#{}", run.n)),
    ];
    let parts = [a.task.as_str(), "Agent"];
    Ok(view! {
        frame(title: &parts, current: "/agents",
            page_header(title: &title, kicker: Kicker::Crumbs(&crumbs))
            tabs(label: "Sections", items: &items)
            info_columns(
                <div>
                    if part == Part::Settings {
                        settings_part(a: a)
                    } else if part == Part::Output {
                        output_part(a: a, raw: raw)
                    } else {
                        prompt_part(a: a)
                    }
                </div>
                facts(a: a)
            )
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_log_is_shown_from_its_end_on_a_character_boundary() {
        assert_eq!(tail("short"), ("short".to_string(), false));
        let long = format!("start{}end", "é".repeat(LOG_BYTES));
        let (text, cut) = tail(&long);
        assert!(cut && text.ends_with("end") && !text.contains("start"));
        assert!(text.len() <= LOG_BYTES);
    }

    #[test]
    fn the_tab_depends_on_whether_the_agent_has_begun() {
        // waiting: its prompt first, and Settings exists
        assert_eq!(part(None, true), Part::Prompt);
        assert_eq!(part(Some("settings"), true), Part::Settings);
        assert_eq!(part(Some("output"), true), Part::Output);
        // begun: its output first, and Settings is not there
        assert_eq!(part(None, false), Part::Output);
        assert_eq!(part(Some("settings"), false), Part::Output);
        assert_eq!(part(Some("prompt"), false), Part::Prompt);
        assert_eq!(part(Some("nonsense"), false), Part::Output);
    }

    #[test]
    fn the_title_says_the_number_and_what_it_does() {
        let mut run = RunRecord {
            n: 3,
            agent: crate::agent::run::Harness::Claude,
            model: "haiku".into(),
            effort: "low".into(),
            kind: "develop".into(),
            instruction: None,
            status: crate::agent::run::Status::Done,
            started: "2026-10-30 10:00:00".into(),
            ended: None,
            exit_code: Some(0),
            branch: None,
            worktree: None,
            pid: None,
        };
        assert_eq!(heading(&run), "#3 Develop");
        run.instruction = Some("review".into());
        assert_eq!(heading(&run), "#3 review");
    }
}
