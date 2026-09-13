//! `acervo movies` — organize a movie library into
//! `Movies/Movie Name (year)/Movie Name (year).ext`.
//!
//! Ported from `organize-movies.py`; see that script's module docstring
//! (preserved in `organize-movies/SKILL.md`) for the naming convention.

use crate::fsops::{execute_moves, prune_empty_dirs};
use crate::naming::{safe_component, smart_title};
use crate::tokens::{ext_of, EDITION_PATTERNS, LANG_RE, NOISE_RE, PART_RE, QUALITY_RE, YEAR_RE};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

static DOT_UNDERSCORE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[._]+").unwrap());
static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

const MOVIE_VIDEO_EXTS: &[&str] = &[".mkv", ".mp4", ".m4v", ".avi", ".mov", ".wmv", ".ts"];
const SUB_EXTS: &[&str] = &[".srt", ".ass", ".ssa", ".sub", ".vtt"];

pub struct Args {
    pub root: PathBuf,
    pub apply: bool,
    pub no_editions: bool,
    pub sub_lang: String,
}

#[derive(Clone)]
struct Parsed {
    title: String,
    year: i32,
    edition: Option<String>,
}

enum Kind {
    Video,
    Sub,
}

fn media_kind(filename: &str) -> Option<(Kind, String)> {
    let ext = ext_of(filename);
    if MOVIE_VIDEO_EXTS.contains(&ext.as_str()) {
        Some((Kind::Video, ext))
    } else if SUB_EXTS.contains(&ext.as_str()) {
        Some((Kind::Sub, ext))
    } else {
        None
    }
}

fn parse_identity(stem: &str) -> Option<Parsed> {
    // Drop a trailing organized split marker ("Title (2010) - pt1") before matching.
    let stem = crate::tokens::SPLIT_MARKER_TRAILING_RE.replace(stem, "");

    if let Some(caps) = crate::tokens::ALREADY_RE.captures(&stem) {
        let title = caps.name("title").unwrap().as_str().to_string();
        let year: i32 = caps.name("year").unwrap().as_str().parse().unwrap();
        let edition = caps.name("ed").map(|m| m.as_str().to_string());
        return Some(Parsed { title, year, edition });
    }

    let norm = DOT_UNDERSCORE_RE.replace_all(&stem, " ");
    let norm = WHITESPACE_RE.replace_all(&norm, " ").trim().to_string();

    let q = QUALITY_RE.find(&norm);
    let mut head = match q {
        Some(m) => norm[..m.start()].to_string(),
        None => norm.clone(),
    };

    let mut editions: Vec<&'static str> = Vec::new();
    for (rgx, label) in EDITION_PATTERNS.iter() {
        if rgx.is_match(&norm) {
            if let Some(label) = label {
                if !editions.contains(label) {
                    editions.push(label);
                }
            }
            head = rgx.replace_all(&head, " ").to_string();
        }
    }

    let years: Vec<_> = YEAR_RE.find_iter(&head).collect();
    let ym = years.last()?;
    let year: i32 = ym.as_str().parse().ok()?;

    let mut title = head[..ym.start()].trim().to_string();
    if title.is_empty() {
        // leading-year style: "2024 War Machine"
        title = head[ym.end()..].trim().to_string();
    }

    let title = NOISE_RE.replace_all(&title, " ").to_string();
    let title = WHITESPACE_RE.replace_all(&title, " ");
    let title = title.trim_matches(|c: char| " -._(".contains(c));
    if title.is_empty() {
        return None;
    }

    Some(Parsed {
        title: smart_title(title),
        year,
        edition: if editions.is_empty() { None } else { Some(editions.join(" ")) },
    })
}

/// Detect a multi-disc/multi-part split marker (e.g. "CD1", "Part 2").
///
/// Skips titles where the parsed movie title already spells out that same
/// part number (e.g. "Mockingjay Part 1"), so it isn't tagged a second time
/// as "... - pt1".
fn detect_part(stem: &str, title: &str) -> Option<u32> {
    let normalized = DOT_UNDERSCORE_RE.replace_all(stem, " ");
    let caps = PART_RE.captures(&normalized)?;
    let part: u32 = caps.get(1).unwrap().as_str().parse().ok()?;
    let already = Regex::new(&format!(r"(?i)\b(?:part|pt)\s*0*{part}\b")).unwrap();
    if already.is_match(title) {
        return None;
    }
    Some(part)
}

fn folder_name(p: &Parsed, editions_on: bool) -> String {
    let mut name = format!("{} ({})", p.title, p.year);
    if editions_on {
        if let Some(ed) = &p.edition {
            name.push_str(&format!(" {{edition-{ed}}}"));
        }
    }
    safe_component(&name)
}

fn file_base(p: &Parsed, editions_on: bool, part: Option<u32>) -> String {
    let mut base = folder_name(p, editions_on);
    if let Some(part) = part {
        base.push_str(&format!(" - pt{part}"));
    }
    base
}

fn sub_stem(stem: &str) -> String {
    LANG_RE.replace(stem, "").to_string()
}

fn sub_lang_of(stem: &str) -> String {
    LANG_RE.captures(stem).map(|c| c.get(1).unwrap().as_str().to_lowercase()).unwrap_or_default()
}

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let root = fs::canonicalize(&args.root)
        .map_err(|_| anyhow::anyhow!("'{}' is not a directory", args.root.display()))?;
    if !root.is_dir() {
        eprintln!("Error: '{}' is not a directory", root.display());
        return Ok(1);
    }

    let editions_on = !args.no_editions;
    let sub_lang = args.sub_lang.trim_matches('.').to_string();

    println!(
        "root={}  mode={}  editions={}  sub-lang={}",
        root.display(),
        if args.apply { "APPLY" } else { "DRY RUN" },
        if editions_on { "on" } else { "off" },
        if sub_lang.is_empty() { "<none>" } else { &sub_lang }
    );
    println!("{}", "-".repeat(70));

    let mut moves: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut old_dirs: Vec<PathBuf> = Vec::new();

    let plan_file = |moves: &mut Vec<(PathBuf, PathBuf)>,
                          src_path: &Path,
                          ident: &Parsed,
                          target_dir: &Path| {
        let fn_ = src_path.file_name().unwrap().to_string_lossy().to_string();
        let Some((kind, ext)) = media_kind(&fn_) else { return };
        let stem = crate::tokens::stem_of(&fn_).to_string();
        let part = detect_part(&stem, &ident.title);
        let base = file_base(ident, editions_on, part);
        let dst_name = match kind {
            Kind::Sub => {
                let lang = if !sub_lang.is_empty() { sub_lang.clone() } else { sub_lang_of(&stem) };
                if !lang.is_empty() {
                    format!("{base}.{lang}{ext}")
                } else {
                    format!("{base}{ext}")
                }
            }
            Kind::Video => format!("{base}{ext}"),
        };
        moves.push((src_path.to_path_buf(), target_dir.join(dst_name)));
    };

    let mut entries: Vec<PathBuf> = fs::read_dir(&root)?.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    entries.sort();

    let mut loose: Vec<PathBuf> = Vec::new();

    for full in &entries {
        if full.is_dir() {
            // Release folders often nest subtitles in a "Subs/" directory, so
            // walk the whole tree rather than just the top level.
            let mut found: Vec<PathBuf> = Vec::new();
            for entry in walkdir::WalkDir::new(full).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if media_kind(&name).is_some() {
                        found.push(entry.path().to_path_buf());
                    }
                }
            }
            found.sort();
            let mut videos: Vec<&PathBuf> = found
                .iter()
                .filter(|p| {
                    matches!(
                        media_kind(&p.file_name().unwrap().to_string_lossy()),
                        Some((Kind::Video, _))
                    )
                })
                .collect();
            videos.sort_by_key(|p| std::cmp::Reverse(p.file_name().unwrap().len()));

            let mut ident: Option<Parsed> = None;
            for v in &videos {
                let stem = crate::tokens::stem_of(&v.file_name().unwrap().to_string_lossy()).to_string();
                if let Some(p) = parse_identity(&stem) {
                    ident = Some(p);
                    break;
                }
            }
            if ident.is_none() {
                let entry_name = full.file_name().unwrap().to_string_lossy().to_string();
                ident = parse_identity(&entry_name);
            }
            let Some(ident) = ident else {
                warnings.push(format!(
                    "could not parse (folder left as-is): {}",
                    full.file_name().unwrap().to_string_lossy()
                ));
                continue;
            };
            if found.is_empty() {
                continue;
            }
            old_dirs.push(full.clone());
            let new_dir = root.join(folder_name(&ident, editions_on));
            for path in &found {
                plan_file(&mut moves, path, &ident, &new_dir);
            }
        } else if full.is_file() {
            let fn_ = full.file_name().unwrap().to_string_lossy().to_string();
            if media_kind(&fn_).is_some() {
                loose.push(full.clone());
            }
        }
    }

    // Group loose root-level files by movie identity. A case-insensitive key
    // avoids duplicate folders from casing differences.
    let mut canonical: HashMap<String, String> = HashMap::new();
    for full in &loose {
        let fn_ = full.file_name().unwrap().to_string_lossy().to_string();
        let (kind, _) = media_kind(&fn_).unwrap();
        let stem = crate::tokens::stem_of(&fn_).to_string();
        let parse_stem = match kind {
            Kind::Sub => sub_stem(&stem),
            Kind::Video => stem.clone(),
        };
        let Some(mut ident) = parse_identity(&parse_stem) else {
            warnings.push(format!("could not parse (file left as-is): {fn_}"));
            continue;
        };
        let lower_key = folder_name(&ident, editions_on).to_lowercase();
        if let Some(canon) = canonical.get(&lower_key) {
            ident.title = canon.clone();
        } else {
            canonical.insert(lower_key, ident.title.clone());
        }
        let new_dir = root.join(folder_name(&ident, editions_on));
        plan_file(&mut moves, full, &ident, &new_dir);
    }

    let result = execute_moves(&moves, &root, args.apply, &[]);
    let removed = if args.apply { prune_empty_dirs(&root, &old_dirs) } else { 0 };

    println!("{}", "-".repeat(70));
    for w in &warnings {
        println!("  ?? {w}");
    }
    let mut summary = format!(
        "{}: {}   skipped: {}   warnings: {}",
        if args.apply { "moved" } else { "planned" },
        result.done,
        result.skipped,
        warnings.len()
    );
    if result.errors > 0 {
        summary.push_str(&format!("   errors: {}", result.errors));
    }
    if args.apply && removed > 0 {
        summary.push_str(&format!("   empty folders removed: {removed}"));
    }
    if !args.apply {
        summary.push_str("   (DRY RUN — re-run with --apply to execute)");
    }
    println!("{summary}");

    Ok(if result.errors > 0 { 1 } else { 0 })
}
