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

/// A platform URL is `https`, or `http` to this machine, and carries no userinfo (the API key goes
/// out with every call). Moves here from `platform` in isolation step 2.
// stub: I3 (the real `platform::check_url` still does the work)
#[allow(dead_code)]
pub fn check_url(url: &str) -> crate::error::Result<()> {
    let _ = url;
    unimplemented!("stub: I3")
}

impl Organization {
    /// Beyond a name: a platform URL passes [`crate::platform::check_url`].
    fn check(&mut self) -> crate::error::Result<()> {
        self.platform_url = self.platform_url.trim().to_string();
        match self.platform_url.as_str() {
            "" => Ok(()),
            url => crate::platform::check_url(url),
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
        assert!(!checked("http://localhost:1@evil.com"));
        assert!(!checked("http://localhost@evil.com/"));
        assert!(!checked("http://127.0.0.1.evil.com"));
        assert!(!checked("https://user:pw@time.example.com"));
        assert!(!checked("localhost"));
    }

    /// Round I3 (step 2): the URL rule is `models::check_url`, with the cases `platform` pins now.
    /// Each accepted URL and each refusal (with the part of the message that says why).
    #[test]
    fn check_url_lives_in_models_and_refuses_what_would_leak_the_key() {
        use crate::models::check_url;
        for ok in [
            "https://time.example.com/api",
            "http://127.0.0.1:8080",
            "http://localhost/api",
            "http://[::1]:8080",
        ] {
            assert!(check_url(ok).is_ok(), "{ok}");
        }
        for (bad, why) in [
            ("http://time.example.com", "must start with https://"),
            ("http://localhost.evil.com", "must start with https://"),
            ("ftp://x", "must start with https://"),
            ("localhost", "must start with https://"),
            ("http://localhost:1@evil.com", "user name or password"),
            ("https://user:pw@time.example.com", "user name or password"),
            ("not a url", "not a valid URL"),
        ] {
            let message = check_url(bad).unwrap_err().to_string();
            assert!(
                message.contains(why) && message.contains(bad),
                "{bad}: {message}"
            );
        }
    }
}
