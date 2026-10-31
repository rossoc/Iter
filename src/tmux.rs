use crate::error::Result;
use crate::process::{Runner, SYSTEM};
use chrono::{DateTime, Local, NaiveDateTime};
use std::path::{Path, PathBuf};

/// The terminal multiplexer `iter` keeps one session per task in, behind a
/// trait so what it asks of tmux can be tested without a server.
pub trait Multiplexer {
    /// Creates a new detached session named `name`, starting in `cwd`.
    fn create_session(&self, name: &str, cwd: &str) -> Result<()>;

    /// Kills a session if it's running. Not an error if it's already gone.
    fn kill_session(&self, name: &str);

    /// Puts the user in the session named `name`: attaching (this blocks,
    /// handing the terminal over until the user detaches) or, when we're
    /// already inside tmux, switching the current client over to it.
    fn attach_session(&self, name: &str) -> Result<()>;

    fn session_exists(&self, name: &str) -> bool;

    /// The names of every session on the server; empty when there is no
    /// server.
    fn list_sessions(&self) -> Vec<String>;

    /// Kills the server, and with it every session and client on it.
    fn kill_server(&self) -> Result<()>;

    /// Whether any client is still attached to `session`.
    ///
    /// One session can have several clients on it -- a second terminal, or
    /// someone pairing -- and each of them detaches separately. Without
    /// this, the first detach closes the task's session while another
    /// client is still working in it, and every minute after that goes
    /// unrecorded.
    ///
    /// This is only usable from a hook because of *when* the hook runs.
    /// Measured on tmux 3.6: by the time `client-detached` or
    /// `client-session-changed` fires, the client that left is already off
    /// the session's list, so what this counts is exactly the clients that
    /// remain.
    ///
    /// Any failure is a `false` -- no tmux, or a session that's already
    /// gone, which is exactly the `session-closed` case -- so the caller
    /// closes the session, which is what it did before this existed.
    fn any_client_attached(&self, session: &str) -> bool;

    /// The session name of the pane this process is running in, if any
    /// (i.e. we're inside a tmux client). Used by `iter t`.
    fn current_session_name(&self) -> Option<String>;

    /// Records that `client` now shows `session`, for
    /// [`Multiplexer::take_client_session`] to read back when it detaches.
    fn remember_client_session(&self, client: &str, session: &str) -> Result<()>;

    /// The session `client` was last recorded on, consumed so a later
    /// client that reuses the same tty starts from nothing. `None` for a
    /// client that attached before the hooks were installed.
    fn take_client_session(&self, client: &str) -> Option<String>;

    /// Installs the global hooks that drive `iter internal hook <event>
    /// <session> [previous]`, for any event that doesn't already have a
    /// working one. Safe to call every time a session is created.
    ///
    /// "Already working" is deliberately not "already ours": a hook wired up
    /// in `tmux.conf` is left exactly as it is. That's the difference
    /// between a path baked in here -- `current_exe`, i.e. whichever build
    /// last ran `session new`, which can be a debug binary in a worktree
    /// that later gets deleted -- and a stable one the user chose.
    fn ensure_hooks_installed(&self, iter_bin: &str) -> Result<()>;

    /// Takes the detach message off `session`, if something left one there:
    /// returns it and unsets the option, so it's consumed once rather than
    /// re-attached to every later session of the same task.
    ///
    /// Every failure is a `None`: the hook this serves fires for every
    /// session, most of which have no such option (and, on `session-closed`,
    /// no longer exist to be asked).
    fn take_detach_message(&self, session: &str) -> Option<String>;

    /// When the running server started, in local time; `None` when there is
    /// no server. What tells a clock this server opened from one a previous
    /// server left open -- a reboot takes the server down without
    /// `iter internal kill-server`, so nothing closed it.
    fn server_start_time(&self) -> Option<NaiveDateTime>;

    /// When tmux-resurrect last saved, in local time: the modification time
    /// of the save `last` points at. That's the last moment the previous
    /// server is known to have been running, and so the best end there is
    /// for a clock it left open. `None` when resurrect has never saved.
    ///
    /// The directory is found the way resurrect finds it: `@resurrect-dir`
    /// (with `~` and `$HOME` expanded), else `~/.tmux/resurrect` if it
    /// exists, else `$XDG_DATA_HOME/tmux/resurrect`.
    fn last_resurrect_save(&self) -> Option<NaiveDateTime>;

    /// Creates the detached session named `name` in `cwd` and makes sure
    /// the hooks that track it are installed -- the two steps `iter session
    /// new` needs done together, and the part of it that can fail, which is
    /// why its unwind path guards exactly this call.
    fn start_tracked_session(&self, name: &str, cwd: &str) -> Result<()> {
        self.create_session(name, cwd)?;
        self.install_hooks_for_current_exe()
    }

    /// [`Multiplexer::ensure_hooks_installed`] pointing at the binary that's
    /// running now.
    fn install_hooks_for_current_exe(&self) -> Result<()> {
        let iter_bin = std::env::current_exe()?.to_string_lossy().to_string();
        self.ensure_hooks_installed(&iter_bin)
    }
}

/// A tmux target that names exactly this session: a bare name would match by
/// prefix (`web/a` also hits `web/ab`) and treat `*`/`?` as globs.
fn exact(name: &str) -> String {
    format!("={name}")
}

/// Whether this process is itself running inside a tmux client -- which
/// decides how `attach_session` gets the user there: a client can't attach
/// to a second session from within one, it switches to it instead.
fn inside_tmux() -> bool {
    std::env::var_os("TMUX").is_some()
}

/// The `tmux` binary, driven through a [`Runner`].
pub struct TmuxClient<'a> {
    runner: &'a dyn Runner,
    /// Whether we are inside a tmux client; see [`inside_tmux`].
    inside: bool,
}

impl TmuxClient<'static> {
    /// The real `tmux`, inside a client iff `$TMUX` says so.
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

    /// Whether `event`'s installed hook already passes exactly the format
    /// variables `args` calls for.
    ///
    /// That comparison, rather than a string match on the whole action, is
    /// what lets a `tmux.conf` hook -- different path, different quoting,
    /// same variables -- count as current, while a hook from an older `iter`
    /// (which passed `#{hook_session_name}` to every event, and so did
    /// nothing) does not and gets replaced.
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
        // Outside tmux the answer is already known, and asking costs a
        // fork+exec on every command that defaults to "the task I'm in".
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

/// Free-function forms over the real `tmux`, for the plumbing (clocks,
/// workspace teardown) that has no client to hand.
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

/// The prefix of the server-wide tmux option that records which session a
/// client is on, e.g. `@iter_client_session_/dev/pts/3`.
///
/// It exists because `client-detached` can't name the session the client
/// left: measured on tmux 3.6, `#{session_name}` there is whichever session
/// is most recently active -- often one another client is still working in
/// -- and the client itself is already gone, so it can't be asked.
/// `#{hook_client}` does still name it, so the session is recorded under
/// that name on every `client-session-changed`, which *is* told the
/// session, and looked up again on the way out.
const CLIENT_SESSION_OPTION_PREFIX: &str = "@iter_client_session_";

fn client_session_option(client: &str) -> String {
    format!("{CLIENT_SESSION_OPTION_PREFIX}{client}")
}

/// The session-scoped tmux option a detach message is left in: something
/// bound to `detach-client` sets it just before detaching, and the
/// `client-detached` hook takes it back off again and stores it on the
/// session it closes. Named as it is because it's a shell-visible
/// interface, not an internal one -- a `bind` in `tmux.conf` writes it.
pub const DETACH_MESSAGE_OPTION: &str = "@iter_detach_message";

/// The hooks `iter` drives, each with the format variables it's passed.
///
/// Which variable is right differs per event, and getting it wrong fails
/// *silently* -- the hook still runs, it just names the wrong session or
/// none at all. Measured on tmux 3.6:
///
/// | event                    | `hook_client` | `session_name`    | `hook_session_name` |
/// |--------------------------|---------------|-------------------|---------------------|
/// | `client-session-changed` | the client    | the new session   | empty               |
/// | `client-detached`        | the client    | *some other one*  | empty               |
/// | `session-closed`         | --            | *some other one*  | the session         |
///
/// `#{client_name}` is no substitute for `#{hook_client}`: on a
/// `switch-client` it can name a different client entirely. And since
/// `client-detached` can't name its session, it's only passed the client,
/// and the session comes from what `client-session-changed` recorded (see
/// [`remember_client_session`]).
///
/// `client-session-changed` rather than `client-attached` is what starts a
/// session: it fires on a plain attach *and* on `switch-client`, so hopping
/// between two tasks inside tmux is seen.
const HOOKS: [(&str, &str); 3] = [
    (
        "client-session-changed",
        "#{q:hook_client} #{q:session_name} #{q:client_last_session}",
    ),
    ("client-detached", "#{q:hook_client}"),
    // The backstop for a session torn down another way (`tmux kill-session`,
    // the shell in it exiting) while still attached.
    ("session-closed", "#{q:hook_session_name}"),
];

/// The variables named by the `#{...}` in a hook body, in order, without the
/// braces or a `q:` quoting modifier: that quoting differs between what we
/// write (`#{q:x}`, so a name with a quote or `$(...)` stays one word for the
/// shell) and what a `tmux.conf` does.
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

/// The `set-hook` body for one entry of [`HOOKS`]. Split out so the format
/// variables can be asserted without a tmux server, since naming the wrong
/// one is invisible at runtime -- the hook fires and quietly matches no
/// session.
fn hook_action(iter_bin: &str, event: &str, args: &str) -> String {
    // The action is a tmux command, and inside its single quotes a `'` cannot
    // be written at all; everything else is quoted for the shell it runs in.
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

    /// Both client hooks are told the client by `#{hook_client}`: it's the
    /// only name `client-detached` has to find its session by, and
    /// `#{client_name}` can name a different client on a switch.
    #[test]
    fn the_client_hooks_are_told_the_client_by_hook_client() {
        for event in ["client-session-changed", "client-detached"] {
            let action = action_for(event);
            assert!(action.contains("#{q:hook_client}"), "{event}: {action}");
            assert!(!action.contains("client_name"), "{event}: {action}");
        }
    }

    /// The bug this guards: `client-detached` was told `#{session_name}`,
    /// which names the most recently active session -- so one client
    /// detaching stopped the clock of a session another client was still in.
    #[test]
    fn client_detached_is_not_told_a_session() {
        assert_eq!(
            format_variables(&action_for("client-detached")),
            vec!["hook_client"]
        );
    }

    /// On `session-closed` it's `#{session_name}` that's wrong, naming
    /// whatever session the client moved to rather than the one that closed.
    #[test]
    fn session_closed_is_told_the_session_by_hook_session_name() {
        let action = action_for("session-closed");
        assert!(action.contains("#{q:hook_session_name}"), "{action}");
        assert!(!action.contains("session_name}") || action.contains("hook_session_name}"), "{action}");
    }

    /// `client-session-changed` records the session and closes the one being
    /// left, so it needs both -- without the second a switch leaks an open
    /// session.
    #[test]
    fn client_session_changed_is_told_the_session_and_the_one_left() {
        assert_eq!(
            format_variables(&action_for("client-session-changed")),
            vec!["hook_client", "session_name", "client_last_session"]
        );
    }

    /// A `tmux.conf` hook naming a stable path and quoting the variables
    /// its own way is still current -- that's what keeps `session new` from
    /// re-pointing it at whichever build happened to run.
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

    /// ...whereas the hook an older `iter` left behind does not, so it gets
    /// replaced rather than kept forever.
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

    /// A path with a space stays one word for the shell the hook runs in.
    #[test]
    fn a_binary_path_with_a_space_is_escaped() {
        let action = hook_action("/my dir/iter", "session-closed", "#{q:hook_session_name}");
        assert!(action.starts_with("run-shell '/my\\ dir/iter internal hook"), "{action}");
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

    /// A bare name would match by prefix and treat `*` as a glob; every
    /// session-targeting command goes through `=name`.
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

    /// The hooks are installed with every variable `#{q:...}`-quoted, so a
    /// session name with a quote stays one shell word.
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

    /// A hook already wired in `tmux.conf` with the right variables is left
    /// alone; a stale one is replaced.
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
        assert!(client(&runner, false).ensure_hooks_installed("/o'brien/iter").is_err());
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn a_client_session_is_remembered_and_consumed() {
        let runner = FakeRunner::default();
        runner.reply("tmux show-options -gv @iter_client_session_/dev/ttys1", "web/a\n");
        let tmux = client(&runner, false);
        tmux.remember_client_session("/dev/ttys1", "web/a").unwrap();
        assert_eq!(tmux.take_client_session("/dev/ttys1").as_deref(), Some("web/a"));
        assert_eq!(
            runner.calls(),
            [
                "tmux set-option -g @iter_client_session_/dev/ttys1 web/a",
                "tmux show-options -gv @iter_client_session_/dev/ttys1",
                "tmux set-option -gu @iter_client_session_/dev/ttys1",
            ]
        );
    }
}
