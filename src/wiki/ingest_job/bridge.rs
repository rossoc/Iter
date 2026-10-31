//! Bridging a task's own notes into the project's note of the day.

use super::*;

/// On `task done`: appends the task's notes to the project's note of today under `## Task: <name>`, one `###` per file.
/// Returns how many files went in; none if there was nothing to add or the section exists.
pub fn bridge_task_notes(app: &App, project: &Project, task: &Task) -> Result<usize> {
    let dir = crate::agent::taskdir::of(&app.db, project, task)?;
    let files = crate::agent::taskdir::TaskDir::files(&dir.notes_dir());
    let mut body = String::new();
    let mut count = 0;
    for file in &files {
        let text = std::fs::read_to_string(&file.path).unwrap_or_default();
        if text.trim().is_empty() {
            continue;
        }
        body.push_str(&format!("### {}\n\n{}\n\n", file.name, text.trim()));
        count += 1;
    }
    if count == 0 {
        return Ok(0);
    }
    let org = match project.organization_id {
        Some(id) => app.db.get::<crate::models::Organization>(id)?,
        None => None,
    };
    let notes_dir = crate::notes::project_dir(project, &org).ok_or(AgentError::NoNotesFolder)?;
    let today = chrono::Local::now().date_naive();
    let added =
        crate::notes::append_section(&notes_dir, today, &format!("Task: {}", task.name), &body)?;
    Ok(if added { count } else { 0 })
}
