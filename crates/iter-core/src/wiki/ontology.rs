//! The ontology (`ontology.ttl` plus a project's or organization's `wiki/ontology.ttl` extension) as an OWL graph,
//! and what the wiki needs from it: folder of each class, relation keys, entailment rules.

use crate::error::{IterError, Result};
use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::vocab::rdf as rdf_vocab;
use oxigraph::model::vocab::{rdf, rdfs};
use oxigraph::model::{Graph, NamedNode, NamedNodeRef, NamedOrBlankNodeRef, TermRef, TripleRef};
use std::collections::{HashMap, HashSet};

const BASE: &str = include_str!("ontology.ttl");

pub const NS: &str = "urn:iter:ns:";
pub const NODE: &str = "urn:iter:";
const OWL: &str = "http://www.w3.org/2002/07/owl#";

pub fn ns(local: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("{NS}{local}"))
}

fn owl(local: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("{OWL}{local}"))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum LevelKind {
    Organization,
    Project,
    Task,
}

impl LevelKind {
    pub fn key(self) -> &'static str {
        match self {
            LevelKind::Organization => "organization",
            LevelKind::Project => "project",
            LevelKind::Task => "task",
        }
    }

    fn of(key: &str) -> Option<LevelKind> {
        [LevelKind::Organization, LevelKind::Project, LevelKind::Task]
            .into_iter()
            .find(|k| k.key() == key)
    }

    pub fn class(self) -> NamedNode {
        ns(match self {
            LevelKind::Organization => "Organization",
            LevelKind::Project => "Project",
            LevelKind::Task => "Task",
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub kind: LevelKind,
    /// Relative to the level's directory: `.` is the root, a name ending in `.md` a file, else a folder.
    pub folder: String,
    pub class: NamedNode,
}

#[derive(Default, Debug)]
pub struct Rules {
    pub sub_class: Vec<(NamedNode, NamedNode)>,
    pub sub_property: Vec<(NamedNode, NamedNode)>,
    pub inverse: Vec<(NamedNode, NamedNode)>,
    pub symmetric: Vec<NamedNode>,
    pub transitive: Vec<NamedNode>,
}

pub struct Ontology {
    pub graph: Graph,
    pub places: Vec<Place>,
    pub rules: Rules,
    pub authorable: Vec<(String, NamedNode)>,
    pub pages: HashSet<NamedNode>,
}

impl Ontology {
    pub fn load(extension: Option<&str>) -> Result<Ontology> {
        let mut graph = Graph::default();
        read_into(&mut graph, BASE, "the built-in ontology")?;
        if let Some(ttl) = extension {
            read_into(&mut graph, ttl, "the ontology extension")?;
        }
        let mut onto = Ontology {
            graph,
            places: Vec::new(),
            rules: Rules::default(),
            authorable: Vec::new(),
            pages: HashSet::new(),
        };
        onto.derive()?;
        Ok(onto)
    }

    fn derive(&mut self) -> Result<()> {
        let g = &self.graph;
        let named = |t: TermRef<'_>| match t {
            TermRef::NamedNode(n) => Some(n.into_owned()),
            _ => None,
        };
        let pairs = |p: NamedNodeRef<'_>| -> Vec<(NamedNode, NamedNode)> {
            g.triples_for_predicate(p)
                .filter_map(|t| Some((named(t.subject.into())?, named(t.object)?)))
                .collect()
        };
        let typed = |class: &NamedNode| -> Vec<NamedNode> {
            g.subjects_for_predicate_object(rdf::TYPE, class)
                .filter_map(|s| named(s.into()))
                .collect()
        };

        self.rules = Rules {
            sub_class: pairs(rdfs::SUB_CLASS_OF),
            sub_property: pairs(rdfs::SUB_PROPERTY_OF),
            inverse: pairs(owl("inverseOf").as_ref()),
            symmetric: typed(&owl("SymmetricProperty")),
            transitive: typed(&owl("TransitiveProperty")),
        };

        let mut places = Vec::new();
        for t in g.triples_for_predicate(ns("place").as_ref()) {
            let (Some(class), TermRef::Literal(lit)) = (named(t.subject.into()), t.object) else {
                continue;
            };
            let Some((kind, folder)) = lit.value().split_once(':') else {
                return Err(bad_place(lit.value()));
            };
            let Some(kind) = LevelKind::of(kind) else {
                return Err(bad_place(lit.value()));
            };
            places.push(Place {
                kind,
                folder: folder.to_string(),
                class,
            });
        }
        places.sort_by(|a, b| (a.kind.key(), &a.folder).cmp(&(b.kind.key(), &b.folder)));
        self.places = places;

        let mut authorable = Vec::new();
        for t in g.triples_for_predicate(ns("authorable").as_ref()) {
            let (Some(prop), TermRef::Literal(lit)) = (named(t.subject.into()), t.object) else {
                continue;
            };
            if lit.value() == "true"
                && let Some(key) = prop.as_str().strip_prefix(NS)
            {
                authorable.push((key.to_string(), prop.clone()));
            }
        }
        authorable.sort();
        self.authorable = authorable;

        let page = ns("Page");
        self.pages = self.subclasses_of(&page);
        Ok(())
    }

    pub fn subclasses_of(&self, class: &NamedNode) -> HashSet<NamedNode> {
        let mut below: HashMap<&NamedNode, Vec<&NamedNode>> = HashMap::new();
        for (sub, sup) in &self.rules.sub_class {
            below.entry(sup).or_default().push(sub);
        }
        let mut seen = HashSet::from([class.clone()]);
        let mut todo = vec![class];
        while let Some(c) = todo.pop() {
            for sub in below.get(c).into_iter().flatten() {
                if seen.insert((*sub).clone()) {
                    todo.push(sub);
                }
            }
        }
        seen
    }

    pub fn superclasses_of(&self, class: &NamedNode) -> HashSet<NamedNode> {
        let mut seen = HashSet::from([class.clone()]);
        let mut todo = vec![class.clone()];
        while let Some(c) = todo.pop() {
            for (sub, sup) in &self.rules.sub_class {
                if *sub == c && seen.insert(sup.clone()) {
                    todo.push(sup.clone());
                }
            }
        }
        seen
    }

    pub fn constraint(&self, property: &NamedNode, which: NamedNodeRef<'_>) -> Vec<NamedNode> {
        self.graph
            .objects_for_subject_predicate(property, which)
            .filter_map(|t| match t {
                TermRef::NamedNode(n) => Some(n.into_owned()),
                _ => None,
            })
            .collect()
    }

    /// The values `property` may take when its range is an `owl:oneOf` enumeration; `None` for any.
    pub fn allowed_values(&self, property: &NamedNode) -> Option<Vec<String>> {
        let range = self
            .graph
            .objects_for_subject_predicate(property, rdfs::RANGE)
            .find_map(|t| match t {
                TermRef::NamedNode(n) => Some(n.into_owned()),
                _ => None,
            })?;
        let one_of = owl("oneOf");
        let mut list = self
            .graph
            .object_for_subject_predicate(&range, one_of.as_ref())?;
        let mut values = Vec::new();
        while list != TermRef::NamedNode(rdf_vocab::NIL) {
            let cell: NamedOrBlankNodeRef<'_> = match list {
                TermRef::NamedNode(n) => n.into(),
                TermRef::BlankNode(b) => b.into(),
                _ => return None,
            };
            match self
                .graph
                .object_for_subject_predicate(cell, rdf_vocab::FIRST)?
            {
                TermRef::Literal(l) => values.push(l.value().to_string()),
                _ => return None,
            }
            list = self
                .graph
                .object_for_subject_predicate(cell, rdf_vocab::REST)?;
        }
        Some(values)
    }

    pub fn relation_keys(&self) -> Vec<&str> {
        self.authorable.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn relation(&self, key: &str) -> Option<&NamedNode> {
        self.authorable
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, p)| p)
    }
}

fn bad_place(value: &str) -> IterError {
    IterError::CommandFailed(format!(
        "the ontology has a bad iter:place \"{value}\" (expected \"<organization|project|task>:<folder>\")"
    ))
}

fn read_into(graph: &mut Graph, ttl: &str, what: &str) -> Result<()> {
    for quad in RdfParser::from_format(RdfFormat::Turtle).for_reader(ttl.as_bytes()) {
        let quad = quad.map_err(|e| IterError::CommandFailed(format!("{what}: {e}")))?;
        graph.insert(TripleRef::new(&quad.subject, &quad.predicate, &quad.object));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_ontology_says_where_classes_live() {
        let o = Ontology::load(None).expect("loads");
        let at = |kind, folder: &str| {
            o.places
                .iter()
                .find(|p| p.kind == kind && p.folder == folder)
                .map(|p| p.class.as_str().to_string())
        };
        assert_eq!(
            at(LevelKind::Project, "wiki/ideas").as_deref(),
            Some("urn:iter:ns:Idea")
        );
        assert_eq!(
            at(LevelKind::Task, "plan.md").as_deref(),
            Some("urn:iter:ns:Plan")
        );
        assert_eq!(
            at(LevelKind::Organization, ".").as_deref(),
            Some("urn:iter:ns:Note")
        );
    }

    #[test]
    fn pages_are_the_classes_under_page() {
        let o = Ontology::load(None).expect("loads");
        for c in ["Idea", "Concept", "Sketch", "Plan", "Page"] {
            assert!(o.pages.contains(&ns(c)), "{c}");
        }
        assert!(!o.pages.contains(&ns("Note")));
    }

    #[test]
    fn relation_keys_are_the_authorable_properties() {
        let o = Ontology::load(None).expect("loads");
        assert_eq!(
            o.relation_keys(),
            [
                "contradicts",
                "include",
                "nb",
                "ref",
                "source",
                "supersedes"
            ]
        );
        assert_eq!(o.relation("nb").map(|p| p.as_str()), Some("urn:iter:ns:nb"));
    }

    #[test]
    fn an_enumerated_range_lists_the_allowed_values() {
        let o = Ontology::load(None).expect("loads");
        assert_eq!(
            o.allowed_values(&ns("confidence")),
            Some(vec!["high".to_string(), "medium".into(), "low".into()])
        );
        // a plain datatype takes anything, and so does a property with no range
        assert_eq!(o.allowed_values(&ns("description")), None);
        assert_eq!(o.allowed_values(&ns("nothing-like-it")), None);
    }

    #[test]
    fn the_rules_are_read_from_the_owl() {
        let o = Ontology::load(None).expect("loads");
        assert!(o.rules.sub_property.contains(&(ns("nb"), ns("ref"))));
        assert!(o.rules.symmetric.contains(&ns("contradicts")));
        assert!(o.rules.transitive.contains(&ns("in")));
        assert!(o.rules.inverse.contains(&(ns("include"), ns("includedIn"))));
    }

    #[test]
    fn an_extension_adds_a_class_with_its_place() {
        let ext = "@prefix iter: <urn:iter:ns:> . @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
                   iter:Decision rdfs:subClassOf iter:Page ; iter:place \"project:wiki/decisions\" .";
        let o = Ontology::load(Some(ext)).expect("loads");
        assert!(o.pages.contains(&ns("Decision")));
        assert!(o.places.iter().any(|p| p.folder == "wiki/decisions"));
    }

    #[test]
    fn a_bad_place_is_an_error() {
        let ext = "@prefix iter: <urn:iter:ns:> . iter:X iter:place \"nowhere:wiki\" .";
        assert!(Ontology::load(Some(ext)).is_err());
        assert!(Ontology::load(Some("this is not turtle")).is_err());
    }
}
