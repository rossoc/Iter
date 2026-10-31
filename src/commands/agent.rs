//! `iter task plan` and `iter task prompt`: what the command line says about the plan and the prompts; the files are `agent::taskdir`'s.

use crate::agent::error::AgentError;
use crate::agent::plan::{self, Plan, Progress, State};
use crate::agent::prompt::{self, Kind};
use crate::agent::task_prompt::{locate, prompt_text};
use crate::agent::taskdir::{TaskDir, mark_step, save_plan};
use crate::app::App;
use crate::error::Result;
use crate::md_edit::edit_text;
use crate::utils::output::{Format as DataFormat, emit};
use crate::utils::resolve::task_display;
use serde::Serialize;
use std::io::Read;
use std::path::Path;

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
        line.push_str(&format!(", {} in progress", p.doing));
    }
    if p.verify_total > 0 {
        line.push_str(&format!(" (checks {}/{})", p.verify_done, p.verify_total));
    }
    line
}

/// What `iter task plan` was asked: alone it edits, `--show` prints,
/// `--stdin` replaces, `--mark` ticks a step.
pub struct PlanRequest<'a> {
    pub task: Option<&'a str>,
    pub show: bool,
    pub stdin: bool,
    /// `[step, state]`, or empty.
    pub mark: &'a [String],
    pub format: DataFormat,
}

pub fn plan(app: &App, req: &PlanRequest) -> Result<()> {
    let (project, task, dir) = locate(app, req.task)?;
    let name = task_display(&project, &task);
    let word = prompt::shell_word(&name);
    if let [step, state] = req.mark {
        let state = <State as clap::ValueEnum>::from_str(state, true)
            .map_err(|_| AgentError::UnknownState(state.clone()))?;
        let p = mark_step(&dir, step, state)?;
        println!("{step}: {} -- {}", state.label(), progress_line(&p));
        return Ok(());
    }
    if req.show {
        let content = dir.read_plan()?;
        let v = view(&name, &dir, content.as_deref());
        return emit(req.format, &v, || match &content {
            None => format!(
                "No plan yet. Write one with `iter task plan {word}`, or send an agent: `iter task prompt {word} --kind plan`.\n"
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

pub fn prompt_cmd(
    app: &App,
    task: Option<&str>,
    kind: Kind,
    template: Option<&Path>,
    onboarding_wanted: bool,
) -> Result<()> {
    print!(
        "{}",
        prompt_text(app, task, kind, template, onboarding_wanted)?
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
