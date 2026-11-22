//! The document every page shares (`<html>`, stylesheet, footer) and the script the board pages load.

use super::assets::{Asset, serve};
use super::footer::site_footer_for;
use topcoat::{
    Result,
    router::{RouterBuilder, Slot, layout},
    view::{View, view},
};

static BOARD_JS: Asset = Asset::js(include_str!("board.js"));
static FAVICON: Asset = Asset::icon(include_bytes!("favicon.ico"));

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    serve(
        builder.layout(root),
        &[("/board.js", &BOARD_JS), ("/favicon.ico", &FAVICON)],
    )
}

#[layout("/")]
async fn root(slot: Slot<'_>) -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <link rel="icon" href="/favicon.ico" sizes="16x16 32x32 48x48">
                <link rel="stylesheet" href="/style.css">
            </head>
            <body>(slot) site_footer_for()</body>
        </html>
    })
}
