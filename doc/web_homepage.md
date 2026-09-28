# Homepage redesign (`/`)

Scope: the homepage only. Every other page keeps the current look.
Design notes: `design/04-direction-for-iter.md`.

## Plan

### Comparing the two designs
1. `/` shows the proposal. `/?design=old` shows the current homepage, unchanged.
2. A fixed control at the bottom left, **Before | Proposal**, appears on both versions. It links `/?design=old` ↔ `/`. It needs no JavaScript and no cookie.

### Files
3. New `src/web/home.rs`: the home page (both versions), the proposal's frame, the compare control and the inline icons.
4. New `src/web/home.css`, served at `/home.css`, loaded only by the homepage. All rules are scoped under `.v2` or `.compare`, so the old version is unaffected.
5. `src/web/pages.rs`: remove `home` and its registration.
6. `src/web/mod.rs`: add `mod home` and register it.
7. `src/web/style.css` and `layout.rs`: unchanged.

### Proposal
8. Tokens: warm neutral surfaces, an ink-blue accent, a mono face for metadata, a 4px spacing scale, a quint ease-out, and a dark theme through `prefers-color-scheme`. *(Rev 1: the dark theme is Tokyo Night from `alacritty.toml`: background `#24283b` and text `#a9b1d6` as in the terminal, with the orange `#ff9e64` as the accent.)*
9. Top bar: sticky and frosted. An `iter` wordmark with a logo mark; Home and Board with icons, the active one shown as a tinted pill. *(Rev 2: the Home link is gone, since the `iter` wordmark already goes home; only Board is left.)*
10. ~~Sidebar~~ *(Rev 1: removed from the proposal; the cards cover it.)*
11. Main pane: an eyebrow, a large heading and the existing hint text. Below them, **an overview grid (new)**: one card per organization *(Rev 1: no number)* with its description's first line and its projects (name and base path). A final card holds the projects that have no organization. If there's no data, it shows an empty state that points to `iter init` / `iter new`.
12. Motion: 150ms hover lift on cards and links, switched off under `prefers-reduced-motion`. *(Rev 2: link hover is a 2px orange (dark) / blue (light) line that slides in from the left and out to the right, with the text taking the accent color. It applies to project names and organization titles, replacing the plain underline. Rev 3: 2px instead of 1.5px, because a fractional width rounds to 1px or 2px depending on the link position.)*
13. ~~Narrow screens: the sidebar stacks~~ *(Rev 1: no sidebar; the main pane is centered, max 1180px.)*
14. Icons: Lucide (ISC), pasted inline, with an attribution comment.

### Feature changes
- Added: the overview grid (item 11) and the compare control (item 2).
- Removed: nothing.

## What changed, checked against the plan

| # | Plan | Status | Where |
|---|---|---|---|
| 1 | `/` proposal, `/?design=old` current | ✅ Any other `design` value shows the proposal | `home.rs` `home` |
| 2 | Before \| Proposal control, bottom left, on both | ✅ | `home.rs` `compare`, `home.css` `.compare` |
| 3 | New `home.rs` | ✅ | `src/web/home.rs` |
| 4 | New `home.css` at `/home.css`, scoped | ✅ Checked by script: no selector outside `.v2`, `body:has(.v2)` or `.compare` | `src/web/home.css` |
| 5 | `home` removed from `pages.rs` | ✅ Function and `.page(home)` deleted | `src/web/pages.rs` |
| 6 | `mod home` registered | ✅ | `src/web/mod.rs` |
| 7 | `style.css`, `layout.rs` unchanged | ✅ Except Rev 4: `<html lang="en">` in `layout.rs`, which affects every page | |
| 8 | Tokens and dark theme (Rev 1: Tokyo Night, orange) | ✅ | `home.css` `.v2` dark block |
| 9 | Sticky frosted top bar, wordmark, icon nav (Rev 2: Home link and `HOUSE` icon removed) | ✅ | `proposal` |
| 10 | Sidebar removed (Rev 1) | ✅ `sidebar`, `project_link`, `BUILDING` and the `.side`/`.frame` CSS deleted | |
| 11 | Eyebrow, heading, hint, overview grid, empty state | ✅ Rev 1: the `index` parameter and `.num` removed | `proposal`, `group_card` |
| 12 | Hover motion and reduced motion (Rev 2: sliding accent line) | ✅ Checked with a forced-hover screenshot | `home.css` `.u` |
| 13 | Centered main pane (Rev 1) | ✅ | `home.css` `.v2 main` |
| 14 | Lucide icons inline, attributed | ✅ | `home.rs` icon consts |

Not in the plan (found while testing):
- Base paths show `$HOME` as `~` (`tilde`) and sit inside `<bdi>`, so start-truncation keeps the leading `/`.
- Project names in cards don't wrap (`flex:none; white-space:nowrap`).
- New copy: the heading "Where to next?" and the empty-state sentence.

Side effects on the old design:
- It also loads `/home.css` and shows the compare control. Nothing else changes.

Rev 1 also restyles the compare control in dark mode to match Tokyo Night (orange for the active side). The light theme is unchanged.

Verified: `cargo clippy --features web` is clean, and `cargo test --features web` passes 176 tests. `/`, `/?design=old`, `/boards`, `/org/1`, `/project/4`, `/home.css` and `/style.css` all return 200. Checked with headless Chromium screenshots in light, dark and mobile.

Once one design is picked: delete the losing branch in `home.rs`, the `compare` component, and `.compare` in `home.css`.

## Rev 4: accessibility and performance

Checked with axe-core 4.13 (WCAG 2.2 AA + best practice) through playwright-cli, in light and dark.

| Issue found | Fix |
|---|---|
| Paths (`--faint`) at 2.6:1 (light) and 3.0:1 (dark), below the 4.5:1 minimum | `--faint` changed to `#76716a` (light) and `#8c90a3` (dark): 4.6:1 each |
| Card headings skipped from `h1` to `h3` | Card headings are now `h2` |
| `<html>` had no `lang` | `lang="en"` in `layout.rs` (fixes every page) |
| The two `<nav>` regions weren't told apart | The top bar nav is `aria-label="Main"`; the compare nav already had a label |
| The compare switch didn't expose which side is shown | `aria-current="page"` on the active side |

Result: axe reports 0 violations on `/` in both themes. `/?design=old` still has no `h1`; that's the untouched reference.

Also passing: every link shows a visible 2px focus ring and the tab order is logical. No horizontal scroll at 320px. All targets are at least 24px. The three icons are `aria-hidden`. The text-spacing override (WCAG 1.4.12) clips nothing. Reduced motion turns the transitions off. Known gap: at phone width the fixed compare switch can cover the last row until you scroll. It goes away with the switch.

Performance (release build, the same data):
- Server: every page at ~0.3 ms median, database included. `/` serves ~2,400 req/s with 8 clients.
- Browser, at 4× CPU slowdown: first paint 48 ms (old design: 40). Total render work 15.3 ms (old: 12.3). The biggest extra cost is style calculation, +1.7 ms.
- The hover line animation and scrolling with 260 projects both hold 60 fps, with or without the frosted top bar.
- Nothing needs optimizing.
