# acervo

A single static binary that organizes movie and TV libraries into
[Jellyfin's naming convention](https://jellyfin.org/docs/general/server/media/movie-naming/)
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
acervo titles --root /path/to/TV     [--apply] [--multi-ep-first] [--threshold 0.75] [--timeout 15]
```

`acervo titles` needs network access to `api.tvmaze.com` (no API key). Run it
after `acervo tv` to fill in episode titles the source release lacked, or to
backfill a premiere year onto a year-less show folder.

See `acervo <subcommand> --help` for the full flag list.

## License

MIT. See [LICENSE](LICENSE).
