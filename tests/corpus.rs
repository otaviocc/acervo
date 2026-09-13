//! Golden-corpus regression test.
//!
//! Builds the synthetic libraries under `tests/corpus/` and diffs `acervo`'s
//! dry-run plan against a checked-in snapshot in `tests/corpus/expected/`.
//!
//! These snapshots were captured by running this same fixture/flag matrix
//! against `organize-movies.py` / `organize-tv.py` (the Python scripts this
//! crate replaced) and confirming byte-identical output before those scripts
//! were deleted from the dotfiles repo — see the crate README's
//! "Differential check against the Python originals" section for how to
//! re-verify that against a pre-removal checkout if the parsing rules ever
//! need to be cross-checked against the original again.

use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_acervo")
}

fn build_fixture(script: &str, dir: &Path) {
    let status = Command::new("bash")
        .arg(manifest_dir().join("tests/corpus").join(script))
        .arg(dir)
        .status()
        .expect("failed to run fixture builder");
    assert!(status.success(), "{script} failed");
}

/// Runs `acervo <sub> --root <fixture> <flags...>`, strips the `root=` banner
/// line (which necessarily differs between the fixture's tempdir and the
/// tempdir the snapshot was originally captured from), and returns stdout.
fn run(sub: &str, root: &Path, flags: &[&str]) -> String {
    let output = Command::new(bin())
        .arg(sub)
        .arg("--root")
        .arg(root)
        .args(flags)
        .output()
        .expect("failed to run acervo");
    assert!(output.status.success(), "acervo {sub} exited non-zero");
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with("root="))
        .collect::<Vec<_>>()
        .join("\n")
}

fn expected(name: &str) -> String {
    std::fs::read_to_string(manifest_dir().join("tests/corpus/expected").join(format!("{name}.txt")))
        .unwrap_or_else(|_| panic!("missing tests/corpus/expected/{name}.txt"))
        .trim_end()
        .to_string()
}

fn check(name: &str, fixture_script: &str, sub: &str, flags: &[&str]) {
    let dir = tempdir().unwrap();
    build_fixture(fixture_script, dir.path());
    let actual = run(sub, dir.path(), flags);
    let expected = expected(name);
    assert_eq!(actual, expected, "dry-run plan for '{name}' drifted from the golden snapshot");
}

#[test]
fn movies_default() {
    check("movies", "build_movies_fixture.sh", "movies", &[]);
}

#[test]
fn movies_no_editions() {
    check("movies-no-editions", "build_movies_fixture.sh", "movies", &["--no-editions"]);
}

#[test]
fn movies_sub_lang() {
    check("movies-sub-lang", "build_movies_fixture.sh", "movies", &["--sub-lang", "en"]);
}

#[test]
fn tv_default() {
    check("tv", "build_tv_fixture.sh", "tv", &[]);
}

#[test]
fn tv_bare_number_episodes() {
    check("tv-bare", "build_tv_fixture.sh", "tv", &["--bare-number-episodes"]);
}

#[test]
fn tv_minimal() {
    check("tv-minimal", "build_tv_fixture.sh", "tv", &["--minimal"]);
}

#[test]
fn tv_sub_lang() {
    check("tv-sub-lang", "build_tv_fixture.sh", "tv", &["--sub-lang", "en"]);
}
