//! The forms that write instructions: create, save, reset.

use super::*;

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

/// Made an instruction that changes no code; Settings changes that.
#[component]
pub async fn new_instruction_modal(scope: &Scope) -> Result<impl View> {
    let project = scope.project_field();
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

/// Where a saved instruction leads: the list, or the project for a project's own.
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

/// Where the instructions of this scope are kept, or a refusal when it has no folder.
fn home_of(scope: &Scope) -> Result<PathBuf> {
    Ok(scope
        .home
        .clone()
        .ok_or_else(|| bad_request("there is no folder to keep it in"))?)
}

/// The refusal for a text that does not make a valid instruction.
fn refuse(e: impl ToString) -> topcoat::Error {
    bad_request(e.to_string()).into()
}

/// Writes `text` at `path`, making its folder, after checking it is a valid instruction named `name`.
fn write_checked(home: &Path, path: &Path, name: &str, text: &str) -> Result<()> {
    instruction::check(name, text).map_err(refuse)?;
    std::fs::create_dir_all(path.parent().unwrap_or(home))?;
    Ok(iter_core::files::write_atomic(path, text)?)
}

/// Removes a file; one that is not there is as good as removed.
fn unlink(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

/// Where a write leads: the instruction's Settings tab.
fn settings_to(scope: &Scope, name: &str) -> topcoat::Error {
    see_other(page_url(&scope.url(name), &[("tab", Some("settings"))])).into()
}

#[route(POST "/instructions")]
pub(super) async fn create_instruction(Form(form): Form<NewForm>) -> Result<Response> {
    let db = fresh_db()?;
    let scope = Scope::asked(&db, Some(&form.project))?;
    let name = form.name.trim().to_string();
    if !instruction::valid_name(&name) {
        return Err(refuse(format!(
            "'{name}' is not a name: lowercase letters, digits and dashes"
        )));
    }
    let home = home_of(&scope)?;
    let path = instruction::file(&home, &name);
    if path.exists() {
        return Err(refuse(format!("'{name}' exists already")));
    }
    let text = instruction::compose(
        None,
        &instruction::Fields {
            description: &form.description,
            prompt: &form.prompt,
            ..Default::default()
        },
    )
    .map_err(refuse)?;
    write_checked(&home, &path, &name, &text)?;
    Err(settings_to(&scope, &name))
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum SaveButton {
    Save,
    Reset,
}

#[derive(Deserialize)]
struct SaveForm {
    #[serde(rename = "do")]
    action: SaveButton,
    #[serde(default)]
    project: String,
    #[serde(default)]
    new_name: String,
    #[serde(default)]
    description: String,
    /// `true` or `false`; absent leaves the file's `worktree` as it is.
    #[serde(default)]
    worktree: Option<String>,
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

/// `do=save` keeps the fields; `do=reset` removes the file at this scope so the inherited one applies; `new_name` also renames.
#[route(POST "/instruction/{name}")]
pub(super) async fn save_instruction(cx: &Cx, Form(form): Form<SaveForm>) -> Result<Response> {
    let name = path_param::<Name>(cx)?.clone();
    let db = fresh_db()?;
    let scope = Scope::asked(&db, Some(&form.project))?;
    let current = instruction::find(&name, &scope.layers).ok_or_not_found()?;
    let home = home_of(&scope)?;
    let path = instruction::file(&home, &name);
    if form.action == SaveButton::Reset {
        unlink(&path)?;
        return Err(settings_to(&scope, &name));
    }
    let renamed = instruction::rename_target(&name, &form.new_name).map_err(refuse)?;
    let target = renamed.as_deref().unwrap_or(&name);
    // the keys the form does not know stay as they are in the file
    let worktree = form.worktree.as_deref().map(|w| w.trim() == "true");
    let text = instruction::compose(
        Some(&current.raw),
        &instruction::Fields {
            description: &form.description,
            worktree,
            model: &form.model,
            mode: &form.mode,
            temperature: &form.temperature,
            permission: &form.permission,
            prompt: &form.prompt,
        },
    )
    .map_err(refuse)?;
    instruction::check(target, &text).map_err(refuse)?;
    let to = instruction::file(&home, target);
    if renamed.is_some() && to.exists() {
        return Err(refuse(format!("'{target}' exists already")));
    }
    write_checked(&home, &to, target, &text)?;
    if renamed.is_some() {
        // only the file at this scope moves; an inherited one stays where it is
        unlink(&path)?;
    }
    Err(settings_to(&scope, target))
}
