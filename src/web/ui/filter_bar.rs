//! A GET form (works without script): text field `q` with a "?" inside its
//! right end showing the syntax, a Filter button, and Clear when filtered. The "?"
//! needs no script: hover shows the syntax, click focuses it and keeps it shown
//! (`:focus-within`); Escape closes it (`filter_bar.js`).

use topcoat::{
    Result,
    view::{View, component, view},
};

pub const FILTER_ID: &str = "filter-q";

const SYNTAX_ID: &str = "filter-syntax";

/// What a filter bar shows and where it sends (syntaxes in `web/task_query.rs`, `web/project_rows.rs`).
pub struct Search {
    pub action: String,
    pub tab: &'static str,
    pub q: String,
    pub label: &'static str,
    pub clear: Option<String>,
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
            <span class="filter-field">
                <input class="control" type="search" id=(FILTER_ID) name="q" value=(search.q.as_str()) spellcheck="false" autocomplete="off" autocapitalize="off">
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
            </span>
            <button class="button" type="submit">"Filter"</button>
            if let Some(href) = &search.clear {
                <a class="u filter-clear" href=(href.as_str())>"Clear"</a>
            }
        </form>
        <script src="/filter_bar.js" defer=""></script>
    })
}
