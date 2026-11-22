//! A task's status as a glyph pill: icon (decorative) plus the word.

use crate::prelude::*;
use iter_core::models::TaskStatus;

fn glyph(state: TaskStatus) -> &'static str {
    match state {
        TaskStatus::Queue => CIRCLE,
        TaskStatus::Wip => HALF,
        TaskStatus::Done => CIRCLE_CHECK,
    }
}

fn class(state: TaskStatus) -> &'static str {
    match state {
        TaskStatus::Queue => "status queue",
        TaskStatus::Wip => "status wip",
        TaskStatus::Done => "status done",
    }
}

#[component]
pub async fn status_icon(state: TaskStatus) -> Result<impl View> {
    Ok(view! {
        <span class=(class(state))>
            (icon(glyph(state)))
            (state.label())
        </span>
    })
}
