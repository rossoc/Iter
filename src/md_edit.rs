use crate::config::config;
use crate::error::{IterError, Result};
use crate::models::MarkdownBody;
use crate::process;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Renders `item` as the editor buffer: YAML front matter between `---` lines, then the description.
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
pub(crate) fn split_at_divider(content: &str) -> Option<(&str, &str)> {
    let mut consumed = 0;
    for line in content.split_inclusive('\n') {
        if line.trim_end_matches('\n') == "---" {
            return Some((&content[..consumed], &content[consumed + line.len()..]));
        }
        consumed += line.len();
    }
    None
}

/// Splits an editor buffer into YAML front matter and markdown body.
fn split_front_matter(content: &str) -> (&str, &str) {
    let content = content.strip_prefix("---\n").unwrap_or(content);
    split_at_divider(content).unwrap_or((content, ""))
}

/// Decides whether the edit counts as a save: unchanged content is `Ok(None)` (abort). Front
/// matter that doesn't parse is an error, not `None`: reading a YAML slip as "no changes" would
/// discard the only copy of the buffer.
pub fn resolve_edit<T: DeserializeOwned + MarkdownBody>(
    original: &str,
    edited: &str,
) -> std::result::Result<Option<T>, serde_yaml::Error> {
    if edited == original {
        return Ok(None);
    }
    let (front, body) = split_front_matter(edited);
    let mut item = T::from_front_matter(front)?;
    // The blank line under the front matter is separator, not description. The end is trimmed of
    // all whitespace to match `github::normalize_body`, else a description never equals its issue
    // and every `task push --body` rewrites it.
    item.set_description(body.trim_start_matches('\n').trim_end().to_string());
    Ok(Some(item))
}

/// Opens `template` in the configured editor (`nvim` by default). `Ok(None)` if it exited
/// without saving. Unparseable front matter is an error carrying the path the buffer is left at.
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

    // `config()` is `&'static`, so the editor name satisfies `run_status`'s `&'static str`.
    let saved = process::run_status(&config().editor, None, &[&path])?;

    let outcome = match saved {
        true => read_edited(&path, &original).map(|item: Option<T>| {
            item.map(|mut item| {
                item.carry_over(template);
                item
            })
        }),
        false => Ok(None),
    };

    // Unparseable front matter leaves the buffer on disk, named in the error: it is the only copy
    // of what was typed.
    if !matches!(outcome, Err(IterError::InvalidBuffer { .. })) {
        let _ = std::fs::remove_file(&path);
    }
    outcome
}

/// Reads the buffer back and resolves it against `original`; the one place deciding whether the
/// file stays.
fn read_edited<T: DeserializeOwned + MarkdownBody>(
    path: &std::path::Path,
    original: &str,
) -> Result<Option<T>> {
    let edited = std::fs::read_to_string(path)?;
    resolve_edit(original, &edited).map_err(|source| IterError::InvalidBuffer {
        path: path.display().to_string(),
        source,
    })
}

/// Opens `initial` as plain markdown in the configured editor. `None` if the
/// editor quit without saving, or saved it unchanged.
pub fn edit_text(initial: &str) -> Result<Option<String>> {
    let path = std::env::temp_dir().join(format!(
        "iter-edit-{}-{}.md",
        std::process::id(),
        chrono::Local::now().format("%Y%m%d%H%M%S%f")
    ));
    std::fs::write(&path, initial)?;
    let saved = process::run_status(&config().editor, None, &[&path]);
    let text = std::fs::read_to_string(&path);
    let _ = std::fs::remove_file(&path);
    Ok(match (saved?, text?) {
        (true, text) if text != initial => Some(text),
        _ => None,
    })
}

/// Opens `template` and hands the result to `save`, which writes and returns the line to print.
/// The no-change message is shared (`noun`/`verb`: "no changes -- project not updated").
pub fn edited<T: Serialize + DeserializeOwned + MarkdownBody>(
    template: &T,
    noun: &str,
    verb: &str,
    save: impl FnOnce(T) -> Result<String>,
) -> Result<()> {
    match edit_in_editor(template)? {
        Some(item) => println!("{}", save(item)?),
        None => println!("{}", no_changes(noun, verb)),
    }
    Ok(())
}

pub fn no_changes(noun: &str, verb: &str) -> String {
    format!("no changes -- {noun} not {verb}")
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
        assert_eq!(resolve_edit::<Thing>(&original, &original).unwrap(), None);
    }

    #[test]
    fn edited_and_valid_parses_through() {
        let original = render_template(&thing("", "")).unwrap();
        let edited = "---\nname: hello\n---\n\nsome notes\n";
        assert_eq!(
            resolve_edit::<Thing>(&original, edited).unwrap(),
            Some(thing("hello", "some notes"))
        );
    }

    #[test]
    fn edited_but_unparseable_front_matter_is_an_error_not_a_no_save() {
        let original = render_template(&thing("", "")).unwrap();
        // Changed from the original, but the front matter doesn't parse
        // into `Thing` (no `name`).
        let edited = "---\nother_field: 5\n---\n\nnotes\n";
        assert!(resolve_edit::<Thing>(&original, edited).is_err());
        // ...as is front matter that isn't YAML at all.
        let broken = "---\nname: hello: there\n---\n\nnotes\n";
        assert!(resolve_edit::<Thing>(&original, broken).is_err());
    }

    /// Trailing whitespace comes off, matching `github::normalize_body`.
    #[test]
    fn a_description_keeps_no_trailing_whitespace() {
        let original = render_template(&thing("", "")).unwrap();
        let edited = "---\nname: hello\n---\n\nsome notes   \n\n";
        assert_eq!(
            resolve_edit::<Thing>(&original, edited).unwrap(),
            Some(thing("hello", "some notes"))
        );
    }

    #[test]
    fn an_organization_edit_round_trips_its_projects() {
        use crate::models::Settings;
        use crate::models::{GroupEdit, Organization};

        let mut organization = Organization::template(&Settings::default());
        organization.id = Some(7);
        organization.name = "acme".to_string();
        organization.description = "notes".to_string();
        let edit = GroupEdit {
            group: organization,
            projects: vec!["one".to_string(), "two".to_string()],
        };
        let rendered = render_template(&edit).unwrap();
        assert!(rendered.contains("projects:\n- one\n- two\n"), "{rendered}");

        let back = resolve_edit::<GroupEdit<Organization>>("", &rendered)
            .unwrap()
            .unwrap();
        assert_eq!(back.projects, ["one", "two"]);
        assert_eq!(back.group.id, Some(7));
        assert_eq!(back.group.name, "acme");
        assert_eq!(back.group.description, "notes");
        assert_eq!(back.group.tmux, edit.group.tmux);

        let without = rendered.replace("projects:\n- one\n- two\n", "");
        let back = resolve_edit::<GroupEdit<Organization>>("", &without)
            .unwrap()
            .unwrap();
        assert!(back.projects.is_empty());
    }

    #[test]
    fn carry_over_restores_only_the_fields_the_buffer_hides() {
        use crate::models::Project;
        use crate::models::Settings;

        let mut original = Project::template(&Settings::default());
        original.organization_id = Some(3);
        original.board_id = Some(4);
        original.name = "before".to_string();
        let rendered = render_template(&original).unwrap();
        let edited = rendered.replace("name: before", "name: after");

        let mut back = resolve_edit::<Project>(&rendered, &edited)
            .unwrap()
            .unwrap();
        assert_eq!((back.organization_id, back.board_id), (None, None));
        back.carry_over(&original);
        assert_eq!((back.organization_id, back.board_id), (Some(3), Some(4)));
        assert_eq!(back.name, "after");
    }

    #[test]
    fn a_task_edit_round_trips_its_tags() {
        use crate::models::{Task, TaskEdit, TaskStatus};

        let mut task = Task::template(9, String::new(), TaskStatus::Queue);
        task.name = "t".to_string();
        let edit = TaskEdit {
            task,
            tags: vec!["Urgent".to_string()],
            project: "alpha".to_string(),
        };
        let rendered = render_template(&edit).unwrap();
        assert!(rendered.contains("tags:\n- Urgent\n"), "{rendered}");
        assert!(rendered.contains("project: alpha\n"), "{rendered}");
        let mut back = resolve_edit::<TaskEdit>("", &rendered).unwrap().unwrap();
        back.carry_over(&edit);
        assert_eq!(
            (back.task.project_id, back.tags),
            (9, vec!["Urgent".to_string()])
        );
    }

    #[test]
    fn description_is_optional() {
        let original = render_template(&thing("", "")).unwrap();
        // Front matter left open, with no body under it -- description
        // stays empty.
        let edited = "---\nname: hello\n";
        assert_eq!(
            resolve_edit::<Thing>(&original, edited).unwrap(),
            Some(thing("hello", ""))
        );
    }

    #[test]
    fn a_rendered_template_round_trips() {
        for item in [thing("hello", "notes\n\nmore notes"), thing("hello", "")] {
            let rendered = render_template(&item).unwrap();
            // An empty `original`, so the buffer counts as edited rather
            // than as the untouched one `resolve_edit` reads as "no save".
            assert_eq!(resolve_edit::<Thing>("", &rendered).unwrap(), Some(item));
        }
    }

    #[test]
    fn wrapped_buffers_take_bare_scalars_as_typed_text() {
        use crate::models::{GroupEdit, Organization, Task, TaskEdit, TaskStatus};
        let original = render_template(&TaskEdit {
            task: Task::template(1, String::new(), TaskStatus::Queue),
            tags: Vec::new(),
            project: String::new(),
        })
        .unwrap();
        let edited = original.replacen("name: ''", "name: 2024", 1);
        let task = resolve_edit::<TaskEdit>(&original, &edited)
            .unwrap()
            .unwrap();
        assert_eq!((task.task.name.as_str(), task.tags.len()), ("2024", 0));

        let front = "name: ~\ndefault_branch: 1.10\ngithub_project: true\nprojects: [a, b]\n";
        let group = GroupEdit::<Organization>::from_front_matter(front).unwrap();
        assert_eq!(
            (
                group.group.name.as_str(),
                group.group.default_branch.as_str(),
                group.group.github_project.as_str(),
            ),
            ("~", "1.10", "true")
        );
        assert_eq!(group.projects, vec!["a".to_string(), "b".to_string()]);
    }
}
