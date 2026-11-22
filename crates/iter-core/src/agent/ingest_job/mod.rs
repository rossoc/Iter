//! Agents writing the wiki and the review of what they wrote: lock, detached ingest, git-based review, bridge, automatic trigger.
//! Shared by the CLI and the web; nothing prints. The agent's changes stay uncommitted in the notes' git repository: accepting is a commit.
//! See `doc/architecture_agent_wiki.md`.

use crate::agent::exec;
use crate::agent::run::{Harness, Invocation, alive};
use crate::app::App;
use crate::db::{Db, Table};
use crate::error::AgentError;
use crate::error::Result;
use crate::models::{Project, Task};
use crate::process;
use crate::text::now_as;
use crate::wiki::graph::Wiki;
use crate::wiki::ingest::{self, Mode};
use crate::wiki::scope::{self, Spec};
use std::path::PathBuf;

mod bridge;
mod lock;
mod review;
mod spawn;
#[cfg(test)]
mod tests;

pub use bridge::bridge_task_notes;
pub use lock::busy;
use lock::lock;
pub use review::*;
pub use spawn::ingest_args;
pub use spawn::ingest_in_background;
pub use spawn::{run_agent, spawn_ingest};

#[derive(Debug, Default)]
pub struct HarnessChoice {
    pub agent: Option<Harness>,
    pub model: Option<String>,
    pub extra: Vec<String>,
}

pub struct Target {
    pub project: Project,
    pub wiki: Wiki,
    pub notes_dir: PathBuf,
}

impl Target {
    pub fn new(db: &Db, project: Project) -> Result<Target> {
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

    pub fn scope(&self) -> String {
        format!(
            "--project {}",
            crate::process::shell_word(&self.project.name)
        )
    }

    pub fn in_git(&self) -> bool {
        process::succeeds(
            "git",
            Some(&self.notes_dir.display().to_string()),
            &["rev-parse", "--is-inside-work-tree"],
        )
    }

    pub fn require_git(&self) -> Result<()> {
        match self.in_git() {
            true => Ok(()),
            false => Err(AgentError::NotesNotInGit(self.notes_dir.clone()).into()),
        }
    }
}

/// The text a [`ingest::Job`] borrows, rendered once per run.
pub struct JobText {
    scope: String,
    stamp: String,
    schema: String,
}

impl JobText {
    pub fn of(target: &Target) -> Self {
        JobText {
            scope: target.scope(),
            stamp: now(),
            schema: crate::wiki::schema::render(&target.wiki.ontology),
        }
    }

    pub fn job<'a>(&'a self, target: &'a Target) -> ingest::Job<'a> {
        ingest::Job {
            project: &target.project.name,
            notes_dir: &target.notes_dir,
            scope: &self.scope,
            now: &self.stamp,
            schema: &self.schema,
        }
    }
}

/// The wiki ingest text for `task`: the notes waiting, the task's sources, the rules. `None` when
/// the project has no wiki; while another ingest is writing the wiki, a text that says so.
pub fn task_ingest_text(db: &Db, project: &Project, task: &Task) -> Option<String> {
    let target = Target::new(db, project.clone()).ok()?;
    if busy(&target) {
        return Some(format!(
            "(an ingest is already writing the wiki of {}: do nothing, say so and stop)",
            project.name
        ));
    }
    let waiting = ingest::waiting(&target.wiki);
    let text = JobText::of(&target);
    Some(ingest::ingest_prompt(
        &text.job(&target),
        &waiting,
        Some(&task.id().to_string()),
    ))
}

pub fn now() -> String {
    now_as("%Y-%m-%dT%H:%M:%S")
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    TaskStart,
    TaskDone,
}

pub enum Auto {
    Started,
    Idle,
    Failed(String),
}

/// Does what the project's `ingest` setting asks for at `trigger`; never fails the caller: problems are [`Auto::Failed`].
pub fn auto(app: &App, project: &Project, task: &Task, trigger: Trigger) -> Auto {
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
        let none = HarnessChoice::default();
        spawn_ingest(project, sketches.then_some(task), &none)?;
        Ok(true)
    })();
    match outcome {
        Ok(true) => Auto::Started,
        Ok(false) => Auto::Idle,
        Err(e) => Auto::Failed(e.to_string()),
    }
}
