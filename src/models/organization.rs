use crate::models::{Configured, Settings, default_branch_template, default_true, main_branch};
use iter_macros::{MarkdownBody, Table};
use serde::{Deserialize, Serialize};

/// A grouping of projects, and the defaults new projects in it start from.
///
/// Deliberately has no `base_path`: an organization isn't a place on disk,
/// it's the settings and the roster its projects share. Everything a
/// project needs that *is* about disk -- where it lives, whether that
/// directory is a repo -- stays on the project, which is why `iter
/// init`/`new`/`clone` still create projects rather than organizations.
///
/// `id` is `None` for a not-yet-created organization (the blank template
/// opened in the YAML editor); it's filled in once inserted.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Table, MarkdownBody)]
#[table(name = "organizations", order_by = "name", kind = "organization", check = "check")]
pub struct Organization {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<i64>,

    pub name: String,

    /// Free-form markdown notes. Edited as the markdown body below the
    /// YAML front matter rather than as a field among the others -- see
    /// `MarkdownBody`.
    #[serde(skip)]
    pub description: String,

    /// Default `github` for projects created in this organization.
    #[serde(default)]
    pub github: bool,

    /// Default `tmux` for projects created in this organization.
    #[serde(default = "default_true")]
    pub tmux: bool,

    /// Default `auto_branch` for projects created in this organization.
    #[serde(default = "default_true")]
    pub auto_branch: bool,

    /// Default `branch_template` for projects created in this organization.
    #[serde(default = "default_branch_template")]
    pub branch_template: String,

    /// Default `default_branch` for projects created in this organization.
    #[serde(default = "main_branch")]
    pub default_branch: String,

    /// Default `github_project` for projects created in this organization.
    #[serde(default)]
    pub github_project: String,

    /// The folder of this organization's daily notes (`~` expanded when
    /// read), and the folder its projects without one of their own keep
    /// theirs in. Empty means no notes.
    #[serde(default)]
    pub notes_path: String,

    /// The endpoint of the time platform hours are pushed to. Empty means
    /// none. The API key is not here: see `crate::secret`.
    #[serde(default)]
    pub platform_url: String,

    /// The platform's organization that entries are logged under. Falls back
    /// to this organization's name when empty. Editable on each share.
    #[serde(default)]
    pub platform_project: String,

    /// The platform's activity that entries are logged under. Editable on
    /// each share.
    #[serde(default)]
    pub platform_activity: String,
}

impl Organization {
    /// Beyond a name: the platform URL, when there is one, must be `https`
    /// (every call carries the API key as a bearer token), or `http` to this
    /// machine, which is how a local platform or a test server is reached.
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

    /// A blank template for `iter organization new` to open in the
    /// editor. Its fields *are* project defaults, so it starts from the
    /// same config block a project without an organization starts from.
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
        let mut org = Organization::default();
        org.name = "acme".into();
        org.platform_url = url.into();
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
