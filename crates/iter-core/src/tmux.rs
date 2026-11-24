//! The terminal multiplexer: the [`Multiplexer`] trait the commands use, its [`TmuxClient`] over a
//! [`Runner`] (so tests drive a fake, never a server), and the hooks iter installs to time sessions.

use crate::error::{IterError, Result};
use crate::process::{Runner, SYSTEM, host, shell_word};
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

    /// Opens a window named `name` in `session`, in `cwd`, running `command` (the default shell
    /// when none).
    fn new_window(&self, session: &str, name: &str, cwd: &str, command: Option<&str>)
    -> Result<()>;

    /// Puts `text` in the paste buffer `name`, whole.
    fn set_buffer(&self, name: &str, text: &str) -> Result<()>;

    /// Pastes the buffer `name` into the pane `target` bracketed (`-p`: the program reads it
    /// as a paste, so a newline in it does not send it), then deletes the buffer. Nothing is
    /// pressed after it.
    fn paste_buffer(&self, name: &str, target: &str) -> Result<()>;

    /// What the pane `target` runs now (`#{pane_current_command}`); none when it cannot be asked.
    fn pane_command(&self, target: &str) -> Option<String>;

    /// Session plus hooks: the part of `session new` that can fail, so its unwind guards this call.
    fn start_tracked_session(&self, name: &str, cwd: &str) -> Result<()> {
        self.create_session(name, cwd)?;
        self.install_hooks_for_current_exe()
    }

    /// [`Multiplexer::ensure_hooks_installed`] for the running binary.
    fn install_hooks_for_current_exe(&self) -> Result<()> {
        self.ensure_hooks_installed(&host()?.to_string_lossy())
    }
}

/// A target naming exactly this session: a bare name matches by prefix and treats `*`/`?` as globs.
fn exact(name: &str) -> String {
    format!("={name}")
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
            // inside a tmux client, attaching must switch instead
            inside: std::env::var_os("TMUX").is_some(),
        }
    }
}

impl<'a> TmuxClient<'a> {
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
    /// replaces one from an old `iter` that passed `#{session_name}` everywhere.
    fn hook_is_current(&self, event: &str, args: &str) -> bool {
        // no tmux, or no such option: install ours
        self.output(&["show-options", "-gv", event])
            .is_ok_and(|hook| {
                !hook.trim().is_empty() && format_variables(&hook) == format_variables(args)
            })
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
        let verb = match self.inside {
            true => "switch-client",
            false => "attach-session",
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
        self.output(&[
            "list-clients",
            "-t",
            &exact(session),
            "-F",
            "#{client_name}",
        ])
        .is_ok_and(|clients| !clients.trim().is_empty())
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
            return Err(IterError::CommandFailed(format!(
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
        let target = exact(session);
        let value =
            self.output_trimmed(&["show-options", "-t", &target, "-v", DETACH_MESSAGE_OPTION])?;
        let _ = self.run(&["set-option", "-t", &target, "-u", DETACH_MESSAGE_OPTION]);
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
            Some(dir) => crate::config::expand_home(&dir.replace("$HOME", &home)),
            None => {
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

    fn new_window(
        &self,
        session: &str,
        name: &str,
        cwd: &str,
        command: Option<&str>,
    ) -> Result<()> {
        // `=name:` is the session itself, the window going to its next free index
        let target = format!("{}:", exact(session));
        let mut args = vec!["new-window", "-t", &target, "-n", name, "-c", cwd];
        args.extend(command);
        self.run(&args)
    }

    fn set_buffer(&self, name: &str, text: &str) -> Result<()> {
        // `--`: a prompt starting with `-` is data, not a flag
        self.run(&["set-buffer", "-b", name, "--", text])
    }

    fn paste_buffer(&self, name: &str, target: &str) -> Result<()> {
        self.run(&["paste-buffer", "-p", "-d", "-b", name, "-t", target])
    }

    fn pane_command(&self, target: &str) -> Option<String> {
        self.output_trimmed(&[
            "display-message",
            "-p",
            "-t",
            target,
            "#{pane_current_command}",
        ])
    }
}

/// The pane of window `window` in exactly the session `session`, for [`Multiplexer::paste_buffer`]
/// and [`Multiplexer::pane_command`].
pub fn window_target(session: &str, window: &str) -> String {
    format!("{}:{window}", exact(session))
}

/// [`Multiplexer::current_session_name`] over the real `tmux`, for `locate::current_task`, which has
/// no client to hand.
pub fn current_session_name() -> Option<String> {
    TmuxClient::system().current_session_name()
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
pub const HOOKS: [(&str, &str); 3] = [
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
fn hook_action(iter_bin: &str, event: &str, variables: &str) -> String {
    // Inside the action's single quotes a `'` cannot be written; the rest is shell-quoted.
    let bin = shell_word(iter_bin);
    let args = hook_args(event, variables).join(" ");
    format!("run-shell '{bin} {args}'")
}

/// The arguments of the hook for `event` (one entry of [`HOOKS`]) after the binary:
/// `internal hook <event> -- <format variables..>`, which `hook_action` quotes into the
/// `set-hook` body. `--`: a session name may start with `-`, which the argument parser would read as a flag.
pub fn hook_args(event: &str, variables: &str) -> Vec<String> {
    ["internal", "hook", event, "--"]
        .into_iter()
        .chain(variables.split(' '))
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same words end up in the `set-hook` body, so what is tested is what is installed.
    #[test]
    fn the_hook_action_holds_the_hook_arguments() {
        for (event, variables) in HOOKS {
            let action = hook_action("/usr/bin/iter", event, variables);
            let words = hook_args(event, variables).join(" ");
            assert!(action.contains(&words), "{action} lacks {words}");
        }
    }

    fn variables_of(event: &str) -> &'static str {
        HOOKS
            .iter()
            .find(|(name, _)| *name == event)
            .unwrap_or_else(|| panic!("{event} is not an installed hook"))
            .1
    }

    fn action_for(event: &str) -> String {
        hook_action("/usr/bin/iter", event, variables_of(event))
    }

    /// What each hook is told. `#{hook_client}` is the only name `client-detached` has
    /// (`#{client_name}` can differ on a switch, `#{session_name}` is the most recently active
    /// session); on `session-closed` `#{session_name}` names the session the client moved to;
    /// `client-session-changed` records the new session and closes the one left, so needs both.
    #[test]
    fn each_hook_is_told_the_variables_that_name_what_it_needs() {
        for (event, variables) in [
            (
                "client-session-changed",
                vec!["hook_client", "session_name", "client_last_session"],
            ),
            ("client-detached", vec!["hook_client"]),
            ("session-closed", vec!["hook_session_name"]),
        ] {
            let action = action_for(event);
            assert_eq!(format_variables(&action), variables, "{event}: {action}");
            assert!(!action.contains("client_name"), "{event}: {action}");
        }
    }

    /// A `tmux.conf` hook with a stable path and its own quoting still counts as current; one from
    /// an older `iter`, which passed `#{session_name}` everywhere, does not and is replaced.
    #[test]
    fn a_hook_is_current_by_its_variables_not_its_spelling() {
        for (event, hook, current) in [
            (
                "client-session-changed",
                "run-shell \"~/bin/iter internal hook client-session-changed \
                 '#{hook_client}' '#{session_name}' '#{client_last_session}'\"",
                true,
            ),
            (
                "client-detached",
                "run-shell '/old/iter internal hook client-detached \"#{session_name}\"'",
                false,
            ),
        ] {
            let same = format_variables(hook) == format_variables(variables_of(event));
            assert_eq!(same, current, "{event}");
        }
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
            "run-shell '/usr/bin/iter internal hook client-detached -- #{q:hook_client}'"
        );
    }

    use crate::process::fake::FakeRunner;

    fn client(runner: &FakeRunner, inside: bool) -> TmuxClient<'_> {
        TmuxClient::new(runner, inside)
    }

    /// R4b of task 302: `session new` opens its other windows in the session (targeted
    /// exactly), each named, in the worktree, running its command when it has one.
    #[test]
    fn a_window_opens_in_the_session_with_its_name_folder_and_command() {
        let runner = FakeRunner::default();
        let tmux = client(&runner, false);
        tmux.new_window("web/1 login", "claude", "/wt", Some("claude"))
            .unwrap();
        tmux.new_window("web/1 login", "logs", "/wt", None).unwrap();
        let calls = runner.calls();
        assert_eq!(calls.len(), 2, "{calls:?}");
        for (call, name) in calls.iter().zip(["claude", "logs"]) {
            assert!(call.starts_with("tmux new-window "), "{call}");
            for piece in ["-t =web/1 login", &format!("-n {name}"), "-c /wt"] {
                assert!(call.contains(piece), "{piece} in {call}");
            }
        }
        assert!(
            calls[0].ends_with(" claude"),
            "the command last: {}",
            calls[0]
        );
        assert!(!calls[1].contains("claude"), "no command: {}", calls[1]);
    }

    /// R4c: the prompt goes into a named buffer whole, and is pasted bracketed (`-p`, so a
    /// newline does not send it), the buffer then deleted (`-d`); nothing presses Enter.
    #[test]
    fn a_prompt_is_put_in_a_buffer_and_pasted_bracketed() {
        let runner = FakeRunner::default();
        let tmux = client(&runner, false);
        tmux.set_buffer("iter-302", "line one\nline two").unwrap();
        tmux.paste_buffer("iter-302", "web/1 login:claude").unwrap();
        let calls = runner.calls();
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert!(calls[0].starts_with("tmux set-buffer "), "{}", calls[0]);
        assert!(calls[0].contains("-b iter-302"), "{}", calls[0]);
        assert!(calls[0].ends_with("line one\nline two"), "{}", calls[0]);
        assert!(calls[1].starts_with("tmux paste-buffer "), "{}", calls[1]);
        for piece in [" -p", " -d", "-b iter-302", "-t ", "web/1 login:claude"] {
            assert!(calls[1].contains(piece), "{piece} in {}", calls[1]);
        }
        assert!(!calls.iter().any(|c| c.contains("send-keys")), "{calls:?}");
    }

    /// R4c, the edge: a prompt that starts with `-` (a list, front matter) is the buffer's data,
    /// not a flag of `set-buffer`.
    #[test]
    fn a_prompt_starting_with_a_dash_is_data_not_a_flag() {
        let runner = FakeRunner::default();
        client(&runner, false)
            .set_buffer("iter-302", "- a list item")
            .unwrap();
        let calls = runner.calls();
        assert!(calls[0].ends_with(" -- - a list item"), "{calls:?}");
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

    /// A bare `-t web/a` is a prefix match: it answered for `web/ab` when `web/a` was gone.
    #[test]
    fn session_probes_name_the_session_exactly() {
        let runner = FakeRunner::default();
        runner.reply("tmux list-clients", "/dev/ttys1\n");
        runner.reply("tmux show-options", "bye");
        let tmux = client(&runner, false);
        assert!(tmux.any_client_attached("web/a"));
        assert_eq!(tmux.take_detach_message("web/a").as_deref(), Some("bye"));
        let calls = runner.calls();
        assert!(!calls.is_empty());
        for call in &calls {
            assert!(call.contains("-t =web/a"), "{call}");
        }
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
                "tmux set-hook -g client-session-changed run-shell '/usr/bin/iter internal hook client-session-changed -- #{q:hook_client} #{q:session_name} #{q:client_last_session}'",
                "tmux set-hook -g client-detached run-shell '/usr/bin/iter internal hook client-detached -- #{q:hook_client}'",
                "tmux set-hook -g session-closed run-shell '/usr/bin/iter internal hook session-closed -- #{q:hook_session_name}'",
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
