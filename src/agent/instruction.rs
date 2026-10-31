//! Instructions: what an agent is told for a kind of work, as markdown files
//! in the format of an opencode agent: front matter (`description`, `model`,
//! `mode`, `temperature`, `permission`; iter adds `kind`) over the prompt.
//!
//! An instruction is `instructions/<name>.md` in one of three places, the
//! nearest winning: the project's notes, its organization's notes, iter's own
//! config folder. The four built-in ones (`plan`, `develop`, `test`,
//! `check`, `fix`) are the last resort. Which place an instruction comes from
//! is its [`Source`].

use crate::agent::error::{AgentError, AgentResult};
use crate::agent::prompt::{self, Context, Kind};
use serde_yaml::Value;
use std::path::{Path, PathBuf};

/// Where an instruction comes from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    /// The task's own folder.
    Task,
    Project,
    Organization,
    /// iter's config folder: what every project of the user shares.
    Global,
    BuiltIn,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Task => "task",
            Source::Project => "project",
            Source::Organization => "organization",
            Source::Global => "iter's config folder",
            Source::BuiltIn => "built in",
        }
    }
}

/// The folders that may hold an `instructions/` folder, nearest first.
pub type Layers = Vec<(Source, PathBuf)>;

/// The layers that apply to `project`: its notes, its organization's notes,
/// iter's config folder (those that exist as folders to look in).
pub fn layers_of(db: &crate::db::Db, project: &crate::models::Project) -> Layers {
    let org = project
        .organization_id
        .and_then(|id| db.get::<crate::models::Organization>(id).ok().flatten());
    let mut layers = Layers::new();
    layers.extend(crate::notes::project_dir(project, &org).map(|d| (Source::Project, d)));
    layers.extend(
        org.as_ref()
            .and_then(crate::notes::org_dir)
            .map(|d| (Source::Organization, d)),
    );
    layers.extend(global().map(|d| (Source::Global, d)));
    layers
}

/// iter's config folder, where the instructions of every project live.
pub fn global() -> Option<PathBuf> {
    crate::config::config_dir().ok()
}

/// The file of instruction `name` in `dir`.
pub fn file(dir: &Path, name: &str) -> PathBuf {
    dir.join("instructions").join(format!("{name}.md"))
}

/// The front matter and the prompt of `raw`: the lines between a first line
/// `---` and the next one, then the rest. A text that does not open with
/// `---` is all prompt.
pub fn split(raw: &str) -> (Option<&str>, &str) {
    let Some(rest) = raw.strip_prefix("---\n") else {
        return (None, raw);
    };
    if let Some(body) = rest.strip_prefix("---\n") {
        return (Some(""), body);
    }
    if let Some(i) = rest.find("\n---\n") {
        return (Some(&rest[..=i]), &rest[i + 5..]);
    }
    match rest.strip_suffix("\n---") {
        Some(front) => (Some(front), ""),
        None => (None, raw),
    }
}

/// The front matter's keys that iter reads. Others are kept in the file and
/// ignored.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Front {
    pub description: String,
    pub model: Option<String>,
    /// One of the four built-in kinds: how the work is set up (a kind that
    /// changes code gets a worktree). Develop when it says nothing.
    pub kind: Option<String>,
    pub mode: Option<String>,
    pub temperature: Option<String>,
    /// The permissions, as the YAML they were written in.
    pub permission: Option<String>,
}

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Reads front matter; an error says what is wrong with it.
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
            Some(v) => scalar(v)
                .map(Some)
                .ok_or_else(|| AgentError::FrontMatterNotText(key.to_string())),
        }
    };
    let front = Front {
        description: text("description")?.unwrap_or_default(),
        model: text("model")?,
        kind: text("kind")?,
        mode: text("mode")?,
        temperature: text("temperature")?,
        permission: match get("permission") {
            None | Some(Value::Null) => None,
            Some(v) => serde_yaml::to_string(v)
                .ok()
                .map(|s| s.trim_end().to_string()),
        },
    };
    if let Some(kind) = &front.kind
        && kind_of_key(kind).is_none()
    {
        return Err(AgentError::FrontMatterKind(kind.clone()));
    }
    Ok(front)
}

fn kind_of_key(key: &str) -> Option<Kind> {
    use clap::ValueEnum;
    Kind::from_str(key, true).ok()
}

#[derive(Clone, Debug)]
pub struct Instruction {
    pub name: String,
    pub front: Front,
    /// The prompt, without the front matter.
    pub body: String,
    /// The whole file, as it is edited.
    pub raw: String,
    pub source: Source,
    /// The file it was read from; none for a built-in one.
    pub path: Option<PathBuf>,
    /// The front matter could not be read: why.
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
            body: body.to_string(),
            front,
            raw,
            source,
            path,
            problem,
        }
    }

    /// How the work is set up: the name when it is a built-in kind, else
    /// `kind:` of the front matter, else Develop.
    pub fn kind(&self) -> Kind {
        kind_of_key(&self.name)
            .or_else(|| self.front.kind.as_deref().and_then(kind_of_key))
            .unwrap_or(Kind::Develop)
    }
}

/// Whether `name` can be a file name here: lowercase letters, digits and
/// dashes, starting with a letter or digit.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 48
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Whether `name` is one of the built-in instructions, which keep their names:
/// the kinds of work iter itself starts agents for.
pub fn is_builtin(name: &str) -> bool {
    kind_of_key(name).is_some()
}

/// The name an instruction is saved under, when the form asks for `wanted`:
/// none when it stays as it is, an error when it cannot be what is asked.
/// A built-in instruction cannot be renamed, nor another one named like it.
pub fn rename_target(old: &str, wanted: &str) -> AgentResult<Option<String>> {
    let wanted = wanted.trim();
    if wanted.is_empty() || wanted == old {
        return Ok(None);
    }
    if !valid_name(wanted) {
        return Err(AgentError::InvalidName(wanted.to_string()));
    }
    if is_builtin(old) {
        return Err(AgentError::BuiltInKeepsName(old.to_string()));
    }
    if is_builtin(wanted) {
        return Err(AgentError::BuiltInName(wanted.to_string()));
    }
    Ok(Some(wanted.to_string()))
}

const KINDS: [Kind; 5] = [
    Kind::Plan,
    Kind::Develop,
    Kind::Test,
    Kind::Check,
    Kind::Fix,
];

fn builtin(kind: Kind) -> Instruction {
    Instruction::new(
        kind.key(),
        kind.builtin().to_string(),
        Source::BuiltIn,
        None,
    )
}

/// The instruction `name` as it applies with these layers: the nearest file,
/// else the built-in one.
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
    kind_of_key(name).map(builtin)
}

/// Every instruction that applies with these layers, by name: the built-in
/// ones and every file, each as `find` gives it.
pub fn list(layers: &Layers) -> Vec<Instruction> {
    let mut names: Vec<String> = KINDS.iter().map(|k| k.key().to_string()).collect();
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
    names[KINDS.len()..].sort();
    names.iter().filter_map(|n| find(n, layers)).collect()
}

/// Whether `raw` is an instruction that can be kept: its front matter reads,
/// it has a prompt, and the prompt has no placeholder iter does not know (an
/// agent must never be told a literal `{{taks}}`).
pub fn check(name: &str, raw: &str) -> AgentResult<()> {
    let (yaml, body) = split(raw);
    let front = match yaml {
        Some(y) => parse_front(y)?,
        None => Front::default(),
    };
    if body.trim().is_empty() {
        return Err(AgentError::NoPrompt);
    }
    let kind = kind_of_key(name)
        .or_else(|| front.kind.as_deref().and_then(kind_of_key))
        .unwrap_or(Kind::Develop);
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
    };
    prompt::render(body, kind, &ctx).map(|_| ())
}

/// The model to start a form with: `model` of the front matter when the
/// harness offers it (opencode writes `anthropic/claude-sonnet-5-5`; the
/// provider is dropped), else none.
pub fn preferred_model(front: &Front, offered: &[String]) -> Option<String> {
    let model = front.model.as_deref()?;
    let bare = model.rsplit('/').next().unwrap_or(model);
    offered.iter().find(|m| m.as_str() == bare).cloned()
}

/// The prompt a new instruction starts from: the built-in `develop` one.
pub fn starter_prompt() -> String {
    split(Kind::Develop.builtin())
        .1
        .trim_start_matches('\n')
        .to_string()
}

/// What the instruction form asks for, field by field.
#[derive(Clone, Debug, Default)]
pub struct Fields<'a> {
    pub description: &'a str,
    /// One of the built-in kinds; none leaves the file's `kind` as it is (a
    /// built-in instruction has no field for it).
    pub kind: Option<&'a str>,
    pub model: &'a str,
    pub mode: &'a str,
    pub temperature: &'a str,
    /// YAML, as the opencode `permission:` key takes it.
    pub permission: &'a str,
    pub prompt: &'a str,
}

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

fn text_value(text: &str) -> Option<Value> {
    let text = text.trim();
    (!text.is_empty()).then(|| Value::String(text.replace('\n', " ")))
}

/// The text of an instruction file from its fields. The front matter is
/// `existing`'s with the fields written over it (an empty field removes its
/// key; keys the form knows nothing about stay as they were), then a blank
/// line and the prompt. An error says which field is wrong.
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
    if let Some(kind) = f.kind.map(str::trim) {
        if kind_of_key(kind).is_none() {
            return Err(AgentError::FieldKind(kind.to_string()));
        }
        put(&mut map, "kind", Some(Value::String(kind.into())));
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
        assert_eq!(split("---\na: 1\n---"), (Some("a: 1"), ""));
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
        assert_eq!(i.kind(), Kind::Check);
        assert_eq!(i.body.trim(), "You are in review mode.");
        let offered = vec!["claude-sonnet-5-5".to_string(), "haiku".to_string()];
        assert_eq!(
            preferred_model(&i.front, &offered).as_deref(),
            Some("claude-sonnet-5-5")
        );
        assert_eq!(preferred_model(&Front::default(), &offered), None);
    }

    #[test]
    fn a_built_in_name_keeps_its_kind_and_a_bad_front_matter_is_said() {
        let i = Instruction::new(
            "fix",
            "---\nkind: plan\n---\nx".into(),
            Source::Project,
            None,
        );
        assert_eq!(i.kind(), Kind::Fix);
        let bad = Instruction::new(
            "a",
            "---\nkind: dance\n---\nx".into(),
            Source::Project,
            None,
        );
        assert!(bad.problem.unwrap().contains("dance"));
        let worse = Instruction::new("a", "---\n[1, 2\n---\nx".into(), Source::Project, None);
        assert!(worse.problem.is_some());
    }

    #[test]
    fn the_nearest_layer_wins_and_the_built_in_ones_are_the_last_resort() {
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
        assert_eq!(find("plan", &layers).unwrap().source, Source::BuiltIn);
        assert!(find("nope", &layers).is_none() && find("../x", &layers).is_none());
        let names: Vec<String> = list(&layers).into_iter().map(|i| i.name).collect();
        assert_eq!(names, ["plan", "develop", "test", "check", "fix", "review"]);
    }

    #[test]
    fn what_cannot_be_kept_is_refused() {
        assert!(check("fix", "---\ndescription: x\n---\nWork on {{task.name}}").is_ok());
        assert!(check("fix", "Work on {{taks}}").is_err());
        assert!(check("fix", "---\ndescription: x\n---\n\n").is_err());
        assert!(check("fix", "---\nkind: dance\n---\nx").is_err());
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
            kind: Some("check"),
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
        assert_eq!(i.kind(), Kind::Check);
        assert_eq!(i.body.trim(), "Review {{task.name}}.\nBe kind.");
    }

    #[test]
    fn empty_fields_drop_their_keys_and_other_keys_stay() {
        let existing = "---\ndescription: old\nmodel: haiku\nsteps: 7\ntools:\n  write: false\n---\n\nOld prompt\n";
        let f = Fields {
            description: "new",
            ..fields("New prompt")
        };
        let f = Fields { kind: None, ..f };
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
        assert!(map.get("kind").is_none());
        assert_eq!(body.trim(), "New prompt");
        // nothing but a prompt: no front matter at all
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
        let bad_kind = Fields {
            kind: Some("dance"),
            ..fields("x")
        };
        assert!(
            compose(None, &bad_kind)
                .unwrap_err()
                .to_string()
                .contains("dance")
        );
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
        // a number that is not one stays text; check() still judges the whole
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
    fn every_built_in_instruction_is_valid_and_says_what_it_is_for() {
        for kind in KINDS {
            let i = builtin(kind);
            assert_eq!(i.problem, None);
            assert!(!i.front.description.is_empty(), "{}", kind.key());
            assert!(check(kind.key(), &i.raw).is_ok(), "{}", kind.key());
        }
    }

    #[test]
    fn a_name_can_change_unless_it_is_built_in_or_would_be() {
        assert_eq!(rename_target("review", "review").unwrap(), None);
        assert_eq!(rename_target("review", "  ").unwrap(), None);
        assert_eq!(
            rename_target("review", " audit ").unwrap(),
            Some("audit".into())
        );
        assert!(rename_target("review", "Bad Name").is_err());
        assert!(rename_target("review", "fix").is_err());
        assert!(rename_target("fix", "audit").is_err());
        assert_eq!(rename_target("fix", "fix").unwrap(), None);
        assert!(is_builtin("plan") && !is_builtin("review"));
    }
}
