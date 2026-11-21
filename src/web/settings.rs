//! The global settings page (`/settings`): the config file's defaults as a form; saving writes the config file.

use super::forms::SettingsForm;
use super::settings_fields::settings_fields;
use super::ui::field::{checkbox, field, select};
use super::ui::group::group;
use super::ui::form::form_shell;
use super::ui::form_error::FormError;
use super::ui::frame::frame;
use super::ui::notice::notice;
use super::ui::page_header::{Kicker, page_header};
use super::url::{SETTINGS, settings_saved_url};
use crate::config::{Config, NotificationConfig, config_file, replace};
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

const EDITOR: &str = "editor";
const PAUSE_GAP: &str = "pause_gap_minutes";
const STATUS: &str = "status";
const DEFAULT_BRANCH: &str = "default_branch";
const NOTIFICATIONS: &str = "notifications";
const NOTIFICATION_SECONDS: &str = "notification_seconds";
/// The longest a toast may stay, in seconds; the shortest is one.
const MAX_NOTIFICATION_SECONDS: u32 = 60;

#[query_params(error = bad_request)]
struct SavedQuery {
    saved: Option<String>,
}

#[derive(Deserialize)]
struct SettingsPage {
    editor: String,
    pause_gap_minutes: String,
    status: String,
    /// The "Notifications" checkbox: present (`on`) only when ticked.
    #[serde(default, deserialize_with = "super::forms::ticked")]
    notifications: bool,
    #[serde(default)]
    notification_seconds: String,
    #[serde(flatten)]
    settings: SettingsForm,
}

impl SettingsPage {
    /// Nothing is changed when it is refused.
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
        let seconds = self
            .notification_seconds
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=MAX_NOTIFICATION_SECONDS).contains(n))
            .ok_or_else(|| {
                bad(
                    NOTIFICATION_SECONDS,
                    format!(
                        "Seconds shown '{}' is not valid. Use a whole number from 1 to {MAX_NOTIFICATION_SECONDS}, such as 5.",
                        self.notification_seconds.trim()
                    ),
                )
            })?;
        config.editor = editor.to_string();
        config.pause_gap_minutes = gap;
        config.task.status = status;
        config.project = settings;
        config.notifications = NotificationConfig {
            enabled: self.notifications,
            seconds,
        };
        Ok(config)
    }
}

struct Values {
    editor: String,
    pause_gap: String,
    status: String,
    notifications: bool,
    notification_seconds: String,
    settings: Settings,
}

impl Values {
    fn of(config: &Config) -> Values {
        Values {
            editor: config.editor.clone(),
            pause_gap: config.pause_gap_minutes.to_string(),
            status: config.task.status.as_str().to_string(),
            notifications: config.notifications.enabled,
            notification_seconds: config.notifications.seconds.to_string(),
            settings: config.project.clone(),
        }
    }

    fn typed(page: &SettingsPage) -> Values {
        Values {
            editor: page.editor.clone(),
            pause_gap: page.pause_gap_minutes.clone(),
            status: page.status.clone(),
            notifications: page.notifications,
            notification_seconds: page.notification_seconds.clone(),
            settings: page.settings.settings(),
        }
    }
}

#[page("/settings")]
async fn show(cx: &Cx) -> Result<impl View> {
    // the toast says "Settings saved."; with the toasts off, this notice says it
    let saved = query_params::<SavedQuery>(cx)?.saved.is_some()
        && !crate::config::config().notifications.enabled;
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
            error: error,
            current: SETTINGS,
            page_header(title: "Settings", kicker: Kicker::Eyebrow("Global"))
            if saved {
                notice(id: "saved", <span>"Saved."</span>)
            }
            form_shell(action: SETTINGS, error: error, cancel: "/",
                field(name: EDITOR, label: "Editor", value: &values.editor, required: true, identifier: true, error: error,
                    hint: "Opened to fill in a new or edited organization, project or task in the terminal.")
                field(name: PAUSE_GAP, label: "Pause gap (minutes)", value: &values.pause_gap, required: true, numeric: true, error: error,
                    hint: "A gap between sessions of this many minutes or less counts as a pause within one span, not a break.")
                select(name: STATUS, label: "Status of a new task", options: &statuses, current: &values.status, error: error)
                settings_fields(s: &values.settings)
                group(title: "Notifications",
                    checkbox(name: NOTIFICATIONS, label: "Notifications", on: values.notifications)
                    field(name: NOTIFICATION_SECONDS, label: "Seconds shown", value: &values.notification_seconds, required: true, numeric: true, error: error,
                        hint: "How long a notification stays in the top right corner before it goes, from 1 to 60.")
                )
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
            notifications: true,
            notification_seconds: "5".into(),
            settings: SettingsForm {
                github: true,
                tmux: false,
                auto_branch: false,
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

    /// The `name` of the "Seconds shown" field (R2). A literal, so the tests compile before it exists.
    const SECONDS: &str = "notification_seconds";

    /// A submission as the browser posts it: a valid form plus `extra`; the "Notifications"
    /// checkbox is the field `notifications`, absent when unticked.
    fn posted(extra: &[(&str, &str)]) -> SettingsPage {
        let mut form = serde_json::json!({
            "editor": "vim", "pause_gap_minutes": "17", "status": "queue",
            "branch_template": "feat/{task}", "default_branch": "main", "github_project": "",
        });
        for (k, v) in extra {
            form[*k] = serde_json::Value::from(*v);
        }
        serde_json::from_value(form).expect("the form deserializes")
    }

    fn notifications(config: &Config) -> serde_yaml::Value {
        serde_yaml::to_value(config).expect("writes")["notifications"].clone()
    }

    /// R2: the checkbox and the seconds are saved into the config.
    #[test]
    fn the_notification_fields_change_the_config() {
        let on = posted(&[("notifications", "on"), (SECONDS, " 12 ")])
            .apply(Config::default())
            .expect("valid");
        assert_eq!(notifications(&on)["enabled"], serde_yaml::Value::Bool(true));
        assert_eq!(notifications(&on)["seconds"], serde_yaml::Value::from(12u32));

        let off = posted(&[(SECONDS, "5")])
            .apply(Config::default())
            .expect("valid");
        assert_eq!(
            notifications(&off)["enabled"],
            serde_yaml::Value::Bool(false),
            "an unticked box turns the toasts off"
        );
    }

    /// R2: the seconds are a whole number from 1 to 60, else refused on that field.
    #[test]
    fn seconds_outside_one_to_sixty_are_refused_on_their_field() {
        for bad in ["0", "61", "x", "-3", "2.5", ""] {
            let refused = posted(&[("notifications", "on"), (SECONDS, bad)])
                .apply(Config::default())
                .err()
                .map(|(_, f)| f);
            assert_eq!(refused, Some(SECONDS), "seconds {bad:?}");
        }
        for good in ["1", "60"] {
            assert!(
                posted(&[("notifications", "on"), (SECONDS, good)])
                    .apply(Config::default())
                    .is_ok(),
                "seconds {good:?}"
            );
        }
    }
}
