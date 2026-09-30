//! Text mappings for the cards and rows: a one-line note from markdown notes.

/// The first line of markdown notes with its `#` marks dropped, as a
/// one-line note (empty when there is none).
pub fn first_line(text: &str) -> &str {
    text.lines()
        .map(|l| l.trim().trim_start_matches('#').trim())
        .find(|l| !l.is_empty())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_line_with_text_is_taken() {
        assert_eq!(
            first_line(
                "
## Plan
more"
            ),
            "Plan"
        );
        assert_eq!(
            first_line(
                "  
"
            ),
            ""
        );
    }
}
