//! The global settings page (`/settings`): the config file's defaults (the
//! editor, the pause gap, the status of a new task, the defaults of a new
//! project) as a plain form. Saving writes the config file and replaces the
//! running config, so the next page already follows it.

use super::forms::SettingsForm;
use super::settings_fields::settings_fields;
use super::ui::field::{field, select};
use super::ui::form::{form, required_note};
use super::ui::form_actions::form_actions;
use super::ui::form_error::FormError;
use super::ui::frame::frame;
use super::ui::notice::{error_box, notice};
use super::ui::page_header::{Kicker, page_header};
use super::url::{SETTINGS, settings_saved_url};
use crate::config::{Config, config_file, replace};
use crate::error::IterError;
use crate::models::{Settings, TaskStatus};
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, content::Form, error::see_other, page, query_params},
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(show).page(save)
}

/// The names of the controls an error can be about (also the submitted
/// fields).
const EDITOR: &str = "editor";
const PAUSE_GAP: &str = "pause_gap_minutes";
const STATUS: &str = "status";
const DEFAULT_BRANCH: &str = "default_branch";

#[query_params(error = bad_request)]
struct SavedQuery {
    saved: Option<String>,
}

/// What the page submits.
#[derive(Deserialize)]
struct SettingsPage {
    editor: String,
    pause_gap_minutes: String,
    status: String,
    #[serde(flatten)]
    settings: SettingsForm,
}

impl SettingsPage {
    /// `config` with the submission applied, or the refusal (the field it is
    /// about, and why). Nothing is changed when it is refused.
    fn apply(&self, mut config: Config) -> std::result::Result<Config, (IterError, &'static str)> {
        let bad = |about: &'static str, message: String| (IterError::CommandFailed(message), about);
        let editor = self.editor.trim();
        if editor.is_empty() {
            return Err(bad(EDITOR, "Editor cannot be empty.".into()));
        }
        let gap = self
            .pause_gap_minutes
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|n| *n >= 0)
            .ok_or_else(|| {
                bad(
                    PAUSE_GAP,
                    format!(
                        "Pause gap '{}' is not valid. Use a whole number of minutes, such as 17.",
                        self.pause_gap_minutes.trim()
                    ),
                )
            })?;
        let status = TaskStatus::parse(&self.status)
            .ok_or_else(|| (IterError::InvalidStatus(self.status.clone()), STATUS))?;
        let settings = self.settings.settings();
        if settings.default_branch.is_empty() {
            return Err(bad(
                DEFAULT_BRANCH,
                "Default branch cannot be empty.".into(),
            ));
        }
        config.editor = editor.to_string();
        config.pause_gap_minutes = gap;
        config.task.status = status;
        config.project = settings;
        Ok(config)
    }
}

/// What the form shows: the values, as text.
struct Values {
    editor: String,
    pause_gap: String,
    status: String,
    settings: Settings,
}

impl Values {
    fn of(config: &Config) -> Values {
        Values {
            editor: config.editor.clone(),
            pause_gap: config.pause_gap_minutes.to_string(),
            status: config.task.status.as_str().to_string(),
            settings: config.project.clone(),
        }
    }

    fn typed(page: &SettingsPage) -> Values {
        Values {
            editor: page.editor.clone(),
            pause_gap: page.pause_gap_minutes.clone(),
            status: page.status.clone(),
            settings: page.settings.settings(),
        }
    }
}

#[page("/settings")]
async fn show(cx: &Cx) -> Result<impl View> {
    let saved = query_params::<SavedQuery>(cx)?.saved.is_some();
    let values = Values::of(&Config::load()?);
    Ok(view! { screen(values: &values, saved: saved) })
}

#[page(POST "/settings")]
async fn save(Form(page): Form<SettingsPage>) -> Result<impl View> {
    let current = Config::load()?;
    match page.apply(current) {
        Ok(config) => {
            config.save()?;
            replace(config);
            Err(see_other(settings_saved_url()).into())
        }
        Err((error, about)) => {
            let error = FormError::new(&error, Some(about));
            let values = Values::typed(&page);
            Ok(view! { screen(values: &values, error: Some(&error)) })
        }
    }
}

#[component]
async fn screen(
    values: &Values,
    #[default] error: Option<&FormError>,
    #[default] saved: bool,
) -> Result<impl View> {
    let path = config_file()
        .map(|p| super::project_rows::tilde(&p.to_string_lossy()))
        .unwrap_or_default();
    let statuses = TaskStatus::options();
    Ok(view! {
        frame(
            title: &["Settings"],
            error: error.is_some(),
            current: SETTINGS,
            page_header(title: "Settings", kicker: Kicker::Eyebrow("Global"))
            if saved {
                notice(id: "saved", <span>"Saved."</span>)
            }
            form(
                action: SETTINGS,
                error_box(error: error)
                required_note()
                field(name: EDITOR, label: "Editor", value: &values.editor, required: true, identifier: true, error: error,
                    hint: "Opened to fill in a new or edited organization, project or task in the terminal.")
                field(name: PAUSE_GAP, label: "Pause gap (minutes)", value: &values.pause_gap, required: true, numeric: true, error: error,
                    hint: "A gap between sessions shorter than this counts as a pause within one span, not a break.")
                select(name: STATUS, label: "Status of a new task", options: &statuses, current: &values.status, error: error)
                settings_fields(s: &values.settings)
                form_actions(cancel: "/")
            )
            <p class="foot-note">"Saved to " <code>(path.as_str())</code>": every key is written, and comments in the file are not kept. The database location is not changed here."</p>
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(editor: &str, gap: &str, status: &str, trunk: &str) -> SettingsPage {
        SettingsPage {
            editor: editor.into(),
            pause_gap_minutes: gap.into(),
            status: status.into(),
            settings: SettingsForm {
                github: Some("on".into()),
                tmux: None,
                auto_branch: None,
                branch_template: "feat/{task}".into(),
                default_branch: trunk.into(),
                github_project: String::new(),
            },
        }
    }

    #[test]
    fn a_valid_submission_changes_the_config() {
        let config = page(" vim ", "20", "wip", "trunk")
            .apply(Config::default())
            .expect("valid");
        assert_eq!(config.editor, "vim");
        assert_eq!(config.pause_gap_minutes, 20);
        assert_eq!(config.task.status, TaskStatus::Wip);
        assert!(config.project.github && !config.project.tmux);
        assert_eq!(config.project.default_branch, "trunk");
    }

    #[test]
    fn a_bad_value_is_refused_on_its_field() {
        let refused = |p: SettingsPage| p.apply(Config::default()).err().map(|(_, f)| f);
        assert_eq!(refused(page("", "17", "queue", "main")), Some(EDITOR));
        assert_eq!(refused(page("vim", "x", "queue", "main")), Some(PAUSE_GAP));
        assert_eq!(refused(page("vim", "-1", "queue", "main")), Some(PAUSE_GAP));
        assert_eq!(refused(page("vim", "17", "nope", "main")), Some(STATUS));
        assert_eq!(
            refused(page("vim", "17", "queue", " ")),
            Some(DEFAULT_BRANCH)
        );
        assert_eq!(refused(page("vim", "17", "queue", "main")), None);
    }
}
