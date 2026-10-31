//! The git-based review of what an agent wrote: pending changes, accept, revert.

use super::*;

pub fn git(target: &Target, args: &[&str]) -> Result<String> {
    process::output("git", Some(&target.notes_dir.display().to_string()), args)
}

/// One page of the wiki that differs from the last commit.
#[derive(Debug, PartialEq)]
pub struct Change {
    /// Relative to the notes folder (`wiki/ideas/x.md`).
    pub path: String,
    /// `new`, `changed` or `deleted`.
    pub kind: &'static str,
    /// A unified diff; for a new file, all of it as added lines.
    pub diff: String,
}

/// A path as git's pathspec takes it: that exact file, no globbing.
pub fn literal(path: &str) -> String {
    format!(":(literal){path}")
}

/// The pages (`.md`) under `wiki/` that are not committed, by path. What an
/// agent writes is pages; the settings and the ontology extension in the same
/// folder are the user's own and are not listed. Nothing is written, not even
/// git's index.
pub fn changes(target: &Target) -> Result<Vec<Change>> {
    // the notes folder may be inside a bigger repository: git reports paths
    // from the repository's root
    let prefix = git(target, &["rev-parse", "--show-prefix"])?
        .trim()
        .to_string();
    let status = git(
        target,
        &["status", "--porcelain", "-z", "-uall", "--", "wiki"],
    )?;
    let mut out = Vec::new();
    let mut entries = status.split('\0').filter(|e| e.len() > 3);
    while let Some(entry) = entries.next() {
        let (code, full) = entry.split_at(3);
        if code.starts_with('R') || code.starts_with('C') {
            entries.next(); // the old name
        }
        let path = full.strip_prefix(prefix.as_str()).unwrap_or(full);
        if !path.ends_with(".md") {
            continue;
        }
        let (kind, diff) = match code.trim() {
            "??" | "A" => {
                let text = std::fs::read_to_string(target.notes_dir.join(path)).unwrap_or_default();
                let body: String = text.lines().map(|l| format!("+{l}\n")).collect();
                ("new", format!("--- /dev/null\n+++ b/{path}\n{body}"))
            }
            c if c.contains('D') => (
                "deleted",
                git(
                    target,
                    &["diff", "HEAD", "--relative", "--", &literal(path)],
                )
                .unwrap_or_default(),
            ),
            _ => (
                "changed",
                git(
                    target,
                    &["diff", "HEAD", "--relative", "--", &literal(path)],
                )
                .unwrap_or_default(),
            ),
        };
        out.push(Change {
            path: path.to_string(),
            kind,
            diff,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// What the wiki of a project has that nobody has accepted, and what is
/// still to be ingested: the page of a task shows it.
pub struct Review {
    pub changes: Vec<Change>,
    pub waiting: Vec<ingest::Waiting>,
    pub mode: Mode,
    /// Whether the notes folder is in a git repository (else there is
    /// nothing to review against, and an ingest is refused).
    pub in_git: bool,
    /// An agent is writing in the wiki now.
    pub busy: bool,
    pub notes_dir: PathBuf,
}

pub fn review(db: &Db, project: &Project) -> Result<Review> {
    let target = Target::new(db, project.clone())?;
    let in_git = target.in_git();
    Ok(Review {
        changes: if in_git {
            changes(&target)?
        } else {
            Vec::new()
        },
        waiting: ingest::waiting(&target.wiki),
        mode: ingest::mode(&target.wiki.levels).0,
        in_git,
        busy: busy(&target),
        notes_dir: target.notes_dir.clone(),
    })
}

/// Commits (`accept`) or throws away the pages that are pending under
/// `wiki/` -- those and no other file. The paths it touched.
pub fn settle(db: &Db, project: &Project, accept: bool) -> Result<Vec<String>> {
    let target = Target::new(db, project.clone())?;
    target.require_git()?;
    if busy(&target) {
        return Err(AgentError::WikiWriting.into());
    }
    let list = changes(&target)?;
    if list.is_empty() {
        return Ok(Vec::new());
    }
    let paths: Vec<String> = list.iter().map(|c| literal(&c.path)).collect();
    let with = |first: &[&str]| -> Result<String> {
        let mut args: Vec<&str> = first.to_vec();
        args.push("--");
        args.extend(paths.iter().map(String::as_str));
        git(&target, &args)
    };
    if accept {
        with(&["add"])?;
        with(&["commit", "-m", "wiki: ingest"])?;
    } else {
        // out of the index first (a commit that failed leaves files staged),
        // then back to the last commit, and the pages that are new are removed
        let _ = with(&["reset", "-q", "HEAD"]);
        for c in list.iter().filter(|c| c.kind != "new") {
            git(&target, &["checkout", "HEAD", "--", &literal(&c.path)])?;
        }
        for c in list.iter().filter(|c| c.kind == "new") {
            std::fs::remove_file(target.notes_dir.join(&c.path))?;
        }
    }
    Ok(list.into_iter().map(|c| c.path).collect())
}
