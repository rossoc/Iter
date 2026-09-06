/// A live (or about-to-be-live) unit of work on a `wip` task: a tmux
/// session (when the project has tmux enabled) tied to a git worktree/branch
/// (when the project is a github-backed repo). Not edited via YAML -- it's
/// entirely derived from the task and project config at `iter session new`
/// time -- so it carries no serde derives.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: Option<i64>,
    pub task_id: i64,
    pub tmux_session_name: Option<String>,
    pub github_branch: Option<String>,
    pub worktree_path: Option<String>,
}
