// The organization report view (`web/org.rs`) nests several components; the
// compiler's `Send` check on the resulting future needs more than the
// default depth of 128.
#![recursion_limit = "256"]

mod agent;
mod app;
mod cli;
mod config;
mod db;
mod error;
mod files;
#[cfg(test)]
mod fixtures;
mod front;
mod git;
mod github;
mod locate;
mod models;
mod note_ref;
mod notes;
mod platform;
mod process;
mod push;
mod reporting;
mod scaffold;
mod secret;
mod sync;
mod text;
mod tmux;
#[cfg(feature = "web")]
mod web;
mod wiki;
mod workspace;

use app::App;
use error::IterError;
use std::process::ExitCode;

fn main() -> ExitCode {
    // before anything can spawn a process (or a thread): no child is handed the platform key
    secret::take_env_key();
    let args = cli::parse();
    let app = &match App::open() {
        Ok(app) => app,
        Err(e @ IterError::OpenDb { .. }) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let result = match &args.command {
        #[cfg(feature = "web")]
        cli::args::Command::Serve { port, no_open } => web::serve(*port, !*no_open),
        _ => cli::run(&args, app),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
