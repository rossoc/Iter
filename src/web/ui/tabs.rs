//! Section tabs with one sliding underline (`tabs.css`). The caller gives each
//! tab its finished `href`.

use topcoat::{
    Result,
    view::{View, component, view},
};

pub struct Tab {
    label: &'static str,
    href: String,
    current: bool,
    count: Option<usize>,
}

impl Tab {
    pub fn new(label: &'static str, href: String, current: bool) -> Tab {
        Tab {
            label,
            href,
            current,
            count: None,
        }
    }

    /// A tab that is not (yet) the current one: `sections::tabs_for` marks it.
    pub fn link(label: &'static str, href: String) -> Tab {
        Tab::new(label, href, false)
    }

    #[cfg(test)]
    pub fn is_current(&self) -> bool {
        self.current
    }

    /// The same tab, current or not.
    pub fn marked(mut self, current: bool) -> Tab {
        self.current = current;
        self
    }

    #[cfg(test)]
    pub fn href(&self) -> &str {
        &self.href
    }

    #[cfg(test)]
    pub fn badge(&self) -> Option<usize> {
        self.count
    }

    /// A count badge after the label.
    pub fn count(mut self, count: usize) -> Tab {
        self.count = Some(count);
        self
    }
}

/// `label` names the navigation for screen readers ("Sections").
#[component]
pub async fn tabs(label: &str, items: &[Tab]) -> Result<impl View> {
    Ok(view! {
        <nav class="tabs" aria-label=(label.to_string())>
            for t in items.iter() {
                <a href=(t.href.as_str()) if t.current { aria-current="page" }>
                    (t.label)
                    if let Some(n) = t.count {
                        <span class="count">(n)</span>
                    }
                </a>
            }
        </nav>
    })
}
