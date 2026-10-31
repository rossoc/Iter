//! The user's `iter.yaml` in `$XDG_CONFIG_HOME/iter` (`$HOME/.config/iter`; `%APPDATA%\iter` off
//! unix). Every key is optional; the database sits next to it by default.

use crate::error::{IterError, Result};
use crate::models::{Settings, TaskStatus};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

/// The config directory's name under the platform's config home, and the
/// two files `iter` keeps in it.
const APP_DIR: &str = "iter";
const CONFIG_FILE: &str = "iter.yaml";
const DB_FILE: &str = "iter.db";

/// Gaps between sessions shorter than this many minutes count as one continuous span.
pub const DEFAULT_PAUSE_GAP_MINUTES: i64 = 17;

/// The editor opened to fill in a new/edited organization, project or task.
pub const DEFAULT_EDITOR: &str = "nvim";

fn default_pause_gap_minutes() -> i64 {
    DEFAULT_PAUSE_GAP_MINUTES
}

fn default_editor() -> String {
    DEFAULT_EDITOR.to_string()
}

/// Settings a new task starts from; `branch_prefix` comes from the project's `branch_template`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TaskDefaults {
    pub status: TaskStatus,
}

/// What a headless agent (`iter task solve`, `iter wiki ingest`) may do. Nobody can approve a
/// command, so anything not allowed here or by the harness's settings is refused.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentConfig {
    /// Shell commands the agent may run, in the harness's own syntax
    /// (`Bash(git *)` for Claude). A command that a hook rewrites (`rtk git
    /// status`) is matched after the rewrite: add `Bash(rtk *)` then.
    pub allowed_tools: Vec<String>,
    /// The models a form offers, when not the harness's own list.
    pub models: Vec<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        AgentConfig {
            models: Vec::new(),
            allowed_tools: [
                // what the prompts tell an agent to run: its plan, the wiki, the notes
                "Bash(iter task plan *)",
                "Bash(iter task prompt *)",
                "Bash(iter wiki find *)",
                "Bash(iter wiki grep *)",
                "Bash(iter wiki show *)",
                "Bash(iter wiki neighbors *)",
                "Bash(iter wiki check *)",
                "Bash(iter wiki schema *)",
                "Bash(iter notes *)",
                // and git, to look at its work and commit it (not to push or rewrite)
                "Bash(git status *)",
                "Bash(git diff *)",
                "Bash(git log *)",
                "Bash(git show *)",
                "Bash(git add *)",
                "Bash(git commit *)",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Where the SQLite file lives; `None` means `iter.db` beside the config. A leading `~` is expanded.
    #[serde(skip_serializing_if = "Option::is_none")]
    db_path: Option<String>,

    #[serde(default = "default_pause_gap_minutes")]
    pub pause_gap_minutes: i64,

    #[serde(default = "default_editor")]
    pub editor: String,

    pub project: Settings,
    pub task: TaskDefaults,
    pub agent: AgentConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            db_path: None,
            pause_gap_minutes: default_pause_gap_minutes(),
            editor: default_editor(),
            project: Settings::default(),
            task: TaskDefaults::default(),
            agent: AgentConfig::default(),
        }
    }
}

impl Config {
    /// Reads the config file. A missing, empty or all-comments file gives the defaults; an
    /// unparseable one is fatal, so a typo can't quietly move the database.
    pub fn load() -> Result<Self> {
        let path = config_file()?;
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
            Err(source) => {
                return Err(IterError::ConfigUnreadable {
                    path: path.display().to_string(),
                    source,
                });
            }
        };
        parse(&text).map_err(|source| IterError::InvalidConfig {
            path: path.display().to_string(),
            source,
        })
    }

    /// Writes every key (creating the directory), making implicit values explicit. Comments in the
    /// file are not kept.
    #[cfg(feature = "web")]
    pub fn save(&self) -> Result<()> {
        let path = config_file()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let body = serde_yaml::to_string(self)
            .map_err(|e| IterError::CommandFailed(format!("could not write the config: {e}")))?;
        let text = format!(
            "# iter's config. Saved from the settings page; comments are not kept.\n{body}"
        );
        crate::agent::taskdir::write_atomic(&path, &text)?;
        Ok(())
    }

    pub fn db_path(&self) -> Result<PathBuf> {
        Ok(self.db_path_within(&config_dir()?, home_dir().as_deref()))
    }

    fn db_path_within(&self, config_dir: &Path, home: Option<&Path>) -> PathBuf {
        match &self.db_path {
            Some(path) => expand_tilde(path, home),
            None => config_dir.join(DB_FILE),
        }
    }
}

/// Parses config text. Going through `Option` makes an empty or all-comments file (YAML null) the
/// defaults, not an "invalid type: unit value" error.
fn parse(text: &str) -> std::result::Result<Config, serde_yaml::Error> {
    Ok(serde_yaml::from_str::<Option<Config>>(text)?.unwrap_or_default())
}

/// The process's config, replaceable by the settings page; each value is leaked once so
/// `config()` can hand out `&'static` (a few hundred bytes per save).
static CONFIG: OnceLock<RwLock<&'static Config>> = OnceLock::new();

/// Read on first use and held. An unreadable config is fatal here: nothing downstream works
/// without knowing where the database is.
pub fn config() -> &'static Config {
    let cell = CONFIG.get_or_init(|| {
        let config = Config::load().unwrap_or_else(|e| {
            eprintln!("error: {e}");
            std::process::exit(1);
        });
        RwLock::new(Box::leak(Box::new(config)))
    });
    *cell.read().unwrap_or_else(|e| e.into_inner())
}

/// Makes `new` the process's config after the settings page saved it.
#[cfg(feature = "web")]
pub fn replace(new: Config) {
    config();
    if let Some(cell) = CONFIG.get() {
        *cell.write().unwrap_or_else(|e| e.into_inner()) = Box::leak(Box::new(new));
    }
}

pub fn config_dir() -> Result<PathBuf> {
    Ok(config_home().ok_or(IterError::NoConfigDir)?.join(APP_DIR))
}

pub fn config_file() -> Result<PathBuf> {
    Ok(config_dir()?.join(CONFIG_FILE))
}

/// Per-user config home: `$XDG_CONFIG_HOME` or `$HOME/.config` on unix, `%APPDATA%` elsewhere.
#[cfg(unix)]
fn config_home() -> Option<PathBuf> {
    var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".config")))
}

#[cfg(not(unix))]
fn config_home() -> Option<PathBuf> {
    var("APPDATA")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".config")))
}

#[cfg(unix)]
fn home_dir() -> Option<PathBuf> {
    var("HOME").map(PathBuf::from)
}

#[cfg(not(unix))]
fn home_dir() -> Option<PathBuf> {
    var("USERPROFILE").map(PathBuf::from)
}

/// An environment variable's value, treating "set but empty" as unset --
/// an empty `$HOME` names the filesystem root, never what the user meant.
fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

pub(crate) fn expand_home(path: &str) -> PathBuf {
    expand_tilde(path, home_dir().as_deref())
}

/// Expands a leading `~` (the shell doesn't in a config file); a mid-path `~` is left alone.
fn expand_tilde(path: &str, home: Option<&Path>) -> PathBuf {
    let Some(home) = home else {
        return PathBuf::from(path);
    };
    match path {
        "~" => home.to_path_buf(),
        _ => match path.strip_prefix("~/") {
            Some(rest) => home.join(rest),
            None => PathBuf::from(path),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_yaml(yaml: &str) -> Config {
        parse(yaml).expect("config parses")
    }

    #[test]
    fn an_absent_key_keeps_its_default() {
        let config = from_yaml("pause_gap_minutes: 42\n");
        assert_eq!(config.pause_gap_minutes, 42);
        assert_eq!(config.editor, DEFAULT_EDITOR);
        assert!(config.project.tmux);
        assert_eq!(config.project.branch_template, "feat/{task}");
        assert_eq!(config.task.status, TaskStatus::Queue);
        assert_eq!(config.db_path, None);
    }

    #[test]
    fn a_nested_default_block_overrides_only_what_it_names() {
        let config = from_yaml("project:\n  github: true\n  branch_template: fix/{task}\n");
        assert!(config.project.github);
        assert_eq!(config.project.branch_template, "fix/{task}");
        assert!(config.project.tmux);
        assert!(config.project.auto_branch);
        assert_eq!(config.project.github_project, "");
    }

    #[test]
    fn a_github_project_board_can_be_defaulted() {
        let config = from_yaml("project:\n  github_project: Roadmap\n");
        assert_eq!(config.project.github_project, "Roadmap");
        let project = crate::models::Project::template(&config.project);
        assert_eq!(project.github_project, "Roadmap");
        assert_eq!(
            crate::models::Project::template(&Settings::default()).github_project,
            ""
        );
    }

    #[test]
    fn a_task_status_default_parses_by_name() {
        assert_eq!(
            from_yaml("task:\n  status: wip\n").task.status,
            TaskStatus::Wip
        );
    }

    /// A mistyped key is reported, not silently ignored.
    #[test]
    fn an_unknown_key_is_rejected() {
        assert!(parse("pause_gap: 5\n").is_err());
        assert!(parse("project:\n  tmuxx: true\n").is_err());
    }

    #[test]
    fn the_database_sits_beside_the_config_file_by_default() {
        let config = Config::default();
        assert_eq!(
            config.db_path_within(
                Path::new("/home/u/.config/iter"),
                Some(Path::new("/home/u"))
            ),
            PathBuf::from("/home/u/.config/iter/iter.db")
        );
    }

    #[test]
    fn a_configured_database_path_wins_and_expands_a_leading_tilde() {
        let config = from_yaml("db_path: ~/work/iter.db\n");
        assert_eq!(
            config.db_path_within(
                Path::new("/home/u/.config/iter"),
                Some(Path::new("/home/u"))
            ),
            PathBuf::from("/home/u/work/iter.db")
        );
        let absolute = from_yaml("db_path: /srv/iter.db\n");
        assert_eq!(
            absolute.db_path_within(
                Path::new("/home/u/.config/iter"),
                Some(Path::new("/home/u"))
            ),
            PathBuf::from("/srv/iter.db")
        );
    }

    #[test]
    fn only_a_leading_tilde_is_a_home_directory() {
        let home = Some(Path::new("/home/u"));
        assert_eq!(expand_tilde("~", home), PathBuf::from("/home/u"));
        assert_eq!(expand_tilde("~/db", home), PathBuf::from("/home/u/db"));
        assert_eq!(expand_tilde("/a/~/db", home), PathBuf::from("/a/~/db"));
        assert_eq!(expand_tilde("~notme/db", home), PathBuf::from("~notme/db"));
        // No home to expand against -- better a path that fails loudly
        // than one silently rooted somewhere else.
        assert_eq!(expand_tilde("~/db", None), PathBuf::from("~/db"));
    }

    #[test]
    fn a_comment_only_file_is_not_a_parse_error() {
        assert_eq!(
            from_yaml("# nothing set yet\n").pause_gap_minutes,
            DEFAULT_PAUSE_GAP_MINUTES
        );
        assert_eq!(from_yaml("").editor, DEFAULT_EDITOR);
    }

    #[test]
    fn a_config_written_reads_back_the_same() {
        let mut config = Config {
            editor: "vim".into(),
            pause_gap_minutes: 30,
            ..Config::default()
        };
        config.task.status = TaskStatus::Wip;
        config.project.github = true;
        let text = serde_yaml::to_string(&config).expect("writes");
        let back = parse(&text).expect("reads");
        assert_eq!(back.editor, "vim");
        assert_eq!(back.pause_gap_minutes, 30);
        assert_eq!(back.task.status, TaskStatus::Wip);
        assert_eq!(back.project, config.project);
        assert!(
            !text.contains("db_path"),
            "an unset database path is not written"
        );
    }
}
