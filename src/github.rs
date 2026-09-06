use crate::error::Result;
use crate::process;

pub struct IssueInfo {
    pub title: String,
    pub body: String,
}

/// Fetches an issue's title/body via the `gh` CLI, run from `repo_path` so
/// `gh` infers the repo from its git remote -- `iter` never stores or
/// parses an "owner/repo" string itself.
pub fn fetch_issue(repo_path: &str, number: i64) -> Result<IssueInfo> {
    Ok(IssueInfo {
        title: issue_field(repo_path, number, ".title")?,
        body: issue_field(repo_path, number, ".body")?,
    })
}

/// Posts `body` as a new comment on an issue, via `gh`, from `repo_path`.
pub fn post_comment(repo_path: &str, number: i64, body: &str) -> Result<()> {
    process::run(
        "gh",
        Some(repo_path),
        &["issue", "comment", &number.to_string(), "--body", body],
    )
}

/// One `jq`-selected field of an issue, with the trailing newline `gh` adds
/// stripped -- but no leading whitespace touched, since an issue body's own
/// indentation is part of the markdown.
fn issue_field(repo_path: &str, number: i64, jq_query: &str) -> Result<String> {
    let value = process::output(
        "gh",
        Some(repo_path),
        &[
            "issue",
            "view",
            &number.to_string(),
            "--json",
            "title,body",
            "-q",
            jq_query,
        ],
    )?;
    Ok(value.trim_end().to_string())
}
