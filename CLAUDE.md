# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

`acervo` is a single Rust binary with three subcommands (`tv`, `movies`,
`titles`) that organize Jellyfin media libraries. It is a faithful Rust port
of three Python scripts that used to live in
`otaviocc/dotfiles`'s `claude/.claude/skills/`; see `README.md`'s
Architecture section for the module layout.

## Commands

```sh
make build                              # cargo build --release
make test                               # cargo test
make lint                               # cargo clippy --all-targets -- -D warnings (not verified here — no rustup)
cargo fmt --all -- --check              # not verified here — no rustup
make install                            # cargo install --path . --locked --force
make corpus SKILLS=/path/to/dotfiles/claude/.claude/skills   # differential check vs the Python originals
```

## Architecture

- `naming.rs` / `tokens.rs` — shared string/regex helpers (path sanitizing,
  title-casing, quality/edition/language-code detection). No lookaround or
  backreferences anywhere; every regex ported 1:1 from Python.
- `fsops.rs` — `execute_moves`, the one place that touches the filesystem for
  all three subcommands. Handles destination collisions (largest file wins),
  refuses overwrites, defers moves blocked by an in-flight destination, and
  falls back to copy+delete on `EXDEV`.
- `sim.rs` — a hand-port of `difflib.SequenceMatcher.ratio()`. Do not replace
  this with a `strsim` crate metric — it is a different algorithm and will
  shift which TVMaze shows clear `--threshold`. `autojunk` is intentionally
  omitted (only engages past 200 chars; irrelevant to show titles).
- `tvmaze.rs` — TVMaze client behind a `Client` trait; `titles.rs`'s tests use
  a fake implementation, never the network.
- `movies.rs`, `tv.rs`, `titles.rs` — one module per subcommand, each holding
  its own `Args`, parsing functions, and `run()`.

## Verifying a change to parsing rules

Any change to `naming.rs`, `tokens.rs`, `movies.rs`, or `tv.rs` should be
checked against `tests/corpus/diff_check.sh`, which runs both this binary and
the original Python scripts (from a dotfiles checkout) over an identical
synthetic library and diffs their dry-run plans. See `make corpus` above.
