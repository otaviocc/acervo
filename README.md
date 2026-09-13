# acervo

A single static binary that organizes movie and TV libraries into
[Jellyfin's naming convention](https://jellyfin.org/docs/general/server/media/movie-naming/)
and backfills TV episode titles from [TVMaze](https://www.tvmaze.com/api).
It replaces three Python scripts (`organize-movies.py`, `organize-tv.py`,
`add-episode-titles.py`) that lived in
[otaviocc/dotfiles](https://github.com/otaviocc/dotfiles)'s Claude Code
skills — same parsing rules, same output, now with a test suite.

```
Movies/
  Movie Name (year)/
    Movie Name (year).mkv
    Movie Name (year).english.srt

TV Shows/
  Show Name (year)/
    Season 01/
      Show Name (year) - s01e01 - Episode Title.mkv
      Show Name (year) - s01e01 - Episode Title.en.srt
```

Filename parsing is algorithmic — no hardcoded titles. Editions
(Director's Cut, Extended, ...), multi-episode files, quality/codec/group
junk, and subtitle language codes are all detected and stripped or tagged.
Nothing is ever overwritten; every run defaults to a dry-run that prints the
planned moves without touching the filesystem.

## Install

```sh
cargo install --git https://github.com/otaviocc/acervo
```

## Usage

Every subcommand defaults to a dry run. Review the output, then re-run with
`--apply`.

```sh
acervo movies --root /path/to/Movies [--apply] [--sub-lang en] [--no-editions]
acervo tv     --root /path/to/TV     [--apply] [--minimal] [--sub-lang en] [--bare-number-episodes]
acervo titles --root /path/to/TV     [--apply] [--multi-ep-first] [--threshold 0.75] [--timeout 15]
```

`acervo titles` needs network access to `api.tvmaze.com` (no API key). Run it
after `acervo tv` to fill in episode titles the source release lacked, or to
backfill a premiere year onto a year-less show folder.

See `acervo <subcommand> --help` for the full flag list, or the `SKILL.md`
files in [otaviocc/dotfiles](https://github.com/otaviocc/dotfiles)'s
`claude/.claude/skills/{organize-movies,organize-tv,add-episode-titles}/`
for the naming rules each subcommand follows.

## Commands

```sh
make build     # cargo build --release
make test      # cargo test
make lint      # cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
make install   # cargo install --path . --locked --force

cargo test --lib movies::   # one module's tests
```

`fmt`, `lint` and `test` should all pass before a PR.

### Golden-corpus regression test

`tests/corpus/` builds a synthetic library covering every parsing branch
(multi-episode files, editions, year-less releases, multi-disc movies, quality
copies colliding on one destination, unparseable leftovers, ...) and checks
this binary's dry-run plan against a snapshot in `tests/corpus/expected/`:

```sh
make corpus   # or: cargo test --test corpus
```

Those snapshots were captured by diffing this same fixture/flag matrix against
`organize-movies.py` / `organize-tv.py`, the Python scripts this crate
replaced, confirming byte-identical output before they were deleted from the
dotfiles repo. If the parsing rules ever need to be cross-checked against the
original again, run the fixtures in `tests/corpus/` against those scripts from
a dotfiles checkout before commit `9c5c568` (which deleted them).

## Architecture

- `naming.rs` — `safe_component` (path sanitizing) and `smart_title`
  (light title-casing for shouty scene names).
- `tokens.rs` — shared regexes: quality/junk detection, editions, language
  codes.
- `fsops.rs` — `execute_moves`, the single filesystem chokepoint used by all
  three subcommands: resolves destination collisions (largest file wins),
  refuses to overwrite, defers a move whose target is itself about to move
  away, and falls back to copy+delete across filesystems.
- `sim.rs` — a hand-port of `difflib.SequenceMatcher.ratio()`
  (Ratcliff/Obershelp), since no Rust crate implements the same algorithm and
  a different one would silently shift which TVMaze shows match.
- `tvmaze.rs` — the TVMaze client, behind a `Client` trait so `titles`' tests
  never touch the network.
- `movies.rs` / `tv.rs` / `titles.rs` — one module per subcommand.

## Why Rust

The parsing and planning logic was already 100% algorithmic — no LLM
judgment involved, just regexes and `difflib`. That makes it a plain
language port, and the case for doing it is the static binary (no Python
dependency to keep working across two machines), the speed on a large
library, and above all a real test suite over rules that previously had none.
