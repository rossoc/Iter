//! A trail of crumbs (built in `web/crumbs.rs`), the last being the page.
//! `aria-current="page"` goes on the current tab if the page has tabs
//! (`Crumb::label`), else on the last crumb (`Crumb::here`).

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

    /// The page itself, as plain text (its tab is the current item).
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
