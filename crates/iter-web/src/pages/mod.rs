//! The pages, one folder per subject. Each folder's `routes` registers its handlers; ranks
//! inside `pages`: `org`, `board`, `agent`, `site` < `project` < `task`.

pub mod agent;
pub mod board;
pub mod org;
pub mod project;
pub mod site;
pub mod task;
