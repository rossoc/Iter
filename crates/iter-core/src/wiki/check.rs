//! `check`: the lint that needs no LLM; contradictions between pages are the agent's lint.

use super::graph::{Wiki, md_files};
use super::ontology::{LevelKind, local, ns};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, Timelike};
use oxigraph::model::NamedNode;
use oxigraph::model::vocab::rdfs;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

#[derive(Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// The wiki is wrong: the check fails.
    Error,
    /// Something is missing or out of place.
    Warning,
    /// Something to do next.
    Info,
}

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Issue {
    pub severity: Severity,
    pub code: &'static str,
    pub id: String,
    pub message: String,
}

impl Issue {
    fn new(
        severity: Severity,
        code: &'static str,
        id: impl Into<String>,
        message: impl Into<String>,
    ) -> Issue {
        Issue {
            severity,
            code,
            id: id.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Dirty {
    New,
    Changed,
}

/// `updated` as a moment: a date means the end of that day, so a note written during it is not newer than a page updated that day.
pub(super) fn parse_updated(text: &str) -> Option<NaiveDateTime> {
    let text = text.trim();
    if let Ok(t) = DateTime::parse_from_rfc3339(text) {
        return Some(t.with_timezone(&Local).naive_local());
    }
    for format in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(t) = NaiveDateTime::parse_from_str(text, format) {
            return Some(t);
        }
    }
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .ok()?
        .and_hms_opt(23, 59, 59)
}

/// The pages naming `note` as a `source`.
fn made_from(wiki: &Wiki, note: usize) -> Vec<usize> {
    wiki.edges_out(note)
        .into_iter()
        .filter(|(prop, _)| prop == "sourceOf")
        .map(|(_, page)| page)
        .collect()
}

/// Notes no page has been made from since they last changed: file mtime against the `updated` of the pages naming it as source. No hash, no ledger.
pub fn dirty_notes(wiki: &Wiki) -> Vec<(usize, Dirty)> {
    let mut out = Vec::new();
    for (i, node) in wiki.nodes.iter().enumerate() {
        if wiki.ontology.pages.contains(&node.class) {
            continue;
        }
        let made = made_from(wiki, i);
        if made.is_empty() {
            out.push((i, Dirty::New));
            continue;
        }
        let ingested = made
            .iter()
            .filter_map(|&p| parse_updated(wiki.nodes[p].front.as_ref()?.updated.as_deref()?))
            .max();
        let modified = node
            .modified
            // `updated` has whole seconds: so does the note's time
            .and_then(|m| DateTime::<Local>::from(m).naive_local().with_nanosecond(0));
        match (modified, ingested) {
            (Some(m), Some(u)) if m > u => out.push((i, Dirty::Changed)),
            (_, None) => out.push((i, Dirty::Changed)),
            _ => {}
        }
    }
    out
}

/// Lints the wiki; `from` is the level ids are written from. Errors sort first.
pub fn check(wiki: &Wiki, from: usize) -> Vec<Issue> {
    let mut issues: Vec<Issue> = (0..wiki.nodes.len())
        .flat_map(|i| page_issues(wiki, from, i))
        .collect();
    issues.extend(wiki.dangling.iter().map(|d| {
        let message = format!("{} -> `{}` does not exist", d.via, d.target);
        Issue::new(Severity::Error, "dangling", wiki.id(d.from, from), message)
    }));
    issues.extend(unplaced(wiki));
    issues.extend(dirty_notes(wiki).into_iter().map(|(n, why)| {
        let message = match why {
            Dirty::New => "not ingested yet",
            Dirty::Changed => "changed since it was ingested",
        };
        Issue::new(Severity::Info, "dirty-note", wiki.id(n, from), message)
    }));
    issues.extend(promotable(wiki, from));
    issues.sort();
    issues
}

/// What is wrong in one page: its front matter, its relations and links, its isolation.
fn page_issues(wiki: &Wiki, from: usize, i: usize) -> Vec<Issue> {
    let (onto, node) = (&wiki.ontology, &wiki.nodes[i]);
    if !onto.pages.contains(&node.class) {
        return Vec::new();
    }
    let id = wiki.id(i, from);
    let mut out = Vec::new();
    let mut add =
        |severity, code, message: String| out.push(Issue::new(severity, code, &id, message));
    if let Some(e) = &node.front_error {
        add(Severity::Error, "front-matter", e.clone());
    }
    // a sketch is the agent's working memory: it only has to parse
    let strict = !onto.subclasses_of(&ns("Sketch")).contains(&node.class);
    match &node.front {
        None if node.front_error.is_none() && strict => add(
            Severity::Warning,
            "no-front-matter",
            "no front matter: the page has no description and no tags".into(),
        ),
        Some(front) => {
            if strict && front.description.as_deref().is_none_or(str::is_empty) {
                add(
                    Severity::Warning,
                    "missing-description",
                    "no description: retrieval cannot say what it is".into(),
                );
            }
            if strict && front.tags.is_empty() {
                add(Severity::Warning, "missing-tags", "no tags".into());
            }
            for (key, value) in front.scalars() {
                if let (Some(value), Some(allowed)) = (value, onto.allowed_values(&ns(key)))
                    && !allowed.iter().any(|a| a == value)
                {
                    let message =
                        format!("`{key}` is `{value}`: it is one of {}", allowed.join(", "));
                    add(Severity::Error, "bad-value", message);
                }
            }
            for key in &front.unknown {
                add(
                    Severity::Warning,
                    "unknown-key",
                    format!("`{key}` is not a key the ontology knows"),
                );
            }
        }
        None => {}
    }
    for (_, prop, target) in node.written(onto) {
        if let Some(t) = wiki.resolve(node.level, target)
            && let Some(m) = violation(wiki, prop, i, t)
        {
            add(Severity::Error, "constraint", m);
        }
    }
    if strict && wiki.edges_out(i).is_empty() {
        add(
            Severity::Warning,
            "orphan",
            "nothing relates to it and it relates to nothing".into(),
        );
    }
    out
}

/// Files under a level's `wiki/` that no class owns.
fn unplaced(wiki: &Wiki) -> Vec<Issue> {
    let claimed: HashSet<&Path> = wiki.nodes.iter().map(|n| n.path.as_path()).collect();
    let mut out = Vec::new();
    for level in wiki.levels.iter().filter(|l| l.kind != LevelKind::Task) {
        for path in md_files(&level.dir.join("wiki")) {
            if !claimed.contains(path.as_path()) {
                let rel = path.strip_prefix(&level.dir).unwrap_or(&path);
                out.push(Issue::new(
                    Severity::Warning,
                    "unplaced",
                    format!("{}:{}", level.label, rel.display()),
                    "in a folder no class owns, so it is not part of the wiki",
                ));
            }
        }
    }
    out
}

/// Three ideas under one tag and no concept built from any: time to build one.
fn promotable(wiki: &Wiki, from: usize) -> Vec<Issue> {
    let (ideas, concepts) = (ns("Idea"), wiki.ontology.subclasses_of(&ns("Concept")));
    let mut by_tag: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, node) in wiki.nodes.iter().enumerate() {
        if let (true, Some(front)) = (node.class == ideas, &node.front) {
            for tag in &front.tags {
                by_tag.entry(tag).or_default().push(i);
            }
        }
    }
    by_tag
        .into_iter()
        .filter(|(_, members)| members.len() >= 3)
        .filter(|(_, members)| {
            !members.iter().any(|&m| {
                (wiki.edges_out(m).iter())
                    .any(|(p, o)| p == "sourceOf" && concepts.contains(&wiki.nodes[*o].class))
            })
        })
        .map(|(tag, members)| {
            let message = format!(
                "{} ideas are tagged `{tag}` and no concept is built from any of them",
                members.len()
            );
            Issue::new(
                Severity::Info,
                "promote",
                wiki.id(members[0], from),
                message,
            )
        })
        .collect()
}

/// Why `prop` may not link `s` to `t`: a domain or range the ontology gives it that their classes do not fit.
fn violation(wiki: &Wiki, prop: NamedNode, s: usize, t: usize) -> Option<String> {
    let onto = &wiki.ontology;
    let fits = |node: usize, wanted: Vec<NamedNode>| {
        wanted.is_empty()
            || (onto.superclasses_of(&wiki.nodes[node].class))
                .iter()
                .any(|c| wanted.contains(c))
    };
    if !fits(s, onto.constraint(&prop, rdfs::DOMAIN)) {
        let class = local(&wiki.nodes[s].class);
        return Some(format!("a {class} cannot have `{}`", local(&prop)));
    }
    if !fits(t, onto.constraint(&prop, rdfs::RANGE)) {
        let target = &wiki.nodes[t];
        let (prop, class) = (local(&prop), local(&target.class));
        return Some(format!(
            "`{prop}` cannot point at a {class} (`{}`)",
            target.rel
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::testutil::*;

    fn codes(issues: &[Issue]) -> Vec<(&'static str, &str)> {
        issues.iter().map(|i| (i.code, i.id.as_str())).collect()
    }

    #[test]
    fn the_fixture_has_a_dangling_link_and_pages_without_tags() {
        let w = build(&fixture("check"));
        let issues = check(&w, 1);
        let got = codes(&issues);
        assert!(got.contains(&("dangling", "wiki/entities/ci")), "{got:?}");
        assert!(got.contains(&("missing-tags", "wiki/concepts/backoff")));
        assert!(!got.iter().any(|(_, id)| id.contains("sketches")));
        assert_eq!(issues[0].severity, Severity::Error);
    }

    #[test]
    fn a_note_is_dirty_until_a_page_updated_after_it_names_it() {
        let w = build(&fixture("dirty"));
        assert_eq!(dirty_notes(&w), [(note_of(&w), Dirty::Changed)]);

        let root = fixture("clean");
        write(
            &root.join("acme/app"),
            "wiki/ideas/retry.md",
            "---\ndescription: a\ntags: [net]\nsource: 2026-10-27\nupdated: 2999-01-01\n---\n",
        );
        assert!(dirty_notes(&build(&root)).is_empty());

        let root = fixture("new");
        write(&root.join("acme/app"), "2026-10-29.md", "unsourced");
        let w = build(&root);
        assert!(
            dirty_notes(&w)
                .iter()
                .any(|(n, d)| w.nodes[*n].rel == "2026-10-29" && *d == Dirty::New)
        );
    }

    fn note_of(w: &Wiki) -> usize {
        w.nodes
            .iter()
            .position(|n| n.rel == "2026-10-27")
            .expect("note")
    }

    #[test]
    fn updated_dates_are_the_end_of_the_day() {
        let day = parse_updated("2026-10-28").expect("date");
        assert_eq!(day.to_string(), "2026-10-28 23:59:59");
        assert!(parse_updated("2026-10-28T10:00:00").is_some());
        assert!(parse_updated("2026-10-28T10:00:00+02:00").is_some());
        assert_eq!(parse_updated("yesterday"), None);
    }

    #[test]
    fn the_ontology_constrains_relations() {
        let root = fixture("constraint");
        write(
            &root.join("acme/app"),
            "wiki/concepts/bad.md",
            "---\ndescription: a\ntags: [x]\ninclude: [2026-10-27]\n---\n",
        );
        let w = build(&root);
        let issues = check(&w, 1);
        let bad = issues
            .iter()
            .find(|i| i.code == "constraint")
            .expect("constraint");
        assert_eq!(bad.id, "wiki/concepts/bad");
        assert!(
            bad.message.contains("cannot point at a Note"),
            "{}",
            bad.message
        );
    }

    #[test]
    fn a_value_outside_the_ontologys_enumeration_is_an_error() {
        let root = fixture("enum");
        let app = root.join("acme/app");
        write(
            &app,
            "wiki/ideas/sure.md",
            "---\ndescription: a\ntags: [x]\nconfidence: certain\n---\n",
        );
        write(
            &app,
            "wiki/ideas/fine.md",
            "---\ndescription: a\ntags: [x]\nconfidence: low\n---\n",
        );
        let issues = check(&build(&root), 1);
        let bad: Vec<_> = issues.iter().filter(|i| i.code == "bad-value").collect();
        assert_eq!(bad.len(), 1);
        assert_eq!(bad[0].id, "wiki/ideas/sure");
        assert_eq!(
            bad[0].message,
            "`confidence` is `certain`: it is one of high, medium, low"
        );
    }

    #[test]
    fn unknown_keys_broken_front_matter_and_strays_are_reported() {
        let root = fixture("strays");
        let app = root.join("acme/app");
        write(
            &app,
            "wiki/ideas/mood.md",
            "---\ndescription: a\ntags: [x]\nmood: sad\n---\n",
        );
        write(&app, "wiki/ideas/bad.md", "---\ndescription: [a]\n---\n");
        write(&app, "wiki/misc/x.md", "stray");
        write(&app, "wiki/ideas/none.md", "no front matter");
        let got_issues = check(&build(&root), 1);
        let got = codes(&got_issues);
        for want in [
            ("unknown-key", "wiki/ideas/mood"),
            ("front-matter", "wiki/ideas/bad"),
            ("unplaced", "acme.app:wiki/misc/x.md"),
            ("no-front-matter", "wiki/ideas/none"),
        ] {
            assert!(got.contains(&want), "{want:?} in {got:?}");
        }
    }

    #[test]
    fn three_ideas_under_a_tag_without_a_concept_are_a_suggestion() {
        let root = fixture("promote");
        let app = root.join("acme/app");
        for n in 1..=3 {
            write(
                &app,
                &format!("wiki/ideas/i{n}.md"),
                "---\ndescription: a\ntags: [cache]\nsource: 2026-10-27\n---\n",
            );
        }
        let issues = check(&build(&root), 1);
        let promote: Vec<_> = issues.iter().filter(|i| i.code == "promote").collect();
        assert_eq!(promote.len(), 1);
        assert!(promote[0].message.starts_with("3 ideas are tagged `cache`"));
        write(
            &app,
            "wiki/concepts/cache.md",
            "---\ndescription: a\ntags: [cache]\nsource: [ideas/i1]\n---\n",
        );
        let issues = check(&build(&root), 1);
        assert!(!issues.iter().any(|i| i.code == "promote"));
    }
}
