//! Instructions in the browser: the list of them (`/instructions`, global,
//! and the Instructions tab of a project, which adds what the project and its
//! organization keep), and the page of one (`/instruction/{name}`) with its
//! Info and Agents tabs. The files and their format are
//! `agent::instruction`'s; this reads them, writes the ones the page edits,
//! and finds the agents that used them in the runs of the tasks.
//!
//! A page is about the global instructions, or, with `?project=<id>`, about
//! those that apply to a project: saving writes the project's own file, and
//! "use the inherited one" removes it.

use super::crumbs::trail;
use super::open_db;
use super::sections::TabQuery;
use super::task_runs::{label_of, state_pill};
use super::ui::breadcrumb::Crumb;
use super::ui::field::{field, select, textarea};
use super::ui::form::form_shell;
use super::ui::frame::frame;
use super::ui::layout::info_columns;
use super::ui::modal::modal;
use super::ui::page_header::{Kicker, page_header};
use super::ui::panel::{Row, panel};
use super::ui::prose::description;
use super::ui::table::{AddRow, cell, data_table, row};
use super::ui::tabs::{Tab, tabs};
use super::url::{instruction_url, instructions_url, page_url, project_url, run_url, task_url};
use crate::agent::instruction::{self, Instruction, Layers, Source};
use crate::agent::prompt::{Kind, PLACEHOLDERS};
use crate::agent::run::Harness;
use crate::agent::run::RunRecord;
use crate::agent::taskdir;
use crate::config::config;
use crate::db::{Db, Table};
use crate::models::{Organization, Project, Task};
use crate::utils::resolve::task_display;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        content::Form,
        error::{RouterErrorExt, bad_request, see_other},
        page, path_param, query_params,
        response::Response,
        route,
    },
    view::{Child, View, component, view},
};

mod list;
mod page;
mod save;
#[cfg(test)]
mod tests;

pub use list::*;
use page::*;
pub use save::*;

path_param!(name: String, error = bad_request);

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder
        .page(list_page)
        .page(detail_page)
        .route(create_instruction)
        .route(save_instruction)
}

/// Which instructions a page is about, and where it writes.
pub struct Scope {
    pub project: Option<Project>,
    pub org: Option<Organization>,
    pub layers: Layers,
    /// The folder a save goes to: the project's notes, or iter's config
    /// folder. None when there is nowhere to write.
    pub home: Option<PathBuf>,
}

impl Scope {
    /// The instructions of iter's config folder alone.
    pub fn global() -> Scope {
        let home = instruction::global();
        Scope {
            project: None,
            org: None,
            layers: home.iter().map(|d| (Source::Global, d.clone())).collect(),
            home,
        }
    }

    /// The instructions that apply to project `id`.
    pub fn of_project(db: &Db, id: i64) -> Result<Scope> {
        let project = db.get::<Project>(id)?.ok_or_not_found()?;
        let org = match project.organization_id {
            Some(o) => db.get::<Organization>(o)?,
            None => None,
        };
        Ok(Scope {
            layers: instruction::layers_of(db, &project),
            home: crate::notes::project_dir(&project, &org),
            project: Some(project),
            org,
        })
    }

    /// The scope a request's `?project=` asks for.
    fn asked(db: &Db, project: Option<&str>) -> Result<Scope> {
        match project.filter(|p| !p.is_empty()) {
            Some(p) => {
                let id: i64 = p.parse().map_err(|_| bad_request("not a project"))?;
                Scope::of_project(db, id)
            }
            None => Ok(Scope::global()),
        }
    }

    fn project_id(&self) -> Option<i64> {
        self.project.as_ref().map(|p| p.id())
    }

    /// The source whose file this page edits.
    fn own(&self) -> Source {
        if self.project.is_some() {
            Source::Project
        } else {
            Source::Global
        }
    }

    fn url(&self, name: &str) -> String {
        instruction_url(name, self.project_id())
    }
}

/// One use of an instruction: the run, and the task it was on.
pub struct Use {
    pub task_id: i64,
    pub task: String,
    pub run: RunRecord,
}

/// Every run that was given an instruction, in the tasks of `only` (a
/// project) or of all projects, the latest first.
pub fn usage(db: &Db, only: Option<i64>) -> Vec<Use> {
    let mut projects: HashMap<i64, Option<Project>> = HashMap::new();
    let mut uses = Vec::new();
    for task in db.list::<Task>().unwrap_or_default() {
        if only.is_some_and(|p| p != task.project_id) {
            continue;
        }
        let project = projects
            .entry(task.project_id)
            .or_insert_with(|| db.get::<Project>(task.project_id).ok().flatten());
        let Some(project) = project else { continue };
        let Ok(dir) = taskdir::of(db, project, &task) else {
            continue;
        };
        let label = task_display(project, &task);
        for run in dir.runs() {
            uses.push(Use {
                task_id: task.id(),
                task: label.clone(),
                run,
            });
        }
    }
    uses.sort_by(|a, b| b.run.started.cmp(&a.run.started));
    uses
}

/// A row of the list of instructions.
pub struct ListRow {
    pub name: String,
    pub href: String,
    pub description: String,
    pub used: usize,
}

pub fn rows(scope: &Scope, uses: &[Use]) -> Vec<ListRow> {
    instruction::list(&scope.layers)
        .into_iter()
        .map(|i| {
            let mine: Vec<&Use> = uses
                .iter()
                .filter(|u| u.run.instruction_name() == i.name)
                .collect();
            ListRow {
                href: scope.url(&i.name),
                description: i.front.description.clone(),
                used: mine.len(),
                name: i.name,
            }
        })
        .collect()
}
