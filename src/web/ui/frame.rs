//! The frame: the top bar and the centered main pane.

use super::search::{palette, search_button};
use super::{BOT, FILE_TEXT, KANBAN, MARK, SLIDERS, icon};
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// The main nav `(label, href, icon)`; hrefs equal `web::url::BOARDS`/`SETTINGS` (tested in `web/url.rs`).
pub const NAV: [(&str, &str, &str); 4] = [
    ("Board", "/boards", KANBAN),
    ("Agents", "/agents", BOT),
    ("Instructions", "/instructions", FILE_TEXT),
    ("Settings", "/settings", SLIDERS),
];

/// The document title, most specific part first; `error` prefixes "Error: " so a screen reader announces the failure.
pub fn document_title(parts: &[&str], error: bool) -> String {
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
/// body because the layout renders before the page knows its title. `current` is the
/// nav `href` that gets `aria-current`. `dialog` is a pop-up (`modal.rs`): the rest
/// becomes `inert` and `modal.js` is loaded for Esc and the focus trap.
#[component]
pub async fn frame(
    title: &[&str],
    #[default] error: bool,
    #[default] current: &str,
    #[default] dialog: Option<Child<'_>>,
    child: Child<'_>,
) -> Result<impl View> {
    let covered = dialog.is_some();
    Ok(view! {
        <title>(document_title(title, error))</title>
        <script src="/search.js"></script>
        <script src="/combobox.js" defer=""></script>
        <script src="/path_pick.js" defer=""></script>
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
            <main id="main" tabindex="-1" if covered { inert="" }>(child)</main>
            palette()
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
