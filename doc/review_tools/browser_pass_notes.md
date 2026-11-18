# Orchestrator's browser pass (2026-11-02)

Independent of the design/accessibility agents (their reports were not read first). The Claude in
Chrome extension was not connected, so this pass used headless Chrome
(`--headless=new --screenshot`), `--force-dark-mode` for the dark theme, and `--dump-dom`.
Screenshots: `doc/review_tools/screens/*.png` (1280x900 and 360px widths). Server: the shared
scale-4 store on 127.0.0.1:3999. Evidence tag for everything below: `run`.

## Security
- **Clickjacking confirmed.** `doc/review_tools/clickjack_poc.html` served from `http://127.0.0.1:4090`
  (a different origin) frames `http://127.0.0.1:3999/task/1`; it renders fully
  (`screens/poc.png`). `curl -sI` shows only `content-type`, `vary`, `content-length`, `date`:
  no `X-Frame-Options`, no CSP, no `nosniff`, no `Referrer-Policy`.
- Cross-origin `fetch('/search?q=task')` from that page is blocked (no CORS headers): good, the
  JSON endpoints are not readable cross-site. Blind cross-site GETs still execute server side.
- All 24 smoke routes answered 200; `/task/abc` 400, `/task/99999` and `/nope` 404 (custom page).

## Layout at 360 px (regression vs the Oct review's "320px reflow sound")
- `screens/task_narrow_tall.png`: the Details panel (`<aside class="panel">`) is wider than the
  viewport; its values (status pill, dates) are off-screen to the right, only labels are visible.
- `screens/org_tasks_narrow.png`: Tasks rows overflow; the status pill is cut at "wo…"; the tab row
  wraps "Settings" onto a second line (fine) but the search box runs off the right edge.
- `screens/matrix_narrow.png`: quadrant cards extend past the viewport; the top edge colours run
  off-screen.
- `screens/board_narrow.png`, `screens/home_narrow.png`: the top bar is cut at "Instruc…"; the
  Search button is unreachable without horizontal scroll. The bar nav (`ui/frame.css:6`) has no
  wrap/overflow rule and the bar has grown three items (Agents, Instructions, Settings) since the
  Oct review.
- Hypothesis to verify in the a11y/design reports: a fixed `min-width` or the `--main-w`/panel
  width tokens, plus the new nav items, make the document wider than 360px; WCAG 1.4.10 reflow
  at 320 CSS px likely fails (High).

## Design consistency observations
- Kicker rule differs: Org pages use the eyebrow "ORGANIZATION" while Project and Task use
  breadcrumbs `ORG-1 / PROJECT-1 / TASK` (`screens/org_report.png` vs `task.png`).
- 404/400 page (`screens/e404.png`): brand without the icon, no nav, no Search, no footer; the rest
  of the site has all four. Reads as a different site.
- `/agents` empty state is a sentence inside an empty table with headers (`screens/agents.png`);
  other lists use "No X yet." without headers. No filter bar, unlike Tasks/Projects.
- Board header crumbs are `BOARDS / AGENDA` (section as the last crumb) whereas the task page's last
  crumb is the type `TASK`; both differ from org/project. Three kicker conventions in total.
- Report tab (`screens/org_report.png`) is dense but coherent: presets, date range, total, Share.
  The sessions table's "Start 09:13" repeats for three rows of the same day (seed artefact, not a
  bug).
- Dark theme (`screens/board_dark.png`, `task_dark.png`, `home_dark.png`): consistent tokens, the
  accent flips to orange; nothing unstyled. Status pill and tab underline keep contrast visually.
- New task pop-up (`screens/newtask.png`): backdrop blur + `inert` works; hint copy consistent
  ("Leave empty for none."). ✕ only, Cancel is below the fold.
- Long task names wrap correctly on cards and in the h1 at 1280; at 360 the h1 is clipped by the
  overflow above, not by wrapping.

## Performance (single GETs, release build, scale 4, cold)
- `/` 18 KB 18 ms; `/org/1/edit` 17 KB 19 ms; everything else 1–5 ms; `/search?q=task` 9 ms;
  `/style.css` 66 KB; `/board/1` 30 KB. Nothing slow at this size; the performance agent scales it.

## Build/tests
- Cold `cargo build --release --timings`: 1 min 50 s wall, 517 s CPU (report at
  `target/cargo-timings/cargo-timing.html`). Binary 25.7 MB unstripped.
- `cargo test` (web feature on, scratch XDG_CONFIG_HOME): 543 passed, 0 failed, 2 ignored; the
  test-profile build took 43 s on top of the release build.
