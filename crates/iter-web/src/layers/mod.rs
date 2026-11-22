//! The layers every request passes through (see `router` in `lib.rs`): off the async workers,
//! security headers, local-only guard, flash cookies, error pages.

pub mod blocking;
pub mod errors;
pub mod flash;
pub mod guard;
