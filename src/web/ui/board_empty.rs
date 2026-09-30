//! The hint of a board with no unfinished task, shared by the board pages.

use super::KANBAN;
use super::empty_state::empty_state;
use topcoat::{
    Result,
    view::{View, component, view},
};

#[component]
pub async fn board_empty() -> Result<impl View> {
    Ok(view! {
        empty_state(svg: KANBAN, "No unfinished tasks yet. Add some to the board's projects, and they show up here.")
    })
}
