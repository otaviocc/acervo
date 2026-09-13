#!/usr/bin/env bash
# Builds a synthetic TV library covering every branch organize-tv.py / `acervo
# tv` need to agree on. Usage: build_tv_fixture.sh <target-dir>
set -euo pipefail
root="$1"
rm -rf "$root"
mkdir -p "$root"
cd "$root"

touch_() { mkdir -p "$(dirname "$1")"; : > "$1"; }

# Already-organized, with edition tag — should be left alone.
touch_ "Arrival (2016) {edition-Director's Cut}/Season 01/Arrival (2016) {edition-Director's Cut} - s01e01 - Pilot.mkv"

# Multi-episode: S01E01E02 style.
touch_ "Chernobyl.S01E01E02.1080p.WEB-DL.x264-GROUP/chernobyl.s01e01e02.mkv"

# Multi-episode: "(1) and (2)" style.
touch_ "Odd.Show.S02E05.(1).and.(2).720p.HDTV.x264.mkv"

# Bare-number episode inside a Season NN folder (needs --bare-number-episodes).
touch_ "Kids Cartoon/Season 01/Kids Cartoon 3.mkv"

# Year-less scene release (no year anywhere) — files under a year-less folder.
touch_ "Severance.S01.1080p.ATVP.WEB-DL/Severance.S01E01.1080p.ATVP.WEB-DL.x264-GROUP.mkv"
touch_ "Severance.S01.1080p.ATVP.WEB-DL/Severance.S01E01.1080p.ATVP.WEB-DL.x264-GROUP.en.srt"

# Two quality copies of the same episode inside one show folder — the
# longer title variant should be used for both, and since that collapses
# both onto the same destination filename, the larger file should win.
touch_ "Good Show (2015)/Season 01/Good.Show.S01E01.The.Pilot.Episode.720p.WEB-DL.x264-GROUP.mkv"
printf 'xxxxxxxxxxxxxxxxxxxx' > "Good Show (2015)/Season 01/Good.Show.S01E01.The.Pilot.720p.HDTV.x264-GROUP.mkv"

# Subtitle carrying a language code, to stay paired without --sub-lang.
touch_ "Paired Show (2020)/Season 01/Paired Show (2020) - s01e01 - Debut.it.srt"
touch_ "Paired Show (2020)/Season 01/Paired Show (2020) - s01e01 - Debut.mkv"

# Unparseable leftover: no year, no season/episode marker, no bare number.
touch_ "###unparseable###.mkv"

# Loose episode file at the root with no folder.
touch_ "Loose Show (2018) - s01e01 - Loose Episode.mkv"
