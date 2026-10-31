//! The forms that write instructions: create, save, reset.

use super::*;

/// The placeholders an instruction's prompt may use, as the forms say them.
pub(super) fn placeholder_hint() -> String {
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

/// Where a saved instruction leads: the list, or (for a project's own) the
/// project, which no longer has a tab for them.
pub(super) fn instructions_target(scope: &Scope) -> String {
    match &scope.project {
        Some(p) => project_url(p.id()),
        None => instructions_url(),
    }
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
pub(super) async fn create_instruction(Form(form): Form<NewForm>) -> Result<Response> {
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
    .map_err(|e| bad_request(e.to_string()))?;
    instruction::check(&name, &text).map_err(|e| bad_request(e.to_string()))?;
    std::fs::create_dir_all(path.parent().unwrap_or(&home))?;
    crate::agent::taskdir::write_atomic(&path, &text)?;
    let to = page_url(&scope.url(&name), &[("tab", Some("settings"))]);
    Err(see_other(to).into())
}

/// The buttons of the instruction form (`name="do"`).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum SaveButton {
    /// Write the file at this scope (and rename it, when asked).
    Save,
    /// Delete this scope's file, so the inherited one shows again.
    Reset,
}

#[derive(Deserialize)]
struct SaveForm {
    /// The button pressed; a submission without one is refused (a 400).
    #[serde(rename = "do")]
    action: SaveButton,
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
pub(super) async fn save_instruction(cx: &Cx, Form(form): Form<SaveForm>) -> Result<Response> {
    let name = path_param::<Name>(cx)?.clone();
    let db = open_db()?;
    let scope = Scope::asked(&db, Some(&form.project))?;
    let current = instruction::find(&name, &scope.layers).ok_or_not_found()?;
    let home = scope
        .home
        .clone()
        .ok_or_else(|| bad_request("there is no folder to keep it in"))?;
    let path = instruction::file(&home, &name);
    match form.action {
        SaveButton::Save => {
            let renamed = instruction::rename_target(&name, &form.new_name)
                .map_err(|e| bad_request(e.to_string()))?;
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
            .map_err(|e| bad_request(e.to_string()))?;
            instruction::check(target, &text).map_err(|e| bad_request(e.to_string()))?;
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
        SaveButton::Reset => match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        },
    }
    Err(see_other(page_url(&scope.url(&name), &[("tab", Some("settings"))])).into())
}
