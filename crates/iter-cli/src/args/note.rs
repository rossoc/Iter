//! `iter note` subcommands: daily notes, a task's notes and sketches, wiki pages.

use super::*;

/// Which notes: a project's or an organization's (default: the current project, the one of the
/// task, worktree, folder or tmux session you are in).
#[derive(clap::Args, Debug, Default)]
pub struct NotesScope {
    #[clap(long, short = 'p', add = ArgValueCompleter::new(project_completer),
           conflicts_with = "org")]
    pub project: Option<String>,

    #[clap(long = "org", short = 'o', add = ArgValueCompleter::new(organization_completer))]
    pub org: Option<String>,
}

/// Which task's notes or sketches (default: the current task: `ITER_TASK`, the worktree you are
/// in, or the tmux session).
#[derive(clap::Args, Debug)]
pub struct TaskScope {
    /// `<project>/<task>` or the task's id
    #[clap(long, short = 't', global = true, add = ArgValueCompleter::new(task_completer))]
    pub task: Option<String>,
}

#[derive(clap::Subcommand, Debug)]
pub enum NoteCommand {
    /// Make today's note if it is not there and print its path (also `iter note` alone)
    Today {
        #[clap(flatten)]
        scope: NotesScope,
    },

    /// List the daily notes, latest first
    List {
        #[clap(flatten)]
        scope: NotesScope,
    },

    /// Print a daily note (default: today's)
    Show {
        /// YYYY-MM-DD
        date: Option<String>,

        #[clap(flatten)]
        scope: NotesScope,
    },

    /// The agent's sketches of a task (`tasks/<id>/sketches/`): its working memory
    Sketch {
        #[clap(flatten)]
        task: TaskScope,
        #[clap(subcommand)]
        action: DocCommand,
    },

    /// A task's own notes (`tasks/<id>/notes/`): the user's
    Task {
        #[clap(flatten)]
        task: TaskScope,
        #[clap(subcommand)]
        action: DocCommand,
    },

    /// Wiki Ideas (`wiki/ideas/`): provisional hypotheses
    Idea {
        #[clap(flatten)]
        scope: NotesScope,
        #[clap(subcommand)]
        action: DocCommand,
    },

    /// Wiki Concepts (`wiki/concepts/`): grounded pieces of knowledge
    Concept {
        #[clap(flatten)]
        scope: NotesScope,
        #[clap(subcommand)]
        action: DocCommand,
    },

    /// Wiki Entities (`wiki/entities/`): people, tools, services
    Entity {
        #[clap(flatten)]
        scope: NotesScope,
        #[clap(subcommand)]
        action: DocCommand,
    },

    /// Wiki Syntheses (`wiki/syntheses/`): analyses across pages
    Synthesis {
        #[clap(flatten)]
        scope: NotesScope,
        #[clap(subcommand)]
        action: DocCommand,
    },

    /// Print any document by its address
    #[clap(after_help = ADDRESSES)]
    Read {
        /// e.g. `sketch/cause`, `acme/app/wiki/concepts/backoff`, `app/tasks/7/note/ideas`
        address: String,
    },

    /// Print the path of any document by its address (it need not exist yet)
    #[clap(after_help = ADDRESSES)]
    Path { address: String },
}

const ADDRESSES: &str = "Addresses (what is left out comes from where you are):
  [org/][project/]note[/YYYY-MM-DD]          a daily note (today)
  [org/][project/]wiki/<class>/<name>        a wiki page (org/wiki/... for the organization's)
  [org/][project/]tasks/<task>/sketch/<name> a task's sketch (note/<name>: its notes)
  sketch/<name>, note/<name>                 the current task's";

/// What to do with one kind of document.
#[derive(clap::Subcommand, Debug)]
pub enum DocCommand {
    /// The documents, one per line: name, then description or title
    List,

    /// Write a new document. The text is `-m`, else standard input when it is not a terminal,
    /// else the editor. Refused if it exists, unless `--append` or `--replace`.
    New {
        /// The name; the file is its lowercase-and-dashes form
        name: String,

        /// The text
        #[clap(long, short = 'm')]
        message: Option<String>,

        /// Wiki pages: the one-line description (required unless the text has front matter)
        #[clap(long, short = 'd')]
        description: Option<String>,

        /// Wiki pages: tags, comma-separated (required unless the text has front matter)
        #[clap(long, value_delimiter = ',')]
        tags: Vec<String>,

        /// Add the text at the end of an existing document (or make it)
        #[clap(long, conflicts_with = "replace")]
        append: bool,

        /// Replace an existing document
        #[clap(long)]
        replace: bool,
    },

    /// Print a document
    Read { name: String },

    /// Print a document's path
    Path { name: String },
}
