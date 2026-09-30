//! The ink of a colored chip: text color computed from the chip's color so
//! the text always reads (WCAG 4.5:1). Pure functions, no markup.

use crate::models::is_hex_color;

/// `#rrggbb` as its channels, or `None` for anything else (the database
/// accepts any text as a tag color).
fn channels(color: &str) -> Option<[u8; 3]> {
    if !is_hex_color(color) {
        return None;
    }
    let hex = &color[1..];
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// The relative luminance of WCAG 2.x.
fn luminance([r, g, b]: [u8; 3]) -> f64 {
    let linear = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

/// The text color for a chip of `color`: `#000` or `#fff`, whichever
/// contrasts more, and so at least 4.58:1 whatever the color. `None` when
/// `color` is not `#rrggbb`.
pub fn readable_ink(color: &str) -> Option<&'static str> {
    let l = luminance(channels(color)?);
    // Contrast with black is (l + .05) / .05, with white 1.05 / (l + .05);
    // they cross where l is about .179.
    Some(if (l + 0.05) / 0.05 >= 1.05 / (l + 0.05) {
        "#000"
    } else {
        "#fff"
    })
}

/// The inline style of one chip: only ever built from a valid `#rrggbb`.
pub fn chip_style(color: &str) -> Option<String> {
    let ink = readable_ink(color)?;
    Some(format!("background:{color};color:{ink}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contrast(color: &str, ink: &str) -> f64 {
        let ink = if ink == "#000" { [0; 3] } else { [255; 3] };
        let (a, b) = (luminance(channels(color).unwrap()), luminance(ink));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn ink_follows_the_color() {
        assert_eq!(readable_ink("#facc15"), Some("#000")); // Urgent's yellow
        assert_eq!(readable_ink("#ffffff"), Some("#000"));
        assert_eq!(readable_ink("#000000"), Some("#fff"));
        assert_eq!(readable_ink("#1e3a8a"), Some("#fff"));
        assert_eq!(readable_ink("#FACC15"), Some("#000"));
    }

    #[test]
    fn every_color_reads_at_4_5_to_1() {
        for r in (0..=255).step_by(15) {
            for g in (0..=255).step_by(15) {
                for b in (0..=255).step_by(15) {
                    let color = format!("#{r:02x}{g:02x}{b:02x}");
                    let ink = readable_ink(&color).unwrap();
                    assert!(contrast(&color, ink) >= 4.5, "{color} with {ink}");
                }
            }
        }
    }

    #[test]
    fn only_a_hex_color_reaches_a_style() {
        for bad in [
            "", "red", "#fff", "#12345", "#12345g", "#1234567", "url(x)", "#+12345",
        ] {
            assert_eq!(readable_ink(bad), None, "{bad}");
            assert_eq!(chip_style(bad), None, "{bad}");
        }
        assert_eq!(
            chip_style("#facc15").as_deref(),
            Some("background:#facc15;color:#000")
        );
    }
}
