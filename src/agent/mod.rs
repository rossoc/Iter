//! What an agent works with on a task: its folder, plan, prompts and runs.
//! See `doc/architecture_agent_wiki.md`.

#[cfg(test)]
mod config_instructions_tests;
pub mod exec;
pub mod instruction;
pub mod plan;
pub mod prompt;
pub mod run;
pub mod solve;
pub mod task_prompt;
pub mod taskdir;
