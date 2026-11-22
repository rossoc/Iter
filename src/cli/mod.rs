//! The command line: the clap types (`args`), one module per group of subcommands, shell
//! completion, and the pieces they are built from (`entity`, `output`, `editor`). The only
//! module besides `web` that may name `clap`.

pub mod agent;
pub mod args;
#[cfg(test)]
mod argv_tests;
pub mod board;
pub mod complete;
#[cfg(test)]
mod config_instructions_tests;
pub mod editor;
pub mod entity;
#[cfg(test)]
mod help_tests;
pub mod ingest;
pub mod misc;
pub mod note;
pub mod organization;
pub mod output;
pub mod project;
mod run;
pub mod session;
pub mod solve;
pub mod sync;
pub mod tag;
pub mod task;
pub mod wiki;

pub use run::{Run, parse, run};
