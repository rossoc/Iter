//! Turning notes into wiki pages: which notes are waiting, the prompts that
//! tell an agent how, and the one setting (`ingest`) that says when to do it
//! on its own. Running the agent is the caller's; this only reads and words.
//!
//! The setting lives in `wiki/settings.yaml` of the project (or of its
//! organization, which the project inherits), so the wiki keeps its own
//! configuration and nothing outside it needs to know.

use super::check::{Dirty, dirty_notes};
use super::graph::{Level, Wiki};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// When ingesting happens without being asked.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Manual,
    OnTaskStart,
    OnTaskDone,
}

impl Mode {
    pub fn key(self) -> &'static str {
        match self {
            Mode::Manual => "manual",
            Mode::OnTaskStart => "on_task_start",
            Mode::OnTaskDone => "on_task_done",
        }
    }
}

#[derive(Deserialize, Default)]
struct Settings {
    ingest: Option<Mode>,
}

/// The mode the first of `levels` (innermost first) that sets one gives;
/// `Manual` when none does. A settings file that cannot be read is skipped,
/// and the reason is returned beside the mode.
pub fn mode(levels: &[Level]) -> (Mode, Vec<String>) {
    let mut problems = Vec::new();
    for level in levels {
        let path = level.dir.join("wiki").join("settings.yaml");
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        match serde_yaml::from_str::<Settings>(&text) {
            Ok(Settings { ingest: Some(mode) }) => return (mode, problems),
            Ok(_) => {}
            Err(e) => problems.push(format!("{}: {e}", path.display())),
        }
    }
    (Mode::Manual, problems)
}

/// A note waiting to be ingested.
#[derive(Debug, PartialEq)]
pub struct Waiting {
    /// As the wiki writes it, from the project level (`2026-10-27`).
    pub id: String,
    pub path: PathBuf,
    pub why: Dirty,
}

/// The notes of the project level (index 0 of the wiki's levels) that no page
/// has been made from since they changed, oldest name first.
pub fn waiting(wiki: &Wiki) -> Vec<Waiting> {
    let mut out: Vec<Waiting> = dirty_notes(wiki)
        .into_iter()
        .filter(|(i, _)| wiki.nodes[*i].level == 0)
        .map(|(i, why)| Waiting {
            id: wiki.id(i, 0),
            path: wiki.nodes[i].path.clone(),
            why,
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// What a prompt is filled in with.
pub struct Job<'a> {
    pub project: &'a str,
    pub notes_dir: &'a Path,
    /// The flags that point `iter wiki` at this project (`--project app`),
    /// already quoted for a shell.
    pub scope: &'a str,
    /// `YYYY-MM-DDTHH:MM:SS`, local time: what `updated` is set to.
    pub now: &'a str,
    /// The rules, from `schema::render`.
    pub schema: &'a str,
}

fn fill(template: &str, job: &Job, extra: &[(&str, String)]) -> String {
    let mut text = template.to_string();
    let wiki_dir = job.notes_dir.join("wiki").display().to_string();
    let mut pairs = vec![
        ("project", job.project.to_string()),
        ("notes.dir", job.notes_dir.display().to_string()),
        ("wiki.dir", wiki_dir),
        ("now", job.now.to_string()),
        ("scope", job.scope.to_string()),
    ];
    pairs.extend(extra.iter().map(|(k, v)| (*k, v.clone())));
    for (key, value) in pairs {
        text = text.replace(&format!("{{{{{key}}}}}"), &value);
    }
    // last, so nothing in the rules is read as a placeholder
    text.replace("{{schema}}", job.schema)
}

/// The prompt that ingests `notes`. `sketches` is the folder of a finished
/// task's sketches, read as notes too.
pub fn ingest_prompt(job: &Job, notes: &[Waiting], sketches: Option<&Path>) -> String {
    let dirty = if notes.is_empty() {
        "(none: only the sketches below)".to_string()
    } else {
        notes
            .iter()
            .map(|n| {
                let why = match n.why {
                    Dirty::New => "new",
                    Dirty::Changed => "changed since it was ingested",
                };
                let rel = n.path.strip_prefix(job.notes_dir).unwrap_or(&n.path);
                format!("- `{}` ({why}): {}", n.id, rel.display())
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let extra = match sketches {
        Some(dir) => format!(
            "\n## Sketches of a finished task\n\nThe agent that worked on the task wrote these. Read the files in {} and keep what the project should remember: decisions and their reasons, causes, dead ends. The project's wiki cannot see a task's folder, so a page made from a sketch must not name it in `source` (that would be a dangling reference): say in the body which task and which sketch it comes from, and leave `source` for the notes.\n\n",
            dir.display()
        ),
        None => String::new(),
    };
    fill(
        include_str!("prompts/ingest.md"),
        job,
        &[("dirty", dirty), ("extra", extra)],
    )
}

/// The prompt that builds the concept `name` from `ideas` (their ids).
pub fn concept_prompt(job: &Job, name: &str, ideas: &[String]) -> String {
    let list = ideas
        .iter()
        .map(|i| format!("- `{i}`"))
        .collect::<Vec<_>>()
        .join("\n");
    fill(
        include_str!("prompts/concept.md"),
        job,
        &[("ideas", list), ("name", name.to_string())],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::ontology::LevelKind;

    fn level(name: &str, settings: Option<&str>) -> Level {
        let dir = crate::fixtures::scratch(&format!("ingest-{name}"));
        std::fs::create_dir_all(dir.join("wiki")).unwrap();
        if let Some(text) = settings {
            std::fs::write(dir.join("wiki/settings.yaml"), text).unwrap();
        }
        Level {
            kind: LevelKind::Project,
            label: name.into(),
            title: name.into(),
            dir,
            parent: None,
        }
    }

    #[test]
    fn the_nearest_level_that_sets_a_mode_decides() {
        let project = level("p1", None);
        let org = level("o1", Some("ingest: on_task_done\n"));
        assert_eq!(mode(&[project, org]).0, Mode::OnTaskDone);
        let own = level("p2", Some("ingest: on_task_start\n"));
        let org = level("o2", Some("ingest: on_task_done\n"));
        assert_eq!(mode(&[own, org]).0, Mode::OnTaskStart);
        assert_eq!(mode(&[level("p3", None)]), (Mode::Manual, vec![]));
    }

    #[test]
    fn a_broken_settings_file_is_reported_and_skipped() {
        let broken = level("p4", Some("ingest: sometimes\n"));
        let org = level("o4", Some("ingest: on_task_done\n"));
        let (mode, problems) = mode(&[broken, org]);
        assert_eq!(mode, Mode::OnTaskDone);
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn the_prompts_name_the_notes_and_leave_no_placeholder() {
        let job = Job {
            project: "app",
            notes_dir: Path::new("/n/app"),
            scope: "--project app",
            now: "2026-10-30T12:00:00",
            schema: "RULES",
        };
        let notes = [Waiting {
            id: "2026-10-27".into(),
            path: PathBuf::from("/n/app/2026-10-27.md"),
            why: Dirty::New,
        }];
        let text = ingest_prompt(&job, &notes, Some(Path::new("/n/app/tasks/3/sketches")));
        assert!(text.contains("- `2026-10-27` (new): 2026-10-27.md"));
        assert!(text.contains("iter wiki check --project app"));
        assert!(text.contains("/n/app/tasks/3/sketches") && text.contains("RULES"));
        assert!(text.contains("must not name it in `source`"));
        assert!(!text.contains("{{"));
        let concept = concept_prompt(&job, "retries", &["ideas/a".into(), "ideas/b".into()]);
        assert!(concept.contains("- `ideas/b`") && concept.contains("concepts/retries.md"));
        assert!(!concept.contains("{{"));
    }
}
