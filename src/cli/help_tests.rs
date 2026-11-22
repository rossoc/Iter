//! `--help` of every subcommand, pinned (task 298, R9: "the same commands and `--help`"). The
//! reorganisation moves the argument types between modules and crates; none of that may change
//! what a person reads. The snapshot is `src/cli/help_snapshot.txt`; after a change to the help on
//! purpose, `ITER_UPDATE_HELP=1 cargo test help_tests` rewrites it.
//!
//! `serve` exists only with the `web` feature, so it is left out of the snapshot.

use clap::Command;

fn render(command: &mut Command, path: &str, out: &mut String) {
    let help = command.render_long_help().to_string();
    out.push_str(&format!("===== {path}\n"));
    for line in help
        .lines()
        .filter(|l| !l.trim_start().starts_with("serve "))
    {
        out.push_str(line.trim_end());
        out.push('\n');
    }
    for sub in command.get_subcommands_mut() {
        let name = sub.get_name().to_string();
        if name != "serve" && name != "help" {
            render(sub, &format!("{path} {name}"), out);
        }
    }
}

#[test]
fn the_help_of_every_subcommand_is_unchanged() {
    let mut command = super::run::command();
    command.build();
    let mut now = String::new();
    render(&mut command, "iter", &mut now);
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/src/cli/help_snapshot.txt");
    if std::env::var_os("ITER_UPDATE_HELP").is_some() {
        std::fs::write(file, &now).expect("snapshot written");
    }
    let before = std::fs::read_to_string(file).expect("src/help_snapshot.txt");
    if before != now {
        let first = before
            .lines()
            .zip(now.lines())
            .position(|(a, b)| a != b)
            .unwrap_or(before.lines().count().min(now.lines().count()));
        panic!(
            "the help changed near line {}:\n  was: {:?}\n  now: {:?}",
            first + 1,
            before.lines().nth(first),
            now.lines().nth(first)
        );
    }
}

/// Every subcommand has help to pin: a leaf the walk missed would pass the snapshot unseen.
#[test]
fn the_snapshot_covers_the_commands_a_person_runs() {
    let snapshot = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/cli/help_snapshot.txt"
    ))
    .expect("src/help_snapshot.txt");
    for command in [
        "iter task",
        "iter task list",
        "iter task pull",
        "iter session new",
        "iter wiki ingest",
        "iter note sketch read",
    ] {
        assert!(
            snapshot.contains(&format!("===== {command}\n")),
            "{command}"
        );
    }
}
