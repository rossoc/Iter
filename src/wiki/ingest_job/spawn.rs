//! Running the agent, and starting an ingest as a detached process.

use super::*;

/// Runs the harness on `prompt` in the notes folder, its output in a log
/// there (`.iter/`, which git is told to ignore). The log's path.
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
    let log = std::fs::File::create(&log_path)?;
    let _lock = lock(target)?;
    let harness = opts.agent.unwrap_or_default();
    let (program, args) = harness.command(&Invocation {
        prompt,
        model: opts.model.as_deref().unwrap_or(""),
        allowed: &crate::config::config().agent.allowed_tools,
        extra: &opts.extra,
        ..Invocation::default()
    });
    let ok = exec::run_logged(program, &args, &target.notes_dir, log)?.success();
    Ok((log_path, ok))
}

/// Starts `iter wiki ingest` for `project` (and `task`) as a process of its
/// own that outlives this one.
pub fn spawn_ingest(
    project: &Project,
    task: Option<&Task>,
    opts: &HarnessChoice,
) -> Result<()> {
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command.args(["wiki", "ingest"]);
    // a task names its project already
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
    exec::spawn_detached(&mut command)?;
    Ok(())
}

/// Starts an ingest of the project (and the sketches of `task`) in the
/// background, as the page's button does.
pub fn ingest_in_background(db: &Db, project: &Project, task: &Task) -> Result<()> {
    let target = Target::new(db, project.clone())?;
    target.require_git()?;
    if busy(&target) {
        return Err(AgentError::IngestRunning.into());
    }
    let none = HarnessChoice::default();
    spawn_ingest(project, Some(task), &none)
}
