//! Task subcommands.

use super::*;

#[derive(clap::Subcommand, Debug)]
pub enum TaskCommand {
    /// Open a new task in nvim (optionally pre-filled from a GitHub issue);
    /// saving & quitting creates it
    New {
        /// Project the task belongs to (default: the project of the tmux
        /// session you're in)
        #[arg(add = ArgValueCompleter::new(project_completer))]
        project: Option<String>,

        /// Pre-fill name/description/github_issue from this issue number
        #[clap(long)]
        issue: Option<i64>,
    },

    /// Edit an existing task in nvim; saving & quitting updates it
    Edit {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,
    },

    /// Delete a task (and its session-config and sessions)
    Delete {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,
    },

    /// Name, description, and every session on a day or over an interval
    /// (default: today)
    Info {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,

        #[clap(flatten)]
        report: ReportOpts,
    },

    /// List tasks, optionally filtered by project and/or status
    List {
        #[clap(long, add = ArgValueCompleter::new(project_completer))]
        project: Option<String>,

        /// queue, wip, or done
        #[clap(long)]
        status: Option<String>,
    },

    /// The plan of a task: its goal, how, and a checklist with the state of
    /// every step. Alone it opens the plan in the editor
    Plan {
        /// `<project>/<task>` (default: the task of the tmux session you're in).
        /// A name with a space needs a backslash: `app/Fix\ it`
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,

        /// Print the plan and how far along it is
        #[clap(long, short = 's', conflicts_with_all = ["stdin", "mark"])]
        show: bool,

        /// Replace the plan with the one on standard input. Steps get their
        /// ids; a missing Goal or Checklist is a warning, not a refusal
        #[clap(long, conflicts_with = "mark")]
        stdin: bool,

        /// Put one step in a state (todo, doing or done): `--mark s3 done`.
        /// Only its checkbox changes
        #[clap(long, num_args = 2, value_names = ["STEP", "STATE"])]
        mark: Vec<String>,

        /// Output format of --show
        #[clap(long, short = 'f', value_enum, default_value_t = DataFormat::Text)]
        format: DataFormat,
    },

    /// Print the prompt that tells an agent what to do with a task: plan it,
    /// work through its plan, fix it or investigate it
    Prompt {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,

        /// What the agent is to do
        #[clap(long, short = 'k', value_enum, default_value_t = PromptKind::Plan)]
        kind: PromptKind,

        /// Use this template instead of the task's, project's or built-in one
        #[clap(long)]
        template: Option<std::path::PathBuf>,

        /// Leave out what the project's wiki knows about the task
        #[clap(long)]
        no_onboarding: bool,
    },

    /// Hand a task to an agent: it works on its own, headless, in the
    /// task's worktree, and leaves a log. Opens the editor first (agent,
    /// model, effort and the description it is given), unless --yes
    Solve {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,

        /// What the agent is to do
        #[clap(long, short = 'k', value_enum, default_value_t = PromptKind::Develop)]
        kind: PromptKind,

        /// The harness that runs the agent
        #[clap(long, value_enum)]
        agent: Option<Harness>,

        #[clap(long)]
        model: Option<String>,

        /// low, medium or high
        #[clap(long)]
        effort: Option<String>,

        /// Skip the editor
        #[clap(long, short = 'y')]
        yes: bool,

        /// Stay until the agent has finished instead of leaving it running
        #[clap(long)]
        wait: bool,

        /// Passed to the harness as they are, after `--`
        #[clap(last = true)]
        extra: Vec<String>,
    },

    /// The runs of a task, or the log of one
    Runs {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,

        /// Print the log of this run
        #[clap(long)]
        log: Option<u32>,
    },

    /// What a detached `solve` runs: the harness itself, to the end
    #[clap(hide = true)]
    RunWorker {
        task_id: i64,
        run: u32,
        #[clap(last = true)]
        extra: Vec<String>,
    },

    /// Mark a task done: closes any open session, tears down its
    /// session-config (tmux session + worktree), and deletes that row
    Done {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,

        /// Land the work first: merge the project's `default_branch` into
        /// the task's worktree, then the task's branch into
        /// `default_branch`. A merge that stops is left half-done for you
        /// to finish, and the task stays as it was
        #[clap(long)]
        save: bool,
    },

    /// Bring the project's GitHub issues down as tasks: an issue nothing
    /// tracks becomes a new task, and a tracked one hands its open/closed
    /// state to the task's status
    Pull {
        /// Project to pull into (default: the project of the tmux session
        /// you're in)
        #[arg(add = ArgValueCompleter::new(project_completer))]
        project: Option<String>,

        /// Pull only this one task, by name, from the issue it tracks
        /// (default: every issue in the repo)
        #[clap(long, add = ArgValueCompleter::new(task_name_completer))]
        task: Option<String>,

        /// Also overwrite each task's description with its issue's body
        #[clap(long)]
        body: bool,
    },

    /// Send the project's tasks up to GitHub: a task tracking no issue gets
    /// one opened for it, and a tracked issue is closed or reopened to
    /// match its task's status
    Push {
        /// Project to push (default: the project of the tmux session you're
        /// in)
        #[arg(add = ArgValueCompleter::new(project_completer))]
        project: Option<String>,

        /// Push only this one task, by name (default: every task in the
        /// project)
        #[clap(long, add = ArgValueCompleter::new(task_name_completer))]
        task: Option<String>,

        /// Also overwrite each issue's body with its task's description
        #[clap(long)]
        body: bool,
    },

    /// Average hours per weekday spent on a task
    Weekday {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,
    },
}
