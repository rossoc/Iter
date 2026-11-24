//! `iter serve`: a browser UI over the same database the CLI uses; one connection per request.
//! Architecture and route map: `doc/web_server.md`. The one public item is [`serve`].
//!
//! A module imports only modules before it in this order: `cx`, `url`, `assets`, `layers`, then
//! `prelude` and `ui`, then `load` and `forms`, then `parts`, then `pages`.

// The organization report view (`pages/org/org.rs`) nests several components; the compiler's
// `Send` check on the resulting future needs more than the default depth of 128.
#![recursion_limit = "256"]

mod assets;
mod cx;
mod forms;
mod layers;
#[cfg(test)]
mod live_tests;
mod load;
#[cfg(test)]
mod modal_tests;
mod pages;
mod parts;
mod prelude;
#[cfg(test)]
mod script_tests;
#[cfg(test)]
mod task_note_tests;
#[cfg(test)]
mod toast_tests;
mod ui;
mod url;

use iter_core::error::Result;
use topcoat::router::{LayerFn, Router};

fn router() -> Router {
    let builder = Router::builder()
        .layer(layers::blocking::off_the_workers)
        .layer(layers::guard::security_headers)
        .layer(layers::guard::local_only)
        .layer(layers::flash::flash)
        .layer(LayerFn::new(
            None::<&'static str>,
            layers::errors::error_pages,
        ));
    [
        pages::site::routes,
        ui::register,
        pages::org::routes,
        pages::project::routes,
        pages::task::routes,
        pages::agent::routes,
        pages::board::routes,
    ]
    .into_iter()
    .fold(builder, |builder, routes| routes(builder))
    .build()
}

/// Serves the UI on `127.0.0.1:<port>` until the process is stopped; `open` also opens a browser.
pub fn serve(port: u16, open: bool) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
        let url = format!("http://127.0.0.1:{port}");
        println!("iter is serving {url} (Ctrl-C to stop)");
        if open {
            let opener = if cfg!(target_os = "macos") {
                "open"
            } else {
                "xdg-open"
            };
            let _ = std::process::Command::new(opener).arg(&url).spawn();
        }
        topcoat::serve(listener, router()).await?;
        Ok(())
    })
}
