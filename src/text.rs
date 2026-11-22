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

/// `content` without a leading byte-order mark. From `md_edit` in isolation step 2.
// stub: I3 (`md_edit::strip_bom` still does the work)
#[allow(dead_code)]
pub fn strip_bom(content: &str) -> &str {
    let _ = content;
    unimplemented!("stub: I3")
}

/// Front matter and body of a markdown text: `front` is what lies between an optional opening
/// `---` line and the first line that is exactly `---` (its last newline included), `body` what
/// follows; with no such line `front` is `None` and `body` all that follows the opening.
#[derive(Debug, PartialEq, Eq)]
pub struct Split<'a> {
    pub front: Option<&'a str>,
    pub body: &'a str,
}

/// Splits after an optional byte-order mark; an opening `---` is optional.
// stub: I3 (`md_edit::split_front` still does the work)
#[allow(dead_code)]
pub fn split_front(content: &str) -> Split<'_> {
    let _ = content;
    unimplemented!("stub: I3")
}

/// [`split_front`] when `content` opens with `---` and a `---` line closes it, else `None`.
// stub: I3 (`md_edit::split_closed` still does the work)
#[allow(dead_code)]
pub fn split_closed(content: &str) -> Option<Split<'_>> {
    let _ = content;
    unimplemented!("stub: I3")
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

    /// Round I3 (step 2): the front-matter rule moves here from `md_edit`, the same cases and the
    /// same exact divider, with the result as a [`Split`].
    #[test]
    fn front_matter_ends_at_the_first_exact_divider() {
        let closed = |front: &'static str, body: &'static str| {
            Some(Split {
                front: Some(front),
                body,
            })
        };
        for (text, expected) in [
            ("---\na: 1\n---\nBody\n", closed("a: 1\n", "Body\n")),
            ("---\n---\nBody", closed("", "Body")),
            ("---\na: 1\n---", closed("a: 1\n", "")),
            ("\u{feff}---\na: 1\n---\nb", closed("a: 1\n", "b")),
            ("---\nnever closed\n", None),
            ("a: 1\n---\nb", None),
            ("---\na: 1\n--- \nb", None),
        ] {
            assert_eq!(split_closed(text), expected, "{text:?}");
        }
    }

    #[test]
    fn a_split_without_the_opening_line_still_finds_the_divider() {
        assert_eq!(
            split_front("a: 1\n---\nb"),
            Split {
                front: Some("a: 1\n"),
                body: "b"
            }
        );
        assert_eq!(
            split_front("---\nnever closed\n"),
            Split {
                front: None,
                body: "never closed\n"
            }
        );
        assert_eq!(
            split_front("plain"),
            Split {
                front: None,
                body: "plain"
            }
        );
    }

    #[test]
    fn only_a_leading_byte_order_mark_is_stripped() {
        assert_eq!(strip_bom("\u{feff}text"), "text");
        assert_eq!(strip_bom("text\u{feff}"), "text\u{feff}");
        assert_eq!(strip_bom(""), "");
    }
}
