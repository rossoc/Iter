use crate::error::Result;
use crate::process::{Runner, SYSTEM};
use chrono::{DateTime, Local, NaiveDateTime};
use std::path::{Path, PathBuf};

/// The terminal multiplexer, behind a trait so tests need no server.
pub trait Multiplexer {
    fn create_session(&self, name: &str, cwd: &str) -> Result<()>;

    /// Not an error if already gone.
    fn kill_session(&self, name: &str);

    /// Attaches (blocking), or switches the client when already inside tmux.
    fn attach_session(&self, name: &str) -> Result<()>;

    fn session_exists(&self, name: &str) -> bool;

    fn list_sessions(&self) -> Vec<String>;

    fn kill_server(&self) -> Result<()>;

    /// Whether another client is still attached. The hooks fire after the leaving
    /// client is off the list (tmux 3.6), so this counts exactly the remaining ones.
    /// Any failure is `false` (no tmux, or session already gone).
    fn any_client_attached(&self, session: &str) -> bool;

    /// The session of the pane this process runs in, if inside tmux.
    fn current_session_name(&self) -> Option<String>;

    fn remember_client_session(&self, client: &str, session: &str) -> Result<()>;

    /// Consumed on read, so a reused tty starts clean. `None` if attached before the hooks.
    fn take_client_session(&self, client: &str) -> Option<String>;

    /// Installs the hooks that lack a working one. A `tmux.conf` hook is left alone: ours
    /// bakes in `current_exe`, which may be a deleted debug build.
    fn ensure_hooks_installed(&self, iter_bin: &str) -> Result<()>;

    /// Takes and unsets the detach message, so it is consumed once. Any failure is `None`:
    /// most sessions have no such option.
    fn take_detach_message(&self, session: &str) -> Option<String>;

    /// Server start time, local; `None` without a server. Separates clocks this server
    /// opened from ones a rebooted server left open.
    fn server_start_time(&self) -> Option<NaiveDateTime>;

    /// Mtime of the save tmux-resurrect's `last` points at: the last moment the previous
    /// server is known to have run. `None` if never saved. Dir: `@resurrect-dir`, else
    /// `~/.tmux/resurrect`, else `$XDG_DATA_HOME/tmux/resurrect`.
    fn last_resurrect_save(&self) -> Option<NaiveDateTime>;

    /// Session plus hooks: the part of `session new` that can fail, so its unwind guards this call.
    fn start_tracked_session(&self, name: &str, cwd: &str) -> Result<()> {
        self.create_session(name, cwd)?;
        self.install_hooks_for_current_exe()
    }

    /// [`Multiplexer::ensure_hooks_installed`] for the running binary.
    fn install_hooks_for_current_exe(&self) -> Result<()> {
        let iter_bin = std::env::current_exe()?.to_string_lossy().to_string();
        self.ensure_hooks_installed(&iter_bin)
    }
}

/// A target naming exactly this session: a bare name matches by prefix and treats `*`/`?` as globs.
fn exact(name: &str) -> String {
    format!("={name}")
}

/// Whether this process runs inside a tmux client (which must switch, not attach).
fn inside_tmux() -> bool {
    std::env::var_os("TMUX").is_some()
}

/// The `tmux` binary, driven through a [`Runner`].
pub struct TmuxClient<'a> {
    runner: &'a dyn Runner,
    inside: bool,
}

impl TmuxClient<'static> {
    pub fn system() -> Self {
        TmuxClient {
            runner: &SYSTEM,
            inside: inside_tmux(),
        }
    }
}

impl<'a> TmuxClient<'a> {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(runner: &'a dyn Runner, inside: bool) -> Self {
        TmuxClient { runner, inside }
    }

    fn run(&self, args: &[&str]) -> Result<()> {
        self.runner.run("tmux", None, args)
    }

    fn output(&self, args: &[&str]) -> Result<String> {
        self.runner.output("tmux", None, args)
    }

    fn output_trimmed(&self, args: &[&str]) -> Option<String> {
        self.runner.output_trimmed("tmux", None, args)
    }

    /// Whether `event`'s installed hook already passes the format variables `args` calls
    /// for. Comparing variables, not the whole action, keeps a `tmux.conf` hook current and
    /// replaces one from an old `iter` that passed `#{hook_session_name}` everywhere.
    fn hook_is_current(&self, event: &str, args: &str) -> bool {
        let Ok(existing) = self.output(&["show-options", "-gv", event]) else {
            return false; // no tmux, or no such option: install ours
        };
        !existing.trim().is_empty() && format_variables(&existing) == format_variables(args)
    }
}

impl Multiplexer for TmuxClient<'_> {
    fn create_session(&self, name: &str, cwd: &str) -> Result<()> {
        self.run(&["new-session", "-d", "-s", name, "-c", cwd])
    }

    fn kill_session(&self, name: &str) {
        let _ = self.run(&["kill-session", "-t", &exact(name)]);
    }

    fn attach_session(&self, name: &str) -> Result<()> {
        let verb = if self.inside {
            "switch-client"
        } else {
            "attach-session"
        };
        self.run(&[verb, "-t", &exact(name)])
    }

    fn session_exists(&self, name: &str) -> bool {
        self.runner
            .succeeds("tmux", None, &["has-session", "-t", &exact(name)])
    }

    fn list_sessions(&self) -> Vec<String> {
        self.output(&["list-sessions", "-F", "#{session_name}"])
            .map(|names| names.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }

    fn kill_server(&self) -> Result<()> {
        self.run(&["kill-server"])
    }

    fn any_client_attached(&self, session: &str) -> bool {
        let Ok(clients) = self.output(&["list-clients", "-t", session, "-F", "#{client_name}"])
        else {
            return false;
        };
        !clients.trim().is_empty()
    }

    fn current_session_name(&self) -> Option<String> {
        // Outside tmux the answer is known; asking costs a fork+exec per command.
        if !self.inside {
            return None;
        }
        self.output_trimmed(&["display-message", "-p", "#S"])
    }

    fn remember_client_session(&self, client: &str, session: &str) -> Result<()> {
        self.run(&["set-option", "-g", &client_session_option(client), session])
    }

    fn take_client_session(&self, client: &str) -> Option<String> {
        let option = client_session_option(client);
        let session = self.output_trimmed(&["show-options", "-gv", &option])?;
        let _ = self.run(&["set-option", "-gu", &option]);
        Some(session)
    }

    fn ensure_hooks_installed(&self, iter_bin: &str) -> Result<()> {
        if iter_bin.contains('\'') {
            return Err(crate::error::IterError::CommandFailed(format!(
                "cannot install the tmux hooks for a binary path containing a quote: {iter_bin}"
            )));
        }
        for (event, args) in HOOKS {
            if self.hook_is_current(event, args) {
                continue;
            }
            let action = hook_action(iter_bin, event, args);
            self.run(&["set-hook", "-g", event, &action])?;
        }
        Ok(())
    }

    fn take_detach_message(&self, session: &str) -> Option<String> {
        let value =
            self.output_trimmed(&["show-options", "-t", session, "-v", DETACH_MESSAGE_OPTION])?;
        let _ = self.run(&["set-option", "-t", session, "-u", DETACH_MESSAGE_OPTION]);
        Some(value)
    }

    fn server_start_time(&self) -> Option<NaiveDateTime> {
        let epoch: i64 = self
            .output_trimmed(&["display-message", "-p", "#{start_time}"])?
            .parse()
            .ok()?;
        let utc = DateTime::from_timestamp(epoch, 0)?;
        Some(utc.with_timezone(&Local).naive_local())
    }

    fn last_resurrect_save(&self) -> Option<NaiveDateTime> {
        let home = std::env::var("HOME").ok()?;
        let dir = match self.output_trimmed(&["show-options", "-gqv", "@resurrect-dir"]) {
            Some(dir) if !dir.is_empty() => {
                crate::config::expand_home(&dir.replace("$HOME", &home))
            }
            _ => {
                let legacy = Path::new(&home).join(".tmux/resurrect");
                match legacy.is_dir() {
                    true => legacy,
                    false => std::env::var("XDG_DATA_HOME")
                        .map(PathBuf::from)
                        .unwrap_or_else(|_| Path::new(&home).join(".local/share"))
                        .join("tmux/resurrect"),
                }
            }
        };
        // `metadata` follows the symlink, so this is the save itself.
        let modified = std::fs::metadata(dir.join("last")).ok()?.modified().ok()?;
        Some(DateTime::<Local>::from(modified).naive_local())
    }
}

/// Free-function forms over the real `tmux`, for plumbing with no client to hand.
pub fn kill_session(name: &str) {
    TmuxClient::system().kill_session(name)
}

pub fn current_session_name() -> Option<String> {
    TmuxClient::system().current_session_name()
}

pub fn take_detach_message(session: &str) -> Option<String> {
    TmuxClient::system().take_detach_message(session)
}

pub fn any_client_attached(session: &str) -> bool {
    TmuxClient::system().any_client_attached(session)
}

/// Server-wide option prefix recording which session a client is on, e.g.
/// `@iter_client_session_/dev/pts/3`. `client-detached` cannot name the session the
/// client left, so `client-session-changed` records it under `#{hook_client}`.
const CLIENT_SESSION_OPTION_PREFIX: &str = "@iter_client_session_";

fn client_session_option(client: &str) -> String {
    format!("{CLIENT_SESSION_OPTION_PREFIX}{client}")
}

/// Session option a `detach-client` bind sets before detaching; the `client-detached` hook
/// moves it onto the session it closes. Shell-visible: `tmux.conf` binds write it.
pub const DETACH_MESSAGE_OPTION: &str = "@iter_detach_message";

/// The hooks `iter` drives, with the format variables each is passed. Wrong variables fail
/// silently, so the per-event table is in `doc/architecture_core.md`.
const HOOKS: [(&str, &str); 3] = [
    (
        "client-session-changed",
        "#{q:hook_client} #{q:session_name} #{q:client_last_session}",
    ),
    ("client-detached", "#{q:hook_client}"),
    // Backstop for a session torn down another way while still attached.
    ("session-closed", "#{q:hook_session_name}"),
];

/// The variables named by the `#{...}` in a hook body, in order, without braces or `q:`
/// (we write `#{q:x}`, a `tmux.conf` may not).
fn format_variables(action: &str) -> Vec<&str> {
    action
        .match_indices("#{")
        .filter_map(|(start, _)| {
            let rest = &action[start + 2..];
            let name = &rest[..rest.find('}')?];
            Some(name.strip_prefix("q:").unwrap_or(name))
        })
        .collect()
}

/// The `set-hook` body for one entry of [`HOOKS`]; split out so variables are testable.
fn hook_action(iter_bin: &str, event: &str, args: &str) -> String {
    // Inside the action's single quotes a `'` cannot be written; the rest is shell-quoted.
    let bin = crate::agent::prompt::shell_word(iter_bin);
    format!("run-shell '{bin} internal hook {event} {args}'")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action_for(event: &str) -> String {
        let (event, args) = HOOKS
            .iter()
            .find(|(name, _)| *name == event)
            .unwrap_or_else(|| panic!("{event} is not an installed hook"));
        hook_action("/usr/bin/iter", event, args)
    }

    /// `#{hook_client}` is the only name `client-detached` has; `#{client_name}` can differ on a switch.
    #[test]
    fn the_client_hooks_are_told_the_client_by_hook_client() {
        for event in ["client-session-changed", "client-detached"] {
            let action = action_for(event);
            assert!(action.contains("#{q:hook_client}"), "{event}: {action}");
            assert!(!action.contains("client_name"), "{event}: {action}");
        }
    }

    /// Guards `client-detached` being told `#{session_name}` (the most recently active session).
    #[test]
    fn client_detached_is_not_told_a_session() {
        assert_eq!(
            format_variables(&action_for("client-detached")),
            vec!["hook_client"]
        );
    }

    /// On `session-closed`, `#{session_name}` names the session the client moved to.
    #[test]
    fn session_closed_is_told_the_session_by_hook_session_name() {
        let action = action_for("session-closed");
        assert!(action.contains("#{q:hook_session_name}"), "{action}");
        assert!(
            !action.contains("session_name}") || action.contains("hook_session_name}"),
            "{action}"
        );
    }

    /// `client-session-changed` records the new session and closes the one left, so needs both.
    #[test]
    fn client_session_changed_is_told_the_session_and_the_one_left() {
        assert_eq!(
            format_variables(&action_for("client-session-changed")),
            vec!["hook_client", "session_name", "client_last_session"]
        );
    }

    /// A `tmux.conf` hook with a stable path and its own quoting still counts as current.
    #[test]
    fn a_conf_written_hook_counts_as_current() {
        let (_, args) = HOOKS
            .iter()
            .find(|(name, _)| *name == "client-session-changed")
            .expect("the hook is installed");
        let from_conf = "run-shell \"~/bin/iter internal hook client-session-changed \
                         '#{hook_client}' '#{session_name}' '#{client_last_session}'\"";
        assert_eq!(format_variables(from_conf), format_variables(args));
    }

    /// An old `iter` hook does not, and is replaced.
    #[test]
    fn a_stale_hook_from_an_older_iter_is_not_current() {
        let (_, args) = HOOKS
            .iter()
            .find(|(name, _)| *name == "client-detached")
            .expect("the hook is installed");
        let stale = "run-shell '/old/iter internal hook client-detached \"#{session_name}\"'";
        assert_ne!(format_variables(stale), format_variables(args));
    }

    #[test]
    fn format_variables_reads_them_in_order_without_quoting() {
        assert_eq!(
            format_variables("a '#{session_name}' b \"#{client_last_session}\" c"),
            vec!["session_name", "client_last_session"]
        );
        assert_eq!(format_variables("run-shell 'echo hi'"), Vec::<&str>::new());
    }

    #[test]
    fn a_binary_path_with_a_space_is_escaped() {
        let action = hook_action("/my dir/iter", "session-closed", "#{q:hook_session_name}");
        assert!(
            action.starts_with("run-shell '/my\\ dir/iter internal hook"),
            "{action}"
        );
    }

    #[test]
    fn a_hook_action_invokes_the_internal_subcommand() {
        assert_eq!(
            action_for("client-detached"),
            "run-shell '/usr/bin/iter internal hook client-detached #{q:hook_client}'"
        );
    }

    use crate::process::fake::FakeRunner;

    fn client(runner: &FakeRunner, inside: bool) -> TmuxClient<'_> {
        TmuxClient::new(runner, inside)
    }

    /// Every session-targeting command goes through `=name`.
    #[test]
    fn sessions_are_targeted_exactly() {
        let runner = FakeRunner::default();
        let tmux = client(&runner, false);
        tmux.kill_session("web/a");
        assert!(tmux.session_exists("web/a*"));
        tmux.attach_session("web/a").unwrap();
        assert_eq!(
            runner.calls(),
            [
                "tmux kill-session -t =web/a",
                "tmux has-session -t =web/a*",
                "tmux attach-session -t =web/a",
            ]
        );
    }

    #[test]
    fn inside_tmux_attaching_switches_the_client() {
        let runner = FakeRunner::default();
        client(&runner, true).attach_session("web/a").unwrap();
        assert_eq!(runner.calls(), ["tmux switch-client -t =web/a"]);
    }

    #[test]
    fn a_missing_session_is_reported_as_absent() {
        let runner = FakeRunner::default();
        runner.fail("tmux has-session");
        assert!(!client(&runner, false).session_exists("nope"));
    }

    #[test]
    fn killing_a_gone_session_is_not_an_error() {
        let runner = FakeRunner::default();
        runner.fail("tmux kill-session");
        client(&runner, false).kill_session("gone");
    }

    #[test]
    fn outside_tmux_the_current_session_is_not_asked_for() {
        let runner = FakeRunner::default();
        assert_eq!(client(&runner, false).current_session_name(), None);
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn sessions_are_listed_one_per_line() {
        let runner = FakeRunner::default();
        runner.reply("tmux list-sessions", "a/b\nc/d\n");
        assert_eq!(client(&runner, false).list_sessions(), ["a/b", "c/d"]);
    }

    #[test]
    fn missing_hooks_are_installed_with_quoted_format_variables() {
        let runner = FakeRunner::default();
        client(&runner, false)
            .ensure_hooks_installed("/usr/bin/iter")
            .unwrap();
        let installs: Vec<String> = runner
            .calls()
            .into_iter()
            .filter(|c| c.starts_with("tmux set-hook"))
            .collect();
        assert_eq!(
            installs,
            [
                "tmux set-hook -g client-session-changed run-shell '/usr/bin/iter internal hook client-session-changed #{q:hook_client} #{q:session_name} #{q:client_last_session}'",
                "tmux set-hook -g client-detached run-shell '/usr/bin/iter internal hook client-detached #{q:hook_client}'",
                "tmux set-hook -g session-closed run-shell '/usr/bin/iter internal hook session-closed #{q:hook_session_name}'",
            ]
        );
    }

    #[test]
    fn a_current_hook_is_left_alone_and_a_stale_one_replaced() {
        let runner = FakeRunner::default();
        runner.reply(
            "tmux show-options -gv client-detached",
            "run-shell \"~/bin/iter internal hook client-detached '#{hook_client}'\"",
        );
        runner.reply(
            "tmux show-options -gv session-closed",
            "run-shell '/old/iter internal hook session-closed #{session_name}'",
        );
        client(&runner, false)
            .ensure_hooks_installed("/usr/bin/iter")
            .unwrap();
        let installed: Vec<String> = runner
            .calls()
            .into_iter()
            .filter(|c| c.starts_with("tmux set-hook"))
            .map(|c| c.split_whitespace().nth(3).unwrap().to_string())
            .collect();
        assert_eq!(installed, ["client-session-changed", "session-closed"]);
    }

    #[test]
    fn a_binary_path_with_a_quote_is_refused_before_touching_tmux() {
        let runner = FakeRunner::default();
        assert!(
            client(&runner, false)
                .ensure_hooks_installed("/o'brien/iter")
                .is_err()
        );
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn a_client_session_is_remembered_and_consumed() {
        let runner = FakeRunner::default();
        runner.reply(
            "tmux show-options -gv @iter_client_session_/dev/ttys1",
            "web/a\n",
        );
        let tmux = client(&runner, false);
        tmux.remember_client_session("/dev/ttys1", "web/a").unwrap();
        assert_eq!(
            tmux.take_client_session("/dev/ttys1").as_deref(),
            Some("web/a")
        );
        assert_eq!(
            runner.calls(),
            [
                "tmux set-option -g @iter_client_session_/dev/ttys1 web/a",
                "tmux show-options -gv @iter_client_session_/dev/ttys1",
                "tmux set-option -gu @iter_client_session_/dev/ttys1",
            ]
        );
    }

    /// Needs a real tmux, so `#[ignore]`d; refuses anything but a throwaway server. Run:
    /// `TMUX_TMPDIR=$(mktemp -d /tmp/itx.XXXX) cargo test real_tmux -- --ignored`
    #[test]
    #[ignore = "needs tmux, and TMUX_TMPDIR on a throwaway directory"]
    fn real_tmux_targets_exactly_and_quotes_what_the_hooks_are_told() {
        let tmp = std::env::var("TMUX_TMPDIR").expect("set TMUX_TMPDIR to a throwaway directory");
        assert!(
            tmp.starts_with("/tmp/itx"),
            "not a throwaway directory: {tmp}"
        );
        assert!(
            std::env::var("TMUX").is_err(),
            "run outside tmux (unset TMUX)"
        );
        let tmux = TmuxClient::system();
        let dir = std::path::PathBuf::from(&tmp);

        // a name is a name, not a prefix: killing `web-a` leaves `web-ab`
        tmux.create_session("web-a", "/tmp").unwrap();
        tmux.create_session("web-ab", "/tmp").unwrap();
        tmux.kill_session("web-a");
        assert!(!tmux.session_exists("web-a"));
        assert!(tmux.session_exists("web-ab"));
        tmux.kill_session("web-a"); // already gone: not an error, and `web-ab` stays
        assert!(tmux.session_exists("web-ab"));
        assert_eq!(tmux.list_sessions(), ["web-ab"]);

        // the hooks get a hostile session name as one argument and run nothing
        let stub = dir.join("iter stub"); // a space in the path, on purpose
        let out = dir.join("args.out");
        std::fs::write(
            &stub,
            format!("#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}'\n", out.display()),
        )
        .unwrap();
        std::fs::set_permissions(&stub, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
        tmux.ensure_hooks_installed(stub.to_str().unwrap()).unwrap();
        // no `.` or `:` in it: those mean pane and window in a tmux target
        let marker = std::path::Path::new("/tmp/itx_PWNED_marker");
        let _ = std::fs::remove_file(marker);
        let hostile = format!("ev\"il$(touch {})", marker.display());
        tmux.create_session(&hostile, "/tmp").unwrap();
        tmux.kill_session(&hostile);
        for _ in 0..40 {
            if out.exists()
                && std::fs::read_to_string(&out).is_ok_and(|t| t.contains("session-closed"))
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let seen = std::fs::read_to_string(&out).expect("the hook ran the stub");
        assert!(seen.contains(&hostile), "the name arrived whole: {seen}");
        assert!(!marker.exists(), "the name was run as shell");
        tmux.kill_server().unwrap();
    }
}
