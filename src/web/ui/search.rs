//! The search palette: `/` (or the bar's button) opens a box with a query, matching
//! pages and a live preview in a frame (bar, footer and palette hidden inside it).
//! All script: `search.js` fills the list from `GET /search?q=`
//! (`web/search_index.rs`); without script the button stays hidden.

use super::{BUILDING, FILE_TEXT, FOLDER, KANBAN, SEARCH, SQUARE_CHECK, icon};
use topcoat::{
    Result,
    view::{View, component, view},
};

#[component]
pub async fn search_button() -> Result<impl View> {
    Ok(view! {
        <button type="button" class="search-btn" data-palette-open="" aria-keyshortcuts="/" hidden="">
            (icon(SEARCH))
            "Search"
            <kbd>"/"</kbd>
        </button>
    })
}

#[component]
pub async fn palette() -> Result<impl View> {
    Ok(view! {
        <div class="palette" id="palette" hidden="">
            <div class="palette-backdrop" data-palette-close=""></div>
            <div class="palette-box" role="dialog" aria-modal="true" aria-label="Search">
                <div class="palette-query">
                    (icon(SEARCH))
                    <input class="palette-input" type="text" role="combobox" aria-label="Search pages"
                        aria-expanded="true" aria-controls="palette-list" aria-autocomplete="list"
                        autocomplete="off" spellcheck="false"
                        placeholder="Search…  proj web · task fix · org acme/proj api · is:wip">
                </div>
                <div class="palette-body">
                    <ul class="palette-list" id="palette-list" role="listbox" aria-label="Results"></ul>
                    <section class="palette-preview" aria-label="Preview">
                        <iframe class="palette-frame" title="Preview of the selected page"></iframe>
                    </section>
                </div>
                <footer class="palette-hint">
                    <span><kbd>"↑"</kbd><kbd>"↓"</kbd>" move"</span>
                    <span><kbd>"↵"</kbd>" open"</span>
                    <span><kbd>"tab"</kbd>" complete / preview"</span>
                    <span><kbd>"esc"</kbd>" close"</span>
                </footer>
                <div hidden="">
                    <template data-kind="board">(icon(KANBAN))</template>
                    <template data-kind="org">(icon(BUILDING))</template>
                    <template data-kind="project">(icon(FOLDER))</template>
                    <template data-kind="task">(icon(SQUARE_CHECK))</template>
                    <template data-kind="note">(icon(FILE_TEXT))</template>
                </div>
                <p class="sr" id="palette-status" aria-live="polite"></p>
            </div>
        </div>
    })
}
