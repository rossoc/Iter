//! The top of the agenda, matrix and info pages: the heading (crumbs, the
//! board's name, Edit) and the section tabs (`board_header`).

use super::crumbs::boards_crumb;
use super::sections::tabs_for;
use super::ui::breadcrumb::Crumb;
use super::ui::button::edit_button;
use super::ui::page_header::{Kicker, page_header};
use super::ui::tabs::{Tab, tabs};
use super::url::{board_edit_url, board_info_url, board_matrix_url, board_url};
use crate::db::Table;
use crate::models::Board;
use topcoat::{
    Result,
    view::{View, component, view},
};

/// The sections of a board.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BoardSection {
    Agenda,
    Matrix,
    Info,
}

impl BoardSection {
    /// The tab's label, and the page title's first part.
    pub fn label(self) -> &'static str {
        match self {
            BoardSection::Agenda => "Agenda",
            BoardSection::Matrix => "Eisenhower matrix",
            BoardSection::Info => "Info",
        }
    }
}

/// The document title of a board page: the pick mode's `lead` ("Moving p/t";
/// empty when nothing is picked, and then left out), then the page's own
/// parts, most specific first.
pub fn title_parts<'a>(lead: &'a str, rest: &[&'a str]) -> Vec<&'a str> {
    Some(lead)
        .filter(|lead| !lead.is_empty())
        .into_iter()
        .chain(rest.iter().copied())
        .collect()
}

/// The three tabs of board `id`, `current` marked.
fn board_tabs(id: i64, current: BoardSection) -> Vec<Tab> {
    let tab = |section: BoardSection, href: String| (section, Tab::link(section.label(), href));
    tabs_for(
        current,
        [
            tab(BoardSection::Agenda, board_url(id)),
            tab(BoardSection::Matrix, board_matrix_url(id)),
            tab(BoardSection::Info, board_info_url(id)),
        ],
    )
}

/// The board's name as the heading, with its crumbs (`Boards / {here}`) and
/// the Edit button. `here` is the page's name, shown as the last crumb.
#[component]
pub async fn board_heading(board: &Board, here: &'static str) -> Result<impl View> {
    let crumbs = [boards_crumb(), Crumb::label(here)];
    Ok(view! {
        page_header(title: board.name.as_str(), kicker: Kicker::Crumbs(&crumbs), edit_button(href: board_edit_url(board.id())))
    })
}

/// [`board_heading`] and the section tabs: the top of the agenda, matrix
/// and info pages.
#[component]
pub async fn board_header(board: &Board, section: BoardSection) -> Result<impl View> {
    let items = board_tabs(board.id(), section);
    Ok(view! {
        board_heading(board: board, here: section.label())
        tabs(label: "Board sections", items: &items)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_starts_at_the_page_when_nothing_is_picked() {
        assert_eq!(title_parts("", &["Agenda", "Work"]), ["Agenda", "Work"]);
        assert_eq!(
            title_parts("Moving p/t", &["Agenda", "Work"]),
            ["Moving p/t", "Agenda", "Work"]
        );
    }

    #[test]
    fn a_board_has_the_three_tabs() {
        let items = board_tabs(2, BoardSection::Matrix);
        assert_eq!(items.len(), 3);
        assert_eq!(items.iter().filter(|t| t.is_current()).count(), 1);
        assert_eq!(BoardSection::Matrix.label(), "Eisenhower matrix");
    }
}
