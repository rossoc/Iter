//! `iter task plan` and `iter task prompt`: what the command line says about the plan and the prompts; the files are `agent::taskdir`'s.

use crate::args::{PlanArgs, PromptArgs};
use crate::editor::edit_text;
use crate::output::emit;
use iter_core::agent::plan::{self, Plan, Progress, State};
use iter_core::agent::task_prompt::{locate, prompt_text};
use iter_core::agent::taskdir::{TaskDir, mark_step, save_plan};
use iter_core::app::App;
use iter_core::db::Table;
use iter_core::error::AgentError;
use iter_core::error::Result;
use serde::Serialize;
use std::fmt::Write as _;
use std::io::Read;

#[derive(Serialize)]
struct PlanView {
    task: String,
    path: String,
    exists: bool,
    plan: Plan,
    progress: Progress,
    problems: Vec<String>,
}

fn view(task: &str, dir: &TaskDir, content: Option<&str>) -> PlanView {
    let text = content.unwrap_or("");
    PlanView {
        task: task.to_string(),
        path: dir.plan_path().display().to_string(),
        exists: content.is_some(),
        plan: plan::parse(text),
        progress: plan::parse(text).progress(),
        problems: if content.is_some() {
            plan::problems(text)
        } else {
            Vec::new()
        },
    }
}

fn progress_line(p: &Progress) -> String {
    let mut line = format!("{}/{} steps done", p.done, p.total);
    if p.doing > 0 {
        let _ = write!(line, ", {} in progress", p.doing);
    }
    if p.verify_total > 0 {
        let _ = write!(line, " (checks {}/{})", p.verify_done, p.verify_total);
    }
    line
}

/// `iter task plan`: alone it edits (or, with no terminal to edit in, prints: an agent asking for
/// its plan must not wait on an editor it cannot see), `--show` prints, `--stdin` replaces,
/// `--mark` ticks a step.
pub fn plan(app: &App, req: &PlanArgs) -> Result<()> {
    let (project, task, dir) = locate(app, req.task.as_deref())?;
    let name = task.label(&project).to_string();
    let word = task.id().to_string();
    if let [step, state] = req.mark.as_slice() {
        let state = <State as clap::ValueEnum>::from_str(state, true)
            .map_err(|_| AgentError::UnknownState(state.clone()))?;
        let p = mark_step(&dir, step, state)?;
        println!("{step}: {} -- {}", state.label(), progress_line(&p));
        return Ok(());
    }
    use std::io::IsTerminal;
    let no_terminal = !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal();
    if req.show || (!req.stdin && no_terminal) {
        let content = dir.read_plan()?;
        let v = view(&name, &dir, content.as_deref());
        return emit(req.format, &v, || match &content {
            None => format!(
                "No plan yet. Write one with `iter task plan {word}`, or send an agent: `iter task prompt {word} -i plan`.\n"
            ),
            Some(text) => format!("{}\n\n{}\n", text.trim_end(), progress_line(&v.progress)),
        });
    }
    let text = if req.stdin {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Some(buf)
    } else {
        let start = match dir.read_plan()? {
            Some(existing) => existing,
            None => plan::skeleton(&task.description),
        };
        edit_text(&start)?
    };
    let Some(text) = text else {
        println!("no changes -- plan not saved");
        return Ok(());
    };
    let saved = save_plan(&dir, &text)?;
    for problem in &saved.problems {
        eprintln!("warning: {problem}");
    }
    println!("saved the plan of {name}: {} steps", saved.steps);
    Ok(())
}

/// `iter task prompt`: the prompt for the instruction, as it is, on standard output.
pub fn prompt_cmd(app: &App, a: &PromptArgs) -> Result<()> {
    let template = a.template.as_deref();
    print!(
        "{}",
        prompt_text(
            app,
            a.task.name(),
            &a.instruction,
            template,
            !a.no_onboarding
        )?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_plan_view_is_the_plan_and_its_progress() {
        let dir = TaskDir::new(PathBuf::from("/t"));
        let v = view(
            "app/Fix it",
            &dir,
            Some("## Goal\ng\n## Checklist\n- [x] {s1} a\n- [ ] {s2} b\n"),
        );
        assert!(v.exists && v.problems.is_empty());
        assert_eq!(progress_line(&v.progress), "1/2 steps done");
        let none = view("app/Fix it", &dir, None);
        assert!(!none.exists && none.plan.items.is_empty() && none.problems.is_empty());
    }
}
