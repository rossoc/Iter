//! The user's config file: where the database lives, and what the values
//! `iter` would otherwise hardcode are set to.
//!
//! It's read from `iter.yaml` in `iter`'s config directory -- `$XDG_CONFIG_HOME/iter`,
//! or `$HOME/.config/iter` when that isn't set (`%APPDATA%\iter` off unix) --
//! and every key is optional: a missing file, an empty one, or one that
//! only sets `pause_gap_minutes` all leave the rest at the built-in
//! defaults below. The database sits next to the config file by default, so
//! the two live together and moving the directory moves both.

use crate::error::{IterError, Result};
use crate::models::{TaskStatus, default_branch_template, default_true};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The config directory's name under the platform's config home, and the
/// two files `iter` keeps in it.
const APP_DIR: &str = "iter";
const CONFIG_FILE: &str = "iter.yaml";
const DB_FILE: &str = "iter.db";

/// A gap between one session's end and the next one's start shorter than
/// this many minutes is treated as a pause within one continuous span (e.g.
/// a quick interruption) rather than a real break between sessions.
pub const DEFAULT_PAUSE_GAP_MINUTES: i64 = 17;

/// The editor opened to fill in a new/edited organization, project or task.
pub const DEFAULT_EDITOR: &str = "nvim";

fn default_pause_gap_minutes() -> i64 {
    DEFAULT_PAUSE_GAP_MINUTES
}

fn default_editor() -> String {
    DEFAULT_EDITOR.to_string()
}

/// The settings a new project or organization starts from, before the
/// editor opens -- an organization's fields *are* the defaults its projects
/// inherit, so `iter organization new` starts from this same block.
///
/// These are only the starting point of a `new`: they're written into the
/// buffer as ordinary front matter and can be changed there, and they never
/// touch a project that already exists.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectDefaults {
    pub github: bool,
    pub tmux: bool,
    pub auto_branch: bool,
    pub branch_template: String,

    /// The GitHub Project board new issues get filed under, by title.
    /// Empty -- the default -- files them nowhere. Worth setting here for
    /// anyone whose boards are named the same across repos; anyone whose
    /// aren't sets it per project (or on the organization) instead.
    pub github_project: String,
}

impl Default for ProjectDefaults {
    fn default() -> Self {
        // Deliberately the same fallbacks serde fills a missing front
        // matter field with, so "left out of the config" and "left out of
        // the editor buffer" can't come to mean two different things.
        ProjectDefaults {
            github: false,
            tmux: default_true(),
            auto_branch: default_true(),
            branch_template: default_branch_template(),
            github_project: String::new(),
        }
    }
}

/// The settings a new task starts from. `branch_prefix` isn't here: it's
/// read off the owning project's `branch_template`, which is the project
/// default that already covers it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TaskDefaults {
    pub status: TaskStatus,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Where the SQLite file lives. `None` -- the default -- means
    /// `iter.db` beside the config file. A leading `~` is expanded; a
    /// relative path is left as it is, resolved against the current
    /// directory the way any other relative path would be.
    db_path: Option<String>,

    /// See [`DEFAULT_PAUSE_GAP_MINUTES`].
    #[serde(default = "default_pause_gap_minutes")]
    pub pause_gap_minutes: i64,

    /// The editor `new`/`edit` opens the markdown buffer in.
    #[serde(default = "default_editor")]
    pub editor: String,

    pub project: ProjectDefaults,
    pub task: TaskDefaults,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            db_path: None,
            pause_gap_minutes: default_pause_gap_minutes(),
            editor: default_editor(),
            project: ProjectDefaults::default(),
            task: TaskDefaults::default(),
        }
    }
}

impl Config {
    /// Reads the config file. A file that isn't there at all is not an
    /// error -- `iter` has always run without one -- and neither is an
    /// empty (or all-comments) one; both give the built-in defaults. A file
    /// that *is* there but doesn't parse is fatal rather than silently
    /// ignored, so a typo can't quietly put the database somewhere else.
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

    /// The database file to open: `db_path` if it's set, and otherwise
    /// `iter.db` in the config directory.
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

/// Parses the config file's text. Deserializing through `Option` is what
/// makes an empty file -- or one that's still nothing but comments -- the
/// defaults rather than a "invalid type: unit value" error: YAML reads both
/// as null.
fn parse(text: &str) -> std::result::Result<Config, serde_yaml::Error> {
    Ok(serde_yaml::from_str::<Option<Config>>(text)?.unwrap_or_default())
}

static CONFIG: OnceLock<Config> = OnceLock::new();

/// The process's config, read from disk the first time it's asked for and
/// held from then on -- every command, and every completion callback the
/// shell fires, sees the same one. A config that can't be read is fatal
/// here rather than propagated: nothing downstream can do anything useful
/// without knowing where the database is.
pub fn config() -> &'static Config {
    CONFIG.get_or_init(|| {
        Config::load().unwrap_or_else(|e| {
            eprintln!("error: {e}");
            std::process::exit(1);
        })
    })
}

/// `iter`'s own config directory, `<config home>/iter`.
pub fn config_dir() -> Result<PathBuf> {
    Ok(config_home().ok_or(IterError::NoConfigDir)?.join(APP_DIR))
}

/// The config file itself, `<config home>/iter/iter.yaml`.
pub fn config_file() -> Result<PathBuf> {
    Ok(config_dir()?.join(CONFIG_FILE))
}

/// The directory per-user config lives under. On unix that's the XDG one --
/// `$XDG_CONFIG_HOME`, or `$HOME/.config`, which is what the variable
/// defaults to when it isn't set -- so `iter` follows wherever the user has
/// pointed it. Elsewhere it's `%APPDATA%`, the equivalent convention.
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

/// Expands a leading `~` (the shell would have done it for a path typed on
/// the command line, but not for one written in a config file). Anything
/// else -- including a `~` mid-path -- is left alone.
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

    /// The whole file is optional, and so is every key in it -- a config
    /// that sets one thing must not reset everything else to nothing.
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
        // Untouched by the block that set the two above.
        assert!(config.project.tmux);
        assert!(config.project.auto_branch);
        assert_eq!(config.project.github_project, "");
    }

    /// `github_project` defaults to "file it nowhere", and travels into a
    /// new project the same way the rest of the block does.
    #[test]
    fn a_github_project_board_can_be_defaulted() {
        let config = from_yaml("project:\n  github_project: Roadmap\n");
        assert_eq!(config.project.github_project, "Roadmap");
        let project = crate::models::Project::template(&config.project);
        assert_eq!(project.github_project, "Roadmap");
        assert_eq!(
            crate::models::Project::template(&ProjectDefaults::default()).github_project,
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

    /// A mistyped key is a mistake worth reporting: silently ignoring it
    /// would leave the user with a setting they think they set.
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

    /// An empty file (or one that's nothing but comments) is a config the
    /// user has started but not filled in -- the defaults, not an error.
    #[test]
    fn a_comment_only_file_is_not_a_parse_error() {
        assert_eq!(
            from_yaml("# nothing set yet\n").pause_gap_minutes,
            DEFAULT_PAUSE_GAP_MINUTES
        );
        assert_eq!(from_yaml("").editor, DEFAULT_EDITOR);
    }
}
