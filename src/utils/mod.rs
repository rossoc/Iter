//! Shared machinery the commands are built from -- everything that is not
//! itself a command.
//!
//! Each file here is named for a concept rather than for a subcommand,
//! because that is how the code actually clusters: of the helpers shared by
//! more than one command in `main.rs`, the great majority were used by
//! commands in two or more different groups. Splitting them by subcommand
//! would have scattered them; splitting them by what they are about keeps
//! each one in a single place with a single reason to change.

pub mod clock;
pub mod output;
pub mod report;
pub mod resolve;
pub mod workspace;
