//! The command lines the core builds for `iter` to run again (a detached worker, the ingest, the
//! tmux hooks) must parse as the subcommand they name: rename a subcommand and these fail, not a
//! process that exits 2 unseen. They live here because only `cli` may name `Args`.

use super::Run;
use super::args::{Args, Command, HookEvent, InternalCommand, TaskCommand, WikiCommand};
use clap::Parser;
use iter_core::agent::ingest_job::{HarnessChoice, ingest_args};
use iter_core::agent::solve::worker_args;
use iter_core::app::App;
use iter_core::fixtures::{last_subject, wiki_repo as repo};
use iter_core::models::Task;
use iter_core::tmux::{HOOKS, hook_args};

fn parse(argv: &[&str]) -> std::result::Result<Args, clap::Error> {
    Args::try_parse_from([&["iter", "wiki", "pending"], argv].concat())
}

/// Every hook iter installs must parse as the `internal hook` event of its name, with the
/// number of arguments that event takes: a renamed subcommand fails here, not in a tmux
/// that ignores a failing hook.
#[test]
fn every_installed_hook_parses_as_its_internal_event() {
    for (event, variables) in HOOKS {
        // what tmux fills the variables with; a leading dash is the case that broke
        let filled: String = variables
            .split(' ')
            .enumerate()
            .map(|(i, _)| format!("-v{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let mut argv = vec!["iter".to_string()];
        argv.extend(hook_args(event, &filled));
        let parsed = Args::try_parse_from(&argv).unwrap_or_else(|e| panic!("{argv:?}: {e}"));
        let Command::Internal {
            action: InternalCommand::Hook { event: parsed },
        } = parsed.command
        else {
            panic!("{argv:?} is not `internal hook`");
        };
        let name = match (&parsed, event) {
            (
                HookEvent::ClientSessionChanged {
                    client,
                    session,
                    previous,
                },
                "client-session-changed",
            ) => {
                assert_eq!((client.as_str(), session.as_str()), ("-v0", "-v1"));
                assert_eq!(previous.as_deref(), Some("-v2"));
                event
            }
            (HookEvent::ClientDetached { client }, "client-detached") => {
                assert_eq!(client, "-v0");
                event
            }
            (HookEvent::SessionClosed { session }, "session-closed") => {
                assert_eq!(session, "-v0");
                event
            }
            (other, event) => panic!("{event} parsed as {other:?}"),
        };
        assert_eq!(name, event);
    }
}

/// The command line the core starts must parse as `iter wiki ingest`, with the same
/// project, task, harness and extra arguments it was built from.
#[test]
fn the_ingest_command_line_parses_as_wiki_ingest() {
    use clap::Parser;
    use iter_core::agent::run::Harness;
    use {Args, Command, WikiCommand};
    let project = iter_core::fixtures::project("-odd project");
    let task = Task {
        id: Some(42),
        ..iter_core::fixtures::task(1, "-Fix it")
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
            let parsed = Args::try_parse_from(&argv).unwrap_or_else(|e| panic!("{argv:?}: {e}"));
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

/// The command line a detached worker is started with must parse as `iter task run-worker`:
/// rename the subcommand and this fails, instead of a worker that exits 2 unseen.
#[test]
fn the_worker_command_line_parses_as_run_worker() {
    use clap::Parser;
    use {Args, Command, TaskCommand};
    for extra in [vec![], vec!["--model".to_string(), "-x".to_string()]] {
        let mut argv = vec!["iter".to_string()];
        argv.extend(worker_args(42, 3, &extra));
        let parsed = Args::try_parse_from(&argv).unwrap_or_else(|e| panic!("{argv:?}: {e}"));
        match parsed.command {
            Command::Task {
                action:
                    Some(TaskCommand::RunWorker {
                        task_id,
                        run,
                        extra: passed,
                    }),
            } => assert_eq!((task_id, run, passed), (42, 3, extra)),
            other => panic!("{argv:?} parsed as {other:?}"),
        }
    }
}

/// R5: `iter wiki pending --accept -m <msg>` commits the pages with that message.
#[test]
fn accepting_with_a_message_uses_it() {
    let (db, _, notes) = repo("accept-message");
    std::fs::write(
        notes.join("wiki/ideas/a.md"),
        "---\ndescription: a\n---\nnew\n",
    )
    .unwrap();
    let message = "update: app/fix-it - 1a2b3c4";
    let args =
        parse(&["-p", "app", "--accept", "-m", message]).unwrap_or_else(|e| panic!("refused: {e}"));
    let Command::Wiki { action } = args.command else {
        panic!("not a wiki command")
    };
    action.run(&App::with_db(db)).unwrap();
    assert_eq!(last_subject(&notes), message);
}

/// R5: a message means nothing without `--accept`.
#[test]
fn a_message_without_accept_is_refused() {
    assert!(parse(&["--accept", "--message", "x"]).is_ok());
    for argv in [&["-m", "x"][..], &["--revert", "-m", "x"]] {
        assert!(parse(argv).is_err(), "{argv:?}");
    }
}
