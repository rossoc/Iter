//! Form controls. Each has a real `<label for>` (id from [`field_id`]), so only
//! `aria-describedby`, `aria-invalid` and `aria-required` are needed.

use super::empty_line::empty_line;
use super::form_error::{FormError, is_about};
use super::group::group;
use super::icons::{FOLDER, icon};
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

/// A control's `aria-describedby`: its hint and, if the error is about it, the error notice.
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

/// Cue beside the label for the control the error is about, for those who
/// cannot rely on the border colour. `aria-hidden` and outside the label, so
/// screen readers get `aria-invalid` and the error notice instead.
#[component]
pub async fn invalid_cue() -> Result<impl View> {
    Ok(view! {
        <span class="bad" aria-hidden="true">"Fix this"</span>
    })
}

/// A control's label (required marker, [`invalid_cue`]) and its hint; the control is the child.
#[component]
async fn labelled(
    id: &str,
    label: &str,
    hint: &str,
    required: bool,
    invalid: bool,
    #[default] hide_label: bool,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <div class="field">
            <div class="field-head">
                <label for=(id) class=(if hide_label { "sr" } else { "" })>
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

/// A single-line text control. `identifier`: names, paths, templates (no spelling
/// help, capitalization or autofill). `required` is announced but enforced by the
/// server. `autofocus` is for a pop-up's first control. `beside` sits after the
/// input on its line. `input_type` defaults to text. `list` is a `<datalist>` id;
/// `form` the owning form's id; `autoload` has `modal.js` submit it on change.
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
    #[default] autofocus: bool,
    #[default] beside: Option<Child<'_>>,
    #[default] input_type: &str,
    #[default] list: &str,
    #[default] form: &str,
    #[default] autoload: bool,
) -> Result<impl View> {
    let id = field_id(name);
    let input_type = if input_type.is_empty() {
        "text"
    } else {
        input_type
    };
    let invalid = is_about(error, name);
    let described = described_by(&id, hint, invalid);
    let input_id = id.clone();
    let input = view! {
        <input type=(input_type) class="control" id=(input_id.as_str()) name=(name) value=(value)
            if identifier { spellcheck="false" autocapitalize="off" autocorrect="off" }
            if identifier { autocomplete="off" }
            if numeric { inputmode="numeric" }
            if autofocus { autofocus="" }
            if !list.is_empty() { list=(list) }
            if !form.is_empty() { form=(form) }
            if autoload { data-autoload="" }
            if required { aria-required="true" }
            if invalid { aria-invalid="true" }
            if !described.is_empty() { aria-describedby=(described.as_str()) }>
    };
    Ok(view! {
        labelled(id: &id, label: label, hint: hint, required: required, invalid: invalid,
            if let Some(beside) = beside {
                <div class="field-row">(input)(beside)</div>
            } else {
                (input)
            }
        )
    })
}

/// A filesystem path control: a [`field`] plus a folder button that `path_pick.js`
/// wires to `POST /folders/pick`. The button is `hidden` until the script runs.
#[component]
pub async fn path_field(
    name: &str,
    label: &str,
    value: &str,
    #[default] hint: &str,
    #[default] error: Option<&FormError>,
    #[default] required: bool,
) -> Result<impl View> {
    let id = field_id(name);
    let aria = path_pick_label(label);
    Ok(view! {
        field(name: name, label: label, value: value, hint: hint, error: error, required: required, identifier: true,
            beside: Some(Child::new(view! {
                <button class="button path-pick" type="button" hidden="" data-path-pick=(id.as_str()) aria-label=(aria.as_str()) title="Choose folder">(icon(FOLDER))</button>
            })))
    })
}

/// The accessible name of the folder button of the path field `label`.
fn path_pick_label(label: &str) -> String {
    format!("Choose folder for {label}")
}

#[component]
pub async fn textarea(
    name: &str,
    label: &str,
    value: &str,
    #[default] hint: &str,
    #[default] error: Option<&FormError>,
    #[default] required: bool,
    #[default] loaded: &str,
    // Label kept for screen readers but not shown.
    #[default] hide_label: bool,
) -> Result<impl View> {
    let id = field_id(name);
    let invalid = is_about(error, name);
    let described = described_by(&id, hint, invalid);
    Ok(view! {
        labelled(id: &id, label: label, hint: hint, required: required, invalid: invalid, hide_label: hide_label,
            <textarea class="control" id=(id.as_str()) name=(name)
                if !loaded.is_empty() { data-loaded=(loaded) }
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

/// One box of a [`checklist`]; `note` is secondary text, part of the accessible name.
pub struct CheckOption {
    pub value: String,
    pub label: String,
    pub note: String,
    pub on: bool,
}

/// Checkboxes sharing one `name`, in a fieldset described by the hint and
/// error; with no `options` it says `empty`. Box ids are [`choice_id`].
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

/// Select options with an optional blank: `empty` (value "") first, then `items` as `(id, name)`.
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

/// The `current` of a select over [`optional_options`]: the id, or "".
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
    fn a_path_field_names_its_folder_button_after_its_label() {
        assert_eq!(path_pick_label("Base path"), "Choose folder for Base path");
    }

    #[test]
    fn ids_derive_from_the_name() {
        assert_eq!(field_id("name"), "f-name");
        assert_eq!(hint_id("f-name"), "f-name-hint");
    }
}
