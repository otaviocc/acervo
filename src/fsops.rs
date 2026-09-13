//! The single filesystem chokepoint: validates and performs a planned batch
//! of moves, plus the empty-directory sweep that follows an `--apply`.
//!
//! Ported from the `execute_moves`/`prune_empty_dirs` pair that was
//! byte-identical across `organize-tv.py`, `organize-movies.py` and (as a
//! superset, with `dir_renames`) `add-episode-titles.py`.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

pub struct MoveResult {
    pub done: usize,
    pub skipped: usize,
    pub errors: usize,
}

fn rel(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).map(Path::to_path_buf).unwrap_or_else(|_| path.to_path_buf())
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// True when `a` and `b` refer to the same inode on the same device — the
/// case-insensitive-filesystem-safe equivalent of Python's `os.path.samefile`.
fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(ma), Ok(mb)) => ma.dev() == mb.dev() && ma.ino() == mb.ino(),
        _ => false,
    }
}

/// `fs::rename`, falling back to copy-then-remove on `EXDEV` (crossing
/// filesystems), matching `shutil.move`'s behaviour.
fn move_file(src: &Path, dst: &Path) -> io::Result<()> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(libc_exdev()) => {
            fs::copy(src, dst)?;
            fs::remove_file(src)?;
            Ok(())
        }
        Err(e) => Err(e),
    }
}

#[cfg(target_os = "linux")]
fn libc_exdev() -> i32 {
    18
}
#[cfg(target_os = "macos")]
fn libc_exdev() -> i32 {
    18
}
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn libc_exdev() -> i32 {
    18
}

/// Print, validate and optionally perform a batch of `(src, dst)` moves.
///
/// Resolves destinations claimed by several sources in favour of the largest
/// file, refuses to overwrite anything already on disk unless the occupant is
/// itself scheduled to move away (deferred until its own move clears the
/// path), and prints every problem during the dry run before anything moves.
/// `dir_renames` (used only by `titles`) is applied via `fs::rename` after
/// every file move completes.
pub fn execute_moves(moves: &[(PathBuf, PathBuf)], root: &Path, apply: bool, dir_renames: &[(PathBuf, PathBuf)]) -> MoveResult {
    let mut problems: Vec<String> = Vec::new();
    let mut skipped = 0usize;

    // Group by destination, preserving first-seen order.
    let mut order: Vec<PathBuf> = Vec::new();
    let mut by_dst: HashMap<PathBuf, Vec<(PathBuf, PathBuf)>> = HashMap::new();
    for (src, dst) in moves {
        let src_abs = absolute(src);
        let dst_abs = absolute(dst);
        if src_abs == dst_abs {
            continue;
        }
        by_dst.entry(dst_abs.clone()).or_insert_with(|| {
            order.push(dst_abs.clone());
            Vec::new()
        });
        by_dst.get_mut(&dst_abs).unwrap().push((src.clone(), dst.clone()));
    }

    let mut winners: Vec<(PathBuf, PathBuf)> = Vec::new();
    for key in &order {
        let mut items = by_dst.remove(key).unwrap();
        if items.len() > 1 {
            // Best candidate first: largest file, then longest name, then path
            // (stable tiebreak so output is deterministic).
            items.sort_by(|a, b| {
                let sa = file_size(&a.0);
                let sb = file_size(&b.0);
                sb.cmp(&sa)
                    .then_with(|| {
                        let la = a.0.file_name().map(|n| n.len()).unwrap_or(0);
                        let lb = b.0.file_name().map(|n| n.len()).unwrap_or(0);
                        lb.cmp(&la)
                    })
                    .then_with(|| a.0.cmp(&b.0))
            });
            for (src, dst) in &items[1..] {
                problems.push(format!(
                    "duplicate target: keeping {}, skipping {} (both map to {})",
                    rel(root, &items[0].0).display(),
                    rel(root, src).display(),
                    rel(root, dst).display(),
                ));
                skipped += 1;
            }
        }
        winners.push(items[0].clone());
    }

    let sources: std::collections::HashSet<PathBuf> = winners.iter().map(|(src, _)| absolute(src)).collect();

    let mut queue: Vec<(PathBuf, PathBuf, bool)> = Vec::new();
    for (src, dst) in winners {
        let mut in_place = false;
        if dst.exists() {
            in_place = same_file(&src, &dst);
            if !in_place && !sources.contains(&absolute(&dst)) {
                problems.push(format!("target exists, skipping: {}", rel(root, &dst).display()));
                skipped += 1;
                continue;
            }
        }
        queue.push((src, dst, in_place));
    }

    for (src, dst, _) in &queue {
        println!("{}\n   -> {}", rel(root, src).display(), rel(root, dst).display());
    }
    for (old_d, new_d) in dir_renames {
        println!("{}\n   -> {}", rel(root, old_d).display(), rel(root, new_d).display());
    }
    for problem in &problems {
        println!("  !! {problem}");
    }

    if !apply {
        return MoveResult { done: queue.len(), skipped, errors: 0 };
    }

    let mut done = 0usize;
    let mut errors = 0usize;
    let mut remaining = queue;
    while !remaining.is_empty() {
        let (ready, blocked): (Vec<_>, Vec<_>) = remaining.into_iter().partition(|(_, dst, in_place)| *in_place || !dst.exists());
        if ready.is_empty() {
            for (_src, dst, _) in &blocked {
                eprintln!("  !! blocked, target still occupied: {}", rel(root, dst).display());
                skipped += 1;
            }
            break;
        }
        for (src, dst, _) in &ready {
            if let Some(parent) = dst.parent() {
                if let Err(exc) = fs::create_dir_all(parent) {
                    eprintln!("  !! error moving {}: {exc}", rel(root, src).display());
                    errors += 1;
                    continue;
                }
            }
            match move_file(src, dst) {
                Ok(()) => done += 1,
                Err(exc) => {
                    eprintln!("  !! error moving {}: {exc}", rel(root, src).display());
                    errors += 1;
                }
            }
        }
        remaining = blocked;
    }

    for (old_d, new_d) in dir_renames {
        if !old_d.is_dir() {
            continue;
        }
        if new_d.exists() {
            eprintln!("  !! directory already exists, skipping: {}", rel(root, new_d).display());
            skipped += 1;
            continue;
        }
        match fs::rename(old_d, new_d) {
            Ok(()) => done += 1,
            Err(exc) => {
                eprintln!("  !! error renaming {}: {exc}", rel(root, old_d).display());
                errors += 1;
            }
        }
    }

    MoveResult { done, skipped, errors }
}

fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(path) }
}

/// Remove any directory under `candidates` (and their descendants) left empty
/// by a completed `--apply`, deepest first. Never removes `root` itself.
pub fn prune_empty_dirs(root: &Path, candidates: &[PathBuf]) -> usize {
    let root_abs = absolute(root);
    let mut removed = 0usize;
    for base in candidates {
        if !base.is_dir() {
            continue;
        }
        let mut dirs: Vec<PathBuf> = walkdir::WalkDir::new(base)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_dir())
            .map(|e| e.path().to_path_buf())
            .collect();
        // Deepest first, so a nested empty dir is removed before its parent
        // is checked in the same pass (matches os.walk(topdown=False)).
        dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
        for dirpath in dirs {
            if absolute(&dirpath) == root_abs {
                continue;
            }
            if let Ok(mut entries) = fs::read_dir(&dirpath) {
                if entries.next().is_none() && fs::remove_dir(&dirpath).is_ok() {
                    removed += 1;
                }
            }
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn touch(path: &Path, contents: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn largest_file_wins_on_duplicate_target() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let small = root.join("small.mkv");
        let big = root.join("big.mkv");
        touch(&small, b"x");
        touch(&big, b"xxxxxxxxxx");
        let dst = root.join("out.mkv");
        let moves = vec![(small.clone(), dst.clone()), (big.clone(), dst.clone())];
        let result = execute_moves(&moves, root, true, &[]);
        assert_eq!(result.done, 1);
        assert_eq!(result.skipped, 1);
        assert!(dst.exists());
        assert_eq!(fs::read(&dst).unwrap(), b"xxxxxxxxxx");
        assert!(small.exists(), "loser is left in place, not deleted");
    }

    #[test]
    fn refuses_to_overwrite_unrelated_file() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let src = root.join("a.mkv");
        let dst = root.join("b.mkv");
        touch(&src, b"a");
        touch(&dst, b"b");
        let result = execute_moves(&[(src.clone(), dst.clone())], root, true, &[]);
        assert_eq!(result.done, 0);
        assert_eq!(result.skipped, 1);
        assert_eq!(fs::read(&dst).unwrap(), b"b");
    }

    #[test]
    fn deferred_move_when_destination_itself_moves_away() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let a = root.join("a.mkv");
        let b = root.join("b.mkv");
        touch(&a, b"a");
        touch(&b, b"b");
        // a -> b, b -> c: b's move must run before a's, in either order given.
        let c = root.join("c.mkv");
        let moves = vec![(a.clone(), b.clone()), (b.clone(), c.clone())];
        let result = execute_moves(&moves, root, true, &[]);
        assert_eq!(result.errors, 0);
        assert_eq!(result.done, 2);
        assert_eq!(fs::read(&b).unwrap(), b"a");
        assert_eq!(fs::read(&c).unwrap(), b"b");
    }

    #[test]
    fn case_only_rename_is_in_place() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let src = root.join("show (2019)");
        fs::create_dir_all(&src).unwrap();
        let dst = root.join("Show (2019)");
        let result = execute_moves(&[(src.clone(), dst.clone())], root, true, &[]);
        // On a case-sensitive filesystem this is just a normal rename; assert
        // only that it isn't reported as a conflict either way.
        assert_eq!(result.skipped, 0);
        assert_eq!(result.done, 1);
    }

    #[test]
    fn prunes_only_empty_leftover_dirs() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let keep = root.join("Show/Season 01");
        fs::create_dir_all(&keep).unwrap();
        touch(&keep.join("keep.txt"), b"x");
        let empty = root.join("Empty/Nested");
        fs::create_dir_all(&empty).unwrap();
        let removed = prune_empty_dirs(root, &[root.join("Show"), root.join("Empty")]);
        assert_eq!(removed, 2);
        assert!(keep.exists());
        assert!(!root.join("Empty").exists());
    }
}
