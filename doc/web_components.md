# Shared components (`src/web/ui/`)

`src/web/` has the pages (route, loader, `screen` component) and the mappers that
turn models into display rows; `src/web/ui/` has the components they share: one
`.rs` and one `.css` per feature, no models, no routes.

## Layout

- `ui/mod.rs` declares the modules, re-exports the plain types and functions
  (a component shares its module's name, e.g. `frame::frame`, so import it from
  its module) and serves the assets: `/style.css` and the scripts below.
- `/style.css` is `base.css` (tokens, themes, links, focus, motion), every
  component sheet in the order of the `concat!` in `ui/mod.rs`, then `org.css`
  (the Organization report only). The order is the cascade: later wins.
- Scripts, each an enhancement that the page works without: `modal.js` (Esc,
  focus trap, instruction prefill), `filter_bar.js` (Esc closes the syntax tip),
  `search.js` (the `/` palette), `combobox.js` (choice lists), `path_pick.js`
  (folder button). Page scripts in `src/web/`: `board.js` (drag and drop),
  `org.js` (report dates, Share), `platform.js` (activity suggestions).
- Helpers: `ink.rs` (readable chip text color, WCAG 4.5:1), `icons.rs` (inline
  Lucide SVGs, styled by `.i`), `form_error.rs` (`FormError`, which control an
  error is about).

## Components

Page chrome
- `frame(title, error, current, dialog, child)`: top bar, skip link, main pane.
  `dialog` is a pop-up: bar and main become `inert`. Used by every page.
- `site_footer(FooterData)`: wordmark and second menu; data from `web/footer.rs`.
- `page_header` (kicker, title, actions) and `lede`; `breadcrumb(&[Crumb])`
  (`Crumb::link`, `label`, `here`: `aria-current` sits on the tab if the page
  has tabs, else the last crumb); `tabs(label, &[Tab])` with a sliding underline.
- `search_button`, `palette`: the `/` search palette (`GET /search?q=`).

Layout atoms (`layout.rs`)
- `info_columns` (body + aside, `side-folded` variant), `info_body` (body alone),
  `sticky_head`, `jump_link`, `side_lists`, `matrix_grid`. Used by the org,
  project, task, agents and board pages.

Content blocks
- `labelled_section(id, label, count, noun)` and `mono_value`: a small-caps
  heading over content (Info bodies). `panel(id, title, &[Row])`: label/value
  aside; `Row::{Text, Flag, Chips, Status}`.
- `description(text, class)` / `markdown`: markdown rendered with raw HTML
  escaped, only `http(s)`, `mailto` and relative URLs kept, headings shifted two
  levels down.
- `tag_chips`, `status` (icon plus word), `count_badge(n, noun)` (a screen
  reader hears "(3 sessions)"), `empty_line`, `empty_state(svg)`, `notice` /
  `error_box`, `group_card` and `card_grid` (Home, Boards), `folded_list`.

Tables and lists
- `data_table(caption, columns, actions, class, headed, add, empty, child)`: the
  one template for every list. A tinted add row (`AddRow::link(href, label)` to a
  `?new=` pop-up, or `AddRow::post(action, label)`), the child rows (`row`,
  `cell`, `col_head`, `section_row`), and `empty: Some("No notes yet.")` renders
  the `tr.no-rows` last row (defined once in `table.css`; pages draw no "No ...
  yet." of their own). Each element repeats its table `role`, as narrow screens
  lay rows out as boxes.
- Variants: `task_table` and `tasks_tab` (search box plus table; org and project
  pages), `sessions_table` (org report, task page), `project_list` (`compact` is a
  plain list for cards, no add row), `notes_tab` (months as `section_row`s;
  daily notes), `files_block` in `task_agent.rs` (a task folder's notes or
  sketches as `<details>` rows; the Notes tab passes `add`), and the agents and
  instructions lists.
- `filter_bar(&Search)`: a GET form with field `q`, a Filter button, Clear, and
  a "?" inside the field showing the syntax (`Search.syntax`, never empty).
  Hover shows it, click pins it (`:focus-within`), Escape closes it (`filter_bar.js`).

Forms
- `form_shell(action, error, cancel, submit_label, lead, child)`: the POST form
  with error box, "* Required" note, controls, Save and Cancel. `lead` comes
  before the error box (New project's hidden default submit).
- `field`, `textarea`, `select`, `checkbox`, `checklist`, `group`: every control
  has a real `<label for>` (`field_id`) and takes the form's error to set
  `aria-invalid` / `aria-describedby` and the `invalid_cue`. `field` takes
  `identifier`, `autofocus` (a pop-up's first control), `beside` (a control on
  the input's line), `list`, `numeric`. `textarea(hide_label)` keeps the label
  for screen readers only (the note editor).
- `path_field(name, label, ...)`: a filesystem path, the text input plus a folder
  button (`.path-pick`) joined in one bar. The button is `hidden` until
  `path_pick.js` runs, then posts the typed path to `POST /folders/pick`
  (`{"path"}`, `{}` if cancelled, `{"unavailable":true}` without a chooser) and
  fills the input. Used by org, project and New project forms.
- `modal(id, title, close, child)`: see conventions.

Boards
- `board_root` (drop target root, `data-post`), `day_nav`, `hour_grid` /
  `hour_slot`, `quadrant`, `unplaced_list`, `drop_zone`, `side_rail` /
  `side_fold` (the folded Unscheduled column), `task_card` (`Edge` priority
  color plus words), `pick_bar`, `pick_here` (`slot` is the agenda's big box),
  `post_button`, `add_link` / `add_post`. Used by the agenda and matrix pages;
  the `board.js` contract is at the top of that file.

## Conventions

- Pop-ups: a page links to itself with a flag (`?new=task`, `?new=project`,
  `?tab=sessions&new=session`, `?tab=notes&new=note`, keeping `tab` and `q`);
  the server renders the page with the modal over it. The backdrop, the X and
  Cancel are links back without the flag; a refused form re-renders the pop-up
  with the error and what was typed. A form is a pop-up or a settings page,
  never a card above a table.
- Tabs: `?tab=` (`sections.rs`); the note page has `NoteTab { Info, Edit }`.
- Forms: errors come back as a `FormError` naming the control; the error box
  takes focus on load; `required` is announced but enforced by the server.
- Progressive enhancement: every page works without script. Pop-ups close by
  links, drops have pick-mode links and forms, the date title is a plain field
  with a hidden submit, the "?" tip works with CSS alone.
- Accessibility: landmarks with labels; `aria-current` on the current tab and
  crumb; icons `aria-hidden`; status is icon plus word; color is never the only
  cue (priority words, the invalid cue, the pick banner shape); focus ring
  `:focus-visible` 2px accent; contrast at least 4.5:1 for text and 3:1 for
  control borders; touch targets 44px (`--touch`) on coarse pointers;
  `forced-colors` and reduced-motion rules in the sheets; no horizontal scroll
  at 320px; modals trap focus (`modal.js`) and make the footer `inert`.
- Choice lists (`combobox.js`): `select.control` becomes a button opening a
  listbox and `input[list]` shows the same list; the native `<select>` stays,
  hidden and in sync, so forms post it and its label still points at it. Arrow
  keys, Home/End, type-ahead, Enter picks, Esc closes only the list.
- Tests that need a database use `src/fixtures.rs` (`db()`, `project`,
  `add_project`, `board`).
