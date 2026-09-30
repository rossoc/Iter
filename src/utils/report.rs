//! Gathering the numbers an `info` report is made of: which days it covers,
//! and the per-task / per-project breakdowns under them.
//!
//! Separate from [`crate::reporting`], which turns these into text or YAML
//! and knows nothing about the database or the command line. Keeping that
//! module free of both is what lets its rendering be tested without either.

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

/// Total minutes across `sessions`, with the pause threshold read from the
/// config file. The one place that lookup happens: `reporting`'s own
/// [`merged_total_minutes`] keeps an explicit gap so it stays testable
/// without a config.
pub(crate) fn total_minutes(sessions: &[Session], now: NaiveDateTime) -> i64 {
    merged_total_minutes(sessions, now, config().pause_gap_minutes)
}

pub(crate) fn parse_day(value: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| IterError::InvalidDate(value.to_string()))
}

/// The days an `info` report covers. `--date` (or nothing at all) is a
/// single day; `--from`/`--to` is an interval, with `--to` defaulting to
/// today and `--from` to no lower bound at all. clap already rejects mixing
/// the two formulations, so only one branch can apply.
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

/// The pure part of a report: one `TaskReport` per task of `tasks` (one
/// project's, by name) that has sessions in `by_task`, which is consumed.
/// Tasks untouched in the period are left out. Also hands back the union of
/// the sessions it used, so the caller can total the project without asking
/// the database twice. `dated` says the report spans more than one day, so
/// each session row shows its date.
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

/// The sessions grouped by the task they belong to, each group in the order
/// the query returned them (oldest first).
fn group_by_task(sessions: Vec<Session>) -> HashMap<i64, Vec<Session>> {
    let mut by_task: HashMap<i64, Vec<Session>> = HashMap::new();
    for session in sessions {
        by_task.entry(session.task_id).or_default().push(session);
    }
    by_task
}

/// One `TaskReport` per task of `project_id` that had a session in `range`
/// -- the per-task breakdown a project report is built from. A task's
/// sessions cover only the period, same as the report's own filter. Two
/// queries for the whole project, not one per task.
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

/// One [`ProjectReport`] per project of `projects` (an organization's, as
/// the caller already loaded them, in the order to report them) that had a
/// session in `range`, plus the union of every session behind them: what
/// [`task_reports`] does, one level up. One sessions query and one tasks
/// query for the whole set, grouped here, however many projects there are.
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
    use crate::models::{Settings, TaskStatus};

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").expect("fixture datetime")
    }

    /// The per-project loop the batch replaced, kept as the reference.
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
        let mut p = Project::template(&Settings::default());
        p.name = name.into();
        p.base_path = format!("/tmp/{name}");
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
        db.insert(&Session {
            id: None,
            task_id,
            start: dt(start),
            end: end.map(dt),
            message: Some("note".into()),
        })
        .expect("session");
    }

    #[test]
    fn the_batched_report_equals_the_per_project_one() {
        let db = Db::open(":memory:").expect("db");
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
