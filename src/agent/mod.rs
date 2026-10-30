//! What an agent works with on a task: its folder, its plan, and the prompts
//! that tell it what to do.
//!
//! Nothing here knows about the wiki. Where a prompt wants the project's
//! knowledge (the onboarding), the caller passes it in as text, so the wiki can
//! be replaced without touching this module.

pub mod plan;
pub mod prompt;
pub mod taskdir;
