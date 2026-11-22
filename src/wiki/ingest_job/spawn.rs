//! Running the agent, and starting an ingest as a detached process.

use super::*;

/// Runs the harness on `prompt` in the notes folder; the log goes to `.iter/` (git-ignored). Returns the log path.
pub fn run_agent(
    target: &Target,
    opts: &HarnessChoice,
    prompt: &str,
    what: &str,
) -> Result<(PathBuf, bool)> {
    let dot = target.notes_dir.join(".iter");
    std::fs::create_dir_all(&dot)?;
    std::fs::write(dot.join(".gitignore"), "*\n")?;
    let log_path = dot.join(format!("{what}-{}.log", now().replace(':', "")));
    let log = crate::utils::fs::create_private(&log_path)?;
    let _lock = lock(target)?;
    let harness = opts.agent.unwrap_or_default();
    let (program, args) = harness.command(&Invocation {
        model: opts.model.as_deref().unwrap_or(""),
        allowed: &crate::config::config().agent.allowed_tools,
        extra: &opts.extra,
        ..Invocation::default()
    });
    let env = [(
        crate::utils::resolve::ENV_PROJECT,
        crate::models::Table::id(&target.project).to_string(),
    )];
    let ok = exec::run_logged(program, &args, &target.notes_dir, &env, prompt, log)?.success();
    Ok((log_path, ok))
}

/// Starts `iter wiki ingest` for `project` (and `task`) as a process that outlives this one.
pub fn spawn_ingest(project: &Project, task: Option<&Task>, opts: &HarnessChoice) -> Result<()> {
    let mut command = ingest_command(std::env::current_exe()?, project, task, opts);
    exec::spawn_detached(&mut command)?;
    Ok(())
}

/// The command [`spawn_ingest`] runs, before it is started (so a test can read its arguments).
fn ingest_command(
    exe: PathBuf,
    project: &Project,
    task: Option<&Task>,
    opts: &HarnessChoice,
) -> std::process::Command {
    let mut command = std::process::Command::new(exe);
    command.args(["wiki", "ingest"]);
    match task {
        // `--flag=value`, so a name starting with `-` is a value, not a flag
        Some(task) => command.arg(format!("--task={}", task_display(project, task))),
        None => command.arg(format!("--project={}", project.name)),
    };
    if let Some(agent) = opts.agent {
        command.arg(format!("--agent={}", agent.key()));
    }
    if let Some(model) = &opts.model {
        command.arg(format!("--model={model}"));
    }
    if !opts.extra.is_empty() {
        command.arg("--").args(&opts.extra);
    }
    command
}

#[cfg(feature = "web")]
/// Starts an ingest of the project (and `task`'s sketches) in the background.
pub fn ingest_in_background(db: &Db, project: &Project, task: Option<&Task>) -> Result<()> {
    let target = Target::new(db, project.clone())?;
    target.require_git()?;
    if busy(&target) {
        return Err(AgentError::IngestRunning.into());
    }
    let none = HarnessChoice::default();
    spawn_ingest(project, task, &none)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(project: &Project, task: Option<&Task>) -> Vec<String> {
        ingest_command(
            PathBuf::from("iter"),
            project,
            task,
            &HarnessChoice::default(),
        )
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
    }

    /// R4: the task is passed by its id, not its title: titles repeat, and a title starting with
    /// `-` or holding a space is one more thing to get wrong.
    #[test]
    fn the_task_is_handed_to_the_ingest_by_id() {
        let project = crate::fixtures::project("app");
        let task = Task {
            id: Some(42),
            ..crate::fixtures::task(1, "Fix it")
        };
        assert_eq!(args(&project, Some(&task)), ["wiki", "ingest", "--task=42"]);
        assert_eq!(args(&project, None), ["wiki", "ingest", "--project=app"]);
    }
}
