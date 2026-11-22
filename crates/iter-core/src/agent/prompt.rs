//! Prompts that tell an agent what to do with a task: instructions (`instruction.rs`) filled from the task.
//! An instruction is the first `instructions/<name>.md` in the task, project notes, organization notes or config folder.

use crate::error::{AgentError, AgentResult};
use crate::process::shell_word;
use std::path::{Path, PathBuf};

pub struct Context<'a> {
    pub task: &'a str,
    pub task_name: &'a str,
    pub task_description: &'a str,
    pub task_id: i64,
    pub task_dir: &'a Path,
    pub project_name: &'a str,
    pub project_path: &'a str,
    pub worktree: Option<&'a str>,
    pub plan_path: &'a Path,
    pub plan: Option<&'a str>,
    /// Wiki onboarding text; empty when none.
    pub onboarding: &'a str,
    /// The project's main branch (`project.default_branch`).
    pub branch_main: &'a str,
    /// The wiki ingest text for the task; empty when the project has no wiki (or it was not asked for).
    pub ingest: &'a str,
}

pub const PLACEHOLDERS: [&str; 14] = [
    "task",
    "task.name",
    "task.description",
    "task.id",
    "task.dir",
    "project.name",
    "project.path",
    "worktree",
    "plan.path",
    "plan",
    "onboarding",
    "kind",
    "branch.main",
    "ingest",
];

fn value(ctx: &Context<'_>, key: &str, name: &str) -> Option<String> {
    let or = |s: &str, none: &str| match s.trim() {
        "" => none.to_string(),
        s => s.to_string(),
    };
    Some(match key {
        "task" => shell_word(ctx.task),
        "task.name" => ctx.task_name.to_string(),
        "task.description" => or(ctx.task_description, "(the task has no description)"),
        "task.id" => ctx.task_id.to_string(),
        "task.dir" => ctx.task_dir.display().to_string(),
        "project.name" => ctx.project_name.to_string(),
        "project.path" => ctx.project_path.to_string(),
        "worktree" => ctx
            .worktree
            .map_or_else(|| ctx.project_path.to_string(), String::from),
        "plan.path" => ctx.plan_path.display().to_string(),
        "plan" => or(ctx.plan.unwrap_or(""), "(there is no plan yet)"),
        "onboarding" => or(ctx.onboarding, "(nothing was found for this task)"),
        "kind" => name.to_string(),
        "branch.main" => ctx.branch_main.to_string(),
        "ingest" => or(ctx.ingest, "(the project has no wiki)"),
        _ => return None,
    })
}

/// `template` (the instruction `name`'s prompt) with its placeholders filled; `{{kind}}` is the
/// name. An unknown one is an error, not a literal `{{taks.name}}` in the prompt.
pub fn render(template: &str, name: &str, ctx: &Context<'_>) -> AgentResult<String> {
    let (mut out, mut rest) = (String::with_capacity(template.len()), template);
    while let Some(open) = rest.find("{{") {
        let Some(close) = rest[open + 2..].find("}}") else {
            break;
        };
        let key = rest[open + 2..open + 2 + close].trim();
        let Some(text) = value(ctx, key, name) else {
            return Err(AgentError::UnknownPlaceholder {
                key: key.to_string(),
                known: PLACEHOLDERS.join(", "),
            });
        };
        out.push_str(&rest[..open]);
        out.push_str(&text);
        rest = &rest[open + 2 + close + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Where the instruction `name` lives under `dir`.
pub fn file(dir: &Path, name: &str) -> PathBuf {
    dir.join("instructions").join(format!("{name}.md"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{SHIPPED, shipped};

    fn ctx<'a>(plan: Option<&'a str>, onboarding: &'a str, dir: &'a Path) -> Context<'a> {
        Context {
            task: "app/Fix it",
            task_name: "Fix it",
            task_description: "It is broken.",
            task_id: 7,
            task_dir: dir,
            project_name: "app",
            project_path: "/code/app",
            worktree: None,
            plan_path: dir,
            plan,
            onboarding,
            branch_main: "main",
            ingest: "",
        }
    }

    /// The prompt of a shipped instruction: what follows its front matter.
    fn body(name: &str) -> String {
        let raw = shipped(name);
        crate::agent::instruction::split(&raw).1.to_string()
    }

    #[test]
    fn every_shipped_instruction_renders_and_leaves_no_placeholder() {
        let dir = Path::new("/notes/app/tasks/7");
        for name in SHIPPED {
            let text = render(&body(name), name, &ctx(None, "", dir))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!text.contains("{{"), "{name}: {text}");
            assert!(
                text.contains("Fix it") && text.contains("It is broken."),
                "{name}"
            );
            assert!(text.contains("(there is no plan yet)"), "{name}");
            assert!(text.contains("(nothing was found for this task)"), "{name}");
        }
    }

    #[test]
    fn the_context_fills_the_placeholders() {
        let dir = Path::new("/notes/app/tasks/7");
        let text = render(
            "{{task}}|{{task.id}}|{{task.dir}}|{{worktree}}|{{plan}}|{{ onboarding }}|{{kind}}",
            "review",
            &ctx(Some("## Goal\nx"), "wiki/a: a thing", dir),
        )
        .unwrap();
        assert_eq!(
            text,
            "app/Fix\\ it|7|/notes/app/tasks/7|/code/app|## Goal\nx|wiki/a: a thing|review"
        );
        let mut c = ctx(None, "", dir);
        c.worktree = Some("/wt/fix-it");
        assert_eq!(render("{{worktree}}", "fix", &c).unwrap(), "/wt/fix-it");
    }

    #[test]
    fn an_unknown_placeholder_is_an_error_that_names_it() {
        let dir = Path::new("/x");
        let e = render("hello {{taks.name}}", "plan", &ctx(None, "", dir)).unwrap_err();
        assert!(matches!(&e, AgentError::UnknownPlaceholder { key, .. } if key == "taks.name"));
        let e = e.to_string();
        assert!(
            e.starts_with("unknown placeholder {{taks.name}} (known: task, "),
            "{e}"
        );
        assert_eq!(
            render("a {{ b", "plan", &ctx(None, "", dir)).unwrap(),
            "a {{ b"
        );
    }

    #[test]
    fn complete_leads_subagents_through_iter_runs_and_sketches() {
        let dir = Path::new("/notes/app/tasks/7");
        let text = render(&body("complete"), "complete", &ctx(None, "", dir)).unwrap();
        for name in ["test", "develop", "check"] {
            assert!(
                text.contains(&format!("iter task solve -i {name} --subagent")),
                "{name}: {text}"
            );
        }
        assert!(text.contains("iter task runs --close"));
        for sketch in ["plan", "rounds", "result"] {
            assert!(
                text.contains(&format!("iter note sketch new {sketch}")),
                "{sketch}"
            );
        }
        assert!(text.contains("iter task plan 7 --stdin"));
    }

    /// `ingest` only writes the wiki, from the ingest text.
    #[test]
    fn ingest_carries_the_ingest_text() {
        assert!(body("ingest").contains("{{ingest}}"));
    }

    /// The main branch and the ingest text are placeholders a template may use.
    #[test]
    fn branch_main_and_ingest_are_known_placeholders() {
        for key in ["branch.main", "ingest"] {
            assert!(PLACEHOLDERS.contains(&key), "{key}");
        }
    }

    /// `complete` closes the task with the user: commit, merge main in, merge into main,
    /// ingest by a subagent, commit the wiki.
    #[test]
    fn complete_closes_the_task_by_merging_and_ingesting() {
        let text = body("complete");
        for wanted in [
            "git merge {{branch.main}}",
            "git -C {{project.path}} merge",
            "iter task solve -i ingest --subagent",
            "iter wiki pending --accept -m \"update:",
            "## Commits",
            "iter task done",
        ] {
            assert!(text.contains(wanted), "{wanted}: {text}");
        }
        // the close comes after the result, not before it
        let result = text.find("**Result.**").expect("a Result step");
        let merge = text.find("git merge {{branch.main}}").unwrap_or(0);
        assert!(merge > result, "the merge must come after the result");
    }
}
