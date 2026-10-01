//! The folded Unscheduled column: a thin strip on the column's own edge
//! that says what is hidden (the name, how many tasks wait, a dot when some
//! are overdue). A link back to the open column, and a drop zone: a card
//! dropped on it is unscheduled, as on the open lists.

use super::{PANEL_RIGHT, icon};
use topcoat::{
    Result,
    view::{View, component, view},
};

/// The screen-reader name: "Show Backlog, 3 tasks, 1 overdue" (`total`
/// counts the overdue ones too).
fn name(label: &str, total: usize, overdue: usize) -> String {
    let mut name = format!("Show {label}, {total} tasks");
    if overdue > 0 {
        name.push_str(&format!(", {overdue} overdue"));
    }
    name
}

/// The button that folds the open column, at the top right of it (on the
/// first heading's row): `href` is the page with the column folded.
#[component]
pub async fn side_fold(#[into] href: String, label: &str) -> Result<impl View> {
    let name = format!("Hide {label}");
    Ok(view! {
        <div class="side-head">
            <a class="button side-fold" href=(href.as_str()) aria-label=(name.as_str()) title=(name.as_str())>(icon(PANEL_RIGHT))</a>
        </div>
    })
}

/// `href` opens the column; `label` names it ("Backlog"); `target` is what a
/// card dropped on the rail is posted as (empty on the agenda: unschedule).
#[component]
pub async fn side_rail(
    #[into] href: String,
    label: &str,
    total: usize,
    overdue: usize,
    #[default] target: &str,
) -> Result<impl View> {
    let name = name(label, total, overdue);
    Ok(view! {
        <a class="drop side-rail" href=(href.as_str()) data-target=(target) aria-label=(name.as_str()) title=(name.as_str())>
            (icon(PANEL_RIGHT))
            <span class="rail-text" aria-hidden="true">(label) " · " (total)</span>
            if overdue > 0 {
                <span class="rail-dot" aria-hidden="true"></span>
            }
        </a>
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_says_the_count_and_what_is_overdue() {
        assert_eq!(name("Backlog", 3, 0), "Show Backlog, 3 tasks");
        assert_eq!(name("Backlog", 3, 1), "Show Backlog, 3 tasks, 1 overdue");
    }
}
