use crate::error::{IterError, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Given the template YAML that was opened in the editor and the file's
/// content after the editor exited, decides whether the edit counts as "a
/// save": unchanged (or unparsable) content means abort, anything else that
/// parses back into `T` means go ahead. Kept separate from `edit_in_nvim` so
/// it's testable without actually spawning an editor.
pub fn resolve_yaml_edit<T: DeserializeOwned>(original: &str, edited: &str) -> Option<T> {
    if edited == original {
        return None;
    }
    serde_yaml::from_str(edited).ok()
}

/// Opens `template` as YAML in `nvim` for the user to fill in/edit. Returns
/// `Ok(None)` if nvim exited without saving (file unchanged) or the result
/// doesn't parse back into `T` -- meaning "don't create/update anything" --
/// and `Ok(Some(edited))` otherwise.
pub fn edit_in_nvim<T: Serialize + DeserializeOwned>(template: &T) -> Result<Option<T>> {
    let original = serde_yaml::to_string(template)?;

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
    }

    #[test]
    fn unchanged_file_means_no_save() {
        let original = "name: ''\n";
        assert_eq!(resolve_yaml_edit::<Thing>(original, original), None);
    }

    #[test]
    fn edited_and_valid_parses_through() {
        let original = "name: ''\n";
        let edited = "name: hello\n";
        assert_eq!(
            resolve_yaml_edit::<Thing>(original, edited),
            Some(Thing {
                name: "hello".to_string()
            })
        );
    }

    #[test]
    fn edited_but_missing_required_field_is_treated_as_no_save() {
        let original = "name: ''\n";
        // Changed from the original, but doesn't parse into `Thing` (no `name`).
        let edited = "other_field: 5\n";
        assert_eq!(resolve_yaml_edit::<Thing>(original, edited), None);
    }
}
