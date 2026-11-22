//! What a front end shows, as data: no markup, no URL, no DOM id. The task query box, the global
//! search, the priority matrix and the agenda day, each pure and tested without a page.

#![cfg_attr(not(feature = "web"), allow(dead_code))]

pub mod day;
pub mod matrix;
pub mod query;
pub mod search;
