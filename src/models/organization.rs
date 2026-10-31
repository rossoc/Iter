use crate::models::{Configured, Settings, default_branch_template, default_true, main_branch};
use iter_macros::{MarkdownBody, Table};
use serde::{Deserialize, Serialize};

/// A grouping of projects and the defaults new projects in it start from. No `base_path`: it is
/// settings and a roster, not a place on disk.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Table, MarkdownBody)]
#[table(
    name = "organizations",
    order_by = "name",
    kind = "organization",
    check = "check"
)]
pub struct Organization {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<i64>,

    pub name: String,

    #[serde(skip)]
    pub description: String,

    #[serde(default)]
    pub github: bool,

    #[serde(default = "default_true")]
    pub tmux: bool,

    #[serde(default = "default_true")]
    pub auto_branch: bool,

    #[serde(default = "default_branch_template")]
    pub branch_template: String,

    #[serde(default = "main_branch")]
    pub default_branch: String,

    #[serde(default)]
    pub github_project: String,

    /// Daily-notes folder (`~` expanded when read), also the parent for its projects' notes.
    /// Empty means no notes.
    #[serde(default)]
    pub notes_path: String,

    /// Time-platform endpoint; empty means none. The API key lives in `crate::secret`.
    #[serde(default)]
    pub platform_url: String,

    /// Platform organization entries are logged under; defaults to this organization's name.
    /// Editable per share.
    #[serde(default)]
    pub platform_project: String,

    /// Platform activity entries are logged under. Editable per share.
    #[serde(default)]
    pub platform_activity: String,
}

impl Organization {
    /// Beyond a name: a platform URL must be `https` (calls carry the API key as a bearer token) or
    /// `http` to this machine (local platform, test server).
    fn check(&mut self) -> crate::error::Result<()> {
        self.platform_url = self.platform_url.trim().to_string();
        let url = self.platform_url.as_str();
        if url.is_empty() || url.starts_with("https://") {
            return Ok(());
        }
        let local = ["http://localhost", "http://127.0.0.1", "http://[::1]"]
            .iter()
            .any(|prefix| {
                url.strip_prefix(prefix)
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with([':', '/']))
            });
        match local {
            true => Ok(()),
            false => Err(crate::error::IterError::Platform(format!(
                "platform_url '{url}' must start with https:// (the API key is sent with every request)"
            ))),
        }
    }

    /// A blank template for `iter organization new`, starting from the project-defaults block.
    pub fn template(defaults: &Settings) -> Self {
        let mut organization = Organization::default();
        organization.set_settings(defaults.clone());
        organization
    }
}

impl crate::models::ProjectGroup for Organization {
    const MEMBER_COLUMN: &'static str = "organization_id";
    const NOT_A_MEMBER: &'static str = "doesn't belong to an organization";
    const DETACHED: &'static str = "without an organization";

    fn group_of(project: &crate::models::Project) -> Option<i64> {
        project.organization_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Named;

    fn checked(url: &str) -> bool {
        let mut org = Organization {
            name: "acme".into(),
            platform_url: url.into(),
            ..Default::default()
        };
        org.validate().is_ok()
    }

    #[test]
    fn a_platform_url_is_https_or_this_machine() {
        assert!(checked(""));
        assert!(checked("https://time.example.com/api"));
        assert!(checked("http://127.0.0.1:8080"));
        assert!(checked("http://localhost/api"));
        assert!(!checked("http://time.example.com"));
        assert!(!checked("http://localhost.evil.com"));
        assert!(!checked("ftp://x"));
    }
}
