//! A `fieldset` whose `legend` names its controls; used by `field::checklist`.

use super::field::{hint_id, invalid_cue};
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// `id` (with `hint`) feeds the fieldset's `aria-describedby`; `invalid` shows
/// the [`invalid_cue`] outside the legend so the legend stays the name.
#[component]
pub async fn group(
    title: &str,
    #[default] id: &str,
    #[default] hint: &str,
    #[default] described: &str,
    #[default] invalid: bool,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <fieldset class="fieldgroup" if !id.is_empty() { id=(id) tabindex="-1" }
            if !described.is_empty() { aria-describedby=(described) }>
            <legend class="label">(title)</legend>
            if invalid { invalid_cue() }
            if !hint.is_empty() {
                <p class="hint" id=(hint_id(id))>(hint)</p>
            }
            <div class="fieldgroup-body">(child)</div>
        </fieldset>
    })
}
