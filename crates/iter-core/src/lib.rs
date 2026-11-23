//! Everything `iter` knows that no front end owns: the database, the models, tmux, git, GitHub,
//! the agent runs and the wiki. It reads no command line and renders no page, so any front end
//! (the CLI, the web server, a desktop app) can sit on top of it.

pub mod agent;
pub mod app;
pub mod config;
pub mod db;
pub mod error;
pub mod files;
#[cfg(any(test, feature = "test-support"))]
pub mod fixtures;
pub mod front;
pub mod git;
pub mod github;
#[cfg(test)]
mod live_tests;
pub mod locate;
pub mod models;
pub mod note_ref;
pub mod notes;
pub mod platform;
pub mod process;
pub mod push;
pub mod reporting;
pub mod scaffold;
pub mod secret;
pub mod sync;
pub mod text;
pub mod tmux;
pub mod wiki;
pub mod workspace;
