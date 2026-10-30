//! The wiki: markdown files an agent reads to get onboarded on a task, seen as
//! a graph.
//!
//! The markdown is the only source of truth. Every call rebuilds the graph in
//! memory (`graph::Wiki::build`), so the two cannot disagree and there is no
//! index file. What goes into the graph:
//!
//! * the **class** of a page comes from the folder it is in, as the ontology
//!   says (`ontology.ttl`); front matter never repeats it;
//! * the **front matter** of a page (strict, `frontmatter.rs`) gives its
//!   abstract, tags, confidence, update time and its typed relations;
//! * the **`[[links]]`** in its body are `iter:ref` relations;
//! * organizations, projects and tasks are nodes too, built from the levels
//!   the caller passes (the database says which exist), and everything is
//!   `iter:in` the level whose folder holds it.
//!
//! The graph is an in-memory `oxigraph` store queried with SPARQL. The
//! ontology is OWL, but oxigraph reasons about none of it, so `graph::entail`
//! applies the small OWL 2 RL subset the ontology uses before any query.
//!
//! Raw notes (`iter:Note`) are listed but never read: they are the user's, and
//! free.

pub mod check;
pub mod frontmatter;
pub mod graph;
pub mod mv;
pub mod ontology;
pub mod retrieve;
pub mod schema;
pub mod scope;

#[cfg(test)]
pub mod testutil;
