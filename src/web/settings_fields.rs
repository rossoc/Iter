//! The inputs of the Settings group, shared by the organization and project
//! forms: `SettingsForm` read back, and the write-side twin of
//! `settings_panel.rs`. The names are `SettingsForm`'s. The hints follow the
//! pattern of every form: what it is, then "Leave empty for X" when empty is
//! allowed.

use super::forms::SettingsForm as S;
use super::ui::field::{checkbox, field};
use super::ui::group::group;
use crate::models::Settings;
use topcoat::{
    Result,
    view::{View, component, view},
};

const BRANCH_TEMPLATE_HINT: &str =
    "How the branch of a new task is named, such as feat/{task}. Leave empty for none.";
const DEFAULT_BRANCH_HINT: &str =
    "The branch that finished tasks are merged into, such as main. It is needed to save a task.";
const GITHUB_PROJECT_HINT: &str =
    "The title of the GitHub Project that new issues are filed under. Leave empty for none.";

#[component]
pub async fn settings_fields(s: &Settings) -> Result<impl View> {
    Ok(view! {
        group(
            title: "Settings",
            checkbox(name: S::GITHUB, label: "GitHub", on: s.github)
            checkbox(name: S::TMUX, label: "tmux", on: s.tmux)
            checkbox(name: S::AUTO_BRANCH, label: "Auto branch", on: s.auto_branch)
            field(name: S::BRANCH_TEMPLATE, label: "Branch template", value: &s.branch_template, identifier: true, hint: BRANCH_TEMPLATE_HINT)
            field(name: S::DEFAULT_BRANCH, label: "Default branch", value: &s.default_branch, identifier: true, hint: DEFAULT_BRANCH_HINT)
            field(name: S::GITHUB_PROJECT, label: "GitHub project", value: &s.github_project, identifier: true, hint: GITHUB_PROJECT_HINT)
        )
    })
}
