use crate::error::{IterError, Result};
use crate::models::MarkdownBody;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Renders `item` as the YAML editor buffer: every field but `description`
/// (which `#[serde(skip)]`s itself out of `Serialize`) up top as normal
/// YAML, then a `---` line, then `description` written out verbatim as
/// plain markdown instead of a quoted/escaped YAML string.
fn render_template<T: Serialize + MarkdownBody>(item: &T) -> Result<String> {
    let mut out = serde_yaml::to_string(item)?;
    out.push_str("---\n");
    out.push_str(item.description());
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// Splits editor buffer content on the first line that is exactly `---`,
/// returning the YAML front matter before it and the markdown body after
/// it. No such line means there's no description section at all.
fn split_front_matter(content: &str) -> (&str, &str) {
    let mut consumed = 0;
    for line in content.split_inclusive('\n') {
        if line.trim_end_matches('\n') == "---" {
            return (&content[..consumed], &content[consumed + line.len()..]);
        }
        consumed += line.len();
    }
    (content, "")
}

/// Given the template YAML that was opened in the editor and the file's
/// content after the editor exited, decides whether the edit counts as "a
/// save": unchanged content means abort, anything else that parses back
/// into `T` (front matter) plus a markdown body means go ahead. Kept
/// separate from `edit_in_nvim` so it's testable without actually spawning
/// an editor.
pub fn resolve_yaml_edit<T: DeserializeOwned + MarkdownBody>(
    original: &str,
    edited: &str,
) -> Option<T> {
    if edited == original {
        return None;
    }
    let (front, body) = split_front_matter(edited);
    let mut item: T = serde_yaml::from_str(front).ok()?;
    item.set_description(body.trim_end_matches('\n').to_string());
    Some(item)
}

/// Opens `template` as YAML (plus a markdown `description` section below a
/// `---` separator) in `nvim` for the user to fill in/edit. Returns
/// `Ok(None)` if nvim exited without saving (file unchanged) or the result
/// doesn't parse back into `T` -- meaning "don't create/update anything" --
/// and `Ok(Some(edited))` otherwise.
pub fn edit_in_nvim<T: Serialize + DeserializeOwned + MarkdownBody>(
    template: &T,
) -> Result<Option<T>> {
    let original = render_template(template)?;

    let path = std::env::temp_dir().join(format!(
        "iter-edit-{}-{}.yaml",
        std::process::id(),
        chrono::Local::now().format("%Y%m%d%H%M%S%f")
    ));
    std::fs::write(&path, &original)?;

    let status = std::process::Command::new("nvim")
        .arg(&path)
        .status()
        .map_err(|source| IterError::Spawn {
            tool: "nvim",
            source,
        })?;

    let edited = std::fs::read_to_string(&path);
    let _ = std::fs::remove_file(&path);
    let edited = edited?;

    if !status.success() {
        return Ok(None);
    }

    Ok(resolve_yaml_edit(&original, &edited))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Thing {
        name: String,
        #[serde(skip)]
        description: String,
    }

    impl MarkdownBody for Thing {
        fn description(&self) -> &str {
            &self.description
        }

        fn set_description(&mut self, description: String) {
            self.description = description;
        }
    }

    fn thing(name: &str, description: &str) -> Thing {
        Thing {
            name: name.to_string(),
            description: description.to_string(),
        }
    }

    #[test]
    fn template_places_description_after_a_separator() {
        let rendered = render_template(&thing("hello", "some *markdown*\n- one\n- two")).unwrap();
        assert_eq!(
            rendered,
            "name: hello\n---\nsome *markdown*\n- one\n- two\n"
        );
    }

    #[test]
    fn unchanged_file_means_no_save() {
        let original = render_template(&thing("", "")).unwrap();
        assert_eq!(resolve_yaml_edit::<Thing>(&original, &original), None);
    }

    #[test]
    fn edited_and_valid_parses_through() {
        let original = render_template(&thing("", "")).unwrap();
        let edited = "name: hello\n---\nsome notes\n";
        assert_eq!(
            resolve_yaml_edit::<Thing>(&original, edited),
            Some(thing("hello", "some notes"))
        );
    }

    #[test]
    fn edited_but_missing_required_field_is_treated_as_no_save() {
        let original = render_template(&thing("", "")).unwrap();
        // Changed from the original, but the front matter doesn't parse
        // into `Thing` (no `name`).
        let edited = "other_field: 5\n---\nnotes\n";
        assert_eq!(resolve_yaml_edit::<Thing>(&original, edited), None);
    }

    #[test]
    fn description_is_optional() {
        let original = render_template(&thing("", "")).unwrap();
        // No `---` section at all -- description stays empty.
        let edited = "name: hello\n";
        assert_eq!(
            resolve_yaml_edit::<Thing>(&original, edited),
            Some(thing("hello", ""))
        );
    }
}
