//! Building the graph from the markdown, and asking it questions.
//!
//! A [`Level`] is a directory with a meaning: an organization's, a project's
//! or one task's folder. The caller (which has the database) decides which
//! levels are in scope and passes them; everything else follows from the
//! ontology: which files are what (by folder), which are read, and how they
//! relate.

use super::frontmatter::{self, Front};
use super::ontology::{LevelKind, NODE, NS, Ontology, Place, Rules, ns};
use crate::error::{IterError, Result};
use crate::notes::parse_date;
use oxigraph::io::RdfFormat;
use oxigraph::model::vocab::{rdf, rdfs};
use oxigraph::model::{GraphName, GraphNameRef, Literal, NamedNode, NamedOrBlankNode, Quad, Term};
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A directory of the wiki and what it is.
#[derive(Clone, Debug)]
pub struct Level {
    pub kind: LevelKind,
    /// Unique among the levels; the prefix of a node's id when it is not in
    /// the level the question is asked from (`acme.app:wiki/concepts/x`).
    pub label: String,
    /// What the level is called (a project's name).
    pub title: String,
    pub dir: PathBuf,
    /// The index of the level this one is in (a task in its project).
    pub parent: Option<usize>,
}

/// One markdown file.
#[derive(Debug)]
pub struct Node {
    pub iri: NamedNode,
    pub level: usize,
    /// The path inside the level's directory, without `.md`, `/`-separated.
    pub rel: String,
    pub class: NamedNode,
    pub path: PathBuf,
    pub title: String,
    /// `Some` only for a page, and only if it opens with a front matter that
    /// parses.
    pub front: Option<Front>,
    /// Why the front matter could not be read.
    pub front_error: Option<String>,
    /// The `[[links]]` in the body (pages only).
    pub links: Vec<String>,
    pub modified: Option<SystemTime>,
}

/// A relation whose target is not in the graph.
#[derive(Debug, PartialEq)]
pub struct Dangling {
    pub from: usize,
    /// The front matter key, or `[[link]]` for a body link.
    pub via: String,
    pub target: String,
}

pub struct Wiki {
    pub levels: Vec<Level>,
    pub nodes: Vec<Node>,
    pub dangling: Vec<Dangling>,
    pub ontology: Ontology,
    store: Store,
    index: HashMap<(usize, String), usize>,
    by_iri: HashMap<String, usize>,
}

/// The rows of a `SELECT`.
#[derive(Debug, PartialEq, serde::Serialize)]
pub struct Table {
    pub vars: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
}

/// What a SPARQL query answers.
#[derive(Debug, PartialEq)]
pub enum Answer {
    Table(Table),
    Boolean(bool),
    /// A `CONSTRUCT` or `DESCRIBE`, as N-Triples.
    Triples(String),
}

const PREFIXES: &str = "PREFIX iter: <urn:iter:ns:>\n\
    PREFIX owl: <http://www.w3.org/2002/07/owl#>\n\
    PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>\n\
    PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>\n\
    PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>\n";

fn failed(e: impl std::fmt::Display) -> IterError {
    IterError::CommandFailed(e.to_string())
}

/// `s` as the path part of an IRI: everything but letters, digits and
/// `-._~/` is percent-encoded.
fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn level_iri(label: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("{NODE}{}", encode(label)))
}

fn node_iri(label: &str, rel: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("{NODE}{}/{}", encode(label), encode(rel)))
}

/// The `.md` files a place holds, as `(path, rel)`, in a fixed order.
fn files_of(dir: &Path, folder: &str) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    if folder == "." {
        // the root holds notes of days; any other file there is not ours
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let stem = path.file_stem().and_then(|s| s.to_str());
                let is_md = path.extension().is_some_and(|e| e == "md");
                let link = entry.file_type().is_ok_and(|t| t.is_symlink());
                if let (true, Some(stem)) = (is_md && path.is_file() && !link, stem)
                    && parse_date(stem).is_some()
                {
                    out.push((path.clone(), stem.to_string()));
                }
            }
        }
    } else if !stays_inside(folder) {
        // an ontology can name any folder; one that leaves the level would
        // read (and, through a move, rewrite) files that are not the wiki's
    } else if let Some(file) = folder.strip_suffix(".md") {
        let path = dir.join(folder);
        if path.is_file() && !path.is_symlink() {
            out.push((path, file.to_string()));
        }
    } else {
        for path in md_files(&dir.join(folder)) {
            if let Ok(rel) = path.strip_prefix(dir) {
                let id: Vec<_> = rel
                    .with_extension("")
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect();
                out.push((path, id.join("/")));
            }
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// Whether `folder` (from an ontology file) is a relative path that stays
/// inside the level: no `..`, no root, no drive.
fn stays_inside(folder: &str) -> bool {
    Path::new(folder)
        .components()
        .all(|c| matches!(c, std::path::Component::Normal(_) | std::path::Component::CurDir))
}

/// Every `.md` file under `at`, however deep: dotfiles are skipped, and a
/// link is not followed (it could lead out of the folder, or round to it).
pub(super) fn md_files(at: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(at) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.')
            || entry.file_type().is_ok_and(|t| t.is_symlink())
        {
            continue;
        }
        if path.is_dir() {
            out.extend(md_files(&path));
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
    out
}

impl Wiki {
    /// Reads every level's files and builds the graph. `levels` come with
    /// their `parent` indexes; each directory may be missing (it is then
    /// empty).
    pub fn build(levels: Vec<Level>, ontology: Ontology) -> Result<Wiki> {
        // which files, then read them side by side: it is most of the time
        let mut jobs: Vec<(usize, &Place, PathBuf, String)> = Vec::new();
        let mut seen: HashSet<PathBuf> = HashSet::new();
        for (li, level) in levels.iter().enumerate() {
            for place in ontology.places.iter().filter(|p| p.kind == level.kind) {
                for (path, rel) in files_of(&level.dir, &place.folder) {
                    if seen.insert(path.clone()) {
                        jobs.push((li, place, path, rel));
                    }
                }
            }
        }
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
        let chunk = jobs.len().div_ceil(threads).max(64);
        let (levels_ref, ontology_ref) = (&levels, &ontology);
        let nodes: Vec<Node> = std::thread::scope(|scope| {
            let readers: Vec<_> = jobs
                .chunks(chunk)
                .map(|part| {
                    scope.spawn(move || {
                        part.iter()
                            .map(|(li, place, path, rel)| {
                                let level = &levels_ref[*li];
                                read_node(
                                    ontology_ref,
                                    *li,
                                    level,
                                    place,
                                    path.clone(),
                                    rel.clone(),
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            readers
                .into_iter()
                .flat_map(|r| r.join().expect("a reader does not panic"))
                .collect()
        });
        let index = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| ((n.level, n.rel.clone()), i))
            .collect();
        let by_iri = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.iri.as_str().to_string(), i))
            .collect();
        let mut wiki = Wiki {
            levels,
            nodes,
            dangling: Vec::new(),
            ontology,
            store: Store::new().map_err(failed)?,
            index,
            by_iri,
        };
        wiki.fill()?;
        Ok(wiki)
    }

    /// The node `target` names, written in a page of `from`: `label:path` for
    /// any level, otherwise a path looked up in `from` and then in the levels
    /// around it, nearest first. `wiki/` may be left out.
    pub fn resolve(&self, from: usize, target: &str) -> Option<usize> {
        let target = target.trim().trim_start_matches('/');
        let target = target.strip_suffix(".md").unwrap_or(target);
        if let Some((label, rel)) = target.split_once(':')
            && let Some(l) = self.levels.iter().position(|l| l.label == label)
        {
            return self.find_in(l, rel);
        }
        let mut at = Some(from);
        while let Some(l) = at {
            if let Some(i) = self.find_in(l, target) {
                return Some(i);
            }
            at = self.levels[l].parent;
        }
        None
    }

    fn find_in(&self, level: usize, rel: &str) -> Option<usize> {
        [rel.to_string(), format!("wiki/{rel}")]
            .into_iter()
            .find_map(|r| self.index.get(&(level, r)).copied())
    }

    /// How a node is written to someone asking from `from` (the innermost
    /// level): its path, if that path names the node from there (the nearest
    /// level wins), otherwise `label:path`. So an id always resolves back to
    /// its node, asked from the same level.
    pub fn id(&self, node: usize, from: usize) -> String {
        let n = &self.nodes[node];
        if self.resolve(from, &n.rel) == Some(node) {
            n.rel.clone()
        } else {
            format!("{}:{}", self.levels[n.level].label, n.rel)
        }
    }

    /// The node an id (as [`Wiki::id`] writes it) names, asked from `from`.
    pub fn node(&self, from: usize, id: &str) -> Option<usize> {
        self.resolve(from, id)
    }

    /// The relations from `node` to other files, as `(property, node)`: the
    /// ones written and the ones the rules derived (the inverses: a page's
    /// backlinks are its `referencedBy`, `includedIn`, `sourceOf`). A property
    /// implied by a more specific one for the same target (`ref` by `nb`) is
    /// left out. Sorted.
    pub fn edges_out(&self, node: usize) -> Vec<(String, usize)> {
        let subject = self.nodes[node].iri.as_ref();
        let mut edges: Vec<(String, usize)> = Vec::new();
        for quad in self
            .store
            .quads_for_pattern(Some(subject.into()), None, None, None)
            .flatten()
        {
            let Term::NamedNode(object) = &quad.object else {
                continue;
            };
            let (Some(&other), Some(local)) = (
                self.by_iri.get(object.as_str()),
                quad.predicate.as_str().strip_prefix(NS),
            ) else {
                continue;
            };
            edges.push((local.to_string(), other));
        }
        let implied: Vec<(String, usize)> = self
            .ontology
            .rules
            .sub_property
            .iter()
            .filter_map(|(sub, sup)| {
                let (sub, sup) = (
                    sub.as_str().strip_prefix(NS)?,
                    sup.as_str().strip_prefix(NS)?,
                );
                Some(
                    edges
                        .iter()
                        .filter(|(p, _)| p == sub)
                        .map(|(_, o)| (sup.to_string(), *o))
                        .collect::<Vec<_>>(),
                )
            })
            .flatten()
            .collect();
        edges.retain(|e| !implied.contains(e));
        edges.sort();
        edges.dedup();
        edges
    }

    /// Collects every fact (the ontology, the levels, the files), lets the
    /// rules derive the rest, and loads the store once: a store insert is a
    /// transaction, so thousands of them one by one cost seconds.
    fn fill(&mut self) -> Result<()> {
        let mut facts: HashSet<Fact> = HashSet::new();
        for t in self.ontology.graph.iter() {
            facts.insert((
                Term::from(t.subject.into_owned()),
                t.predicate.into_owned(),
                t.object.into_owned(),
            ));
        }
        let mut add = |s: &NamedNode, p: &NamedNode, o: Term| {
            facts.insert((Term::from(s.clone()), p.clone(), o));
        };
        let lit = |s: &str| Term::from(Literal::new_simple_literal(s));
        let (type_, label, in_) = (rdf::TYPE.into_owned(), rdfs::LABEL.into_owned(), ns("in"));
        for level in &self.levels {
            let iri = level_iri(&level.label);
            add(&iri, &type_, level.kind.class().into());
            add(&iri, &label, lit(&level.title));
            if let Some(p) = level.parent {
                add(&iri, &in_, level_iri(&self.levels[p].label).into());
            }
        }
        let mut dangling = Vec::new();
        for (i, node) in self.nodes.iter().enumerate() {
            add(&node.iri, &type_, node.class.clone().into());
            add(&node.iri, &label, lit(&node.title));
            add(
                &node.iri,
                &in_,
                level_iri(&self.levels[node.level].label).into(),
            );
            if let Some(front) = &node.front {
                for (key, value) in front.scalars() {
                    if let Some(v) = value {
                        add(&node.iri, &ns(key), lit(v));
                    }
                }
                for tag in &front.tags {
                    add(&node.iri, &ns("tag"), lit(tag));
                }
                for (key, targets) in &front.relations {
                    let Some(prop) = self.ontology.relation(key) else {
                        continue;
                    };
                    for target in targets {
                        match self.resolve(node.level, target) {
                            Some(t) => add(&node.iri, prop, self.nodes[t].iri.clone().into()),
                            None => dangling.push(Dangling {
                                from: i,
                                via: key.clone(),
                                target: target.clone(),
                            }),
                        }
                    }
                }
            }
            for link in &node.links {
                match self.resolve(node.level, link) {
                    Some(t) => add(&node.iri, &ns("ref"), self.nodes[t].iri.clone().into()),
                    None => dangling.push(Dangling {
                        from: i,
                        via: "[[link]]".into(),
                        target: link.clone(),
                    }),
                }
            }
        }
        self.dangling = dangling;
        entail(&mut facts, &self.ontology.rules);
        let quads = facts.into_iter().filter_map(|(s, p, o)| {
            let subject = NamedOrBlankNode::try_from(s).ok()?;
            Some(Quad::new(subject, p, o, GraphName::DefaultGraph))
        });
        self.store.extend(quads).map_err(failed)
    }

    /// Runs a SPARQL query over the graph (the default prefixes `iter:`,
    /// `owl:`, `rdf:`, `rdfs:` and `xsd:` are declared). `UPDATE` is not
    /// possible: the markdown is where things change.
    pub fn query(&self, sparql: &str) -> Result<Answer> {
        let text = format!("{PREFIXES}{sparql}");
        let prepared = SparqlEvaluator::new().parse_query(&text).map_err(failed)?;
        match prepared.on_store(&self.store).execute().map_err(failed)? {
            QueryResults::Solutions(solutions) => {
                let vars: Vec<String> = solutions
                    .variables()
                    .iter()
                    .map(|v| v.as_str().to_string())
                    .collect();
                let mut rows = Vec::new();
                for solution in solutions {
                    let solution = solution.map_err(failed)?;
                    rows.push(
                        vars.iter()
                            .map(|v| solution.get(v.as_str()).map(term_text))
                            .collect(),
                    );
                }
                Ok(Answer::Table(Table { vars, rows }))
            }
            QueryResults::Boolean(b) => Ok(Answer::Boolean(b)),
            QueryResults::Graph(triples) => {
                let mut out = String::new();
                for t in triples {
                    out.push_str(&format!("{} .\n", t.map_err(failed)?));
                }
                Ok(Answer::Triples(out))
            }
        }
    }

    /// The whole graph, with what the rules derived, as N-Triples.
    pub fn ntriples(&self) -> Result<String> {
        let mut out = Vec::new();
        self.store
            .dump_graph_to_writer(GraphNameRef::DefaultGraph, RdfFormat::NTriples, &mut out)
            .map_err(failed)?;
        String::from_utf8(out).map_err(failed)
    }
}

/// An IRI as itself, a literal as its text.
pub fn term_text(t: &Term) -> String {
    match t {
        Term::NamedNode(n) => n.as_str().to_string(),
        Term::BlankNode(b) => format!("_:{}", b.as_str()),
        Term::Literal(l) => l.value().to_string(),
        #[allow(unreachable_patterns)]
        other => other.to_string(),
    }
}

fn read_node(
    ontology: &Ontology,
    li: usize,
    level: &Level,
    place: &Place,
    path: PathBuf,
    rel: String,
) -> Node {
    let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    let stem = rel.rsplit('/').next().unwrap_or(&rel).to_string();
    let mut node = Node {
        iri: node_iri(&level.label, &rel),
        level: li,
        class: place.class.clone(),
        title: stem,
        front: None,
        front_error: None,
        links: Vec::new(),
        modified,
        rel,
        path,
    };
    // notes are the user's: listed, never read
    if !ontology.pages.contains(&place.class) {
        return node;
    }
    let Ok(content) = std::fs::read_to_string(&node.path) else {
        node.front_error = Some("the file cannot be read".into());
        return node;
    };
    let split = frontmatter::split(&content);
    if let Some(yaml) = split.yaml {
        match frontmatter::parse(yaml, &ontology.relation_keys()) {
            Ok(front) => node.front = Some(front),
            Err(e) => node.front_error = Some(e),
        }
    }
    if let Some(title) = frontmatter::title(split.body) {
        node.title = title;
    }
    node.links = frontmatter::wikilinks(split.body);
    node
}

type Fact = (Term, NamedNode, Term);

/// Rules by the predicate they fire on.
struct Index<'a> {
    sub_class: HashMap<&'a NamedNode, Vec<&'a NamedNode>>,
    sub_property: HashMap<&'a NamedNode, Vec<&'a NamedNode>>,
    inverse: HashMap<&'a NamedNode, Vec<&'a NamedNode>>,
    symmetric: HashSet<&'a NamedNode>,
    transitive: HashSet<&'a NamedNode>,
}

fn by_first(
    pairs: &[(NamedNode, NamedNode)],
    both_ways: bool,
) -> HashMap<&NamedNode, Vec<&NamedNode>> {
    let mut map: HashMap<&NamedNode, Vec<&NamedNode>> = HashMap::new();
    for (a, b) in pairs {
        map.entry(a).or_default().push(b);
        if both_ways {
            map.entry(b).or_default().push(a);
        }
    }
    map
}

/// Adds to `facts` what the ontology's rules entail: the OWL 2 RL subset of
/// subclass, subproperty, inverse, symmetric and transitive. (Domains and
/// ranges are constraints for `check`, not rules.) Each fact is looked at once,
/// when it appears: what it derives is queued, and a transitive property joins
/// it with the facts seen so far on either side.
fn entail(facts: &mut HashSet<Fact>, rules: &Rules) {
    let index = Index {
        sub_class: by_first(&rules.sub_class, false),
        sub_property: by_first(&rules.sub_property, false),
        inverse: by_first(&rules.inverse, true),
        symmetric: rules.symmetric.iter().collect(),
        transitive: rules.transitive.iter().collect(),
    };
    let rdf_type = rdf::TYPE.into_owned();
    // per transitive property: who it leads to, and who leads to it
    type Edges = HashMap<Term, Vec<Term>>;
    let mut edges: HashMap<NamedNode, (Edges, Edges)> = HashMap::new();
    let mut queue: Vec<Fact> = facts.iter().cloned().collect();
    while let Some((s, p, o)) = queue.pop() {
        let mut derived: Vec<Fact> = Vec::new();
        if p == rdf_type
            && let Term::NamedNode(class) = &o
        {
            for sup in index.sub_class.get(class).into_iter().flatten() {
                derived.push((s.clone(), rdf_type.clone(), (*sup).clone().into()));
            }
        }
        for sup in index.sub_property.get(&p).into_iter().flatten() {
            derived.push((s.clone(), (*sup).clone(), o.clone()));
        }
        for other in index.inverse.get(&p).into_iter().flatten() {
            derived.push((o.clone(), (*other).clone(), s.clone()));
        }
        if index.symmetric.contains(&p) {
            derived.push((o.clone(), p.clone(), s.clone()));
        }
        if index.transitive.contains(&p) {
            let (to, from) = edges.entry(p.clone()).or_default();
            for next in to.get(&o).into_iter().flatten() {
                derived.push((s.clone(), p.clone(), next.clone()));
            }
            for before in from.get(&s).into_iter().flatten() {
                derived.push((before.clone(), p.clone(), o.clone()));
            }
            to.entry(s.clone()).or_default().push(o.clone());
            from.entry(o.clone()).or_default().push(s.clone());
        }
        for fact in derived {
            // a literal cannot be a subject: a rule that would make one is skipped
            if !matches!(fact.0, Term::Literal(_)) && facts.insert(fact.clone()) {
                queue.push(fact);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::testutil::*;

    fn ask(w: &Wiki, q: &str) -> bool {
        match w.query(q).expect("query") {
            Answer::Boolean(b) => b,
            other => panic!("not an ASK: {other:?}"),
        }
    }

    #[test]
    fn the_class_comes_from_the_folder() {
        let w = build(&fixture("class"));
        assert!(ask(
            &w,
            "ASK { <urn:iter:acme.app/wiki/ideas/retry> a iter:Idea }"
        ));
        assert!(ask(
            &w,
            "ASK { <urn:iter:acme.app/wiki/concepts/backoff> a iter:Concept }"
        ));
        assert!(ask(
            &w,
            "ASK { <urn:iter:acme.app/2026-10-27> a iter:Note }"
        ));
        assert!(ask(
            &w,
            "ASK { <urn:iter:acme.app.task-7/plan> a iter:Plan }"
        ));
        // a plan is a sketch is a page: the subclass rule ran
        assert!(ask(
            &w,
            "ASK { <urn:iter:acme.app.task-7/plan> a iter:Page }"
        ));
        assert!(!ask(
            &w,
            "ASK { <urn:iter:acme.app/2026-10-27> a iter:Page }"
        ));
    }

    #[test]
    fn notes_are_listed_but_never_read() {
        let w = build(&fixture("notes"));
        let note = w
            .nodes
            .iter()
            .find(|n| n.rel == "2026-10-27")
            .expect("note");
        assert!(note.front.is_none() && note.front_error.is_none() && note.links.is_empty());
    }

    #[test]
    fn front_matter_and_links_become_triples() {
        let w = build(&fixture("triples"));
        let idea = "<urn:iter:acme.app/wiki/ideas/retry>";
        assert!(ask(
            &w,
            &format!(
                "ASK {{ {idea} iter:description \"Retries need a budget\" ; iter:tag \"net\" ; iter:source <urn:iter:acme.app/2026-10-27> }}"
            )
        ));
        assert!(ask(
            &w,
            &format!("ASK {{ {idea} iter:ref <urn:iter:acme.app/wiki/concepts/backoff> }}")
        ));
        assert!(ask(
            &w,
            &format!("ASK {{ {idea} rdfs:label \"Retry budget\" }}")
        ));
    }

    #[test]
    fn the_owl_rules_derive_the_rest() {
        let w = build(&fixture("rules"));
        let (idea, concept) = (
            "<urn:iter:acme.app/wiki/ideas/retry>",
            "<urn:iter:acme.app/wiki/concepts/backoff>",
        );
        // nb is a sub-property of ref
        assert!(ask(&w, &format!("ASK {{ {concept} iter:ref {idea} }}")));
        // inverseOf
        assert!(ask(
            &w,
            &format!("ASK {{ {idea} iter:includedIn {concept} }}")
        ));
        assert!(ask(
            &w,
            &format!("ASK {{ {idea} iter:referencedBy {concept} }}")
        ));
        // iter:in is transitive: the idea is in the organization too
        assert!(ask(
            &w,
            &format!("ASK {{ {idea} iter:in <urn:iter:acme> }}")
        ));
        assert!(ask(
            &w,
            "ASK { <urn:iter:acme.app.task-7> iter:in <urn:iter:acme> }"
        ));
    }

    #[test]
    fn a_link_resolves_in_its_level_then_outward_or_by_label() {
        let w = build(&fixture("resolve"));
        let org_page = w
            .nodes
            .iter()
            .position(|n| n.rel == "wiki/entities/ci")
            .expect("ci");
        let target = w.resolve(w.nodes[org_page].level, "acme.app:concepts/backoff");
        assert_eq!(
            target.map(|i| w.nodes[i].rel.as_str()),
            Some("wiki/concepts/backoff")
        );
        // from the task, outward to the project
        let from_task = w.resolve(0, "concepts/backoff").expect("found");
        assert_eq!(w.id(from_task, 0), "wiki/concepts/backoff");
        assert_eq!(w.resolve(0, "ideas/nothing"), None);
    }

    #[test]
    fn unresolved_targets_are_dangling_not_triples() {
        let w = build(&fixture("dangling"));
        let from = w
            .nodes
            .iter()
            .position(|n| n.rel == "wiki/entities/ci")
            .expect("ci");
        assert_eq!(
            w.dangling,
            [Dangling {
                from,
                via: "[[link]]".into(),
                target: "missing".into()
            }]
        );
    }

    #[test]
    fn select_returns_a_table_and_a_bad_query_is_an_error() {
        let w = build(&fixture("select"));
        let Answer::Table(t) = w
            .query("SELECT ?a WHERE { ?p iter:description ?a ; a iter:Concept }")
            .expect("query")
        else {
            panic!("not a table")
        };
        assert_eq!(t.vars, ["a"]);
        assert_eq!(t.rows, [[Some("Back off between retries".to_string())]]);
        assert!(w.query("SELEKT nothing").is_err());
        assert!(w.query("INSERT DATA { <a:a> <a:b> <a:c> }").is_err());
    }

    #[test]
    fn a_page_with_broken_front_matter_is_kept_with_the_error() {
        let root = fixture("broken");
        write(
            &root.join("acme/app"),
            "wiki/ideas/bad.md",
            "---\ndescription: [a, b]\n---\n",
        );
        let w = build(&root);
        let bad = w
            .nodes
            .iter()
            .find(|n| n.rel == "wiki/ideas/bad")
            .expect("bad");
        assert!(bad.front.is_none() && bad.front_error.is_some());
    }

    #[test]
    fn a_missing_directory_is_an_empty_level() {
        let root = scratch("empty");
        let w = build(&root);
        assert!(w.nodes.is_empty());
        assert!(ask(
            &w,
            "ASK { <urn:iter:acme.app> iter:in <urn:iter:acme> }"
        ));
    }

    #[test]
    fn the_graph_exports_as_ntriples() {
        let w = build(&fixture("export"));
        let nt = w.ntriples().expect("dump");
        assert!(
            nt.contains("<urn:iter:ns:description>") && nt.contains("Back off between retries")
        );
    }

    #[test]
    fn an_id_resolves_back_to_its_node_and_is_prefixed_only_when_shadowed() {
        let root = fixture("ids");
        // the same path in the task and in the project: the nearest one is bare
        write(
            &root.join("acme/app"),
            "wiki/concepts/same.md",
            "---\ndescription: p\n---\n",
        );
        write(
            &root.join("acme/app"),
            "tasks/7/sketches/same.md",
            "---\ndescription: t\n---\n",
        );
        write(
            &root.join("acme/app/tasks/7"),
            "wiki/concepts/same.md",
            "---\ndescription: t\n---\n",
        );
        let w = build(&root);
        for from in 0..w.levels.len() {
            for node in 0..w.nodes.len() {
                let id = w.id(node, from);
                assert_eq!(w.node(from, &id), Some(node), "{id} from {from}");
            }
        }
        let task_page = w
            .nodes
            .iter()
            .position(|n| n.rel == "sketches/same")
            .expect("sketch");
        assert_eq!(w.id(task_page, 0), "sketches/same");
        assert_eq!(w.id(task_page, 1), "acme.app.task-7:sketches/same");
    }

    /// `cargo test --release -- --ignored --nocapture timing`
    #[test]
    #[ignore]
    fn timing_of_a_big_wiki() {
        use std::time::Instant;
        let root = big("timing", 5000);
        let t = Instant::now();
        let w = Wiki::build(
            vec![Level {
                kind: LevelKind::Project,
                label: "acme.app".into(),
                title: "app".into(),
                dir: root.join("acme/app"),
                parent: None,
            }],
            Ontology::load(None).expect("ontology"),
        )
        .expect("builds");
        let built = t.elapsed();
        let t = Instant::now();
        let found = crate::wiki::retrieve::find(&w, 0, "topic 17 t3", 800);
        let find = t.elapsed();
        let t = Instant::now();
        let near = crate::wiki::retrieve::neighbors(&w, 0, 10, 2, &[]);
        let neighbors = t.elapsed();
        let t = Instant::now();
        let issues = crate::wiki::check::check(&w, 0);
        let checked = t.elapsed();
        let t = Instant::now();
        let Answer::Table(rows) = w
            .query("SELECT (COUNT(*) AS ?n) WHERE { ?a iter:ref ?b . ?b iter:ref ?c }")
            .expect("query")
        else {
            panic!("table")
        };
        let sparql = t.elapsed();
        println!(
            "5000 pages: build {built:?} | find {find:?} ({} hits) | neighbors {neighbors:?} ({}) | check {checked:?} ({} issues) | 2-hop sparql {sparql:?} ({:?})",
            found.hits.len(),
            near.len(),
            issues.len(),
            rows.rows[0][0]
        );
    }

    #[test]
    fn an_ontology_folder_stays_inside_its_level() {
        assert!(stays_inside("wiki/ideas"));
        assert!(stays_inside("."));
        assert!(!stays_inside("../other"));
        assert!(!stays_inside("wiki/../../x"));
        assert!(!stays_inside("/etc"));
    }
}
