# Performance review tools (2026-11-02)

All scripts take the store (`XDG_CONFIG_HOME`) and the release binary as arguments; never point them at `~/.config/iter`.

- `routes.sh <base-url> <label>`: median wall time, bytes and status of every route in the brief's route map (`RUNS=n`, default 5).
- `fsload.sh <store> [live-pid]`: fills a seeded store with 365 notes per organization, 60 per project and 3 agent runs per task (run 3 `running` with `live-pid`).
- `wikigen.sh <store> <pages>`: N well-formed wiki pages in the org-1 wiki, notes folder put under git.
- `wiki_bench.sh <store> <bin> <base-url>`: wiki CLI and pages at 0 / 200 / 2000 pages.
- `cli.sh <store> <bin> [runs]`: median wall time of the common CLI commands (hyperfine is not installed; a Python-timed loop, baseline row included).
- `results/`: the raw numbers the report quotes (`routes_seedonly_*` = bare seed, `routes_loaded_*` = after `fsload.sh`).
