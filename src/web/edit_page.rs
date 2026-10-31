//! The skeleton of every edit and create page: frame, header, form with error box, Name and Description, Save / Cancel.

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

/// A component only so the two parts can borrow from its parameters (`frame` takes `&str`s).
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
