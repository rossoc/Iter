use crate::error::{IterError, Result};
use crate::models::TaskStatus;
use crate::process;
use serde::Deserialize;
use serde::de::DeserializeOwned;

/// Whether an issue is open or closed -- the whole of an issue's state, as
/// far as a task is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum IssueState {
    #[serde(alias = "open")]
    Open,
    #[serde(alias = "closed")]
    Closed,
}

impl IssueState {
    /// The state an issue has to be in for a task at `status`: only `done`
    /// closes one -- `queue` and `wip` are both still-open work.
    pub fn for_status(status: TaskStatus) -> Self {
        match status {
            TaskStatus::Done => IssueState::Closed,
            TaskStatus::Queue | TaskStatus::Wip => IssueState::Open,
        }
    }
}

/// The status a task takes when its issue is in `state`, given the status
/// it has now.
///
/// Closing an issue marks its task done. Reopening one sends a `done` task
/// back to `queue` -- the status `iter session new` accepts -- so an issue
/// reopened on GitHub can be worked again here. An open issue on a
/// `queue`/`wip` task changes nothing: both mean the same thing to GitHub,
/// and the local one is the more precise of the two, so pulling must not
/// flatten a `wip` task back to `queue`.
pub fn status_for_issue_state(current: TaskStatus, state: IssueState) -> TaskStatus {
    match state {
        IssueState::Closed => TaskStatus::Done,
        IssueState::Open if current == TaskStatus::Done => TaskStatus::Queue,
        IssueState::Open => current,
    }
}

/// One issue, in as much of it as a task mirrors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub number: i64,
    pub title: String,
    pub body: String,
    pub state: IssueState,
}

/// The fields every `gh --json` call here asks for -- one list, so what's
/// requested and what [`RawIssue`] expects to find can't drift apart.
const ISSUE_FIELDS: &str = "number,title,body,state";

/// The most issues `list_issues` brings back in one go. `gh` defaults to 30,
/// which would silently truncate a real backlog; this is high enough that
/// the cap isn't the thing you hit first -- and [`list_issues`] says so
/// when it is.
const ISSUE_LIMIT: usize = 1000;

/// Fetches an issue via the `gh` CLI, run from `repo_path` so `gh` infers
/// the repo from its git remote -- `iter` never stores or parses an
/// "owner/repo" string itself.
pub fn fetch_issue(repo_path: &str, number: i64) -> Result<Issue> {
    let json = process::output(
        "gh",
        Some(repo_path),
        &["issue", "view", &number.to_string(), "--json", ISSUE_FIELDS],
    )?;
    Ok(parse_json::<RawIssue>("issue view", &json)?.into())
}

/// Every issue in the repo at `repo_path`, open and closed alike -- pulling
/// only the open ones would leave a task no way to learn its issue was
/// closed. `gh issue list` excludes pull requests of its own accord.
pub fn list_issues(repo_path: &str) -> Result<Vec<Issue>> {
    let limit = ISSUE_LIMIT.to_string();
    let json = process::output(
        "gh",
        Some(repo_path),
        &[
            "issue",
            "list",
            "--state",
            "all",
            "--limit",
            &limit,
            "--json",
            ISSUE_FIELDS,
        ],
    )?;
    let raw: Vec<RawIssue> = parse_json("issue list", &json)?;
    // `gh` lists newest first, so a repo past the cap loses its *oldest*
    // issues -- and a task linked to one of those reads as "issue #N isn't
    // in this repo", which is what a deleted issue looks like too. Better
    // to say the list was cut short than to let the two be the same thing.
    if raw.len() >= ISSUE_LIMIT {
        eprintln!(
            "warning: only the {ISSUE_LIMIT} most recent issues were read -- \
             anything older is invisible to this run"
        );
    }
    Ok(raw.into_iter().map(Issue::from).collect())
}

/// Opens a new issue and returns its number. A new issue is always open,
/// whatever the task that asked for it is at -- the caller closes it after
/// if it has to.
///
/// `project` is the title of a GitHub Project to file the issue under, or
/// empty for none. It's handed to `gh` unchecked: `gh` resolves the title
/// itself and refuses to create the issue if there's no such board, so a
/// name that doesn't resolve costs a failed call rather than an issue filed
/// nowhere. (Resolving one at all needs a token with the Projects
/// permission -- `gh auth refresh -s project` on a classic login.)
pub fn create_issue(repo_path: &str, title: &str, body: &str, project: &str) -> Result<i64> {
    let mut args = vec!["issue", "create", "--title", title, "--body", body];
    if !project.is_empty() {
        args.extend(["--project", project]);
    }
    let output = process::output("gh", Some(repo_path), &args)?;
    issue_number_from_url(&output).ok_or_else(|| {
        IterError::CommandFailed(format!(
            "gh issue create printed no issue URL: {}",
            output.trim()
        ))
    })
}

/// Replaces an issue's body with `body`. Only ever called for `--body`,
/// which is what makes overwriting what's on GitHub something you asked
/// for rather than something a status sync did on its own.
pub fn set_issue_body(repo_path: &str, number: i64, body: &str) -> Result<()> {
    process::run(
        "gh",
        Some(repo_path),
        &["issue", "edit", &number.to_string(), "--body", body],
    )
}

/// Adds the authenticated user to an issue's assignees, leaving whoever is
/// already there in place -- `--add-assignee`, not `--assignee`. Signing an
/// issue "done at least by me", not claiming it alone.
pub fn assign_self(repo_path: &str, number: i64) -> Result<()> {
    process::run(
        "gh",
        Some(repo_path),
        &[
            "issue",
            "edit",
            &number.to_string(),
            "--add-assignee",
            "@me",
        ],
    )
}

/// Closes or reopens an issue, to match the status of the task tracking it.
pub fn set_issue_state(repo_path: &str, number: i64, state: IssueState) -> Result<()> {
    let verb = match state {
        IssueState::Open => "reopen",
        IssueState::Closed => "close",
    };
    process::run("gh", Some(repo_path), &["issue", verb, &number.to_string()])
}

/// Posts `body` as a new comment on an issue, via `gh`, from `repo_path`.
pub fn post_comment(repo_path: &str, number: i64, body: &str) -> Result<()> {
    process::run(
        "gh",
        Some(repo_path),
        &["issue", "comment", &number.to_string(), "--body", body],
    )
}

/// An issue exactly as `gh --json` hands it over, before the tidying
/// [`Issue`] gets. Separate from `Issue` so the normalisation below happens
/// on the one path everything comes in through, rather than at each use.
#[derive(Debug, Deserialize)]
struct RawIssue {
    number: i64,
    title: String,
    body: String,
    state: IssueState,
}

impl From<RawIssue> for Issue {
    fn from(raw: RawIssue) -> Self {
        Issue {
            number: raw.number,
            title: raw.title.trim().to_string(),
            body: normalize_body(&raw.body),
            state: raw.state,
        }
    }
}

/// An issue body as a task description: GitHub keeps bodies with CRLF line
/// endings, and this one is headed for a Unix editor buffer, a markdown
/// report and a SQLite column, so the `\r`s come off. Trailing blank lines
/// go too -- the editor template ends the body with its own newline -- but
/// leading whitespace isn't touched, since an issue body's own indentation
/// is part of the markdown.
fn normalize_body(body: &str) -> String {
    body.replace("\r\n", "\n").trim_end().to_string()
}

/// `gh --json` output, parsed. JSON is a subset of YAML, so the parser
/// already in the tree reads it as-is -- `what` names the `gh` subcommand
/// whose output didn't parse, since that's all a reader can act on.
fn parse_json<T: DeserializeOwned>(what: &str, json: &str) -> Result<T> {
    serde_yaml::from_str(json)
        .map_err(|e| IterError::CommandFailed(format!("could not read `gh {what}` output: {e}")))
}

/// The issue number out of the URL `gh issue create` prints, e.g. `42` out
/// of `https://github.com/owner/repo/issues/42`. Read from the last line
/// back, because `gh` prints progress chatter ("Creating issue in
/// owner/repo") above the URL.
fn issue_number_from_url(output: &str) -> Option<i64> {
    output.lines().rev().find_map(|line| {
        let (_, number) = line.trim_end().rsplit_once("/issues/")?;
        number.parse().ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compact JSON, uppercase states, CRLF and escapes in a body -- what
    /// `gh issue list --json` actually prints.
    const GH_LIST_OUTPUT: &str = concat!(
        r#"[{"body":"one\r\ntwo \"quoted\"\r\n","number":42,"state":"OPEN","title":"Fix: login"},"#,
        r#"{"body":"","number":7,"state":"CLOSED","title":"shipped"}]"#,
        "\n"
    );

    /// The load-bearing assumption behind [`parse_json`]: the YAML parser
    /// reads `gh`'s compact JSON, so no second parser is needed for it.
    #[test]
    fn gh_json_parses_into_issues() {
        let issues: Vec<RawIssue> = parse_json("issue list", GH_LIST_OUTPUT).expect("parses");
        let issues: Vec<Issue> = issues.into_iter().map(Issue::from).collect();
        assert_eq!(
            issues,
            vec![
                Issue {
                    number: 42,
                    title: "Fix: login".to_string(),
                    body: "one\ntwo \"quoted\"".to_string(),
                    state: IssueState::Open,
                },
                Issue {
                    number: 7,
                    title: "shipped".to_string(),
                    body: String::new(),
                    state: IssueState::Closed,
                },
            ]
        );
    }

    /// `gh issue view` hands over one object rather than an array -- the
    /// same fields, so the same target type has to take both shapes.
    #[test]
    fn a_single_issue_object_parses_too() {
        let raw: RawIssue = parse_json(
            "issue view",
            r#"{"body":"b","number":3,"state":"OPEN","title":"t"}"#,
        )
        .expect("parses");
        assert_eq!(Issue::from(raw).number, 3);
    }

    #[test]
    fn output_that_isnt_json_is_a_command_failure_naming_the_command() {
        let error = parse_json::<RawIssue>("issue view", "not json at all: [")
            .expect_err("garbage doesn't parse");
        assert!(
            error.to_string().contains("gh issue view"),
            "unhelpful message: {error}"
        );
    }

    /// A body's own indentation is markdown; only the line endings and the
    /// trailing blank space are ours to normalise.
    #[test]
    fn a_body_keeps_its_leading_indentation() {
        assert_eq!(normalize_body("    indented\r\n\r\n"), "    indented");
    }

    #[test]
    fn the_created_issue_number_comes_off_the_printed_url() {
        assert_eq!(
            issue_number_from_url("https://github.com/owner/repo/issues/42\n"),
            Some(42)
        );
    }

    /// `gh` prints progress lines above the URL, and the URL is the last
    /// thing it says.
    #[test]
    fn chatter_above_the_url_is_ignored() {
        let output = "\nCreating issue in owner/repo\n\nhttps://github.com/owner/repo/issues/7\n";
        assert_eq!(issue_number_from_url(output), Some(7));
    }

    #[test]
    fn output_with_no_issue_url_yields_no_number() {
        assert_eq!(
            issue_number_from_url("https://github.com/owner/repo\n"),
            None
        );
    }

    #[test]
    fn only_a_done_task_wants_its_issue_closed() {
        assert_eq!(IssueState::for_status(TaskStatus::Done), IssueState::Closed);
        assert_eq!(IssueState::for_status(TaskStatus::Queue), IssueState::Open);
        assert_eq!(IssueState::for_status(TaskStatus::Wip), IssueState::Open);
    }

    #[test]
    fn a_closed_issue_marks_its_task_done() {
        for status in [TaskStatus::Queue, TaskStatus::Wip, TaskStatus::Done] {
            assert_eq!(
                status_for_issue_state(status, IssueState::Closed),
                TaskStatus::Done
            );
        }
    }

    /// The reopened-upstream case: `done` goes back to `queue`, which is
    /// the status `iter session new` will take, so the task can be worked
    /// again.
    #[test]
    fn reopening_an_issue_puts_its_done_task_back_in_the_queue() {
        assert_eq!(
            status_for_issue_state(TaskStatus::Done, IssueState::Open),
            TaskStatus::Queue
        );
    }

    /// `queue` and `wip` are both "open" to GitHub, so an open issue must
    /// not knock a task that's underway back to the queue.
    #[test]
    fn an_open_issue_leaves_unfinished_work_where_it_is() {
        assert_eq!(
            status_for_issue_state(TaskStatus::Wip, IssueState::Open),
            TaskStatus::Wip
        );
        assert_eq!(
            status_for_issue_state(TaskStatus::Queue, IssueState::Open),
            TaskStatus::Queue
        );
    }
}
