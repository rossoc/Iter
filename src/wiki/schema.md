# The wiki

You are working in a project whose notes are kept as a wiki you can query with
`iter wiki`. Read as little as you can: find the few pages that matter for the
task, load those, and start.

## Layers

- **Notes** are the user's raw thoughts: free text, one file per day
  (`2026-10-27.md`) or in a task's `notes/` folder. They are the source of
  truth. **Never edit a note.** If a page is wrong, the user will say so in
  today's note and you will read it there.
- **Pages** are what you write from the notes: strict front matter, one idea
  per file. Their class is the folder they are in (below), never a field.
- **A task's folder** (`tasks/<id>/`) is yours while you work on it: `plan.md`,
  and `sketches/` for your own memory. It is private to the task.

## Where things live

{{places}}

## A page

```markdown
---
description: One line saying what the page says. Retrieval reads this first.
tags: [net, retry]
confidence: medium
updated: 2026-10-27T14:03:00
source: [2026-10-27]
nb: [concepts/backoff]
---
# Title

Body, with [[links]] to other pages. Every link is a `ref`.
```

- `confidence` is {{confidence}}.
- `description` and `tags` are required on every page outside `sketches/`. Reuse
  tags that exist: `iter wiki sparql 'SELECT DISTINCT ?t WHERE { ?s iter:tag ?t }'`.
- `updated` is when you last wrote the page, as ISO 8601. A note that changed
  after the `updated` of the pages made from it has not been ingested yet.
- Paths are ids. Write a target as its path inside the level (`ideas/retry`,
  the `wiki/` may be left out), or `level:path` for another level.

Relations you may write in the front matter (a value or a list):

{{relations}}

## Reading

1. `iter wiki find <words> --budget 800` ranks pages by path, title, tags and
   description. Start here.
2. `iter wiki show <id>` gives one page at a depth: `--depth description`,
   `summary` (default: relations and headings) or `full`.
3. `iter wiki neighbors <id> --hops 1` follows relations. `nb` links are the
   ones that matter: read them before you work on the topic. Backlinks are
   `referencedBy`, `includedIn`, `sourceOf`.
4. `iter wiki grep <pattern>` searches the text (`--notes` includes raw notes).
5. `iter wiki sparql '<query>'` asks the graph anything. Prefixes `iter:`,
   `owl:`, `rdf:`, `rdfs:`, `xsd:` are declared. A page is `<urn:iter:LEVEL/PATH>`,
   for example `<urn:iter:acme.app/wiki/concepts/backoff>`.

Every command takes `--task`, `--project` or `--org` (default: the task of the
tmux session you are in), and `--format text|yaml|json`.

## Writing

- **Ingest.** For each note that changed, make or update Ideas in
  `wiki/ideas/`: one thought each, in the user's words, `source` pointing at the
  note, a description and tags. Do not summarize a note as a whole.
- **Build.** When several Ideas say one thing, write a Concept in
  `wiki/concepts/` with `source` pointing at the Ideas (and `include` if it is
  made of them). People, tools and services go in `wiki/entities/`; a
  comparison or analysis across pages goes in `wiki/syntheses/`.
- **Contradiction.** If a new note disagrees with a page, keep both and say so:
  `contradicts` on the new page, or `supersedes` if it replaces the old one.
  Lower `confidence` where the evidence is thin.
- **Move** a page with `iter wiki mv <from> <to>`, never with `mv`: it rewrites
  every reference.
- **Check** your work with `iter wiki check`. Fix every error. Warnings are
  yours to judge. `info` lines say what to do next (notes to ingest, ideas that
  could become a concept).
