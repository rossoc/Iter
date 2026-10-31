//! The heading and section tabs of the board pages; the last tab is Settings.

use super::crumbs::boards_crumb;
use super::sections::tabs_for;
use super::ui::breadcrumb::Crumb;
use super::ui::page_header::{Kicker, page_header};
use super::ui::tabs::{Tab, tabs};
use super::url::{board_edit_url, board_info_url, board_matrix_url, board_url};
use crate::db::Table;
use crate::models::Board;
use topcoat::{
    Result,
    view::{View, component, view},
};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BoardSection {
    Agenda,
    Matrix,
    Info,
    Settings,
}

impl BoardSection {
    pub fn label(self) -> &'static str {
        match self {
            BoardSection::Agenda => "Agenda",
            BoardSection::Matrix => "Eisenhower matrix",
            BoardSection::Info => "Info",
            BoardSection::Settings => "Settings",
        }
    }
}

/// `lead` is the pick mode's "Moving p/t", left out when empty.
pub fn title_parts<'a>(lead: &'a str, rest: &[&'a str]) -> Vec<&'a str> {
    Some(lead)
        .filter(|lead| !lead.is_empty())
        .into_iter()
        .chain(rest.iter().copied())
        .collect()
}

pub fn board_tabs(id: i64, current: BoardSection) -> Vec<Tab> {
    let tab = |section: BoardSection, href: String| (section, Tab::link(section.label(), href));
    tabs_for(
        current,
        [
            tab(BoardSection::Agenda, board_url(id)),
            tab(BoardSection::Matrix, board_matrix_url(id)),
            tab(BoardSection::Info, board_info_url(id)),
            tab(BoardSection::Settings, board_edit_url(id)),
        ],
    )
}

#[component]
pub async fn board_heading(board: &Board, here: &'static str) -> Result<impl View> {
    let crumbs = [boards_crumb(), Crumb::label(here)];
    Ok(view! {
        page_header(title: board.name.as_str(), kicker: Kicker::Crumbs(&crumbs))
    })
}

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
    fn a_board_has_four_tabs_and_settings_is_the_last() {
        let items = board_tabs(2, BoardSection::Matrix);
        assert_eq!(items.len(), 4);
        assert_eq!(items[3].href(), "/board/2/edit");
        assert_eq!(items.iter().filter(|t| t.is_current()).count(), 1);
        assert_eq!(BoardSection::Matrix.label(), "Eisenhower matrix");
    }
}
