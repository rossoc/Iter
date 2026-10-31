//! Wiki subcommands, the scope they read and the harness options of the agent ones.

use super::*;

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
#[derive(clap::Args, Debug, Default)]
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
