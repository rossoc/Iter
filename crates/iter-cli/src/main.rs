//! The `iter` binary: the command line (`args`), one module per group of subcommands, shell
//! completion, and the pieces they are built from (`entity`, `output`, `editor`). The only
//! crate that names `clap`, and, through the `web` feature, the one that starts `iter-web`.

mod agent;
mod args;
#[cfg(test)]
mod argv_tests;
mod board;
mod complete;
#[cfg(test)]
mod config_instructions_tests;
mod editor;
mod entity;
#[cfg(test)]
mod help_tests;
mod ingest;
mod instruction;
mod misc;
mod note;
mod organization;
mod output;
mod project;
mod run;
mod session;
mod solve;
mod sync;
mod tag;
mod task;
mod wiki;

use iter_core::app::App;
use iter_core::error::IterError;
use run::{Run, parse};
use std::process::ExitCode;

fn main() -> ExitCode {
    // before anything can spawn a process (or a thread): no child is handed the platform key
    iter_core::secret::take_env_key();
    let args = parse();
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
        args::Command::Serve { port, no_open } => iter_web::serve(*port, !*no_open),
        _ => run::run(&args, app),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
