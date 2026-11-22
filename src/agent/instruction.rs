//! Instructions: markdown files (opencode agent format, front matter over the prompt) saying what an agent is told.
//! Looked up nearest first: the task's folder, project notes, organization notes, then iter's config
//! folder (`<config>/instructions/<name>.md`, beside `iter.yaml`); see [`Source`]. None is built in.

#[cfg(feature = "web")]
use crate::agent::prompt::{self, Context};
use crate::error::{AgentError, AgentResult};
use crate::text::yaml_scalar;
use serde_yaml::Value;
#[cfg(feature = "web")]
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    Task,
    Project,
    Organization,
    Global,
}

#[cfg(feature = "web")]
impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Task => "task",
            Source::Project => "project",
            Source::Organization => "organization",
            Source::Global => "iter's config folder",
        }
    }
}

pub type Layers = Vec<(Source, PathBuf)>;

/// The layers that apply to `project`, nearest first (those that exist).
pub fn layers_of(
    db: &crate::db::Db,
    project: &crate::models::Project,
) -> crate::error::Result<Layers> {
    let org = db.org_of(project)?;
    let mut layers = Layers::new();
    layers.extend(crate::notes::project_dir(project, &org).map(|d| (Source::Project, d)));
    layers.extend(
        org.as_ref()
            .and_then(crate::notes::org_dir)
            .map(|d| (Source::Organization, d)),
    );
    layers.extend(global().map(|d| (Source::Global, d)));
    Ok(layers)
}

/// [`layers_of`] with the task's own folder in front: the layers a run of `task` is offered, checked
/// and filled from, by the CLI and the web alike. A task with no folder adds nothing.
pub fn layers_for_task(
    db: &crate::db::Db,
    project: &crate::models::Project,
    task: &crate::models::Task,
) -> crate::error::Result<Layers> {
    let mut layers = layers_of(db, project)?;
    if let Ok(dir) = crate::agent::taskdir::of(db, project, task) {
        layers.insert(0, (Source::Task, dir.root().to_path_buf()));
    }
    Ok(layers)
}

/// iter's config folder, where the instructions everyone shares live.
pub fn global() -> Option<PathBuf> {
    crate::config::config_dir().ok()
}

pub use crate::agent::prompt::file;

/// The front matter (between a first line `---` and the next) and the prompt; no `---` means all prompt.
pub fn split(raw: &str) -> (Option<&str>, &str) {
    match crate::md_edit::split_closed(raw) {
        Some((front, body)) => (Some(front), body),
        None => (None, raw),
    }
}

/// The front matter keys iter reads; others stay in the file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Front {
    pub description: String,
    pub model: Option<String>,
    /// The run works on the code: it gets the task's branch and worktree.
    pub worktree: bool,
    /// The instruction leads subagents from a session someone is in: it is printed
    /// (`iter task prompt`), never run headless.
    pub interactive: bool,
    pub mode: Option<String>,
    pub temperature: Option<String>,
    pub permission: Option<String>,
}

pub fn parse_front(yaml: &str) -> AgentResult<Front> {
    let value: Value = serde_yaml::from_str(yaml).map_err(AgentError::FrontMatterUnreadable)?;
    let map = match value {
        Value::Null => return Ok(Front::default()),
        Value::Mapping(m) => m,
        _ => return Err(AgentError::FrontMatterNotMapping),
    };
    let get = |key: &str| map.get(key);
    let text = |key: &str| -> AgentResult<Option<String>> {
        match get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => yaml_scalar(v)
                .map(Some)
                .ok_or_else(|| AgentError::FrontMatterNotText(key.to_string())),
        }
    };
    let flag = |key: &str| -> AgentResult<bool> {
        match get(key) {
            None | Some(Value::Null) => Ok(false),
            Some(Value::Bool(b)) => Ok(*b),
            Some(_) => Err(AgentError::FrontMatterNotFlag(key.to_string())),
        }
    };
    Ok(Front {
        description: text("description")?.unwrap_or_default(),
        model: text("model")?,
        worktree: flag("worktree")?,
        interactive: flag("interactive")?,
        mode: text("mode")?,
        temperature: text("temperature")?,
        permission: match get("permission") {
            None | Some(Value::Null) => None,
            Some(v) => serde_yaml::to_string(v)
                .ok()
                .map(|s| s.trim_end().to_string()),
        },
    })
}

#[derive(Clone, Debug)]
#[cfg_attr(not(feature = "web"), allow(dead_code))] // the web lists and edits them; the CLI reads front and body
pub struct Instruction {
    pub name: String,
    pub front: Front,
    pub body: String,
    pub raw: String,
    pub source: Source,
    pub path: Option<PathBuf>,
    pub problem: Option<String>,
}

impl Instruction {
    fn new(name: &str, raw: String, source: Source, path: Option<PathBuf>) -> Instruction {
        let (yaml, body) = split(&raw);
        let (front, problem) = match yaml.map(parse_front) {
            Some(Ok(front)) => (front, None),
            Some(Err(e)) => (Front::default(), Some(e.to_string())),
            None => (Front::default(), None),
        };
        Instruction {
            name: name.to_string(),
            body: body.trim_start_matches('\n').to_string(),
            front,
            raw,
            source,
            path,
            problem,
        }
    }
}

/// Whether `name` can be a file name: lowercase letters, digits and dashes, starting with a letter or digit.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 48
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The instruction `name` under these layers: the nearest file; none if no layer holds one.
pub fn find(name: &str, layers: &Layers) -> Option<Instruction> {
    if !valid_name(name) {
        return None;
    }
    for (source, dir) in layers {
        let path = file(dir, name);
        if let Ok(raw) = std::fs::read_to_string(&path) {
            return Some(Instruction::new(name, raw, *source, Some(path)));
        }
    }
    None
}

/// [`find`] for a run or a prompt: an instruction found nowhere is an error naming where it is
/// expected (`<config>/instructions/<name>.md`), and one whose front matter does not read is one too.
pub fn resolve(name: &str, layers: &Layers) -> AgentResult<Instruction> {
    if !valid_name(name) {
        return Err(AgentError::InvalidName(name.to_string()));
    }
    let Some(found) = find(name, layers) else {
        let config = layers
            .iter()
            .find(|(s, _)| *s == Source::Global)
            .map(|(_, d)| d.clone())
            .or_else(global)
            .unwrap_or_default();
        return Err(AgentError::InstructionNotFound {
            name: name.to_string(),
            path: file(&config, name),
        });
    };
    if let Some(problem) = &found.problem {
        return Err(AgentError::InstructionUnreadable {
            path: found.path.clone().unwrap_or_default(),
            problem: problem.clone(),
        });
    }
    Ok(found)
}

#[cfg(feature = "web")]
/// The name to save under when the form asks for `wanted`: none if unchanged; an error if not a name.
pub fn rename_target(old: &str, wanted: &str) -> AgentResult<Option<String>> {
    let wanted = wanted.trim();
    if wanted.is_empty() || wanted == old {
        return Ok(None);
    }
    if !valid_name(wanted) {
        return Err(AgentError::InvalidName(wanted.to_string()));
    }
    Ok(Some(wanted.to_string()))
}

#[cfg(feature = "web")]
/// Every instruction under these layers, by name: what the files hold, nothing else.
pub fn list(layers: &Layers) -> Vec<Instruction> {
    let mut names: Vec<String> = Vec::new();
    for (_, dir) in layers {
        let Ok(entries) = std::fs::read_dir(dir.join("instructions")) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            let name = path.file_stem().and_then(|s| s.to_str());
            if let Some(name) = name
                && path.extension().is_some_and(|x| x == "md")
                && valid_name(name)
                && !names.iter().any(|n| n == name)
            {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    names.iter().filter_map(|n| find(n, layers)).collect()
}

#[cfg(feature = "web")]
/// Whether `raw` can be kept: front matter reads, has a prompt, and no unknown placeholder (an agent must never see a literal `{{taks}}`).
pub fn check(name: &str, raw: &str) -> AgentResult<()> {
    let (yaml, body) = split(raw);
    if let Some(y) = yaml {
        parse_front(y)?;
    }
    if body.trim().is_empty() {
        return Err(AgentError::NoPrompt);
    }
    let dir = Path::new("/");
    let ctx = Context {
        task: "p/t",
        task_name: "t",
        task_description: "d",
        task_id: 1,
        task_dir: dir,
        project_name: "p",
        project_path: "/",
        worktree: None,
        plan_path: dir,
        plan: None,
        onboarding: "",
        branch_main: "main",
        ingest: "",
    };
    prompt::render(body, name, &ctx).map(|_| ())
}

#[cfg(feature = "web")]
/// `model` of the front matter if the harness offers it (opencode's `anthropic/claude-sonnet-5-5` loses the provider), else none.
pub fn preferred_model(front: &Front, offered: &[String]) -> Option<String> {
    let model = front.model.as_deref()?;
    let bare = model.rsplit('/').next().unwrap_or(model);
    offered.iter().find(|m| m.as_str() == bare).cloned()
}

#[cfg(feature = "web")]
/// What a new instruction's prompt starts as in the editor: the placeholders most prompts use.
pub fn starter_prompt() -> String {
    "You are working on the task \"{{task.name}}\" of the project {{project.name}}.\n\n\
     ## The task\n\n{{task.description}}\n\n\
     ## What the project already knows\n\n{{onboarding}}\n\n\
     ## The plan\n\n{{plan}}\n\n\
     The code is in {{worktree}}.\n"
        .to_string()
}

#[cfg(feature = "web")]
#[derive(Clone, Debug, Default)]
pub struct Fields<'a> {
    pub description: &'a str,
    /// `worktree: true`: the run works on the code. None leaves the file's key as it is.
    pub worktree: Option<bool>,
    pub model: &'a str,
    pub mode: &'a str,
    pub temperature: &'a str,
    pub permission: &'a str,
    pub prompt: &'a str,
}

#[cfg(feature = "web")]
/// Sets `key` to `value` in `map`, or removes it when the field is empty.
fn put(map: &mut serde_yaml::Mapping, key: &str, value: Option<Value>) {
    match value {
        Some(v) => {
            map.insert(Value::String(key.into()), v);
        }
        None => {
            map.remove(key);
        }
    }
}

#[cfg(feature = "web")]
fn text_value(text: &str) -> Option<Value> {
    let text = text.trim();
    (!text.is_empty()).then(|| Value::String(text.replace('\n', " ")))
}

#[cfg(feature = "web")]
/// An instruction file from its fields: `existing`'s front matter with the fields written over it (empty removes a key, unknown keys stay), a blank line, the prompt.
pub fn compose(existing: Option<&str>, f: &Fields) -> AgentResult<String> {
    let mut map = existing
        .and_then(|raw| split(raw).0)
        .and_then(|yaml| serde_yaml::from_str::<Value>(yaml).ok())
        .and_then(|v| match v {
            Value::Mapping(m) => Some(m),
            _ => None,
        })
        .unwrap_or_default();
    put(&mut map, "description", text_value(f.description));
    put(&mut map, "model", text_value(f.model));
    put(&mut map, "mode", text_value(f.mode));
    let temperature = f.temperature.trim();
    put(
        &mut map,
        "temperature",
        (!temperature.is_empty()).then(|| match temperature.parse::<f64>() {
            Ok(n) => serde_yaml::to_value(n).unwrap_or(Value::String(temperature.into())),
            Err(_) => Value::String(temperature.into()),
        }),
    );
    if let Some(worktree) = f.worktree {
        put(&mut map, "worktree", worktree.then_some(Value::Bool(true)));
    }
    let permission = f.permission.trim();
    put(
        &mut map,
        "permission",
        if permission.is_empty() {
            None
        } else {
            match serde_yaml::from_str::<Value>(permission) {
                Ok(Value::Null) => None,
                Ok(v) => Some(v),
                Err(e) => return Err(AgentError::FieldPermission(e)),
            }
        },
    );
    let prompt = f.prompt.replace("\r\n", "\n");
    let prompt = prompt.trim_matches('\n');
    if map.is_empty() {
        return Ok(format!("{prompt}\n"));
    }
    let yaml = serde_yaml::to_string(&Value::Mapping(map)).map_err(AgentError::Yaml)?;
    Ok(format!("---\n{yaml}---\n\n{prompt}\n"))
}

#[cfg(feature = "web")]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    fn write(dir: &Path, name: &str, text: &str) {
        let path = file(dir, name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn front_matter_is_split_from_the_prompt() {
        assert_eq!(split("---\na: 1\n---\nBody\n"), (Some("a: 1\n"), "Body\n"));
        assert_eq!(split("---\n---\nBody"), (Some(""), "Body"));
        assert_eq!(split("no front"), (None, "no front"));
        assert_eq!(split("---\nnever closed\n"), (None, "---\nnever closed\n"));
        assert_eq!(split("---\na: 1\n---"), (Some("a: 1\n"), ""));
    }

    #[test]
    fn an_opencode_agent_is_read_with_its_keys() {
        let raw = "---\ndescription: Reviews code\nmode: subagent\nmodel: anthropic/claude-sonnet-5-5\ntemperature: 0.1\npermission:\n  edit: deny\n  bash: deny\nkind: check\n---\n\nYou are in review mode.\n";
        let i = Instruction::new("review", raw.into(), Source::Global, None);
        assert_eq!(i.problem, None);
        assert_eq!(i.front.description, "Reviews code");
        assert_eq!(i.front.temperature.as_deref(), Some("0.1"));
        assert_eq!(
            i.front.permission.as_deref(),
            Some("edit: deny\nbash: deny")
        );
        assert!(!i.front.worktree && !i.front.interactive);
        assert_eq!(i.body.trim(), "You are in review mode.");
        let offered = vec!["claude-sonnet-5-5".to_string(), "haiku".to_string()];
        assert_eq!(
            preferred_model(&i.front, &offered).as_deref(),
            Some("claude-sonnet-5-5")
        );
        assert_eq!(preferred_model(&Front::default(), &offered), None);
    }

    #[test]
    fn worktree_and_interactive_are_flags_and_a_bad_front_matter_is_said() {
        let i = Instruction::new(
            "fix",
            "---\nworktree: true\ninteractive: true\n---\nx".into(),
            Source::Project,
            None,
        );
        assert!(i.front.worktree && i.front.interactive && i.problem.is_none());
        let bad = Instruction::new(
            "a",
            "---\nworktree: dance\n---\nx".into(),
            Source::Project,
            None,
        );
        assert!(bad.problem.unwrap().contains("worktree"));
        let worse = Instruction::new("a", "---\n[1, 2\n---\nx".into(), Source::Project, None);
        assert!(worse.problem.is_some());
    }

    #[test]
    fn the_nearest_layer_wins_and_nothing_is_built_in() {
        let (project, org) = (fixtures::scratch("instr-p"), fixtures::scratch("instr-o"));
        write(&org, "fix", "---\ndescription: Org fix\n---\nOrg {{task}}");
        write(&org, "review", "Review it");
        write(&project, "review", "Project review");
        let layers: Layers = vec![
            (Source::Project, project.clone()),
            (Source::Organization, org.clone()),
        ];
        let fix = find("fix", &layers).unwrap();
        assert_eq!(
            (fix.source, fix.front.description.as_str()),
            (Source::Organization, "Org fix")
        );
        let review = find("review", &layers).unwrap();
        assert_eq!(
            (review.source, review.body.as_str()),
            (Source::Project, "Project review")
        );
        assert!(find("plan", &layers).is_none());
        assert!(find("nope", &layers).is_none() && find("../x", &layers).is_none());
        let names: Vec<String> = list(&layers).into_iter().map(|i| i.name).collect();
        assert_eq!(names, ["fix", "review"]);
        let e = resolve("plan", &layers).unwrap_err().to_string();
        assert!(
            e.contains("'plan'") && e.contains("instructions/plan.md"),
            "{e}"
        );
        write(&project, "broken", "---\nworktree: 3\n---\nx");
        let e = resolve("broken", &layers).unwrap_err().to_string();
        assert!(e.contains("broken.md") && e.contains("worktree"), "{e}");
    }

    #[test]
    fn what_cannot_be_kept_is_refused() {
        assert!(check("fix", "---\ndescription: x\n---\nWork on {{task.name}}").is_ok());
        assert!(check("fix", "Work on {{taks}}").is_err());
        assert!(check("fix", "---\ndescription: x\n---\n\n").is_err());
        assert!(check("fix", "---\nkind: dance\n---\nx").is_ok());
        assert!(check("fix", "---\ninteractive: maybe\n---\nx").is_err());
        assert!(
            valid_name("code-review")
                && !valid_name("Code")
                && !valid_name("a/b")
                && !valid_name("")
        );
    }

    fn fields<'a>(prompt: &'a str) -> Fields<'a> {
        Fields {
            description: "Reviews: code",
            worktree: Some(true),
            prompt,
            ..Fields::default()
        }
    }

    #[test]
    fn fields_become_a_file_that_reads_back_as_the_same_fields() {
        let f = Fields {
            model: "anthropic/claude-sonnet-5-5",
            mode: "subagent",
            temperature: "0.1",
            permission: "edit: deny\nbash: deny",
            ..fields("Review {{task.name}}.\r\nBe kind.")
        };
        let raw = compose(None, &f).unwrap();
        assert!(check("review", &raw).is_ok(), "{raw}");
        let i = Instruction::new("review", raw, Source::Global, None);
        assert_eq!(i.problem, None);
        assert_eq!(i.front.description, "Reviews: code");
        assert_eq!(
            i.front.model.as_deref(),
            Some("anthropic/claude-sonnet-5-5")
        );
        assert_eq!(i.front.mode.as_deref(), Some("subagent"));
        assert_eq!(i.front.temperature.as_deref(), Some("0.1"));
        assert_eq!(
            i.front.permission.as_deref(),
            Some("edit: deny\nbash: deny")
        );
        assert!(i.front.worktree);
        assert_eq!(i.body.trim(), "Review {{task.name}}.\nBe kind.");
    }

    #[test]
    fn empty_fields_drop_their_keys_and_other_keys_stay() {
        let existing = "---\ndescription: old\nmodel: haiku\nsteps: 7\ntools:\n  write: false\n---\n\nOld prompt\n";
        let f = Fields {
            description: "new",
            ..fields("New prompt")
        };
        let f = Fields {
            worktree: None,
            ..f
        };
        let raw = compose(Some(existing), &f).unwrap();
        let (yaml, body) = split(&raw);
        let map: serde_yaml::Mapping = serde_yaml::from_str(yaml.unwrap()).unwrap();
        assert_eq!(map.get("description").and_then(Value::as_str), Some("new"));
        assert!(map.get("model").is_none(), "{raw}");
        assert_eq!(map.get("steps").and_then(Value::as_u64), Some(7));
        assert_eq!(
            map.get("tools")
                .and_then(|t| t.get("write"))
                .and_then(Value::as_bool),
            Some(false)
        );
        assert!(map.get("worktree").is_none());
        assert_eq!(body.trim(), "New prompt");
        let bare = compose(
            None,
            &Fields {
                prompt: "Only this",
                ..Fields::default()
            },
        )
        .unwrap();
        assert_eq!(bare, "Only this\n");
    }

    #[test]
    fn a_field_that_cannot_be_kept_is_refused_by_name() {
        let off = Fields {
            worktree: Some(false),
            ..fields("x")
        };
        let raw = compose(Some("---\nworktree: true\n---\nx"), &off).unwrap();
        assert!(!raw.contains("worktree"), "{raw}");
        let bad_yaml = Fields {
            permission: "[1, 2",
            ..fields("x")
        };
        assert!(
            compose(None, &bad_yaml)
                .unwrap_err()
                .to_string()
                .starts_with("permission")
        );
        let hot = Fields {
            temperature: "hot",
            ..fields("x")
        };
        assert!(compose(None, &hot).unwrap().contains("temperature: hot"));
        assert!(check("a", &compose(None, &fields("  ")).unwrap()).is_err());
    }

    #[test]
    fn the_starter_prompt_is_valid_as_it_is() {
        let raw = compose(None, &fields(&starter_prompt())).unwrap();
        assert!(check("review", &raw).is_ok(), "{raw}");
    }

    #[test]
    fn a_name_can_change_to_any_valid_name() {
        assert_eq!(rename_target("review", "review").unwrap(), None);
        assert_eq!(rename_target("review", "  ").unwrap(), None);
        assert_eq!(
            rename_target("review", " audit ").unwrap(),
            Some("audit".into())
        );
        assert!(rename_target("review", "Bad Name").is_err());
        assert_eq!(rename_target("fix", "fix").unwrap(), None);
    }

    /// Characterisation (R7, YAML scalar): an instruction's front-matter scalar is text exactly
    /// as written (not trimmed), a number or a bool; a null, a list or a map is none.
    #[test]
    fn a_scalar_keeps_its_text_as_written() {
        let value = |yaml: &str| serde_yaml::from_str::<Value>(yaml).unwrap();
        assert_eq!(
            yaml_scalar(&value("'  padded  '")).as_deref(),
            Some("  padded  ")
        );
        assert_eq!(yaml_scalar(&value("42")).as_deref(), Some("42"));
        assert_eq!(yaml_scalar(&value("false")).as_deref(), Some("false"));
        for none in ["~", "[a]", "{a: b}"] {
            assert_eq!(yaml_scalar(&value(none)), None, "{none}");
        }
    }
}
