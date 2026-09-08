use crate::config::config;
use crate::error::Result;
use crate::models::MarkdownBody;
use crate::process;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Renders `item` as the editor buffer: a markdown document whose YAML
/// front matter -- a `---` line
fn render_template<T: Serialize + MarkdownBody>(item: &T) -> Result<String> {
    let mut out = String::from("---\n");
    out.push_str(&serde_yaml::to_string(item)?);
    out.push_str("---\n\n");
    out.push_str(item.description());
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// Splits `content` on the first line that is exactly `---`, or `None` if
/// there is no such line.
fn split_at_divider(content: &str) -> Option<(&str, &str)> {
    let mut consumed = 0;
    for line in content.split_inclusive('\n') {
        if line.trim_end_matches('\n') == "---" {
            return Some((&content[..consumed], &content[consumed + line.len()..]));
        }
        consumed += line.len();
    }
    None
}

/// Splits editor buffer content into its YAML front matter and the
/// markdown body below it
fn split_front_matter(content: &str) -> (&str, &str) {
    let content = content.strip_prefix("---\n").unwrap_or(content);
    split_at_divider(content).unwrap_or((content, ""))
}

/// Given the template that was opened in the editor and the file's content
/// after the editor exited, decides whether the edit counts as "a save":
/// unchanged content means abort, anything else that parses back into `T`
/// (front matter) plus a markdown body means go ahead
pub fn resolve_edit<T: DeserializeOwned + MarkdownBody>(original: &str, edited: &str) -> Option<T> {
    if edited == original {
        return None;
    }
    let (front, body) = split_front_matter(edited);
    let mut item: T = serde_yaml::from_str(front).ok()?;
    // The blank line the template leaves under the front matter is
    // separator, not description -- trimming it keeps a description from
    // growing an extra leading newline on every round trip.
    item.set_description(
        body.trim_start_matches('\n')
            .trim_end_matches('\n')
            .to_string(),
    );
    Some(item)
}

/// Opens `template` as a markdown file -- YAML front matter with the
/// fields, the `description` as the body -- in the configured editor
/// (`nvim` unless `editor` says otherwise) for the user to fill in/edit.
/// Returns `Ok(None)` if the editor exited without saving (file unchanged)
/// or the result doesn't parse back into `T`
pub fn edit_in_editor<T: Serialize + DeserializeOwned + MarkdownBody>(
    template: &T,
) -> Result<Option<T>> {
    let original = render_template(template)?;

    let path = std::env::temp_dir().join(format!(
        "iter-edit-{}-{}.md",
        std::process::id(),
        chrono::Local::now().format("%Y%m%d%H%M%S%f")
    ));
    std::fs::write(&path, &original)?;

    // `config()` hands out a `&'static Config`, so the configured name
    // lives as long as the process and satisfies `run_status`'s
    // `&'static str` -- no leaking a `String` to name the tool.
    let saved = process::run_status(&config().editor, None, &[&path])?;

    let edited = std::fs::read_to_string(&path);
    let _ = std::fs::remove_file(&path);
    let edited = edited?;

    if !saved {
        return Ok(None);
    }

    Ok(resolve_edit(&original, &edited))
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
    fn template_is_fenced_front_matter_then_the_description() {
        let rendered = render_template(&thing("hello", "some *markdown*\n- one\n- two")).unwrap();
        assert_eq!(
            rendered,
            "---\nname: hello\n---\n\nsome *markdown*\n- one\n- two\n"
        );
    }

    #[test]
    fn unchanged_file_means_no_save() {
        let original = render_template(&thing("", "")).unwrap();
        assert_eq!(resolve_edit::<Thing>(&original, &original), None);
    }

    #[test]
    fn edited_and_valid_parses_through() {
        let original = render_template(&thing("", "")).unwrap();
        let edited = "---\nname: hello\n---\n\nsome notes\n";
        assert_eq!(
            resolve_edit::<Thing>(&original, edited),
            Some(thing("hello", "some notes"))
        );
    }

    #[test]
    fn edited_but_missing_required_field_is_treated_as_no_save() {
        let original = render_template(&thing("", "")).unwrap();
        // Changed from the original, but the front matter doesn't parse
        // into `Thing` (no `name`).
        let edited = "---\nother_field: 5\n---\n\nnotes\n";
        assert_eq!(resolve_edit::<Thing>(&original, edited), None);
    }

    #[test]
    fn description_is_optional() {
        let original = render_template(&thing("", "")).unwrap();
        // Front matter left open, with no body under it -- description
        // stays empty.
        let edited = "---\nname: hello\n";
        assert_eq!(
            resolve_edit::<Thing>(&original, edited),
            Some(thing("hello", ""))
        );
    }

    /// What the editor writes has to read back as what went in, or a
    /// description would drift (gaining or losing blank lines) every time
    /// the thing is edited and saved again.
    #[test]
    fn a_rendered_template_round_trips() {
        for item in [thing("hello", "notes\n\nmore notes"), thing("hello", "")] {
            let rendered = render_template(&item).unwrap();
            // An empty `original`, so the buffer counts as edited rather
            // than as the untouched one `resolve_edit` reads as "no save".
            assert_eq!(resolve_edit::<Thing>("", &rendered), Some(item));
        }
    }
}
