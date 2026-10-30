//! `mv`: renaming a file inside its level without breaking what points at it.
//!
//! Ids are paths, so moving a page (which changes its class when the folder
//! changes) must rewrite every reference to it: the front matter values and
//! `[[links]]` of the pages that resolve to it. Each reference keeps its
//! spelling: a level prefix stays, and so does a missing `wiki/`. Raw notes
//! are the user's and are never rewritten.

use super::graph::Wiki;
use crate::error::{IterError, Result};
use crate::md_edit::split_at_divider;
use serde::Serialize;

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

fn failed(message: impl Into<String>) -> IterError {
    IterError::CommandFailed(message.into())
}

/// `target`, which names `old_rel` in `label`'s level, as it names `new_rel`.
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

/// `text` with each whole-word `old` replaced by `new`: not part of a longer
/// path. How many were replaced.
fn replace_token(text: &str, old: &str, new: &str) -> (String, usize) {
    let (mut out, mut rest, mut n) = (String::new(), text, 0);
    while let Some(at) = rest.find(old) {
        let before = rest[..at].chars().next_back();
        let after = rest[at + old.len()..].chars().next();
        let whole = !before.is_some_and(path_char) && !after.is_some_and(path_char);
        out.push_str(&rest[..at]);
        if whole {
            out.push_str(new);
            n += 1;
        } else {
            out.push_str(old);
        }
        rest = &rest[at + old.len()..];
    }
    out.push_str(rest);
    (out, n)
}

/// The `[[target]]`s of `body` that are `old`, rewritten.
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
        if inner[..cut].trim() == old {
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

/// `content` with the references in `renames` (`old spelling -> new`)
/// rewritten in the front matter and in the links of the body.
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

/// Moves `node` to the path `to` (no `.md`) in its own level and rewrites the
/// references to it. With `dry_run`, only says what it would do.
pub fn mv(wiki: &Wiki, node: usize, to: &str, dry_run: bool) -> Result<Moved> {
    let n = &wiki.nodes[node];
    let level = &wiki.levels[n.level];
    let to = to.trim().trim_start_matches('/');
    let to = to.strip_suffix(".md").unwrap_or(to);
    if to.is_empty() || to.split('/').any(|p| p.is_empty() || p == "." || p == "..") {
        return Err(failed(format!("`{to}` is not a path inside the level")));
    }
    if to.contains(':') {
        return Err(failed(
            "a file moves inside its level: give a path, not a `level:path`",
        ));
    }
    let target = level.dir.join(format!("{to}.md"));
    if target.exists() {
        return Err(failed(format!("{} already exists", target.display())));
    }

    // what refers to it, and how each spells it
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
            edits.push((page.path.clone(), new_content));
        }
    }

    if !dry_run {
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::rename(&n.path, &target)?;
        for (path, content) in edits {
            // the moved file itself may refer to itself
            let path = if path == n.path { target.clone() } else { path };
            std::fs::write(path, content)?;
        }
    }
    Ok(Moved {
        from: n.rel.clone(),
        to: to.to_string(),
        rewrites,
    })
}

/// The spellings in `page` (front matter `targets` and body links) that name
/// `node`, with how they are written once it is at `new_rel`.
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
        // the idea's body link, the org entity's prefixed link, the sketch's `ref`
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

        // the graph is whole again: only the link that was already broken is left
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
        for bad in ["", "../x", "a//b", "acme:wiki/x", "wiki/concepts/backoff"] {
            assert!(mv(&w, idea, bad, true).is_err(), "{bad}");
        }
    }
}
