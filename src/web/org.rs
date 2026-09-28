//! The proposed design of the organization page (`/org/{id}`). The route
//! itself stays in `pages.rs`, which renders the old design under
//! `?design=old` and this one otherwise. The page's own rules are in
//! `/org.css`; the shared ones in `/v2.css`.

use super::v2::{
    CHECK, CIRCLE, CIRCLE_CHECK, CLIPBOARD_CHECK, COPY, HALF, MINUS, PENCIL, frame, icon, tilde,
};
use crate::db::{Db, Table};
use crate::models::{Configured, Organization, Project, Settings, Task, TaskStatus};
use crate::reporting::{DateRange, ProjectReport, TaskReport, Total, fmt_date};
use crate::utils::report::{parse_day, project_reports, total_minutes};
use chrono::{Days, NaiveDate, NaiveDateTime};
use topcoat::{
    Result,
    router::{RouterBuilder, content::Css, route},
    view::{Unescaped, View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.route(stylesheet)
}

#[route(GET "/org.css")]
async fn stylesheet() -> Result<Css<&'static str>> {
    Ok(Css(include_str!("org.css")))
}

fn status_icon(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Queue => CIRCLE,
        TaskStatus::Wip => HALF,
        TaskStatus::Done => CIRCLE_CHECK,
    }
}

/// One yes/no setting.
#[component]
async fn flag(label: &str, on: bool) -> Result<impl View> {
    Ok(view! {
        <dt>(label.to_string())</dt>
        <dd class=(if on { "flag on" } else { "flag" })>
            (icon(if on { CHECK } else { MINUS }))
            (if on { "on" } else { "off" })
        </dd>
    })
}

/// One text setting, "—" when unset.
#[component]
async fn text(label: &str, value: &str) -> Result<impl View> {
    Ok(view! {
        <dt>(label.to_string())</dt>
        if value.is_empty() {
            <dd class="unset">"—"</dd>
        } else {
            <dd class="mono">(value.to_string())</dd>
        }
    })
}

#[component]
async fn settings_panel(s: Settings) -> Result<impl View> {
    Ok(view! {
        <aside class="panel" aria-labelledby="settings-title">
            <h2 id="settings-title" class="label">"Settings"</h2>
            <dl>
                flag(label: "GitHub", on: s.github)
                flag(label: "tmux", on: s.tmux)
                flag(label: "Auto branch", on: s.auto_branch)
                text(label: "Branch template", value: &s.branch_template)
                text(label: "Default branch", value: &s.default_branch)
                text(label: "GitHub project", value: &s.github_project)
            </dl>
        </aside>
    })
}

#[component]
async fn info(org: &Organization, projects: &[Project]) -> Result<impl View> {
    Ok(view! {
        <div class="info">
            <div class="body">
                if !org.description.is_empty() {
                    <div class="desc">(org.description.clone())</div>
                }
                <section aria-labelledby="projects-title">
                    <h2 id="projects-title" class="label">"Projects"</h2>
                    if projects.is_empty() {
                        <p class="none">"No projects."</p>
                    } else {
                        <ul class="rows">
                            for p in projects.iter() {
                                <li>
                                    <a class="u" href=(format!("/project/{}", p.id()))>(p.name.clone())</a>
                                    <span class="path"><bdi>(tilde(&p.base_path))</bdi></span>
                                </li>
                            }
                        </ul>
                    }
                </section>
            </div>
            settings_panel(s: org.settings())
        </div>
    })
}

/// `rows` are `(project name, task)`, the shape `Db::tasks_in` returns.
#[component]
async fn task_table(rows: &[(String, Task)]) -> Result<impl View> {
    Ok(view! {
        if rows.is_empty() {
            <p class="none">"No tasks."</p>
        } else {
            <table class="tasks">
                <caption class="sr">"Tasks in this organization"</caption>
                <thead>
                    <tr><th scope="col">"Task"</th><th scope="col">"Project"</th><th scope="col">"Status"</th><th scope="col">"Issue"</th></tr>
                </thead>
                <tbody>
                    for (project, task) in rows.iter() {
                        <tr>
                            <td><a class="u" href=(format!("/task/{}", task.id()))>(task.name.clone())</a></td>
                            <td><a class="u muted" href=(format!("/project/{}", task.project_id))>(project.clone())</a></td>
                            <td>
                                <span class=(format!("status {}", task.status.as_str()))>
                                    (icon(status_icon(task.status)))
                                    (task.status.label())
                                </span>
                            </td>
                            <td class="mono">(task.github_issue.map(|n| format!("#{n}")).unwrap_or_default())</td>
                        </tr>
                    }
                </tbody>
            </table>
        }
    })
}

// ---- the report tab -------------------------------------------------------

/// Which tab the proposal shows.
#[derive(Clone, Copy, PartialEq)]
pub enum Section {
    Info,
    Tasks,
    Report,
}

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

pub fn load_report(
    db: &Db,
    org_id: i64,
    from: Option<&str>,
    to: Option<&str>,
    now: NaiveDateTime,
) -> crate::error::Result<Report> {
    let (range, error) = period(from, to, now.date());
    let (projects, sessions) = project_reports(db, org_id, range, now)?;
    Ok(Report {
        range,
        // The union across projects, as `iter org info` totals it.
        total: Total::new(total_minutes(&sessions, now)),
        projects,
        sessions: sessions.len(),
        error,
    })
}

fn report_url(base: &str, range: &DateRange) -> String {
    let from = range.from.map(fmt_date).unwrap_or_default();
    format!("{base}?tab=report&from={from}&to={}", fmt_date(range.to))
}

fn plural(n: usize, one: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {one}s")
    }
}

/// Applies the From/To dates without a button. A date picked from the
/// calendar applies at once. A typed one applies on Enter (the form's
/// hidden submit button) or when focus leaves both fields -- not on each
/// keystroke, since Chrome reports a change after every digit of a year
/// ("0002", "0020", ...). Only a four-digit year from 1970 counts as typed.
const REPORT_JS: &str = r#"(function () {
  var form = document.querySelector(".v2 .range");
  if (!form) return;
  var timer, typing = false;
  var ready = function (v) { return v === "" || (/^\d{4}-\d\d-\d\d$/.test(v) && +v.slice(0, 4) >= 1970); };
  var go = function () { clearTimeout(timer); timer = setTimeout(function () { form.requestSubmit(); }, 250); };
  form.addEventListener("keydown", function (e) { if (e.key !== "Enter" && e.key !== "Tab") typing = true; });
  form.addEventListener("change", function (e) {
    clearTimeout(timer);
    if (!typing && ready(e.target.value)) go();
  });
  // Over the calendar icon a click picks the whole date, so the whole date
  // lights up (.whole, v2.css). The icon is the last 14px before the right
  // padding and border; 8px of its left margin count too.
  form.querySelectorAll("input[type=date]").forEach(function (input) {
    input.addEventListener("mousemove", function (e) {
      var r = input.getBoundingClientRect(), cs = getComputedStyle(input);
      var edge = r.right - parseFloat(cs.paddingRight) - parseFloat(cs.borderRightWidth);
      input.classList.toggle("whole", e.clientX >= edge - 14 - 8 && e.clientX <= edge);
    });
    input.addEventListener("mouseleave", function () { input.classList.remove("whole"); });
  });
  form.addEventListener("focusout", function (e) {
    if (!typing || form.contains(e.relatedTarget)) return;
    typing = false;
    var dates = form.querySelectorAll("input[type=date]");
    if (ready(dates[0].value) && ready(dates[1].value)) go();
  });
})();"#;

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

/// Wires the Copy button: shown only where the clipboard is available,
/// it copies `data-copy` and says so for two seconds.
const COPY_JS: &str = r#"(function () {
  var button = document.querySelector(".v2 .copy");
  if (!button || !navigator.clipboard) return;
  var status = document.getElementById("copy-status");
  var timer;
  button.hidden = false;
  button.addEventListener("click", function () {
    navigator.clipboard.writeText(button.dataset.copy).then(function () {
      button.classList.add("copied");
      status.textContent = "Copied to the clipboard";
      clearTimeout(timer);
      timer = setTimeout(function () {
        button.classList.remove("copied");
        status.textContent = "";
      }, 2000);
    }, function () {
      status.textContent = "Could not copy";
    });
  });
})();"#;

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
                    <a href=(report_url(base, range)) if *range == report.range { aria-current="page" }>(label.to_string())</a>
                }
            </nav>
            <form class="range" method="get" action=(base.to_string())>
                <input type="hidden" name="tab" value="report">
                <label>"From" <input type="date" name="from" value=(from.clone())></label>
                <label>"To" <input type="date" name="to" value=(to.clone())></label>
                // Dates apply as they change (`REPORT_JS`). This button is what
                // makes Enter in a date field submit (also without JavaScript);
                // it's hidden and out of the tab order, since Enter reaches it.
                <button type="submit" class="sr" tabindex="-1">"Show report"</button>
            </form>
            <script>(Unescaped::new_unchecked(REPORT_JS))</script>
        </div>
    })
}

#[component]
async fn task_section(t: &TaskReport, dated: bool) -> Result<impl View> {
    Ok(view! {
        <article class="task">
            <header>
                <h3><a class="u" href=(format!("/task/{}", t.id))>(t.name.clone())</a></h3>
                <span class=(format!("status {}", t.status.as_str()))>
                    (icon(status_icon(t.status)))
                    (t.status_label.clone())
                </span>
                <span class="time">(t.total.total_hhmm.clone())</span>
            </header>
            if let Some(d) = &t.description {
                <p class="tdesc">(d.clone())</p>
            }
            <table class="sessions">
                <caption class="sr">(format!("Sessions of {}", t.name))</caption>
                <thead>
                    <tr>
                        if dated {
                            <th scope="col">"Date"</th>
                        }
                        <th scope="col">"Start"</th>
                        <th scope="col">"Duration"</th>
                        <th scope="col">"Note"</th>
                    </tr>
                </thead>
                <tbody>
                    for r in t.sessions.iter() {
                        <tr>
                            if dated {
                                <td class="mono">(r.date.clone().unwrap_or_default())</td>
                            }
                            <td class="mono">(r.start.clone())</td>
                            <td class="mono">
                                (r.duration.clone())
                                // Still open: its duration counts up to now.
                                if r.end.is_none() {
                                    <span class="live" title="Still running"><span class="sr">" (still running)"</span></span>
                                }
                            </td>
                            <td class="note">(r.message.clone().unwrap_or_default())</td>
                        </tr>
                    }
                </tbody>
            </table>
        </article>
    })
}

#[component]
async fn report_view(base: &str, report: &Report, today: NaiveDate) -> Result<impl View> {
    let dated = report.range.single_day().is_none();
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
            <p class="notice" role="alert">(e.clone())</p>
        }
        <div class="summary">
            <span class="big">(report.total.total_hhmm.clone())</span>
            <span>
                (plural(report.projects.len(), "project"))
                " · " (plural(tasks, "task"))
                " · " (plural(report.sessions, "session"))
            </span>
            if !report.projects.is_empty() {
                // Hidden until `COPY_JS` finds a clipboard to write to.
                <button type="button" class="copy" data-copy=(status_list(report)) hidden="">
                    <span class="idle">(icon(COPY)) "Copy summary"</span>
                    <span class="done">(icon(CLIPBOARD_CHECK)) "Copied"</span>
                </button>
                <span class="sr" id="copy-status" role="status"></span>
                <script>(Unescaped::new_unchecked(COPY_JS))</script>
            }
        </div>
        if report.projects.is_empty() {
            <p class="none">"No sessions in this period."</p>
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
                                <h2 id=(format!("p-{}-title", p.id))><a class="u" href=(format!("/project/{}", p.id))>(p.name.clone())</a></h2>
                                <span class="time">(p.total.total_hhmm.clone())</span>
                            </header>
                            if let Some(d) = &p.description {
                                <p class="pdesc">(d.clone())</p>
                            }
                            for t in p.tasks.iter() {
                                task_section(t: t, dated: dated)
                            }
                        </section>
                    }
                </div>
            </div>
        }
    })
}

#[component]
pub async fn proposal(
    org: &Organization,
    projects: &[Project],
    tasks: &[(String, Task)],
    section: Section,
    report: &Option<Report>,
    today: NaiveDate,
) -> Result<impl View> {
    let base = format!("/org/{}", org.id());
    let seven = report_url(&base, &last_days(today, 7));
    Ok(view! {
        frame(
            css: Some("/org.css"),
            <div class="head">
                <div>
                    <p class="label">"Organization"</p>
                    <h1>(org.name.clone())</h1>
                </div>
                <a class="button" href=(format!("{base}/edit"))>(icon(PENCIL)) "Edit"</a>
            </div>
            <nav class="tabs" aria-label="Sections">
                <a href=(base.clone()) if section == Section::Info { aria-current="page" }>"Info"</a>
                <a href=(format!("{base}?tab=tasks")) if section == Section::Tasks { aria-current="page" }>
                    "Tasks" <span class="count">(tasks.len().to_string())</span>
                </a>
                <a href=(seven.clone()) if section == Section::Report { aria-current="page" }>"Report"</a>
            </nav>
            if section == Section::Tasks {
                task_table(rows: tasks)
            } else if let Some(r) = report {
                report_view(base: &base, report: r, today: today)
            } else {
                info(org: org, projects: projects)
            }
        )
    })
}
