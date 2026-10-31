//! The organization's Report tab (`/org/{id}?tab=report`): the period, the report from the function `iter org info` uses, and the view.

use super::org_share::{Outcome, share_outcome};
use super::session_lines::report_lines;
use super::ui::empty_line::empty_line;
use super::ui::notice::notice;
use super::ui::prose::description;
use super::ui::sessions_table::sessions_table;
use super::ui::status::status;
use super::ui::{CHEVRON_LEFT, CHEVRON_RIGHT};
use super::ui::{SHARE, icon};
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

pub struct Report {
    pub range: DateRange,
    pub total: Total,
    pub projects: Vec<ProjectReport>,
    pub sessions: usize,
    pub error: Option<String>,
}

fn last_days(today: NaiveDate, days: u64) -> DateRange {
    DateRange {
        from: today.checked_sub_days(Days::new(days - 1)),
        to: today,
    }
}

/// An empty `from` is no lower bound ("All time"); an empty or absent `to` is today; a bad date or `from` after `to` falls back to the default.
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
        total: Total::new(total_minutes(&sessions, now)),
        projects,
        sessions: sessions.len(),
        error,
    })
}

/// The one place a report URL is built; `from` and `to` are given both or neither.
pub(super) fn report_link(base: &str, from: Option<&str>, to: Option<&str>) -> String {
    page_url(base, &[("tab", Some("report")), ("from", from), ("to", to)])
}

fn report_url(base: &str, range: &DateRange) -> String {
    let from = range.from.map(fmt_date).unwrap_or_default();
    report_link(base, Some(&from), Some(&fmt_date(range.to)))
}

fn share_url(base: &str, range: &DateRange) -> String {
    let from = range.from.map(fmt_date).unwrap_or_default();
    page_url(
        base,
        &[
            ("tab", Some("report")),
            ("from", Some(&from)),
            ("to", Some(&fmt_date(range.to))),
            ("share", Some("1")),
        ],
    )
}

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

pub fn status_list(report: &Report) -> String {
    let mut out = String::new();
    for p in &report.projects {
        out.push_str(&format!("{}:\n", p.name));
        for t in &p.tasks {
            out.push_str(&format!("- {}: {}\n", t.name, t.status_label));
        }
    }
    out.trim_end().to_string()
}

/// "All time" has no neighbours; nothing comes after a period that reaches today.
fn neighbours(range: &DateRange, today: NaiveDate) -> (Option<DateRange>, Option<DateRange>) {
    let Some(from) = range.from else {
        return (None, None);
    };
    let days = (range.to - from).num_days() as u64 + 1;
    let prev = from.checked_sub_days(Days::new(1)).map(|to| DateRange {
        from: to.checked_sub_days(Days::new(days - 1)),
        to,
    });
    let next = (range.to < today).then(|| {
        let from = range.to + Days::new(1);
        DateRange {
            from: Some(from),
            to: (from + Days::new(days - 1)).min(today),
        }
    });
    (prev, next)
}

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
    let (prev, next) = neighbours(&report.range, today);
    Ok(view! {
        <div class="period">
            <nav class="day-nav" aria-label="Change period">
                if let Some(range) = &prev {
                    <a class="button" href=(report_url(base, range)) aria-label="Previous period" rel="prev">(icon(CHEVRON_LEFT))</a>
                } else {
                    <span class="button" aria-disabled="true" aria-label="Previous period">(icon(CHEVRON_LEFT))</span>
                }
                if let Some(range) = &next {
                    <a class="button" href=(report_url(base, range)) aria-label="Next period" rel="next">(icon(CHEVRON_RIGHT))</a>
                } else {
                    <span class="button" aria-disabled="true" aria-label="Next period">(icon(CHEVRON_RIGHT))</span>
                }
            </nav>
            <nav class="presets" aria-label="Period">
                for (label, range) in presets.iter() {
                    <a href=(report_url(base, range)) if *range == report.range { aria-current="true" }>(label.to_string())</a>
                }
            </nav>
            <form class="range" method="get" action=(base.to_string())>
                <input type="hidden" name="tab" value="report">
                <label>"From" <input type="date" class="control" name="from" value=(from.clone())></label>
                <label>"To" <input type="date" class="control" name="to" value=(to.clone())></label>
                // Dates apply as they change (`org.js`); this button makes Enter submit, also without JavaScript.
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
                description(text: d, class: "tdesc")
            }
            sessions_table(lines: &lines, caption: &caption)
        </article>
    })
}

#[component]
pub async fn report_view(
    base: &str,
    report: &Report,
    today: NaiveDate,
    #[default] shared: Option<&Outcome>,
) -> Result<impl View> {
    let share_link = share_url(base, &report.range);
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
                <a class="button share-open" href=(share_link.as_str())>(icon(SHARE)) "Share"</a>
            }
        </div>
        if let Some(outcome) = shared {
            share_outcome(outcome: outcome)
        }
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
                                description(text: d, class: "pdesc")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        parse_day(s).expect("a date")
    }

    fn range(from: &str, to: &str) -> DateRange {
        DateRange {
            from: Some(d(from)),
            to: d(to),
        }
    }

    #[test]
    fn a_period_has_neighbours_of_its_own_length() {
        let today = d("2026-10-29");
        let (prev, next) = neighbours(&range("2026-10-10", "2026-10-16"), today);
        assert_eq!(prev, Some(range("2026-10-03", "2026-10-09")));
        assert_eq!(next, Some(range("2026-10-17", "2026-10-23")));
        let (_, next) = neighbours(&range("2026-10-20", "2026-10-26"), today);
        assert_eq!(next, Some(range("2026-10-27", "2026-10-29")));
        assert_eq!(
            neighbours(&range("2026-10-23", "2026-10-29"), today).1,
            None
        );
        let all = DateRange {
            from: None,
            to: today,
        };
        assert_eq!(neighbours(&all, today), (None, None));
    }
}
