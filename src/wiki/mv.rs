//! `mv`: renames a file inside its level and rewrites every reference to it (front matter values and `[[links]]`),
//! each keeping its spelling. Raw notes are never rewritten.

use super::graph::Wiki;
use crate::agent::taskdir::write_atomic;
use crate::error::{Result, failed};
use crate::md_edit::split_at_divider;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize, PartialEq)]
pub struct Rewrite {
    pub id: String,
    pub references: usize,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Moved {
    pub from: String,
    pub to: String,
    pub rewrites: Vec<Rewrite>,
}

fn respell(target: &str, label: &str, old_rel: &str, new_rel: &str) -> String {
    let (prefix, rest) = match target.split_once(':') {
        Some((l, r)) if l == label => (format!("{l}:"), r),
        _ => (String::new(), target),
    };
    let (rest, md) = match rest.strip_suffix(".md") {
        Some(r) => (r, ".md"),
        None => (rest, ""),
    };
    let short = rest != old_rel && format!("wiki/{rest}") == old_rel;
    let new = if short {
        new_rel.strip_prefix("wiki/").unwrap_or(new_rel)
    } else {
        new_rel
    };
    format!("{prefix}{new}{md}")
}

fn path_char(c: char) -> bool {
    c.is_alphanumeric() || "_./:-".contains(c)
}

fn replace_token(text: &str, old: &str, new: &str) -> (String, usize) {
    let (mut out, mut from, mut n) = (String::new(), 0, 0);
    while let Some(found) = text[from..].find(old) {
        let at = from + found;
        let end = at + old.len();
        let before = text[..at].chars().next_back();
        let after = text[end..].chars().next();
        let whole = !before.is_some_and(path_char) && !after.is_some_and(path_char);
        out.push_str(&text[from..at]);
        if whole {
            out.push_str(new);
            n += 1;
        } else {
            out.push_str(old);
        }
        from = end;
    }
    out.push_str(&text[from..]);
    (out, n)
}

fn replace_links(body: &str, old: &str, new: &str) -> (String, usize) {
    let (mut out, mut rest, mut n) = (String::new(), body, 0);
    while let Some(open) = rest.find("[[") {
        let Some(close) = rest[open + 2..].find("]]") else {
            break;
        };
        let (start, end) = (open + 2, open + 2 + close);
        let inner = &rest[start..end];
        let cut = inner.find(['|', '#']).unwrap_or(inner.len());
        out.push_str(&rest[..start]);
        // `[[a/b.md]]` is `[[a/b]]` (the link reader drops the extension too)
        let target = inner[..cut].trim();
        if target.strip_suffix(".md").unwrap_or(target) == old {
            out.push_str(new);
            out.push_str(&inner[cut..]);
            n += 1;
        } else {
            out.push_str(inner);
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    (out, n)
}

fn rewrite(content: &str, renames: &[(String, String)]) -> (String, usize) {
    let (head, yaml, body) = match content.strip_prefix("---\n").and_then(split_at_divider) {
        Some((yaml, body)) => ("---\n", yaml.to_string(), body.to_string()),
        None => ("", String::new(), content.to_string()),
    };
    let (mut yaml, mut body, mut total) = (yaml, body, 0);
    for (old, new) in renames {
        let (y, a) = replace_token(&yaml, old, new);
        let (b, c) = replace_links(&body, old, new);
        (yaml, body, total) = (y, b, total + a + c);
    }
    if head.is_empty() {
        (body, total)
    } else {
        (format!("{head}{yaml}---\n{body}"), total)
    }
}

/// Moves `node` to path `to` (no `.md`) in its level and rewrites references; `dry_run` only says what it would do.
pub fn mv(wiki: &Wiki, node: usize, to: &str, dry_run: bool) -> Result<Moved> {
    let n = &wiki.nodes[node];
    let level = &wiki.levels[n.level];
    let to = to.trim().trim_start_matches('/');
    let to = to.strip_suffix(".md").unwrap_or(to);
    // no part starts with a dot: not `..`, and not a hidden folder (`.git`,
    // `.iter`) that the wiki, git and the rest of iter treat as their own
    if to.is_empty() || to.split('/').any(|p| p.is_empty() || p.starts_with('.')) {
        return Err(failed(format!("`{to}` is not a path inside the level")));
    }
    if to.contains(':') {
        return Err(failed(
            "a file moves inside its level: give a path, not a `level:path`",
        ));
    }
    if !to.chars().all(path_char) {
        return Err(failed(format!(
            "`{to}` has a character a page's path cannot have: letters, digits and _ . - / only"
        )));
    }
    // a page stays a page: it moves to a folder the wiki gives a class to,
    // else it would leave the wiki and every reference to it would dangle
    if wiki.ontology.pages.contains(&n.class) {
        let folders: Vec<&str> = wiki
            .ontology
            .places
            .iter()
            .filter(|p| p.kind == level.kind && wiki.ontology.pages.contains(&p.class))
            .map(|p| p.folder.as_str())
            .collect();
        let inside = folders.iter().any(|f| {
            to.strip_prefix(*f)
                .is_some_and(|rest| rest.starts_with('/'))
        });
        if !inside {
            return Err(failed(format!(
                "`{to}` is not in a folder of the wiki: a page goes under {}",
                folders.join(", ")
            )));
        }
    }
    let target = level.dir.join(format!("{to}.md"));
    if target.exists() {
        return Err(failed(format!("{} already exists", target.display())));
    }

    let mut plans = Vec::new();
    for (i, page) in wiki.nodes.iter().enumerate() {
        let Some(front) = &page.front else {
            if page.links.is_empty() {
                continue;
            }
            plans.push((
                i,
                collect(wiki, page, node, level.label.as_str(), &n.rel, to, &[]),
            ));
            continue;
        };
        let targets: Vec<&String> = front.relations.iter().flat_map(|(_, t)| t).collect();
        plans.push((
            i,
            collect(wiki, page, node, level.label.as_str(), &n.rel, to, &targets),
        ));
    }
    let plans: Vec<_> = plans.into_iter().filter(|(_, r)| !r.is_empty()).collect();

    let mut rewrites = Vec::new();
    let mut edits = Vec::new();
    for (i, renames) in plans {
        let page = &wiki.nodes[i];
        let content = std::fs::read_to_string(&page.path)?;
        let (new_content, count) = rewrite(&content, &renames);
        if count > 0 {
            rewrites.push(Rewrite {
                id: wiki.id(i, n.level),
                references: count,
            });
            edits.push((page.path.clone(), content, new_content));
        }
    }

    if !dry_run {
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::rename(&n.path, &target)?;
        let mut written: Vec<(PathBuf, String)> = Vec::new();
        for (path, original, content) in edits {
            let path = if path == n.path { target.clone() } else { path };
            match write_atomic(&path, &content) {
                Ok(()) => written.push((path, original)),
                Err(e) => {
                    // half a move leaves dangling references: put back what was changed, report the original error
                    for (done, original) in written {
                        let _ = write_atomic(&done, &original);
                    }
                    let _ = std::fs::rename(&target, &n.path);
                    return Err(e.into());
                }
            }
        }
    }
    Ok(Moved {
        from: n.rel.clone(),
        to: to.to_string(),
        rewrites,
    })
}

fn collect(
    wiki: &Wiki,
    page: &super::graph::Node,
    node: usize,
    label: &str,
    old_rel: &str,
    new_rel: &str,
    targets: &[&String],
) -> Vec<(String, String)> {
    let mut renames: Vec<(String, String)> = Vec::new();
    for t in targets.iter().copied().chain(page.links.iter()) {
        let plain = t.strip_suffix(".md").unwrap_or(t);
        if wiki.resolve(page.level, plain) == Some(node) {
            let new = respell(t, label, old_rel, new_rel);
            if !renames.iter().any(|(o, _)| o == t) {
                renames.push((t.clone(), new));
            }
        }
    }
    // a longer spelling first, so one that contains another is not cut
    renames.sort_by_key(|(o, _)| std::cmp::Reverse(o.len()));
    renames
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::testutil::*;

    #[test]
    fn a_spelling_keeps_its_shape() {
        assert_eq!(
            respell("ideas/a", "p", "wiki/ideas/a", "wiki/ideas/b"),
            "ideas/b"
        );
        assert_eq!(
            respell("wiki/ideas/a", "p", "wiki/ideas/a", "wiki/concepts/a"),
            "wiki/concepts/a"
        );
        assert_eq!(
            respell("p:ideas/a.md", "p", "wiki/ideas/a", "wiki/concepts/a"),
            "p:concepts/a.md"
        );
        // moved out of wiki/: the short spelling cannot stay short
        assert_eq!(respell("ideas/a", "p", "wiki/ideas/a", "tmp/a"), "tmp/a");
    }

    #[test]
    fn tokens_are_whole_paths() {
        assert_eq!(
            replace_token("source: [a/b, a/bc, x/a/b]", "a/b", "a/z"),
            ("source: [a/z, a/bc, x/a/b]".to_string(), 1)
        );
        assert_eq!(
            replace_links("[[a/b]] [[a/b|label]] [[a/b#h]] [[a/bc]]", "a/b", "a/z"),
            ("[[a/z]] [[a/z|label]] [[a/z#h]] [[a/bc]]".to_string(), 3)
        );
    }

    #[test]
    fn moving_a_page_rewrites_what_points_at_it() {
        let root = fixture("mv");
        let w = build(&root);
        let backoff = w.node(1, "concepts/backoff").expect("concept");
        let moved = mv(&w, backoff, "wiki/syntheses/backoff", false).expect("moves");
        assert_eq!(moved.to, "wiki/syntheses/backoff");
        let mut ids: Vec<_> = moved.rewrites.iter().map(|r| r.id.as_str()).collect();
        ids.sort();
        assert_eq!(
            ids,
            [
                "acme.app.task-7:sketches/s",
                "wiki/entities/ci",
                "wiki/ideas/retry"
            ]
        );

        let app = root.join("acme/app");
        assert!(!app.join("wiki/concepts/backoff.md").exists());
        assert!(app.join("wiki/syntheses/backoff.md").exists());
        let idea = std::fs::read_to_string(app.join("wiki/ideas/retry.md")).expect("idea");
        assert!(idea.contains("see [[syntheses/backoff]]"), "{idea}");
        let ci = std::fs::read_to_string(root.join("acme/wiki/entities/ci.md")).expect("ci");
        assert!(
            ci.contains("[[acme.app:syntheses/backoff]] [[missing]]"),
            "{ci}"
        );
        let sketch = std::fs::read_to_string(app.join("tasks/7/sketches/s.md")).expect("sketch");
        assert!(sketch.contains("ref: syntheses/backoff"), "{sketch}");

        let again = build(&root);
        assert_eq!(again.dangling.len(), 1);
        assert_eq!(again.dangling[0].target, "missing");
        let moved = again.node(1, "syntheses/backoff").expect("moved");
        assert_eq!(again.nodes[moved].class.as_str(), "urn:iter:ns:Synthesis");
    }

    #[test]
    fn a_dry_run_changes_nothing() {
        let root = fixture("dry");
        let w = build(&root);
        let idea = w.node(1, "ideas/retry").expect("idea");
        let moved = mv(&w, idea, "wiki/ideas/retry-budget", true).expect("plans");
        assert!(!moved.rewrites.is_empty());
        assert!(root.join("acme/app/wiki/ideas/retry.md").exists());
        assert!(!root.join("acme/app/wiki/ideas/retry-budget.md").exists());
    }

    #[test]
    fn bad_targets_are_refused() {
        let root = fixture("refuse");
        let w = build(&root);
        let idea = w.node(1, "ideas/retry").expect("idea");
        for bad in [
            "",
            "../x",
            "a//b",
            ".git/x",
            "wiki/.hidden/x",
            "acme:wiki/x",
            "wiki/concepts/backoff",
        ] {
            assert!(mv(&w, idea, bad, true).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_token_is_replaced_only_when_whole_and_nothing_is_lost_between_matches() {
        assert_eq!(replace_token("a/b a/ba a/b", "a/b", "x").0, "x a/ba x");
        assert_eq!(
            replace_token("a/ba/b", "a/b", "x"),
            ("a/ba/b".to_string(), 0)
        );
        assert_eq!(replace_token("é a/b é", "a/b", "x").0, "é x é");
    }

    #[test]
    fn links_written_with_the_extension_are_rewritten_too() {
        let (out, n) = replace_links(
            "see [[ideas/a.md]] and [[ideas/a.md|that]] and [[ideas/ab]]",
            "ideas/a",
            "ideas/b",
        );
        assert_eq!(n, 2);
        assert_eq!(out, "see [[ideas/b]] and [[ideas/b|that]] and [[ideas/ab]]");
    }
}
