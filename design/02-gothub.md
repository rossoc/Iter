# 02 – Gothub

[codeberg.org/gothub/gothub](https://codeberg.org/gothub/gothub) is an
alternative GitHub frontend written in Go. The README claims **3 requests,
8 kB, no JavaScript**, against GitHub's 88 requests and 629 kB. The whole UI
is `views/*.html` (Go templates) plus one `public/css/global.css` of about
400 lines. Iter has the same architecture: server-rendered HTML plus one
CSS file.

## What it does well (adopt)

- **Five tokens are enough to theme everything.** `--text`, `--background`,
  `--secondary-background`, `--background-darker` and `--accent`, then
  swapped wholesale in `@media (prefers-color-scheme: light)`. Iter already
  does this. Keep the set of *semantic* tokens small, even if the *scale*
  tokens grow.
- **`color-scheme: dark` / `light` on `:root`.** Form controls and
  scrollbars follow the theme for free. **Iter lacks this. Add it.**
- **Layered surfaces instead of borders.** Cards are `--background` on a
  `--background-darker` page and hover to `--secondary-background`, with
  `transition: ease-in-out .25s`. It shows depth by lightness steps, the
  way Linear does (03).
- **Underlined links with `text-underline-offset: 5px`**, colored on hover.
  They're clearly links but not shouty.
- **Accessibility basics.** A skip link, a `.visually-hidden` utility using
  `clip-path: inset(50%)`, and `alt` text.
- **A single breakpoint (`max-width: 900px`)** collapses the side layout into
  a column. That's simple and good enough for a local tool.
- **Proportion bars with plain HTML:** the language bar is `<span
  style="width:X%;background:C">` inside a rounded `overflow:hidden`
  container. It's a perfect no-JS pattern for **time-per-task bars** in
  Iter's project and session reports.

## What to avoid (don't copy)

- **Emoji as icons** (⭐ 🍴 👀 📁 🗒️). They render differently on every
  OS, can't be colored and look toy-like. Use inline SVG (see 05).
- **`main { margin: 0 24vw }`.** Margins in viewport units make line length
  depend on the screen. Use `max-width` in `ch` or `rem` and center it.
- **A font stack repeated in three rules.** Put it in a token.
- **Hard-coded colors in the light override** (`a { color: black }`).
  Everything should go through tokens.
- **`transition` on `body`**: it animates nothing useful.

## Verdict

Gothub is the *engineering* model: no JS, one stylesheet and tiny pages.
It is not the *aesthetic* model, since it looks functional but plain. Iter
should match its weight and take its visual polish from 01 and 03.
