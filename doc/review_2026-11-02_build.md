# Review 2026-11-02 — build time, dev loop and binary size (lens: build)

Reviewer: build. Machine: Apple Silicon (8 cores, cargo `jobs=8`), rustc 1.98.1 stable, nightly installed is 1.91 (too old for topcoat, see "could not assess"). All builds ran in a copy of the sources (`$SCRATCH/build-copy`) with `CARGO_TARGET_DIR=$SCRATCH/build-target/<row>`; `$ROOT` and `$ROOT/target` were not touched.

## Summary (top 5)

1. **The `Iter` crate itself is half the clean build and all of the critical path tail.** In the orchestrator's `--timings` run (110.9 s wall, 417 s CPU, 261 units) `Iter` compiles for 57.2 s starting at 53.8 s, i.e. 52 % of the wall time is a single serial unit. `-Z time-passes` on a non-incremental compile of `Iter` (61.4 s): frontend (expand + resolve + typeck 6.0 + coherence 5.1 + borrowck 2.1) ≈ 15 s, monomorphization collection 7.1 s, `codegen_crate` 20.5 s and the LLVM tail (`finish_ongoing_codegen` 30.0 s, of which `LLVM_thinlto` 25.1 s and `LLVM_passes` 17.5 s overlap) — roughly **75 % backend**. The symbol attribution agrees: `topcoat_view` generics instantiated by Iter (2.50 MB) plus `Iter` (2.04 MB) are 34 % of the 13.25 MB of `__text`; `core`/`alloc` 2.5 MB; everything else is under 0.6 MB per crate. The lever is the amount of monomorphised async view code, not the dependency list. `[run]`
2. **Fat LTO destroys the incremental loop and barely pays in size.** `lto="fat", codegen-units=1`: clean 266-271 s, incremental 228-255 s (vs 7 s), stripped 15.8 MB (vs 18.5 MB). `lto="thin"` costs 32 s per incremental rebuild and is *bigger* (18.9 MB). Neither belongs in the profile the developer builds with. A separate `[profile.dist]` for releases is the right shape — see the recommended block. `[run]`
3. **`opt-level = "s"`/`"z"` is the cheapest size win and also the fastest clean build.** `opt-level="z"`: clean 100.6 s, incremental 4.6 s, stripped 13.5 MB (−27 % vs baseline 18.5 MB); `"s"`: 103 s / 5.1 s / 17.2 MB. Combined with fat LTO + strip in a dist profile: `opt-level="s"` gives **12.1 MB** (clean 213 s). The web views are I/O-bound HTML rendering; `-Os` is unlikely to be measurable at runtime (not benchmarked — see "could not assess"). `[run]`
4. **Half the critical path before `Iter` is the sqlite C amalgamation.** `libsqlite3-sys` build script runs 26.1 s (26.9 → 53.0 s) serially; `rusqlite` then compiles 53.0 → 53.8 and `Iter` starts at 53.8. Without it, `Iter` could start at 49.6 s (oxigraph's end). Linking the macOS system sqlite (3.51.0) cut the clean build to 105 s (vs 119-143 s) and 1.7 MB of stripped size, and compiled with no code change. It is a portability trade (see F5); it only matters for clean builds, which are rare. `[run]`
5. **`syn 2` is pulled by exactly one crate: `macros/`** (`cargo tree -i syn@2`). `iter-macros` compiles unchanged against `syn = "3"` (`cargo check -p iter-macros` in the copy, 2.85 s, syn@2 gone from the graph). All other duplicates (sha1/digest 0.10 via tungstenite, base64 0.22 via topcoat-core, getrandom 0.2 via ring, hashbrown 0.14 via dashmap←oxigraph, rand 0.9 via oxigraph) are upstream and not fixable from this repo. `[run]`

Baseline → best: clean 142.7 s / 25.6 MB (18.5 MB stripped) → **dev loop unchanged** (7 s incremental; 4.6 s with `-Oz`), **dist 12.1 MB** (`-Os` + fat LTO + cgu=1 + strip, 213 s clean, tests green on the LTO profile: 543 passed / 2 ignored).

## Scope

Clean and incremental release build time, debug build and `cargo test --no-run` time, binary size (unstripped/stripped), dependency graph trims, duplicate crates, proc-macro cost, and the risks of each profile knob. Not in scope: runtime performance of the produced binaries, Linux/Windows builds, CI configuration (there is none in the repo).

## Method

- Read: `Cargo.toml`, `macros/Cargo.toml`, `Cargo.lock`, `target/cargo-timings/cargo-timing.html` (parsed `UNIT_DATA` with python), `src/main.rs`, `src/completion.rs`, `src/args/*.rs` (completer wiring), `src/wiki/retrieve.rs:169`, `src/wiki/graph.rs`, `src/wiki/ontology.rs`, `src/platform.rs:143-170`, `src/push.rs:13`, `src/md_edit.rs`, `src/config.rs`; registry sources `topcoat-0.9.0/{Cargo.toml,src/lib.rs,src/dev.rs}`, `topcoat-router-0.9.0/{Cargo.toml,src/body.rs,src/router.rs}`, `topcoat-core-0.9.0/src/memoize/cache.rs`, `oxigraph-0.5.13/Cargo.toml`, `ureq-3.5.0/Cargo.toml`, `rusqlite-0.40.2/Cargo.toml`.
- Ran: 17 timed builds via `doc/review_tools/build/exp.sh` (clean release build → `touch src/main.rs` → rebuild → `strip` copy) and `doc/review_tools/build/dev_exp.sh` (debug clean / incremental / `cargo test --no-run` twice); `cargo tree -d`, `cargo tree -i <crate>`, `cargo tree -e normal --prefix none | sort -u | wc -l`; `doc/review_tools/build/prune_sim.py` (removes edges from `cargo metadata`'s resolve and counts packages that disappear — lock-wide, not platform-filtered, so numbers are "of 297"); `doc/review_tools/build/size_by_crate.py` (per-crate `__text` size from `nm -n` address deltas, parsing v0 mangling — `cargo-bloat` was **not** installed, nothing was installed); `RUSTC_BOOTSTRAP=1 cargo rustc --release --config profile.release.incremental=false -- -Z time-passes` on stable, reusing the baseline deps; `size -m` on the binary; `cargo check` for the clap_complete and regex-lite trims; `cargo test --release` with the LTO/cgu1/strip overrides (`XDG_CONFIG_HOME=$SCRATCH/test-home`, `ITER_PLATFORM_KEY=dummy`).
- Variance: clean wall times on this machine moved ±20 % between identical-ish rows (baseline 142.7 s, `incremental=false` 119.2 s, `strip` 168.8 s, orchestrator's run 110.9 s). The baseline row ran while I was running `cargo tree`/`cargo metadata` in parallel. Treat clean-time differences under ~25 s as noise; incremental times and sizes are stable.
- NOT checked: runtime speed of `-Os`/`-Oz` binaries; Linux builds (system sqlite / native-tls behave differently there); `cargo udeps` (nightly 1.91 refused topcoat's `rust-version = 1.98`; I did not update the toolchain); `panic = "abort"` (not built — argued below); whether `--timings` on the copy reproduces the orchestrator's critical path exactly (it does within noise: `Iter` starts after `rusqlite` in every log).

## Experiments

All release rows: `cargo build --release` in the copy, `[profile.release] incremental = true` as in HEAD unless overridden. "incr" = rebuild after `touch src/main.rs`. Stripped = `strip` applied to a copy (what `strip = true` would produce).

| # | Row (override) | clean | incr | size | stripped | tests |
|---|---|---|---|---|---|---|
| 1 | baseline (HEAD profile) | 142.7 s | 7.0 s | 25.6 MB | 18.5 MB | green (orchestrator Phase 0) |
| 1b | `incremental = false` (cargo default release) | 119.2 s | 61.6 s | 25.5 MB | 18.6 MB | n/a |
| 2 | `strip = true` | 168.8 s (noise) | 4.2 s | 18.5 MB | 18.5 MB | n/a (no codegen change) |
| 3 | `lto = "thin"` | 123.1 s | 32.4 s | 26.0 MB | 18.9 MB | not run: superseded by 4 |
| 4 | `lto = "fat"`, `codegen-units = 1` | 270.9 s | 254.7 s | 18.7 MB | 15.8 MB | **543 passed, 2 ignored** (with strip, incremental=false; 144 s build+run) |
| 5 | `opt-level = "s"` | 103.3 s | 5.1 s | 26.8 MB | 17.2 MB | not run (see note) |
| 6 | `opt-level = "z"` | 100.6 s | 4.6 s | 25.6 MB | 13.5 MB | not run (see note) |
| 7 | dist: fat + cgu1 + strip + no incremental | 266.5 s | 228.2 s | 15.8 MB | 15.8 MB | = row 4 run |
| 7b | dist + `opt-level = "s"` | 213.5 s | 179.2 s | **12.1 MB** | 12.1 MB | not run (see note) |
| 7c | dist + system sqlite + ureq native-tls | 264.1 s | 211.0 s | 13.4 MB | 13.4 MB | not run |
| 9 | `--no-default-features` (CLI only) | 71.1 s | 1.7 s | 12.8 MB | 10.3 MB | green per brief |
| T1 | `rusqlite` without `bundled` (system sqlite 3.51.0) | 105.1 s | 4.3 s | 23.9 MB | 16.8 MB | not run |
| T2 | `ureq` `native-tls` + `gzip` (needs `der = "0.8.0-rc.8"` pin) | 121.2 s | 4.4 s | 24.4 MB | 17.4 MB | not run |

Note on tests for rows 5/6/7b: `opt-level` changes codegen but not semantics for safe Rust; the test suite was run once on the most aggressive codegen change (row 4/7: fat LTO, one codegen unit, stripped) and is green. I did not spend another ~2.5 min per row on `-Os`/`-Oz`; the maintainer should run `cargo test --profile dist` once after adopting the block below.

Debug profile (row 8), `cargo build` / `cargo test --no-run`:

| Row | dev clean | dev incr | `test --no-run` after build | `test --no-run` incr | debug bin | `target/` |
|---|---|---|---|---|---|---|
| baseline `[profile.dev]` | 53.7 s | 4.3 s | 17.6 s | 3.3 s | 93 MB | 2.2 GB |
| `debug = "line-tables-only"` + `[profile.dev.package."*"] debug = false` | 45.2 s | 5.9 s | 15.7 s | 3.4 s | 84 MB | 1.7 GB |
| …+ `[profile.dev.package."*"] opt-level = 1` | 92.8 s | 4.1 s | 15.9 s | 3.1 s | 63 MB | 1.6 GB |

The first variant is a free −16 % on the clean debug build and −0.5 GB of `target/`; `opt-level = 1` for dependencies nearly doubles the clean build and buys nothing here (the test suite runs in under 1 s even unoptimised).

Feature trims (copy only):

| Trim | compiles | crates (`cargo tree -e normal`, platform-filtered) | effect | verdict |
|---|---|---|---|---|
| topcoat: drop `tokio-tungstenite`, `h2`, `proc-macro-crate`/`toml_edit` | not possible from Iter | would remove 11 of 297 lock packages (tungstenite, tokio-tungstenite, sha1 0.10, data-encoding; h2, fnv, tokio-util, tracing, tracing-core; proc-macro-crate, toml_edit) | h2 is 7.6 s CPU off the critical path, 0.19 MB text; tungstenite ~0.03 MB | upstream only (F6) |
| `clap_complete` without `unstable-dynamic` | **no**: 105 errors (`src/completion.rs:8` `engine`, `src/main.rs:37` `env`, 51× `Arg::add` in `src/args/*.rs`) | −2 (clap_complete, is_executable) | 6.8 s CPU off critical path, 0.03 MB | keep |
| `regex` → `regex-lite` (edit `src/wiki/retrieve.rs:8`) | yes (`cargo check` ok) | **223 → 223**: `spareval` keeps `regex` | only Iter's own instantiations | not worth it |
| `serde_yaml` → `serde_yaml_ng`/`serde_yml` | not tried | both are forks of the same `unsafe-libyaml` code: 0 crate delta | 0.14 + 0.06 MB text either way | keep (frozen, not broken) |
| `oxigraph` features | already minimal: `default-features = false` drops rocksdb; formats are not feature-gated | 29 packages, ~1.2 MB text (spargebra .23, spareval .22, oxiri .17, sparopt .15, oxjsonld .12, oxttl .08, oxrdfxml .05 …) | `src/wiki/graph.rs` uses Store + SPARQL + N-Triples dump; `ontology.rs:256` parses Turtle; JSON-LD/RDF-XML parsers are dead weight but not optional | nothing to do |
| `rusqlite` system sqlite (T1) | yes | 223 (libsqlite3-sys stays) | −26 s CPU on the critical path, −1.7 MB | dev-only convenience; keep `bundled` for shipped binaries (F5) |
| `ureq` native-tls (T2) | yes, with a prerelease `der` pin | removes ring, rustls, rustls-webpki, webpki-roots, untrusted, subtle, getrandom 0.2; **adds** native-tls, security-framework(+sys), core-foundation, der, pem-rfc7468, base64ct, tempfile, rustix, getrandom 0.4, webpki-root-certs | −1.1 MB stripped, build time within noise | not worth it: swaps one duplicate (getrandom 0.2) for another (0.4), needs a pre-release pin, and needs OpenSSL on Linux |

Proc-macro cost (orchestrator's timing report, CPU seconds, all finished before 40 s and none on the critical path): syn 3.0.6 5.3, serde_derive 2.5, syn 2.0.119 2.2, clap_derive 1.6, peg-macros 1.4, toml_edit 1.0, thiserror-impl 1.0, iter-macros 0.7, topcoat-view-macro 0.4, topcoat-core-macro 0.3, topcoat-router-macro 0.3, proc-macro-crate 0.4, proc-macro2+quote (libs and build scripts) ~3.5 — about 21 s CPU, 5 % of the 417 s total. The 43 build scripts total 48.6 s CPU, 26.1 s of it libsqlite3-sys. The expansion cost *inside* `Iter` is small: `macro_expand_crate` 1.19 s of 61.4 s.

## Findings

| ID | lens | severity | file:line | evidence | effort | one-line fix |
|---|---|---|---|---|---|---|
| F1 | build | Medium | `Cargo.toml:49-50` | run | S | Add a `[profile.dist]` (inherits release, `opt-level="s"`, `lto="fat"`, `codegen-units=1`, `strip=true`, `incremental=false`) for shipped binaries; leave `release` incremental. |
| F2 | build | Medium | `src/web/**` (15.2 k of 37.5 k LOC; `src/main.rs:4` `recursion_limit = "256"`) | run | L | The `Iter` crate's own codegen is 52 % of the clean build; reduce monomorphised view code (fewer generic `view!` instantiations / `Box<dyn View>` at page boundaries) before touching deps. |
| F3 | build | Low | `macros/Cargo.toml:12` | run | S | `syn = "3"`: `iter-macros` is the only `syn 2` user; compiles unchanged, removes one syn rlib (2.2 s CPU). |
| F4 | build | Low | `Cargo.toml` (no `[profile.dev]`) | run | S | `[profile.dev] debug = "line-tables-only"` + `[profile.dev.package."*"] debug = false`: debug clean 53.7 → 45.2 s, `target/` 2.2 → 1.7 GB. Do not add `opt-level = 1` for deps (92.8 s). |
| F5 | build | Low | `Cargo.toml:20` (`rusqlite … features = ["bundled"]`) | run | S | The 26 s sqlite C compile is the serial step right before `Iter`. Keep `bundled` for distribution; optionally expose it as a default feature so a dev can opt out (`--no-default-features --features web`). |
| F6 | build | Low | `Cargo.toml:30-34` (topcoat comment) ; `topcoat-0.9.0/src/dev.rs:7,26`, `topcoat-router-0.9.0/Cargo.toml:129-136` | read | S (issue upstream) | topcoat `serve` hard-depends on `tokio-tungstenite` (`connect`) for `notify_ready` and on hyper `http2`; neither is feature-gated. The sha1 0.10/0.11 workaround comment in `Cargo.toml:21-25` exists only because of this. File upstream: gate `dev.rs` websocket client and `http2` behind features. |
| F7 | build | Low | `Cargo.toml:46-50` (comment) | run | S | The comment's numbers are stale: incremental=false costs 61.6 s per rebuild today (not 75 s), incremental 7 s (not 2-27 s). Update the comment when F1 lands. |
| F8 | build | Low | `Cargo.toml:14` (`clap_complete … unstable-dynamic`) | run | — | Confirmed load-bearing: 105 compile errors without it (`src/completion.rs:8`, `src/main.rs:37`, 51 `Arg::add` sites). Nothing to do; recorded so nobody re-tries the trim. |

### Details

**F1 — dist profile.** `run`. Rows 4/7/7b. A fat-LTO profile on `release` would make every `cargo build --release` after a one-line change take 228-255 s (row 4, 7) instead of 7 s. Thin LTO is strictly worse for this crate (incremental 32 s, binary larger). So the two profiles must be separate. Measured candidates for `dist`: fat+cgu1+strip 15.8 MB / 266 s; **+ `opt-level = "s"` 12.1 MB / 213 s** (recommended: smaller *and* faster to build). `strip = true` removes the 8 MB `__LINKEDIT` symbol table (57 k symbols) — backtraces from a stripped binary show addresses only; `release` already has `debug = 0`, so line numbers were never there, only function names are lost. If panics in the field must be debuggable, use `strip = "debuginfo"` (keeps symbol names; on this binary that is the same as unstripped since there is no debuginfo) or ship a `dSYM` via `split-debuginfo = "packed"` with `debug = "line-tables-only"` (+~1 MB in the binary before strip, not measured). `panic = "abort"` would save `__eh_frame` (1.79 MB) + `__gcc_except_tab` (0.87 MB) but is **ruled out**: `topcoat-router-0.9.0/src/body.rs:112` and `src/router.rs:100` wrap body/handler polls in `catch_unwind` so a panicking view takes down one response instead of the server, and `topcoat-core/src/memoize/cache.rs:311` relies on it too; with `abort` a panic in any `view!` kills `iter serve`. Repro: `cargo build --release --config 'profile.release.lto="fat"' --config 'profile.release.codegen-units=1' --config 'profile.release.strip=true' --config 'profile.release.incremental=false' --config 'profile.release.opt-level="s"'`.

**F2 — the Iter crate is the build.** `run`. `cargo-timing.html`: `Iter` 57.2 s of 110.9 s wall, starting at 53.8 s. `-Z time-passes` (non-incremental): `codegen_crate` 20.5 s, `finish_ongoing_codegen` 30.0 s, `monomorphization_collector_graph_walk` 6.5 s, `type_check_crate` 6.0 s, `coherence_checking` 5.1 s, `MIR_borrow_checking` 2.1 s, `expand_crate` 1.2 s, `link` 0.7 s; total 61.4 s. Per-crate text (`size_by_crate.py`): `topcoat_view` 2.50 MB / 6772 symbols (generic view code instantiated for Iter's 200 `view!` sites and 216 `async fn`s under `src/web`), `Iter` 2.04 MB / 1996 symbols, `core` 1.96 MB / 9489 symbols (mostly `fmt` and iterator adaptors instantiated by the same code). The `--no-default-features` row (71 s, 10.3 MB, 1.7 s incremental) is the control: the web feature is ~70 s of clean build, 8 MB of stripped binary and 5 s of every incremental rebuild. Without reading the view code in depth (not this lens) the levers are the usual ones: type-erase at page boundaries, avoid generic helpers that take `impl View`/closures where a concrete type works, and keep `incremental = true`. `recursion_limit = "256"` in `src/main.rs:4` is itself a symptom of deep nested generic types.

**F3 — syn 2 only from iter-macros.** `run`. `cargo tree -i syn@2 -e normal --depth 1` → `iter-macros` only. In the copy: `sed s/syn = "2.0.119"/syn = "3"/ macros/Cargo.toml; cargo check -p iter-macros` → ok in 2.85 s; `cargo tree -i syn@2` → "nothing to print". The macro uses `parse_macro_input`, `DeriveInput`, `Data`, `Fields`, `LitStr`, `syn::Error`, `meta.value()?.parse()` (`macros/src/lib.rs:6-84`), all unchanged in syn 3. Saves one 2.2 s compile unit and one rlib; the other duplicates need upstream bumps (sha1 0.10 ← tungstenite; base64 0.22 ← topcoat-core; getrandom 0.2 ← ring; hashbrown 0.14 ← dashmap 6 ← oxigraph; rand/rand_core 0.9 ← oxigraph/oxrdf, no 0.8 is actually present — the seed's "rand 0.8/0.9" is refuted: `cargo tree -i rand@0.8` is empty).

**F4 — dev profile.** `run`. See the debug table. `test --no-run` is dominated by linking the 93 MB test binary; deps-without-debuginfo cuts it 17.6 → 15.7 s and the debug binary 93 → 84 MB. Backtraces in `cargo test` keep file:line for Iter's code (line tables) and lose only variable info and dependency frames' lines.

**F5 — sqlite amalgamation on the critical path.** `run`. Units: `libsqlite3-sys build-script (run)` 26.9 → 53.0 s; `libsqlite3-sys` 53.0 → 53.1; `rusqlite` 53.0 → 53.8; `Iter` 53.8 → 110.9. Next-latest dependency to finish: `oxigraph` at 49.6 s. T1 (system sqlite) clean 105.1 s, stripped 16.8 MB (−1.7 MB), no code change. macOS ships sqlite 3.51.0 (`/usr/bin/sqlite3 --version`), far above rusqlite 0.40's minimum, and no FTS5/JSON extension is used by `src/` (grep). Risk: on Linux the system sqlite version varies and `bundled` is what makes a single binary portable; also cargo caches the C compile, so this costs only on clean builds. Recommend keeping `bundled` as the shipped default; if clean builds matter in CI, make it a feature (`default = ["web", "bundled-sqlite"]`).

**F6 — topcoat's non-optional extras.** `read` (registry sources) + `run` (`prune_sim.py`). `topcoat-0.9.0/Cargo.toml` feature `serve = ["router", "dep:futures-util", "dep:tokio", "dep:tokio-tungstenite", …]`; `src/dev.rs:7` `use tokio_tungstenite::tungstenite::Message;`, `:26` `connect_async` — a websocket *client* used only when `TOPCOAT_DEV_URL` is set. `topcoat-router-0.9.0/Cargo.toml:129-136` `hyper = { features = ["server","http1","http2"] }` unconditionally, `hyper-util` `server-auto`. `topcoat-core-grammar` → `proc-macro-crate` → `toml_edit` (1.0 s CPU). The 11 packages are ~10 s CPU and ~0.25 MB; low value, but this is also what forces the `sha1 = "0.11"` direct dependency with the long comment in `Cargo.toml:21-25` (Iter needs sha1 for `src/push.rs:13` anyway, so the dependency stays; the comment could go once tungstenite is out).

**F7 — stale comment.** `run`. Row 1 vs 1b: incremental saves 61.6 − 7.0 = 55 s per rebuild and costs nothing on the clean build within noise (142.7 vs 119.2 is noise in the wrong direction; the orchestrator's clean run was 110.9 s with incremental on). The "75 s → 2-27 s" figures are from an older, smaller crate.

**F8 — clap_complete.** `run`. `cargo check --release` with `clap_complete = "4.6.9"`: `error[E0432]: unresolved import clap_complete::engine` (`src/completion.rs:8`), `clap_complete::env` (`src/main.rs:37`), 51× `error[E0599]: no method named add found for struct Arg` (`src/args/*.rs`). 22 crates in the seed is wrong as a *delta*: `prune_sim.py` shows only `clap_complete` and `is_executable` would disappear; the other 20 are shared with `clap`.

### Recommended `Cargo.toml` profile block (text; `$ROOT` not edited)

```toml
# Day-to-day builds: `cargo build --release`. A rebuild after one change reuses
# what did not change (7 s instead of 62 s measured 2026-11-02); LTO is not
# used here because it would make that rebuild 230 s.
[profile.release]
incremental = true

# Shipped binaries: `cargo build --profile dist` (binary in target/dist/).
# Measured 2026-11-02 on the full feature set: 12.1 MB vs 25.6 MB, clean build
# 213 s vs ~120 s, `cargo test --profile dist` green. `panic = "abort"` is
# deliberately absent: topcoat contains panics per request with
# `catch_unwind` (topcoat-router body.rs / router.rs), abort would take the
# server down with the first panicking view.
[profile.dist]
inherits = "release"
incremental = false
opt-level = "s"      # the views are HTML rendering; 15.8 MB -> 12.1 MB
lto = "fat"
codegen-units = 1
strip = true         # drops the 8 MB symbol table; backtraces become addresses only

# Debug builds and tests: full debuginfo for this crate's lines only.
# Clean debug build 54 s -> 45 s, target/ 2.2 GB -> 1.7 GB (measured).
[profile.dev]
debug = "line-tables-only"
[profile.dev.package."*"]
debug = false
```

And in `macros/Cargo.toml`: `syn = "3"` (F3).

## Confidence

High on the incremental-rebuild and size numbers (stable across runs, deterministic). Medium on clean wall times (±20 % run-to-run on this laptop; order the rows by their ranking, not by the second decimal). High on the dependency-graph facts (`cargo tree`, registry sources read). The `time-passes` split is from one run with `RUSTC_BOOTSTRAP=1` on stable, non-incremental; incremental compiles shift more time into `incr_comp` bookkeeping but the backend share stays dominant. Not measured: runtime impact of `-Os`.

## Out-of-scope observations

- Security: `ureq` with `rustls` + `webpki-roots` means TLS trust is the bundled Mozilla root set, not the OS keychain; corporate MITM proxies and private CAs will fail against the platform webhook (`src/platform.rs:148`). `ureq`'s `platform-verifier` feature (rustls-platform-verifier) would use the system trust store without switching to native-tls. Not a vulnerability; a deployment surprise.
- Security: `strip = true` plus `debug = 0` means a panic in production yields a backtrace of bare addresses; pair with the `dSYM`/`split-debuginfo` route if crash reports matter.
- `serde_yaml 0.9.34+deprecated` is unmaintained (not a build issue; the forks are byte-identical for this use). If a CVE lands in `unsafe-libyaml` there is no upstream to take the fix.
- `cargo udeps` could not run (nightly 1.91 < topcoat's `rust-version = 1.98`); a `rustup update nightly` would allow it. `serde_json` and `sha1` are used directly (`src/push.rs:13`; `serde_json` in `src/reporting/worklog.rs`, `src/web/search_index.rs`, `src/web/folders.rs`), so no obvious unused direct dependency by grep.
- `target/` of the review worktree is 2.2 GB for debug + release; nothing in the repo sets `CARGO_TARGET_DIR` or a `.cargo/config.toml`, so each worktree (the project creates them per task, `src/utils/workspace.rs`) rebuilds everything from scratch — a shared `CARGO_TARGET_DIR` or `sccache` would help the project's own workflow more than any profile flag.

## What I could not assess

- Runtime effect of `opt-level = "s"`/`"z"` on `iter serve` latency and on the SPARQL evaluation in `src/wiki/graph.rs` (needs the performance lens' harness).
- Whether a Linux build with the system sqlite / native-tls trims links (not attempted; macOS only).
- `cargo udeps` / unused features (nightly too old; toolchain not updated during the review).
- The exact share of `Iter`'s codegen attributable to specific views (would need `-Z self-profile` or `cargo llvm-lines`, neither installed).
