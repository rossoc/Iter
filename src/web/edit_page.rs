//! The skeleton every edit (and create) page shares: frame, page header with
//! the breadcrumb, the form with its error box, the Name and Description
//! controls every entity has, and Save / Cancel. A page supplies only what
//! is its own -- the crumbs, and the controls that come between Description
//! and the buttons (a relation, then Settings) -- as the child nodes.
//!
//! It lives here and not in `ui/` because it knows the forms' shared field
//! names (`forms::NAME`, `DESCRIPTION`); `ui/` knows no page code.

use super::forms::{DESCRIPTION, NAME};
use super::ui::breadcrumb::Crumb;
use super::ui::field::{field, textarea};
use super::ui::form::form_shell;
use super::ui::form_error::FormError;
use super::ui::frame::frame;
use super::ui::page_header::{Kicker, page_header};
use super::ui::tabs::{Tab, tabs};
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// The heading and the document title's parts of the page: "Edit {subject}"
/// for an edit, "New {kind}" for a create (`subject` is then what the new
/// one goes in).
fn heading(kind: &str, subject: &str, creating: bool) -> (String, [String; 2]) {
    if creating {
        let title = format!("New {kind}");
        (title.clone(), [title, subject.to_string()])
    } else {
        (
            format!("Edit {subject}"),
            [format!("Edit {kind}"), subject.to_string()],
        )
    }
}

/// The tab row of a page seen from its Settings tab: the tabs `entries`
/// names (label, link, badge: the counts the page itself shows), then
/// Settings, marked.
pub fn settings_row(base: &str, entries: &[(&'static str, String, Option<usize>)]) -> Vec<Tab> {
    entries
        .iter()
        .map(|(label, href, count)| {
            let tab = Tab::link(label, href.clone());
            match count {
                Some(n) => tab.count(*n),
                None => tab,
            }
        })
        .chain([super::sections::settings_tab(base, true)])
        .collect()
}

/// The frame, titled `head - subject - iter`. (A component only so the two
/// parts can be borrowed from its parameters: `frame` takes a slice of
/// `&str`.)
#[component]
async fn titled(
    head: &str,
    subject: &str,
    error: bool,
    current: &str,
    child: Child<'_>,
) -> Result<impl View> {
    let parts = [head, subject];
    Ok(view! {
        frame(title: &parts, error: error, current: current, (child))
    })
}

/// `kind` is what the page edits ("organization"); `subject` its stored name
/// (not what was typed), which the title uses. `name` and `description` are
/// what the form shows. `crumbs` is where the page sits (`crumbs::trail`, or the board's own);
/// `action` is where the form posts and `cancel` where Cancel goes. `error`
/// is the refusal, if any. `creating` is for a new entity: the heading is
/// "New {kind}", and `submit_label` names the button. `current` is the `href`
/// of the top bar item the page belongs to (Board pages), if any. `sections` is
/// the tab row of the page the form is the Settings tab of (an edit only).
#[component]
pub async fn edit_page(
    kind: &str,
    subject: &str,
    name: &str,
    description: &str,
    crumbs: &[Crumb],
    #[into] action: String,
    #[into] cancel: String,
    #[default] error: Option<&FormError>,
    #[default] creating: bool,
    #[default("Save")] submit_label: &str,
    #[default] current: &str,
    #[default] sections: Option<&[Tab]>,
    child: Child<'_>,
) -> Result<impl View> {
    let (mut title, mut parts) = heading(kind, subject, creating);
    // an edit with the page's tab row is the page's Settings tab: its name is the title
    if let (Some(_), false) = (sections, creating) {
        title = subject.to_string();
        parts = ["Settings".to_string(), subject.to_string()];
    }
    Ok(view! {
        titled(
            head: &parts[0],
            subject: &parts[1],
            error: error.is_some(),
            current: current,
            page_header(title: &title, kicker: Kicker::Crumbs(crumbs))
            if let Some(items) = sections {
                tabs(label: "Sections", items: items)
            }
            form_shell(action: action, error: error, cancel: cancel, submit_label: submit_label,
                field(name: NAME, label: "Name", value: name, required: true, identifier: true, error: error)
                textarea(name: DESCRIPTION, label: "Description", value: description)
                (child)
            )
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_row_keeps_the_badges_and_ends_with_settings() {
        let row = settings_row(
            "/org/1",
            &[
                ("Info", "/org/1".into(), None),
                ("Tasks", "/org/1?tab=tasks".into(), Some(3)),
            ],
        );
        assert_eq!(row.len(), 3);
        assert_eq!((row[0].badge(), row[1].badge()), (None, Some(3)));
        assert!(row[2].is_current() && row[2].href() == "/org/1/edit");
    }

    #[test]
    fn the_heading_says_edit_or_new() {
        let (title, parts) = heading("project", "app", false);
        assert_eq!(title, "Edit app");
        assert_eq!(parts, ["Edit project", "app"]);
        let (title, parts) = heading("task", "app", true);
        assert_eq!(title, "New task");
        assert_eq!(parts, ["New task", "app"]);
    }
}
