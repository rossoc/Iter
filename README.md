# Iter

A tiny CLI for tracking how long you spend on things, by name.

Tell it when you start something and when you stop, and it'll tell you back
how long that took. No accounts, no server, just a CSV file it appends to.

## Usage

```sh
iter begin writing                       # start (or resume) a session called "writing"
iter end writing                         # close it
iter end writing -m "finished the intro" # ...with a note attached
iter elapsed writing                     # how long has "writing" been running right now?
iter info writing                        # total time + notes for today
iter info writing --date 2026-09-01      # ...or for a specific day
iter list                                # every name you've ever logged
iter session weekday writing             # average hours per weekday for "writing"
```

Names are just strings — call them whatever you're actually tracking
(a project, a client, a habit). You can have several running at once, as
long as they're different names.

`info` and `session weekday` print YAML, so you can pipe them wherever's
convenient.

## Storage

Everything lands in one CSV file — the path is hardcoded in `main.rs`
(`LOG_FILE`). One row per session: `name,begin,end,message`.
`end` and `message` sit blank while a session is still open. It's plain text
on purpose, so it's easy to read or fix by hand if something looks wrong.

## Shell completion

```sh
source <(COMPLETE=zsh iter)
```

gets you tab-completion on session names for every subcommand that takes
one.

## Building

```sh
cargo build --release
```
