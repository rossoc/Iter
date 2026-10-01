//! The document every page shares (`<html>`, the viewport, the language, the
//! footer after the page) and the script the board pages load. The stylesheet is linked by the page's
//! `frame` (`ui/frame.rs`), which knows which extra sheets it needs.

use super::assets::Asset;
use super::footer::site_footer_for;
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, Slot, layout, response::Response, route},
    view::{View, view},
};

static BOARD_JS: Asset = Asset::js(include_str!("board.js"));

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.layout(root).route(board_script)
}

#[layout("/")]
async fn root(slot: Slot<'_>) -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
            </head>
            <body>(slot) site_footer_for()</body>
        </html>
    })
}

#[route(GET "/board.js")]
async fn board_script(cx: &Cx) -> Result<Response> {
    BOARD_JS.respond(cx)
}
