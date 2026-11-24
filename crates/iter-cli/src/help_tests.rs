//! `--help` of every subcommand, pinned ("the same commands and `--help`"). The
//! reorganisation moves the argument types between modules and crates; none of that may change
//! what a person reads. The snapshot is `crates/iter-cli/src/help_snapshot.txt`; after a change to the help on
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
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/src/help_snapshot.txt");
    if std::env::var_os("ITER_UPDATE_HELP").is_some() {
        std::fs::write(file, &now).expect("snapshot written");
    }
    let before = std::fs::read_to_string(file).expect("crates/iter-cli/src/help_snapshot.txt");
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

/// The help of `===== {command}` in the snapshot: from its header to the next one.
fn section<'a>(snapshot: &'a str, command: &str) -> &'a str {
    let header = format!("===== {command}\n");
    let at = snapshot
        .find(&header)
        .unwrap_or_else(|| panic!("the snapshot has no `{command}`"))
        + header.len();
    let rest = &snapshot[at..];
    &rest[..rest.find("===== ").unwrap_or(rest.len())]
}

/// R6 of task 303: the snapshot is regenerated with the new commands and flags (R1, R2, R3,
/// R8), and the README tells a person about them. Fails today: none of them exists.
#[test]
fn the_snapshot_and_the_readme_have_the_commands_of_task_303() {
    let snapshot = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/help_snapshot.txt"
    ))
    .expect("crates/iter-cli/src/help_snapshot.txt");
    let wanted: [(&str, &[&str]); 5] = [
        ("iter note today", &["--print"]),
        (
            "iter task new",
            &["--name", "--description", "--tag", "--status"],
        ),
        ("iter task edit", &["--tag", "--untag"]),
        ("iter instruction list", &[]),
        ("iter instruction show", &[]),
    ];
    for (command, flags) in wanted {
        let help = section(&snapshot, command);
        for flag in flags {
            assert!(
                help.contains(flag),
                "`{command}` --help has no {flag}:\n{help}"
            );
        }
    }
    let readme = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../README.md"))
        .expect("README.md");
    for words in [
        "--print",
        "--name",
        "--untag",
        "iter instruction list",
        "iter instruction show",
    ] {
        assert!(
            readme.contains(words),
            "the README does not mention `{words}`"
        );
    }
}

/// Every subcommand has help to pin: a leaf the walk missed would pass the snapshot unseen.
#[test]
fn the_snapshot_covers_the_commands_a_person_runs() {
    let snapshot = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/help_snapshot.txt"
    ))
    .expect("crates/iter-cli/src/help_snapshot.txt");
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
