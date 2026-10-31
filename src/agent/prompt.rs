//! The prompts that tell an agent what to do with a task: plan it, work on it,
//! fix it, investigate it. Each is a markdown template with `{{placeholders}}`
//! filled from the task.
//!
//! The templates are data, and are called instructions (`instruction.rs`). The
//! built-in ones can be replaced, so a task that needs another structure of
//! plan, or a project with its own way of working, says so in a file: the
//! first of `tasks/<id>/instructions/<kind>.md`,
//! `<project notes>/instructions/<kind>.md`,
//! `<organization notes>/instructions/<kind>.md` and
//! `<iter's config folder>/instructions/<kind>.md` that exists is used, the
//! built-in one if none does.

use crate::agent::error::{AgentError, AgentResult};
use std::path::{Path, PathBuf};

/// What an agent is asked to do. Saved in a run's record, where the names
/// older versions wrote (`execute`, `investigate`) still read.
#[derive(
    Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Read the task and write its plan; change no code
    Plan,
    /// Work through the plan's development steps
    #[value(alias = "execute")]
    #[serde(alias = "execute")]
    Develop,
    /// Write and run the tests for the work
    Test,
    /// Check the work against the plan; change no code
    #[value(alias = "investigate")]
    #[serde(alias = "investigate")]
    Check,
    /// Reproduce a defect, find its cause and fix it
    Fix,
}

impl Kind {
    #[cfg(test)]
    pub const ALL: [Kind; 5] = [
        Kind::Plan,
        Kind::Develop,
        Kind::Test,
        Kind::Check,
        Kind::Fix,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Kind::Plan => "plan",
            Kind::Develop => "develop",
            Kind::Test => "test",
            Kind::Check => "check",
            Kind::Fix => "fix",
        }
    }

    /// Whether the work changes code, and so happens in the task's worktree.
    pub fn changes_code(self) -> bool {
        matches!(self, Kind::Develop | Kind::Test | Kind::Fix)
    }

    pub fn builtin(self) -> &'static str {
        match self {
            Kind::Plan => include_str!("prompts/plan.md"),
            Kind::Develop => include_str!("prompts/develop.md"),
            Kind::Test => include_str!("prompts/test.md"),
            Kind::Check => include_str!("prompts/check.md"),
            Kind::Fix => include_str!("prompts/fix.md"),
        }
    }
}

/// `word` as one shell word: a backslash before every character the shell
/// would read as something else (`app/Fix\ it`). A task is named
/// `<project>/<task>` and a task name may have spaces, and `{{task}}` is what
/// goes after `iter task ...` in a command.
pub fn shell_word(word: &str) -> String {
    if word.is_empty() {
        return "''".to_string();
    }
    let plain = |c: char| c.is_ascii_alphanumeric() || "_./:-@%+=,".contains(c);
    let mut out = String::new();
    for c in word.chars() {
        // a newline cannot be escaped: quoted, it is one word again
        if c == '\n' {
            out.push_str("'\n'");
        } else if plain(c) || !c.is_ascii() {
            out.push(c);
        } else {
            out.push('\\');
            out.push(c);
        }
    }
    out
}

/// What a template can say about a task.
pub struct Context<'a> {
    /// `<project>/<task>`: what the `iter task ...` commands take.
    pub task: &'a str,
    pub task_name: &'a str,
    pub task_description: &'a str,
    pub task_id: i64,
    pub task_dir: &'a Path,
    pub project_name: &'a str,
    pub project_path: &'a str,
    /// The task's worktree, when it has one.
    pub worktree: Option<&'a str>,
    pub plan_path: &'a Path,
    /// The plan's text, when there is one.
    pub plan: Option<&'a str>,
    /// What the project knows that bears on the task (any text; empty when
    /// nothing was found or nothing was asked).
    pub onboarding: &'a str,
}

pub const PLACEHOLDERS: [&str; 12] = [
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
];

fn value(ctx: &Context<'_>, key: &str, kind: Kind) -> Option<String> {
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
        "kind" => kind.key().to_string(),
        _ => return None,
    })
}

/// `template` with its `{{placeholders}}` filled. An unknown one is an error
/// naming it and the known ones: a typo in a template should not reach an
/// agent as a literal `{{taks.name}}`.
pub fn render(template: &str, kind: Kind, ctx: &Context<'_>) -> AgentResult<String> {
    let (mut out, mut rest) = (String::with_capacity(template.len()), template);
    while let Some(open) = rest.find("{{") {
        let Some(close) = rest[open + 2..].find("}}") else {
            break;
        };
        let key = rest[open + 2..open + 2 + close].trim();
        let Some(text) = value(ctx, key, kind) else {
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

/// The template to use for `kind`: the prompt of the first
/// `instructions/<kind>.md` in `dirs` (nearest first), with where it came
/// from, or of the built-in one. The front matter is not part of it.
pub fn template(kind: Kind, dirs: &[PathBuf]) -> (Option<PathBuf>, String) {
    for dir in dirs {
        let path = crate::agent::instruction::file(dir, kind.key());
        if let Ok(text) = std::fs::read_to_string(&path) {
            return (
                Some(path),
                crate::agent::instruction::split(&text)
                    .1
                    .trim_start_matches('\n')
                    .to_string(),
            );
        }
    }
    (
        None,
        crate::agent::instruction::split(kind.builtin())
            .1
            .trim_start_matches('\n')
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
        }
    }

    #[test]
    fn every_builtin_renders_and_leaves_no_placeholder() {
        let dir = Path::new("/notes/app/tasks/7");
        for kind in Kind::ALL {
            let text = render(kind.builtin(), kind, &ctx(None, "", dir))
                .unwrap_or_else(|e| panic!("{}: {e}", kind.key()));
            assert!(!text.contains("{{"), "{}: {text}", kind.key());
            assert!(text.contains("Fix it") && text.contains("It is broken."));
            assert!(text.contains("(there is no plan yet)"));
            assert!(text.contains("(nothing was found for this task)"));
        }
    }

    #[test]
    fn the_context_fills_the_placeholders() {
        let dir = Path::new("/notes/app/tasks/7");
        let text = render(
            "{{task}}|{{task.id}}|{{task.dir}}|{{worktree}}|{{plan}}|{{ onboarding }}",
            Kind::Plan,
            &ctx(Some("## Goal\nx"), "wiki/a: a thing", dir),
        )
        .unwrap();
        // no worktree yet: the project's own folder
        assert_eq!(
            text,
            "app/Fix\\ it|7|/notes/app/tasks/7|/code/app|## Goal\nx|wiki/a: a thing"
        );
        let mut c = ctx(None, "", dir);
        c.worktree = Some("/wt/fix-it");
        assert_eq!(render("{{worktree}}", Kind::Fix, &c).unwrap(), "/wt/fix-it");
    }

    #[test]
    fn the_task_is_one_shell_word() {
        assert_eq!(shell_word("app/fix-it"), "app/fix-it");
        assert_eq!(shell_word("app/Fix it"), r"app/Fix\ it");
        assert_eq!(shell_word("app/It's"), r"app/It\'s");
        assert_eq!(shell_word("a;rm -rf"), r"a\;rm\ -rf");
        assert_eq!(shell_word("café au"), r"café\ au");
        assert_eq!(shell_word(""), "''");
    }

    #[test]
    fn an_unknown_placeholder_is_an_error_that_names_it() {
        let dir = Path::new("/x");
        let e = render("hello {{taks.name}}", Kind::Plan, &ctx(None, "", dir)).unwrap_err();
        assert!(matches!(&e, AgentError::UnknownPlaceholder { key, .. } if key == "taks.name"));
        let e = e.to_string();
        assert!(
            e.starts_with("unknown placeholder {{taks.name}} (known: task, "),
            "{e}"
        );
        // an unclosed brace pair is just text
        assert_eq!(
            render("a {{ b", Kind::Plan, &ctx(None, "", dir)).unwrap(),
            "a {{ b"
        );
    }

    #[test]
    fn the_nearest_override_wins_and_the_builtin_is_the_default() {
        let root = crate::fixtures::scratch("prompt");
        let (task, project) = (root.join("task"), root.join("project"));
        for (dir, text) in [(&task, "from the task"), (&project, "from the project")] {
            std::fs::create_dir_all(dir.join("instructions")).unwrap();
            std::fs::write(dir.join("instructions/plan.md"), text).unwrap();
        }
        let dirs = [task.clone(), project.clone()];
        let (from, text) = template(Kind::Plan, &dirs);
        assert_eq!(
            (from, text.as_str()),
            (Some(task.join("instructions/plan.md")), "from the task")
        );
        // the task has no `fix` override, the project neither: built in
        let (from, text) = template(Kind::Fix, &dirs);
        assert_eq!(from, None);
        assert!(text.starts_with("You are fixing a defect"), "{text}");
        std::fs::remove_file(task.join("instructions/plan.md")).unwrap();
        assert_eq!(template(Kind::Plan, &dirs).1, "from the project");
    }

    /// A run written by an older version still reads: the kinds were `execute`
    /// and `investigate` before they were `develop` and `check`.
    #[test]
    fn a_kind_reads_the_names_older_runs_were_saved_with() {
        let read = |word: &str| serde_yaml::from_str::<Kind>(word).unwrap();
        assert_eq!(read("execute"), Kind::Develop);
        assert_eq!(read("investigate"), Kind::Check);
        for kind in Kind::ALL {
            assert_eq!(read(kind.key()), kind);
            assert_eq!(serde_yaml::to_string(&kind).unwrap().trim(), kind.key());
        }
        assert!(serde_yaml::from_str::<Kind>("nonsense").is_err());
    }
}
