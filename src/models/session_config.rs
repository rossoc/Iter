use iter_macros::Table;

/// The tmux + git worktree/branch environment set up for a `wip` task: a
/// tmux session (when the project has tmux enabled) tied to a git
/// worktree/branch (when the project is a github-backed repo). Not edited
/// via YAML -- it's entirely derived from the task and project config at
/// `iter session new` time -- so it carries no serde derives. Distinct from
/// `Session`, which is the started/stopped record of time actually spent.
#[derive(Debug, Clone, Table)]
#[table(name = "session_configs")]
pub struct SessionConfig {
    pub id: Option<i64>,
    pub task_id: i64,
    pub tmux_session_name: Option<String>,
    pub github_branch: Option<String>,
    pub worktree_path: Option<String>,
}
