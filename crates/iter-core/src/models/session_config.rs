use iter_macros::Table;

/// The tmux session and git worktree/branch set up for a `wip` task, derived from task and project
/// config at `iter session new` time. No serde derives. Distinct from `Session`, the record of
/// time spent.
#[derive(Debug, Clone, Table)]
#[table(name = "session_configs")]
pub struct SessionConfig {
    pub id: Option<i64>,
    pub task_id: i64,
    pub tmux_session_name: Option<String>,
    pub github_branch: Option<String>,
    pub worktree_path: Option<String>,
}
