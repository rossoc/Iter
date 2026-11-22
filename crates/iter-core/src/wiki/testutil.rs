//! What the wiki's tests start from: a directory holding an organization
//! `acme`, its project `acme.app` and that project's task 7, with a few pages.

use super::graph::{Level, Wiki};
use super::ontology::{LevelKind, Ontology};
use std::fs;
use std::path::{Path, PathBuf};

pub fn scratch(name: &str) -> PathBuf {
    let dir = crate::fixtures::scratch(&format!("wiki-{name}"));
    fs::create_dir_all(&dir).expect("dir");
    dir
}

pub fn write(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
    fs::write(path, text).expect("write");
}

pub fn levels(root: &Path) -> Vec<Level> {
    let level = |kind, label: &str, dir: PathBuf, parent| Level {
        kind,
        label: label.into(),
        title: label.into(),
        dir,
        parent,
    };
    vec![
        level(
            LevelKind::Task,
            "acme.app.task-7",
            root.join("acme/app/tasks/7"),
            Some(1),
        ),
        level(
            LevelKind::Project,
            "acme.app",
            root.join("acme/app"),
            Some(2),
        ),
        level(LevelKind::Organization, "acme", root.join("acme"), None),
    ]
}

pub fn build(root: &Path) -> Wiki {
    Wiki::build(levels(root), Ontology::load(None).expect("ontology")).expect("builds")
}

pub fn fixture(name: &str) -> PathBuf {
    let root = scratch(name);
    let app = root.join("acme/app");
    write(
        &app,
        "2026-10-27.md",
        "raw thoughts: [[nothing]] ---\n: not yaml",
    );
    write(
        &app,
        "wiki/ideas/retry.md",
        "---\ndescription: Retries need a budget\ntags: [net]\nsource: 2026-10-27\nupdated: 2026-10-28\n---\n# Retry budget\nsee [[concepts/backoff]]\n",
    );
    write(
        &app,
        "wiki/concepts/backoff.md",
        "---\ndescription: Back off between retries\ninclude: [ideas/retry]\nnb: ideas/retry\n---\nbody\n",
    );
    write(
        &root.join("acme"),
        "wiki/entities/ci.md",
        "---\ndescription: The CI\n---\n[[acme.app:concepts/backoff]] [[missing]]\n",
    );
    write(
        &app,
        "tasks/7/plan.md",
        "---\ndescription: Plan\n---\n## Goal\n",
    );
    write(
        &app,
        "tasks/7/sketches/s.md",
        "---\ndescription: Scratch\nref: concepts/backoff\n---\n",
    );
    root
}

/// `pages` pages and a tenth as many notes in `acme.app`, for timing; deterministic.
pub fn big(name: &str, pages: usize) -> PathBuf {
    let root = scratch(name);
    let app = root.join("acme/app");
    let notes = (pages / 10).max(1);
    let mut seed = 12345u64;
    let mut next = |n: usize| {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) as usize) % n
    };
    for n in 0..notes {
        write(&app, &format!("2026-01-{:02}-{n}.md", n % 28 + 1), "raw");
    }
    let folders = ["ideas", "ideas", "ideas", "concepts", "entities"];
    let path = |i: usize| format!("wiki/{}/p{i}", folders[i % folders.len()]);
    for i in 0..pages {
        let (a, b, c) = (next(pages), next(pages), next(pages));
        let text = format!(
            "---\ndescription: Page {i} says something about topic {}\ntags: [t{}, t{}, t{}]\nsource: [n]\nnb: [{}]\nupdated: 2026-10-01\n---\n# Page {i}\nSee [[{}]] and [[{}]].\n{}\n",
            next(200),
            next(50),
            next(50),
            next(50),
            path(a).trim_start_matches("wiki/"),
            path(b).trim_start_matches("wiki/"),
            path(c).trim_start_matches("wiki/"),
            "lorem ipsum dolor sit amet ".repeat(20),
        );
        write(&app, &format!("{}.md", path(i)), &text);
    }
    root
}
