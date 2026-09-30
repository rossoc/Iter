//! Where a page sits: a trail of crumbs (built in `web/crumbs.rs`), the last
//! item being the page.
//!
//! One rule for `aria-current="page"`: a page with tabs marks its current
//! tab (`Crumb::label` for its own crumb); a page without marks its last
//! crumb (`Crumb::here`). So a page never has two.

use topcoat::{
    Result,
    view::{View, component, view},
};

pub struct Crumb {
    pub label: String,
    pub href: Option<String>,
    pub current: bool,
}

impl Crumb {
    /// An ancestor, linked.
    pub fn link(label: impl Into<String>, href: impl Into<String>) -> Crumb {
        Crumb {
            label: label.into(),
            href: Some(href.into()),
            current: false,
        }
    }

    /// The page itself, as plain text: for a page whose tab is the current
    /// item (see the module doc).
    pub fn label(label: impl Into<String>) -> Crumb {
        Crumb {
            label: label.into(),
            href: None,
            current: false,
        }
    }

    /// The current page (no link).
    pub fn here(label: impl Into<String>) -> Crumb {
        Crumb {
            label: label.into(),
            href: None,
            current: true,
        }
    }
}

#[component]
pub async fn breadcrumb(items: &[Crumb]) -> Result<impl View> {
    Ok(view! {
        <nav class="crumbs" aria-label="Breadcrumb">
            <ol class="label">
                for c in items.iter() {
                    <li>
                        if let Some(href) = &c.href {
                            <a href=(href.as_str())>(c.label.as_str())</a>
                        } else {
                            <span if c.current { aria-current="page" }>(c.label.as_str())</span>
                        }
                    </li>
                }
            </ol>
        </nav>
    })
}
