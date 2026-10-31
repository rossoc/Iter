use crate::error::Result;
use crate::process::{Git, Tool};
use std::path::Path;

/// Whether `path` has a `.git` entry (a directory in a repo, a file in a worktree checkout).
pub fn is_git_repo(path: &str) -> bool {
    Path::new(path).join(".git").exists()
}

/// Lowercases `name`, collapses runs of non `[a-z0-9]` into one `-` and trims dashes, for branch
/// names and directories. May be empty; branch names go through [`task_slug`], which rules that out.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

/// The fixed part of a `branch_template` (`"feat/"` out of `"feat/{task}"`); a template without
/// `{task}` is returned whole.
pub fn branch_prefix(template: &str) -> String {
    match template.split_once("{task}") {
        Some((prefix, _)) => prefix.to_string(),
        None => template.to_string(),
    }
}

/// The slug of a task's branch *and* worktree directory (one slug, so uniqueness for one holds for
/// the other). A name with no ASCII letters or digits falls back to the task id: an empty slug makes
/// a bare-prefix branch git refuses and a worktree path equal to `.iter-worktrees` itself.
pub fn task_slug(name: &str, task_id: i64) -> String {
    let slug = slugify(name);
    match slug.is_empty() {
        true => format!("task-{task_id}"),
        false => slug,
    }
}

pub fn create_worktree(base_path: &str, branch: &str, worktree_path: &str) -> Result<()> {
    Git(Some(base_path)).run(&["worktree", "add", "-b", branch, worktree_path])
}

/// `--force`: the task is done, so a stray untracked file must not block cleanup.
pub fn remove_worktree(base_path: &str, worktree_path: &str) -> Result<()> {
    Git(Some(base_path)).run(&["worktree", "remove", "--force", worktree_path])
}

/// A branch name starting with `-` would be read by git as an option.
fn refuse_option(name: &str) -> Result<()> {
    if name.starts_with('-') {
        return Err(crate::error::IterError::CommandFailed(format!(
            "invalid branch name '{name}'"
        )));
    }
    Ok(())
}

/// `-D` for the same reason as `--force` above. Must run after the branch's worktree is removed:
/// git refuses to delete a checked-out branch.
pub fn delete_branch(base_path: &str, branch: &str) -> Result<()> {
    Git(Some(base_path)).run(&["branch", "-D", "--", branch])
}

/// The branch checked out at `path`; `None` if detached or not a repo (the same answer to "is this
/// branch checked out").
pub fn current_branch(path: &str) -> Option<String> {
    Git(Some(path)).output_trimmed(&["symbolic-ref", "--quiet", "--short", "HEAD"])
}

/// Merges `branch` into what `dir` has checked out, git printing its own output; reports whether
/// it was clean. A `false` is not an error and nothing is rolled back: the half-done merge state
/// (`git status` in `dir`) is the point, and the caller stops and says where.
pub fn merge(dir: &str, branch: &str) -> Result<bool> {
    refuse_option(branch)?;
    Git(Some(dir)).run_status(&["merge", branch])
}

/// Clones like `git clone`: `source` is a URL or local path; `dest` must not exist or be empty.
pub fn clone_repo(source: &str, dest: &str) -> Result<()> {
    Git(None).run(&["clone", "--", source, dest])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_lowercases_and_dashes_punctuation() {
        assert_eq!(slugify("Fix Login Bug!"), "fix-login-bug");
    }

    #[test]
    fn slugify_collapses_runs_and_trims_ends() {
        assert_eq!(slugify("  weird   Name__here--"), "weird-name-here");
    }

    #[test]
    fn branch_prefix_is_the_template_up_to_the_placeholder() {
        assert_eq!(branch_prefix("feat/{task}"), "feat/");
        assert_eq!(branch_prefix("{task}"), "");
    }

    #[test]
    fn branch_prefix_of_a_placeholderless_template_is_the_whole_thing() {
        assert_eq!(branch_prefix("wip/"), "wip/");
    }

    /// Without the fallback this is empty: a `feat/` branch git rejects and a worktree path equal to
    /// the container directory.
    #[test]
    fn a_name_with_nothing_sluggable_in_it_falls_back_to_the_task_id() {
        assert_eq!(task_slug("設計を見直す", 42), "task-42");
        assert_eq!(task_slug("!!! ---", 7), "task-7");
        assert_eq!(task_slug("", 3), "task-3");
    }
}
