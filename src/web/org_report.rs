//! The organization's Report tab (`/org/{id}?tab=report`): the period asked
//! for, the report loaded through the same function `iter org info` uses, and
//! the view (period picker, summary, contents, one section per project). The
//! page around it is `org.rs`; its rules are in `/org.css` and its script
//! is `/org.js`.

use super::session_lines::report_lines;
use super::ui::empty_line::empty_line;
use super::ui::notice::notice;
use super::ui::sessions_table::sessions_table;
use super::ui::status::status;
use super::ui::{CLIPBOARD_CHECK, COPY, icon};
use super::url::{page_url, project_url, task_url};
use crate::db::Db;
use crate::models::Project;
use crate::reporting::{DateRange, ProjectReport, TaskReport, Total, fmt_date};
use crate::utils::report::{parse_day, project_reports, total_minutes};
use chrono::{Days, NaiveDate, NaiveDateTime};
use topcoat::{
    Result,
    view::{View, component, view},
};

/// The organization's report for a period: what `iter org info` prints,
/// built by the same function, so the numbers are the same.
pub struct Report {
    pub range: DateRange,
    pub total: Total,
    pub projects: Vec<ProjectReport>,
    pub sessions: usize,
    /// Why the requested period was ignored, if it was.
    pub error: Option<String>,
}

/// The default period: the last seven days, today included.
fn last_days(today: NaiveDate, days: u64) -> DateRange {
    DateRange {
        from: today.checked_sub_days(Days::new(days - 1)),
        to: today,
    }
}

/// The period `from`/`to` ask for. Both absent is the default (7 days);
/// an empty `from` is no lower bound ("All time"); an empty or absent `to`
/// is today. A date that doesn't parse, or `from` after `to`, falls back
/// to the default with an error to show.
fn period(from: Option<&str>, to: Option<&str>, today: NaiveDate) -> (DateRange, Option<String>) {
    if from.is_none() && to.is_none() {
        return (last_days(today, 7), None);
    }
    let day = |v: Option<&str>| match v.map(str::trim) {
        None | Some("") => Ok(None),
        Some(v) => parse_day(v).map(Some),
    };
    match (day(from), day(to)) {
        (Ok(from), Ok(to)) => {
            let to = to.unwrap_or(today);
            match from {
                Some(f) if f > to => (
                    last_days(today, 7),
                    Some(format!(
                        "{} is after {}; showing the last 7 days.",
                        fmt_date(f),
                        fmt_date(to)
                    )),
                ),
                _ => (DateRange { from, to }, None),
            }
        }
        _ => (
            last_days(today, 7),
            Some("Dates are YYYY-MM-DD; showing the last 7 days.".to_string()),
        ),
    }
}

pub fn load(
    db: &Db,
    projects: &[Project],
    from: Option<&str>,
    to: Option<&str>,
    now: NaiveDateTime,
) -> crate::error::Result<Report> {
    let (range, error) = period(from, to, now.date());
    let (projects, sessions) = project_reports(db, projects, range, now)?;
    Ok(Report {
        range,
        // The union across projects, as `iter org info` totals it.
        total: Total::new(total_minutes(&sessions, now)),
        projects,
        sessions: sessions.len(),
        error,
    })
}

/// The report tab of `base` for a period: the one place a report URL is
/// built. `from` and `to` are given both or neither (an empty `from` is
/// "all time").
fn report_link(base: &str, from: Option<&str>, to: Option<&str>) -> String {
    page_url(base, &[("tab", Some("report")), ("from", from), ("to", to)])
}

fn report_url(base: &str, range: &DateRange) -> String {
    let from = range.from.map(fmt_date).unwrap_or_default();
    report_link(base, Some(&from), Some(&fmt_date(range.to)))
}

/// The Report tab's own link: the default period (the last 7 days).
pub fn tab_url(base: &str, today: NaiveDate) -> String {
    report_url(base, &last_days(today, 7))
}

fn plural(n: usize, one: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {one}s")
    }
}

/// The report's projects and their tasks' statuses, as plain text to paste
/// somewhere else:
///
/// ```text
/// proj:
/// - task: work in progress
/// ```
fn status_list(report: &Report) -> String {
    let mut out = String::new();
    for p in &report.projects {
        out.push_str(&format!("{}:\n", p.name));
        for t in &p.tasks {
            out.push_str(&format!("- {}: {}\n", t.name, t.status_label));
        }
    }
    out.trim_end().to_string()
}

/// The preset periods and the From/To form.
#[component]
async fn period_picker(base: &str, report: &Report, today: NaiveDate) -> Result<impl View> {
    let presets = [
        ("Today", DateRange::day(today)),
        ("7 days", last_days(today, 7)),
        ("30 days", last_days(today, 30)),
        (
            "All time",
            DateRange {
                from: None,
                to: today,
            },
        ),
    ];
    let from = report.range.from.map(fmt_date).unwrap_or_default();
    let to = fmt_date(report.range.to);
    Ok(view! {
        <div class="period">
            <nav class="presets" aria-label="Period">
                for (label, range) in presets.iter() {
                    <a href=(report_url(base, range)) if *range == report.range { aria-current="true" }>(label.to_string())</a>
                }
            </nav>
            <form class="range" method="get" action=(base.to_string())>
                <input type="hidden" name="tab" value="report">
                <label>"From" <input type="date" class="control" name="from" value=(from.clone())></label>
                <label>"To" <input type="date" class="control" name="to" value=(to.clone())></label>
                // Dates apply as they change (`org.js`). This button is what
                // makes Enter in a date field submit (also without JavaScript)
                // and gives keyboard users an explicit Apply: it is off screen
                // until it has focus.
                <button type="submit" class="button small apply">"Apply"</button>
            </form>
            <script src="/org.js" defer=""></script>
        </div>
    })
}

#[component]
async fn task_section(t: &TaskReport) -> Result<impl View> {
    let lines = report_lines(&t.sessions);
    let caption = format!("Sessions of {}", t.name);
    Ok(view! {
        <article class="task">
            <header>
                <h3><a class="u" href=(task_url(t.id))>(t.name.clone())</a></h3>
                status(state: t.status)
                <span class="time">(t.total.total_hhmm.clone())</span>
            </header>
            if let Some(d) = &t.description {
                <p class="tdesc">(d.clone())</p>
            }
            sessions_table(lines: &lines, caption: &caption)
        </article>
    })
}

#[component]
pub async fn report_view(base: &str, report: &Report, today: NaiveDate) -> Result<impl View> {
    let tasks: usize = report.projects.iter().map(|p| p.tasks.len()).sum();
    // Each project's share of the time the projects add up to (which can
    // exceed the merged total when two were worked on at once).
    let sum: i64 = report
        .projects
        .iter()
        .map(|p| p.total.minutes)
        .sum::<i64>()
        .max(1);
    let share = move |p: &ProjectReport| {
        format!("width:{:.1}%", p.total.minutes as f64 * 100.0 / sum as f64)
    };
    Ok(view! {
        period_picker(base: base, report: report, today: today)
        if let Some(e) = &report.error {
            notice(error: true, (e.as_str()))
        }
        <div class="summary">
            <span class="big">(report.total.total_hhmm.clone())</span>
            <span>
                (plural(report.projects.len(), "project"))
                " · " (plural(tasks, "task"))
                " · " (plural(report.sessions, "session"))
            </span>
            if !report.projects.is_empty() {
                // Hidden until `org.js` finds a clipboard to write to.
                <button type="button" class="button copy" data-copy=(status_list(report)) hidden="">
                    <span class="idle">(icon(COPY)) "Copy summary"</span>
                    <span class="done">(icon(CLIPBOARD_CHECK)) "Copied"</span>
                </button>
                <span class="sr" id="copy-status" role="status"></span>
            }
        </div>
        if report.projects.is_empty() {
            empty_line("No sessions in this period.")
        } else {
            <div class="report">
                <nav class="toc" id="contents" aria-labelledby="toc-title">
                    <h2 id="toc-title" class="label">"Contents"</h2>
                    <ol>
                        for p in report.projects.iter() {
                            <li>
                                <a href=(format!("#p-{}", p.id))>
                                    <span class="name">(p.name.clone())</span>
                                    <span class="time">(p.total.total_hhmm.clone())</span>
                                    <span class="share" aria-hidden="true"><i style=(share(p))></i></span>
                                </a>
                            </li>
                        }
                    </ol>
                </nav>
                <div class="sections">
                    for p in report.projects.iter() {
                        <section class="proj" id=(format!("p-{}", p.id)) aria-labelledby=(format!("p-{}-title", p.id))>
                            <header>
                                <h2 id=(format!("p-{}-title", p.id))><a class="u" href=(project_url(p.id))>(p.name.clone())</a></h2>
                                <span class="time">(p.total.total_hhmm.clone())</span>
                            </header>
                            if let Some(d) = &p.description {
                                <p class="pdesc">(d.clone())</p>
                            }
                            for t in p.tasks.iter() {
                                task_section(t: t)
                            }
                        </section>
                    }
                </div>
            </div>
        }
    })
}
