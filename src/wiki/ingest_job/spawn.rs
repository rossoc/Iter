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
        // `--flag=value`, so a name starting with `-` is a value, not a flag; a task goes by its id
        Some(task) => command.arg(format!("--task={}", task.id())),
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

/// The arguments of `iter wiki ingest` for `project` (or `task`, by id) and `opts`: what
/// [`ingest_command`] puts after the program.
///
/// stub: I2 (task 298): `unimplemented!()`; `ingest_command` builds the same list inline until
/// the develop step of round I2 calls this.
#[allow(dead_code)]
fn ingest_args(_project: &Project, _task: Option<&Task>, _opts: &HarnessChoice) -> Vec<String> {
    unimplemented!("stub: I2")
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

    /// The command line the core starts must parse as `iter wiki ingest`, with the same
    /// project, task, harness and extra arguments it was built from.
    #[test]
    fn the_ingest_command_line_parses_as_wiki_ingest() {
        use crate::agent::run::Harness;
        use crate::args::{Args, Command, WikiCommand};
        use clap::Parser;
        let project = crate::fixtures::project("-odd project");
        let task = Task {
            id: Some(42),
            ..crate::fixtures::task(1, "-Fix it")
        };
        let choices = [
            HarnessChoice::default(),
            HarnessChoice {
                agent: Some(Harness::default()),
                model: Some("-model-x".into()),
                extra: vec!["--flag".into(), "-v".into()],
            },
        ];
        for opts in &choices {
            for task in [Some(&task), None] {
                let mut argv = vec!["iter".to_string()];
                argv.extend(ingest_args(&project, task, opts));
                let parsed =
                    Args::try_parse_from(&argv).unwrap_or_else(|e| panic!("{argv:?}: {e}"));
                let Command::Wiki {
                    action:
                        WikiCommand::Ingest {
                            project: p,
                            task: t,
                            harness,
                            dry_run,
                            detach,
                        },
                } = parsed.command
                else {
                    panic!("{argv:?} is not `wiki ingest`");
                };
                assert_eq!((dry_run, detach), (false, false));
                assert_eq!(t, task.map(|t| t.id.unwrap().to_string()));
                assert_eq!(p, task.is_none().then(|| "-odd project".to_string()));
                assert_eq!(harness.model, opts.model);
                assert_eq!(harness.extra, opts.extra);
                assert_eq!(harness.agent.is_some(), opts.agent.is_some());
            }
        }
    }

    /// `ingest_command` is these arguments after the program: one list, not two.
    #[test]
    fn the_command_carries_exactly_those_arguments() {
        let project = crate::fixtures::project("app");
        let task = Task {
            id: Some(7),
            ..crate::fixtures::task(1, "x")
        };
        let opts = HarnessChoice {
            agent: None,
            model: Some("m".into()),
            extra: vec!["--".into()],
        };
        assert_eq!(
            args_with(&project, Some(&task), &opts),
            ingest_args(&project, Some(&task), &opts)
        );
    }

    fn args_with(project: &Project, task: Option<&Task>, opts: &HarnessChoice) -> Vec<String> {
        ingest_command(PathBuf::from("iter"), project, task, opts)
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }
}
