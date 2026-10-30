//! The prompts that tell an agent what to do with a task: plan it, work on it,
//! fix it, investigate it. Each is a markdown template with `{{placeholders}}`
//! filled from the task.
//!
//! The templates are data. The built-in ones can be replaced, so a task that
//! needs another structure of plan, or a project with its own way of working,
//! says so in a file: the first of `tasks/<id>/prompts/<kind>.md`,
//! `<project notes>/prompts/<kind>.md` and `<organization notes>/prompts/<kind>.md`
//! that exists is used, the built-in one if none does.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum)]
pub enum Kind {
    /// Read the task and write its plan; change no code
    Plan,
    /// Work through the plan
    Execute,
    /// Reproduce a defect, find its cause and fix it
    Fix,
    /// Answer a question about the project; change no code
    Investigate,
}

impl Kind {
    #[cfg(test)]
    pub const ALL: [Kind; 4] = [Kind::Plan, Kind::Execute, Kind::Fix, Kind::Investigate];

    pub fn key(self) -> &'static str {
        match self {
            Kind::Plan => "plan",
            Kind::Execute => "execute",
            Kind::Fix => "fix",
            Kind::Investigate => "investigate",
        }
    }

    pub fn builtin(self) -> &'static str {
        match self {
            Kind::Plan => include_str!("prompts/plan.md"),
            Kind::Execute => include_str!("prompts/execute.md"),
            Kind::Fix => include_str!("prompts/fix.md"),
            Kind::Investigate => include_str!("prompts/investigate.md"),
        }
    }
}

/// `word` as one shell word: as it is when that is safe, else in single
/// quotes. A task is named `<project>/<task>` and a task name may have spaces,
/// and `{{task}}` is what goes after `iter task ...` in a command.
pub fn shell_word(word: &str) -> String {
    let plain = |b: u8| b.is_ascii_alphanumeric() || b"_./:-@%+=,".contains(&b);
    if !word.is_empty() && word.bytes().all(plain) {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
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

const PLACEHOLDERS: [&str; 12] = [
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
pub fn render(template: &str, kind: Kind, ctx: &Context<'_>) -> Result<String, String> {
    let (mut out, mut rest) = (String::with_capacity(template.len()), template);
    while let Some(open) = rest.find("{{") {
        let Some(close) = rest[open + 2..].find("}}") else {
            break;
        };
        let key = rest[open + 2..open + 2 + close].trim();
        let Some(text) = value(ctx, key, kind) else {
            return Err(format!(
                "unknown placeholder {{{{{key}}}}} (known: {})",
                PLACEHOLDERS.join(", ")
            ));
        };
        out.push_str(&rest[..open]);
        out.push_str(&text);
        rest = &rest[open + 2 + close + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

/// The template to use for `kind`: the first `prompts/<kind>.md` in `dirs`
/// (nearest first), with where it came from, or the built-in one.
pub fn template(kind: Kind, dirs: &[PathBuf]) -> (Option<PathBuf>, String) {
    for dir in dirs {
        let path = dir.join("prompts").join(format!("{}.md", kind.key()));
        if let Ok(text) = std::fs::read_to_string(&path) {
            return (Some(path), text);
        }
    }
    (None, kind.builtin().to_string())
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
            let text = render(kind.builtin(), kind, &ctx(None, "", dir)).expect(kind.key());
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
            "'app/Fix it'|7|/notes/app/tasks/7|/code/app|## Goal\nx|wiki/a: a thing"
        );
        let mut c = ctx(None, "", dir);
        c.worktree = Some("/wt/fix-it");
        assert_eq!(render("{{worktree}}", Kind::Fix, &c).unwrap(), "/wt/fix-it");
    }

    #[test]
    fn the_task_is_one_shell_word() {
        assert_eq!(shell_word("app/fix-it"), "app/fix-it");
        assert_eq!(shell_word("app/Fix it"), "'app/Fix it'");
        assert_eq!(shell_word("app/It's"), r"'app/It'\''s'");
        assert_eq!(shell_word("a;rm -rf"), "'a;rm -rf'");
        assert_eq!(shell_word(""), "''");
    }

    #[test]
    fn an_unknown_placeholder_is_an_error_that_names_it() {
        let dir = Path::new("/x");
        let e = render("hello {{taks.name}}", Kind::Plan, &ctx(None, "", dir)).unwrap_err();
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
        let root = std::env::temp_dir().join(format!("iter-prompt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (task, project) = (root.join("task"), root.join("project"));
        for (dir, text) in [(&task, "from the task"), (&project, "from the project")] {
            std::fs::create_dir_all(dir.join("prompts")).unwrap();
            std::fs::write(dir.join("prompts/plan.md"), text).unwrap();
        }
        let dirs = [task.clone(), project.clone()];
        let (from, text) = template(Kind::Plan, &dirs);
        assert_eq!(
            (from, text.as_str()),
            (Some(task.join("prompts/plan.md")), "from the task")
        );
        // the task has no `fix` override, the project neither: built in
        let (from, text) = template(Kind::Fix, &dirs);
        assert_eq!((from, text), (None, Kind::Fix.builtin().to_string()));
        std::fs::remove_file(task.join("prompts/plan.md")).unwrap();
        assert_eq!(template(Kind::Plan, &dirs).1, "from the project");
    }
}
