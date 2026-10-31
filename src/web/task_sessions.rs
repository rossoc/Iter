//! Adding a session by hand to a task: the pop-up over the Sessions tab (`?tab=sessions&new=session`) and its route.

use super::ui::field::{field, textarea};
use super::ui::form::form_shell;
use super::ui::modal::modal;
use super::url::{sessions_tab_url, task_url};
use super::{Id, open_db};
use crate::models::{Session, Task};
use chrono::NaiveDateTime;
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        content::Form,
        error::{RouterErrorExt, bad_request, see_other},
        path_param,
        response::Response,
        route,
    },
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.route(add_session)
}

#[component]
pub async fn session_modal(task_id: i64) -> Result<impl View> {
    let close = sessions_tab_url(&task_url(task_id));
    let action = format!("{}/sessions", task_url(task_id));
    Ok(view! {
        modal(id: "session-title", title: "New session", close: close.as_str(),
            form_shell(action: action.as_str(), error: None, cancel: close.as_str(), submit_label: "Add session",
                <div class="compose-pick">
                    field(name: "start", label: "Start", value: "", input_type: "datetime-local", required: true, autofocus: true)
                    field(name: "end", label: "End", value: "", input_type: "datetime-local", required: true)
                </div>
                textarea(name: "message", label: "Note", value: "")
            )
        )
    })
}

#[derive(Deserialize)]
struct SessionForm {
    start: String,
    end: String,
    #[serde(default)]
    message: String,
}

fn moment(text: &str) -> std::result::Result<NaiveDateTime, String> {
    ["%Y-%m-%dT%H:%M", "%Y-%m-%dT%H:%M:%S"]
        .iter()
        .find_map(|f| NaiveDateTime::parse_from_str(text.trim(), f).ok())
        .ok_or_else(|| format!("'{text}' is not a date and time"))
}

fn parse(task_id: i64, form: &SessionForm) -> std::result::Result<Session, String> {
    let (start, end) = (moment(&form.start)?, moment(&form.end)?);
    if end <= start {
        return Err("a session must end after it starts".into());
    }
    let message = form.message.trim();
    Ok(Session {
        id: None,
        task_id,
        start,
        end: Some(end),
        message: (!message.is_empty()).then(|| message.to_string()),
    })
}

#[route(POST "/task/{id}/sessions")]
async fn add_session(cx: &Cx, Form(form): Form<SessionForm>) -> Result<Response> {
    let id = *path_param::<Id>(cx)?;
    let db = open_db()?;
    db.get::<Task>(id)?.ok_or_not_found()?;
    let session = parse(id, &form).map_err(bad_request)?;
    db.insert(&session)?;
    Err(see_other(sessions_tab_url(&task_url(id))).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(start: &str, end: &str, message: &str) -> SessionForm {
        SessionForm {
            start: start.into(),
            end: end.into(),
            message: message.into(),
        }
    }

    #[test]
    fn a_session_needs_two_moments_in_order() {
        let s = parse(7, &form("2026-10-30T09:00", "2026-10-30T10:30", " did it ")).unwrap();
        assert_eq!((s.task_id, s.message.as_deref()), (7, Some("did it")));
        assert_eq!((s.end.unwrap() - s.start).num_minutes(), 90);
        assert!(parse(7, &form("2026-10-30T09:00", "2026-10-30T09:00", "")).is_err());
        assert!(parse(7, &form("2026-10-30T10:00", "2026-10-30T09:00", "")).is_err());
        assert!(parse(7, &form("yesterday", "2026-10-30T09:00", "")).is_err());
        assert_eq!(
            parse(7, &form("2026-10-30T09:00:30", "2026-10-30T10:00", ""))
                .unwrap()
                .message,
            None
        );
    }
}
