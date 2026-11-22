//! Views more than one page folder renders (edit and create screens, tab rows, task and project
//! rows, notes). `all` is the glob a page imports (`use crate::parts::all::*`).

pub mod board_header;
pub mod create;
pub mod crumbs;
pub mod edit_page;
pub mod notes;
pub mod project_options;
pub mod project_rows;
pub mod sections;
pub mod session_lines;
pub mod settings_fields;
pub mod settings_panel;
pub mod task_fields;
pub mod task_rows;
pub mod words;

pub(crate) mod all {
    pub use super::board_header::*;
    pub use super::create::*;
    pub use super::crumbs::*;
    pub use super::edit_page::*;
    pub use super::notes::*;
    pub use super::project_options::*;
    pub use super::project_rows::*;
    pub use super::sections::*;
    pub use super::session_lines::*;
    pub use super::settings_fields::*;
    pub use super::settings_panel::*;
    pub use super::task_fields::*;
    pub use super::task_rows::*;
    pub use super::words::*;
}
