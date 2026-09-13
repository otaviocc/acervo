#!/usr/bin/env bash
# Differential check: runs organize-movies.py/organize-tv.py and the Rust
# `acervo` binary, in dry-run, over the same synthetic tree, and diffs their
# planned moves. Requires python3 and the dotfiles skill scripts checked out.
#
# Usage:
#   tests/corpus/diff_check.sh <skills-dir> [movies|tv]
#
# <skills-dir> is claude/.claude/skills/ from the dotfiles repo (holds
# organize-movies/scripts/organize-movies.py and organize-tv/scripts/organize-tv.py).
set -euo pipefail
skills_dir="$1"
which_="${2:-both}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cd "$here/../.."
cargo build --quiet

normalize() {
    # Drop the "root=" line, which necessarily differs (different tempdir),
    # and collapse the mode/threshold banner into something order-agnostic.
    grep -v '^root='
}

run_pair() {
    local name="$1" fixture_script="$2" py_script="$3" rust_sub="$4"
    shift 4
    local extra_flags=("$@")
    echo "=== $name  ${extra_flags[*]:-} ==="
    "$here/$fixture_script" "$work/py-$name"
    "$here/$fixture_script" "$work/rs-$name"
    py_out=$(python3 "$skills_dir/$py_script" --root "$work/py-$name" "${extra_flags[@]}" | normalize)
    rs_out=$(./target/debug/acervo "$rust_sub" --root "$work/rs-$name" "${extra_flags[@]}" | normalize)
    if diff <(echo "$py_out") <(echo "$rs_out") > "$work/$name.diff"; then
        echo "OK: identical dry-run plan"
    else
        echo "MISMATCH:"
        cat "$work/$name.diff"
        return 1
    fi
}

status=0
if [[ "$which_" == "movies" || "$which_" == "both" ]]; then
    run_pair movies build_movies_fixture.sh organize-movies/scripts/organize-movies.py movies || status=1
    run_pair movies-no-editions build_movies_fixture.sh organize-movies/scripts/organize-movies.py movies --no-editions || status=1
    run_pair movies-sub-lang build_movies_fixture.sh organize-movies/scripts/organize-movies.py movies --sub-lang en || status=1
fi
if [[ "$which_" == "tv" || "$which_" == "both" ]]; then
    run_pair tv build_tv_fixture.sh organize-tv/scripts/organize-tv.py tv || status=1
    run_pair tv-bare build_tv_fixture.sh organize-tv/scripts/organize-tv.py tv --bare-number-episodes || status=1
    run_pair tv-minimal build_tv_fixture.sh organize-tv/scripts/organize-tv.py tv --minimal || status=1
    run_pair tv-sub-lang build_tv_fixture.sh organize-tv/scripts/organize-tv.py tv --sub-lang en || status=1
fi
exit $status
