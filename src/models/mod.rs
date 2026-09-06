mod project;
mod record;
mod session;
mod task;

pub use project::Project;
pub use record::Record;
pub use session::Session;
pub use task::{Task, TaskStatus};

/// A type edited via `yaml_edit::edit_in_nvim` that carries a free-form
/// markdown `description`. The field is `#[serde(skip)]`ed on the type
/// itself and instead placed below a `---` line, after every other field,
/// so it's written and read back as plain markdown rather than a
/// quoted/escaped YAML string.
pub trait MarkdownBody {
    fn description(&self) -> &str;
    fn set_description(&mut self, description: String);
}
