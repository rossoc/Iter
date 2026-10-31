//! Gathering the numbers an `info` report is made of: the days covered and the per-task and
//! per-project breakdowns. Kept apart from [`crate::reporting`], which renders and knows nothing of
//! the database or the CLI.

use crate::args::ReportOpts;
use crate::config::config;
use crate::db::{Db, Table};
use crate::error::{IterError, Result};
use crate::models::{Project, Session, Task};
use crate::reporting::{
    DateRange, ProjectReport, TaskReport, merged_total_minutes, non_empty, session_rows,
};
use chrono::{NaiveDate, NaiveDateTime};
use std::collections::HashMap;

/// Total minutes across `sessions`, with the pause threshold from the config; `reporting`'s own
/// [`merged_total_minutes`] takes an explicit gap so it is testable.
pub(crate) fn total_minutes(sessions: &[Session], now: NaiveDateTime) -> i64 {
    merged_total_minutes(sessions, now, config().pause_gap_minutes)
}

pub(crate) fn parse_day(value: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| IterError::InvalidDate(value.to_string()))
}

/// The days an `info` report covers: `--date` (or nothing) is one day; `--from`/`--to` an interval
/// (`--to` defaults to today, `--from` to unbounded). clap rejects mixing them.
pub(crate) fn resolve_range(opts: &ReportOpts, now: chrono::NaiveDateTime) -> Result<DateRange> {
    let day = |value: &Option<String>| value.as_deref().map(parse_day).transpose();
    if opts.from.is_none() && opts.to.is_none() {
        return Ok(DateRange::day(day(&opts.date)?.unwrap_or(now.date())));
    }
    let from = day(&opts.from)?;
    let to = day(&opts.to)?.unwrap_or(now.date());
    if let Some(from) = from
        && from > to
    {
        return Err(IterError::InvalidDateRange {
            from: from.to_string(),
            to: to.to_string(),
        });
    }
    Ok(DateRange { from, to })
}

/// One `TaskReport` per task of `tasks` with sessions in `by_task` (consumed), plus the union of
/// the sessions used. `dated` shows each session's date.
fn build_task_reports(
    tasks: Vec<Task>,
    by_task: &mut HashMap<i64, Vec<Session>>,
    now: NaiveDateTime,
    dated: bool,
) -> (Vec<TaskReport>, Vec<Session>) {
    let mut reports = Vec::new();
    let mut all = Vec::new();
    for task in tasks {
        let Some(sessions) = by_task.remove(&task.id()) else {
            continue; // nothing in the period -- left out of the report
        };
        let total_minutes = total_minutes(&sessions, now);
        reports.push(TaskReport::new(
            task.id(),
            task.name,
            task.status,
            total_minutes,
            non_empty(&task.description),
            session_rows(&sessions, now, dated),
        ));
        all.extend(sessions);
    }
    (reports, all)
}

fn group_by_task(sessions: Vec<Session>) -> HashMap<i64, Vec<Session>> {
    let mut by_task: HashMap<i64, Vec<Session>> = HashMap::new();
    for session in sessions {
        by_task.entry(session.task_id).or_default().push(session);
    }
    by_task
}

/// One `TaskReport` per task of `project_id` with a session in `range`; two queries in all.
pub(crate) fn task_reports(
    db: &Db,
    project_id: i64,
    range: DateRange,
    now: chrono::NaiveDateTime,
) -> Result<(Vec<TaskReport>, Vec<Session>)> {
    let mut by_task =
        group_by_task(db.sessions_for_project_in_range(project_id, range.from, range.to)?);
    let tasks = db.tasks_for_project(project_id)?;
    Ok(build_task_reports(
        tasks,
        &mut by_task,
        now,
        range.single_day().is_none(),
    ))
}

/// [`task_reports`] one level up: one `ProjectReport` per project with a session in `range`, plus
/// the union of all sessions; one sessions query and one tasks query in all.
pub(crate) fn project_reports(
    db: &Db,
    projects: &[Project],
    range: DateRange,
    now: NaiveDateTime,
) -> Result<(Vec<ProjectReport>, Vec<Session>)> {
    let ids: Vec<i64> = projects.iter().map(Project::id).collect();
    let mut by_task = group_by_task(db.sessions_for_projects_in_range(&ids, range.from, range.to)?);
    let mut tasks_of: HashMap<i64, Vec<Task>> = HashMap::new();
    for task in db.tasks_for_projects(&ids)? {
        tasks_of.entry(task.project_id).or_default().push(task);
    }
    let dated = range.single_day().is_none();

    let mut reports = Vec::new();
    let mut all = Vec::new();
    for project in projects {
        let tasks = tasks_of.remove(&project.id()).unwrap_or_default();
        let (tasks, sessions) = build_task_reports(tasks, &mut by_task, now, dated);
        if tasks.is_empty() {
            continue;
        }
        reports.push(ProjectReport {
            id: project.id(),
            name: project.name.clone(),
            total: crate::reporting::Total::new(total_minutes(&sessions, now)),
            description: non_empty(&project.description),
            tasks,
        });
        all.extend(sessions);
    }
    Ok((reports, all))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::fixtures::dt;
    use crate::models::TaskStatus;

    /// The per-project loop, as the reference for the batch.
    fn per_project(
        db: &Db,
        projects: &[Project],
        range: DateRange,
        now: NaiveDateTime,
    ) -> (Vec<ProjectReport>, Vec<Session>) {
        let mut reports = Vec::new();
        let mut all = Vec::new();
        for project in projects {
            let (tasks, sessions) = task_reports(db, project.id(), range, now).expect("tasks");
            if tasks.is_empty() {
                continue;
            }
            reports.push(ProjectReport {
                id: project.id(),
                name: project.name.clone(),
                total: crate::reporting::Total::new(total_minutes(&sessions, now)),
                description: non_empty(&project.description),
                tasks,
            });
            all.extend(sessions);
        }
        (reports, all)
    }

    fn project(db: &Db, name: &str) -> Project {
        let mut p = fixtures::project(name);
        p.description = format!("about {name}");
        let id = db.insert(&p).expect("project");
        db.get::<Project>(id).expect("read").expect("row")
    }

    fn task(db: &Db, project: &Project, name: &str) -> i64 {
        let mut t = Task::template(project.id(), String::new(), TaskStatus::Wip);
        t.name = name.into();
        t.status = TaskStatus::Wip;
        db.insert(&t).expect("task")
    }

    fn session(db: &Db, task_id: i64, start: &str, end: Option<&str>) {
        let session = Session {
            message: Some("note".into()),
            ..fixtures::session(task_id, dt(start), end.map(dt))
        };
        db.insert(&session).expect("session");
    }

    #[test]
    fn the_batched_report_equals_the_per_project_one() {
        let db = fixtures::db();
        // Three projects (the middle one is idle), tasks created out of
        // alphabetical order, and overlapping sessions across projects.
        let (a, b, c) = (
            project(&db, "alpha"),
            project(&db, "beta"),
            project(&db, "gamma"),
        );
        let (a2, a1) = (task(&db, &a, "zed"), task(&db, &a, "amp"));
        let b1 = task(&db, &b, "idle");
        let c1 = task(&db, &c, "solo");
        let c2 = task(&db, &c, "empty");
        session(&db, a1, "2026-09-28 09:00", Some("2026-09-28 10:00"));
        session(&db, a2, "2026-09-28 09:30", Some("2026-09-28 11:00"));
        session(&db, a2, "2026-09-29 08:00", Some("2026-09-29 08:45"));
        session(&db, c1, "2026-09-28 09:15", Some("2026-09-28 09:45"));
        session(&db, c1, "2026-09-29 13:00", None);
        session(&db, b1, "2026-01-05 09:00", Some("2026-01-05 10:00"));
        let _ = c2;
        let now = dt("2026-09-29 15:00");
        let day = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("day");
        let projects = [a, b, c];
        for range in [
            DateRange::day(day("2026-09-28")),
            DateRange {
                from: Some(day("2026-09-27")),
                to: day("2026-09-29"),
            },
            DateRange {
                from: None,
                to: day("2026-09-29"),
            },
        ] {
            let batched = project_reports(&db, &projects, range, now).expect("batch");
            let looped = per_project(&db, &projects, range, now);
            assert_eq!(format!("{batched:?}"), format!("{looped:?}"));
        }
        let all_time = DateRange {
            from: None,
            to: day("2026-09-29"),
        };
        let (reports, _) = project_reports(&db, &projects, all_time, now).expect("batch");
        // Report order follows the projects; tasks come by name.
        let names: Vec<_> = reports.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["alpha", "beta", "gamma"]);
        let tasks: Vec<_> = reports[0].tasks.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(tasks, ["amp", "zed"]);
    }
}
