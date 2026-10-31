use crate::error::Result;
use crate::models::{Duration, Session, TaskStatus};
use chrono::{Datelike, NaiveDate, NaiveDateTime};
use serde::Serialize;
use serde_yaml::{Mapping, Value};
use std::collections::HashSet;
use std::fmt::Write;

/// How an `info` report is printed.
#[derive(clap::ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    /// A readable markdown report: YAML front matter, a `---` divider, then
    /// the description and a breakdown of the period's sessions
    #[default]
    Text,
    /// The same report as plain YAML, for piping somewhere else
    Yaml,
    /// One line per task worked on in the period: `Completed <task>` when
    /// the task is done, `Worked on <task>` otherwise -- what is pushed to
    /// a time platform
    Worklog,
}

/// What a text report says where a session table would otherwise go.
mod merge;
mod render;
#[cfg(test)]
mod tests;
mod worklog;

pub use merge::*;
pub use render::{Md, non_empty};
#[cfg(test)]
use render::message_bullets;
use render::{cell, document, summary_line};
pub use worklog::*;

const NO_SESSIONS: &str = "_No sessions in this period._";

// ---- report shapes -----------------------------------------------------

/// A model's own YAML fields minus the two every report already prints for
/// itself (`id`, `name`) -- i.e. its settings, in declaration order, for
/// the front matter block at the top of a text report. Taken
/// straight off `Serialize` so a field added to `Project`/`Task`/
/// `Organization` shows up here without a second list to keep in sync.
pub fn settings_of<T: Serialize>(item: &T) -> Result<Mapping> {
    let map = match serde_yaml::to_value(item)? {
        Value::Mapping(map) => map,
        _ => Mapping::new(),
    };
    // Rebuilt rather than `Mapping::remove`d from: that's a swap-remove,
    // which would shuffle the field a struct declared last into the hole it
    // left and lose the declaration order this is printed in.
    Ok(map
        .into_iter()
        .filter(|(key, _)| !matches!(key.as_str(), Some("id" | "name")))
        .collect())
}

/// The scalars every `info` report shares: what it's about, the period it
/// covers, the time that went into it, and the underlying row's settings.
/// Serialized on its own it's the YAML front matter of a text report; it's
/// `#[serde(flatten)]`ed into the YAML of the full report so both formats
/// carry the same fields under the same names.
#[derive(Debug, Clone, Serialize)]
pub struct Header {
    pub name: String,
    /// The period covered. Serializes as `date:` for a single day and
    /// `from:`/`to:` for an interval -- one field carrying what used to be
    /// three derived strings alongside it.
    #[serde(flatten)]
    pub period: DateRange,
    #[serde(flatten)]
    pub total: Total,
    #[serde(skip_serializing_if = "Mapping::is_empty")]
    pub settings: Mapping,
    /// The markdown notes off the row. Kept out of a text report's front
    /// matter (it's the body below the divider instead) but present in the
    /// YAML, where there is no body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Header {
    pub fn new(
        name: String,
        description: &str,
        settings: Mapping,
        period: DateRange,
        total_minutes: i64,
    ) -> Self {
        Header {
            name,
            period,
            total: Total::new(total_minutes),
            settings,
            description: non_empty(description),
        }
    }

    /// A header for `item`, whose settings are taken off its own fields.
    pub fn of<T: Serialize>(
        name: String,
        description: &str,
        item: &T,
        period: DateRange,
        total_minutes: i64,
    ) -> Result<Self> {
        Ok(Self::new(name, description, settings_of(item)?, period, total_minutes))
    }
}

/// Renders `report` in `format` and prints it: the tail of every `info`.
pub fn print_report(report: &impl Report, format: Format) -> Result<()> {
    print!("{}", report.render(format)?);
    Ok(())
}

/// One row of a session table: when the stretch of work ran, how long it
/// lasted, and the note left when it was closed. `end: None` means the
/// session is still open, and `duration` is measured against now.
#[derive(Debug, Serialize)]
pub struct SessionRow {
    /// The calendar day the session started on, and its length in minutes:
    /// what a worklog groups by. Not part of the printed report.
    #[serde(skip)]
    pub day: NaiveDate,
    /// Only carried when the report spans more than one day -- on a
    /// single-day report every row would repeat the header's own date.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    pub start: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    pub duration: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Turns `sessions` (already filtered to the report's period) into table
/// rows. `dated` comes from the period covering more than one day.
pub fn session_rows(sessions: &[Session], now: NaiveDateTime, dated: bool) -> Vec<SessionRow> {
    sessions
        .iter()
        .map(|s| SessionRow {
            day: s.start.date(),
            date: dated.then(|| fmt_date(s.start.date())),
            start: s.start.format("%H:%M").to_string(),
            end: s.end.map(|e| e.format("%H:%M").to_string()),
            duration: s.duration_text(now),
            message: s.message.as_deref().and_then(non_empty),
        })
        .collect()
}

/// One task's section of a project or organization report: what it is,
/// where it stands, and every session it had in the period.
#[derive(Debug, Serialize)]
pub struct TaskReport {
    /// The task's row id -- for the web report's links; not part of the
    /// printed report.
    #[serde(skip)]
    #[cfg_attr(not(feature = "web"), allow(dead_code))]
    pub id: i64,
    pub name: String,
    /// Serializes as `queue`/`wip`/`done`, as the plain string it replaced
    /// did -- but carried as the enum, so the label below is derived from
    /// it rather than being a second thing to keep in step.
    pub status: TaskStatus,
    /// The status spelled out -- `wip` reads as "work in progress" once
    /// it's sitting next to a task name. Derived from `status` by
    /// [`TaskReport::new`] rather than passed in, but stored rather than
    /// computed at render time because `-f yaml` has always carried it and
    /// something may be reading it.
    pub status_label: String,
    #[serde(flatten)]
    pub total: Total,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub sessions: Vec<SessionRow>,
}

impl TaskReport {
    pub fn new(
        id: i64,
        name: String,
        status: TaskStatus,
        total_minutes: i64,
        description: Option<String>,
        sessions: Vec<SessionRow>,
    ) -> Self {
        TaskReport {
            id,
            name,
            status,
            status_label: status.label().to_string(),
            total: Total::new(total_minutes),
            description,
            sessions,
        }
    }
}

/// One project's section of an organization report. `total_hhmm` is the
/// union of its tasks' sessions, so it can be less than their totals summed
/// -- see `merged_total_minutes`.
#[derive(Debug, Serialize)]
pub struct ProjectReport {
    /// The project's row id -- for the web report's links; not part of the
    /// printed report.
    #[serde(skip)]
    #[cfg_attr(not(feature = "web"), allow(dead_code))]
    pub id: i64,
    pub name: String,
    #[serde(flatten)]
    pub total: Total,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub tasks: Vec<TaskReport>,
}

#[derive(Debug, Serialize)]
pub struct TaskInfo {
    #[serde(flatten)]
    pub header: Header,
    pub sessions: Vec<SessionRow>,
}

#[derive(Debug, Serialize)]
pub struct ProjectInfo {
    #[serde(flatten)]
    pub header: Header,
    pub tasks: Vec<TaskReport>,
}

#[derive(Debug, Serialize)]
pub struct OrganizationInfo {
    #[serde(flatten)]
    pub header: Header,
    pub projects: Vec<ProjectReport>,
}

#[derive(Debug, Serialize)]
pub struct WeekdayReport {
    pub name: String,
    pub weekdays: Vec<WeekdayAverage>,
}

// ---- rendering ---------------------------------------------------------

/// A report that can be printed: a header every report shares, and the
/// body only this kind of report knows how to write.
///
/// The two formats are handled once here rather than three times: `Yaml` is
/// the serialized struct, and `Text` is the same fenced front matter every
/// report opens with -- title, summary line, description -- followed by
/// [`Self::body`]. A new report kind implements two methods and gets both.
pub trait Report: Serialize {
    fn header(&self) -> &Header;

    /// Everything below the title, summary line and description.
    fn body(&self, md: &mut Md);

    /// The `worklog` rendering, for the report kinds that list tasks.
    fn worklog(&self) -> Result<String> {
        Err(crate::error::IterError::CommandFailed(
            "the worklog format is only available for project and organization reports".into(),
        ))
    }

    fn render(&self, format: Format) -> Result<String> {
        match format {
            Format::Yaml => return Ok(serde_yaml::to_string(self)?),
            Format::Worklog => return self.worklog(),
            Format::Text => {}
        }
        let header = self.header();
        let mut md = Md::new();
        md.heading(1, &header.name);
        md.block(&summary_line(header));
        if let Some(description) = &header.description {
            md.block(description);
        }
        self.body(&mut md);
        document(header, md)
    }
}

impl Report for TaskInfo {
    fn header(&self) -> &Header {
        &self.header
    }

    fn body(&self, md: &mut Md) {
        md.session_table(&self.sessions);
    }
}

impl Report for ProjectInfo {
    fn header(&self) -> &Header {
        &self.header
    }

    fn body(&self, md: &mut Md) {
        md.task_breakdown(&self.tasks, 2);
    }

    fn worklog(&self) -> Result<String> {
        Ok(worklog_text(self.tasks.iter()))
    }
}

impl Report for OrganizationInfo {
    fn header(&self) -> &Header {
        &self.header
    }

    fn body(&self, md: &mut Md) {
        if self.projects.is_empty() {
            md.block(NO_SESSIONS);
        }
        md.table(
            &["Project", "Time"],
            &self
                .projects
                .iter()
                .map(|p| vec![cell(&p.name), p.total.total_hhmm.clone()])
                .collect::<Vec<_>>(),
        );
        for project in &self.projects {
            md.project_section(project, 2);
        }
    }

    fn worklog(&self) -> Result<String> {
        Ok(worklog_text(self.projects.iter().flat_map(|p| &p.tasks)))
    }
}
