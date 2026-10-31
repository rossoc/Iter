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
use super::ui::button::add_link;
use super::ui::empty_line::empty_line;
use super::ui::field::{field, select, textarea};
use super::ui::form::form_shell;
use super::ui::frame::frame;
use super::ui::layout::info_columns;
use super::ui::modal::modal;
use super::ui::page_header::{Kicker, page_header};
use super::ui::panel::{Row, panel};
use super::ui::prose::description;
use super::ui::table::{cell, col_head};
use super::ui::tabs::{Tab, tabs};
use super::url::{instruction_url, instructions_url, page_url, project_url, run_url, task_url};
use crate::agent::instruction::{self, Instruction, Layers, Source};
use crate::agent::prompt::{Kind, PLACEHOLDERS};
use crate::agent::run::Harness;
use crate::agent::run::Run;
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
    pub run: Run,
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

/// The table of instructions, in the look of the tasks table: the first row
/// (`add`) writes a new one.
#[component]
pub async fn instruction_table(
    rows: &[ListRow],
    #[default] add: Option<String>,
) -> Result<impl View> {
    Ok(view! {
        <table class="tasks instructions" role="table">
            <caption class="sr">"Instructions"</caption>
            <thead role="rowgroup">
                <tr role="row">
                    col_head("Instruction")
                    col_head("Used")
                </tr>
            </thead>
            <tbody role="rowgroup">
                if let Some(href) = &add {
                    <tr class="add" role="row">
                        cell(colspan: "2", add_link(href: href, label: "New instruction\u{2026}"))
                    </tr>
                }
                for r in rows.iter() {
                    <tr role="row">
                        cell(
                            <a class="u" href=(r.href.as_str())>(label_of(&r.name))</a>
                            // its space is kept when there is nothing to say, so rows do not jump
                            <span class="sub">(if r.description.is_empty() { "\u{a0}" } else { r.description.as_str() })</span>
                        )
                        cell(class: "mono", (r.used.to_string()))
                    </tr>
                }
            </tbody>
        </table>
    })
}

/// The placeholders an instruction's prompt may use, as the forms say them.
fn placeholder_hint() -> String {
    format!(
        "Filled in for the task when an agent starts: {}.",
        PLACEHOLDERS
            .iter()
            .map(|p| format!("{{{{{p}}}}}"))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

/// The pop-up of a new instruction (`?new=instruction`): a name, what it is
/// for, and the prompt, which starts as the `develop` one. It is made a
/// `develop`; Settings changes that.
#[component]
pub async fn new_instruction_modal(scope: &Scope) -> Result<impl View> {
    let project = scope
        .project_id()
        .map(|p| p.to_string())
        .unwrap_or_default();
    let close = instructions_target(scope);
    let starter = instruction::starter_prompt();
    let hint = placeholder_hint();
    Ok(view! {
        modal(id: "new-instruction-title", title: "New instruction", close: close.as_str(),
            <div class="new-instruction">
                form_shell(action: "/instructions", error: None, cancel: close.as_str(), submit_label: "Create",
                    <input type="hidden" name="project" value=(project.as_str())>
                    field(name: "name", label: "Name", value: "", identifier: true, required: true, autofocus: true,
                        hint: "Lowercase letters, digits and dashes: it is the file name.")
                    field(name: "description", label: "Description", value: "",
                        hint: "One line: what the instruction is for.")
                    textarea(name: "prompt", label: "Prompt", value: &starter, hint: &hint)
                )
            </div>
        )
    })
}

/// The page that lists the instructions of `scope`.
fn instructions_target(scope: &Scope) -> String {
    match &scope.project {
        Some(p) => super::url::instructions_tab_url(&project_url(p.id())),
        None => instructions_url(),
    }
}

#[page("/instructions")]
async fn list_page(cx: &Cx) -> Result<impl View> {
    let query = query_params::<TabQuery>(cx)?;
    let db = open_db()?;
    let scope = Scope::global();
    let rows = rows(&scope, &usage(&db, None));
    let adding = query.new.as_deref() == Some("instruction");
    Ok(view! {
        list_screen(scope: &scope, rows: &rows, adding: adding)
    })
}

#[component]
async fn list_screen(scope: &Scope, rows: &[ListRow], adding: bool) -> Result<impl View> {
    let add = page_url(&instructions_url(), &[("new", Some("instruction"))]);
    let dialog = adding.then(|| Child::new(view! { new_instruction_modal(scope: scope) }));
    Ok(view! {
        frame(title: &["Instructions"], current: "/instructions", dialog: dialog,
            page_header(title: "Instructions", kicker: Kicker::Eyebrow("Global"))
            <p class="prompts-intro">
                "What an agent is told for a kind of work, in the format of an opencode agent: front matter over the prompt. "
                "These are the ones every project shares; a project or its organization can keep its own of the same name."
            </p>
            instruction_table(rows: rows, add: Some(add))
        )
    })
}

/// What the Instructions tab of a project shows.
pub struct InstructionsTab {
    pub scope: Scope,
    pub rows: Vec<ListRow>,
}

impl InstructionsTab {
    pub fn load(db: &Db, project_id: i64) -> Result<InstructionsTab> {
        let scope = Scope::of_project(db, project_id)?;
        let rows = rows(&scope, &usage(db, Some(project_id)));
        Ok(InstructionsTab { scope, rows })
    }
}

/// The tab's body: the list, its first row writing a new instruction.
#[component]
pub async fn instructions_tab(tab: &InstructionsTab) -> Result<impl View> {
    let add = page_url(
        &super::url::instructions_tab_url(&project_url(tab.scope.project_id().unwrap_or(0))),
        &[("new", Some("instruction"))],
    );
    Ok(view! {
        <p class="prompts-intro">
            "A project's own instruction wins; without one, its organization's, then the one in iter's config folder, then the one built in."
        </p>
        instruction_table(rows: &tab.rows, add: Some(add))
    })
}

/// The tabs of an instruction's page.
#[derive(Clone, Copy, PartialEq)]
enum Part {
    Info,
    Agents,
    /// The edit form, as a tab of its own, always the last.
    Settings,
}

/// The Settings tab: the instruction field by field, over the file.
#[component]
async fn edit_form(scope: &Scope, i: &Instruction) -> Result<impl View> {
    let project = scope
        .project_id()
        .map(|p| p.to_string())
        .unwrap_or_default();
    let own = i.source == scope.own();
    let builtin = instruction::is_builtin(&i.name);
    let close = scope.url(&i.name);
    let action = format!("/instruction/{}", i.name);
    let note = if scope.home.is_none() {
        "There is nowhere to keep this: the project and its organization have no notes folder."
    } else if own {
        ""
    } else {
        "Saving keeps a copy of its own, here; the one it comes from stays as it is."
    };
    let kinds: Vec<(String, String)> = [
        (
            Kind::Develop,
            "Develop: works on the code, following the plan",
        ),
        (Kind::Test, "Test: works on the tests"),
        (Kind::Fix, "Fix: works on the code, to fix a defect"),
        (Kind::Check, "Check: reads and runs checks, changes no code"),
        (Kind::Plan, "Plan: reads, writes the plan"),
    ]
    .into_iter()
    .map(|(k, text)| (k.key().to_string(), text.to_string()))
    .collect();
    let kind = i.kind().key();
    let models: Vec<String> = Harness::Claude.models(&config().agent.models);
    let hint = placeholder_hint();
    let prompt = i.body.trim_matches('\n').to_string();
    let model = i.front.model.clone().unwrap_or_default();
    let mode = i.front.mode.clone().unwrap_or_default();
    let temperature = i.front.temperature.clone().unwrap_or_default();
    let permission = i.front.permission.clone().unwrap_or_default();
    Ok(view! {
        <section class="panel compose" aria-labelledby="edit-title">
            <h2 id="edit-title" class="label">"Settings"</h2>
            <form method="post" action=(action.as_str())>
                <input type="hidden" name="project" value=(project.as_str())>
                <div class="compose-pick">
                    if !builtin {
                        field(name: "new_name", label: "Name", value: &i.name, identifier: true,
                            hint: "Lowercase letters, digits and dashes: it is the file name. Agents already run keep the old name.")
                        select(name: "kind", label: "Set up like", options: &kinds, current: kind)
                    }
                </div>
                field(name: "description", label: "Description", value: &i.front.description,
                    hint: "One line: what the instruction is for.")
                <div class="compose-pick">
                    field(name: "model", label: "Model", value: &model, identifier: true, list: "model-list",
                        hint: "Optional: the model agents start with.")
                    field(name: "mode", label: "Mode", value: &mode, identifier: true,
                        hint: "Optional: primary, subagent or all.")
                    field(name: "temperature", label: "Temperature", value: &temperature, identifier: true,
                        hint: "Optional: a number.")
                </div>
                <datalist id="model-list">
                    for m in models.iter() {
                        <option value=(m.as_str())></option>
                    }
                </datalist>
                textarea(name: "permission", label: "Permission", value: &permission,
                    hint: "Optional, YAML: edit: deny, bash: deny.")
                textarea(name: "prompt", label: "Prompt", value: &prompt, hint: &hint)
                if !note.is_empty() {
                    <p class="empty-line">(note)</p>
                }
                <div class="compose-actions">
                    if scope.home.is_some() {
                        <button class="button primary" type="submit" name="do" value="save">"Save"</button>
                    }
                    if own && i.path.is_some() {
                        <button class="button" type="submit" name="do" value="reset">"Use the inherited one"</button>
                    }
                    <a class="button" href=(close.as_str())>"Back"</a>
                </div>
            </form>
        </section>
    })
}

#[component]
async fn info_part(i: &Instruction) -> Result<impl View> {
    let edited = i
        .path
        .as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok())
        .map(|t| {
            chrono::DateTime::<chrono::Local>::from(t)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_default();
    let rows = [
        Row::Text("From", i.source.label().to_string()),
        Row::Text("Set up like", i.kind().key().to_string()),
        Row::Text("Model", i.front.model.clone().unwrap_or_default()),
        Row::Text("Mode", i.front.mode.clone().unwrap_or_default()),
        Row::Text(
            "Temperature",
            i.front.temperature.clone().unwrap_or_default(),
        ),
        Row::Text("Permission", i.front.permission.clone().unwrap_or_default()),
        Row::Text(
            "File",
            i.path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        ),
        Row::Text("Edited", edited),
    ];
    Ok(view! {
        info_columns(
            <div>
                if let Some(problem) = &i.problem {
                    <p class="empty-line">"The front matter cannot be read: " (problem.as_str())</p>
                }
                description(text: &i.front.description)
                <h2 class="label prompt-head">"Prompt"</h2>
                description(text: &i.body)
            </div>
            panel(id: "details-title", title: "Details", rows: &rows)
        )
    })
}

/// The agents that were given the instruction: the run, the task, how it
/// ended (the start time is stored with the run, not shown).
#[component]
async fn agents_part(uses: &[Use]) -> Result<impl View> {
    Ok(view! {
        if uses.is_empty() {
            empty_line("No agent has been given this instruction yet.")
        } else {
            <table class="tasks agents" role="table">
                <caption class="sr">"Agents given this instruction, the latest first"</caption>
                <thead role="rowgroup">
                    <tr role="row">
                        col_head("Agent")
                        col_head("Task")
                        col_head("Status")
                    </tr>
                </thead>
                <tbody role="rowgroup">
                    for u in uses.iter() {
                        <tr role="row">
                            cell(
                                <a class="u" href=(run_url(u.task_id, u.run.n))>"#" (u.run.n.to_string())</a>
                                <span class="hint">(u.run.model.as_str()) ", " (u.run.effort.as_str())</span>
                            )
                            cell(<a class="u muted" href=(task_url(u.task_id))>(u.task.as_str())</a>)
                            cell(state_pill(run: &u.run))
                        </tr>
                    }
                </tbody>
            </table>
        }
    })
}

#[page("/instruction/{name}")]
async fn detail_page(cx: &Cx) -> Result<impl View> {
    let name = path_param::<Name>(cx)?.clone();
    let query = query_params::<TabQuery>(cx)?;
    let db = open_db()?;
    let scope = Scope::asked(&db, query.project.as_deref())?;
    let i = instruction::find(&name, &scope.layers).ok_or_not_found()?;
    // `?edit=1` is the old link to the form: it is the Settings tab now
    let part = match query.tab.as_deref() {
        Some("agents") => Part::Agents,
        Some("settings") => Part::Settings,
        _ if query.edit.is_some() => Part::Settings,
        _ => Part::Info,
    };
    let uses: Vec<Use> = usage(&db, scope.project_id())
        .into_iter()
        .filter(|u| u.run.instruction_name() == name)
        .collect();
    Ok(view! {
        detail_screen(scope: &scope, i: &i, part: part, uses: &uses)
    })
}

#[component]
async fn detail_screen(
    scope: &Scope,
    i: &Instruction,
    part: Part,
    uses: &[Use],
) -> Result<impl View> {
    let name = i.name.as_str();
    let base = scope.url(name);
    let items = [
        Tab::link("Info", base.clone()).marked(part == Part::Info),
        Tab::link("Agents", page_url(&base, &[("tab", Some("agents"))]))
            .count(uses.len())
            .marked(part == Part::Agents),
        Tab::link("Settings", page_url(&base, &[("tab", Some("settings"))]))
            .marked(part == Part::Settings),
    ];
    let heading = label_of(name);
    let crumbs = match &scope.project {
        Some(p) => {
            let mut c = trail(
                &scope.org,
                Some(p),
                Crumb::link("Instructions", instructions_target(scope)),
            );
            c.push(Crumb::label(heading.as_str()));
            c
        }
        None => vec![
            Crumb::link("Instructions", instructions_url()),
            Crumb::label(heading.as_str()),
        ],
    };
    let parts = [name, "Instruction"];
    let current = if scope.project.is_none() {
        "/instructions"
    } else {
        ""
    };
    Ok(view! {
        frame(title: &parts, current: current,
            page_header(title: &heading, kicker: Kicker::Crumbs(&crumbs))
            tabs(label: "Sections", items: &items)
            if part == Part::Agents {
                agents_part(uses: uses)
            } else if part == Part::Settings {
                edit_form(scope: scope, i: i)
            } else {
                info_part(i: i)
            }
        )
    })
}

#[derive(Deserialize)]
struct NewForm {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    prompt: String,
    #[serde(default)]
    project: String,
}

/// `POST /instructions`: writes a new instruction (a `develop` one, with the
/// description and the prompt asked for) and opens its Settings tab.
#[route(POST "/instructions")]
async fn create_instruction(Form(form): Form<NewForm>) -> Result<Response> {
    let db = open_db()?;
    let scope = Scope::asked(&db, Some(&form.project))?;
    let name = form.name.trim().to_string();
    if !instruction::valid_name(&name) {
        return Err(bad_request(format!(
            "'{name}' is not a name: lowercase letters, digits and dashes"
        ))
        .into());
    }
    let home = scope
        .home
        .clone()
        .ok_or_else(|| bad_request("there is no folder to keep it in"))?;
    let path = instruction::file(&home, &name);
    if path.exists() {
        return Err(bad_request(format!("'{name}' exists already")).into());
    }
    let text = instruction::compose(
        None,
        &instruction::Fields {
            description: &form.description,
            kind: Some("develop"),
            prompt: &form.prompt,
            ..Default::default()
        },
    )
    .map_err(bad_request)?;
    instruction::check(&name, &text).map_err(bad_request)?;
    std::fs::create_dir_all(path.parent().unwrap_or(&home))?;
    crate::agent::taskdir::write_atomic(&path, &text)?;
    let to = page_url(&scope.url(&name), &[("tab", Some("settings"))]);
    Err(see_other(to).into())
}

#[derive(Deserialize)]
struct SaveForm {
    #[serde(rename = "do", default)]
    action: String,
    #[serde(default)]
    project: String,
    /// The name to save it under; empty or the same: no change.
    #[serde(default)]
    new_name: String,
    #[serde(default)]
    description: String,
    /// Absent for a built-in instruction, which keeps its kind.
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    model: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    temperature: String,
    #[serde(default)]
    permission: String,
    #[serde(default)]
    prompt: String,
}

/// `POST /instruction/{name}`: keeps the fields as the instruction of the
/// page's scope (`do=save`), or removes that file so the inherited one applies
/// again (`do=reset`). With `new_name` a save also renames: the file at the
/// scope's home moves to the new name (runs already made keep the old one).
#[route(POST "/instruction/{name}")]
async fn save_instruction(cx: &Cx, Form(form): Form<SaveForm>) -> Result<Response> {
    let name = path_param::<Name>(cx)?.clone();
    let db = open_db()?;
    let scope = Scope::asked(&db, Some(&form.project))?;
    let current = instruction::find(&name, &scope.layers).ok_or_not_found()?;
    let home = scope
        .home
        .clone()
        .ok_or_else(|| bad_request("there is no folder to keep it in"))?;
    let path = instruction::file(&home, &name);
    match form.action.as_str() {
        "save" => {
            let renamed = instruction::rename_target(&name, &form.new_name).map_err(bad_request)?;
            let target = renamed.as_deref().unwrap_or(&name);
            // the keys the form does not know stay as they are in the file
            let kind = if instruction::is_builtin(&name) {
                None
            } else {
                form.kind.as_deref().filter(|k| !k.is_empty())
            };
            let text = instruction::compose(
                Some(&current.raw),
                &instruction::Fields {
                    description: &form.description,
                    kind,
                    model: &form.model,
                    mode: &form.mode,
                    temperature: &form.temperature,
                    permission: &form.permission,
                    prompt: &form.prompt,
                },
            )
            .map_err(bad_request)?;
            instruction::check(target, &text).map_err(bad_request)?;
            let to = instruction::file(&home, target);
            if renamed.is_some() && to.exists() {
                return Err(bad_request(format!("'{target}' exists already")).into());
            }
            std::fs::create_dir_all(to.parent().unwrap_or(&home))?;
            crate::agent::taskdir::write_atomic(&to, &text)?;
            if renamed.is_some() {
                // only the file at this scope moves; an inherited one stays where it is
                match std::fs::remove_file(&path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
            return Err(
                see_other(page_url(&scope.url(target), &[("tab", Some("settings"))])).into(),
            );
        }
        "reset" => match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        },
        other => return Err(bad_request(format!("'{other}' is not an action")).into()),
    }
    Err(see_other(page_url(&scope.url(&name), &[("tab", Some("settings"))])).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn the_list_counts_the_runs_each_instruction_was_given() {
        let scope = Scope::global();
        let run = |n: u32, instruction: Option<&str>, kind: &str| Use {
            task_id: 1,
            task: "app/Fix it".into(),
            run: Run {
                n,
                agent: crate::agent::run::Harness::Claude,
                model: "haiku".into(),
                effort: "low".into(),
                kind: kind.into(),
                instruction: instruction.map(String::from),
                status: crate::agent::run::Status::Done,
                started: format!("2026-10-30 10:0{n}:00"),
                ended: None,
                exit_code: Some(0),
                branch: None,
                worktree: None,
                pid: None,
            },
        };
        // a run from before instructions existed counts for its kind
        let uses = vec![
            run(3, Some("fix"), "fix"),
            run(2, None, "fix"),
            run(1, None, "plan"),
        ];
        let rows = rows(&scope, &uses);
        let by = |name: &str| rows.iter().find(|r| r.name == name).unwrap();
        assert_eq!(by("fix").used, 2);
        assert_eq!((by("plan").used, by("develop").used), (1, 0));
        assert!(by("fix").description.starts_with("Reproduce"));
    }

    #[test]
    fn usage_is_found_in_the_runs_of_the_tasks() {
        let notes = std::env::temp_dir().join(format!("iter-usage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&notes);
        let db = fixtures::db();
        let mut p = fixtures::project("app");
        p.notes_path = notes.display().to_string();
        let project = db.insert(&p).unwrap();
        let task = db
            .insert(&Task::template(
                project,
                String::new(),
                crate::models::TaskStatus::Queue,
            ))
            .unwrap();
        let (p, t) = (
            db.get::<Project>(project).unwrap().unwrap(),
            db.get::<Task>(task).unwrap().unwrap(),
        );
        let dir = taskdir::of(&db, &p, &t).unwrap();
        let mut run = Run {
            n: 1,
            agent: crate::agent::run::Harness::Claude,
            model: "haiku".into(),
            effort: "low".into(),
            kind: "fix".into(),
            instruction: Some("review".into()),
            status: crate::agent::run::Status::Done,
            started: "2026-10-30 10:00:00".into(),
            ended: None,
            exit_code: Some(0),
            branch: None,
            worktree: None,
            pid: None,
        };
        dir.write_run(&run).unwrap();
        run.n = 2;
        run.instruction = None;
        dir.write_run(&run).unwrap();
        let all = usage(&db, None);
        assert_eq!(all.len(), 2);
        assert_eq!(usage(&db, Some(project)).len(), 2);
        assert!(usage(&db, Some(project + 1)).is_empty());
    }
}
