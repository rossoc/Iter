//! Wrapper of a board's drag-and-drop area: `data-post` is where `board.js`
//! sends a drop, `style` carries the priority colors. Starts with the notes
//! region and loads the script: a board page has both or neither.

use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// `style` is checked CSS custom properties; `note` is what the last move did,
/// empty for none (`board.js` rewrites it after load: a live region reads only changes).
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
