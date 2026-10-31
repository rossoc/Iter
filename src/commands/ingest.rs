//! `iter wiki ingest`, `build-concept` and `pending`: agents writing the
//! wiki, and the review of what they wrote. The reading and the prompts are
//! `wiki::ingest`'s; the harness is `agent::run`'s. What the agent changes is
//! left uncommitted in the notes' git repository: `pending` shows it, and
//! accepting is a commit.

use crate::agent::prompt::shell_word;
use crate::agent::run::alive;
use crate::app::App;
use crate::args::HarnessOpts;
use crate::db::Db;
use crate::error::{IterError, Result};
use crate::models::{Project, Task};
use crate::process;
use crate::utils::resolve::{resolve_project_or_current, resolve_task, task_display};
use crate::wiki::graph::Wiki;
use crate::wiki::ingest::{self, Job, Mode};
use crate::wiki::schema;
use crate::wiki::scope::{self, Spec};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Stdio;

fn failed(message: impl Into<String>) -> IterError {
    IterError::CommandFailed(message.into())
}

/// A project's wiki, and where it is kept.
struct Target {
    project: Project,
    wiki: Wiki,
    notes_dir: PathBuf,
}

impl Target {
    fn new(db: &Db, project: Project) -> Result<Target> {
        let spec = Spec {
            project: Some(project.name.clone()),
            ..Spec::default()
        };
        let wiki = scope::wiki(db, &spec)?;
        let notes_dir = wiki.levels[0].dir.clone();
        Ok(Target {
            project,
            wiki,
            notes_dir,
        })
    }

    fn scope(&self) -> String {
        format!("--project {}", shell_word(&self.project.name))
    }

    fn in_git(&self) -> bool {
        process::succeeds(
            "git",
            Some(&self.notes_dir.display().to_string()),
            &["rev-parse", "--is-inside-work-tree"],
        )
    }

    fn require_git(&self) -> Result<()> {
        match self.in_git() {
            true => Ok(()),
            false => Err(failed(format!(
                "{} is not in a git repository, so the changes of an agent could not be reviewed: `git init` there first",
                self.notes_dir.display()
            ))),
        }
    }
}

fn now() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn project_of(
    app: &App,
    project: Option<&str>,
    task: Option<&str>,
) -> Result<(Project, Option<Task>)> {
    if let Some(reference) = task {
        let (project, task) = resolve_task(&app.db, reference)?;
        return Ok((project, Some(task)));
    }
    let project = match project {
        Some(name) => app.db.resolve::<Project>(name)?,
        None => resolve_project_or_current(&app.db, None)?,
    };
    Ok((project, None))
}

/// The lock of a running job: its file, if a process holds it.
fn lock_path(target: &Target) -> PathBuf {
    target.notes_dir.join(".iter").join("wiki.lock")
}

fn busy(target: &Target) -> bool {
    std::fs::read_to_string(lock_path(target))
        .ok()
        .and_then(|t| t.trim().parse::<u32>().ok())
        .is_some_and(alive)
}

/// The lock of a job, held until it is dropped.
struct Lock(PathBuf);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Takes the lock, or says who has it: the file is made with a call that
/// fails if it exists, and one left by a process that is gone is replaced.
fn lock(target: &Target) -> Result<Lock> {
    let path = lock_path(target);
    std::fs::create_dir_all(path.parent().unwrap_or(&target.notes_dir))?;
    for _ in 0..2 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                use std::io::Write;
                write!(file, "{}", std::process::id())?;
                return Ok(Lock(path));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if busy(target) {
                    return Err(failed("an agent is already writing in this wiki"));
                }
                let _ = std::fs::remove_file(&path);
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err(failed("could not take the wiki's lock"))
}

/// Runs the harness on `prompt` in the notes folder, its output in a log
/// there (`.iter/`, which git is told to ignore). The log's path.
fn run_agent(
    target: &Target,
    opts: &HarnessOpts,
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
    let (program, args) = harness.command(
        opts.model.as_deref().unwrap_or(""),
        "",
        prompt,
        &[],
        &crate::config::config().agent.allowed_tools,
        &opts.extra,
    );
    let outcome = std::process::Command::new(program)
        .args(&args)
        .current_dir(&target.notes_dir)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .status();
    let ok = outcome
        .map_err(|source| IterError::Spawn {
            tool: "claude",
            source,
        })?
        .success();
    Ok((log_path, ok))
}

/// Starts `iter wiki ingest` for `project` (and `task`) as a process of its
/// own that outlives this one.
fn spawn_ingest(project: &Project, task: Option<&Task>, opts: &HarnessOpts) -> Result<()> {
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
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|source| IterError::Spawn {
            tool: "iter",
            source,
        })?;
    // waited for, so that in the web server it does not stay a zombie
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

pub fn ingest(
    app: &App,
    project: Option<&str>,
    task: Option<&str>,
    dry_run: bool,
    detach: bool,
    opts: &HarnessOpts,
) -> Result<()> {
    let (project, task) = project_of(app, project, task)?;
    let target = Target::new(&app.db, project)?;
    let waiting = ingest::waiting(&target.wiki);
    let sketches = task
        .as_ref()
        .and_then(|t| crate::agent::taskdir::of(&app.db, &target.project, t).ok())
        .map(|d| d.sketches_dir())
        .filter(|d| d.is_dir() && std::fs::read_dir(d).is_ok_and(|mut e| e.next().is_some()));
    let (mode, problems) = ingest::mode(&target.wiki.levels);
    for problem in &problems {
        eprintln!("warning: {problem}");
    }
    if dry_run {
        println!("{}: ingest {}", target.project.name, mode.key());
        for w in &waiting {
            println!("  {} ({:?})", w.id, w.why);
        }
        if let Some(dir) = &sketches {
            println!("  sketches in {}", dir.display());
        }
        println!("{} notes waiting", waiting.len());
        return Ok(());
    }
    if waiting.is_empty() && sketches.is_none() {
        println!("nothing to ingest: every note has pages made after it last changed");
        return Ok(());
    }
    target.require_git()?;
    if busy(&target) {
        println!("an ingest is already running for {}", target.project.name);
        return Ok(());
    }
    if detach {
        spawn_ingest(&target.project, task.as_ref(), opts)?;
        println!(
            "ingesting {} notes of {} in the background; review with `iter wiki pending {}`",
            waiting.len(),
            target.project.name,
            target.scope()
        );
        return Ok(());
    }
    let schema_text = schema::render(&target.wiki.ontology);
    let scope = target.scope();
    let stamp = now();
    let job = Job {
        project: &target.project.name,
        notes_dir: &target.notes_dir,
        scope: &scope,
        now: &stamp,
        schema: &schema_text,
    };
    let prompt = ingest::ingest_prompt(&job, &waiting, sketches.as_deref());
    println!(
        "ingesting {} notes of {}...",
        waiting.len(),
        target.project.name
    );
    finish(&target, run_agent(&target, opts, &prompt, "ingest")?)
}

fn finish(target: &Target, (log, ok): (PathBuf, bool)) -> Result<()> {
    if !ok {
        return Err(failed(format!("the agent failed: see {}", log.display())));
    }
    println!(
        "done (log: {}). Review with `iter wiki pending {}`",
        log.display(),
        target.scope()
    );
    Ok(())
}

pub fn build_concept(
    app: &App,
    project: Option<&str>,
    name: &str,
    ideas: &[String],
    opts: &HarnessOpts,
) -> Result<()> {
    let (project, _) = project_of(app, project, None)?;
    let target = Target::new(&app.db, project)?;
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
        return Err(failed("the name is a file name: no slash, not empty"));
    }
    for idea in ideas {
        if target.wiki.node(0, idea).is_none() {
            return Err(IterError::NotFound {
                kind: "wiki page",
                name: idea.clone(),
            });
        }
    }
    target.require_git()?;
    if busy(&target) {
        return Err(failed("an agent is already writing in this wiki"));
    }
    let schema_text = schema::render(&target.wiki.ontology);
    let scope = target.scope();
    let stamp = now();
    let job = Job {
        project: &target.project.name,
        notes_dir: &target.notes_dir,
        scope: &scope,
        now: &stamp,
        schema: &schema_text,
    };
    let prompt = ingest::concept_prompt(&job, name, ideas);
    println!("building the concept {name} from {} ideas...", ideas.len());
    finish(&target, run_agent(&target, opts, &prompt, "concept")?)
}

fn git(target: &Target, args: &[&str]) -> Result<String> {
    process::output("git", Some(&target.notes_dir.display().to_string()), args)
}

/// One page of the wiki that differs from the last commit.
#[derive(Debug, PartialEq)]
pub struct Change {
    /// Relative to the notes folder (`wiki/ideas/x.md`).
    pub path: String,
    /// `new`, `changed` or `deleted`.
    pub kind: &'static str,
    /// A unified diff; for a new file, all of it as added lines.
    pub diff: String,
}

/// A path as git's pathspec takes it: that exact file, no globbing.
fn literal(path: &str) -> String {
    format!(":(literal){path}")
}

/// The pages (`.md`) under `wiki/` that are not committed, by path. What an
/// agent writes is pages; the settings and the ontology extension in the same
/// folder are the user's own and are not listed. Nothing is written, not even
/// git's index.
fn changes(target: &Target) -> Result<Vec<Change>> {
    // the notes folder may be inside a bigger repository: git reports paths
    // from the repository's root
    let prefix = git(target, &["rev-parse", "--show-prefix"])?
        .trim()
        .to_string();
    let status = git(
        target,
        &["status", "--porcelain", "-z", "-uall", "--", "wiki"],
    )?;
    let mut out = Vec::new();
    let mut entries = status.split('\0').filter(|e| e.len() > 3);
    while let Some(entry) = entries.next() {
        let (code, full) = entry.split_at(3);
        if code.starts_with('R') || code.starts_with('C') {
            entries.next(); // the old name
        }
        let path = full.strip_prefix(prefix.as_str()).unwrap_or(full);
        if !path.ends_with(".md") {
            continue;
        }
        let (kind, diff) = match code.trim() {
            "??" | "A" => {
                let text = std::fs::read_to_string(target.notes_dir.join(path)).unwrap_or_default();
                let body: String = text.lines().map(|l| format!("+{l}\n")).collect();
                ("new", format!("--- /dev/null\n+++ b/{path}\n{body}"))
            }
            c if c.contains('D') => (
                "deleted",
                git(
                    target,
                    &["diff", "HEAD", "--relative", "--", &literal(path)],
                )
                .unwrap_or_default(),
            ),
            _ => (
                "changed",
                git(
                    target,
                    &["diff", "HEAD", "--relative", "--", &literal(path)],
                )
                .unwrap_or_default(),
            ),
        };
        out.push(Change {
            path: path.to_string(),
            kind,
            diff,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// What the wiki of a project has that nobody has accepted, and what is
/// still to be ingested: the page of a task shows it.
pub struct Review {
    pub changes: Vec<Change>,
    pub waiting: Vec<ingest::Waiting>,
    pub mode: Mode,
    /// Whether the notes folder is in a git repository (else there is
    /// nothing to review against, and an ingest is refused).
    pub in_git: bool,
    /// An agent is writing in the wiki now.
    pub busy: bool,
    pub notes_dir: PathBuf,
}

pub fn review(db: &Db, project: &Project) -> Result<Review> {
    let target = Target::new(db, project.clone())?;
    let in_git = target.in_git();
    Ok(Review {
        changes: if in_git {
            changes(&target)?
        } else {
            Vec::new()
        },
        waiting: ingest::waiting(&target.wiki),
        mode: ingest::mode(&target.wiki.levels).0,
        in_git,
        busy: busy(&target),
        notes_dir: target.notes_dir.clone(),
    })
}

/// Commits (`accept`) or throws away the pages that are pending under
/// `wiki/` -- those and no other file. The paths it touched.
pub fn settle(db: &Db, project: &Project, accept: bool) -> Result<Vec<String>> {
    let target = Target::new(db, project.clone())?;
    target.require_git()?;
    if busy(&target) {
        return Err(failed(
            "an agent is writing in the wiki: wait for it to finish",
        ));
    }
    let list = changes(&target)?;
    if list.is_empty() {
        return Ok(Vec::new());
    }
    let paths: Vec<String> = list.iter().map(|c| literal(&c.path)).collect();
    let with = |first: &[&str]| -> Result<String> {
        let mut args: Vec<&str> = first.to_vec();
        args.push("--");
        args.extend(paths.iter().map(String::as_str));
        git(&target, &args)
    };
    if accept {
        with(&["add"])?;
        with(&["commit", "-m", "wiki: ingest"])?;
    } else {
        // out of the index first (a commit that failed leaves files staged),
        // then back to the last commit, and the pages that are new are removed
        let _ = with(&["reset", "-q", "HEAD"]);
        for c in list.iter().filter(|c| c.kind != "new") {
            git(&target, &["checkout", "HEAD", "--", &literal(&c.path)])?;
        }
        for c in list.iter().filter(|c| c.kind == "new") {
            std::fs::remove_file(target.notes_dir.join(&c.path))?;
        }
    }
    Ok(list.into_iter().map(|c| c.path).collect())
}

/// Starts an ingest of the project (and the sketches of `task`) in the
/// background, as the page's button does.
pub fn ingest_in_background(db: &Db, project: &Project, task: &Task) -> Result<()> {
    let target = Target::new(db, project.clone())?;
    target.require_git()?;
    if busy(&target) {
        return Err(failed("an ingest is already running"));
    }
    let none = HarnessOpts {
        agent: None,
        model: None,
        extra: Vec::new(),
    };
    spawn_ingest(project, Some(task), &none)
}

pub fn pending(app: &App, project: Option<&str>, accept: bool, revert: bool) -> Result<()> {
    let (project, _) = project_of(app, project, None)?;
    let target = Target::new(&app.db, project.clone())?;
    target.require_git()?;
    let list = changes(&target)?;
    if list.is_empty() {
        println!("nothing pending in the wiki of {}", project.name);
        return Ok(());
    }
    let summary: String = list
        .iter()
        .map(|c| format!("{:<8}{}\n", c.kind, c.path))
        .collect();
    if accept || revert {
        settle(&app.db, &project, accept)?;
        let verb = if accept { "committed" } else { "thrown away" };
        println!("{verb}:\n{summary}");
    } else {
        print!("{summary}");
        for c in &list {
            print!("\n{}", c.diff);
        }
        println!(
            "\naccept with `iter wiki pending {0} --accept`, throw away with `--revert`",
            target.scope()
        );
    }
    Ok(())
}

/// When a task starts or finishes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    TaskStart,
    TaskDone,
}

/// Does what the project's `ingest` setting asks for at `trigger`: starts an
/// ingest in the background. It never fails the command it is called from:
/// what goes wrong is a warning.
pub fn auto(app: &App, project: &Project, task: &Task, trigger: Trigger) {
    let outcome = (|| -> Result<bool> {
        let Ok(target) = Target::new(&app.db, project.clone()) else {
            return Ok(false);
        };
        let wanted = match ingest::mode(&target.wiki.levels).0 {
            Mode::OnTaskStart => trigger == Trigger::TaskStart,
            Mode::OnTaskDone => trigger == Trigger::TaskDone,
            Mode::Manual => false,
        };
        if !wanted || busy(&target) || !target.in_git() {
            return Ok(false);
        }
        let sketches = trigger == Trigger::TaskDone;
        if ingest::waiting(&target.wiki).is_empty() && !sketches {
            return Ok(false);
        }
        let none = HarnessOpts {
            agent: None,
            model: None,
            extra: Vec::new(),
        };
        spawn_ingest(project, sketches.then_some(task), &none)?;
        Ok(true)
    })();
    match outcome {
        Ok(true) => println!(
            "ingesting the notes of {} in the background (`iter wiki pending` to review)",
            project.name
        ),
        Ok(false) => {}
        Err(e) => eprintln!("warning: could not start the ingest: {e}"),
    }
}

/// On `task done`: the task's own notes, appended to the project's note of
/// today under `## Task: <name>`, one `###` per file. How many files went in;
/// none when there was nothing to add or the note already had the section.
pub fn bridge_task_notes(app: &App, project: &Project, task: &Task) -> Result<usize> {
    let dir = crate::agent::taskdir::of(&app.db, project, task)?;
    let files = crate::agent::taskdir::TaskDir::files(&dir.notes_dir());
    let mut body = String::new();
    let mut count = 0;
    for file in &files {
        let text = std::fs::read_to_string(&file.path).unwrap_or_default();
        if text.trim().is_empty() {
            continue;
        }
        body.push_str(&format!("### {}\n\n{}\n\n", file.name, text.trim()));
        count += 1;
    }
    if count == 0 {
        return Ok(0);
    }
    let org = match project.organization_id {
        Some(id) => app.db.get::<crate::models::Organization>(id)?,
        None => None,
    };
    let notes_dir = crate::notes::project_dir(project, &org)
        .ok_or_else(|| failed("no notes folder for the project"))?;
    let today = chrono::Local::now().date_naive();
    let added =
        crate::notes::append_section(&notes_dir, today, &format!("Task: {}", task.name), &body)?;
    Ok(if added { count } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Table;
    use crate::fixtures;
    use crate::models::TaskStatus;

    fn world(name: &str) -> (App, Project, Task, PathBuf) {
        let notes =
            std::env::temp_dir().join(format!("iter-ingestcmd-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&notes);
        let db = fixtures::db();
        let mut p = fixtures::project("app");
        p.notes_path = notes.display().to_string();
        let project_id = db.insert(&p).unwrap();
        let task_id = db
            .insert(&Task {
                id: None,
                project_id,
                name: "Fix it".into(),
                description: String::new(),
                github_issue: None,
                status: TaskStatus::Done,
                branch_prefix: String::new(),
                matrix_placed: false,
                start_time: None,
                duration: None,
            })
            .unwrap();
        let project = db.get::<Project>(project_id).unwrap().unwrap();
        let task = db.get::<Task>(task_id).unwrap().unwrap();
        (App::with_db(db), project, task, notes)
    }

    #[test]
    fn a_finished_tasks_notes_go_into_todays_note_once() {
        let (app, project, task, notes) = world("bridge");
        // no notes folder of the task, nothing to add
        assert_eq!(bridge_task_notes(&app, &project, &task).unwrap(), 0);
        let dir = crate::agent::taskdir::of(&app.db, &project, &task).unwrap();
        dir.ensure().unwrap();
        std::fs::write(dir.notes_dir().join("thought.md"), "Maybe per route.\n").unwrap();
        std::fs::write(dir.notes_dir().join("empty.md"), "  \n").unwrap();
        assert_eq!(bridge_task_notes(&app, &project, &task).unwrap(), 1);
        let today = chrono::Local::now().date_naive();
        let text = std::fs::read_to_string(notes.join(format!("{}.md", today.format("%Y-%m-%d"))))
            .unwrap();
        assert!(text.contains("## Task: Fix it\n\n### thought\n\nMaybe per route."));
        assert!(!text.contains("empty"));
        // done again: the section is there already
        assert_eq!(bridge_task_notes(&app, &project, &task).unwrap(), 0);
    }

    fn git_in(dir: &std::path::Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git runs");
        assert!(status.status.success(), "git {args:?}: {status:?}");
    }

    /// An organization repository `root` whose project `app` keeps its notes in
    /// `root/app` -- the usual layout -- with one committed page.
    fn repo(name: &str) -> (Db, Project, PathBuf) {
        let root = std::env::temp_dir().join(format!("iter-pending-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let notes = root.join("app");
        std::fs::create_dir_all(notes.join("wiki/ideas")).unwrap();
        std::fs::write(
            notes.join("wiki/ideas/b.md"),
            "---\ndescription: b\ntags: [x]\n---\nold\n",
        )
        .unwrap();
        git_in(&root, &["init", "-q", "-b", "main"]);
        git_in(&root, &["config", "user.email", "t@t"]);
        git_in(&root, &["config", "user.name", "t"]);
        git_in(&root, &["add", "-A"]);
        git_in(&root, &["commit", "-q", "-m", "init"]);
        let db = fixtures::db();
        let mut p = fixtures::project("app");
        p.notes_path = notes.display().to_string();
        let id = db.insert(&p).unwrap();
        let project = db.get::<Project>(id).unwrap().unwrap();
        (db, project, notes)
    }

    #[test]
    fn pending_pages_are_found_with_their_diffs_in_a_repository_above_the_notes() {
        let (db, project, notes) = repo("nested");
        std::fs::write(
            notes.join("wiki/ideas/a.md"),
            "---\ndescription: a\ntags: [x]\n---\nnew\n",
        )
        .unwrap();
        std::fs::write(
            notes.join("wiki/ideas/b.md"),
            "---\ndescription: b\ntags: [x]\n---\nnewer\n",
        )
        .unwrap();
        // the user's own files in the same folder are not the agent's
        std::fs::write(notes.join("wiki/settings.yaml"), "ingest: manual\n").unwrap();
        let review = review(&db, &project).unwrap();
        let seen: Vec<(&str, &str)> = review
            .changes
            .iter()
            .map(|c| (c.path.as_str(), c.kind))
            .collect();
        assert_eq!(
            seen,
            [("wiki/ideas/a.md", "new"), ("wiki/ideas/b.md", "changed")]
        );
        assert!(review.changes[0].diff.contains("+new"));
        assert!(
            review.changes[1].diff.contains("-old") && review.changes[1].diff.contains("+newer")
        );
    }

    #[test]
    fn throwing_away_touches_the_pages_and_nothing_else() {
        let (db, project, notes) = repo("revert");
        std::fs::write(
            notes.join("wiki/ideas/a.md"),
            "---\ndescription: a\n---\nnew\n",
        )
        .unwrap();
        std::fs::write(notes.join("wiki/ideas/b.md"), "changed\n").unwrap();
        std::fs::write(notes.join("wiki/settings.yaml"), "ingest: manual\n").unwrap();
        // a commit that failed leaves the page staged
        git_in(&notes, &["add", "wiki/ideas/a.md"]);
        let paths = settle(&db, &project, false).unwrap();
        assert_eq!(paths, ["wiki/ideas/a.md", "wiki/ideas/b.md"]);
        assert!(!notes.join("wiki/ideas/a.md").exists());
        assert!(
            std::fs::read_to_string(notes.join("wiki/ideas/b.md"))
                .unwrap()
                .contains("old")
        );
        assert!(notes.join("wiki/settings.yaml").exists());
        assert!(review(&db, &project).unwrap().changes.is_empty());
    }

    #[test]
    fn accepting_commits_the_pages_only() {
        let (db, project, notes) = repo("accept");
        std::fs::write(
            notes.join("wiki/ideas/a.md"),
            "---\ndescription: a\n---\nnew\n",
        )
        .unwrap();
        std::fs::write(notes.join("wiki/settings.yaml"), "ingest: manual\n").unwrap();
        settle(&db, &project, true).unwrap();
        let committed = process::output(
            "git",
            Some(&notes.display().to_string()),
            &["show", "--name-only", "--format=", "HEAD"],
        )
        .unwrap();
        assert!(committed.contains("wiki/ideas/a.md") && !committed.contains("settings.yaml"));
        assert!(review(&db, &project).unwrap().changes.is_empty());
    }

    #[test]
    fn only_one_agent_holds_the_wikis_lock() {
        let (db, project, _) = repo("lock");
        let target = Target::new(&db, project).unwrap();
        let first = lock(&target).unwrap();
        assert!(busy(&target) && lock(&target).is_err());
        drop(first);
        assert!(!busy(&target) && lock(&target).is_ok());
    }

    #[test]
    fn an_ingest_needs_a_project_with_a_notes_folder() {
        let db = fixtures::db();
        let id = db.insert(&fixtures::project("bare")).unwrap();
        let project = db.get::<Project>(id).unwrap().unwrap();
        assert!(Target::new(&db, project).is_err());
    }
}
