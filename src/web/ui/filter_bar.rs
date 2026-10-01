//! The search box above a list: a GET form (works without script) with one
//! text field named `q`, a Filter button, a "?" that shows the syntax the
//! field understands, and a Clear link when the list is filtered.
//!
//! The "?" needs no script either: hovering it shows the syntax, and
//! clicking it focuses it, which keeps the syntax shown (`:focus-within`)
//! until a click anywhere else takes the focus away.

use topcoat::{
    Result,
    view::{View, component, view},
};

/// The id of the field (the label and the tests point at it).
pub const FILTER_ID: &str = "filter-q";

/// The id of the syntax the "?" shows (its description).
const SYNTAX_ID: &str = "filter-syntax";

/// What a filter bar shows and where it sends (see `web/task_query.rs` and
/// `web/project_rows.rs` for the syntaxes).
pub struct Search {
    /// Where the form goes (the page itself).
    pub action: String,
    /// A `tab` the page needs kept (`tasks`), or empty for none.
    pub tab: &'static str,
    /// The text in the field.
    pub q: String,
    /// What the field is called for a screen reader ("Filter tasks").
    pub label: &'static str,
    /// Where Clear leads, when the list is filtered.
    pub clear: Option<String>,
    /// `(term, meaning)` of the syntax, which the "?" shows. Every search
    /// box has one.
    pub syntax: &'static [(&'static str, &'static str)],
}

#[component]
pub async fn filter_bar(search: &Search) -> Result<impl View> {
    debug_assert!(
        !search.syntax.is_empty(),
        "a search box explains its syntax"
    );
    Ok(view! {
        <form class="filter" role="search" method="get" action=(search.action.as_str())>
            if !search.tab.is_empty() {
                <input type="hidden" name="tab" value=(search.tab)>
            }
            <label class="sr" for=(FILTER_ID)>(search.label)</label>
            <input class="control" type="search" id=(FILTER_ID) name="q" value=(search.q.as_str()) spellcheck="false" autocomplete="off" autocapitalize="off">
            <button class="button" type="submit">"Filter"</button>
            <span class="filter-help">
                // tabindex: Safari focuses a button on click only with one
                <button class="filter-help-btn" type="button" tabindex="0" aria-describedby=(SYNTAX_ID)>
                    <span aria-hidden="true">"?"</span><span class="sr">"Search syntax"</span>
                </button>
                // tabindex -1: a click inside keeps the focus here, so it stays open
                <div class="filter-tip" id=(SYNTAX_ID) role="tooltip" tabindex="-1">
                    <dl>
                        for (term, meaning) in search.syntax.iter() {
                            <dt><code>(*term)</code></dt>
                            <dd>(*meaning)</dd>
                        }
                    </dl>
                </div>
            </span>
            if let Some(href) = &search.clear {
                <a class="u filter-clear" href=(href.as_str())>"Clear"</a>
            }
        </form>
    })
}
