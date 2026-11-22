//! [`Organization`]: a group of projects with the defaults they start from and a time platform.

use crate::error::{IterError, Result};
use crate::models::{
    Configured, Project, ProjectGroup, Settings, default_branch_template, default_true, main_branch,
};
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

/// A platform URL is `https`, or `http` to this machine (local platform, test server), and has
/// no userinfo: the API key goes out as a bearer token with every call, and a prefix check
/// would accept `http://localhost:1@evil.com` or `http://localhost.evil.com`.
pub fn check_url(url: &str) -> Result<()> {
    let bad = |why: &str| {
        IterError::Platform(format!(
            "platform_url '{url}' {why} (the API key is sent with every request)"
        ))
    };
    let uri: ureq::http::Uri = url.parse().map_err(|_| bad("is not a valid URL"))?;
    let authority = uri
        .authority()
        .ok_or_else(|| bad("must start with https://"))?;
    if authority.as_str().contains('@') {
        return Err(bad("must not carry a user name or password"));
    }
    match uri.scheme_str() {
        Some("https") => Ok(()),
        Some("http") if matches!(uri.host(), Some("localhost" | "127.0.0.1" | "[::1]")) => Ok(()),
        _ => Err(bad("must start with https://")),
    }
}

impl Organization {
    /// Beyond a name: a platform URL passes [`check_url`].
    fn check(&mut self) -> Result<()> {
        self.platform_url = self.platform_url.trim().to_string();
        match self.platform_url.as_str() {
            "" => Ok(()),
            url => check_url(url),
        }
    }

    /// A blank template for `iter organization new`, starting from the project-defaults block.
    pub fn template(defaults: &Settings) -> Self {
        let mut organization = Organization::default();
        organization.set_settings(defaults.clone());
        organization
    }
}

impl ProjectGroup for Organization {
    const MEMBER_COLUMN: &'static str = "organization_id";
    const NOT_A_MEMBER: &'static str = "doesn't belong to an organization";
    const DETACHED: &'static str = "without an organization";

    fn group_of(project: &Project) -> Option<i64> {
        project.organization_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Named;

    /// Every URL the platform field takes (through `validate`) or refuses.
    #[test]
    fn a_platform_url_is_https_or_this_machine() {
        let checked = |url: &str| {
            let mut org = Organization {
                name: "acme".into(),
                platform_url: url.into(),
                ..Default::default()
            };
            org.validate().is_ok()
        };
        for (url, accepted) in [
            ("", true),
            ("https://time.example.com/api", true),
            ("http://127.0.0.1:8080", true),
            ("http://localhost/api", true),
            ("http://time.example.com", false),
            ("http://localhost.evil.com", false),
            ("ftp://x", false),
            ("http://localhost:1@evil.com", false),
            ("http://localhost@evil.com/", false),
            ("http://127.0.0.1.evil.com", false),
            ("https://user:pw@time.example.com", false),
            ("localhost", false),
        ] {
            assert_eq!(checked(url), accepted, "{url:?}");
        }
    }

    /// `check_url` alone: each accepted URL and each refusal, with the part of the message that
    /// says why.
    #[test]
    fn check_url_refuses_what_would_leak_the_key() {
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
