//! The Settings aside of an organization or a project: `Settings` mapped
//! onto the generic `ui::panel` rows.

use super::ui::panel::{Row, panel};
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
    let rows = [
        Row::Flag("GitHub", s.github),
        Row::Flag("tmux", s.tmux),
        Row::Flag("Auto branch", s.auto_branch),
        Row::Text("Branch template", s.branch_template),
        Row::Text("Default branch", s.default_branch),
        Row::Text("GitHub project", s.github_project),
    ];
    Ok(view! {
        panel(id: id, title: "Settings", rows: &rows)
    })
}
