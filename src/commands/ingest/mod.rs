//! `iter wiki ingest`, `build-concept` and `pending`: agents writing the
//! wiki, and the review of what they wrote. The reading and the prompts are
//! `wiki::ingest`'s; the harness is `agent::run`'s. What the agent changes is
//! left uncommitted in the notes' git repository: `pending` shows it, and
//! accepting is a commit.

use crate::agent::exec;
use crate::agent::prompt::shell_word;
use crate::agent::run::{Invocation, alive};
use crate::app::App;
use crate::args::HarnessOpts;
use crate::db::Db;
use crate::agent::error::AgentError;
use crate::error::{IterError, Result};
use crate::models::{Project, Task};
use crate::process;
use crate::utils::resolve::{resolve_project_or_current, resolve_task, task_display};
use crate::wiki::graph::Wiki;
use crate::wiki::ingest::{self, Job, Mode};
use crate::wiki::schema;
use crate::wiki::scope::{self, Spec};

use std::path::PathBuf;


/// A project's wiki, and where it is kept.
struct Target {
    project: Project,
    wiki: Wiki,
    notes_dir: PathBuf,
}

impl Target {
    fn new(db: &Db, project: Project) -> Result<Target> {
        let spec = Spec {
            project: Some(project.name.clone()),
            ..Spec::default()
        };
        let wiki = scope::wiki(db, &spec)?;
        let notes_dir = wiki.levels[0].dir.clone();
        Ok(Target {
            project,
            wiki,
            notes_dir,
        })
    }

    fn scope(&self) -> String {
        format!("--project {}", shell_word(&self.project.name))
    }

    fn in_git(&self) -> bool {
        process::succeeds(
            "git",
            Some(&self.notes_dir.display().to_string()),
            &["rev-parse", "--is-inside-work-tree"],
        )
    }

    fn require_git(&self) -> Result<()> {
        match self.in_git() {
            true => Ok(()),
            false => Err(AgentError::NotesNotInGit(self.notes_dir.clone()).into()),
        }
    }
}

fn now() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn project_of(
    app: &App,
    project: Option<&str>,
    task: Option<&str>,
) -> Result<(Project, Option<Task>)> {
    if let Some(reference) = task {
        let (project, task) = resolve_task(&app.db, reference)?;
        return Ok((project, Some(task)));
    }
    let project = match project {
        Some(name) => app.db.resolve::<Project>(name)?,
        None => resolve_project_or_current(&app.db, None)?,
    };
    Ok((project, None))
}

mod bridge;
mod lock;
mod review;
mod spawn;
#[cfg(test)]
mod tests;

pub use bridge::bridge_task_notes;
pub use review::*;
#[cfg(feature = "web")]
pub use spawn::ingest_in_background;
use lock::{busy, lock};
use spawn::{run_agent, spawn_ingest};


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
        spawn_ingest(&target.project, task.as_ref(), opts)?;
        println!(
            "ingesting {} notes of {} in the background; review with `iter wiki pending {}`",
            waiting.len(),
            target.project.name,
            target.scope()
        );
        return Ok(());
    }
    let schema_text = schema::render(&target.wiki.ontology);
    let scope = target.scope();
    let stamp = now();
    let job = Job {
        project: &target.project.name,
        notes_dir: &target.notes_dir,
        scope: &scope,
        now: &stamp,
        schema: &schema_text,
    };
    let prompt = ingest::ingest_prompt(&job, &waiting, sketches.as_deref());
    println!(
        "ingesting {} notes of {}...",
        waiting.len(),
        target.project.name
    );
    finish(&target, run_agent(&target, opts, &prompt, "ingest")?)
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
        if target.wiki.node(0, idea).is_none() {
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
    let schema_text = schema::render(&target.wiki.ontology);
    let scope = target.scope();
    let stamp = now();
    let job = Job {
        project: &target.project.name,
        notes_dir: &target.notes_dir,
        scope: &scope,
        now: &stamp,
        schema: &schema_text,
    };
    let prompt = ingest::concept_prompt(&job, name, ideas);
    println!("building the concept {name} from {} ideas...", ideas.len());
    finish(&target, run_agent(&target, opts, &prompt, "concept")?)
}


pub fn pending(app: &App, project: Option<&str>, accept: bool, revert: bool) -> Result<()> {
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
        settle(&app.db, &project, accept)?;
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

/// When a task starts or finishes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    TaskStart,
    TaskDone,
}

/// Does what the project's `ingest` setting asks for at `trigger`: starts an
/// ingest in the background. It never fails the command it is called from:
/// what goes wrong is a warning.
pub fn auto(app: &App, project: &Project, task: &Task, trigger: Trigger) {
    let outcome = (|| -> Result<bool> {
        let Ok(target) = Target::new(&app.db, project.clone()) else {
            return Ok(false);
        };
        let wanted = match ingest::mode(&target.wiki.levels).0 {
            Mode::OnTaskStart => trigger == Trigger::TaskStart,
            Mode::OnTaskDone => trigger == Trigger::TaskDone,
            Mode::Manual => false,
        };
        if !wanted || busy(&target) || !target.in_git() {
            return Ok(false);
        }
        let sketches = trigger == Trigger::TaskDone;
        if ingest::waiting(&target.wiki).is_empty() && !sketches {
            return Ok(false);
        }
        let none = HarnessOpts::default();
        spawn_ingest(project, sketches.then_some(task), &none)?;
        Ok(true)
    })();
    match outcome {
        Ok(true) => println!(
            "ingesting the notes of {} in the background (`iter wiki pending` to review)",
            project.name
        ),
        Ok(false) => {}
        Err(e) => eprintln!("warning: could not start the ingest: {e}"),
    }
}
