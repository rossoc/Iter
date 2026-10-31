//! A run: one headless agent working on a task in its worktree, kept as `runs/<n>.yaml`, `.prompt.md` and `.log` in the task's folder.

use crate::agent::error::AgentResult;
use crate::agent::prompt::Kind;
use crate::agent::taskdir::TaskDir;
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
    pub prompt: &'a str,
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

/// How one agent program is driven; `Harness::backend` dispatches here.
pub trait AgentBackend {
    fn command(&self, job: &Invocation) -> (&'static str, Vec<String>);
}

struct ClaudeCode;

impl AgentBackend for ClaudeCode {
    fn command(&self, job: &Invocation) -> (&'static str, Vec<String>) {
        let Invocation {
            prompt,
            model,
            effort,
            dirs,
            allowed,
            extra,
        } = *job;
        let mut args = vec!["-p".to_string(), prompt.to_string()];
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

/// Whether `pid` exists and is not a zombie (`kill -0` answers for those).
pub fn alive(pid: u32) -> bool {
    let pid = pid.to_string();
    if !crate::process::succeeds("kill", None, &["-0".to_string(), pid.clone()]) {
        return false;
    }
    crate::process::output_trimmed("ps", None, &["-o", "stat=", "-p", &pid])
        .is_none_or(|stat| !stat.starts_with('Z'))
}

/// Asks a run's process to stop: its whole group when it leads one, else the process.
pub fn interrupt(pid: u32) -> bool {
    let pid = pid.to_string();
    let args = |target: String| ["-TERM".to_string(), "--".to_string(), target];
    crate::process::succeeds("kill", None, &args(format!("-{pid}")))
        || crate::process::succeeds("kill", None, &args(pid))
}

impl TaskDir {
    pub fn runs_dir(&self) -> PathBuf {
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
    pub fn next_run(&self) -> u32 {
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
        Ok(crate::agent::taskdir::write_whole(
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
            prompt: "do it",
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
                "do it",
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

    #[test]
    fn claude_runs_headless_with_the_prompt_and_the_model() {
        let dir = Path::new("/n/tasks/1");
        let (program, args) = Harness::Claude.command(&Invocation {
            prompt: "do it",
            model: "haiku",
            effort: "low",
            dirs: &[dir],
            allowed: &["Bash(git *)".into()],
            extra: &["--verbose".into()],
        });
        assert_eq!(program, "claude");
        assert_eq!(
            args[..6],
            ["-p", "do it", "--model", "haiku", "--effort", "low"]
        );
        assert!(args.windows(2).any(|w| w == ["--add-dir", "/n/tasks/1"]));
        assert!(
            args.windows(2)
                .any(|w| w == ["--allowedTools", "Bash(git *)"])
        );
        assert_eq!(args.last().map(String::as_str), Some("--verbose"));
        let (_, bare) = Harness::Claude.command(&Invocation {
            prompt: "x",
            ..Invocation::default()
        });
        assert!(!bare.contains(&"--model".to_string()) && !bare.contains(&"--effort".to_string()));
    }
}
