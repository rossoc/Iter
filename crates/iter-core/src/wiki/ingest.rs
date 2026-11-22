//! Turning notes into wiki pages: pending notes, the ingest prompts, and the `ingest` setting. The caller runs the agent.
//! The setting lives in `wiki/settings.yaml` of the project or its organization (inherited).

use super::check::{Dirty, dirty_notes};
use super::graph::{Level, Wiki};
use serde::Deserialize;
use std::path::{Path, PathBuf};

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

/// The mode of the first of `levels` (innermost first) that sets one, else `Manual`; an unreadable settings file is skipped and its reason returned.
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

#[derive(Debug, PartialEq)]
pub struct Waiting {
    /// As the wiki writes it, from the project level (`2026-10-27`).
    pub id: String,
    pub path: PathBuf,
    pub why: Dirty,
}

/// Project-level notes no page was made from since they changed, oldest name first.
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

pub struct Job<'a> {
    pub project: &'a str,
    pub notes_dir: &'a Path,
    /// Flags pointing `iter wiki` at this project (`--project app`), shell-quoted.
    pub scope: &'a str,
    /// `YYYY-MM-DDTHH:MM:SS`, local time: what `updated` is set to.
    pub now: &'a str,
    pub schema: &'a str,
}

fn fill(template: &str, job: &Job, extra: &[(&str, String)]) -> String {
    let own = [
        ("project", job.project.to_string()),
        ("notes.dir", job.notes_dir.display().to_string()),
        ("wiki.dir", job.notes_dir.join("wiki").display().to_string()),
        ("now", job.now.to_string()),
        ("scope", job.scope.to_string()),
    ];
    let pairs = own
        .into_iter()
        .chain(extra.iter().map(|(k, v)| (*k, v.clone())));
    let text = pairs.fold(template.to_string(), |text, (key, value)| {
        text.replace(&format!("{{{{{key}}}}}"), &value)
    });
    // last, so nothing in the rules is read as a placeholder
    text.replace("{{schema}}", job.schema)
}

/// The prompt that ingests `notes`; `task` (`<project>/<task>`) is a finished task whose sketches,
/// notes and commits are read as sources too. Every source is named by the command that reads it.
pub fn ingest_prompt(job: &Job, notes: &[Waiting], task: Option<&str>) -> String {
    let dirty = if notes.is_empty() {
        "(none: only the task below)".to_string()
    } else {
        notes
            .iter()
            .map(|n| {
                let why = match n.why {
                    Dirty::New => "new",
                    Dirty::Changed => "changed since it was ingested",
                };
                format!(
                    "- `{}` ({why}): `iter note read {}`",
                    n.id,
                    crate::process::shell_word(&n.id)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let extra = match task {
        Some(task) => {
            let t = crate::process::shell_word(task);
            format!(
                "\n### The task {t}\n\n\
                 The task is finished. Keep what the project should remember: decisions and their \
                 reasons, causes, dead ends.\n\n\
                 - Its sketches, the working memory of the agents that did it: \
                 `iter note sketch list --task {t}`, then `iter note sketch read <name> --task {t}`.\n\
                 - Its notes, the user's: `iter note task list --task {t}`, then \
                 `iter note task read <name> --task {t}`.\n\
                 - Its commits: the short shas under `## Commits` in the sketch `result`; read \
                 each with `git show --stat <sha>` (what changed where; the message says why).\n\n\
                 The wiki cannot see a task's files, so a page made from one must not name it \
                 in `source` (that would be a dangling reference): say in the body which task \
                 and which sketch or commit it comes from, and leave `source` for the notes.\n"
            )
        }
        None => String::new(),
    };
    fill(
        include_str!("prompts/ingest.md"),
        job,
        &[("dirty", dirty), ("extra", extra)],
    )
}

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

    fn job() -> Job<'static> {
        Job {
            project: "app",
            notes_dir: Path::new("/n/app"),
            scope: "--project app",
            now: "2026-10-30T12:00:00",
            schema: "RULES",
        }
    }

    fn new_note() -> [Waiting; 1] {
        [Waiting {
            id: "2026-10-27".into(),
            path: PathBuf::from("/n/app/2026-10-27.md"),
            why: Dirty::New,
        }]
    }

    #[test]
    fn the_prompts_name_the_notes_and_leave_no_placeholder() {
        let (job, notes) = (job(), new_note());
        let text = ingest_prompt(&job, &notes, Some("app/Fix it"));
        assert!(text.contains("- `2026-10-27` (new): `iter note read 2026-10-27`"));
        assert!(text.contains("iter wiki check --project app"));
        assert!(
            text.contains(r"iter note sketch list --task app/Fix\ it") && text.contains("RULES")
        );
        assert!(!text.contains("/n/app/tasks"));
        assert!(text.contains("must not name it in `source`"));
        assert!(!text.contains("{{"));
        let concept = concept_prompt(&job, "retries", &["ideas/a".into(), "ideas/b".into()]);
        assert!(concept.contains("- `ideas/b`") && concept.contains("concepts/retries.md"));
        assert!(!concept.contains("{{"));
    }

    /// The ingest instruction is laid out like a CLAUDE.md (purpose, sources, workflow, rules)
    /// and reads only through iter commands.
    #[test]
    fn the_ingest_instruction_has_the_claude_md_sections_and_reads_through_iter() {
        let text = ingest_prompt(&job(), &new_note(), None);
        let mut at = 0;
        for section in ["## Purpose", "## Sources", "## Workflow", "## Rules"] {
            let found = text[at..]
                .find(section)
                .unwrap_or_else(|| panic!("{section} missing or out of order: {text}"));
            at += found + section.len();
        }
        for command in [
            "iter note read",
            "iter wiki find --project app",
            "iter wiki show",
            "iter wiki neighbors",
            "iter wiki check --project app",
        ] {
            assert!(text.contains(command), "{command}: {text}");
        }
        for word in ["contradicts", "supersedes"] {
            assert!(text.contains(word), "{word}: {text}");
        }
        // cross-links both ways
        assert!(text.contains("`nb`") || text.contains("nb:"), "{text}");
        assert!(text.contains("2026-10-27") && text.contains("RULES"));
        assert!(!text.contains("{{"), "{text}");
    }
}
