//! The wrapper of a board's drag-and-drop area: `data-post` is where
//! `board.js` sends a drop, and `style` carries the priority colors
//! (`--urgent`, `--important`) the cards' edges use. It starts with the
//! notes region, where the page says a move was done (`note`) and `board.js`
//! says a drop failed, and it loads the script: a board page has both or
//! neither.

use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// `style` is CSS custom properties, already checked by the caller. `note` is
/// what the last move did ("Moved Task to Plan"), empty for none: the notes
/// region is a status, so a screen reader reads it (`board.js` writes it
/// again after the load, as a live region only reads a change).
#[component]
pub async fn board_root(
    #[into] post: String,
    #[default] style: &str,
    #[default] note: &str,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <div class="board-root" data-post=(post.as_str()) if !style.is_empty() { style=(style) }>
            <div class="board-notes" role="status">
                if !note.is_empty() {
                    <p class="notice">(note)</p>
                }
            </div>
            (child)
        </div>
        <script src="/board.js" defer=""></script>
    })
}
