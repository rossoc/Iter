//! A titled group of controls: a `fieldset` whose `legend` names them. With
//! an `id` it can also be described (a hint, an error) and marked as the
//! thing the form's error is about: that is how `field::checklist` uses it.

use super::field::{hint_id, invalid_cue};
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// `id` (with `hint`) names the group and its hint for `described`, the
/// `aria-describedby` of the fieldset (the hint's id and the error's, see
/// `field::described_by`); a plain group needs none of them. `invalid`
/// shows the [`invalid_cue`] under the legend, outside it, so the legend
/// stays the group's name. The hint sits between the legend and the controls.
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
