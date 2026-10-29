//! The document every page shares (`<html>`, the viewport, the language, the
//! one stylesheet, the footer after the page) and the script the board pages
//! load. The stylesheet is linked in `<head>`, so the browser has it before
//! it paints the body: the page is never shown unstyled first.

use super::assets::Asset;
use super::footer::site_footer_for;
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, Slot, layout, response::Response, route},
    view::{View, view},
};

static BOARD_JS: Asset = Asset::js(include_str!("board.js"));
static FAVICON: Asset = Asset::icon(include_bytes!("favicon.ico"));

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.layout(root).route(board_script).route(favicon)
}

#[layout("/")]
async fn root(slot: Slot<'_>) -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <link rel="icon" href="/favicon.ico" sizes="32x32">
                <link rel="stylesheet" href="/style.css">
            </head>
            <body>(slot) site_footer_for()</body>
        </html>
    })
}

#[route(GET "/board.js")]
async fn board_script(cx: &Cx) -> Result<Response> {
    BOARD_JS.respond(cx)
}

#[route(GET "/favicon.ico")]
async fn favicon(cx: &Cx) -> Result<Response> {
    FAVICON.respond(cx)
}
