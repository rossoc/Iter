//! The frame: the top bar and the centered main pane.

use super::{KANBAN, MARK, icon};
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// The links of the main nav: `(label, href, icon)`. The Board href is
/// `web::url::BOARDS` (a test in `web/url.rs` keeps the two equal): `ui` does
/// not know the routes.
pub const NAV: [(&str, &str, &str); 1] = [("Board", "/boards", KANBAN)];

/// The document title: the page's own parts, most specific first, then the
/// app ("Edit organization - Acme - iter"). `error` puts "Error: " in front:
/// a form that was refused, so a screen reader announces the failure first.
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

/// `title` is the parts of the document title (see [`document_title`]; none
/// is just "iter"); `error` is for a form that was refused. The `<title>` is written here, first in the body, and
/// not in the layout's `<head>`, because the layout renders before the page
/// knows its own title; browsers take the first `<title>` of the document.
/// `current` is the `href` of the nav item the page belongs to, if any
/// (it gets `aria-current`). `styles` are the page's own stylesheets,
/// loaded after `/v2.css`.
#[component]
pub async fn frame(
    title: &[&str],
    #[default] error: bool,
    #[default] current: &str,
    #[default] styles: &[&'static str],
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <title>(document_title(title, error))</title>
        <link rel="stylesheet" href="/v2.css">
        for href in styles.iter() {
            <link rel="stylesheet" href=(*href)>
        }
        <div class="v2">
            <a class="skip" href="#main">"Skip to content"</a>
            <header class="bar">
                <a class="brand" href="/">(icon(MARK)) "iter"</a>
                <nav aria-label="Main">
                    for (label, href, svg) in NAV.iter() {
                        <a href=(*href) if *href == current { aria-current="true" }>(icon(svg)) (*label)</a>
                    }
                </nav>
            </header>
            <main id="main" tabindex="-1">(child)</main>
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
