//! A task's status as a glyph pill: icon (decorative) plus the word.

use super::{CIRCLE, CIRCLE_CHECK, HALF, icon};
use crate::models::TaskStatus;
use topcoat::{
    Result,
    view::{View, component, view},
};

fn glyph(state: TaskStatus) -> &'static str {
    match state {
        TaskStatus::Queue => CIRCLE,
        TaskStatus::Wip => HALF,
        TaskStatus::Done => CIRCLE_CHECK,
    }
}

/// `status.css`'s class for `state`.
fn class(state: TaskStatus) -> &'static str {
    match state {
        TaskStatus::Queue => "status queue",
        TaskStatus::Wip => "status wip",
        TaskStatus::Done => "status done",
    }
}

#[component]
pub async fn status(state: TaskStatus) -> Result<impl View> {
    Ok(view! {
        <span class=(class(state))>
            (icon(glyph(state)))
            (state.label())
        </span>
    })
}
