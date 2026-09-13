# acervo

A single static binary that organizes movie and TV libraries into
Jellyfin's naming convention for
[movies](https://jellyfin.org/docs/general/server/media/movies/) and
[shows](https://jellyfin.org/docs/general/server/media/shows/)
and backfills TV episode titles from [TVMaze](https://www.tvmaze.com/api).

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
cargo install acervo
```

Or, on macOS:

```sh
brew install otaviocc/apps/acervo
```

## Usage

Every subcommand defaults to a dry run. Review the output, then re-run with
`--apply`.

```sh
acervo movies --root /path/to/Movies [--apply] [--sub-lang en] [--no-editions]
acervo tv     --root /path/to/TV     [--apply] [--minimal] [--sub-lang en] [--bare-number-episodes]
acervo titles --root /path/to/TV     [--apply] [--multi-ep-first] [--threshold 0.75] [--interactive] [--timeout 15]
```

`acervo titles` needs network access to `api.tvmaze.com` (no API key). Run it
after `acervo tv` to fill in episode titles the source release lacked, or to
backfill a premiere year onto a year-less show folder.

### Picking a show by hand

When no TVMaze result clears `--threshold`, the show is skipped. `--threshold`
is a single number for the whole library, though, so loosening it to rescue one
show loosens matching for every other show too. `--interactive` asks instead:

```
resolving: The Office (2005)
  ?? no reliable TVMaze match — 201 file(s) skipped
     closest: The Office (2001)  similarity 0.71
     pick [1-3], #<tvmaze-id> or URL, s=skip, q=quit:
```

Answer with a candidate number, or paste a TVMaze id or show URL when the right
show is not listed at all. Picks are written to `.acervo-titles.json` at the
library root, and later runs reuse them without searching or asking again —
including the `--apply` run that follows a dry run, which is why that file is
written during a dry run too.

The file is a plain list you can edit or delete by hand, and pinning a show id
there works without ever passing `--interactive`:

```json
[
  { "show": "The Office", "year": 2005, "tvmaze_id": 526 },
  { "show": "Severance",  "year": 0,    "tvmaze_id": 44933 }
]
```

All three fields are required — a malformed file is reported and then ignored
wholesale, so a typo costs you every pin in it. `show` is matched
case-insensitively against the show name in the filenames, and `year` is the
year in those filenames, or `0` for a show that has none yet (the case
`acervo titles` backfills a premiere year onto).

A pinned show is never second-guessed: if TVMaze cannot be reached for that id,
the show is skipped rather than re-matched by similarity.

`--interactive` is the only thing that ever writes the file, and it is ignored
when stdin is not a terminal, so piped and scripted runs behave exactly as they
do without it.

See `acervo <subcommand> --help` for the full flag list.

## License

MIT. See [LICENSE](LICENSE).
