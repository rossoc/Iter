//! A POST form: the `<form>` wrapper and nothing else. The page puts in it
//! the error box (`notice::error_box`), `required_note`, the controls (`field.rs`) and the
//! way out (`form_actions.rs`).

use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// "* Required", above the controls of a form that has required ones (the
/// `*` beside their labels, `field.rs`).
#[component]
pub async fn required_note() -> Result<impl View> {
    Ok(view! {
        <p class="required-note"><span aria-hidden="true">"*"</span> " Required"</p>
    })
}

#[component]
pub async fn form(#[into] action: String, child: Child<'_>) -> Result<impl View> {
    Ok(view! {
        <form class="form" method="post" action=(action.as_str())>(child)</form>
    })
}
