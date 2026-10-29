# Search palette

`/` anywhere (not while typing in a field) or the Search button in the bar opens the palette (`ui/search.rs`, `search.css`, `search.js`).

- Left: the results (an icon per kind: board, organization, project, task, note; title with matched letters marked, path and status). Right: the selected page in an `<iframe>` (same origin, so interactive). Under 900px the preview is hidden; under 600px the box is the whole screen.
- Inside the frame `search.js` only adds `.in-preview` to `<html>`; `search.css` hides `.bar`, `.skip`, `.site-foot` and the palette there, so the preview shows the page content only.
- Results: `GET /search?q=` (`web/search_index.rs`) builds an entry for every board, organization, project, task and note, and `web/search.rs::search` ranks them.

## Query

| query | meaning |
|---|---|
| `proj ` | every project; a kind word is a prefix of `board`, `org`/`organization`, `project`, `task`, `note`, then a blank |
| `task fix` | tasks whose name matches `fix` |
| `org acme/proj web` | projects `web` under an organization `acme` (`/` separates scopes) |
| `is:wip` | tasks in a status |
| `login` | titles first (prefix, then word start, then substring, then subsequence), then descriptions |

## Keys

Arrows or Ctrl-N/Ctrl-P move; Enter opens; Shift/Ctrl/Cmd+Enter opens in a new tab; Tab completes a kind word (`pr` -> `project `), otherwise moves into the preview; Esc closes.

## Checks

- `cargo test web::search`.
- Press `/` on a page: the palette opens; in a text field it does not.
- Select a task: the preview shows only its main content, and its links and forms work.
- Type a word: the first place it occurs in the previewed page (the title first) is selected and scrolled into view.
