mod app;
mod args;
mod commands;
mod completion;
mod config;
mod db;
mod error;
mod git;
mod github;
mod md_edit;
mod models;
mod process;
mod reporting;
mod scaffold;
mod sync;
mod tmux;
mod utils;

use app::App;
use args::{Args, Command, ProjectCommand};
use clap::{CommandFactory, Parser};
use clap_complete::env::CompleteEnv;
use commands::{Run, misc, project, task};
use models::TaskStatus;
use std::process::ExitCode;

fn cli() -> clap::Command {
    Args::command().name("iter")
}

fn main() -> ExitCode {
    // Shell-driven dynamic completion: when invoked as `COMPLETE=<shell> iter
    // ...` (which the shell does behind the scenes on every Tab press, once
    // `source <(COMPLETE=zsh iter)` has registered it)
    CompleteEnv::with_factory(cli).complete();

    let args = Args::parse();
    // Opened once, and one `now` for the whole run -- see `app::App`.
    let app = &App::new();

    let result = match &args.command {
        Command::Init { organization } => project::init_cmd(app, organization.as_deref()),
        Command::New { path, organization } => project::new_cmd(app, path, organization.as_deref()),
        Command::Clone {
            source,
            organization,
        } => project::clone_cmd(app, source, organization.as_deref()),
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
        Command::Session { action } => action.run(app),
        Command::Comment { task, message } => misc::comment_cmd(app, task.as_deref(), message),
        Command::T => misc::t_cmd(app),
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

// ---- sessions: shared by manual start/stop and tmux hooks -----------------

// ---- init / new / clone ---------------------------------------------------

// ---- editing ---------------------------------------------------------

// ---- organization ----------------------------------------------------

// ---- project ---------------------------------------------------------

// ---- task --------------------------------------------------------------

// ---- session -------------------------------------------------------------

// ---- comment / t / internal hook -----------------------------------------
