//! The Settings aside of an organization or a project: `Settings` mapped
//! onto the generic `ui::panel` rows.

use super::ui::panel::{flag_row, panel, text_row};
use crate::models::Settings;
use topcoat::{
    Result,
    view::{View, component, view},
};

#[component]
pub async fn settings_panel(
    s: Settings,
    #[default("settings-title")] id: &str,
) -> Result<impl View> {
    Ok(view! {
        panel(
            id: id,
            title: "Settings",
            flag_row(label: "GitHub", on: s.github)
            flag_row(label: "tmux", on: s.tmux)
            flag_row(label: "Auto branch", on: s.auto_branch)
            text_row(label: "Branch template", value: &s.branch_template)
            text_row(label: "Default branch", value: &s.default_branch)
            text_row(label: "GitHub project", value: &s.github_project)
        )
    })
}
