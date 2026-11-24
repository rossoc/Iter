//! The frame: the top bar and the centered main pane.

use super::{all::*, kit::*};

/// The main nav `(label, href, icon)`; hrefs equal `url::BOARDS`/`SETTINGS` (tested in `url.rs`).
pub const NAV: [(&str, &str, &str); 4] = [
    ("Board", "/boards", KANBAN),
    ("Agents", "/agents", BOT),
    ("Instructions", "/instructions", FILE_TEXT),
    ("Settings", "/settings", SLIDERS),
];

/// The document title, most specific part first; `error` prefixes "Error: " so a screen reader announces the failure.
pub(super) fn document_title(parts: &[&str], error: bool) -> String {
    let mut title = String::new();
    if error {
        title.push_str("Error: ");
    }
    for part in parts {
        title.push_str(part);
        title.push_str(" - ");
    }
    title.push_str("iter");
    title
}

/// `title` parts feed [`document_title`]. The `<title>` is written first in the
/// body because the layout renders before the page knows its title. `error` is the
/// form's refusal: it prefixes the title and is raised as a toast too (`toast.rs`).
/// `current` is the nav `href` that gets `aria-current`. `dialog` is a pop-up
/// (`modal.rs`): the rest becomes `inert` and `modal.js` is loaded for Esc and the focus trap.
/// `<main>` is a live region (`data-live`): `live.js` swaps it for a fresh copy of the page when
/// the data behind it changes (`GET /live`).
#[component]
pub async fn frame(
    title: &[&str],
    #[default] error: Option<&FormError>,
    #[default] current: &str,
    #[default] dialog: Option<Child<'_>>,
    child: Child<'_>,
) -> Result<impl View> {
    let covered = dialog.is_some();
    Ok(view! {
        <title>(document_title(title, error.is_some()))</title>
        <script src="/search.js"></script>
        <script src="/combobox.js" defer=""></script>
        <script src="/path_pick.js" defer=""></script>
        <script src="/notice.js" defer=""></script>
        <script src="/live.js" defer=""></script>
        <div class="page">
            <a class="skip" href="#main" if covered { inert="" }>"Skip to content"</a>
            <header class="bar" if covered { inert="" }>
                <a class="brand" href="/">(icon(MARK)) "iter"</a>
                <nav aria-label="Main">
                    for (label, href, svg) in NAV.iter() {
                        <a href=(*href) if *href == current { aria-current="true" }>(icon(svg)) (*label)</a>
                    }
                </nav>
                search_button()
            </header>
            <main id="main" tabindex="-1" data-live="" if covered { inert="" }>(child)</main>
            palette()
            toasts(error: error)
            if let Some(dialog) = dialog {
                (dialog)
            }
            if covered {
                <script src="/modal.js" defer=""></script>
            }
        </div>
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_ends_with_the_app() {
        assert_eq!(document_title(&[], false), "iter");
        assert_eq!(document_title(&["Acme"], false), "Acme - iter");
        assert_eq!(
            document_title(&["Edit organization", "Acme"], true),
            "Error: Edit organization - Acme - iter"
        );
    }
}
