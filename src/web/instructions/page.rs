//! The page of one instruction, with its Info, Agents and Settings tabs.

use super::*;

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
        data_table(caption: "Agents given this instruction, the latest first", columns: &["Agent", "Task", "Status"],
            class: "tasks agents", empty: uses.is_empty().then_some("No agent has been given this instruction yet."),
            for u in uses.iter() {
                row(
                    cell(
                        <a class="u" href=(run_url(u.task_id, u.run.n))>"#" (u.run.n.to_string())</a>
                        <span class="hint">(u.run.model.as_str()) ", " (u.run.effort.as_str())</span>
                    )
                    cell(<a class="u muted" href=(task_url(u.task_id))>(u.task.as_str())</a>)
                    cell(state_pill(run: &u.run))
                )
            }
        )
    })
}

#[page("/instruction/{name}")]
pub(super) async fn detail_page(cx: &Cx) -> Result<impl View> {
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
