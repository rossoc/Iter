//! The controls of a form. Every control has a real `<label for>`, its id
//! derived from its name by [`field_id`], so the form needs no ARIA beyond
//! `aria-describedby` (hint, error), `aria-invalid` and `aria-required`.
//!
//! A control is told the form's error (`error`) and works out for itself
//! whether it is the one the error is about.

use super::empty_line::empty_line;
use super::form_error::{FormError, is_about};
use super::group::group;
use super::notice::ERROR_ID;
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// The `id` of the control named `name`.
pub fn field_id(name: &str) -> String {
    format!("f-{name}")
}

/// The `id` of the hint under the control `id`.
pub(super) fn hint_id(id: &str) -> String {
    format!("{id}-hint")
}

/// The `aria-describedby` of a control: its hint (if it has one) and, when
/// the form's error is about this control, the error notice.
fn described_by(id: &str, hint: &str, invalid: bool) -> String {
    let mut ids = Vec::new();
    if !hint.is_empty() {
        ids.push(hint_id(id));
    }
    if invalid {
        ids.push(ERROR_ID.to_string());
    }
    ids.join(" ")
}

/// The text that marks a control the form's error is about, beside its label:
/// the control's own border is a colour and a thickness, this is the cue
/// that is neither. It is `aria-hidden` and outside the label: a screen
/// reader gets `aria-invalid` and the error notice (`aria-describedby`)
/// instead, and the control's name stays its label.
#[component]
pub async fn invalid_cue() -> Result<impl View> {
    Ok(view! {
        <span class="bad" aria-hidden="true">"Fix this"</span>
    })
}

/// A control's label (with the marker of a required one, and the
/// [`invalid_cue`] beside it when `invalid`) and, under it, its hint. The
/// control itself is the child.
#[component]
async fn labelled(
    id: &str,
    label: &str,
    hint: &str,
    required: bool,
    invalid: bool,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <div class="field">
            <div class="field-head">
                <label for=(id)>
                    (label)
                    if required { <span class="req" aria-hidden="true">"*"</span> }
                </label>
                if invalid { invalid_cue() }
            </div>
            (child)
            if !hint.is_empty() {
                <p class="hint" id=(hint_id(id))>(hint)</p>
            }
        </div>
    })
}

/// A single-line text control. `identifier` is for a value that is a name,
/// a path or a template rather than prose: no spelling help, no
/// capitalization, no autofill (the names of organizations, projects and
/// tasks are such values too: they are what the CLI addresses them by).
/// `numeric` brings up the number keypad. `required` is announced and
/// marked, but left to the server to enforce, so its message is the form's
/// own.
#[component]
pub async fn field(
    name: &str,
    label: &str,
    value: &str,
    #[default] hint: &str,
    #[default] error: Option<&FormError>,
    #[default] required: bool,
    #[default] identifier: bool,
    #[default] numeric: bool,
) -> Result<impl View> {
    let id = field_id(name);
    let invalid = is_about(error, name);
    let described = described_by(&id, hint, invalid);
    Ok(view! {
        labelled(id: &id, label: label, hint: hint, required: required, invalid: invalid,
            <input type="text" class="control" id=(id.as_str()) name=(name) value=(value)
                if identifier { spellcheck="false" autocapitalize="off" autocorrect="off" }
                if identifier { autocomplete="off" }
                if numeric { inputmode="numeric" }
                if required { aria-required="true" }
                if invalid { aria-invalid="true" }
                if !described.is_empty() { aria-describedby=(described.as_str()) }>
        )
    })
}

#[component]
pub async fn textarea(
    name: &str,
    label: &str,
    value: &str,
    #[default] hint: &str,
    #[default] error: Option<&FormError>,
    #[default] required: bool,
) -> Result<impl View> {
    let id = field_id(name);
    let invalid = is_about(error, name);
    let described = described_by(&id, hint, invalid);
    Ok(view! {
        labelled(id: &id, label: label, hint: hint, required: required, invalid: invalid,
            <textarea class="control" id=(id.as_str()) name=(name)
                if required { aria-required="true" }
                if invalid { aria-invalid="true" }
                if !described.is_empty() { aria-describedby=(described.as_str()) }>(value)</textarea>
        )
    })
}

#[component]
pub async fn checkbox(name: &str, label: &str, on: bool) -> Result<impl View> {
    let id = field_id(name);
    Ok(view! {
        <label class="check" for=(id.as_str())>
            <input type="checkbox" id=(id.as_str()) name=(name) if on { checked="" }>
            <span class="check-text">(label)</span>
        </label>
    })
}

/// One box of a [`checklist`]: what it posts (`value`), what it is called
/// (`label`), a `note` beside the label (secondary text, part of the
/// accessible name; left out when empty) and whether it is `on`.
pub struct CheckOption {
    pub value: String,
    pub label: String,
    pub note: String,
    pub on: bool,
}

/// A group of checkboxes that go together under one `name` (each posts
/// `name=value`), for a choice of several. A `group` (fieldset, legend
/// `label`) with the hint and error like the other controls: the fieldset is
/// described by them, and the group is the control the error is about when
/// `error` names `name`. With no `options` the group says `empty`. Each
/// box's id is [`choice_id`].
#[component]
pub async fn checklist(
    name: &str,
    label: &str,
    options: &[CheckOption],
    #[default] hint: &str,
    #[default] empty: &str,
    #[default] error: Option<&FormError>,
) -> Result<impl View> {
    let id = field_id(name);
    let invalid = is_about(error, name);
    let described = described_by(&id, hint, invalid);
    Ok(view! {
        group(id: &id, title: label, hint: hint, described: &described, invalid: invalid,
            if options.is_empty() {
                empty_line((empty))
            }
            for (i, option) in options.iter().enumerate() {
                <label class="check" for=(choice_id(name, i))>
                    <input type="checkbox" id=(choice_id(name, i)) name=(name) value=(option.value.as_str())
                        if option.on { checked="" }>
                    <span class="check-text">
                        (option.label.as_str())
                        if !option.note.is_empty() {
                            " "
                            <span class="note">(option.note.as_str())</span>
                        }
                    </span>
                </label>
            }
        )
    })
}

/// The `id` of the `index`th box of the checklist named `name`.
fn choice_id(name: &str, index: usize) -> String {
    format!("{}-{index}", field_id(name))
}

/// The options of a select that may be left without a choice: `empty`
/// (value "") first, then `items` as `(id, name)`. Pair it with [`id_text`].
pub fn optional_options<'a>(
    empty: &str,
    items: impl IntoIterator<Item = (i64, &'a str)>,
) -> Vec<(String, String)> {
    std::iter::once((String::new(), empty.to_string()))
        .chain(
            items
                .into_iter()
                .map(|(id, name)| (id.to_string(), name.to_string())),
        )
        .collect()
}

/// The `current` of a select over [`optional_options`]: the id, or "" for
/// none.
pub fn id_text(id: Option<i64>) -> String {
    id.map(|id| id.to_string()).unwrap_or_default()
}

/// `options` are `(value, label)`; `current` is the selected value.
#[component]
pub async fn select(
    name: &str,
    label: &str,
    options: &[(String, String)],
    current: &str,
    #[default] hint: &str,
    #[default] error: Option<&FormError>,
    #[default] required: bool,
) -> Result<impl View> {
    let id = field_id(name);
    let invalid = is_about(error, name);
    let described = described_by(&id, hint, invalid);
    Ok(view! {
        labelled(id: &id, label: label, hint: hint, required: required, invalid: invalid,
            <select class="control" id=(id.as_str()) name=(name)
                if required { aria-required="true" }
                if invalid { aria-invalid="true" }
                if !described.is_empty() { aria-describedby=(described.as_str()) }>
                for (value, text) in options.iter() {
                    <option value=(value.as_str()) if value == current { selected="" }>(text.as_str())</option>
                }
            </select>
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn described_by_lists_the_hint_and_the_error() {
        assert_eq!(described_by("f-a", "", false), "");
        assert_eq!(described_by("f-a", "h", false), "f-a-hint");
        assert_eq!(described_by("f-a", "", true), "form-error");
        assert_eq!(described_by("f-a", "h", true), "f-a-hint form-error");
    }

    #[test]
    fn optional_options_start_with_the_empty_choice() {
        let options = optional_options("No organization", [(3, "acme"), (5, "beta")]);
        assert_eq!(options[0], (String::new(), "No organization".to_string()));
        assert_eq!(options[2], ("5".to_string(), "beta".to_string()));
        assert_eq!(options.len(), 3);
        assert_eq!(
            (id_text(Some(3)), id_text(None)),
            ("3".to_string(), String::new())
        );
    }

    #[test]
    fn a_checklist_numbers_its_boxes() {
        assert_eq!(choice_id("projects", 0), "f-projects-0");
        assert_eq!(choice_id("projects", 12), "f-projects-12");
    }

    #[test]
    fn ids_derive_from_the_name() {
        assert_eq!(field_id("name"), "f-name");
        assert_eq!(hint_id("f-name"), "f-name-hint");
    }
}
