use crate::error::{IterError, Result};
use crate::models::TaskStatus;
use crate::process::{Runner, SYSTEM};
use serde::Deserialize;
use serde::de::DeserializeOwned;

/// Whether an issue is open or closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum IssueState {
    #[serde(alias = "open")]
    Open,
    #[serde(alias = "closed")]
    Closed,
}

impl IssueState {
    /// Only `done` closes an issue; `queue` and `wip` are both open work.
    pub fn for_status(status: TaskStatus) -> Self {
        match status {
            TaskStatus::Done => IssueState::Closed,
            TaskStatus::Queue | TaskStatus::Wip => IssueState::Open,
        }
    }
}

/// The status a task takes when its issue is in `state`. Closing marks it done; reopening
/// sends `done` back to `queue` (what `iter session new` accepts). An open issue leaves
/// `queue`/`wip` alone, so pulling never flattens `wip` to `queue`.
pub fn status_for_issue_state(current: TaskStatus, state: IssueState) -> TaskStatus {
    match state {
        IssueState::Closed => TaskStatus::Done,
        IssueState::Open if current == TaskStatus::Done => TaskStatus::Queue,
        IssueState::Open => current,
    }
}

/// One issue, as much of it as a task mirrors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub number: i64,
    pub title: String,
    pub body: String,
    pub state: IssueState,
}

/// The fields every `gh --json` call asks for; one list so it matches [`RawIssue`].
const ISSUE_FIELDS: &str = "number,title,body,state";

/// The most issues `list_issues` fetches. `gh` defaults to 30, which would silently
/// truncate a real backlog; [`list_issues`] reports when the cap is hit.
const ISSUE_LIMIT: usize = 1000;

/// The GitHub side of the CLI, behind a trait so it can be faked. Methods run in `repo_path`,
/// whose git remote tells `gh` which repo is meant.
pub trait IssueTracker {
    fn fetch(&self, repo_path: &str, number: i64) -> Result<Issue>;

    /// Open and closed alike: pulling only open ones could never learn a task's issue closed.
    fn list(&self, repo_path: &str) -> Result<Vec<Issue>>;

    /// Opens an issue and returns its number. Always created open; the caller closes it.
    /// `project` is a GitHub Project title (or empty), passed to `gh` unchecked: `gh` fails
    /// the call if no such board exists, rather than filing the issue nowhere.
    fn create(&self, repo_path: &str, title: &str, body: &str, project: &str) -> Result<i64>;

    /// Only called for `--body`, so overwriting GitHub is always asked for.
    fn set_body(&self, repo_path: &str, number: i64, body: &str) -> Result<()>;

    /// Adds the authenticated user, keeping existing assignees.
    fn assign_self(&self, repo_path: &str, number: i64) -> Result<()>;

    fn set_state(&self, repo_path: &str, number: i64, state: IssueState) -> Result<()>;

    fn comment(&self, repo_path: &str, number: i64, body: &str) -> Result<()>;

    fn is_closed(&self, repo_path: &str, number: i64) -> Result<bool> {
        Ok(self.fetch(repo_path, number)?.state == IssueState::Closed)
    }

    fn close(&self, repo_path: &str, number: i64) -> Result<()> {
        self.set_state(repo_path, number, IssueState::Closed)
    }
}

/// The `gh` CLI, driven through a [`Runner`].
pub struct GhCli<'a> {
    runner: &'a dyn Runner,
}

impl GhCli<'static> {
    pub fn system() -> Self {
        GhCli { runner: &SYSTEM }
    }
}

impl<'a> GhCli<'a> {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(runner: &'a dyn Runner) -> Self {
        GhCli { runner }
    }

    /// Runs `gh issue <verb> <number> [rest]` in `repo_path`; every write has this shape.
    fn issue(&self, repo_path: &str, verb: &str, number: i64, rest: &[&str]) -> Result<()> {
        let number = number.to_string();
        let mut args = vec!["issue", verb, &number];
        args.extend_from_slice(rest);
        self.runner.run("gh", Some(repo_path), &args)
    }
}

impl IssueTracker for GhCli<'_> {
    fn fetch(&self, repo_path: &str, number: i64) -> Result<Issue> {
        let json = self.runner.output(
            "gh",
            Some(repo_path),
            &["issue", "view", &number.to_string(), "--json", ISSUE_FIELDS],
        )?;
        Ok(parse_json::<RawIssue>("issue view", &json)?.into())
    }

    fn list(&self, repo_path: &str) -> Result<Vec<Issue>> {
        let limit = ISSUE_LIMIT.to_string();
        let json = self.runner.output(
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
        // `gh` lists newest first, so past the cap the *oldest* issues drop, and a task
        // linked to one would look like a deleted issue. Say the list was cut short.
        if raw.len() >= ISSUE_LIMIT {
            eprintln!(
                "warning: only the {ISSUE_LIMIT} most recent issues were read -- \
                 anything older is invisible to this run"
            );
        }
        Ok(raw.into_iter().map(Issue::from).collect())
    }

    fn create(&self, repo_path: &str, title: &str, body: &str, project: &str) -> Result<i64> {
        let mut args = vec!["issue", "create", "--title", title, "--body", body];
        if !project.is_empty() {
            args.extend(["--project", project]);
        }
        let output = self.runner.output("gh", Some(repo_path), &args)?;
        issue_number_from_url(&output).ok_or_else(|| {
            IterError::CommandFailed(format!(
                "gh issue create printed no issue URL: {}",
                output.trim()
            ))
        })
    }

    fn set_body(&self, repo_path: &str, number: i64, body: &str) -> Result<()> {
        self.issue(repo_path, "edit", number, &["--body", body])
    }

    fn assign_self(&self, repo_path: &str, number: i64) -> Result<()> {
        self.issue(repo_path, "edit", number, &["--add-assignee", "@me"])
    }

    fn set_state(&self, repo_path: &str, number: i64, state: IssueState) -> Result<()> {
        let verb = match state {
            IssueState::Open => "reopen",
            IssueState::Closed => "close",
        };
        self.issue(repo_path, verb, number, &[])
    }

    fn comment(&self, repo_path: &str, number: i64, body: &str) -> Result<()> {
        self.issue(repo_path, "comment", number, &["--body", body])
    }
}

#[cfg(test)]
pub mod fake {
    use super::*;

    /// A tracker with nothing in it: writes succeed and are forgotten.
    pub struct Inert;

    impl IssueTracker for Inert {
        fn fetch(&self, _: &str, number: i64) -> Result<Issue> {
            Ok(Issue {
                number,
                title: String::new(),
                body: String::new(),
                state: IssueState::Open,
            })
        }
        fn list(&self, _: &str) -> Result<Vec<Issue>> {
            Ok(Vec::new())
        }
        fn create(&self, _: &str, _: &str, _: &str, _: &str) -> Result<i64> {
            Ok(1)
        }
        fn set_body(&self, _: &str, _: i64, _: &str) -> Result<()> {
            Ok(())
        }
        fn assign_self(&self, _: &str, _: i64) -> Result<()> {
            Ok(())
        }
        fn set_state(&self, _: &str, _: i64, _: IssueState) -> Result<()> {
            Ok(())
        }
        fn comment(&self, _: &str, _: i64, _: &str) -> Result<()> {
            Ok(())
        }
    }
}

/// An issue as `gh --json` hands it over, before normalisation, which happens on this one path.
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

/// An issue body as a task description: `\r`s and trailing blank lines come off; leading
/// whitespace stays, since indentation is markdown.
fn normalize_body(body: &str) -> String {
    body.replace("\r\n", "\n").trim_end().to_string()
}

/// `gh --json` output, parsed; `what` names the subcommand whose output failed.
fn parse_json<T: DeserializeOwned>(what: &str, json: &str) -> Result<T> {
    serde_json::from_str(json)
        .map_err(|e| IterError::CommandFailed(format!("could not read `gh {what}` output: {e}")))
}

/// The issue number out of the URL `gh issue create` prints (`.../issues/42` -> `42`). Read
/// from the last line back: `gh` prints progress chatter above the URL.
fn issue_number_from_url(output: &str) -> Option<i64> {
    output.lines().rev().find_map(|line| {
        let (_, number) = line.trim_end().rsplit_once("/issues/")?;
        number.parse().ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::process::fake::FakeRunner;

    const REPO: &str = "/repo";

    #[test]
    fn closing_an_issue_runs_gh_issue_close_in_the_repo() {
        let runner = FakeRunner::default();
        GhCli::new(&runner).close(REPO, 7).unwrap();
        assert_eq!(runner.calls(), ["[/repo] gh issue close 7"]);
    }

    #[test]
    fn reopening_runs_gh_issue_reopen() {
        let runner = FakeRunner::default();
        GhCli::new(&runner)
            .set_state(REPO, 7, IssueState::Open)
            .unwrap();
        assert_eq!(runner.calls(), ["[/repo] gh issue reopen 7"]);
    }

    #[test]
    fn a_comment_is_posted_with_its_body_as_one_argument() {
        let runner = FakeRunner::default();
        GhCli::new(&runner).comment(REPO, 3, "two words").unwrap();
        assert_eq!(
            runner.calls(),
            ["[/repo] gh issue comment 3 --body two words"]
        );
    }

    #[test]
    fn a_failing_gh_is_an_error() {
        let runner = FakeRunner::default();
        runner.fail("gh issue close");
        let error = GhCli::new(&runner).close(REPO, 7).unwrap_err();
        assert!(
            error.to_string().contains("gh issue close 7 failed"),
            "{error}"
        );
    }

    #[test]
    fn is_closed_reads_the_state_from_gh_issue_view() {
        let runner = FakeRunner::default();
        runner.reply(
            "gh issue view 7",
            r#"{"body":"","number":7,"state":"CLOSED","title":"t"}"#,
        );
        assert!(GhCli::new(&runner).is_closed(REPO, 7).unwrap());
        assert_eq!(
            runner.calls(),
            ["[/repo] gh issue view 7 --json number,title,body,state"]
        );
    }

    #[test]
    fn creating_an_issue_files_it_under_the_project_when_given() {
        let runner = FakeRunner::default();
        runner.reply("gh issue create", "https://github.com/o/r/issues/12\n");
        let n = GhCli::new(&runner).create(REPO, "T", "B", "Board").unwrap();
        assert_eq!(n, 12);
        assert_eq!(
            runner.calls(),
            ["[/repo] gh issue create --title T --body B --project Board"]
        );
    }

    #[test]
    fn signing_adds_me_as_an_assignee() {
        let runner = FakeRunner::default();
        GhCli::new(&runner).assign_self(REPO, 5).unwrap();
        assert_eq!(
            runner.calls(),
            ["[/repo] gh issue edit 5 --add-assignee @me"]
        );
    }

    const GH_LIST_OUTPUT: &str = concat!(
        r#"[{"body":"one\r\ntwo \"quoted\"\r\n","number":42,"state":"OPEN","title":"Fix: login"},"#,
        r#"{"body":"","number":7,"state":"CLOSED","title":"shipped"}]"#,
        "\n"
    );

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

    /// `gh issue view` yields one object, not an array; the same type must take both.
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
        for status in TaskStatus::ALL {
            assert_eq!(
                status_for_issue_state(status, IssueState::Closed),
                TaskStatus::Done
            );
        }
    }

    #[test]
    fn reopening_an_issue_puts_its_done_task_back_in_the_queue() {
        assert_eq!(
            status_for_issue_state(TaskStatus::Done, IssueState::Open),
            TaskStatus::Queue
        );
    }

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
