mod args;
mod csv_log;
mod session_log;
mod sessions;

use args::{Args, Command, SessionCommand};
use chrono::{Local, NaiveDate};
use clap::{CommandFactory, Parser};
use clap_complete::engine::CompletionCandidate;
use clap_complete::env::CompleteEnv;
use csv_log::CSVLog;
use session_log::SessionLog;
use sessions::{
    SessionDetailReport, SessionRecord, SessionWeekdayReport, concat_messages, distinct_names,
    merged_total_minutes, minutes_to_hhmm, round_to_half_hour, weekday_averages,
};

fn cli() -> clap::Command {
    Args::command().name("iter")
}

const LOG_FILE: &str = "/home/local/.config/programmini/buff/log.csv";

/// A gap between one session's end and the next session's begin shorter than
/// this many minutes is treated as a pause within one continuous span (e.g.
/// a quick interruption) rather than a real break between sessions.
pub const MERGE_GAP_MINUTES: i64 = 17;

fn main() {
    // Shell-driven dynamic completion: when invoked as `COMPLETE=<shell> iter
    // ...` (which the shell does behind the scenes on every Tab press, once
    // `source <(COMPLETE=zsh iter)` has registered it)
    CompleteEnv::with_factory(cli).complete();

    let args = Args::parse();

    match &args.command {
        Command::Begin { value } => begin(value),
        Command::End { value, message } => end(value, message.as_deref()),
        Command::Info { value, date } => info(value, date.as_deref()),
        Command::Elapsed { value } => elapsed(value),
        Command::List => list_cmd(),
        Command::Session { action } => match action {
            SessionCommand::Weekday { value } => session_weekday_cmd(value),
        },
    }
}

fn begin(value: &str) {
    let log = CSVLog::new(LOG_FILE);
    log.begin(value);
}

fn end(value: &str, message: Option<&str>) {
    let log = CSVLog::new(LOG_FILE);
    log.end_with_message(value, message);
}

fn elapsed(value: &str) {
    let log = CSVLog::new(LOG_FILE);
    log.elapsed(value);
}

fn read_log() -> Vec<SessionRecord> {
    CSVLog::new(LOG_FILE).select()
}

/// Dynamic completer for every `<VALUE>` positional that names a session
/// (`begin`, `end`, `info`, `elapsed`, `session weekday`).
/// Attached via `#[arg(add = ArgValueCompleter::new(...))]` in `args.rs`.
pub(crate) fn session_name_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    distinct_names(&read_log())
        .into_iter()
        .filter(|name| name.starts_with(current))
        .map(CompletionCandidate::new)
        .collect()
}

fn list_cmd() {
    let records = read_log();
    let names = distinct_names(&records);
    print!(
        "{}",
        serde_yaml::to_string(&names).expect("yaml serialization failed")
    );
}

fn info(name: &str, date_filter: Option<&str>) {
    let now = Local::now().naive_local();

    let date = match date_filter {
        Some(d) => match NaiveDate::parse_from_str(d, "%Y-%m-%d") {
            Ok(date) => date,
            Err(_) => {
                eprintln!("Invalid --date value '{}', expected YYYY-MM-DD", d);
                return;
            }
        },
        _ => now.date(),
    };

    let records: Vec<_> = read_log()
        .into_iter()
        .filter(|r| r.name == name && r.begin.date() == date)
        .collect();

    let total_minutes = merged_total_minutes(&records, now, MERGE_GAP_MINUTES);
    let report = SessionDetailReport {
        name: name.to_string(),
        date: date.format("%Y-%m-%d").to_string(),
        total_hours: round_to_half_hour(total_minutes as f64 / 60.0),
        total_hhmm: minutes_to_hhmm(total_minutes),
        messages: concat_messages(&records),
    };
    print!("{}", format_report(&report));
}

/// Renders `report` as YAML, appending `messages` by hand right after the
/// `messages:` key instead of letting serde_yaml serialize it as a normal
/// string. A plain scalar gets wrapped in single quotes (and any `'` inside
/// doubled to `''`) the moment it starts with `-` or contains a `'` —
/// exactly what a typed note tends to do — which breaks copy-pasting a
/// message straight back out of the terminal.
fn format_report(report: &SessionDetailReport) -> String {
    let mut out = serde_yaml::to_string(report).expect("yaml serialization failed");
    match &report.messages {
        Some(m) => {
            out.push_str("messages:\n");
            out.push_str(m);
            out.push('\n');
        }
        _ => out.push_str("messages: null\n"),
    }
    out
}

fn session_weekday_cmd(name: &str) {
    let now = Local::now().naive_local();
    let records: Vec<_> = read_log().into_iter().filter(|r| r.name == name).collect();
    let report = SessionWeekdayReport {
        name: name.to_string(),
        weekdays: weekday_averages(&records, now),
    };
    print!(
        "{}",
        serde_yaml::to_string(&report).expect("yaml serialization failed")
    );
}
