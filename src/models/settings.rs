//! Branch and tool settings shared by organizations and projects; the config file's `project:`
//! block is the default when there is no organization.

use crate::models::{default_branch_template, default_true, main_branch};
use serde::{Deserialize, Serialize};

/// The settings a new project or organization starts from. Only the starting point of a `new`:
/// existing projects are untouched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub github: bool,
    pub tmux: bool,
    pub auto_branch: bool,
    pub branch_template: String,

    /// The branch `iter task done --save` merges into.
    pub default_branch: String,

    /// The GitHub Project board new issues are filed under, by title; empty files them nowhere.
    pub github_project: String,
}

impl Default for Settings {
    fn default() -> Self {
        // Same fallbacks as serde's missing-field defaults, so config and editor buffer agree.
        Settings {
            github: false,
            tmux: default_true(),
            auto_branch: default_true(),
            branch_template: default_branch_template(),
            default_branch: main_branch(),
            github_project: String::new(),
        }
    }
}

/// A type carrying the six [`Settings`] fields as its own columns.
pub(crate) trait Configured {
    fn settings(&self) -> Settings;
    fn set_settings(&mut self, settings: Settings);
}

/// Implements [`Configured`] -- the one place the six fields are listed
/// outside the structs themselves.
macro_rules! configured {
    ($($ty:ty),*) => {$(
        impl Configured for $ty {
            fn settings(&self) -> Settings {
                Settings {
                    github: self.github,
                    tmux: self.tmux,
                    auto_branch: self.auto_branch,
                    branch_template: self.branch_template.clone(),
                    default_branch: self.default_branch.clone(),
                    github_project: self.github_project.clone(),
                }
            }

            fn set_settings(&mut self, settings: Settings) {
                let Settings {
                    github,
                    tmux,
                    auto_branch,
                    branch_template,
                    default_branch,
                    github_project,
                } = settings;
                self.github = github;
                self.tmux = tmux;
                self.auto_branch = auto_branch;
                self.branch_template = branch_template;
                self.default_branch = default_branch;
                self.github_project = github_project;
            }
        }
    )*};
}

configured!(crate::models::Organization, crate::models::Project);
