//! The API key of an organization's time platform.
//!
//! It lives in the operating system's keychain (or in `ITER_PLATFORM_KEY`
//! for a single run), and nowhere else: not the database, not `iter.yaml`,
//! not a report. Everything that holds it prints as `***`, and any text that
//! may have echoed it goes through [`Secret::redact`] first.

use crate::error::{IterError, Result};
use std::io::Write;
use std::process::{Command, Stdio};

pub const ENV_KEY: &str = "ITER_PLATFORM_KEY";

/// An API key. Never `Display`ed or `Debug`ged in the clear.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: &str) -> Option<Self> {
        let value = value.trim();
        (!value.is_empty()).then(|| Secret(value.to_string()))
    }

    /// The key itself, for the one place that sends it.
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn redact(&self, text: &str) -> String {
        text.replace(&self.0, "***")
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

impl std::fmt::Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

pub trait KeyStore {
    fn get(&self, organization: &str) -> Result<Option<Secret>>;
    fn set(&self, organization: &str, key: &Secret) -> Result<()>;
    fn delete(&self, organization: &str) -> Result<()>;
}

/// Moves the key kept under `old` to `new`, when there is one: keys are
/// kept by the organization's name, so a rename that left it behind would
/// lose the key (and leave a secret nothing can reach or clear).
pub fn rename_key(store: &dyn KeyStore, old: &str, new: &str) -> Result<()> {
    if old == new {
        return Ok(());
    }
    if let Some(key) = store.get(old)? {
        store.set(new, &key)?;
        store.delete(old)?;
    }
    Ok(())
}

/// The key to use for `organization`: `ITER_PLATFORM_KEY` when set, else the
/// store's. A missing key is an error that says how to set one.
pub fn resolve(store: &dyn KeyStore, organization: &str, env: Option<&str>) -> Result<Secret> {
    if let Some(key) = env.and_then(Secret::new) {
        return Ok(key);
    }
    store
        .get(organization)?
        .ok_or_else(|| IterError::MissingKey(organization.to_string()))
}

pub fn resolve_env(store: &dyn KeyStore, organization: &str) -> Result<Secret> {
    resolve(store, organization, std::env::var(ENV_KEY).ok().as_deref())
}

#[cfg_attr(not(feature = "web"), allow(dead_code))]
pub fn is_set(store: &dyn KeyStore, organization: &str) -> bool {
    resolve_env(store, organization).is_ok()
}

/// The macOS keychain, driven through the `security` tool. The key is handed
/// over on its standard input (`security -i`), never as an argument, so it
/// does not show in `ps`.
pub struct Keychain;

const SERVICE: &str = "iter-time-platform";

impl Keychain {
    fn session(&self, command: &str) -> Result<bool> {
        let mut child = Command::new("security")
            .arg("-i")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|source| IterError::Spawn {
                tool: "security",
                source,
            })?;
        if let Some(mut stdin) = child.stdin.take() {
            writeln!(stdin, "{command}")?;
        }
        Ok(child.wait()?.success())
    }
}

/// A word safe to put between double quotes in a `security -i` line.
fn quotable(value: &str) -> Result<&str> {
    if value.chars().any(|c| matches!(c, '"' | '\\' | '\n' | '\r')) {
        return Err(IterError::CommandFailed(
            "the keychain cannot store a name or key containing quotes, backslashes or newlines"
                .into(),
        ));
    }
    Ok(value)
}

impl KeyStore for Keychain {
    fn get(&self, organization: &str) -> Result<Option<Secret>> {
        let out = Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                SERVICE,
                "-a",
                organization,
                "-w",
            ])
            .stderr(Stdio::null())
            .output()
            .map_err(|source| IterError::Spawn {
                tool: "security",
                source,
            })?;
        if !out.status.success() {
            return Ok(None);
        }
        Ok(Secret::new(&String::from_utf8_lossy(&out.stdout)))
    }

    fn set(&self, organization: &str, key: &Secret) -> Result<()> {
        let line = format!(
            "add-generic-password -U -s \"{SERVICE}\" -a \"{}\" -w \"{}\"",
            quotable(organization)?,
            quotable(key.expose())?
        );
        match self.session(&line)? {
            true => Ok(()),
            false => Err(IterError::CommandFailed(
                "the keychain refused to store the key".into(),
            )),
        }
    }

    fn delete(&self, organization: &str) -> Result<()> {
        // Deleting a key that is not there is fine: the end state is the same.
        self.session(&format!(
            "delete-generic-password -s \"{SERVICE}\" -a \"{}\"",
            quotable(organization)?
        ))?;
        Ok(())
    }
}

#[cfg(test)]
pub mod fake {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Default)]
    pub struct MemoryKeys(pub RefCell<HashMap<String, Secret>>);

    impl KeyStore for MemoryKeys {
        fn get(&self, organization: &str) -> Result<Option<Secret>> {
            Ok(self.0.borrow().get(organization).cloned())
        }
        fn set(&self, organization: &str, key: &Secret) -> Result<()> {
            self.0.borrow_mut().insert(organization.into(), key.clone());
            Ok(())
        }
        fn delete(&self, organization: &str) -> Result<()> {
            self.0.borrow_mut().remove(organization);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::MemoryKeys;
    use super::*;

    #[test]
    fn a_renamed_organization_keeps_its_key() {
        let store = MemoryKeys::default();
        store.set("acme", &Secret::new("k").unwrap()).unwrap();
        rename_key(&store, "acme", "acme-corp").unwrap();
        assert!(store.get("acme").unwrap().is_none());
        assert_eq!(store.get("acme-corp").unwrap().unwrap().expose(), "k");
        // nothing to move is fine, and so is the same name
        rename_key(&store, "none", "other").unwrap();
        rename_key(&store, "acme-corp", "acme-corp").unwrap();
        assert!(store.get("acme-corp").unwrap().is_some());
    }

    #[test]
    fn the_environment_overrides_the_store() {
        let store = MemoryKeys::default();
        store.set("acme", &Secret::new("stored").unwrap()).unwrap();
        let key = resolve(&store, "acme", Some("from-env")).unwrap();
        assert_eq!(key.expose(), "from-env");
        assert_eq!(resolve(&store, "acme", None).unwrap().expose(), "stored");
    }

    #[test]
    fn a_missing_key_says_how_to_set_one() {
        let err = resolve(&MemoryKeys::default(), "acme", Some("  ")).unwrap_err();
        assert!(matches!(err, IterError::MissingKey(_)));
        assert!(err.to_string().contains("iter org key acme"));
    }

    #[test]
    fn deleting_removes_the_key() {
        let store = MemoryKeys::default();
        store.set("acme", &Secret::new("k").unwrap()).unwrap();
        store.delete("acme").unwrap();
        assert!(!is_set_in(&store, "acme"));
    }

    fn is_set_in(store: &MemoryKeys, org: &str) -> bool {
        store.get(org).unwrap().is_some()
    }

    #[test]
    fn a_secret_never_prints_or_survives_redaction() {
        let key = Secret::new("s3cr3t-token").unwrap();
        assert_eq!(format!("{key:?}"), "Secret(***)");
        assert_eq!(format!("{key}"), "***");
        assert_eq!(key.redact("bad token s3cr3t-token!"), "bad token ***!");
    }

    #[test]
    fn the_keychain_refuses_names_it_cannot_quote() {
        assert!(quotable("a\"b").is_err());
        assert!(quotable("fine").is_ok());
    }
}
