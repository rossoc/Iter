use crate::error::Result;
use crate::process::{Git, Tool};
use std::path::Path;

/// Whether `path` looks like a git repo (has a `.git` entry -- a plain repo
/// has a `.git` directory, a worktree checkout has a `.git` file).
pub fn is_git_repo(path: &str) -> bool {
    Path::new(path).join(".git").exists()
}

/// Lowercases `name` and collapses every run of non `[a-z0-9]` characters
/// into a single `-`, trimming leading/trailing dashes -- used to turn a
/// free-text task name into something safe for a branch name / directory.
///
/// Private: [`task_slug`] is the way in, so nothing can end up with the
/// empty slug that the fallback there exists to rule out.
fn slugify(name: &str) -> String {
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

/// The slug a task's branch *and* its worktree directory are both named
/// from -- one slug for both, so a name made unique for one is unique for
/// the other too.
///
/// A name that slugifies to nothing (no ASCII letters or digits in it at
/// all -- a task pulled from a non-Latin issue title, say) falls back to
/// the task's id. The empty slug is not a cosmetic problem: it makes the
/// branch a bare prefix, which git refuses, and the worktree path the
/// `.iter-worktrees` directory itself.
pub fn task_slug(name: &str, task_id: i64) -> String {
    let slug = slugify(name);
    match slug.is_empty() {
        true => format!("task-{task_id}"),
        false => slug,
    }
}

/// Creates a new worktree at `worktree_path`, on a new branch `branch`,
/// checked out from `base_path`'s current `HEAD`.
pub fn create_worktree(base_path: &str, branch: &str, worktree_path: &str) -> Result<()> {
    Git(Some(base_path)).run(&["worktree", "add", "-b", branch, worktree_path])
}

/// Removes a worktree created by `create_worktree`. `--force` because the
/// task is done and we don't want a stray untracked file to block cleanup.
pub fn remove_worktree(base_path: &str, worktree_path: &str) -> Result<()> {
    Git(Some(base_path)).run(&["worktree", "remove", "--force", worktree_path])
}

/// Deletes a branch created by `create_worktree`. `-D` (not `-d`), like
/// `remove_worktree`'s `--force`: the task is done and we don't want
/// unmerged commits to block cleanup. Must run after the branch's worktree
/// (if any) has already been removed -- git refuses to delete a branch
/// that's still checked out in one.
pub fn delete_branch(base_path: &str, branch: &str) -> Result<()> {
    Git(Some(base_path)).run(&["branch", "-D", branch])
}

/// The branch checked out at `path` -- `None` if `HEAD` is detached, or if
/// `path` isn't a repo at all (the two are the same answer to the only
/// question anyone asks here: is *this* branch the one checked out).
pub fn current_branch(path: &str) -> Option<String> {
    Git(Some(path)).output_trimmed(&["symbolic-ref", "--quiet", "--short", "HEAD"])
}

/// Merges `branch` into whatever `dir` has checked out, letting git print
/// its own output, and reports whether it came out clean.
///
/// A `false` is not an error here, and nothing is rolled back on one: git
/// has left the merge exactly as far as it got -- the conflicted files, the
/// `MERGE_HEAD` beside them -- and that half-done state, which `git status`
/// in `dir` spells out, is the point. The caller's job is to stop and say
/// where, not to tidy it away.
pub fn merge(dir: &str, branch: &str) -> Result<bool> {
    Git(Some(dir)).run_status(&["merge", branch])
}

/// Clones `source` into `dest`, exactly like `git clone <source> <dest>` --
/// `source` can be a GitHub (or any remote) URL or a local path to another
/// repo. `dest` must not already exist (or must be empty); `git` creates it.
pub fn clone_repo(source: &str, dest: &str) -> Result<()> {
    Git(None).run(&["clone", source, dest])
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

    /// Without the fallback this is the empty string, which makes `feat/`
    /// (git rejects it) and a worktree path that is the container
    /// directory -- both of them silent until `git worktree add` fails.
    #[test]
    fn a_name_with_nothing_sluggable_in_it_falls_back_to_the_task_id() {
        assert_eq!(task_slug("設計を見直す", 42), "task-42");
        assert_eq!(task_slug("!!! ---", 7), "task-7");
        assert_eq!(task_slug("", 3), "task-3");
    }
}
