//! One module per group of subcommands, each exposing a single [`Run`] implementation over its clap enum.

pub mod agent;
pub mod board;
pub mod ingest;
pub mod misc;
pub mod notes;
pub mod organization;
pub mod project;
pub mod session;
pub mod solve;
pub mod sync;
pub mod tag;
pub mod task;
pub mod wiki;

use crate::app::App;
use crate::error::Result;

/// A parsed subcommand that can be carried out. Implemented per enum, not per variant, and used with static dispatch:
/// the bodies borrow from the parsed `Args`, which a `dyn Run` would force to be owned.
pub trait Run {
    fn run(&self, app: &App) -> Result<()>;
}
