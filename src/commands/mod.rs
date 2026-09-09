//! One module per group of subcommands.
//!
//! Each exposes a single [`Run`] implementation over the clap enum it
//! handles, so the crate root reaches a whole family of commands through
//! one name and everything else in the file stays private to it.

pub mod misc;
pub mod organization;
pub mod project;
pub mod session;
pub mod sync;
pub mod task;

use crate::app::App;
use crate::error::Result;

/// A parsed subcommand that can be carried out.
///
/// Deliberately implemented per *enum* rather than per variant: clap has
/// already produced the typed value, so the inner `match` is the dispatch
/// and a trait per variant would only add a struct and an impl for each one
/// without removing it. What this does buy is encapsulation -- the command
/// bodies stop being crate-root functions and each module exports one name.
///
/// Always used with static dispatch. A `dyn Run` would force each variant's
/// data to be owned, where today the bodies borrow straight out of the
/// parsed `Args`.
pub trait Run {
    fn run(&self, app: &App) -> Result<()>;
}
