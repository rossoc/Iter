//! What an agent works with on a task: its folder, its plan, the prompts
//! that tell it what to do, and the runs that carry it out.
//!
//! Where the agent side reaches for the wiki it says so: `task_prompt` asks it
//! for the onboarding, and `solve` starts the ingest the project's setting
//! asks for when a task's first run begins. Everything else here is
//! independent of it.

pub mod error;
pub mod exec;
pub mod instruction;
pub mod plan;
pub mod prompt;
pub mod run;
pub mod solve;
pub mod task_prompt;
pub mod taskdir;
