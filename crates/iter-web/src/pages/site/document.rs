//! The document every page shares (`<html>`, stylesheet, footer) and the script the board pages load.

use crate::{
    assets::{Asset, serve},
    pages::site::footer::site_footer_for,
    prelude::*,
};
use topcoat::router::{Slot, layout};

static BOARD_JS: Asset = Asset::js(include_str!("board.js"));
static FAVICON: Asset = Asset::icon(include_bytes!("favicon.ico"));

pub(super) fn register(builder: RouterBuilder) -> RouterBuilder {
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
