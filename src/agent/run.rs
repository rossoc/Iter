//! A run: one headless agent working on a task in its worktree, kept as `runs/<n>.yaml`, `.prompt.md` and `.log` in the task's folder.

use crate::agent::error::AgentResult;
use crate::agent::prompt::Kind;
use crate::agent::taskdir::TaskDir;
use crate::process::{Runner, SYSTEM};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The program that runs the loop. iter starts it; it does not know what it
/// does.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Harness {
    #[default]
    Claude,
}

/// What one headless run of a harness is asked to do.
#[derive(Clone, Copy, Default)]
pub struct Invocation<'a> {
    /// Empty: the harness's own default.
    pub model: &'a str,
    /// Empty: the harness's own default.
    pub effort: &'a str,
    /// Extra folders the agent may read and write.
    pub dirs: &'a [&'a Path],
    pub allowed: &'a [String],
    /// Goes last, for what the harness takes that iter does not know about.
    pub extra: &'a [String],
}

impl Harness {
    pub fn key(self) -> &'static str {
        match self {
            Harness::Claude => "claude",
        }
    }

    #[cfg(feature = "web")]
    /// The models a form offers: the harness's aliases, unless `configured` (`agent.models`) overrides.
    pub fn models(self, configured: &[String]) -> Vec<String> {
        if !configured.is_empty() {
            return configured.to_vec();
        }
        match self {
            Harness::Claude => [
                "haiku",
                "sonnet",
                "opus",
                "claude-haiku-5-5",
                "claude-sonnet-5-5",
                "claude-opus-5-5",
                "claude-fable-5-1",
            ]
            .map(String::from)
            .to_vec(),
        }
    }

    #[cfg(feature = "web")]
    pub fn efforts(self) -> &'static [&'static str] {
        match self {
            Harness::Claude => &["low", "medium", "high", "xhigh", "max"],
        }
    }

    pub fn command(self, job: &Invocation) -> (&'static str, Vec<String>) {
        self.backend().command(job)
    }

    fn backend(self) -> &'static dyn AgentBackend {
        match self {
            Harness::Claude => &ClaudeCode,
        }
    }
}

/// How one agent program is driven; `Harness::backend` dispatches here. The prompt is not in the
/// command: it goes on the program's standard input (`exec::run_logged`), out of `ps`.
pub(super) trait AgentBackend {
    fn command(&self, job: &Invocation) -> (&'static str, Vec<String>);
}

struct ClaudeCode;

impl AgentBackend for ClaudeCode {
    fn command(&self, job: &Invocation) -> (&'static str, Vec<String>) {
        let Invocation {
            model,
            effort,
            dirs,
            allowed,
            extra,
        } = *job;
        // `-p` with no prompt argument reads it from standard input
        let mut args = vec!["-p".to_string()];
        if !model.is_empty() {
            args.push("--model".into());
            args.push(model.into());
        }
        if !effort.is_empty() {
            args.push("--effort".into());
            args.push(effort.into());
        }
        for dir in dirs {
            args.push("--add-dir".into());
            args.push(dir.display().to_string());
        }
        // headless: nobody to ask, so edits and the commands the prompts name are allowed
        args.push("--permission-mode".into());
        args.push("acceptEdits".into());
        if !allowed.is_empty() {
            args.push("--allowedTools".into());
            args.extend(allowed.iter().cloned());
        }
        args.extend(extra.iter().cloned());
        ("claude", args)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Written down, not started.
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Queued => "queued",
            Status::Running => "running",
            Status::Done => "done",
            Status::Failed => "failed",
            Status::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    pub n: u32,
    pub agent: Harness,
    pub model: String,
    pub effort: String,
    pub kind: Kind,
    /// The instruction name; empty for runs that predate instructions (the kind is the name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
    pub status: Status,
    /// `YYYY-MM-DD HH:MM:SS` local time the run began (queued: when written).
    pub started: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

impl RunRecord {
    /// The status to show: `lost` for a running run whose process is gone, or one that never got a process after a minute.
    pub fn shown_status(&self) -> &'static str {
        match (self.status, self.pid) {
            (Status::Running, Some(pid)) if !alive(pid) => "lost",
            (Status::Running, None) if self.age_seconds() > 60 => "lost",
            (status, _) => status.label(),
        }
    }

    #[cfg(feature = "web")]
    pub fn instruction_name(&self) -> &str {
        self.instruction.as_deref().unwrap_or(self.kind.key())
    }

    pub fn is_queued(&self) -> bool {
        self.status == Status::Queued
    }

    pub fn is_running(&self) -> bool {
        self.shown_status() == "running"
    }

    fn age_seconds(&self) -> i64 {
        chrono::NaiveDateTime::parse_from_str(&self.started, "%Y-%m-%d %H:%M:%S")
            .map(|t| (chrono::Local::now().naive_local() - t).num_seconds())
            .unwrap_or(i64::MAX)
    }
}

/// Whether `pid` exists and is not a zombie (`kill -0` answers for those). Pid 0 and 1 are
/// never a run: `kill -0 0` answers for the caller's own group.
pub fn alive(pid: u32) -> bool {
    Procs::recent().alive(pid)
}

#[cfg(feature = "web")]
/// Asks a run's process to stop: its whole group, when `pid` is a process iter can vouch for
/// (see [`Procs::signalable`]). The record is a file the agent can write, so its `pid` is a
/// claim; there is no fallback to a bare pid.
pub fn interrupt(pid: u32) -> bool {
    interrupt_with(&SYSTEM, pid)
}

#[cfg(any(feature = "web", test))]
fn interrupt_with(runner: &dyn Runner, pid: u32) -> bool {
    if !Procs::scan(runner).signalable(pid) {
        return false;
    }
    runner.succeeds("kill", None, &["-TERM", "--", &format!("-{pid}")])
}

/// One `ps` of every process: `pid pgid stat comm`.
struct Procs(Vec<Proc>);

struct Proc {
    pid: u32,
    #[cfg_attr(not(feature = "web"), allow(dead_code))]
    pgid: u32,
    zombie: bool,
    /// The program's file name (macOS prints a path).
    #[cfg_attr(not(feature = "web"), allow(dead_code))]
    comm: String,
}

impl Procs {
    fn scan(runner: &dyn Runner) -> Procs {
        let text = runner
            .output("ps", None, &["-A", "-o", "pid=,pgid=,stat=,comm="])
            .unwrap_or_default();
        Procs(text.lines().filter_map(Proc::parse).collect())
    }

    /// The scan of the last second, or a new one: a page asks about every running record in
    /// turn, and each used to cost two spawns.
    fn recent() -> std::sync::Arc<Procs> {
        use std::sync::{Arc, Mutex};
        use std::time::{Duration, Instant};
        static LAST: Mutex<Option<(Instant, Arc<Procs>)>> = Mutex::new(None);
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        match last.as_ref() {
            Some((at, procs)) if at.elapsed() < Duration::from_secs(1) => procs.clone(),
            _ => {
                let procs = Arc::new(Procs::scan(&SYSTEM));
                *last = Some((Instant::now(), procs.clone()));
                procs
            }
        }
    }

    fn get(&self, pid: u32) -> Option<&Proc> {
        self.0.iter().find(|p| p.pid == pid)
    }

    /// Exists and is not a zombie; pid 0 and 1 are never a run.
    fn alive(&self, pid: u32) -> bool {
        pid > 1 && self.get(pid).is_some_and(|p| !p.zombie)
    }

    #[cfg(any(feature = "web", test))]
    /// Whether iter may signal `-pid`: a live process other than 0 and init that leads its own
    /// group (what `spawn_detached` makes), whose program is iter or claude, and that is
    /// neither this process nor in its group (the record could name the server).
    fn signalable(&self, pid: u32) -> bool {
        let me = std::process::id();
        let my_group = self.get(me).map(|p| p.pgid);
        pid > 1
            && pid != me
            && self.get(pid).is_some_and(|p| {
                !p.zombie
                    && p.pgid == pid
                    && Some(p.pgid) != my_group
                    && matches!(p.comm.as_str(), "iter" | "claude")
            })
    }
}

impl Proc {
    fn parse(line: &str) -> Option<Proc> {
        let mut fields = line.split_whitespace();
        let pid = fields.next()?.parse().ok()?;
        let pgid = fields.next()?.parse().ok()?;
        let stat = fields.next()?;
        // The program's path may hold spaces: it is the rest of the line.
        let comm = (0..3).fold(line.trim_start(), |rest, _| {
            rest.trim_start_matches(|c: char| !c.is_whitespace())
                .trim_start()
        });
        Some(Proc {
            pid,
            pgid,
            zombie: stat.starts_with('Z'),
            comm: comm.rsplit('/').next().unwrap_or(comm).trim().to_string(),
        })
    }
}

impl TaskDir {
    pub(super) fn runs_dir(&self) -> PathBuf {
        self.root().join("runs")
    }

    pub fn run_path(&self, n: u32) -> PathBuf {
        self.runs_dir().join(format!("{n}.yaml"))
    }

    pub fn run_log_path(&self, n: u32) -> PathBuf {
        self.runs_dir().join(format!("{n}.log"))
    }

    pub fn run_prompt_path(&self, n: u32) -> PathBuf {
        self.runs_dir().join(format!("{n}.prompt.md"))
    }

    /// The next run number: one past the highest file in the folder.
    pub(super) fn next_run(&self) -> u32 {
        let highest = std::fs::read_dir(self.runs_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name();
                name.to_str()?.split('.').next()?.parse::<u32>().ok()
            })
            .max();
        highest.unwrap_or(0) + 1
    }

    /// Claims the next number: the record is created with a call that fails if it exists, so concurrent runs get distinct numbers.
    pub fn claim_run(&self) -> AgentResult<u32> {
        std::fs::create_dir_all(self.runs_dir())?;
        loop {
            let n = self.next_run();
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.run_path(n))
            {
                Ok(_) => return Ok(n),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }

    pub fn read_run(&self, n: u32) -> AgentResult<Option<RunRecord>> {
        match std::fs::read_to_string(self.run_path(n)) {
            Ok(text) => Ok(serde_yaml::from_str(&text)
                .map(Some)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn write_run(&self, run: &RunRecord) -> AgentResult<()> {
        std::fs::create_dir_all(self.runs_dir())?;
        let text = serde_yaml::to_string(run)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(crate::utils::fs::write_whole(
            &self.runs_dir(),
            &self.run_path(run.n),
            &text,
        )?)
    }

    /// Every run, first first; unreadable records are skipped.
    pub fn runs(&self) -> Vec<RunRecord> {
        let Ok(entries) = std::fs::read_dir(self.runs_dir()) else {
            return Vec::new();
        };
        let mut runs: Vec<RunRecord> = entries
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                let n: u32 = path.file_stem()?.to_str()?.parse().ok()?;
                (path.extension()? == "yaml")
                    .then(|| self.read_run(n).ok().flatten())
                    .flatten()
            })
            .collect();
        runs.sort_by_key(|r| r.n);
        runs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_harness_dispatches_to_its_backend() {
        let dirs = [Path::new("/tasks/7")];
        let allowed = ["Bash(git:*)".to_string()];
        let extra = ["--verbose".to_string()];
        let job = Invocation {
            model: "opus",
            effort: "",
            dirs: &dirs,
            allowed: &allowed,
            extra: &extra,
        };
        let (program, args) = Harness::Claude.command(&job);
        assert_eq!(program, "claude");
        assert_eq!(
            args,
            [
                "-p",
                "--model",
                "opus",
                "--add-dir",
                "/tasks/7",
                "--permission-mode",
                "acceptEdits",
                "--allowedTools",
                "Bash(git:*)",
                "--verbose",
            ]
        );
        assert_eq!(ClaudeCode.command(&job), (program, args));
    }

    fn dir(name: &str) -> TaskDir {
        TaskDir::new(crate::fixtures::scratch(&format!("run-{name}")))
    }

    fn run(n: u32) -> RunRecord {
        RunRecord {
            n,
            agent: Harness::Claude,
            model: "haiku".into(),
            effort: "medium".into(),
            kind: Kind::Develop,
            instruction: None,
            status: Status::Running,
            started: "2026-10-30 10:00:00".into(),
            ended: None,
            exit_code: None,
            branch: Some("feat/x".into()),
            worktree: None,
            pid: Some(42),
        }
    }

    #[test]
    fn runs_are_numbered_stored_and_listed_in_order() {
        let d = dir("list");
        assert_eq!((d.next_run(), d.runs()), (1, vec![]));
        d.write_run(&run(1)).unwrap();
        d.write_run(&run(2)).unwrap();
        let mut first = run(1);
        first.status = Status::Done;
        first.exit_code = Some(0);
        d.write_run(&first).unwrap();
        assert_eq!(d.next_run(), 3);
        assert_eq!(d.runs(), vec![first, run(2)]);
        assert_eq!(d.read_run(9).unwrap(), None);
    }

    fn ps(rows: &[&str]) -> crate::process::fake::FakeRunner {
        let runner = crate::process::fake::FakeRunner::default();
        runner.reply("ps ", &rows.join("\n"));
        runner
    }

    fn kills(runner: &crate::process::fake::FakeRunner) -> Vec<String> {
        let calls = runner.calls();
        calls
            .into_iter()
            .filter(|c| c.starts_with("kill"))
            .collect()
    }

    #[test]
    fn a_pid_below_two_is_never_signalled() {
        let runner = ps(&["0 0 Ss kernel_task", "1 1 Ss /sbin/launchd", "2 2 S iter"]);
        assert!(!interrupt_with(&runner, 0));
        assert!(!interrupt_with(&runner, 1));
        assert!(kills(&runner).is_empty());
        assert!(!alive(0) && !alive(1));
    }

    #[test]
    fn many_pids_cost_one_ps_and_zombies_are_not_alive() {
        let runner = ps(&["10 10 S iter", "11 11 Z iter", "12 12 R claude"]);
        let procs = Procs::scan(&runner);
        assert_eq!(runner.calls().len(), 1);
        assert!(procs.alive(10) && !procs.alive(11) && procs.alive(12));
        assert!(!procs.alive(13) && !procs.alive(0) && !procs.alive(1));
        assert_eq!(runner.calls().len(), 1, "asking again does not spawn");
    }

    #[test]
    fn a_group_leader_that_is_iter_or_claude_is_signalled_by_group() {
        let runner = ps(&["500 500 S /usr/local/bin/iter", "600 600 S+ claude"]);
        assert!(interrupt_with(&runner, 500));
        assert!(interrupt_with(&runner, 600));
        assert_eq!(kills(&runner), ["kill -TERM -- -500", "kill -TERM -- -600"]);
    }

    #[test]
    fn a_foreign_group_or_program_is_not_signalled() {
        let runner = ps(&[
            "500 1 S iter",
            "501 501 S vim",
            "502 502 Z iter",
            "503 503 S Google Chrome Helper",
        ]);
        for pid in [500, 501, 502, 503, 999] {
            assert!(!interrupt_with(&runner, pid), "{pid}");
        }
        assert!(kills(&runner).is_empty());
    }

    #[test]
    fn this_process_and_its_group_are_not_signalled() {
        let me = std::process::id();
        let runner = ps(&[
            &format!("{me} {me} S iter"),
            &format!("{} {me} S claude", me + 1),
        ]);
        assert!(!interrupt_with(&runner, me));
        assert!(!interrupt_with(&runner, me + 1));
        assert!(kills(&runner).is_empty());
    }

    #[test]
    fn a_program_path_with_spaces_is_read_whole() {
        let p = Proc::parse(" 77  77 S  /Applications/My App/iter").unwrap();
        assert_eq!((p.pid, p.pgid, p.comm.as_str()), (77, 77, "iter"));
        assert!(Proc::parse("garbage").is_none());
    }

    #[test]
    fn claude_runs_headless_with_the_prompt_and_the_model() {
        let dir = Path::new("/n/tasks/1");
        let (program, args) = Harness::Claude.command(&Invocation {
            model: "haiku",
            effort: "low",
            dirs: &[dir],
            allowed: &["Bash(git *)".into()],
            extra: &["--verbose".into()],
        });
        assert_eq!(program, "claude");
        assert_eq!(args[..5], ["-p", "--model", "haiku", "--effort", "low"]);
        assert!(args.windows(2).any(|w| w == ["--add-dir", "/n/tasks/1"]));
        assert!(
            args.windows(2)
                .any(|w| w == ["--allowedTools", "Bash(git *)"])
        );
        assert_eq!(args.last().map(String::as_str), Some("--verbose"));
        let (_, bare) = Harness::Claude.command(&Invocation::default());
        assert!(!bare.contains(&"--model".to_string()) && !bare.contains(&"--effort".to_string()));
    }
}
