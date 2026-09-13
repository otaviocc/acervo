//! `acervo tv` — organize a TV library into
//! `TV Shows/Show Name (year)/Season NN/Show Name (year) - sNNeNN - Title.ext`.
//!
//! Ported from `organize-tv.py`; see that script's module docstring
//! (preserved in `organize-tv/SKILL.md`) for the naming convention.

use crate::fsops::{execute_moves, prune_empty_dirs};
use crate::naming::{safe_component, smart_title};
use crate::tokens::{EDITION_PATTERNS, LANG_RE, NOISE_RE, QUALITY_RE, YEAR_RE, ext_of};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

static DOT_UNDERSCORE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[._]+").unwrap());
static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static SE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[Ss](\d{1,2})[Ee](\d{1,2})").unwrap());
static MULTI_EP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\s._-]*-?\s*[eE](\d{1,2})\b").unwrap());
static MULTI_PAREN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\((\d+)\)[\s._]*and[\s._]*\((\d+)\)").unwrap());
static BARE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^|[\s._-])(\d{1,2})\s*$").unwrap());
static SEASON_DIR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[Ss]eason\s*(\d{1,2})").unwrap());
static SEASON_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b[Ss]\d{1,2}(?:[Ee]\d{1,2})?\b").unwrap());
static SEASON_TRAIL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b[Ss]eason\s*\d{1,2}\b.*$").unwrap());
static RES_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\d{3,4}p$").unwrap());
static TRAILING_BRACKET_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\[[^\]]*$").unwrap());

const LEADING_JUNK: &[&str] = &["repack", "proper", "internal", "real"];
const VIDEO_EXTS: &[&str] = &[".mkv", ".mp4", ".m4v", ".avi", ".mov", ".ts"];
const SUB_EXTS: &[&str] = &[".srt", ".ass", ".ssa", ".sub", ".vtt"];

pub struct Args {
    pub root: PathBuf,
    pub apply: bool,
    pub minimal: bool,
    pub sub_lang: String,
    pub bare_number_episodes: bool,
}

#[derive(Clone)]
struct Identity {
    title: String,
    year: i32, // 0 means "no year"
    edition: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Video,
    Sub,
}

/// A file with no episode number yet, awaiting the specials pass: (path, filename, kind, ext).
type SpecialItem = (PathBuf, String, Kind, String);
/// A file already grouped by stem for the specials pass: (path, kind, ext).
type SpecialGroup = (PathBuf, Kind, String);
/// A parsed episode file: (path, title, season, episode, second episode, kind, ext).
type EpisodeItem = (PathBuf, String, u32, u32, Option<u32>, Kind, String);
/// An episode number: (season, episode, second episode for multi-episode files).
type EpNum = (u32, u32, Option<u32>);

fn media_kind(filename: &str) -> Option<(Kind, String)> {
    let ext = ext_of(filename);
    if VIDEO_EXTS.contains(&ext.as_str()) {
        Some((Kind::Video, ext))
    } else if SUB_EXTS.contains(&ext.as_str()) {
        Some((Kind::Sub, ext))
    } else {
        None
    }
}

fn parse_show_identity(name: &str) -> Option<Identity> {
    if let Some(caps) = crate::tokens::ALREADY_RE.captures(name) {
        let title = caps.name("title").unwrap().as_str().to_string();
        let year: i32 = caps.name("year").unwrap().as_str().parse().unwrap();
        let edition = caps.name("ed").map(|m| m.as_str().to_string());
        return Some(Identity { title, year, edition });
    }

    let norm = DOT_UNDERSCORE_RE.replace_all(name, " ");
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
        title = head[ym.end()..].trim().to_string();
    }

    let title = NOISE_RE.replace_all(&title, " ").to_string();
    let title = WHITESPACE_RE.replace_all(&title, " ");
    let title = title.trim_matches(|c: char| " -._(".contains(c));
    if title.is_empty() {
        return None;
    }

    Some(Identity { title: smart_title(title), year, edition: if editions.is_empty() { None } else { Some(editions.join(" ")) } })
}

/// Derive a show title from a release name that carries no year.
///
/// Most scene TV releases omit the year ("Severance.S01.1080p.ATVP.WEB-DL"),
/// so cut the release tail, any sNN/sNNeNN marker and a trailing "Season N"
/// and keep what is left. Returns `None` if nothing usable remains.
fn fallback_show_title(name: &str) -> Option<String> {
    let norm = DOT_UNDERSCORE_RE.replace_all(name, " ");
    let mut norm = WHITESPACE_RE.replace_all(&norm, " ").trim().to_string();
    if let Some(q) = QUALITY_RE.find(&norm) {
        norm.truncate(q.start());
    }
    if let Some(s) = SEASON_TOKEN_RE.find(&norm) {
        norm.truncate(s.start());
    }
    let norm = SEASON_TRAIL_RE.replace(&norm, "").to_string();
    let mut norm = norm;
    for (rgx, _label) in EDITION_PATTERNS.iter() {
        norm = rgx.replace_all(&norm, " ").to_string();
    }
    let norm = NOISE_RE.replace_all(&norm, " ").to_string();
    let norm = WHITESPACE_RE.replace_all(&norm, " ");
    let norm = norm.trim_matches(|c: char| " -._([".contains(c));
    if norm.is_empty() { None } else { Some(smart_title(norm)) }
}

/// `(season, episode, episode2, title)`. `season` is `None` when the number
/// came from a trailing bare number, in which case the caller resolves the
/// season from the containing folder.
fn parse_episode(stem: &str, allow_bare: bool) -> Option<(Option<u32>, u32, Option<u32>, String)> {
    if let Some(m) = SE_RE.find(stem) {
        let caps = SE_RE.captures(stem).unwrap();
        let ss: u32 = caps.get(1).unwrap().as_str().parse().unwrap();
        let ee: u32 = caps.get(2).unwrap().as_str().parse().unwrap();
        let mut after = &stem[m.end()..];
        let mut ee2 = None;
        if let Some(m2) = MULTI_EP_RE.find(after) {
            let c2 = MULTI_EP_RE.captures(after).unwrap();
            ee2 = Some(c2.get(1).unwrap().as_str().parse().unwrap());
            after = &after[..m2.start()];
        } else if let Some(m3) = MULTI_PAREN_RE.find(after) {
            ee2 = Some(ee + 1);
            after = &after[..m3.start()];
        }
        let title = clean_episode_title(after);
        return Some((Some(ss), ee, ee2, title));
    }
    if allow_bare {
        if let Some(caps) = BARE_RE.captures(stem) {
            let n: u32 = caps.get(1).unwrap().as_str().parse().unwrap();
            return Some((None, n, None, String::new()));
        }
    }
    None
}

/// Try to detect a season number from the folder containing a file.
fn season_from_folder(path: &Path) -> Option<u32> {
    let parent = path.parent()?.file_name()?.to_string_lossy().to_string();
    if let Some(caps) = SEASON_DIR_RE.captures(&parent) {
        return caps.get(1).unwrap().as_str().parse().ok();
    }
    if YEAR_RE.find(&parent).is_some() {
        return None;
    }
    if BARE_RE.is_match(&parent) {
        return Some(1);
    }
    None
}

fn clean_episode_title(text: &str) -> String {
    let s = text.replace(['_', '.'], " ");
    let s = WHITESPACE_RE.replace_all(&s, " ").trim().to_string();
    if s.is_empty() {
        return String::new();
    }
    let mut tokens: Vec<&str> = Vec::new();
    for tok in s.split(' ') {
        if RES_RE.is_match(tok) {
            break;
        }
        tokens.push(tok);
    }
    while let Some(first) = tokens.first() {
        if LEADING_JUNK.contains(&first.to_lowercase().as_str()) {
            tokens.remove(0);
        } else {
            break;
        }
    }
    let title = tokens.join(" ");
    let title = title.trim_matches(|c: char| c == ' ' || c == '-').to_string();
    let title = TRAILING_BRACKET_RE.replace(&title, "").to_string();
    title.trim_matches(|c: char| c == ' ' || c == '-').to_string()
}

fn show_folder_name(ident: &Identity, editions_on: bool) -> String {
    let mut name = if ident.year != 0 { format!("{} ({})", ident.title, ident.year) } else { ident.title.clone() };
    if editions_on {
        if let Some(ed) = &ident.edition {
            name.push_str(&format!(" {{edition-{ed}}}"));
        }
    }
    safe_component(&name)
}

fn ep_tag(ss: u32, ee: u32, ee2: Option<u32>) -> String {
    let mut tag = format!("s{ss:02}e{ee:02}");
    if let Some(ee2) = ee2 {
        tag.push_str(&format!("-e{ee2:02}"));
    }
    tag
}

fn file_base(ident: &Identity, editions_on: bool, ss: u32, ee: u32, ee2: Option<u32>, title: &str, minimal: bool) -> String {
    let mut base = format!("{} - {}", show_folder_name(ident, editions_on), ep_tag(ss, ee, ee2));
    if !title.is_empty() && !minimal {
        base.push_str(&format!(" - {}", safe_component(title)));
    }
    base
}

fn season_dir(ss: u32) -> String {
    if ss == 0 { "Season 00".to_string() } else { format!("Season {ss:02}") }
}

fn sub_stem(stem: &str) -> String {
    LANG_RE.replace(stem, "").to_string()
}

struct Ctx<'a> {
    root: &'a Path,
    minimal: bool,
    sub_lang: &'a str,
    moves: Vec<(PathBuf, PathBuf)>,
}

impl<'a> Ctx<'a> {
    fn plan_episode(&mut self, src_path: &Path, ident: &Identity, title: &str, ep: EpNum, kind: Kind, ext: &str) {
        let (ss, ee, ee2) = ep;
        let base = file_base(ident, true, ss, ee, ee2, if self.minimal { "" } else { title }, self.minimal);
        let fname = if kind == Kind::Sub && !self.sub_lang.is_empty() {
            format!("{base}.{}{ext}", self.sub_lang)
        } else {
            format!("{base}{ext}")
        };
        let target_dir = self.root.join(show_folder_name(ident, true)).join(season_dir(ss));
        self.moves.push((src_path.to_path_buf(), target_dir.join(fname)));
    }
}

fn canonicalize(canonical: &mut HashMap<String, String>, ident: &mut Identity) {
    let key = show_folder_name(ident, true).to_lowercase();
    if let Some(title) = canonical.get(&key) {
        ident.title = title.clone();
    } else {
        canonical.insert(key, ident.title.clone());
    }
}

/// File items with no episode number as numbered Season 00 specials.
///
/// Videos and their subtitles are grouped by stem so a pair keeps one number,
/// and the original name becomes the episode title so the files stay
/// identifiable.
fn plan_specials(ctx: &mut Ctx, warnings: &mut Vec<String>, items: &[SpecialItem], ident: &Identity, label: &str, bare: bool) {
    let mut groups: Vec<(String, Vec<SpecialGroup>)> = Vec::new();
    for (fpath, fn_, kind, ext) in items {
        let stem = crate::tokens::stem_of(fn_).to_string();
        let key = if *kind == Kind::Sub { sub_stem(&stem) } else { stem };
        match groups.iter_mut().find(|(k, _)| k == &key) {
            Some((_, v)) => v.push((fpath.clone(), *kind, ext.clone())),
            None => groups.push((key, vec![(fpath.clone(), *kind, ext.clone())])),
        }
    }
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    for (index, (key, files)) in groups.iter().enumerate() {
        let title = {
            let cleaned = clean_episode_title(key);
            if cleaned.is_empty() { key.clone() } else { cleaned }
        };
        for (fpath, kind, ext) in files {
            ctx.plan_episode(fpath, ident, &title, (0, (index + 1) as u32, None), *kind, ext);
        }
    }
    if !groups.is_empty() {
        let mut msg = format!("{} item(s) in '{}' had no episode number — filed as Season 00 specials", groups.len(), label);
        if !bare {
            msg.push_str("  [if they are episodes named 'Name 1..N', add --bare-number-episodes]");
        }
        warnings.push(msg);
    }
}

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let root = fs::canonicalize(&args.root).map_err(|_| anyhow::anyhow!("'{}' is not a directory", args.root.display()))?;
    if !root.is_dir() {
        eprintln!("Error: '{}' is not a directory", root.display());
        return Ok(1);
    }

    let sub_lang = args.sub_lang.trim_matches('.').to_string();
    let bare = args.bare_number_episodes;

    println!(
        "root={}  mode={}  titles={}  sub-lang={}  bare-numbers={}",
        root.display(),
        if args.apply { "APPLY" } else { "DRY RUN" },
        if args.minimal { "minimal" } else { "keep" },
        if sub_lang.is_empty() { "<none>" } else { &sub_lang },
        if bare { "on" } else { "off" }
    );
    println!("{}", "-".repeat(70));

    let mut ctx = Ctx { root: &root, minimal: args.minimal, sub_lang: &sub_lang, moves: Vec::new() };
    let mut warnings: Vec<String> = Vec::new();
    let mut old_dirs: Vec<PathBuf> = Vec::new();
    let mut canonical: HashMap<String, String> = HashMap::new();
    let mut warned_no_year: HashSet<String> = HashSet::new();

    let mut entries: Vec<PathBuf> = fs::read_dir(&root)?.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    entries.sort();

    let mut loose: Vec<String> = Vec::new();

    for full in &entries {
        if !full.is_dir() {
            let fn_ = full.file_name().unwrap().to_string_lossy().to_string();
            if media_kind(&fn_).is_some() {
                loose.push(fn_);
            }
            continue;
        }

        let entry_name = full.file_name().unwrap().to_string_lossy().to_string();
        let mut ident = parse_show_identity(&entry_name);
        let mut videos: Vec<String> = Vec::new();
        if ident.is_none() {
            for e in walkdir::WalkDir::new(full).into_iter().filter_map(|e| e.ok()) {
                if e.file_type().is_file() {
                    let name = e.file_name().to_string_lossy().to_string();
                    if let Some((Kind::Video, _)) = media_kind(&name) {
                        videos.push(name);
                    }
                }
            }
            let mut sorted_videos = videos.clone();
            sorted_videos.sort_by_key(|v| std::cmp::Reverse(v.len()));
            for v in &sorted_videos {
                ident = parse_show_identity(crate::tokens::stem_of(v));
                if ident.is_some() {
                    break;
                }
            }
        }

        let ident = match ident {
            Some(i) => i,
            None => {
                let mut title = fallback_show_title(&entry_name);
                if title.is_none() {
                    let mut sorted_videos = videos.clone();
                    sorted_videos.sort_by_key(|v| std::cmp::Reverse(v.len()));
                    for v in &sorted_videos {
                        title = fallback_show_title(crate::tokens::stem_of(v));
                        if title.is_some() {
                            break;
                        }
                    }
                }
                let Some(title) = title else {
                    warnings.push(format!("could not parse show folder (left as-is): {entry_name}"));
                    continue;
                };
                let ident = Identity { title: title.clone(), year: 0, edition: None };
                // Only worth saying while the folder is still being reshaped; an
                // already-settled year-less library should not re-warn every run.
                if entry_name != show_folder_name(&ident, true) {
                    warnings.push(format!(
                        "no year detected in '{entry_name}' — filing under '{title}'; rename to '{title} (year)' for a reliable Jellyfin match"
                    ));
                }
                ident
            }
        };

        let mut ident = ident;
        canonicalize(&mut canonical, &mut ident);
        old_dirs.push(full.clone());

        let mut episodes: Vec<EpisodeItem> = Vec::new();
        let mut unmatched: Vec<SpecialItem> = Vec::new();

        let mut files: Vec<PathBuf> = walkdir::WalkDir::new(full)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.path().to_path_buf())
            .collect();
        files.sort();

        for fpath in &files {
            let fn_ = fpath.file_name().unwrap().to_string_lossy().to_string();
            let Some((kind, ext)) = media_kind(&fn_) else { continue };
            let raw_stem = crate::tokens::stem_of(&fn_).to_string();
            let stem = if kind == Kind::Sub { sub_stem(&raw_stem) } else { raw_stem };
            match parse_episode(&stem, bare) {
                Some((ss, ee, ee2, title)) => {
                    let ss = match ss {
                        Some(s) => s,
                        None => season_from_folder(fpath).unwrap_or(1),
                    };
                    episodes.push((fpath.clone(), title, ss, ee, ee2, kind, ext));
                }
                None => unmatched.push((fpath.clone(), fn_, kind, ext)),
            }
        }

        let mut ep_best: HashMap<(u32, u32), String> = HashMap::new();
        for (_, title, ss, ee, ee2, _, _) in &episodes {
            if ee2.is_none() && !title.is_empty() {
                let key = (*ss, *ee);
                let better = ep_best.get(&key).map(|t| title.len() > t.len()).unwrap_or(true);
                if better {
                    ep_best.insert(key, title.clone());
                }
            }
        }

        for (path, title, ss, ee, ee2, kind, ext) in &episodes {
            let title =
                if ee2.is_none() { ep_best.get(&(*ss, *ee)).cloned().unwrap_or_else(|| title.clone()) } else { title.clone() };
            ctx.plan_episode(path, &ident, &title, (*ss, *ee, *ee2), *kind, ext);
        }

        plan_specials(&mut ctx, &mut warnings, &unmatched, &ident, &entry_name, bare);
    }

    // Loose root-level files, grouped for the specials pass by show folder name.
    let mut loose_specials: Vec<(String, Identity, Vec<SpecialItem>)> = Vec::new();
    for fn_ in &loose {
        let (kind, ext) = media_kind(fn_).unwrap();
        let raw_stem = crate::tokens::stem_of(fn_).to_string();
        let sub = if kind == Kind::Sub { sub_stem(&raw_stem) } else { raw_stem };
        let ident_opt = parse_show_identity(&sub);
        let ep = parse_episode(&sub, bare);

        let mut ident = match ident_opt {
            Some(i) => i,
            None => {
                // Only fall back for files that clearly are episodes; a loose
                // file with neither a year nor an episode marker is most
                // likely a movie.
                let title = if ep.is_some() { fallback_show_title(&sub) } else { None };
                let Some(title) = title else {
                    warnings.push(format!("could not parse (file left as-is): {fn_}"));
                    continue;
                };
                if warned_no_year.insert(title.clone()) {
                    warnings.push(format!(
                        "no year detected for '{title}' — filing under '{title}'; add the year for a reliable Jellyfin match"
                    ));
                }
                Identity { title, year: 0, edition: None }
            }
        };
        canonicalize(&mut canonical, &mut ident);

        let Some((ss, ee, ee2, title)) = ep else {
            let label = show_folder_name(&ident, true);
            match loose_specials.iter_mut().find(|(l, _, _)| l == &label) {
                Some((_, _, items)) => items.push((root.join(fn_), fn_.clone(), kind, ext)),
                None => loose_specials.push((label, ident.clone(), vec![(root.join(fn_), fn_.clone(), kind, ext)])),
            }
            continue;
        };
        let ss = ss.unwrap_or(1);
        ctx.plan_episode(&root.join(fn_), &ident, &title, (ss, ee, ee2), kind, &ext);
    }

    loose_specials.sort_by(|a, b| a.0.cmp(&b.0));
    for (label, ident, items) in &loose_specials {
        plan_specials(&mut ctx, &mut warnings, items, ident, label, bare);
    }

    let result = execute_moves(&ctx.moves, &root, args.apply, &[]);
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
