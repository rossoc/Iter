//! `iter serve`: a browser UI over the same database the CLI uses.
//!
//! A local topcoat server, bound to loopback only. Nothing is copied: every
//! request opens the configured database file itself (WAL + busy timeout,
//! so it coexists with a concurrent `iter` command), reads or writes through
//! the same `Db` methods the CLI does, and drops the connection again. A
//! `rusqlite::Connection` isn't `Sync`, which is the other reason to open
//! one per request instead of sharing one.

mod board;
mod forms;
mod guard;
mod layout;
mod pages;

use crate::config::config;
use crate::db::Db;
use crate::error::Result;
use topcoat::router::Router;

/// A connection for the duration of one request.
fn open_db() -> topcoat::Result<Db> {
    Ok(Db::open(config().db_path()?)?)
}

fn router() -> Router {
    let builder = Router::builder().layer(guard::local_only);
    board::register(pages::register(builder)).build()
}

/// Serves until interrupted, optionally opening the browser first.
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
            // Best effort: the URL is printed either way.
            let _ = std::process::Command::new(opener).arg(&url).spawn();
        }
        topcoat::serve(listener, router()).await?;
        Ok(())
    })
}
