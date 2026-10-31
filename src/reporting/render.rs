//! The Markdown writer behind a text report.

use super::*;

/// A text report: header scalars as fenced YAML front matter, then the markdown body (the shape `md_edit` opens things in).
pub(super) fn document(header: &Header, md: Md) -> Result<String> {
    let mut front = header.clone();
    front.description = None; // it's the body below the front matter instead
    let mut out = String::from("---\n");
    out.push_str(&serde_yaml::to_string(&front)?);
    out.push_str("---\n\n");
    out.push_str(&md.out);
    Ok(out)
}

/// The line under the title: the period and the time in it.
pub(super) fn summary_line(header: &Header) -> String {
    format!("{} -- {}", header.period.label(), header.total.spent())
}

/// Trimmed text, or `None` when empty: the one "empty means absent" rule.
pub fn non_empty(text: &str) -> Option<String> {
    Some(text.trim())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

/// A markdown cell: pipes escaped, newlines flattened.
pub(super) fn cell(text: &str) -> String {
    text.replace('|', "\\|")
        .replace(['\n', '\r'], " ")
        .trim()
        .to_string()
}

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

/// One `| a | b |` row padded to the column widths; generic so header and body rows share the code.
pub(super) fn row_line<S: AsRef<str>>(widths: &[usize], cells: &[S]) -> String {
    let mut out = String::from("|");
    for (i, width) in widths.iter().enumerate() {
        let cell = cells.get(i).map_or("", AsRef::as_ref);
        let _ = write!(out, " {cell:<width$} |");
    }
    out
}

/// A markdown buffer keeping exactly one blank line between blocks.
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

    /// A pipe table with columns padded to the widest cell, so it reads as-is in a terminal.
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

    /// A project's tasks: summary table, then a section per task; shared by project and organization reports.
    pub(super) fn task_breakdown(&mut self, tasks: &[TaskReport], level: usize) {
        self.task_summary(tasks);
        for task in tasks {
            self.task_section(task, level);
        }
    }

    pub(super) fn project_section(&mut self, project: &ProjectReport, level: usize) {
        self.heading(level, &project.name);
        self.block(&project.total.spent());
        if let Some(description) = &project.description {
            self.block(description);
        }
        self.task_breakdown(&project.tasks, level + 1);
    }

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
        // a one-day report would repeat the date on every row: `session_rows` leaves it off and the column goes
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
