//! Pure text helpers with no dependency on the rest of iter: a slug, percent-encoding, a YAML
//! scalar as text, a timestamp.

use serde_yaml::Value;

/// Lowercases `name`, collapses runs of anything but `[a-z0-9]` into one `-` and trims the
/// dashes: for branch names and folders. May be empty. Non-ASCII letters count as "anything
/// else" (`notes::stem` keeps them and caps the length: it stays its own function).
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

/// `text` with every byte that is not an ASCII letter, a digit or one of `keep` as `%XX`.
pub fn percent_encode(text: &str, keep: &[u8]) -> String {
    use std::fmt::Write as _;
    text.bytes()
        .fold(String::with_capacity(text.len()), |mut out, b| {
            match b.is_ascii_alphanumeric() || keep.contains(&b) {
                true => out.push(b as char),
                false => {
                    let _ = write!(out, "%{b:02X}");
                }
            }
            out
        })
}

/// A YAML string, number or boolean as text; anything else is not a scalar.
pub fn yaml_scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// The local time now, formatted with the `chrono` format `format`.
pub fn now_as(format: &str) -> String {
    chrono::Local::now().format(format).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slug_is_lowercase_words_joined_by_one_dash() {
        for (name, slug_of) in [
            ("Fix Login Bug!", "fix-login-bug"),
            ("  weird   Name__here--", "weird-name-here"),
            ("already-a-slug", "already-a-slug"),
            ("v2.0 release", "v2-0-release"),
            ("a/b\\c", "a-b-c"),
            ("--lead and trail--", "lead-and-trail"),
            ("UPPER 123", "upper-123"),
            // letters outside ASCII are separators, (a slug is ASCII)
            ("Café au lait", "caf-au-lait"),
            ("日本語 task", "task"),
        ] {
            assert_eq!(slug(name), slug_of, "{name:?}");
        }
    }

    /// May be empty: the caller (`git::task_slug`) owns the `task-<id>` fallback.
    #[test]
    fn a_name_with_nothing_sluggable_makes_an_empty_slug() {
        for name in ["", "   ", "!!! ---", "設計を見直す"] {
            assert_eq!(slug(name), "", "{name:?}");
        }
    }

    #[test]
    fn a_slug_has_no_length_cap_and_never_ends_in_a_dash() {
        let long = "word ".repeat(40);
        let made = slug(&long);
        assert_eq!(made.len(), 40 * 5 - 1);
        assert!(!made.ends_with('-') && !made.starts_with('-'));
    }
}
