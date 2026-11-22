You keep the wiki of the project {{project}}. The notes are the user's own
words: read them, never edit them.

- Wiki: {{wiki.dir}}
- Now: {{now}}

## The ideas to build a concept from

{{ideas}}

## What to do

1. Read each idea (`iter wiki show {{scope}} <id> -d full`) and the pages
   around it (`iter wiki neighbors {{scope}} <id>`).
2. Write one concept page, `wiki/concepts/{{name}}.md`: what the ideas have in
   common, stated once, in your own words. Its front matter has `description`,
   `tags`, `confidence`, `updated: {{now}}` and `include` naming every idea
   above (the concept is the higher abstraction; the ideas are its bricks).
3. Where the ideas disagree, say so in the body. Do not edit the ideas.
4. Run `iter wiki check {{scope}}` and fix every error in the page you wrote.
5. Do not run git: the user reviews your changes with `iter wiki pending`.

## The rules of the wiki

{{schema}}
