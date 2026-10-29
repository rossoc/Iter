# Daily notes (`Notes` tab of organizations and projects)

Every organization and project has a Notes tab: one note a day, earlier days one click away. Inspired by a plain folder of `YYYY-MM-DD.md` files (a journal kept in Obsidian); the files stay the source of truth, so the notes can be edited anywhere and the database holds only the path.

## Where notes live
- `notes_path` on the organization and on the project (edit forms, `~` expanded when read).
- A project's folder is its own `notes_path`, else `<organization notes_path>/<project name as a slug>`, else none: the tab then says "Set a notes path" and links to the edit form. An organization's notes are directly in its `notes_path`.
- Resolution is `notes::project_dir` / `notes::org_dir`.

## A note
`<folder>/YYYY-MM-DD.md`:
```
---
layout: page      # default page
title: 29/10      # default dd/mm
---
markdown
```
The day is the file name's (a `date:` in the file is ignored); other keys are ignored. A missing opening `---`, no front matter at all and an empty body are all read. A file whose name is not a date is not a note. The only way from a URL to a file is `notes::parse_date` (strict `YYYY-MM-DD`), so a path cannot be built from request text.

## Pages
- **List** (`?tab=notes`): `ui::notes::notes_tab`; months as headings, rows with the day, the note's own title if it has one, and the first line of its text. The tab's badge counts files without reading them (`notes::count`).
- **Note** (`/{org,project}/{id}/notes/{date}`): crumbs `Organization / Project / Notes / 29 October 2026`, the date written out (the year is on the page), the body as markdown (`ui::prose`), previous/next (`ui::notes::note_nav`). No such day is a 404.
- **Today**: `POST .../notes/today` creates the folder and the day's file (front matter only) if need be and redirects to it. An existing note is never touched. Notes are written in the user's own editor; there is no edit form.

## Tests
`notes.rs` (parsing, listing, creating, folders), `sections.rs` (the tab).
