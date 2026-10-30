use crate::agent::prompt::Kind as PromptKind;
use crate::agent::run::Harness;
use crate::completion::{
    board_completer, organization_completer, project_completer, queued_task_completer,
    tag_completer, task_completer, task_name_completer,
};
use crate::reporting::Format;
use crate::utils::output::Format as DataFormat;
use crate::wiki::retrieve::Depth;
use clap::Parser;
use clap_complete::engine::ArgValueCompleter;

/// The period an `info` report covers, and how it's printed.
///
/// Two mutually exclusive ways to say which days: `--date` for a single one
/// (the default, today), or `--from`/`--to` for an interval. Either bound of
/// the interval may be left off -- `--from` alone runs up to today, `--to`
/// alone reaches back over every session there is.
#[derive(clap::Args, Debug)]
pub struct ReportOpts {
    /// A single day to report on, YYYY-MM-DD (default: today)
    #[clap(long, conflicts_with_all = ["from", "to"])]
    pub date: Option<String>,

    /// First day of an interval, YYYY-MM-DD (default: no lower bound)
    #[clap(long)]
    pub from: Option<String>,

    /// Last day of an interval, YYYY-MM-DD (default: today)
    #[clap(long)]
    pub to: Option<String>,

    /// Output format
    #[clap(long, short = 'f', value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

/// A project/task/session time tracker, with tmux + GitHub integration.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Create a project from the current directory (cwd becomes base_path)
    Init {
        /// Organization to put the project in; its settings become the
        /// project's starting defaults (optional -- a project need not
        /// belong to one)
        #[clap(long = "organization", short = 'o',
               add = ArgValueCompleter::new(organization_completer))]
        organization: Option<String>,
    },

    /// Create a project from scratch in a new folder
    New {
        /// Directory to create; becomes the project's base_path
        path: String,

        /// Organization to put the project in; its settings become the
        /// project's starting defaults (optional -- a project need not
        /// belong to one)
        #[clap(long = "organization", short = 'o',
               add = ArgValueCompleter::new(organization_completer))]
        organization: Option<String>,
    },

    /// Create a project from a template project (copies its fields) or a
    /// git/GitHub repo (like `git clone`)
    Clone {
        /// An existing project's name, or a git remote URL / local repo
        /// path if no such project exists
        #[arg(add = ArgValueCompleter::new(project_completer))]
        source: String,

        /// Organization to put the project in; its settings become the
        /// project's starting defaults (optional -- a project need not
        /// belong to one)
        #[clap(long = "organization", short = 'o',
               add = ArgValueCompleter::new(organization_completer))]
        organization: Option<String>,

        /// Bind the new project to this board (when cloning a project, the
        /// clone otherwise lands on that project's board, if any)
        #[clap(long, short = 'b', add = ArgValueCompleter::new(board_completer))]
        board: Option<String>,

        /// Leave the new project on no board, even if the project it was
        /// cloned from is on one
        #[clap(long, conflicts_with = "board")]
        no_board: bool,
    },

    /// Manage tags: coloured labels shared by every task
    Tag {
        #[clap(subcommand)]
        action: TagCommand,
    },

    /// Manage boards: calendars that hold projects
    Board {
        #[clap(subcommand)]
        action: BoardCommand,
    },

    /// Manage organizations: groups of projects that share defaults
    #[clap(visible_alias = "org")]
    Organization {
        #[clap(subcommand)]
        action: OrganizationCommand,
    },

    /// Manage projects; with no subcommand, lists every project by name
    Project {
        #[clap(subcommand)]
        action: Option<ProjectCommand>,
    },

    /// Manage tasks; with no subcommand, lists every unfinished (queue or
    /// wip) task
    Task {
        #[clap(subcommand)]
        action: Option<TaskCommand>,
    },

    /// Read the wiki built from the notes: find the pages worth loading,
    /// follow their relations, query the graph, keep it consistent
    Wiki {
        #[clap(subcommand)]
        action: WikiCommand,
    },

    /// Daily notes of a project or an organization
    Notes {
        #[clap(subcommand)]
        action: NotesCommand,
    },

    /// Set up and track a task's session (tmux + git worktree/branch, and
    /// time spent)
    Session {
        #[clap(subcommand)]
        action: SessionCommand,
    },

    /// Post a message as a comment on a task's linked GitHub issue
    Comment {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,

        /// The comment body
        #[clap(short = 'm', long = "message")]
        message: String,
    },

    /// Print `<project>/<task>` for the tmux session you're currently in
    T,

    /// Browse and edit organizations, projects and tasks in the browser
    /// (a local server over the same database the CLI uses)
    #[cfg(feature = "web")]
    Serve {
        /// Port to listen on (127.0.0.1 only)
        #[clap(long, short = 'p', default_value_t = 3000)]
        port: u16,

        /// Don't open the browser
        #[clap(long)]
        no_open: bool,
    },

    /// Commands invoked by tmux hooks -- not meant to be run by hand
    #[clap(hide = true)]
    Internal {
        #[clap(subcommand)]
        action: InternalCommand,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum ProjectCommand {
    /// Open a blank project in nvim; saving & quitting creates it
    New {
        /// Organization to put the project in; its settings become the
        /// project's starting defaults (optional -- a project need not
        /// belong to one)
        #[clap(long = "organization", short = 'o',
               add = ArgValueCompleter::new(organization_completer))]
        organization: Option<String>,

        /// Board to bind the project to (optional)
        #[clap(long, short = 'b', add = ArgValueCompleter::new(board_completer))]
        board: Option<String>,
    },

    /// Edit an existing project in nvim; saving & quitting updates it
    Edit {
        /// Default: the project of the tmux session you're in
        #[arg(add = ArgValueCompleter::new(project_completer))]
        name: Option<String>,

        /// Move the project into this organization (its current
        /// organization, if any, is kept when this isn't given)
        #[clap(long = "organization", short = 'o',
               add = ArgValueCompleter::new(organization_completer))]
        organization: Option<String>,

        /// Bind the project to this board (its current board, if any, is
        /// kept when this isn't given)
        #[clap(long, short = 'b', add = ArgValueCompleter::new(board_completer))]
        board: Option<String>,

        /// Unbind the project from its board
        #[clap(long, conflicts_with = "board")]
        no_board: bool,
    },

    /// Delete a project (and its tasks, session-configs and sessions)
    Delete {
        /// Default: the project of the tmux session you're in
        #[arg(add = ArgValueCompleter::new(project_completer))]
        name: Option<String>,
    },

    /// Name, description, and time spent (union of all its tasks) over a
    /// day or an interval, broken down per task
    Info {
        /// Default: the project of the tmux session you're in
        #[arg(add = ArgValueCompleter::new(project_completer))]
        name: Option<String>,

        #[clap(flatten)]
        report: ReportOpts,
    },

    /// List every project
    List,
}

#[derive(clap::Subcommand, Debug)]
pub enum TagCommand {
    /// Open a blank tag in nvim; saving & quitting creates it
    New,

    /// Edit an existing tag in nvim; saving & quitting updates it
    Edit {
        #[arg(add = ArgValueCompleter::new(tag_completer))]
        name: String,
    },

    /// Delete a tag; it's removed from every task that had it
    Delete {
        #[arg(add = ArgValueCompleter::new(tag_completer))]
        name: String,
    },

    /// A tag's color and description
    Info {
        #[arg(add = ArgValueCompleter::new(tag_completer))]
        name: String,
    },

    /// List every tag
    List,
}

#[derive(clap::Subcommand, Debug)]
pub enum BoardCommand {
    /// Open a blank board in nvim; saving & quitting creates it
    New,

    /// Edit an existing board in nvim; saving & quitting updates it
    Edit {
        /// Default: the board of the tmux session's project
        #[arg(add = ArgValueCompleter::new(board_completer))]
        name: Option<String>,
    },

    /// Delete a board; its projects are kept, no longer bound to one
    Delete {
        /// Default: the board of the tmux session's project
        #[arg(add = ArgValueCompleter::new(board_completer))]
        name: Option<String>,
    },

    /// A board's description and the projects bound to it
    Info {
        /// Default: the board of the tmux session's project
        #[arg(add = ArgValueCompleter::new(board_completer))]
        name: Option<String>,
    },

    /// List every board
    List,
}

#[derive(clap::Subcommand, Debug)]
pub enum OrganizationCommand {
    /// Open a blank organization in nvim; saving & quitting creates it
    New,

    /// Edit an existing organization (and which projects are in it) in
    /// nvim; saving & quitting updates it
    Edit {
        /// Default: the organization of the tmux session's project
        #[arg(add = ArgValueCompleter::new(organization_completer))]
        name: Option<String>,
    },

    /// Delete an organization; its projects are kept, no longer in one
    Delete {
        /// Default: the organization of the tmux session's project
        #[arg(add = ArgValueCompleter::new(organization_completer))]
        name: Option<String>,
    },

    /// Time spent across the organization over a day or an interval, broken
    /// down per project and per task
    Info {
        /// Default: the organization of the tmux session's project
        #[arg(add = ArgValueCompleter::new(organization_completer))]
        name: Option<String>,

        #[clap(flatten)]
        report: ReportOpts,
    },

    /// List every organization
    List,
}

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
        #[clap(long, short = 'k', value_enum, default_value_t = PromptKind::Execute)]
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

#[derive(clap::Subcommand, Debug)]
pub enum SessionCommand {
    /// Set up a session for a task: creates a git worktree/branch (unless
    /// the project has no github or auto_branch is off) and, if the
    /// project has tmux enabled, a tmux session named `<project>/<task>`
    New {
        /// `<project>/<task>` (completion offers only `queue` tasks)
        #[arg(add = ArgValueCompleter::new(queued_task_completer))]
        task: String,

        /// Override the branch name (default: the task's branch_prefix
        /// followed by its slugified name)
        #[clap(short = 'b', long = "branch")]
        branch: Option<String>,

        /// Skip branch/worktree creation entirely
        #[clap(long = "no-branch")]
        no_branch: bool,
    },

    /// Manually start a session for a task (for tmux-less projects, or when
    /// the tmux hooks aren't in play)
    Start {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,
    },

    /// Manually end the open session for a task
    Stop {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,

        /// Note stored alongside the closed session
        #[clap(short = 'm', long = "message")]
        message: Option<String>,
    },

    /// Time spent so far on the currently open session of the task of the
    /// tmux session you're in
    Elapse,
}

#[derive(clap::Subcommand, Debug)]
pub enum InternalCommand {
    /// Called by a tmux hook; see `tmux::HOOKS` for which format variable
    /// fills which argument
    Hook {
        #[clap(subcommand)]
        event: HookEvent,
    },

    /// Stops the clock of every task with a tmux session, then kills the
    /// tmux server -- tmux has no hook of its own for the server exiting
    KillServer,

    /// Re-links iter's sessions to a freshly started tmux server: recreates
    /// the tmux sessions it lacks, installs the hooks, closes the clocks the
    /// previous server left open and starts those of attached sessions.
    /// Meant for tmux-resurrect's `@resurrect-hook-post-restore-all`
    Restore,
}

/// The tmux events `iter` tracks time on, each with exactly the arguments
/// that name the sessions it concerns.
#[derive(clap::Subcommand, Debug)]
pub enum HookEvent {
    /// `client` now shows `session`, having left `previous` (empty on a
    /// first attach)
    ClientSessionChanged {
        client: String,
        session: String,
        previous: Option<String>,
    },
    /// `client` is gone; which session it was on is what `iter` recorded
    /// the last time it changed
    ClientDetached { client: String },
    /// `session` no longer exists
    SessionClosed { session: String },
}

/// Which wiki a `wiki` command sees: a task's (its own folder, its project's
/// wiki, its organization's), a project's or an organization's. With none of
/// the three, the task of the tmux session you're in.
#[derive(clap::Args, Debug)]
pub struct WikiScope {
    /// `<project>/<task>`
    #[clap(long, short = 't', add = ArgValueCompleter::new(task_completer),
           conflicts_with_all = ["project", "org"])]
    pub task: Option<String>,

    /// A project: its wiki and its organization's
    #[clap(long, short = 'p', add = ArgValueCompleter::new(project_completer),
           conflicts_with = "org")]
    pub project: Option<String>,

    /// An organization
    #[clap(long = "org", short = 'o', add = ArgValueCompleter::new(organization_completer))]
    pub org: Option<String>,

    /// With an organization: also the wikis of its projects
    #[clap(long, requires = "org")]
    pub down: bool,

    /// Output format
    #[clap(long, short = 'f', value_enum, default_value_t = DataFormat::Text)]
    pub format: DataFormat,
}

#[derive(clap::Subcommand, Debug)]
pub enum WikiCommand {
    /// Rank pages by how well their path, title, tags and description match the
    /// words, best first, until the token budget is spent
    Find {
        /// What to look for
        #[clap(required = true)]
        words: Vec<String>,

        /// Stop after about this many tokens
        #[clap(long, short = 'b', default_value_t = 800)]
        budget: usize,

        #[clap(flatten)]
        scope: WikiScope,
    },

    /// Search the text of the pages for a regular expression
    Grep {
        pattern: String,

        /// Search the raw notes too
        #[clap(long)]
        notes: bool,

        /// Match case
        #[clap(long)]
        case: bool,

        /// Stop after about this many tokens
        #[clap(long, short = 'b', default_value_t = 800)]
        budget: usize,

        #[clap(flatten)]
        scope: WikiScope,
    },

    /// One page: `<path>` in the level you ask from, `<level>:<path>` in another
    Show {
        id: String,

        /// How much of it
        #[clap(long, short = 'd', value_enum, default_value_t = Depth::Summary)]
        depth: Depth,

        #[clap(flatten)]
        scope: WikiScope,
    },

    /// The pages within a few relations of one, nearest first
    Neighbors {
        id: String,

        /// How many relations away
        #[clap(long, default_value_t = 1)]
        hops: usize,

        /// Only follow these relations (nb, include, source, referencedBy...)
        #[clap(long)]
        prop: Vec<String>,

        #[clap(flatten)]
        scope: WikiScope,
    },

    /// Lint the wiki: dangling links, missing descriptions and tags, broken
    /// front matter, relations the ontology does not allow, and what to do
    /// next. Fails if there are errors
    Check {
        #[clap(flatten)]
        scope: WikiScope,
    },

    /// Move a page inside its level, rewriting every reference to it. Only the
    /// levels in scope are rewritten: references from other tasks' folders or
    /// from an organization's projects are not (run `check` there)
    Mv {
        /// The page
        from: String,

        /// Its new path in the same level, without `.md`
        to: String,

        /// Say what would change and change nothing
        #[clap(long)]
        dry_run: bool,

        #[clap(flatten)]
        scope: WikiScope,
    },

    /// Run a SPARQL query over the graph (`iter:`, `owl:`, `rdf:`, `rdfs:`,
    /// `xsd:` are declared). Reads only
    Sparql {
        /// The query (or `-` for standard input)
        query: String,

        #[clap(flatten)]
        scope: WikiScope,
    },

    /// The whole graph as N-Triples, with what the ontology derives
    Export {
        #[clap(flatten)]
        scope: WikiScope,
    },

    /// The instructions for an agent working with the wiki, filled in from
    /// the ontology
    Schema {
        #[clap(flatten)]
        scope: WikiScope,
    },

    /// Have an agent turn the notes that changed (or were never ingested)
    /// into pages. Its changes are left uncommitted, to be reviewed with
    /// `pending`. Needs the notes folder to be in a git repository
    Ingest {
        /// A project (default: the one of the tmux session you're in)
        #[clap(long, short = 'p', add = ArgValueCompleter::new(project_completer),
               conflicts_with = "task")]
        project: Option<String>,

        /// A finished task: its sketches are ingested too
        #[clap(long, short = 't', add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,

        /// Only list what is waiting and when it would run on its own
        #[clap(long)]
        dry_run: bool,

        /// Leave the agent running and return
        #[clap(long)]
        detach: bool,

        #[clap(flatten)]
        harness: HarnessOpts,
    },

    /// Have an agent write a concept that includes the given ideas
    BuildConcept {
        /// The ideas (`ideas/retry-budget`), at least two
        #[clap(required = true, num_args = 2..)]
        ideas: Vec<String>,

        /// The concept's file name, without `.md`
        #[clap(long)]
        name: String,

        /// A project (default: the one of the tmux session you're in)
        #[clap(long, short = 'p', add = ArgValueCompleter::new(project_completer))]
        project: Option<String>,

        #[clap(flatten)]
        harness: HarnessOpts,
    },

    /// What the agents changed in the wiki and nobody has accepted yet
    Pending {
        /// A project (default: the one of the tmux session you're in)
        #[clap(long, short = 'p', add = ArgValueCompleter::new(project_completer))]
        project: Option<String>,

        /// Commit the changes
        #[clap(long, conflicts_with = "revert")]
        accept: bool,

        /// Throw away every uncommitted change under wiki/, yours too
        #[clap(long)]
        revert: bool,
    },
}

/// How a job that runs an agent is run (the agent of `iter wiki ingest` and
/// `iter wiki build-concept`).
#[derive(clap::Args, Debug)]
pub struct HarnessOpts {
    /// The harness that runs the agent
    #[clap(long, value_enum)]
    pub agent: Option<Harness>,

    /// The model (the harness's own default when left out)
    #[clap(long)]
    pub model: Option<String>,

    /// Passed to the harness as they are, after `--`
    #[clap(last = true)]
    pub extra: Vec<String>,
}

/// Which notes: a project's or an organization's (default: the project of the
/// tmux session you're in).
#[derive(clap::Args, Debug)]
pub struct NotesScope {
    #[clap(long, short = 'p', add = ArgValueCompleter::new(project_completer),
           conflicts_with = "org")]
    pub project: Option<String>,

    #[clap(long = "org", short = 'o', add = ArgValueCompleter::new(organization_completer))]
    pub org: Option<String>,
}

#[derive(clap::Subcommand, Debug)]
pub enum NotesCommand {
    /// Make today's note if it is not there and print its path
    Today {
        #[clap(flatten)]
        scope: NotesScope,
    },

    /// List the notes, latest first
    List {
        #[clap(flatten)]
        scope: NotesScope,
    },

    /// Print a note (default: today's)
    Show {
        /// YYYY-MM-DD
        date: Option<String>,

        #[clap(flatten)]
        scope: NotesScope,
    },
}
