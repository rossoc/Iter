//! The wiki: markdown files an agent reads to get onboarded on a task, seen as a graph.
//! The markdown is the only source of truth: every call rebuilds the in-memory graph (`graph::Wiki::build`), so there is no index file.
//! Model and entailment: `doc/architecture_agent_wiki.md`.

pub mod check;
pub mod frontmatter;
pub mod graph;
pub mod ingest;
pub mod ingest_job;
pub mod mv;
pub mod ontology;
pub mod retrieve;
pub mod schema;
pub mod scope;

#[cfg(test)]
pub mod testutil;
