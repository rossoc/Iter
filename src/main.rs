// The organization report view (`web/org.rs`) nests several components; the
// compiler's `Send` check on the resulting future needs more than the
// default depth of 128.
#![recursion_limit = "256"]

mod agent;
mod app;
mod args;
mod commands;
mod completion;
mod config;
mod db;
mod editor;
mod error;
mod files;
#[cfg(test)]
mod fixtures;
mod front;
mod git;
mod github;
#[cfg(test)]
mod help_tests;
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
mod utils;
#[cfg(feature = "web")]
mod web;
mod wiki;
mod workspace;

use app::App;
use args::{Args, Command, ProjectCommand};
use clap::{CommandFactory, Parser};
use clap_complete::env::CompleteEnv;
use commands::{Run, misc, project, task};
use error::IterError;
use models::TaskStatus;
use std::process::ExitCode;

fn cli() -> clap::Command {
    Args::command().name("iter")
}

fn main() -> ExitCode {
    // before anything can spawn a process (or a thread): no child is handed the platform key
    secret::take_env_key();
    // Shell-driven completion (`COMPLETE=<shell> iter ...`, on every Tab once registered).
    CompleteEnv::with_factory(cli).complete();

    let args = Args::parse();
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
        Command::Init { organization } => project::init_cmd(app, organization.as_deref()),
        Command::New { path, organization } => project::new_cmd(app, path, organization.as_deref()),
        Command::Clone {
            source,
            organization,
            board,
            no_board,
        } => project::clone_cmd(
            app,
            source,
            organization.as_deref(),
            board.as_deref(),
            *no_board,
        ),
        Command::Board { action } => action.run(app),
        Command::Tag { action } => action.run(app),
        Command::Organization { action } => action.run(app),
        // No subcommand is exactly `project list`, so the default stands in
        // for the variant rather than being a separate arm.
        Command::Project { action } => action.as_ref().unwrap_or(&ProjectCommand::List).run(app),
        // `iter task` with no subcommand is *not* any `TaskCommand`: it
        // means the unfinished work, a filter `TaskCommand::List` cannot
        // express. So this one keeps an explicit arm.
        Command::Task { action } => match action {
            Some(action) => action.run(app),
            None => task::task_list(app, None, &[TaskStatus::Queue, TaskStatus::Wip]),
        },
        Command::Wiki { action } => action.run(app),
        Command::Note { action } => match action {
            Some(action) => action.run(app),
            None => commands::note::today(app, &args::NotesScope::default()),
        },
        Command::Session { action } => action.run(app),
        Command::Comment { task, message } => misc::comment_cmd(app, task.as_deref(), message),
        Command::T => misc::t_cmd(app),
        #[cfg(feature = "web")]
        Command::Serve { port, no_open } => web::serve(*port, !*no_open),
        Command::Internal { action } => action.run(app),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
