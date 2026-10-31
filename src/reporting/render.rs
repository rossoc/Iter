//! The Markdown writer behind a text report.

use super::*;

/// A text report: the header's scalars as fenced YAML front matter, then
/// the markdown body -- the same shape `md_edit` opens a
/// project/task/organization in, so a report reads like the thing it
/// reports on
pub(super) fn document(header: &Header, md: Md) -> Result<String> {
    let mut front = header.clone();
    front.description = None; // it's the body below the front matter instead
    let mut out = String::from("---\n");
    out.push_str(&serde_yaml::to_string(&front)?);
    out.push_str("---\n\n");
    out.push_str(&md.out);
    Ok(out)
}

/// The line under a report's title: the period it covers and the time in
/// it. The period goes here rather than in the heading so the title stays
/// just the name of the thing.
pub(super) fn summary_line(header: &Header) -> String {
    format!("{} -- {}", header.period.label(), header.total.spent())
}

/// Trimmed text, or `None` when there's nothing left -- the one rule for
/// "empty means absent" that every optional description and message in a
/// report goes through.
pub fn non_empty(text: &str) -> Option<String> {
    Some(text.trim())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

/// A markdown cell: pipes escaped and newlines flattened, so a typed note
/// can't break the table it's printed in.
pub(super) fn cell(text: &str) -> String {
    text.replace('|', "\\|")
        .replace(['\n', '\r'], " ")
        .trim()
        .to_string()
}

/// The messages recorded on `rows`' closing events, as a bulleted,
/// copy-paste-ready list.
pub(super) fn message_bullets(rows: &[SessionRow]) -> Option<String> {
    let messages: Vec<String> = rows
        .iter()
        .filter_map(|r| r.message.as_deref())
        .map(|m| format!("- {}", m.trim_start_matches('-').trim_start()))
        .collect();
    if messages.is_empty() {
        None
    } else {
        Some(messages.join("\n"))
    }
}

/// One `| a | b |` row, every cell padded to its column's width. Generic
/// over the cell type so a `&[&str]` header row and a `&[String]` body row
/// go through the same code without either being copied into the other's
/// shape first.
pub(super) fn row_line<S: AsRef<str>>(widths: &[usize], cells: &[S]) -> String {
    let mut out = String::from("|");
    for (i, width) in widths.iter().enumerate() {
        let cell = cells.get(i).map_or("", AsRef::as_ref);
        let _ = write!(out, " {cell:<width$} |");
    }
    out
}

/// A markdown buffer that keeps exactly one blank line between blocks, so
/// the callers above can just append sections without tracking spacing.
pub struct Md {
    pub(super) out: String,
}

impl Md {
    pub(super) fn new() -> Self {
        Md { out: String::new() }
    }

    pub(super) fn block(&mut self, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        if !self.out.is_empty() {
            self.out.push('\n');
        }
        self.out.push_str(text.trim_end_matches('\n'));
        self.out.push('\n');
    }

    pub(super) fn heading(&mut self, level: usize, text: &str) {
        self.block(&format!("{} {}", "#".repeat(level), text));
    }

    /// A pipe table with every column padded to its widest cell -- the
    /// point of the text format is being read as-is in a terminal, where
    /// ragged columns are the hard part.
    pub(super) fn table(&mut self, headers: &[&str], rows: &[Vec<String>]) {
        if rows.is_empty() {
            return;
        }
        let widths: Vec<usize> = headers
            .iter()
            .enumerate()
            .map(|(i, h)| {
                rows.iter()
                    .filter_map(|r| r.get(i))
                    .map(|c| c.chars().count())
                    .chain(std::iter::once(h.chars().count()))
                    .max()
                    .unwrap_or(0)
            })
            .collect();

        let mut table = row_line(&widths, headers);
        table.push_str("\n|");
        for width in &widths {
            let _ = write!(table, "{}|", "-".repeat(width + 2));
        }
        for row in rows {
            table.push('\n');
            table.push_str(&row_line(&widths, row));
        }
        self.block(&table);
    }

    /// A project's tasks: the summary table, then a section per task. The
    /// tail shared by a project report (which puts it at the top level) and
    /// a project's section of an organization report (one level deeper).
    pub(super) fn task_breakdown(&mut self, tasks: &[TaskReport], level: usize) {
        self.task_summary(tasks);
        for task in tasks {
            self.task_section(task, level);
        }
    }

    /// One project's section of an organization report -- the mirror of
    /// [`Self::task_section`] one level up.
    pub(super) fn project_section(&mut self, project: &ProjectReport, level: usize) {
        self.heading(level, &project.name);
        self.block(&project.total.spent());
        if let Some(description) = &project.description {
            self.block(description);
        }
        self.task_breakdown(&project.tasks, level + 1);
    }

    /// The "how much time went into each task" table under a project.
    pub(super) fn task_summary(&mut self, tasks: &[TaskReport]) {
        if tasks.is_empty() {
            self.block(NO_SESSIONS);
            return;
        }
        self.table(
            &["Task", "Status", "Time"],
            &tasks
                .iter()
                .map(|t| {
                    vec![
                        cell(&t.name),
                        t.status_label.clone(),
                        t.total.total_hhmm.clone(),
                    ]
                })
                .collect::<Vec<_>>(),
        );
    }

    pub(super) fn task_section(&mut self, task: &TaskReport, level: usize) {
        self.heading(level, &format!("{} -- {}", task.name, task.status_label));
        self.block(&task.total.spent());
        if let Some(description) = &task.description {
            self.block(description);
        }
        if let Some(messages) = message_bullets(&task.sessions) {
            self.block(&messages);
        }
        self.session_table(&task.sessions);
    }

    pub(super) fn session_table(&mut self, rows: &[SessionRow]) {
        if rows.is_empty() {
            self.block(NO_SESSIONS);
            return;
        }
        // A report over one day would repeat that date on every row, so
        // `session_rows` leaves it off and the column goes with it.
        let dated = rows.iter().any(|r| r.date.is_some());
        let headers: &[&str] = match dated {
            true => &["Date", "Start", "End", "Duration", "Message"],
            false => &["Start", "End", "Duration", "Message"],
        };

        let body: Vec<Vec<String>> = rows
            .iter()
            .map(|r| {
                let mut row = Vec::new();
                if dated {
                    row.push(r.date.clone().unwrap_or_default());
                }
                row.push(r.start.clone());
                row.push(r.end.clone().unwrap_or_else(|| "--".to_string()));
                row.push(r.duration.clone());
                row.push(match (&r.message, &r.end) {
                    (Some(m), _) => cell(m),
                    (None, None) => "(ongoing)".to_string(),
                    (None, Some(_)) => String::new(),
                });
                row
            })
            .collect();
        self.table(headers, &body);
    }
}
