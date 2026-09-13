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
make lint                               # cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
make install                            # cargo install --path . --locked --force
make corpus                             # golden-corpus regression test (tests/corpus.rs) on its own
```

`rustfmt`/`clippy` on this machine live at `/usr/bin/rustfmt` and
`/usr/bin/cargo-fmt` (Fedora's `rustfmt`/`clippy` packages); an unrelated,
much older `rustfmt`/`cargo-fmt` pair sitting in `~/.cargo/bin` shadows them
in `PATH`, so run `PATH="/usr/bin:$PATH" cargo fmt ...` rather than plain
`cargo fmt` if formatting looks like it did nothing or errors on unrecognized
flags.

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

## Code conventions

- **No comments in Rust.** A file may carry a single `//!` line saying what it
  is, for navigation. Nothing else: no `///`, no `//`. A comment is a claim
  nobody checks, and it lends authority to whatever it sits above. Put the
  explanation in the commit message and PR body, or in this file's
  Architecture section above, which are dated/searchable and tied to a diff.
  If code needs a paragraph to be understood, prefer a name, a smaller
  function, or a test. The one structural exception is `clap`'s derive
  macros in `cli.rs`: use `#[arg(help = "...")]` / `#[command(about = "...")]`
  rather than a `///` doc comment above a field or variant, since clap reads
  doc comments as `--help` text and a stray `///` there is functional, not
  narrative.
- Every `src/*.rs` and `tests/*.rs` file starts with `// SPDX-License-Identifier: MIT`
  as its first line, above the `//!` module line.

## Verifying a change to parsing rules

Any change to `naming.rs`, `tokens.rs`, `movies.rs`, or `tv.rs` should be
checked against `tests/corpus.rs`, which builds the synthetic libraries in
`tests/corpus/` and diffs this binary's dry-run plan against the checked-in
snapshots in `tests/corpus/expected/`. If a change is intentional, regenerate
the snapshot for the subcommand/flags it affects rather than hand-editing it —
see `README.md`'s "Golden-corpus regression test" section for how those
snapshots were originally captured (against the Python scripts this crate
replaced, before they were deleted from the dotfiles repo).
