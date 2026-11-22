use crate::error::Result;
use crate::process::{Git, Tool};
use std::path::Path;

/// Whether `path` has a `.git` entry (a directory in a repo, a file in a worktree checkout).
pub fn is_git_repo(path: &str) -> bool {
    Path::new(path).join(".git").exists()
}

/// The fixed part of a `branch_template` (`"feat/"` out of `"feat/{task}"`); a template without
/// `{task}` is returned whole.
pub fn branch_prefix(template: &str) -> String {
    match template.split_once("{task}") {
        Some((prefix, _)) => prefix.to_string(),
        None => template.to_string(),
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

/// Whether every commit of `branch` is already in what `base_path` has checked out (merged, or
/// never committed to). A failing probe says no, so nothing is deleted on a doubt.
pub fn is_merged(base_path: &str, branch: &str) -> bool {
    refuse_option(branch).is_ok()
        && crate::process::succeeds(
            Git::BIN,
            Some(base_path),
            &["merge-base", "--is-ancestor", branch, "HEAD"],
        )
}

/// Deletes `branch` when [`is_merged`]; otherwise keeps it and says so, as `Ok(false)`: its commits
/// would survive only in the reflog. `-D` once checked, because `-d` also asks about an upstream.
/// Must run after the branch's worktree is removed: git refuses to delete a checked-out branch.
pub fn delete_branch(base_path: &str, branch: &str) -> Result<bool> {
    if !is_merged(base_path, branch) {
        return Ok(false);
    }
    Git(Some(base_path)).run(&["branch", "-D", "--", branch])?;
    Ok(true)
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

    /// A throwaway repo with one commit on `main`; `None` when git is not installed.
    fn repo(name: &str) -> Option<std::path::PathBuf> {
        let dir = crate::fixtures::scratch(name);
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args([
                    "-c",
                    "user.name=t",
                    "-c",
                    "user.email=t@t",
                    "-c",
                    "commit.gpgsign=false",
                ])
                .args(args)
                .current_dir(&dir)
                .output()
                .is_ok_and(|o| o.status.success())
        };
        (git(&["init", "-q", "-b", "main"]) && git(&["commit", "-q", "--allow-empty", "-m", "a"]))
            .then_some(dir)
    }

    fn git_in(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?}");
    }

    #[test]
    fn a_branch_with_unmerged_commits_is_kept() {
        let Some(dir) = repo("git-unmerged") else {
            return;
        };
        let base = dir.to_str().unwrap();
        git_in(&dir, &["checkout", "-q", "-b", "feat/x"]);
        git_in(&dir, &["commit", "-q", "--allow-empty", "-m", "work"]);
        git_in(&dir, &["checkout", "-q", "main"]);

        assert!(!delete_branch(base, "feat/x").unwrap());
        assert!(succeeds("feat/x", base), "the branch is still there");

        git_in(&dir, &["merge", "-q", "feat/x"]);
        assert!(delete_branch(base, "feat/x").unwrap());
        assert!(!succeeds("feat/x", base));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_branch_with_no_commits_of_its_own_is_deleted() {
        let Some(dir) = repo("git-fresh") else { return };
        let base = dir.to_str().unwrap();
        git_in(&dir, &["branch", "feat/y"]);
        assert!(delete_branch(base, "feat/y").unwrap());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn succeeds(branch: &str, base: &str) -> bool {
        crate::process::succeeds(
            "git",
            Some(base),
            &["rev-parse", "--verify", "--quiet", branch],
        )
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
}
