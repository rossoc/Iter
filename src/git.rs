use crate::error::Result;
use crate::process;
use std::path::Path;

/// Whether `path` looks like a git repo (has a `.git` entry -- a plain repo
/// has a `.git` directory, a worktree checkout has a `.git` file).
pub fn is_git_repo(path: &str) -> bool {
    Path::new(path).join(".git").exists()
}

/// Lowercases `name` and collapses every run of non `[a-z0-9]` characters
/// into a single `-`, trimming leading/trailing dashes -- used to turn a
/// free-text task name into something safe for a branch name / directory.
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

/// The fixed part of a project's `branch_template` (e.g. `"feat/"` out of
/// `"feat/{task}"`) -- what a task carries as its own, editable
/// `branch_prefix`. A template with no `{task}` placeholder is a prefix
/// already, and is returned whole.
pub fn branch_prefix(template: &str) -> String {
    match template.split_once("{task}") {
        Some((prefix, _)) => prefix.to_string(),
        None => template.to_string(),
    }
}

/// A task's branch name: its `branch_prefix` followed by its slugified name.
pub fn branch_name(prefix: &str, task_name: &str) -> String {
    format!("{prefix}{}", slugify(task_name))
}

/// Creates a new worktree at `worktree_path`, on a new branch `branch`,
/// checked out from `base_path`'s current `HEAD`.
pub fn create_worktree(base_path: &str, branch: &str, worktree_path: &str) -> Result<()> {
    process::run(
        "git",
        Some(base_path),
        &["worktree", "add", "-b", branch, worktree_path],
    )
}

/// Removes a worktree created by `create_worktree`. `--force` because the
/// task is done and we don't want a stray untracked file to block cleanup.
pub fn remove_worktree(base_path: &str, worktree_path: &str) -> Result<()> {
    process::run(
        "git",
        Some(base_path),
        &["worktree", "remove", "--force", worktree_path],
    )
}

/// Deletes a branch created by `create_worktree`. `-D` (not `-d`), like
/// `remove_worktree`'s `--force`: the task is done and we don't want
/// unmerged commits to block cleanup. Must run after the branch's worktree
/// (if any) has already been removed -- git refuses to delete a branch
/// that's still checked out in one.
pub fn delete_branch(base_path: &str, branch: &str) -> Result<()> {
    process::run("git", Some(base_path), &["branch", "-D", branch])
}

/// Clones `source` into `dest`, exactly like `git clone <source> <dest>` --
/// `source` can be a GitHub (or any remote) URL or a local path to another
/// repo. `dest` must not already exist (or must be empty); `git` creates it.
pub fn clone_repo(source: &str, dest: &str) -> Result<()> {
    process::run("git", None, &["clone", source, dest])
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

    /// A template with no `{task}` at all is already just a prefix.
    #[test]
    fn branch_prefix_of_a_placeholderless_template_is_the_whole_thing() {
        assert_eq!(branch_prefix("wip/"), "wip/");
    }

    #[test]
    fn branch_name_appends_the_slug_to_the_prefix() {
        assert_eq!(branch_name("feat/", "Fix Login Bug"), "feat/fix-login-bug");
    }

    #[test]
    fn branch_name_respects_a_custom_prefix() {
        assert_eq!(branch_name("fix/", "Login Bug"), "fix/login-bug");
    }

    #[test]
    fn branch_name_with_an_empty_prefix_is_the_bare_slug() {
        assert_eq!(branch_name("", "Login Bug"), "login-bug");
    }
}
