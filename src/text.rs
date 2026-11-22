//! Pure text helpers with no dependency on the rest of iter (a leaf: sketch `architecture`,
//! section 3). Round I2 moves `utils/text.rs` here and adds [`slug`].
//!
//! stub: I2 (task 298): `slug` is `unimplemented!()` until the develop step of round I2; the
//! tests below pin what `git::slugify` does today, which is what the one merged helper keeps.

/// Lowercases `name`, collapses runs of anything but `[a-z0-9]` into one `-` and trims the
/// dashes: for branch names and folders. May be empty. Non-ASCII letters count as "anything
/// else" (`notes::stem` keeps them and caps the length: it stays its own function).
#[allow(dead_code)]
pub fn slug(_name: &str) -> String {
    unimplemented!("stub: I2")
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
            // letters outside ASCII are separators, as in `git::slugify`
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
