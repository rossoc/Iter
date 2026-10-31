//! Reading the wiki cheaply, in four steps of rising cost: [`find`] (rank by path, title, tags, description), [`grep`] (text),
//! [`neighbors`] (follow the graph) and [`show`] (load a page at a depth). `find` and `grep` stop at a token budget.

use super::frontmatter;
use super::graph::{Node, Wiki};
use super::ontology::NS;
use crate::error::{IterError, Result};
use regex::RegexBuilder;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

pub fn tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

fn class_name(node: &Node) -> String {
    node.class
        .as_str()
        .strip_prefix(NS)
        .unwrap_or(node.class.as_str())
        .to_string()
}

fn description_of(node: &Node) -> Option<String> {
    node.front.as_ref().and_then(|f| f.description.clone())
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Hit {
    pub id: String,
    pub class: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub score: u32,
    pub why: Vec<String>,
}

impl Hit {
    fn cost(&self) -> usize {
        tokens(&self.id)
            + tokens(&self.title)
            + self.description.as_deref().map_or(0, tokens)
            + self.tags.iter().map(|t| tokens(t)).sum::<usize>()
            + 4
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Found {
    pub hits: Vec<Hit>,
    pub left_out: usize,
}

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(String::from)
        .collect()
}

/// Ranks files by how well `query`'s words match path, title, tags and description, best first, until `budget` tokens are spent (the best hit is always returned). A note matches by name only.
pub fn find(wiki: &Wiki, from: usize, query: &str, budget: usize) -> Found {
    let words = words(query);
    let mut scored: Vec<(u32, usize, Vec<String>)> = Vec::new();
    for (i, node) in wiki.nodes.iter().enumerate() {
        let path = node.rel.to_lowercase();
        let title = node.title.to_lowercase();
        let front = node.front.as_ref();
        let description = front
            .and_then(|f| f.description.as_deref())
            .unwrap_or("")
            .to_lowercase();
        let tags: Vec<String> = front
            .map(|f| f.tags.iter().map(|t| t.to_lowercase()).collect())
            .unwrap_or_default();
        let (mut score, mut why, mut matched) = (0, Vec::<String>::new(), 0);
        for word in &words {
            let mut hit = false;
            let mut add = |points: u32, what: &str| {
                score += points;
                hit = true;
                if !why.iter().any(|w| w == what) {
                    why.push(what.to_string());
                }
            };
            if tags.iter().any(|t| t == word) {
                add(4, "tag");
            } else if tags.iter().any(|t| t.contains(word.as_str())) {
                add(2, "tag");
            }
            if path.contains(word.as_str()) {
                add(3, "path");
            }
            if title.contains(word.as_str()) {
                add(3, "title");
            }
            if description.contains(word.as_str()) {
                add(2, "description");
            }
            matched += hit as usize;
        }
        if score > 0 {
            // a page that answers every word beats one that answers some
            if matched == words.len() {
                score += words.len() as u32;
            }
            scored.push((score, i, why));
        }
    }
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| wiki.nodes[a.1].rel.cmp(&wiki.nodes[b.1].rel))
    });
    let total = scored.len();
    let mut hits = Vec::new();
    let mut spent = 0;
    for (score, i, why) in scored {
        let node = &wiki.nodes[i];
        let hit = Hit {
            id: wiki.id(i, from),
            class: class_name(node),
            title: node.title.clone(),
            description: description_of(node),
            tags: node
                .front
                .as_ref()
                .map(|f| f.tags.clone())
                .unwrap_or_default(),
            score,
            why,
        };
        if !hits.is_empty() && spent + hit.cost() > budget {
            break;
        }
        spent += hit.cost();
        hits.push(hit);
    }
    Found {
        left_out: total - hits.len(),
        hits,
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Match {
    pub id: String,
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Grepped {
    pub matches: Vec<Match>,
    pub left_out: usize,
}

/// Lines of pages (and notes with `notes`) matching regex `pattern`, in file order, until `budget` is spent; case ignored unless `case`.
pub fn grep(
    wiki: &Wiki,
    from: usize,
    pattern: &str,
    notes: bool,
    case: bool,
    budget: usize,
) -> Result<Grepped> {
    let re = RegexBuilder::new(pattern)
        .case_insensitive(!case)
        .build()
        .map_err(|e| IterError::CommandFailed(format!("bad pattern: {e}")))?;
    let (mut matches, mut spent, mut left_out) = (Vec::new(), 0, 0);
    for (i, node) in wiki.nodes.iter().enumerate() {
        if !notes && !wiki.ontology.pages.contains(&node.class) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&node.path) else {
            continue;
        };
        for (n, line) in content.lines().enumerate() {
            if !re.is_match(line) {
                continue;
            }
            let text: String = line.trim().chars().take(160).collect();
            let m = Match {
                id: wiki.id(i, from),
                line: n + 1,
                text,
            };
            let cost = tokens(&m.id) + tokens(&m.text) + 2;
            if !matches.is_empty() && spent + cost > budget {
                left_out += 1;
                continue;
            }
            spent += cost;
            matches.push(m);
        }
    }
    Ok(Grepped { matches, left_out })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum)]
pub enum Depth {
    /// What the page says in a line, its tags and where it stands
    Description,
    /// And what it relates to, and its headings
    Summary,
    /// And the whole text
    Full,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Related {
    pub prop: String,
    pub id: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Shown {
    pub id: String,
    pub class: String,
    pub path: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relations: Option<Vec<Related>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// One file at `depth`; the text is read from disk, the graph does not keep it.
pub fn show(wiki: &Wiki, from: usize, node: usize, depth: Depth) -> Result<Shown> {
    let n = &wiki.nodes[node];
    let front = n.front.clone().unwrap_or_default();
    let mut shown = Shown {
        id: wiki.id(node, from),
        class: class_name(n),
        path: n.path.display().to_string(),
        title: n.title.clone(),
        description: front.description,
        tags: front.tags,
        confidence: front.confidence,
        updated: front.updated,
        relations: None,
        outline: None,
        content: None,
    };
    if depth == Depth::Description {
        return Ok(shown);
    }
    let content = std::fs::read_to_string(&n.path)?;
    let body = frontmatter::split(&content).body;
    shown.relations = Some(
        wiki.edges_out(node)
            .into_iter()
            .map(|(prop, other)| Related {
                prop,
                id: wiki.id(other, from),
            })
            .collect(),
    );
    shown.outline = Some(
        body.lines()
            .filter(|l| l.starts_with('#'))
            .map(|l| l.trim().to_string())
            .collect(),
    );
    if depth == Depth::Full {
        shown.content = Some(content);
    }
    Ok(shown)
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Neighbor {
    pub id: String,
    pub class: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub hop: usize,
    /// The relations it was reached by (a page can be related in several
    /// ways: it includes it and marks it `nb`)...
    pub via: Vec<String>,
    /// ...from this page.
    pub from: String,
}

/// Files within `hops` relations of `start`, nearest first, along any relation (backlinks included) or only the `props` named (local names: `nb`, `include`);
/// within a hop the `nb` pages come first.
pub fn neighbors(
    wiki: &Wiki,
    from: usize,
    start: usize,
    hops: usize,
    props: &[String],
) -> Vec<Neighbor> {
    let mut seen = HashSet::from([start]);
    let mut frontier = vec![start];
    let mut out = Vec::new();
    for hop in 1..=hops {
        // page -> (the relations that reach it, the first page it was reached from)
        let mut reached: BTreeMap<usize, (Vec<String>, usize)> = BTreeMap::new();
        for &at in &frontier {
            for (prop, other) in wiki.edges_out(at) {
                if seen.contains(&other) || (!props.is_empty() && !props.contains(&prop)) {
                    continue;
                }
                let entry = reached.entry(other).or_insert((Vec::new(), at));
                if !entry.0.contains(&prop) {
                    entry.0.push(prop);
                }
            }
        }
        let mut batch: Vec<Neighbor> = reached
            .iter()
            .map(|(&other, (via, at))| {
                let node = &wiki.nodes[other];
                Neighbor {
                    id: wiki.id(other, from),
                    class: class_name(node),
                    description: description_of(node),
                    hop,
                    via: via.clone(),
                    from: wiki.id(*at, from),
                }
            })
            .collect();
        batch.sort_by(|a, b| {
            let nb = |n: &Neighbor| !n.via.iter().any(|p| p == "nb");
            nb(a).cmp(&nb(b)).then_with(|| a.id.cmp(&b.id))
        });
        seen.extend(reached.keys());
        frontier = reached.into_keys().collect();
        out.extend(batch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::testutil::*;

    #[test]
    fn find_ranks_by_tag_path_and_description() {
        let w = build(&fixture("find"));
        let found = find(&w, 1, "retr", 10_000);
        let ids: Vec<_> = found.hits.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids[0], "wiki/ideas/retry");
        assert!(ids.contains(&"wiki/concepts/backoff"));
        assert_eq!(found.hits[0].why, ["path", "title", "description"]);
        let by_tag = find(&w, 1, "net", 10_000);
        assert_eq!(by_tag.hits[0].id, "wiki/ideas/retry");
        assert_eq!(by_tag.hits[0].why, ["tag"]);
        assert!(find(&w, 1, "zzz", 10_000).hits.is_empty());
    }

    #[test]
    fn find_stops_at_the_budget_but_keeps_the_best() {
        let w = build(&fixture("budget"));
        let all = find(&w, 1, "retr", 10_000);
        assert!(all.hits.len() >= 2 && all.left_out == 0);
        let one = find(&w, 1, "retr", 1);
        assert_eq!(one.hits.len(), 1);
        assert_eq!(one.left_out, all.hits.len() - 1);
    }

    #[test]
    fn ids_are_bare_unless_the_page_is_in_a_level_the_question_cannot_see_into() {
        let w = build(&fixture("ids"));
        let id = |from, query: &str| find(&w, from, query, 10_000).hits.remove(0).id;
        assert_eq!(id(0, "backoff"), "wiki/concepts/backoff");
        assert_eq!(id(1, "scratch"), "acme.app.task-7:sketches/s");
    }

    #[test]
    fn grep_finds_lines_in_pages_and_in_notes_on_request() {
        let w = build(&fixture("grep"));
        let g = grep(&w, 1, "BODY", false, false, 10_000).expect("grep");
        assert_eq!(
            g.matches,
            [Match {
                id: "wiki/concepts/backoff".into(),
                line: 6,
                text: "body".into()
            }]
        );
        assert!(
            grep(&w, 1, "raw thoughts", false, false, 10_000)
                .expect("grep")
                .matches
                .is_empty()
        );
        let with_notes = grep(&w, 1, "raw thoughts", true, false, 10_000).expect("grep");
        assert_eq!(with_notes.matches[0].id, "2026-10-27");
        assert!(
            grep(&w, 1, "BODY", false, true, 10_000)
                .expect("grep")
                .matches
                .is_empty()
        );
        assert!(grep(&w, 1, "(", false, false, 100).is_err());
    }

    #[test]
    fn show_goes_deeper_with_the_depth() {
        let w = build(&fixture("show"));
        let idea = w.node(1, "ideas/retry").expect("idea");
        let a = show(&w, 1, idea, Depth::Description).expect("show");
        assert_eq!(a.description.as_deref(), Some("Retries need a budget"));
        assert_eq!(a.class, "Idea");
        assert!(a.relations.is_none() && a.content.is_none());
        let s = show(&w, 1, idea, Depth::Summary).expect("show");
        assert_eq!(
            s.outline.as_deref(),
            Some(&["# Retry budget".to_string()][..])
        );
        let rels = s.relations.expect("relations");
        assert!(rels.contains(&Related {
            prop: "source".into(),
            id: "2026-10-27".into()
        }));
        assert!(rels.contains(&Related {
            prop: "includedIn".into(),
            id: "wiki/concepts/backoff".into()
        }));
        assert!(s.content.is_none());
        let f = show(&w, 1, idea, Depth::Full).expect("show");
        assert!(
            f.content
                .expect("content")
                .contains("see [[concepts/backoff]]")
        );
    }

    #[test]
    fn neighbors_walk_the_graph_nearest_first() {
        let w = build(&fixture("neighbors"));
        let backoff = w.node(1, "concepts/backoff").expect("concept");
        let one = neighbors(&w, 1, backoff, 1, &[]);
        let ids: Vec<_> = one.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"wiki/ideas/retry"));
        assert!(one.iter().all(|n| n.hop == 1));
        let idea = w.node(1, "ideas/retry").expect("idea");
        let two = neighbors(&w, 1, idea, 2, &[]);
        assert!(two.iter().any(|n| n.hop == 2 && n.id == "wiki/entities/ci"));
        let idea_via = &one
            .iter()
            .find(|n| n.id == "wiki/ideas/retry")
            .expect("idea")
            .via;
        assert_eq!(idea_via, &["include", "nb", "referencedBy"]);
        let only_nb = neighbors(&w, 1, backoff, 2, &["nb".to_string()]);
        assert_eq!(only_nb.len(), 1);
        assert_eq!(only_nb[0].via, ["nb"]);
    }
}
