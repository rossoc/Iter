//! A titled aside of label / value rows; the caller decides what the rows say.

use super::status::status;
use super::tag_chips::{Chip, tag_chips};
use super::{CHECK, MINUS, icon};
use crate::models::TaskStatus;
use topcoat::{
    Result,
    view::{View, component, view},
};

pub enum Row<'a> {
    Text(&'a str, String),
    Flag(&'a str, bool),
    Chips(&'a str, Vec<Chip<'a>>),
    Status(&'a str, TaskStatus),
}

impl Row<'_> {
    fn label(&self) -> &str {
        match self {
            Row::Text(label, _)
            | Row::Flag(label, _)
            | Row::Chips(label, _)
            | Row::Status(label, _) => label,
        }
    }

    fn is_unset(&self) -> bool {
        match self {
            Row::Text(_, value) => value.is_empty(),
            Row::Chips(_, chips) => chips.is_empty(),
            Row::Flag(..) | Row::Status(..) => false,
        }
    }
}

#[component]
pub async fn panel(id: &str, title: &str, rows: &[Row<'_>]) -> Result<impl View> {
    Ok(view! {
        <aside class="panel" aria-labelledby=(id)>
            <h2 id=(id) class="label">(title)</h2>
            <dl>
                for r in rows.iter() {
                    row(r: r)
                }
            </dl>
        </aside>
    })
}

#[component]
async fn row(r: &Row<'_>) -> Result<impl View> {
    Ok(view! {
        <dt>(r.label())</dt>
        if r.is_unset() {
            unset()
        } else {
            match r {
                Row::Text(_, value) => <dd class="mono">(value.as_str())</dd>,
                Row::Flag(_, on) => flag(on: *on),
                Row::Chips(_, chips) => <dd>tag_chips(chips: chips)</dd>,
                Row::Status(_, state) => <dd>status(state: *state)</dd>,
            }
        }
    })
}

#[component]
async fn flag(on: bool) -> Result<impl View> {
    Ok(view! {
        <dd class=(if on { "flag on" } else { "flag" })>
            (icon(if on { CHECK } else { MINUS }))
            (if on { "on" } else { "off" })
        </dd>
    })
}

/// The value of an empty row: "—", read as "Not set".
#[component]
async fn unset() -> Result<impl View> {
    Ok(view! {
        <dd class="unset"><span aria-hidden="true">"—"</span><span class="sr">"Not set"</span></dd>
    })
}
