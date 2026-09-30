//! The overview card (Home: an organization and its projects; Boards: a
//! board and its projects) and the grid that holds the cards.

use super::empty_line::empty_line;
use super::project_list::{ProjectItem, project_list};
use super::{ARROW, icon};
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// What a card with no projects says.
pub const NO_PROJECTS: &str = "No projects yet.";

/// The cards of a page, as a responsive grid.
#[component]
pub async fn card_grid(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <section class="grid">(child)</section> })
}

/// `title` links to `href` when there is one (the arrow shows it leaves the
/// overview); `note` is a one-line summary, left out when empty. With no
/// `items` the card says `none`.
#[component]
pub async fn group_card(
    title: &str,
    href: Option<String>,
    note: &str,
    items: &[ProjectItem<'_>],
    none: &str,
) -> Result<impl View> {
    Ok(view! {
        <article class="group">
            <header>
                if let Some(href) = &href {
                    <h2><a href=(href.as_str())><span class="u">(title)</span> (icon(ARROW))</a></h2>
                } else {
                    <h2>(title)</h2>
                }
            </header>
            if !note.is_empty() {
                <p class="note">(note)</p>
            }
            if items.is_empty() {
                empty_line((none))
            } else {
                project_list(items: items, compact: true)
            }
        </article>
    })
}
