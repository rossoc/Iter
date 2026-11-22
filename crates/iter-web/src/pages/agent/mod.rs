//! Agent pages: the agent log and instructions, and the runs and steps of a task.

use crate::prelude::*;

pub mod agents;
pub mod instructions;
pub mod task_agent;
pub mod task_compose;
pub mod task_runs;

pub fn routes(builder: RouterBuilder) -> RouterBuilder {
    instructions::routes(builder)
        .page(agents::list_page)
        .page(agents::detail_page)
        .route(task_agent::mark_step)
        .route(task_runs::create_run)
        .route(task_runs::edit_run)
        .route(task_runs::run_now)
        .route(task_runs::cancel_run)
        .route(task_runs::prompt_of)
}
