//! `iter serve`: a browser UI over the same database the CLI uses.
//!
//! A local topcoat server, bound to loopback only. Nothing is copied: every
//! request opens the configured database file itself (WAL + busy timeout,
//! so it coexists with a concurrent `iter` command), reads or writes through
//! the same `Db` methods the CLI does, and drops the connection again. A
//! `rusqlite::Connection` isn't `Sync`, which is the other reason to open
//! one per request instead of sharing one.

mod agenda;
mod agenda_cards;
mod agenda_day;
mod agenda_drop;
mod agenda_pick;
mod assets;
mod blocking;
mod board_cards;
mod board_edit;
mod board_header;
mod board_info;
mod board_page;
mod boards;
mod crumbs;
mod drop;
mod edit;
mod edit_page;
mod errors;
mod forms;
mod guard;
mod home;
mod layout;
mod load;
mod matrix;
mod notes;
mod org;
mod org_edit;
mod org_report;
mod pick;
mod project;
mod project_edit;
mod project_options;
mod project_rows;
mod quadrants;
mod sections;
mod session_lines;
mod settings_fields;
mod settings_panel;
mod task;
mod task_edit;
mod task_fields;
mod task_new;
mod task_panel;
mod task_rows;
mod ui;
mod url;

use crate::config::config;
use crate::db::Db;
use crate::error::Result;
use topcoat::router::{Router, path_param};

// The `{id}` of every route with one: a number, else a 400.
path_param!(id: i64, error = bad_request);

/// A connection for the duration of one request.
fn open_db() -> topcoat::Result<Db> {
    Ok(Db::open(config().db_path()?)?)
}

/// The local time now: what every page measures "today" and open sessions
/// against.
fn now() -> chrono::NaiveDateTime {
    chrono::Local::now().naive_local()
}

fn router() -> Router {
    let builder = Router::builder()
        .layer(blocking::off_the_workers)
        .layer(guard::local_only)
        .layer(errors::error_pages);
    [
        layout::register,
        home::register,
        ui::register,
        org::register,
        project::register,
        task::register,
        org_edit::register,
        project_edit::register,
        task_edit::register,
        task_new::register,
        boards::register,
        matrix::register,
        agenda::register,
        agenda_drop::register,
        board_info::register,
        board_edit::register,
    ]
    .into_iter()
    .fold(builder, |builder, register| register(builder))
    .build()
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
