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

/// Fills a project's `branch_template` (e.g. `"feat/{task}"`) in with a
/// slugified task name.
pub fn branch_name(template: &str, task_name: &str) -> String {
    template.replace("{task}", &slugify(task_name))
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
    fn branch_name_substitutes_task_placeholder() {
        assert_eq!(
            branch_name("feat/{task}", "Fix Login Bug"),
            "feat/fix-login-bug"
        );
    }

    #[test]
    fn branch_name_respects_a_custom_template() {
        assert_eq!(branch_name("fix/{task}", "Login Bug"), "fix/login-bug");
    }
}
