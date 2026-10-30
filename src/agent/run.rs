//! A run: one headless agent working on a task in its worktree. Each run is
//! a record (`runs/<n>.yaml`), the prompt it was given (`runs/<n>.prompt.md`)
//! and what the harness printed (`runs/<n>.log`), in the task's folder. Like
//! the plan, they are files: `iter task runs` and the Agent tab read the same
//! ones.

use crate::agent::taskdir::TaskDir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

    /// The program and the arguments that run `prompt` once, headless, to
    /// the end. `extra` goes last, for what the harness takes that iter does
    /// not know about.
    pub fn command(
        self,
        model: &str,
        prompt: &str,
        extra: &[String],
    ) -> (&'static str, Vec<String>) {
        match self {
            Harness::Claude => {
                let mut args = vec!["-p".to_string(), prompt.to_string()];
                if !model.is_empty() {
                    args.push("--model".into());
                    args.push(model.into());
                }
                // headless there is nobody to ask: edits in the worktree are
                // allowed, the rest is the harness's own settings
                args.push("--permission-mode".into());
                args.push("acceptEdits".into());
                args.extend(extra.iter().cloned());
                ("claude", args)
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Running,
    Done,
    Failed,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Running => "running",
            Status::Done => "done",
            Status::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub n: u32,
    pub agent: Harness,
    pub model: String,
    pub effort: String,
    /// What the agent was asked to do: `execute`, `fix` or `investigate`.
    pub kind: String,
    pub status: Status,
    /// `YYYY-MM-DD HH:MM:SS`, local time.
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
    /// is gone (killed, or the machine restarted) is `lost`.
    pub fn shown_status(&self) -> &'static str {
        match (self.status, self.pid) {
            (Status::Running, Some(pid)) if !alive(pid) => "lost",
            (status, _) => status.label(),
        }
    }
}

/// Whether process `pid` exists.
fn alive(pid: u32) -> bool {
    crate::process::succeeds("kill", None, &["-0".to_string(), pid.to_string()])
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

    /// The number the next run gets: one more than the highest so far.
    pub fn next_run(&self) -> u32 {
        self.runs().iter().map(|r| r.n).max().unwrap_or(0) + 1
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
        let tmp = self
            .runs_dir()
            .join(format!(".{}.{}.tmp", run.n, std::process::id()));
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, self.run_path(run.n))
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
            kind: "execute".into(),
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
        let (program, args) = Harness::Claude.command("haiku", "do it", &["--verbose".into()]);
        assert_eq!(program, "claude");
        assert_eq!(args[..4], ["-p", "do it", "--model", "haiku"]);
        assert_eq!(args.last().map(String::as_str), Some("--verbose"));
        // no model: the harness's own default
        assert!(
            !Harness::Claude
                .command("", "x", &[])
                .1
                .contains(&"--model".to_string())
        );
    }
}
