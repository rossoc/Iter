//! Parsing the command line and carrying a parsed command out.

use super::args::{Args, Command, NotesScope, ProjectCommand};
use super::{misc, note, project, task};
use crate::app::App;
use crate::error::Result;
use crate::models::TaskStatus;
use clap::{CommandFactory, Parser};
use clap_complete::env::CompleteEnv;

/// A parsed subcommand that can be carried out. Implemented per enum, not per variant, and used with static dispatch:
/// the bodies borrow from the parsed `Args`, which a `dyn Run` would force to be owned.
pub trait Run {
    fn run(&self, app: &App) -> Result<()>;
}

/// The command tree as clap sees it, named `iter` whatever the binary is called.
pub(super) fn command() -> clap::Command {
    Args::command().name("iter")
}

/// The arguments of this process. Shell-driven completion (`COMPLETE=<shell> iter ...`, on every
/// Tab once registered) is answered, and the process ends, before any argument is read.
pub fn parse() -> Args {
    CompleteEnv::with_factory(command).complete();
    Args::parse()
}

/// Carries out the command `args` names, `serve` apart (see `main`).
pub fn run(args: &Args, app: &App) -> Result<()> {
    match &args.command {
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
            None => note::today(app, &NotesScope::default()),
        },
        Command::Session { action } => action.run(app),
        Command::Comment { task, message } => misc::comment_cmd(app, task.as_deref(), message),
        Command::T => misc::t_cmd(app),
        // the one command the binary's root runs itself: the web front end is not the CLI's
        #[cfg(feature = "web")]
        Command::Serve { .. } => Err(crate::error::failed("`serve` is started by main")),
        Command::Internal { action } => action.run(app),
    }
}
