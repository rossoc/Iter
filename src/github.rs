use crate::error::{IterError, Result};
use std::process::Command;

pub struct IssueInfo {
    pub title: String,
    pub body: String,
}

/// Fetches an issue's title/body via the `gh` CLI, run from `repo_path` so
/// `gh` infers the repo from its git remote -- `iter` never stores or
/// parses an "owner/repo" string itself.
pub fn fetch_issue(repo_path: &str, number: i64) -> Result<IssueInfo> {
    Ok(IssueInfo {
        title: run_gh_query(repo_path, number, ".title")?,
        body: run_gh_query(repo_path, number, ".body")?,
    })
}

/// Posts `body` as a new comment on an issue, via `gh`, from `repo_path`.
pub fn post_comment(repo_path: &str, number: i64, body: &str) -> Result<()> {
    let status = Command::new("gh")
        .args(["issue", "comment", &number.to_string(), "--body", body])
        .current_dir(repo_path)
        .status()
        .map_err(|source| IterError::Spawn { tool: "gh", source })?;
    if status.success() {
        Ok(())
    } else {
        Err(IterError::CommandFailed(format!(
            "gh issue comment {number} failed"
        )))
    }
}

fn run_gh_query(repo_path: &str, number: i64, jq_query: &str) -> Result<String> {
    let output = Command::new("gh")
        .args([
            "issue",
            "view",
            &number.to_string(),
            "--json",
            "title,body",
            "-q",
            jq_query,
        ])
        .current_dir(repo_path)
        .output()
        .map_err(|source| IterError::Spawn { tool: "gh", source })?;
    if !output.status.success() {
        return Err(IterError::CommandFailed(format!(
            "gh issue view {number} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string())
}
