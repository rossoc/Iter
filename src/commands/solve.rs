//! `iter task solve`, `iter task runs`: handing a task to an agent that works
//! on its own in the task's worktree. The files of a run are
//! `agent::run`'s; the harness is whatever `Harness` says to start.

use crate::agent::error::AgentError;
use crate::agent::prompt::Kind;
use crate::agent::run::{Harness, RunRecord};
use crate::agent::solve::{Launch, Observer, Spec, start};
use crate::agent::task_prompt::locate;
use crate::app::App;
use crate::commands::ingest::say_auto;
use crate::error::Result;
use crate::md_edit::{edit_text, split_at_divider};
use crate::models::Project;
use crate::utils::resolve::task_display;
use crate::wiki::ingest_job::Auto;

/// What the command line says while a run begins.
struct Console;

impl Observer for Console {
    fn ingest(&self, project: &Project, outcome: &Auto) {
        say_auto(project, outcome);
    }

    fn working(&self, name: &str, run: &RunRecord) {
        println!("run {} of {name}: {} working...", run.n, run.agent.key());
    }
}

pub struct SolveRequest<'a> {
    pub task: Option<&'a str>,
    pub kind: Kind,
    pub agent: Option<Harness>,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
    pub yes: bool,
    pub wait: bool,
    pub extra: &'a [String],
}

/// What the buffer holds: the front matter and, below it, the description the
/// agent is given.
fn render(spec: &Spec, description: &str) -> Result<String> {
    Ok(format!(
        "---\n{}---\n\n{}\n",
        serde_yaml::to_string(spec)?,
        description.trim_end()
    ))
}

fn parse(buffer: &str) -> Result<(Spec, String)> {
    let buffer = buffer.strip_prefix("---\n").unwrap_or(buffer);
    let (front, body) = split_at_divider(buffer).ok_or(AgentError::BufferLostFront)?;
    let spec = serde_yaml::from_str(front).map_err(AgentError::BufferFront)?;
    Ok((spec, body.trim().to_string()))
}

pub fn solve(app: &App, req: &SolveRequest) -> Result<()> {
    let (project, task, dir) = locate(app, req.task)?;
    let name = task_display(&project, &task);
    let mut spec = Spec {
        agent: req.agent.unwrap_or_default(),
        model: req.model.unwrap_or("haiku").to_string(),
        effort: req.effort.unwrap_or("medium").to_string(),
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
        req.kind,
        &spec,
        &description,
        &Launch {
            wait: req.wait,
            extra: req.extra,
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
            crate::agent::prompt::shell_word(&name)
        );
    }
    Ok(())
}

pub fn runs(app: &App, task: Option<&str>, log: Option<u32>) -> Result<()> {
    let (project, task, dir) = locate(app, task)?;
    if let Some(n) = log {
        let text = std::fs::read_to_string(dir.run_log_path(n))
            .map_err(|_| AgentError::RunLogMissing(n))?;
        print!("{text}");
        return Ok(());
    }
    let runs = dir.runs();
    if runs.is_empty() {
        println!(
            "No runs of {} yet: `iter task solve {}`.",
            task_display(&project, &task),
            crate::agent::prompt::shell_word(&task_display(&project, &task))
        );
    }
    for r in runs {
        println!(
            "#{:<3} {:<8} {:<11} {:<6} {}{}",
            r.n,
            r.shown_status(),
            r.kind.key(),
            r.model,
            r.started,
            r.ended.map(|e| format!(" -> {e}")).unwrap_or_default()
        );
    }
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
