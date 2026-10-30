//! `iter wiki`: reading the wiki the notes are the source of. Each command
//! builds the graph from the files (see `crate::wiki`) and answers; none keeps
//! anything between runs.

use crate::app::App;
use crate::args::{WikiCommand, WikiScope};
use crate::commands::Run;
use crate::error::{IterError, Result};
use crate::utils::output::emit;
use crate::wiki::check::{self, Severity};
use crate::wiki::graph::{Answer, Wiki};
use crate::wiki::retrieve::{self, Shown};
use crate::wiki::scope::{self, Spec};
use crate::wiki::{mv, schema};
use std::io::Read;

fn spec(s: &WikiScope) -> Spec {
    Spec {
        task: s.task.clone(),
        project: s.project.clone(),
        org: s.org.clone(),
        down: s.down,
    }
}

fn load(app: &App, s: &WikiScope) -> Result<Wiki> {
    scope::wiki(&app.db, &spec(s))
}

/// The page `id` names, written from the innermost level.
fn page(wiki: &Wiki, id: &str) -> Result<usize> {
    wiki.node(0, id).ok_or_else(|| IterError::NotFound {
        kind: "wiki page",
        name: id.to_string(),
    })
}

impl Run for WikiCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::Find {
                words,
                budget,
                scope,
            } => {
                let wiki = load(app, scope)?;
                let found = retrieve::find(&wiki, 0, &words.join(" "), *budget);
                emit(scope.format, &found, || find_text(&found))
            }
            Self::Grep {
                pattern,
                notes,
                case,
                budget,
                scope,
            } => {
                let wiki = load(app, scope)?;
                let found = retrieve::grep(&wiki, 0, pattern, *notes, *case, *budget)?;
                emit(scope.format, &found, || {
                    let mut out = String::new();
                    for m in &found.matches {
                        out.push_str(&format!("{}:{}: {}\n", m.id, m.line, m.text));
                    }
                    if found.matches.is_empty() {
                        out.push_str("No matches.\n");
                    }
                    if found.left_out > 0 {
                        out.push_str(&format!("... {} more (raise --budget)\n", found.left_out));
                    }
                    out
                })
            }
            Self::Show { id, depth, scope } => {
                let wiki = load(app, scope)?;
                let shown = retrieve::show(&wiki, 0, page(&wiki, id)?, *depth)?;
                emit(scope.format, &shown, || show_text(&shown))
            }
            Self::Neighbors {
                id,
                hops,
                prop,
                scope,
            } => {
                let wiki = load(app, scope)?;
                let near = retrieve::neighbors(&wiki, 0, page(&wiki, id)?, *hops, prop);
                emit(scope.format, &near, || {
                    let mut out = String::new();
                    for n in &near {
                        let said = n.abstract_.as_deref().unwrap_or("");
                        out.push_str(&format!(
                            "{} {:<13} {}  [{}]  {said}\n",
                            n.hop,
                            n.via.join(","),
                            n.id,
                            n.class
                        ));
                    }
                    if near.is_empty() {
                        out.push_str("Nothing related.\n");
                    }
                    out
                })
            }
            Self::Check { scope } => {
                let wiki = load(app, scope)?;
                let issues = check::check(&wiki, 0);
                let count = |s| issues.iter().filter(|i| i.severity == s).count();
                let (errors, warnings, infos) = (
                    count(Severity::Error),
                    count(Severity::Warning),
                    count(Severity::Info),
                );
                emit(scope.format, &issues, || {
                    let mut out = String::new();
                    for i in &issues {
                        let severity = format!("{:?}", i.severity).to_lowercase();
                        out.push_str(&format!(
                            "{severity:<8}{:<16} {}: {}\n",
                            i.code, i.id, i.message
                        ));
                    }
                    out.push_str(&format!(
                        "{errors} errors, {warnings} warnings, {infos} to do\n"
                    ));
                    out
                })?;
                match errors {
                    0 => Ok(()),
                    n => Err(IterError::CommandFailed(format!(
                        "the wiki has {n} error{}",
                        if n == 1 { "" } else { "s" }
                    ))),
                }
            }
            Self::Mv {
                from,
                to,
                dry_run,
                scope,
            } => {
                let wiki = load(app, scope)?;
                let moved = mv::mv(&wiki, page(&wiki, from)?, to, *dry_run)?;
                emit(scope.format, &moved, || {
                    let verb = if *dry_run { "would move" } else { "moved" };
                    let mut out = format!("{verb} {} -> {}\n", moved.from, moved.to);
                    for r in &moved.rewrites {
                        out.push_str(&format!("  {} reference(s) in {}\n", r.references, r.id));
                    }
                    out
                })
            }
            Self::Sparql { query, scope } => {
                let wiki = load(app, scope)?;
                let text = match query.as_str() {
                    "-" => {
                        let mut buf = String::new();
                        std::io::stdin().read_to_string(&mut buf)?;
                        buf
                    }
                    q => q.to_string(),
                };
                let answer = wiki.query(&text)?;
                let value = match &answer {
                    Answer::Table(t) => serde_json::json!({ "vars": t.vars, "rows": t.rows }),
                    Answer::Boolean(b) => serde_json::json!({ "boolean": b }),
                    Answer::Triples(t) => serde_json::json!({ "ntriples": t }),
                };
                emit(scope.format, &value, || answer_text(&answer))
            }
            Self::Export { scope } => {
                print!("{}", load(app, scope)?.ntriples()?);
                Ok(())
            }
            Self::Schema { scope } => {
                let levels = scope::levels(&app.db, &spec(scope))?;
                println!("{}", schema::render(&scope::ontology(&levels)?).trim_end());
                Ok(())
            }
        }
    }
}

fn find_text(found: &retrieve::Found) -> String {
    let mut out = String::new();
    for h in &found.hits {
        out.push_str(&format!("{}  [{}]", h.id, h.class));
        if let Some(a) = &h.abstract_ {
            out.push_str(&format!("  {a}"));
        }
        if !h.tags.is_empty() {
            out.push_str(&format!("  #{}", h.tags.join(" #")));
        }
        out.push_str(&format!("  ({})\n", h.why.join(", ")));
    }
    if found.hits.is_empty() {
        out.push_str("No matches.\n");
    }
    if found.left_out > 0 {
        out.push_str(&format!("... {} more (raise --budget)\n", found.left_out));
    }
    out
}

fn show_text(s: &Shown) -> String {
    let mut out = format!("{}  [{}]\n{}\n", s.id, s.class, s.title);
    let mut line = |key: &str, value: Option<&str>| {
        if let Some(v) = value {
            out.push_str(&format!("{key}: {v}\n"));
        }
    };
    line("abstract", s.abstract_.as_deref());
    line(
        "tags",
        Some(s.tags.join(", ").as_str()).filter(|t| !t.is_empty()),
    );
    line("confidence", s.confidence.as_deref());
    line("updated", s.updated.as_deref());
    line("path", Some(&s.path));
    if let Some(relations) = &s.relations {
        out.push_str("relations:\n");
        for r in relations {
            out.push_str(&format!("  {:<13} {}\n", r.prop, r.id));
        }
    }
    if let Some(outline) = &s.outline {
        out.push_str("outline:\n");
        for h in outline {
            out.push_str(&format!("  {h}\n"));
        }
    }
    if let Some(content) = &s.content {
        out.push_str(&format!("\n{content}"));
    }
    out
}

fn answer_text(answer: &Answer) -> String {
    match answer {
        Answer::Table(t) => {
            let mut out = format!("{}\n", t.vars.join("\t"));
            for row in &t.rows {
                let cells: Vec<&str> = row.iter().map(|c| c.as_deref().unwrap_or("")).collect();
                out.push_str(&format!("{}\n", cells.join("\t")));
            }
            out
        }
        Answer::Boolean(b) => format!("{b}\n"),
        Answer::Triples(t) => t.clone(),
    }
}
