//! Instructions in the browser: the list (`/instructions`) and the page of one (`/instruction/{name}`).

use super::crumbs::trail;
use super::fresh_db;
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
use super::url::{agent_url, instruction_url, instructions_url, page_url, project_url, task_url};
use iter_core::agent::instruction::{self, Instruction, Layers, Source};
use iter_core::agent::prompt::PLACEHOLDERS;
use iter_core::agent::run::Harness;
use iter_core::agent::run::RunRecord;
use iter_core::agent::taskdir;
use iter_core::config::config;
use iter_core::db::{Db, Table};
use iter_core::models::{Organization, Project, Task};
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

pub struct Scope {
    pub project: Option<Project>,
    pub org: Option<Organization>,
    pub layers: Layers,
    pub home: Option<PathBuf>,
}

impl Scope {
    pub fn global() -> Scope {
        let home = instruction::global();
        Scope {
            project: None,
            org: None,
            layers: home.iter().map(|d| (Source::Global, d.clone())).collect(),
            home,
        }
    }

    pub fn of_project(db: &Db, id: i64) -> Result<Scope> {
        let project = db.get::<Project>(id)?.ok_or_not_found()?;
        let org = db.org_of(&project)?;
        Ok(Scope {
            layers: instruction::layers_of(db, &project)?,
            home: iter_core::notes::project_dir(&project, &org),
            project: Some(project),
            org,
        })
    }

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

pub struct Use {
    pub task_id: i64,
    pub task: String,
    pub run: RunRecord,
}

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
        let label = task.label(project).to_string();
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
                .filter(|u| u.run.instruction == i.name)
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
