# Core architecture notes

Design rationale for the non-web, non-agent modules. Current behaviour only.

## Database (`src/db`, `macros`)

- `#[derive(Table)]` (in `macros/`) reads a struct's fields as the columns, in declaration order, minus the `id` primary key (assigned on insert, read from column 0). `COLUMNS`, the row reader and the bound values therefore come from one list. Field types without `FromSql`/`ToSql` use `#[table(with = "module")]` (e.g. `datetime`).
- Timestamps are stored as fixed-width `YYYY-MM-DD HH:MM:SS` text, so lexicographic order is chronological. Date ranges are half-open (`[from 00:00:00, to+1day 00:00:00)`).
- Migrations: `PRAGMA user_version` counts the steps run. Each step and its version bump share one `IMMEDIATE` transaction, so concurrent openers (a tmux hook and an `iter serve` request) queue on the write lock, and the version is re-read under it. Steps are append-only.
- Index creation failures are swallowed on purpose: an index is an optimisation and may name columns an old database does not have yet.
- `organization_id` and `board_id` on `projects` are `ON DELETE SET NULL`: groups are containers, deleting one only detaches its projects.
- WAL mode is stored in the file; on a brand-new file two processes can race to switch it and SQLite reports the loser busy immediately (bypassing `busy_timeout`), so the switch is retried.
- Sessions under `Session::MIN_SECONDS` are never stored; the migration that dropped existing ones measures in whole seconds (`julianday` float error puts an exact minute at 59.99...).

## tmux hooks (`src/tmux.rs`, `src/utils/clock.rs`)

Wrong format variables fail silently (the hook runs but names the wrong session), measured on tmux 3.6:

| event                    | `hook_client` | `session_name`   | `hook_session_name` |
|--------------------------|---------------|------------------|---------------------|
| `client-session-changed` | the client    | the new session  | empty               |
| `client-detached`        | the client    | some other one   | empty               |
| `session-closed`         | --            | some other one   | the session         |

- `#{client_name}` can name a different client on `switch-client`; use `#{hook_client}`.
- `client-detached` cannot name its session, so `client-session-changed` records it in a server-wide option (`@iter_client_session_<tty>`) and the detach hook reads it back (consumed on read).
- `client-session-changed`, not `client-attached`, starts a session: it fires on attach and on `switch-client`.
- The leaving client is already off the session's client list when the hooks fire, so counting attached clients tells whether it was the last one; only then is the clock stopped.
- A hook already working in `tmux.conf` (right variables, any path or quoting) is left alone; ours bakes in `current_exe`, which may be a deleted debug build. Hook variables are `#{q:...}`-quoted so a name with a quote stays one shell word.
- After a reboot nothing closes open clocks; they are closed at the last tmux-resurrect save when that lies between the session start and the new server start, otherwise deleted.
- The detach message option is a shell-visible interface: a `bind` in `tmux.conf` sets it, the `client-detached` hook moves it onto the session it closes.

## Workspace teardown (`src/utils/workspace.rs`)

`kill-session` kills every pane, including the process running `task done` from inside that session. So the database is written first, and the tmux kill happens last; teardown returns the session to kill so a loop (`project delete`) can kill after finishing.

## Daily notes (`src/notes.rs`)

`YYYY-MM-DD.md` per day. Front matter lines (`layout`, `title`, `date`) closed by `---`, then the body. Reading is forgiving (opening `---` accepted, header optional, body optional); the day is always the file name's.

## Config and secrets

- `iter.yaml` lives in `$XDG_CONFIG_HOME/iter` (else `$HOME/.config/iter`; `%APPDATA%` off unix); every key optional. An unparseable file is fatal so a typo cannot silently move the database.
- The platform API key lives only in the OS keychain (or `ITER_PLATFORM_KEY`), is handed to the macOS `security` tool on stdin, and is redacted from any text that may echo it. Keys are kept by organization name, so renames move the key.
