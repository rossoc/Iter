//! `iter wiki ingest`, `build-concept` and `pending`: agents writing the wiki and the review of what they wrote (`wiki::ingest_job`).

use crate::app::App;
use crate::args::HarnessOpts;
use crate::db::Table;
use crate::error::AgentError;
use crate::error::{IterError, Result};
use crate::locate::{self, project_or_current};
use crate::models::{Project, Task};
use crate::wiki::ingest;
use crate::wiki::ingest_job::{
    Auto, HarnessChoice, JobText, Target, Trigger, auto as wiki_auto, busy, changes, run_agent,
    settle_with, spawn_ingest,
};
use std::path::PathBuf;

fn choice(opts: &HarnessOpts) -> HarnessChoice {
    HarnessChoice {
        agent: opts.agent,
        model: opts.model.clone(),
        extra: opts.extra.clone(),
    }
}

fn project_of(
    app: &App,
    project: Option<&str>,
    task: Option<&str>,
) -> Result<(Project, Option<Task>)> {
    if let Some(reference) = task {
        let (project, task) = locate::task(&app.db, &reference.parse()?)?;
        return Ok((project, Some(task)));
    }
    let project = match project {
        Some(name) => app.db.resolve::<Project>(name)?,
        None => project_or_current(&app.db, None)?,
    };
    Ok((project, None))
}

pub fn ingest(
    app: &App,
    project: Option<&str>,
    task: Option<&str>,
    dry_run: bool,
    detach: bool,
    opts: &HarnessOpts,
) -> Result<()> {
    let (project, task) = project_of(app, project, task)?;
    let target = Target::new(&app.db, project)?;
    let waiting = ingest::waiting(&target.wiki);
    let sketches = task
        .as_ref()
        .and_then(|t| crate::agent::taskdir::of(&app.db, &target.project, t).ok())
        .map(|d| d.sketches_dir())
        .filter(|d| d.is_dir() && std::fs::read_dir(d).is_ok_and(|mut e| e.next().is_some()));
    let (mode, problems) = ingest::mode(&target.wiki.levels);
    for problem in &problems {
        eprintln!("warning: {problem}");
    }
    if dry_run {
        println!("{}: ingest {}", target.project.name, mode.key());
        for w in &waiting {
            println!("  {} ({:?})", w.id, w.why);
        }
        if let Some(dir) = &sketches {
            println!("  sketches in {}", dir.display());
        }
        println!("{} notes waiting", waiting.len());
        return Ok(());
    }
    if waiting.is_empty() && sketches.is_none() {
        println!("nothing to ingest: every note has pages made after it last changed");
        return Ok(());
    }
    target.require_git()?;
    if busy(&target) {
        println!("an ingest is already running for {}", target.project.name);
        return Ok(());
    }
    if detach {
        spawn_ingest(&target.project, task.as_ref(), &choice(opts))?;
        println!(
            "ingesting {} notes of {} in the background; review with `iter wiki pending {}`",
            waiting.len(),
            target.project.name,
            target.scope()
        );
        return Ok(());
    }
    let text = JobText::of(&target);
    let name = task
        .as_ref()
        .filter(|_| sketches.is_some())
        .map(|t| t.id().to_string());
    let prompt = ingest::ingest_prompt(&text.job(&target), &waiting, name.as_deref());
    println!(
        "ingesting {} notes of {}...",
        waiting.len(),
        target.project.name
    );
    finish(
        &target,
        run_agent(&target, &choice(opts), &prompt, "ingest")?,
    )
}

fn finish(target: &Target, (log, ok): (PathBuf, bool)) -> Result<()> {
    if !ok {
        return Err(AgentError::AgentFailed(log).into());
    }
    println!(
        "done (log: {}). Review with `iter wiki pending {}`",
        log.display(),
        target.scope()
    );
    Ok(())
}

pub fn build_concept(
    app: &App,
    project: Option<&str>,
    name: &str,
    ideas: &[String],
    opts: &HarnessOpts,
) -> Result<()> {
    let (project, _) = project_of(app, project, None)?;
    let target = Target::new(&app.db, project)?;
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
        return Err(AgentError::ConceptName.into());
    }
    for idea in ideas {
        if target.wiki.resolve(0, idea).is_none() {
            return Err(IterError::NotFound {
                kind: "wiki page",
                name: idea.clone(),
            });
        }
    }
    target.require_git()?;
    if busy(&target) {
        return Err(AgentError::WikiBusy.into());
    }
    let text = JobText::of(&target);
    let prompt = ingest::concept_prompt(&text.job(&target), name, ideas);
    println!("building the concept {name} from {} ideas...", ideas.len());
    finish(
        &target,
        run_agent(&target, &choice(opts), &prompt, "concept")?,
    )
}

pub fn pending(
    app: &App,
    project: Option<&str>,
    accept: bool,
    revert: bool,
    message: Option<&str>,
) -> Result<()> {
    let (project, _) = project_of(app, project, None)?;
    let target = Target::new(&app.db, project.clone())?;
    target.require_git()?;
    let list = changes(&target)?;
    if list.is_empty() {
        println!("nothing pending in the wiki of {}", project.name);
        return Ok(());
    }
    let summary: String = list
        .iter()
        .map(|c| format!("{:<8}{}\n", c.kind, c.path))
        .collect();
    if accept || revert {
        settle_with(&app.db, &project, accept, message)?;
        let verb = if accept { "committed" } else { "thrown away" };
        println!("{verb}:\n{summary}");
    } else {
        print!("{summary}");
        for c in &list {
            print!("\n{}", c.diff);
        }
        println!(
            "\naccept with `iter wiki pending {0} --accept`, throw away with `--revert`",
            target.scope()
        );
    }
    Ok(())
}

/// Does what the project's `ingest` setting asks for at `trigger` and says so; problems are warnings, never a failure.
pub fn auto(app: &App, project: &Project, task: &Task, trigger: Trigger) {
    say_auto(project, &wiki_auto(app, project, task, trigger));
}

pub fn say_auto(project: &Project, outcome: &Auto) {
    match outcome {
        Auto::Started => println!(
            "ingesting the notes of {} in the background (`iter wiki pending` to review)",
            project.name
        ),
        Auto::Idle => {}
        Auto::Failed(e) => eprintln!("warning: could not start the ingest: {e}"),
    }
}
