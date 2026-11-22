//! `iter task solve`, `iter task runs`: handing a task to an agent working on its own in the task's worktree.

use crate::args::{RunEnd, RunsArgs, SolveArgs};
use crate::editor::edit_text;
use crate::ingest::say_auto;
use iter_core::agent::ingest_job::Auto;
use iter_core::agent::run::{Harness, RunRecord, Status};
use iter_core::agent::solve::{Launch, Observer, Spec, close, open_subagent, start};
use iter_core::agent::task_prompt::{instruction_for, locate};
use iter_core::app::App;
use iter_core::db::Table;
use iter_core::error::AgentError;
use iter_core::error::Result;
use iter_core::models::Project;
use iter_core::text::split_front;

struct Console;

impl Observer for Console {
    fn ingest(&self, project: &Project, outcome: &Auto) {
        say_auto(project, outcome);
    }

    fn working(&self, name: &str, run: &RunRecord) {
        println!("run {} of {name}: {} working...", run.n, run.agent.key());
    }
}

/// A subagent's model or effort when the session does not say: the session's own.
const INHERIT: &str = "inherit";

/// The buffer: front matter, then the description the agent is given.
fn render(spec: &Spec, description: &str) -> Result<String> {
    Ok(format!(
        "---\n{}---\n\n{}\n",
        serde_yaml::to_string(spec)?,
        description.trim_end()
    ))
}

fn parse(buffer: &str) -> Result<(Spec, String)> {
    let split = split_front(buffer);
    let front = split.front.ok_or(AgentError::BufferLostFront)?;
    let body = split.body;
    let spec = serde_yaml::from_str(front).map_err(AgentError::BufferFront)?;
    Ok((spec, body.trim().to_string()))
}

pub fn solve(app: &App, req: &SolveArgs) -> Result<()> {
    let (project, task, dir) = locate(app, req.task.name())?;
    // before the editor: an instruction found nowhere, or interactive, has nothing to start
    if instruction_for(app, &project, &task, &req.instruction)?
        .front
        .interactive
    {
        return Err(AgentError::Interactive(req.instruction.clone()).into());
    }
    let name = task.label(&project).to_string();
    let mut spec = Spec {
        agent: req.agent.unwrap_or_default(),
        model: req.model.as_deref().unwrap_or("haiku").to_string(),
        effort: req.effort.as_deref().unwrap_or("medium").to_string(),
    };
    let mut description = task.description.clone();
    if !req.yes {
        match edit_text(&render(&spec, &description)?)? {
            Some(text) => (spec, description) = parse(&text)?,
            None => {
                println!("no changes -- nothing started (save the buffer to start, or use --yes)");
                return Ok(());
            }
        }
    }
    let n = start(
        app,
        &name,
        &req.instruction,
        &spec,
        &description,
        &Launch {
            wait: req.wait,
            extra: &req.extra,
            say: &Console,
        },
    )?;
    if req.wait {
        let run = dir.read_run(n)?.ok_or(AgentError::RunVanished(n))?;
        println!("run {n}: {}", run.shown_status());
    } else {
        println!(
            "run {n} of {name} started ({}). Follow it: iter task runs {} --log {n}",
            spec.agent.key(),
            iter_core::process::shell_word(&name)
        );
    }
    Ok(())
}

/// `solve --subagent`: records the run, prints the prompt to give the subagent; how to close it
/// goes to standard error, so the prompt can be taken as it is.
pub fn subagent(app: &App, a: &SolveArgs) -> Result<()> {
    let (project, task, _) = locate(app, a.task.name())?;
    let name = task.label(&project).to_string();
    let inherit = |setting: &Option<String>| setting.clone().unwrap_or_else(|| INHERIT.to_string());
    let spec = Spec {
        agent: Harness::default(),
        model: inherit(&a.model),
        effort: inherit(&a.effort),
    };
    let (n, prompt) = open_subagent(app, &name, &a.instruction, &spec, &Console)?;
    eprintln!(
        "run {n} ({}, {}, effort {}): give the prompt below to a new subagent; when it is done: iter task runs --close {n} --status done -m \"<its report>\"",
        a.instruction, spec.model, spec.effort
    );
    println!("{}", prompt.trim_end());
    Ok(())
}

impl From<RunEnd> for Status {
    fn from(end: RunEnd) -> Self {
        match end {
            RunEnd::Done => Status::Done,
            RunEnd::Failed => Status::Failed,
            RunEnd::Cancelled => Status::Cancelled,
        }
    }
}

/// `iter task runs`: `--close` ends a subagent's run, `--log` prints one, else the runs are listed.
pub fn runs(app: &App, a: &RunsArgs) -> Result<()> {
    let (project, task, dir) = locate(app, a.task.name())?;
    if let Some(n) = a.close {
        return close_run(app, &task.label(&project).to_string(), n, a);
    }
    if let Some(n) = a.log {
        let text = std::fs::read_to_string(dir.run_log_path(n))
            .map_err(|_| AgentError::RunLogMissing(n))?;
        print!("{text}");
        return Ok(());
    }
    let runs = dir.runs();
    if runs.is_empty() {
        println!(
            "No runs of {} yet: `iter task solve {} -i develop`.",
            task.label(&project),
            task.id()
        );
    }
    for r in runs {
        println!(
            "#{:<3} {:<9} {:<8} {:<17} {:<7} {:<7} {}{}",
            r.n,
            r.shown_status(),
            r.instruction,
            r.agent_label(),
            r.model,
            r.effort,
            r.started,
            r.ended.map(|e| format!(" -> {e}")).unwrap_or_default()
        );
    }
    Ok(())
}

/// `runs --close`: the report is `-m`, else standard input when it is not a terminal, else none.
fn close_run(app: &App, name: &str, n: u32, a: &RunsArgs) -> Result<()> {
    use std::io::{IsTerminal, Read};
    let report = match &a.message {
        Some(text) => text.clone(),
        None if !std::io::stdin().is_terminal() => {
            let mut text = String::new();
            std::io::stdin().read_to_string(&mut text)?;
            text
        }
        None => String::new(),
    };
    let status = Status::from(a.status.unwrap_or(RunEnd::Done));
    close(app, name, n, status, &report)?;
    println!("run {n}: {}", status.label());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_buffer_round_trips_and_a_lost_front_matter_is_an_error() {
        let spec = Spec {
            agent: Harness::Claude,
            model: "sonnet".into(),
            effort: "high".into(),
        };
        let text = render(&spec, "Fix the retries.\n\nSecond paragraph.\n").unwrap();
        assert_eq!(
            parse(&text).unwrap(),
            (spec, "Fix the retries.\n\nSecond paragraph.".to_string())
        );
        assert!(parse("just text").is_err());
        assert!(parse("---\nagent: nobody\nmodel: x\neffort: y\n---\nbody").is_err());
    }
}
