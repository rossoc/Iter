//! Tests of "Instructions should be defined in the config folder" (sketch `plan`, R1-R9).
//!
//! They go through what stays the same across the change: the command line as typed (`iter task
//! solve -i ...`), the files on disk (run records, prompts, the repo's `instructions/`), and the
//! instruction lookup of the web (`find`, `list`, `check`, `rename_target`). So they compile
//! before the work and after it, and fail before it for the reason each one names.

use crate::Run;
use crate::args::{Args, Command};
use clap::Parser;
use iter_core::app::App;
use iter_core::db::Table;
use iter_core::fixtures;
use iter_core::models::Project;
use serde_yaml::Value;
use std::path::{Path, PathBuf};

/// The seven instructions the repo ships.
const SHIPPED: [&str; 7] = [
    "plan", "develop", "test", "check", "fix", "complete", "ingest",
];

/// The repo's root, where `instructions/` lives (R8: read at run time, not `include_str!`).
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn shipped_dir() -> PathBuf {
    repo().join("instructions")
}

/// `iter <words>`, parsed as the shell would hand it over; a parse error panics (the flag is wrong).
fn parse(words: &[&str]) -> Args {
    Args::try_parse_from(std::iter::once("iter").chain(words.iter().copied()))
        .unwrap_or_else(|e| panic!("`iter {}` does not parse: {e}", words.join(" ")))
}

/// Runs `iter task ...` against `app`.
fn task(app: &App, words: &[&str]) -> iter_core::error::Result<()> {
    let args = parse(words);
    match &args.command {
        Command::Task { action: Some(a) } => a.run(app),
        other => panic!("not a task command: {other:?}"),
    }
}

const TASK: &str = "app/Fix it";

/// The agent test world (project `app`, task `Fix it`, notes in a scratch folder).
fn world(tag: &str) -> (App, PathBuf) {
    let w = fixtures::world(&format!("cfg-instr-{tag}"));
    (App::with_db(w.db), w.notes)
}

/// Writes `<dir>/instructions/<name>.md`.
fn put(dir: &Path, name: &str, text: &str) {
    let path = dir.join("instructions").join(format!("{name}.md"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn task_dir(notes: &Path) -> PathBuf {
    notes.join("tasks/1")
}

/// Run `n`'s record, as the YAML on disk.
fn record(notes: &Path, n: u32) -> Value {
    let text = std::fs::read_to_string(task_dir(notes).join(format!("runs/{n}.yaml")))
        .unwrap_or_else(|e| panic!("run {n} has no record: {e}"));
    serde_yaml::from_str(&text).unwrap()
}

fn prompt_of_run(notes: &Path, n: u32) -> String {
    std::fs::read_to_string(task_dir(notes).join(format!("runs/{n}.prompt.md"))).unwrap()
}

fn runs_on_disk(notes: &Path) -> usize {
    std::fs::read_dir(task_dir(notes).join("runs"))
        .map(|d| {
            d.flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "yaml"))
                .count()
        })
        .unwrap_or(0)
}

/// `iter task solve app/Fix\ it -i <name> --subagent`.
fn subagent(app: &App, name: &str) -> iter_core::error::Result<()> {
    task(app, &["task", "solve", TASK, "-i", name, "--subagent"])
}

fn close(app: &App, n: u32) {
    let n = n.to_string();
    task(
        app,
        &[
            "task", "runs", TASK, "--close", &n, "--status", "done", "-m", "ok",
        ],
    )
    .unwrap();
}

/// The front matter of a shipped instruction, as YAML (empty mapping when it has none).
fn front_of(raw: &str) -> serde_yaml::Mapping {
    match iter_core::agent::instruction::split(raw).0 {
        Some(yaml) => match serde_yaml::from_str::<Value>(yaml).unwrap() {
            Value::Mapping(m) => m,
            Value::Null => serde_yaml::Mapping::new(),
            other => panic!("front matter is not a mapping: {other:?}"),
        },
        None => serde_yaml::Mapping::new(),
    }
}

fn flag(front: &serde_yaml::Mapping, key: &str) -> bool {
    front.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// Every file under `dir` with extension `ext`.
fn files(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    // a scratch folder another test removes mid-walk is simply skipped
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            files(&path, ext, out);
        } else if path.extension().is_some_and(|x| x == ext) {
            out.push(path);
        }
    }
}

// --- R1 -----------------------------------------------------------------------------------

/// T1 (R1): no agent instruction is compiled in; the seven live in the repo's `instructions/`.
#[test]
fn no_instruction_is_compiled_into_the_binary() {
    assert!(
        !repo().join("crates/iter-core/src/agent/prompts").exists(),
        "crates/iter-core/src/agent/prompts still exists"
    );
    // built so that this file does not match itself
    let needle = format!("include_str!(\"{}/", "prompts");
    let mut rs = Vec::new();
    files(&repo().join("crates"), "rs", &mut rs);
    let offenders: Vec<String> = rs
        .iter()
        // the wiki's own prompts are out of scope
        .filter(|p| !p.starts_with(repo().join("crates/iter-core/src/wiki")))
        .filter(|p| std::fs::read_to_string(p).unwrap().contains(&needle))
        .map(|p| p.display().to_string())
        .collect();
    assert!(
        offenders.is_empty(),
        "include_str! of prompts in {offenders:?}"
    );
    for name in SHIPPED {
        let path = shipped_dir().join(format!("{name}.md"));
        assert!(path.is_file(), "{} is missing", path.display());
    }
}

// --- R3 -----------------------------------------------------------------------------------

/// T3 (R3): `-i/--instruction` names the instruction, any name; `-k/--kind` is gone; `-i` is required.
#[test]
fn the_cli_names_the_instruction_with_i() {
    for words in [
        &["task", "solve", "-i", "review"][..],
        &["task", "prompt", "-i", "plan"],
        &["task", "prompt", TASK, "--instruction", "my-own"],
        &["task", "solve", TASK, "-i", "test", "--subagent"],
    ] {
        let parsed = Args::try_parse_from(std::iter::once("iter").chain(words.iter().copied()));
        let args = parsed.unwrap_or_else(|e| panic!("`{}` should parse: {e}", words.join(" ")));
        let shown = format!("{args:?}");
        let name = words[words.len() - 1 - usize::from(words.ends_with(&["--subagent"]))];
        assert!(
            shown.contains(&format!("{name:?}")),
            "{name} not kept: {shown}"
        );
    }
    for words in [
        &["task", "solve", "-k", "develop"][..],
        &["task", "prompt", "--kind", "plan"],
        &["task", "solve"],
        &["task", "prompt", TASK],
        &["task", "solve", TASK, "--subagent"],
    ] {
        assert!(
            Args::try_parse_from(std::iter::once("iter").chain(words.iter().copied())).is_err(),
            "`{}` should be refused",
            words.join(" ")
        );
    }
}

// --- R2 -----------------------------------------------------------------------------------

/// T2 (R2), as the CLI looks: nearest first (task folder, then project notes); `{{kind}}` (R5)
/// is the instruction's name; a name that is no built-in kind works (R3).
#[test]
fn the_cli_takes_the_nearest_instruction_of_any_name() {
    let (app, notes) = world("nearest");
    put(&notes, "t-named", "from the project {{task.name}}");
    put(
        &task_dir(&notes),
        "t-named",
        "kind={{kind}} for {{task.name}}",
    );
    subagent(&app, "t-named").unwrap();
    assert_eq!(prompt_of_run(&notes, 1), "kind=t-named for Fix it");
    let run = record(&notes, 1);
    assert_eq!(run["instruction"].as_str(), Some("t-named"), "{run:?}");
}

/// T2 (R2): a name found in no layer is an error naming it and where the config folder was looked
/// in (`<config>/instructions/<name>.md`); nothing is recorded, nothing built in stands in.
#[test]
fn an_instruction_found_nowhere_is_an_error_naming_the_config_path() {
    let (app, notes) = world("nowhere");
    let name = "t-nowhere-to-be-found";
    let wanted = iter_core::config::config_dir()
        .unwrap()
        .join("instructions")
        .join(format!("{name}.md"));
    for e in [
        subagent(&app, name).unwrap_err(),
        task(&app, &["task", "prompt", TASK, "-i", name]).unwrap_err(),
    ] {
        let e = e.to_string();
        assert!(e.contains(name), "{e}");
        assert!(e.contains(&wanted.display().to_string()), "{e}");
    }
    assert_eq!(runs_on_disk(&notes), 0);
}

// --- R4 -----------------------------------------------------------------------------------

/// T4 (R4): the front matter's `worktree: true` decides the worktree; the name (`develop`) and a
/// `kind:` key no longer do. The project wants branches but its folder is no git repository, so
/// a run that asks for a worktree is refused with that, and one that does not goes ahead.
#[test]
fn worktree_in_the_front_matter_gives_the_run_a_worktree() {
    let (app, notes) = world("worktree");
    let mut project = app.db.get::<Project>(1).unwrap().unwrap();
    let nogit = fixtures::scratch("cfg-instr-nogit");
    project.github = true;
    project.auto_branch = true;
    project.base_path = nogit.display().to_string();
    app.db.update(project.id(), &project).unwrap();

    put(&notes, "t-reader", "Read {{task.name}}.");
    put(
        &notes,
        "develop",
        "---\ndescription: no worktree asked\n---\nDevelop {{task.name}}.",
    );
    put(
        &notes,
        "t-legacy",
        "---\nkind: develop\n---\nOld style {{task.name}}.",
    );
    put(
        &notes,
        "t-quiet",
        "---\nworktree: false\n---\nQuiet {{task.name}}.",
    );
    put(
        &notes,
        "t-builder",
        "---\nworktree: true\n---\nBuild {{task.name}}.",
    );

    for (n, name) in [
        (1, "t-reader"),
        (2, "develop"),
        (3, "t-legacy"),
        (4, "t-quiet"),
    ] {
        subagent(&app, name).unwrap_or_else(|e| panic!("{name}: {e}"));
        let run = record(&notes, n);
        assert_eq!(run["instruction"].as_str(), Some(name), "{run:?}");
        assert!(run.get("branch").is_none(), "{name} got a branch: {run:?}");
        close(&app, n);
    }
    let e = subagent(&app, "t-builder").unwrap_err().to_string();
    assert!(e.contains("is not a git repository"), "{e}");
}

/// T4 (R4, R6): `interactive: true` makes solve refuse the instruction, also as a subagent, and
/// point at `iter task prompt -i <name>`; a file named `complete` without it is a run like any other.
#[test]
fn interactive_in_the_front_matter_is_refused_as_a_run() {
    let (app, notes) = world("interactive");
    put(
        &notes,
        "t-lead",
        "---\ninteractive: true\n---\nLead {{task.name}}.",
    );
    put(&notes, "complete", "Not interactive here: {{task.name}}.");
    let e = subagent(&app, "t-lead").unwrap_err().to_string();
    assert!(e.contains("iter task prompt -i t-lead"), "{e}");
    assert_eq!(runs_on_disk(&notes), 0);
    // printing it is what it is for
    task(&app, &["task", "prompt", TASK, "-i", "t-lead"]).unwrap();
    subagent(&app, "complete").unwrap();
    assert_eq!(record(&notes, 1)["instruction"].as_str(), Some("complete"));
}

/// R4: the shipped front matter says what the kinds used to: develop, test and fix get a
/// worktree, complete is interactive, and no other one is either.
#[test]
fn the_shipped_instructions_carry_worktree_and_interactive() {
    for name in SHIPPED {
        let path = shipped_dir().join(format!("{name}.md"));
        let raw =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let front = front_of(&raw);
        assert_eq!(
            flag(&front, "worktree"),
            matches!(name, "develop" | "test" | "fix"),
            "{name}: worktree"
        );
        assert_eq!(
            flag(&front, "interactive"),
            name == "complete",
            "{name}: interactive"
        );
    }
}

// --- R5 -----------------------------------------------------------------------------------

/// T5 (R5): a record from before (`kind`, no `instruction`, legacy names) loads with its name;
/// one written now carries `instruction` and needs no `kind`.
#[test]
fn run_records_keep_loading_and_carry_the_instruction_name() {
    use iter_core::agent::run::RunRecord;
    let legacy = |kind: &str, instruction: Option<&str>| {
        let mut text = format!(
            "n: 1\nagent: claude\nmodel: haiku\neffort: low\nkind: {kind}\nstatus: done\nstarted: 2026-10-30 10:00:00\n"
        );
        if let Some(i) = instruction {
            text.push_str(&format!("instruction: {i}\n"));
        }
        text
    };
    let name_of = |yaml: &str| -> String {
        let run: RunRecord =
            serde_yaml::from_str(yaml).unwrap_or_else(|e| panic!("does not load: {e}\n{yaml}"));
        let back = serde_yaml::to_value(&run).unwrap();
        back["instruction"]
            .as_str()
            .unwrap_or_else(|| panic!("written without its instruction: {back:?}"))
            .to_string()
    };
    for (kind, name) in [
        ("execute", "develop"),
        ("investigate", "check"),
        ("develop", "develop"),
        ("plan", "plan"),
        ("ingest", "ingest"),
    ] {
        assert_eq!(name_of(&legacy(kind, None)), name, "kind: {kind}");
    }
    assert_eq!(name_of(&legacy("check", Some("review"))), "review");
    let now = "n: 2\nagent: claude\nmodel: haiku\neffort: low\ninstruction: review\nstatus: done\nstarted: 2026-10-30 10:00:00\n";
    assert_eq!(name_of(now), "review");
}

// --- R6 -----------------------------------------------------------------------------------

/// T6 (R6): `complete` names its subagents with `-i`, and says `-k` nowhere.
#[test]
fn complete_names_its_subagents_with_i() {
    let path = shipped_dir().join("complete.md");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    for name in ["test", "develop", "check", "ingest"] {
        let wanted = format!("iter task solve -i {name} --subagent");
        assert!(text.contains(&wanted), "{wanted}");
    }
    assert!(!text.contains("-k "), "complete.md still says -k");
    assert!(!text.contains("--kind"), "complete.md still says --kind");
}

/// R6: no hint in the code or in the docs (README, architecture) tells the user `-k`/`--kind`.
#[test]
fn no_hint_says_k_or_kind() {
    let mut paths = Vec::new();
    files(&repo().join("crates"), "rs", &mut paths);
    paths.retain(|p| !p.ends_with("config_instructions_tests.rs"));
    paths.push(repo().join("README.md"));
    paths.push(repo().join("doc/architecture_agent_wiki.md"));
    let mut found = Vec::new();
    for path in &paths {
        let text = std::fs::read_to_string(path).unwrap();
        for (i, line) in text.lines().enumerate() {
            let iter_task = line.contains("iter task prompt") || line.contains("iter task solve");
            let old = line.contains(" -k ") || line.contains("--kind");
            if iter_task && old {
                found.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
            }
        }
    }
    assert!(found.is_empty(), "still -k/--kind:\n{}", found.join("\n"));
}

// --- R9 -----------------------------------------------------------------------------------

/// R9: the README says how to install the instructions and names the front-matter keys.
#[test]
fn the_readme_tells_how_to_install_the_instructions() {
    let readme = std::fs::read_to_string(repo().join("README.md")).unwrap();
    for wanted in [
        "instructions/",
        "-i ",
        "worktree: true",
        "interactive: true",
    ] {
        assert!(readme.contains(wanted), "README does not say `{wanted}`");
    }
}
