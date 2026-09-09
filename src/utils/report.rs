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
use crate::models::Session;
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

/// One `TaskReport` per task of `project_id` that had a session in `range`
/// -- the per-task breakdown a project (or organization) report is built
/// from. Tasks untouched in the period are left out; a task's sessions
/// cover only the period, same as the report's own filter. Also hands back
/// the union of every session it looked at, so the caller can total the
/// project without asking the database twice.
pub(crate) fn task_reports(
    db: &Db,
    project_id: i64,
    range: DateRange,
    now: chrono::NaiveDateTime,
) -> Result<(Vec<TaskReport>, Vec<Session>)> {
    let dated = range.single_day().is_none();

    // Two queries for the whole project, not one per task: the sessions
    // come back already filtered to the period and ordered by start, so
    // grouping them by task is all that's left to do here.
    let mut by_task: HashMap<i64, Vec<Session>> = HashMap::new();
    for session in db.sessions_for_project_in_range(project_id, range.from, range.to)? {
        by_task.entry(session.task_id).or_default().push(session);
    }

    let mut reports = Vec::new();
    let mut all = Vec::new();
    for task in db.tasks_for_project(project_id)? {
        let Some(sessions) = by_task.remove(&task.id()) else {
            continue; // nothing in the period -- left out of the report
        };
        let total_minutes = total_minutes(&sessions, now);
        reports.push(TaskReport::new(
            task.name,
            task.status,
            total_minutes,
            non_empty(&task.description),
            session_rows(&sessions, now, dated),
        ));
        all.extend(sessions);
    }
    Ok((reports, all))
}

/// One [`ProjectReport`] per project of `organization_id` that had a session
/// in `range`, plus the union of every session behind them -- exactly what
/// [`task_reports`] does, one level up, so an organization can be totalled
/// without walking its projects twice.
pub(crate) fn project_reports(
    db: &Db,
    organization_id: i64,
    range: DateRange,
    now: NaiveDateTime,
) -> Result<(Vec<ProjectReport>, Vec<Session>)> {
    let mut reports = Vec::new();
    let mut all = Vec::new();
    for project in db.projects_for_organization(organization_id)? {
        let (tasks, sessions) = task_reports(db, project.id(), range, now)?;
        if tasks.is_empty() {
            continue;
        }
        reports.push(ProjectReport {
            name: project.name,
            total: crate::reporting::Total::new(total_minutes(&sessions, now)),
            description: non_empty(&project.description),
            tasks,
        });
        all.extend(sessions);
    }
    Ok((reports, all))
}
