//! Pushing an organization's worklog to its time platform, and on to GitHub, in three steps shared by the CLI and the web page: [`plan`] (what would be sent; nothing leaves the machine),
//! [`check`] (ledger and platform conflicts) and [`execute`] (platform first; only if it accepts, GitHub for completed tasks, then the ledger). See `doc/architecture_agent_wiki.md`.

use crate::config::config;
use crate::db::{Db, Table};
use crate::error::{IterError, Result};
use crate::github::IssueTracker;
use crate::models::{Organization, Project, Push, Task, TaskStatus};
use crate::platform::{PlatformItem, TimePlatform};
use crate::reporting::{DateRange, Worklog, WorklogEntry, fmt_date, worklog_line};
use crate::utils::report::project_reports;
use chrono::{NaiveDate, NaiveDateTime};
use sha1::{Digest, Sha1};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub organization: String,
    pub activity: String,
}

impl Target {
    pub fn of(org: &Organization) -> Self {
        Target {
            organization: match org.platform_project.trim() {
                "" => org.name.clone(),
                set => set.to_string(),
            },
            activity: org.platform_activity.trim().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueJob {
    pub task: String,
    pub repo_path: String,
    pub issue: i64,
    pub line: String,
    /// The day the task was last worked on; the issue is touched only if that day was accepted.
    pub day: String,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub range: DateRange,
    pub target: Target,
    pub worklog: Worklog,
    pub issues: Vec<IssueJob>,
    pub hash: String,
}

/// A fingerprint of what is sent: the same worklog gives the same one, a corrected one another.
pub fn fingerprint(worklog: &Worklog) -> Result<String> {
    let mut text = worklog.to_json()?;
    if worklog.open {
        text.push_str("#open");
    }
    let digest = Sha1::digest(text.as_bytes());
    Ok(digest.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    }))
}

pub fn plan(
    db: &Db,
    org: &Organization,
    range: DateRange,
    now: NaiveDateTime,
    target: &Target,
) -> Result<Plan> {
    let (projects, sessions) =
        project_reports(db, &db.projects_in::<Organization>(org.id())?, range, now)?;
    let worklog = Worklog::build(
        &projects,
        &sessions,
        now,
        config().pause_gap_minutes,
        &target.organization,
        &target.activity,
    );
    if worklog.entries.is_empty() {
        return Err(IterError::CommandFailed(format!(
            "nothing to push: no sessions in {}",
            range.label()
        )));
    }

    let last_day = |sessions: &[crate::reporting::SessionRow]| sessions.iter().map(|r| r.day).max();
    let mut issues = Vec::new();
    for task in projects.iter().flat_map(|p| &p.tasks) {
        // "Completed" in a worklog: done, and worked on in the period.
        let Some(day) = last_day(&task.sessions).filter(|_| task.status == TaskStatus::Done) else {
            continue;
        };
        let Some(row) = db.get::<Task>(task.id)? else {
            continue;
        };
        let Some(project) = db.get::<Project>(row.project_id)? else {
            continue;
        };
        let Some(issue) = row.github_issue.filter(|_| project.github) else {
            continue;
        };
        issues.push(IssueJob {
            task: format!("{}/{}", project.name, row.name),
            repo_path: project.base_path.clone(),
            issue,
            line: worklog_line(&task.name, true),
            day: fmt_date(day),
        });
    }
    let hash = fingerprint(&worklog)?;
    Ok(Plan {
        range,
        target: target.clone(),
        worklog,
        issues,
        hash,
    })
}

impl Plan {
    pub fn days(&self) -> (NaiveDate, NaiveDate) {
        let parse = |e: &WorklogEntry| NaiveDate::parse_from_str(&e.day, "%Y-%m-%d").ok();
        let first = self.worklog.entries.iter().find_map(parse);
        let last = self.worklog.entries.iter().rev().find_map(parse);
        (
            first.unwrap_or(self.range.to),
            last.unwrap_or(self.range.to),
        )
    }

    pub fn preview(&self) -> String {
        let mut out = format!(
            "Period: {}\nEntries: {} -- {:.2} h in total\n",
            self.range.label(),
            self.worklog.entries.len(),
            self.worklog.total_hours()
        );
        for e in &self.worklog.entries {
            let _ = writeln!(
                out,
                "\n{}  {:.2} h  {} / {}\n{}",
                e.day, e.time, e.project, e.activity, e.note
            );
        }
        if self.worklog.open {
            out.push_str("\nWarning: a session is still open; its time is counted up to now, so this figure is partial.\n");
        }
        for job in &self.issues {
            let _ = writeln!(
                out,
                "GitHub: close #{} ({}) and comment \"{}\"",
                job.issue, job.task, job.line
            );
        }
        out
    }
}

#[derive(Debug, Clone, Default)]
pub struct Check {
    pub already_pushed: bool,
    /// The platform's entries on days this push covers: a day has one entry, so these are conflicts to resolve there first.
    pub conflicts: Vec<WorklogEntry>,
}

impl Check {
    pub fn conflicts_on(&self, day: &str) -> bool {
        self.conflicts.iter().any(|e| e.day == day)
    }

    pub fn conflict_lines(&self) -> Vec<String> {
        self.conflicts.iter().map(conflict_line).collect()
    }

    pub fn already_pushed_line(&self) -> Option<&'static str> {
        self.already_pushed
            .then_some("This exact worklog was already pushed.")
    }
}

fn conflict_line(e: &WorklogEntry) -> String {
    format!(
        "{}: the platform already has an entry ({:.2} h, {}); not pushed until resolved",
        e.day,
        e.time,
        e.note.replace('\n', " ")
    )
}

pub fn check(
    db: &Db,
    org: &Organization,
    plan: &Plan,
    platform: &dyn TimePlatform,
) -> Result<Check> {
    let already_pushed = db
        .find_push(org.id(), &plan.hash)?
        .is_some_and(|row| row.complete());
    let (from, to) = plan.days();
    let conflicts = platform
        .fetch(from, to)?
        .into_iter()
        .filter(|e| plan.worklog.entries.iter().any(|mine| mine.day == e.day))
        .collect();
    Ok(Check {
        already_pushed,
        conflicts,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub project: String,
    pub activity: String,
}

/// The platform's project and activity that `target` names, by id or name. A platform without such lists resolves nothing (the text is sent as is);
/// a list that fails to load is an error, since a name sent where an id is expected files hours under the wrong project; named but unlisted is an error.
pub fn resolve(platform: &dyn TimePlatform, target: &Target) -> Result<Resolved> {
    let mut out = Resolved {
        project: target.organization.clone(),
        activity: target.activity.clone(),
    };
    let projects = platform.projects()?;
    if projects.is_empty() {
        return Ok(out);
    }
    let Some(project) = projects.iter().find(|p| p.is(&target.organization)) else {
        return Err(IterError::Platform(format!(
            "'{}' is not one of your projects",
            target.organization
        )));
    };
    out.project = project.id.clone();
    let activities = platform.activities(&project.id)?;
    if !activities.is_empty() {
        let Some(activity) = activities.iter().find(|a| a.is(&target.activity)) else {
            return Err(IterError::Platform(format!(
                "'{}' is not an activity of '{}'",
                target.activity, project.name
            )));
        };
        out.activity = activity.id.clone();
    }
    Ok(out)
}

pub fn unknown_targets(platform: &dyn TimePlatform, target: &Target) -> Vec<String> {
    match resolve(platform, target) {
        Ok(_) => Vec::new(),
        Err(e) => vec![
            e.to_string()
                .trim_start_matches("time platform: ")
                .to_string(),
        ],
    }
}

/// What the platform offers for the settings form: projects, and the activities of `project`; whatever cannot be fetched is an empty list (free text).
pub fn choices(
    platform: &dyn TimePlatform,
    project: &str,
) -> (Vec<PlatformItem>, Vec<PlatformItem>) {
    let projects = platform.projects().unwrap_or_default();
    let activities = match projects.iter().find(|p| p.is(project)) {
        Some(found) => platform.activities(&found.id).unwrap_or_default(),
        None => Vec::new(),
    };
    (projects, activities)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub what: String,
    pub error: Option<String>,
}

impl Item {
    fn ok(what: impl Into<String>) -> Self {
        Item {
            what: what.into(),
            error: None,
        }
    }
    fn failed(what: impl Into<String>, error: impl ToString) -> Self {
        Item {
            what: what.into(),
            error: Some(error.to_string()),
        }
    }
    pub fn line(&self) -> String {
        match &self.error {
            None => format!("ok: {}", self.what),
            Some(e) => format!("FAILED: {}: {e}", self.what),
        }
    }
}

/// Pushes the days of `plan` that `check` found free; days the platform holds are reported as failed items and the rest go through.
/// A request refused as a whole is the returned error, GitHub untouched, no ledger row. Otherwise each entry's verdict and each GitHub issue of a task completed on an accepted day is an item; the ledger row records them all unless the platform took nothing.
pub fn execute(
    db: &Db,
    org: &Organization,
    plan: &Plan,
    check: &Check,
    platform: &dyn TimePlatform,
    gh: &dyn IssueTracker,
    now: NaiveDateTime,
) -> Result<Vec<Item>> {
    // what an earlier incomplete run of this worklog already got through: those days are ours on the platform, and those issues need no second comment
    let before = db
        .find_push(org.id(), &plan.hash)?
        .filter(|row| !row.complete());
    let earlier: Vec<&str> = before.iter().flat_map(|row| row.ok_items()).collect();
    let pushed_earlier = |day: &str| {
        let prefix = format!("platform: {day} ");
        earlier.iter().any(|what| what.starts_with(&prefix))
    };

    let mut items = Vec::new();
    let mut done_days = Vec::new();
    for entry in &check.conflicts {
        match pushed_earlier(&entry.day) {
            true => {
                items.push(Item::ok(format!(
                    "platform: {} (pushed earlier)",
                    entry.day
                )));
                done_days.push(entry.day.clone());
            }
            false => items.push(Item::failed("platform", conflict_line(entry))),
        }
    }

    let mut free = Worklog {
        entries: plan
            .worklog
            .entries
            .iter()
            .filter(|e| !check.conflicts_on(&e.day))
            .cloned()
            .collect(),
        open: plan.worklog.open,
    };
    if !free.entries.is_empty() {
        // the platform takes ids; the plan holds what was chosen (a name or an id)
        let ids = resolve(platform, &plan.target)?;
        for entry in &mut free.entries {
            entry.project = ids.project.clone();
            entry.activity = ids.activity.clone();
        }
        for result in platform.push(&free)? {
            match result.accepted() {
                true => {
                    let hours = free
                        .entries
                        .iter()
                        .find(|e| e.day == result.day)
                        .map_or(0.0, |e| e.time);
                    items.push(Item::ok(format!("platform: {} {hours:.2} h", result.day)));
                    done_days.push(result.day);
                }
                false => items.push(Item::failed(
                    format!("platform: {}", result.day),
                    format!("{}: {}", result.status, result.message),
                )),
            }
        }
    }
    if done_days.is_empty() {
        return Ok(items);
    }

    for job in plan.issues.iter().filter(|j| done_days.contains(&j.day)) {
        let what = format!("github #{} ({})", job.issue, job.task);
        let prefix = format!("{what}: ");
        if earlier.iter().any(|w| w.starts_with(&prefix)) {
            items.push(Item::ok(format!("{what}: done earlier")));
            continue;
        }
        let step = || -> Result<String> {
            let closed = gh.is_closed(&job.repo_path, job.issue)?;
            if !closed {
                gh.close(&job.repo_path, job.issue)?;
            }
            gh.comment(&job.repo_path, job.issue, &job.line)?;
            Ok(match closed {
                true => "commented (already closed)".to_string(),
                false => "closed and commented".to_string(),
            })
        };
        items.push(match step() {
            Ok(done) => Item::ok(format!("{what}: {done}")),
            Err(e) => Item::failed(what, e),
        });
    }

    let (from, to) = plan.days();
    let recorded = db.insert(&Push {
        id: None,
        organization_id: org.id(),
        from_day: fmt_date(from),
        to_day: fmt_date(to),
        hash: plan.hash.clone(),
        at: now,
        results: items.iter().map(Item::line).collect::<Vec<_>>().join("\n"),
    });
    // The hours are on the platform by now: failing here would lose what
    // just happened, so it is reported with the rest.
    if let Err(e) = recorded {
        items.push(Item::failed("ledger", e));
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{self, dt};
    use crate::github::{Issue, IssueState};
    use crate::platform::fake::FakePlatform;
    use std::cell::RefCell;

    #[derive(Default)]
    struct FakeGh {
        calls: RefCell<Vec<String>>,
        closed: bool,
        fail_comment: bool,
    }

    impl IssueTracker for FakeGh {
        fn fetch(&self, _: &str, n: i64) -> Result<Issue> {
            self.calls.borrow_mut().push(format!("view #{n}"));
            Ok(Issue {
                number: n,
                title: String::new(),
                body: String::new(),
                state: match self.closed {
                    true => IssueState::Closed,
                    false => IssueState::Open,
                },
            })
        }
        fn set_state(&self, _: &str, n: i64, state: IssueState) -> Result<()> {
            assert_eq!(state, IssueState::Closed);
            self.calls.borrow_mut().push(format!("close #{n}"));
            Ok(())
        }
        fn comment(&self, _: &str, n: i64, body: &str) -> Result<()> {
            if self.fail_comment {
                return Err(IterError::CommandFailed("gh down".into()));
            }
            self.calls.borrow_mut().push(format!("comment #{n} {body}"));
            Ok(())
        }
        fn list(&self, _: &str) -> Result<Vec<Issue>> {
            unreachable!("a push never lists issues")
        }
        fn create(&self, _: &str, _: &str, _: &str, _: &str) -> Result<i64> {
            unreachable!("a push never opens issues")
        }
        fn set_body(&self, _: &str, _: i64, _: &str) -> Result<()> {
            unreachable!("a push never edits issue bodies")
        }
        fn assign_self(&self, _: &str, _: i64) -> Result<()> {
            unreachable!("a push never assigns")
        }
    }

    fn setup() -> (Db, Organization) {
        let db = fixtures::db();
        let mut org = Organization::template(&crate::models::Settings::default());
        org.name = "acme".into();
        org.platform_activity = "dev".into();
        let org_id = db.insert(&org).unwrap();
        let org = db.get::<Organization>(org_id).unwrap().unwrap();
        let mut p = fixtures::project("web");
        p.github = true;
        p.organization_id = Some(org_id);
        let pid = db.insert(&p).unwrap();
        for (name, status, issue) in [
            ("Ship", TaskStatus::Done, Some(7)),
            ("Fix", TaskStatus::Wip, Some(8)),
        ] {
            let mut t = Task::template(pid, String::new(), status);
            t.name = name.into();
            t.status = status;
            t.github_issue = issue;
            let tid = db.insert(&t).unwrap();
            let (start, end) = if name == "Ship" {
                ("2026-09-04 09:00", "2026-09-04 10:00")
            } else {
                ("2026-09-04 11:00", "2026-09-04 12:00")
            };
            fixtures::add_session(&db, tid, dt(start), Some(dt(end)));
        }
        (db, org)
    }

    fn range() -> DateRange {
        DateRange::day(NaiveDate::from_ymd_opt(2026, 9, 4).unwrap())
    }

    fn now() -> NaiveDateTime {
        dt("2026-09-04 18:00")
    }

    fn the_plan(db: &Db, org: &Organization) -> Plan {
        plan(db, org, range(), now(), &Target::of(org)).unwrap()
    }

    fn run(
        db: &Db,
        org: &Organization,
        plan: &Plan,
        platform: &FakePlatform,
        gh: &FakeGh,
    ) -> Result<Vec<Item>> {
        let c = check(db, org, plan, platform)?;
        execute(db, org, plan, &c, platform, gh, now())
    }

    fn a_task(db: &Db, org: &Organization) -> i64 {
        let project = db.projects_in::<Organization>(org.id()).unwrap()[0].id();
        db.tasks_for_project(project).unwrap()[0].id()
    }

    #[test]
    fn choices_are_the_platforms_lists_and_empty_without_a_project() {
        let platform = FakePlatform {
            projects: vec![PlatformItem::new("p1", "Acme")],
            activities: vec![PlatformItem::new("a1", "Dev")],
            ..Default::default()
        };
        let both = (
            vec![PlatformItem::new("p1", "Acme")],
            vec![PlatformItem::new("a1", "Dev")],
        );
        assert_eq!(choices(&platform, "acme"), both);
        assert_eq!(choices(&platform, "p1"), both);
        assert_eq!(choices(&platform, "").1, Vec::<PlatformItem>::new());
    }

    #[test]
    fn a_plan_has_one_entry_a_day_and_github_work_for_completed_tasks_only() {
        let (db, org) = setup();
        let plan = the_plan(&db, &org);
        assert_eq!(plan.worklog.entries.len(), 1);
        assert_eq!(plan.worklog.entries[0].project, "acme");
        assert_eq!(plan.worklog.entries[0].time, 2.0);
        assert_eq!(plan.issues.len(), 1);
        assert_eq!(plan.issues[0].issue, 7);
        assert_eq!(plan.issues[0].line, "Completed Ship");
        assert_eq!(plan.issues[0].day, "2026-09-04");
        assert!(plan.preview().contains("Period: 2026-09-04"));
    }

    #[test]
    fn an_empty_period_is_refused() {
        let (db, org) = setup();
        let day = DateRange::day(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert!(plan(&db, &org, day, now(), &Target::of(&org)).is_err());
    }

    #[test]
    fn a_push_closes_comments_and_records_the_ledger() {
        let (db, org) = setup();
        let (platform, gh) = (FakePlatform::default(), FakeGh::default());
        let plan = the_plan(&db, &org);
        let items = run(&db, &org, &plan, &platform, &gh).unwrap();
        assert!(items.iter().all(|i| i.error.is_none()), "{items:?}");
        assert_eq!(platform.pushed.borrow().len(), 1);
        assert_eq!(
            *gh.calls.borrow(),
            vec!["view #7", "close #7", "comment #7 Completed Ship"]
        );
        assert!(db.find_push(org.id(), &plan.hash).unwrap().is_some());
    }

    #[test]
    fn an_identical_second_push_is_flagged_and_a_changed_one_is_not() {
        let (db, org) = setup();
        let platform = FakePlatform::default();
        let first = the_plan(&db, &org);
        assert!(!check(&db, &org, &first, &platform).unwrap().already_pushed);
        run(&db, &org, &first, &platform, &FakeGh::default()).unwrap();

        let again = the_plan(&db, &org);
        assert!(check(&db, &org, &again, &platform).unwrap().already_pushed);

        // another session is worked: the figure differs, so it may be pushed
        let task = a_task(&db, &org);
        fixtures::add_session(
            &db,
            task,
            dt("2026-09-04 14:00"),
            Some(dt("2026-09-04 15:00")),
        );
        let changed = the_plan(&db, &org);
        assert!(
            !check(&db, &org, &changed, &platform)
                .unwrap()
                .already_pushed
        );
    }

    #[test]
    fn a_day_the_platform_holds_is_a_conflict_that_is_not_pushed() {
        let (db, org) = setup();
        let plan = the_plan(&db, &org);
        let platform = FakePlatform {
            existing: plan.worklog.entries.clone(),
            ..Default::default()
        };
        let gh = FakeGh::default();
        let c = check(&db, &org, &plan, &platform).unwrap();
        assert!(c.conflicts_on("2026-09-04") && c.conflict_lines()[0].contains("not pushed"));
        let items = execute(&db, &org, &plan, &c, &platform, &gh, now()).unwrap();
        assert!(platform.pushed.borrow().is_empty());
        assert!(gh.calls.borrow().is_empty());
        assert!(items.iter().all(|i| i.error.is_some()));
        assert!(db.find_push(org.id(), &plan.hash).unwrap().is_none());
    }

    #[test]
    fn a_conflict_does_not_stop_the_other_days() {
        let (db, org) = setup();
        let task = a_task(&db, &org);
        fixtures::add_session(
            &db,
            task,
            dt("2026-09-03 09:00"),
            Some(dt("2026-09-03 10:00")),
        );
        let both = DateRange {
            from: Some(NaiveDate::from_ymd_opt(2026, 9, 3).unwrap()),
            to: NaiveDate::from_ymd_opt(2026, 9, 4).unwrap(),
        };
        let plan = plan(&db, &org, both, now(), &Target::of(&org)).unwrap();
        assert_eq!(plan.worklog.entries.len(), 2);
        let mut taken = plan.worklog.entries[1].clone(); // the 4th
        taken.time = 9.0;
        let platform = FakePlatform {
            existing: vec![taken],
            ..Default::default()
        };
        let gh = FakeGh::default();
        let c = check(&db, &org, &plan, &platform).unwrap();
        let items = execute(&db, &org, &plan, &c, &platform, &gh, now()).unwrap();
        let sent = platform.pushed.borrow();
        assert_eq!(sent[0].entries.len(), 1);
        assert_eq!(sent[0].entries[0].day, "2026-09-03");
        assert!(items.iter().any(|i| i.error.is_some()), "the 4th is red");
        assert!(
            items
                .iter()
                .any(|i| i.line() == "ok: platform: 2026-09-03 1.00 h")
        );
        // "Ship" was completed on the conflicting 4th: its issue is left alone
        assert!(gh.calls.borrow().is_empty());
        assert!(db.find_push(org.id(), &plan.hash).unwrap().is_some());
    }

    #[test]
    fn an_entry_the_platform_refuses_is_red_and_leaves_github_alone() {
        let (db, org) = setup();
        let platform = FakePlatform {
            conflicts: vec!["2026-09-04".into()],
            ..Default::default()
        };
        let gh = FakeGh::default();
        let plan = the_plan(&db, &org);
        let items = run(&db, &org, &plan, &platform, &gh).unwrap();
        assert!(items[0].error.as_deref().unwrap().contains("conflict"));
        assert!(gh.calls.borrow().is_empty());
        assert!(db.find_push(org.id(), &plan.hash).unwrap().is_none());
    }

    #[test]
    fn an_already_closed_issue_is_not_closed_again() {
        let (db, org) = setup();
        let gh = FakeGh {
            closed: true,
            ..Default::default()
        };
        run(
            &db,
            &org,
            &the_plan(&db, &org),
            &FakePlatform::default(),
            &gh,
        )
        .unwrap();
        assert_eq!(
            *gh.calls.borrow(),
            vec!["view #7", "comment #7 Completed Ship"]
        );
    }

    #[test]
    fn a_platform_failure_leaves_github_and_the_ledger_untouched() {
        let (db, org) = setup();
        let platform = FakePlatform {
            refuse: Some("HTTP 500".into()),
            ..Default::default()
        };
        let gh = FakeGh::default();
        let plan = the_plan(&db, &org);
        assert!(run(&db, &org, &plan, &platform, &gh).is_err());
        assert!(gh.calls.borrow().is_empty());
        assert!(db.find_push(org.id(), &plan.hash).unwrap().is_none());
    }

    #[test]
    fn a_github_failure_is_reported_per_item_and_still_recorded() {
        let (db, org) = setup();
        let gh = FakeGh {
            fail_comment: true,
            ..Default::default()
        };
        let plan = the_plan(&db, &org);
        let items = run(&db, &org, &plan, &FakePlatform::default(), &gh).unwrap();
        assert!(items[0].error.is_none());
        assert!(items[1].error.as_deref().unwrap().contains("gh down"));
        let row = db.find_push(org.id(), &plan.hash).unwrap().unwrap();
        assert!(row.results.contains("FAILED") && row.results.contains("ok: platform"));
    }

    /// A day on the platform whose issue was not commented: rerunning finishes it, without a conflict or a second push.
    #[test]
    fn a_partial_push_is_not_already_pushed_and_a_rerun_finishes_the_github_step() {
        let (db, org) = setup();
        let plan = the_plan(&db, &org);
        let first = FakePlatform::default();
        let down = FakeGh {
            fail_comment: true,
            ..Default::default()
        };
        run(&db, &org, &plan, &first, &down).unwrap();

        let second = FakePlatform {
            existing: first.pushed.borrow()[0].entries.clone(),
            ..Default::default()
        };
        assert!(!check(&db, &org, &plan, &second).unwrap().already_pushed);
        let gh = FakeGh::default();
        let items = run(&db, &org, &plan, &second, &gh).unwrap();

        assert!(
            second.pushed.borrow().is_empty(),
            "the day is not sent twice"
        );
        assert!(items.iter().all(|i| i.error.is_none()), "{items:?}");
        assert_eq!(
            *gh.calls.borrow(),
            vec!["view #7", "close #7", "comment #7 Completed Ship"]
        );
        let row = db.find_push(org.id(), &plan.hash).unwrap().unwrap();
        assert!(row.complete());
        assert!(check(&db, &org, &plan, &second).unwrap().already_pushed);
    }

    #[test]
    fn an_open_session_marks_the_preview_and_changes_the_fingerprint() {
        let (db, org) = setup();
        let closed = the_plan(&db, &org);
        let task = a_task(&db, &org);
        fixtures::add_session(&db, task, dt("2026-09-04 17:00"), None);
        let open = the_plan(&db, &org);
        assert!(open.preview().contains("still open"));
        assert_ne!(open.hash, closed.hash);
    }

    #[test]
    fn unknown_projects_and_activities_are_flagged_only_against_a_non_empty_list() {
        let target = Target {
            organization: "acme".into(),
            activity: "dev".into(),
        };
        assert!(unknown_targets(&FakePlatform::default(), &target).is_empty());
        let wrong_project = FakePlatform {
            projects: vec![PlatformItem::new("p2", "beta")],
            ..Default::default()
        };
        assert!(unknown_targets(&wrong_project, &target)[0].contains("not one of your projects"));
        let wrong_activity = FakePlatform {
            projects: vec![PlatformItem::new("p1", "acme")],
            activities: vec![PlatformItem::new("a1", "ops")],
            ..Default::default()
        };
        assert!(unknown_targets(&wrong_activity, &target)[0].contains("not an activity of 'acme'"));
    }

    #[test]
    fn names_are_sent_as_the_platforms_ids() {
        let (db, org) = setup();
        let plan = the_plan(&db, &org);
        let platform = FakePlatform {
            projects: vec![PlatformItem::new("p1", "ACME")],
            activities: vec![PlatformItem::new("a1", "Dev")],
            ..Default::default()
        };
        let check = check(&db, &org, &plan, &platform).unwrap();
        execute(
            &db,
            &org,
            &plan,
            &check,
            &platform,
            &FakeGh::default(),
            dt("2026-09-04 18:00"),
        )
        .unwrap();
        let sent = platform.pushed.borrow();
        assert!(
            sent[0]
                .entries
                .iter()
                .all(|e| e.project == "p1" && e.activity == "a1")
        );
    }

    #[test]
    fn the_fingerprint_is_stable() {
        let (db, org) = setup();
        assert_eq!(the_plan(&db, &org).hash, the_plan(&db, &org).hash);
    }
}
