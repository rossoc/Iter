use crate::error::{IterError, Result};
use crate::models::TaskStatus;
use crate::process::{Runner, SYSTEM};
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

/// The GitHub side of the CLI, behind a trait so it can be faked. Every
/// method runs against the repository at `repo_path`, whose git remote tells
/// `gh` which GitHub repo is meant -- `iter` never stores or parses an
/// "owner/repo" string itself.
pub trait IssueTracker {
    /// Fetches one issue.
    fn fetch(&self, repo_path: &str, number: i64) -> Result<Issue>;

    /// Every issue in the repo, open and closed alike -- pulling only the
    /// open ones would leave a task no way to learn its issue was closed.
    fn list(&self, repo_path: &str) -> Result<Vec<Issue>>;

    /// Opens a new issue and returns its number. A new issue is always
    /// open, whatever the task that asked for it is at -- the caller closes
    /// it after if it has to.
    ///
    /// `project` is the title of a GitHub Project to file the issue under,
    /// or empty for none. It's handed to `gh` unchecked: `gh` resolves the
    /// title itself and refuses to create the issue if there's no such
    /// board, so a name that doesn't resolve costs a failed call rather
    /// than an issue filed nowhere.
    fn create(&self, repo_path: &str, title: &str, body: &str, project: &str) -> Result<i64>;

    /// Replaces an issue's body. Only ever called for `--body`, which is
    /// what makes overwriting what's on GitHub something you asked for
    /// rather than something a status sync did on its own.
    fn set_body(&self, repo_path: &str, number: i64, body: &str) -> Result<()>;

    /// Adds the authenticated user to an issue's assignees, leaving whoever
    /// is already there in place. Signing an issue "done at least by me",
    /// not claiming it alone.
    fn assign_self(&self, repo_path: &str, number: i64) -> Result<()>;

    /// Closes or reopens an issue, to match the status of its task.
    fn set_state(&self, repo_path: &str, number: i64, state: IssueState) -> Result<()>;

    /// Posts `body` as a new comment on an issue.
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
    /// The real `gh`.
    pub fn system() -> Self {
        GhCli { runner: &SYSTEM }
    }
}

impl<'a> GhCli<'a> {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(runner: &'a dyn Runner) -> Self {
        GhCli { runner }
    }

    /// Runs `gh issue <verb> <number> [rest]` in `repo_path`. Every write is
    /// that shape, so the number is stringified and the argument list
    /// assembled in one place rather than at each of them.
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

    /// `gh issue list` excludes pull requests of its own accord.
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
        // `gh` lists newest first, so a repo past the cap loses its *oldest*
        // issues -- and a task linked to one of those reads as "issue #N
        // isn't in this repo", which is what a deleted issue looks like too.
        // Better to say the list was cut short than to let the two be the
        // same thing.
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

    /// `--add-assignee`, not `--assignee`.
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

/// Tracker fakes for tests elsewhere.
#[cfg(test)]
pub mod fake {
    use super::*;

    /// A tracker with nothing in it: reads find no closed issue, writes
    /// succeed and are forgotten.
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

/// `gh --json` output, parsed -- `what` names the `gh` subcommand whose
/// output didn't parse, since that's all a reader can act on.
fn parse_json<T: DeserializeOwned>(what: &str, json: &str) -> Result<T> {
    serde_json::from_str(json)
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

    /// Compact JSON, uppercase states, CRLF and escapes in a body -- what
    /// `gh issue list --json` actually prints.
    const GH_LIST_OUTPUT: &str = concat!(
        r#"[{"body":"one\r\ntwo \"quoted\"\r\n","number":42,"state":"OPEN","title":"Fix: login"},"#,
        r#"{"body":"","number":7,"state":"CLOSED","title":"shipped"}]"#,
        "\n"
    );

    /// `gh`'s compact JSON, including the escapes in a body, reads as issues.
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
        for status in TaskStatus::ALL {
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
