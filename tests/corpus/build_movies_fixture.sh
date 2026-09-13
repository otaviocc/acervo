#!/usr/bin/env bash
# Builds a synthetic movie library covering every branch organize-movies.py /
# `acervo movies` need to agree on. Usage: build_movies_fixture.sh <target-dir>
set -euo pipefail
root="$1"
rm -rf "$root"
mkdir -p "$root"
cd "$root"

touch_() { mkdir -p "$(dirname "$1")"; : > "$1"; }

# Already-organized — left alone.
mkdir -p "Arrival (2016)"
touch_ "Arrival (2016)/Arrival (2016).mkv"

# A title containing a number, to prove "last year-like token" extraction.
touch_ "Blade.Runner.2049.2017.1080p.BluRay.x264-GROUP/Blade.Runner.2049.2017.1080p.BluRay.x264-GROUP.mkv"

# Edition tag.
touch_ "The.Batman.2022.Directors.Cut.2160p.WEB-DL.x265-GROUP/The.Batman.2022.Directors.Cut.2160p.WEB-DL.x265-GROUP.mkv"

# Release folder with a nested Subs/ directory and a subtitle carrying a
# language code that should be preserved without --sub-lang.
touch_ "John.Wick.2014.1080p.BluRay.x264-GROUP/John.Wick.2014.1080p.BluRay.x264-GROUP.mkv"
touch_ "John.Wick.2014.1080p.BluRay.x264-GROUP/Subs/2_English.srt"

# Multi-part movie: title already spells "Part 1", so no extra "- pt1" tag.
touch_ "Mockingjay.Part.1.2014.1080p.BluRay.x264-GROUP/Mockingjay.Part.1.2014.1080p.BluRay.x264-GROUP.mkv"

# Multi-disc loose files: CD1/CD2 split, same movie.
touch_ "The.Old.Movie.1999.CD1.DVDRip.XviD-GROUP.avi"
touch_ "The.Old.Movie.1999.CD2.DVDRip.XviD-GROUP.avi"

# No detectable year — reported and left alone.
touch_ "NoYearHere.1080p.BluRay.x264-GROUP.mkv"

# Loose file with leading-year style naming.
touch_ "2024 War Machine 1080p WEB-DL x264-GROUP.mkv"
