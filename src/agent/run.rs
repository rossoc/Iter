//! A run: one headless agent working on a task in its worktree. Each run is
//! a record (`runs/<n>.yaml`), the prompt it was given (`runs/<n>.prompt.md`)
//! and what the harness printed (`runs/<n>.log`), in the task's folder. Like
//! the plan, they are files: `iter task runs` and the Agent tab read the same
//! ones.

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

impl Harness {
    pub fn key(self) -> &'static str {
        match self {
            Harness::Claude => "claude",
        }
    }

    /// The models the harness takes, the ones a form offers: its own aliases
    /// and names, unless `configured` (`agent.models`) says otherwise.
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

    /// How hard the harness can be told to work.
    pub fn efforts(self) -> &'static [&'static str] {
        match self {
            Harness::Claude => &["low", "medium", "high", "xhigh", "max"],
        }
    }

    /// The program and the arguments that run `prompt` once, headless, to
    /// the end. `dirs` are folders besides the working one the agent may
    /// read and write (the task's folder). `extra` goes last, for what the
    /// harness takes that iter does not know about.
    pub fn command(
        self,
        model: &str,
        effort: &str,
        prompt: &str,
        dirs: &[&Path],
        allowed: &[String],
        extra: &[String],
    ) -> (&'static str, Vec<String>) {
        match self {
            Harness::Claude => {
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
                // headless there is nobody to ask: edits are allowed, and so
                // are the commands the prompts tell the agent to run; the
                // rest is the harness's own settings
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
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Written down, not started: it begins when asked to, or when the run
    /// before it ends.
    Queued,
    Running,
    Done,
    Failed,
    /// Stopped by the user, while it waited or while it worked.
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
pub struct Run {
    pub n: u32,
    pub agent: Harness,
    pub model: String,
    pub effort: String,
    /// What the agent was asked to do: `plan`, `execute`, `fix` or `investigate`.
    pub kind: String,
    /// The instruction it was given (`instructions/<name>.md`); a run from
    /// before there were any has none, and its kind is the name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
    pub status: Status,
    /// `YYYY-MM-DD HH:MM:SS`, local time: when the run began (a queued one
    /// has not: this is when it was written down).
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

impl Run {
    /// The status to show: a run that says it is running but whose process
    /// is gone (killed, or the machine restarted) is `lost`. One that never
    /// got a process (the start failed halfway) is `lost` once it is a
    /// minute old.
    pub fn shown_status(&self) -> &'static str {
        match (self.status, self.pid) {
            (Status::Running, Some(pid)) if !alive(pid) => "lost",
            (Status::Running, None) if self.age_seconds() > 60 => "lost",
            (status, _) => status.label(),
        }
    }

    /// The name of the instruction the run was given.
    pub fn instruction_name(&self) -> &str {
        self.instruction.as_deref().unwrap_or(&self.kind)
    }

    /// Whether the run waits for its turn.
    pub fn is_queued(&self) -> bool {
        self.status == Status::Queued
    }

    /// Whether an agent is working on this run.
    pub fn is_running(&self) -> bool {
        self.shown_status() == "running"
    }

    fn age_seconds(&self) -> i64 {
        chrono::NaiveDateTime::parse_from_str(&self.started, "%Y-%m-%d %H:%M:%S")
            .map(|t| (chrono::Local::now().naive_local() - t).num_seconds())
            .unwrap_or(i64::MAX)
    }
}

/// Whether process `pid` exists and is not a zombie (a child nobody has
/// waited for still answers `kill -0`).
pub fn alive(pid: u32) -> bool {
    let pid = pid.to_string();
    if !crate::process::succeeds("kill", None, &["-0".to_string(), pid.clone()]) {
        return false;
    }
    crate::process::output_trimmed("ps", None, &["-o", "stat=", "-p", &pid])
        .is_none_or(|stat| !stat.starts_with('Z'))
}

/// Asks the process of a run to stop: its whole group when it leads one (a
/// detached worker and the harness under it), else the process alone.
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

    /// The number the next run gets: one more than the highest file in the
    /// folder, readable or not.
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

    /// Takes the next number for good: the record file is made, empty, with
    /// a call that fails if it is there, so two runs started at once get two
    /// numbers.
    pub fn claim_run(&self) -> std::io::Result<u32> {
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
                Err(e) => return Err(e),
            }
        }
    }

    pub fn read_run(&self, n: u32) -> std::io::Result<Option<Run>> {
        match std::fs::read_to_string(self.run_path(n)) {
            Ok(text) => serde_yaml::from_str(&text)
                .map(Some)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn write_run(&self, run: &Run) -> std::io::Result<()> {
        std::fs::create_dir_all(self.runs_dir())?;
        let text = serde_yaml::to_string(run)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        crate::agent::taskdir::write_whole(&self.runs_dir(), &self.run_path(run.n), &text)
    }

    /// Every run, the first one first. A record that cannot be read is left
    /// out.
    pub fn runs(&self) -> Vec<Run> {
        let Ok(entries) = std::fs::read_dir(self.runs_dir()) else {
            return Vec::new();
        };
        let mut runs: Vec<Run> = entries
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

    fn dir(name: &str) -> TaskDir {
        let root = std::env::temp_dir().join(format!("iter-run-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        TaskDir::new(root)
    }

    fn run(n: u32) -> Run {
        Run {
            n,
            agent: Harness::Claude,
            model: "haiku".into(),
            effort: "medium".into(),
            kind: "develop".into(),
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
        let (program, args) = Harness::Claude.command(
            "haiku",
            "low",
            "do it",
            &[dir],
            &["Bash(git *)".into()],
            &["--verbose".into()],
        );
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
        // no model, no effort: the harness's own defaults
        let (_, bare) = Harness::Claude.command("", "", "x", &[], &[], &[]);
        assert!(!bare.contains(&"--model".to_string()) && !bare.contains(&"--effort".to_string()));
    }
}
