//! `acervo titles` — backfill missing TV episode titles (and premiere years)
//! from TVMaze into filenames already in the Jellyfin convention.
//!
//! Ported from `add-episode-titles.py`; see that script's module docstring
//! (preserved in `add-episode-titles/SKILL.md`) for the two supported
//! filename formats.

use crate::fsops::execute_moves;
use crate::naming::safe_component;
use crate::sim::ratio;
use crate::tokens::SUB_LANG_DOT_RE;
use crate::tvmaze::{Client, HttpClient, Show};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

const VIDEO_EXTS: &[&str] = &[".mkv", ".mp4", ".m4v", ".avi", ".mov", ".ts"];
const SUB_EXTS: &[&str] = &[".srt", ".ass", ".ssa", ".sub", ".vtt"];

static FILENAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?P<show>.+?) \((?P<year>\d{4})\)(?P<edition> \{edition-[^}]+\})? - [sS](?P<season>\d{1,2})[eE](?P<ep>\d{1,2})(?:-e(?P<ep2>\d{1,2}))?(?: - (?P<title>.+))?$",
    )
    .unwrap()
});

static FILENAME_NO_YEAR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<show>.+?) - [sS](?P<season>\d{1,2})[eE](?P<ep>\d{1,2})(?:-e(?P<ep2>\d{1,2}))?(?: - (?P<title>.+))?$")
        .unwrap()
});

pub struct Args {
    pub root: PathBuf,
    pub apply: bool,
    pub multi_ep_first: bool,
    pub threshold: f64,
    pub timeout: u64,
}

#[derive(Clone)]
struct ParsedFile {
    show: String,
    year: Option<i32>,
    edition: String,
    season: u32,
    ep: u32,
    ep2: Option<u32>,
    title: Option<String>,
}

fn ext_lower(filename: &str) -> String {
    crate::tokens::ext_of(filename)
}

fn normalize(text: &str) -> String {
    let mut out = String::new();
    let mut in_run = false;
    for c in text.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            in_run = false;
        } else if !in_run {
            out.push(' ');
            in_run = true;
        }
    }
    out.trim().to_string()
}

fn title_sim(a: &str, b: &str) -> f64 {
    ratio(&normalize(a), &normalize(b))
}

/// `(show, show's own name if not the best match else same, best score)`.
struct ResolveResult {
    show: Option<Show>,
    candidates: Vec<(String, String, f64)>,
}

fn resolve_show(client: &mut dyn Client, title: &str, year: i32, threshold: f64) -> ResolveResult {
    let results = client.search_shows(title);
    if results.is_empty() {
        return ResolveResult { show: None, candidates: Vec::new() };
    }
    let mut scored: Vec<(f64, f64, Show)> = Vec::new();
    for show in results {
        let sim = title_sim(title, show.name.as_deref().unwrap_or(""));
        let sy: Option<i32> = show.premiered.as_ref().and_then(|p| p.get(0..4)).and_then(|y| y.parse().ok());
        let year_ok = year == 0 || sy == Some(year);
        let score = sim + if year_ok { 0.25 } else { 0.0 };
        scored.push((score, sim, show));
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let candidates = scored
        .iter()
        .take(3)
        .map(|(_, sim, show)| {
            let name = show.name.clone().unwrap_or_else(|| "?".to_string());
            let year = show.premiered.as_ref().and_then(|p| p.get(0..4)).unwrap_or("None").to_string();
            (name, year, *sim)
        })
        .collect();
    let best = &scored[0];
    if best.1 < threshold {
        ResolveResult { show: None, candidates }
    } else {
        ResolveResult { show: Some(best.2.clone()), candidates }
    }
}

fn fetch_episodes(client: &mut dyn Client, show: &Show) -> HashMap<(u32, u32), String> {
    client.episodes(show.id).into_iter().map(|(s, n, name)| ((s, n), name)).collect()
}

fn new_stem(p: &ParsedFile, name: &str, resolved_year: Option<i32>) -> String {
    let mut tag = format!("s{:02}e{:02}", p.season, p.ep);
    if let Some(ep2) = p.ep2 {
        tag.push_str(&format!("-e{ep2:02}"));
    }
    let year = p.year.or(resolved_year).map(|y| y.to_string()).unwrap_or_default();
    format!("{} ({}){} - {} - {}", p.show, year, p.edition, tag, safe_component(name))
}

fn sub_sibling_names(dirpath: &Path, old_stem: &str, new_base: &str) -> Vec<(PathBuf, PathBuf)> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dirpath) else { return out };
    let mut names: Vec<String> = entries.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    names.sort();
    for fn_ in names {
        let ext = ext_lower(&fn_);
        if !SUB_EXTS.contains(&ext.as_str()) {
            continue;
        }
        let mut stem = crate::tokens::stem_of(&fn_).to_string();
        let mut lang = String::new();
        if let Some(m) = SUB_LANG_DOT_RE.find(&stem) {
            lang = m.as_str().to_string();
            stem.truncate(m.start());
        }
        if stem == old_stem {
            out.push((dirpath.join(&fn_), dirpath.join(format!("{new_base}{lang}{ext}"))));
        }
    }
    out
}

fn parse_file(stem: &str) -> Option<ParsedFile> {
    if let Some(caps) = FILENAME_RE.captures(stem) {
        return Some(ParsedFile {
            show: caps.name("show").unwrap().as_str().to_string(),
            year: caps.name("year").map(|m| m.as_str().parse().unwrap()),
            edition: caps.name("edition").map(|m| m.as_str().to_string()).unwrap_or_default(),
            season: caps.name("season").unwrap().as_str().parse().unwrap(),
            ep: caps.name("ep").unwrap().as_str().parse().unwrap(),
            ep2: caps.name("ep2").map(|m| m.as_str().parse().unwrap()),
            title: caps.name("title").map(|m| m.as_str().to_string()),
        });
    }
    None
}

fn parse_file_no_year(stem: &str) -> Option<ParsedFile> {
    let caps = FILENAME_NO_YEAR_RE.captures(stem)?;
    Some(ParsedFile {
        show: caps.name("show").unwrap().as_str().to_string(),
        year: None,
        edition: String::new(),
        season: caps.name("season").unwrap().as_str().parse().unwrap(),
        ep: caps.name("ep").unwrap().as_str().parse().unwrap(),
        ep2: caps.name("ep2").map(|m| m.as_str().parse().unwrap()),
        title: caps.name("title").map(|m| m.as_str().to_string()),
    })
}

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let mut client = HttpClient::new(args.timeout);
    run_with_client(args, &mut client)
}

/// Per-(show, year) TVMaze resolution cache: the matched show (if any) and
/// its episode-title lookup, keyed by (season, episode).
type ShowCache = HashMap<(String, i32), (Option<Show>, HashMap<(u32, u32), String>)>;

pub fn run_with_client(args: &Args, client: &mut dyn Client) -> anyhow::Result<i32> {
    let root = fs::canonicalize(&args.root).map_err(|_| anyhow::anyhow!("'{}' is not a directory", args.root.display()))?;
    if !root.is_dir() {
        eprintln!("Error: '{}' is not a directory", root.display());
        return Ok(1);
    }

    println!("root={}  mode={}  threshold={}", root.display(), if args.apply { "APPLY" } else { "DRY RUN" }, args.threshold);
    println!("{}", "-".repeat(70));

    // (show, year-or-0, edition-or-empty, needs_year) -> [(path, parsed)]
    type JobKey = (String, i32, String, bool);
    let mut job_order: Vec<JobKey> = Vec::new();
    let mut jobs: HashMap<JobKey, Vec<(PathBuf, ParsedFile)>> = HashMap::new();
    let mut already_titled = 0usize;

    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();
    files.sort();

    for path in &files {
        let fn_ = path.file_name().unwrap().to_string_lossy().to_string();
        let ext = ext_lower(&fn_);
        if !VIDEO_EXTS.contains(&ext.as_str()) {
            continue;
        }
        let stem = crate::tokens::stem_of(&fn_);
        if let Some(p) = parse_file(stem) {
            if p.title.is_some() {
                already_titled += 1;
                continue;
            }
            let key = (p.show.clone(), p.year.unwrap_or(0), p.edition.clone(), false);
            jobs.entry(key.clone()).or_insert_with(|| {
                job_order.push(key.clone());
                Vec::new()
            });
            jobs.get_mut(&key).unwrap().push((path.clone(), p));
            continue;
        }
        if let Some(p) = parse_file_no_year(stem) {
            let key = (p.show.clone(), 0, String::new(), true);
            jobs.entry(key.clone()).or_insert_with(|| {
                job_order.push(key.clone());
                Vec::new()
            });
            jobs.get_mut(&key).unwrap().push((path.clone(), p));
        }
    }

    if jobs.is_empty() {
        println!("no Jellyfin-formatted files needing titles or years found (already complete: {already_titled})");
        return Ok(0);
    }

    job_order.sort();

    let mut plans: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut dir_renames: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut no_show = 0usize;
    let mut no_episode = 0usize;
    let mut no_year = 0usize;
    let mut multi_skip = 0usize;
    let mut seen_shows: ShowCache = HashMap::new();

    for key in &job_order {
        let (show, year, _edition, needs_year) = key.clone();
        let items = &jobs[key];
        let cache_key = (show.to_lowercase(), year);
        if !seen_shows.contains_key(&cache_key) {
            if year != 0 {
                println!("\nresolving: {show} ({year})");
            } else {
                println!("\nresolving: {show} (no year)");
            }
            let result = resolve_show(client, &show, year, args.threshold);
            let (tv, eps) = match &result.show {
                None => {
                    println!("  ?? no reliable TVMaze match — {} file(s) skipped", items.len());
                    for (name, premiered, sim) in &result.candidates {
                        println!("     closest: {name} ({premiered})  similarity {sim:.2}");
                    }
                    if !result.candidates.is_empty() {
                        println!("     (lower --threshold below {} to accept one)", args.threshold);
                    }
                    (None, HashMap::new())
                }
                Some(tv) => {
                    let eps = fetch_episodes(client, tv);
                    let tv_year = tv.premiered.as_ref().and_then(|p| p.get(0..4)).unwrap_or("no year").to_string();
                    println!("  matched -> {} ({})  {} episode titles", tv.name.clone().unwrap_or_default(), tv_year, eps.len());
                    (Some(tv.clone()), eps)
                }
            };
            seen_shows.insert(cache_key.clone(), (tv, eps));
        }
        let (tv, eps) = seen_shows.get(&cache_key).unwrap();

        let Some(tv) = tv else {
            no_show += items.len();
            continue;
        };

        let mut resolved_year: Option<i32> = None;
        if needs_year {
            if let Some(premiered) = &tv.premiered {
                resolved_year = premiered.get(0..4).and_then(|y| y.parse().ok());
            }
        }

        if needs_year && resolved_year.is_none() {
            // The show matched but TVMaze carries no premiere date, so there
            // is no year to insert. Skip rather than write a literal "(None)".
            no_year += items.len();
            println!("  ?? no premiere year on TVMaze — {} file(s) skipped", items.len());
            continue;
        }

        for (path, p) in items {
            let rel = path.strip_prefix(&root).unwrap_or(path);
            if p.ep2.is_some() && !args.multi_ep_first {
                multi_skip += 1;
                println!("  -- multi-episode, skipping: {}", rel.display());
                continue;
            }
            let name = if let Some(title) = &p.title {
                title.clone()
            } else {
                match eps.get(&(p.season, p.ep)) {
                    Some(n) => n.clone(),
                    None => {
                        no_episode += 1;
                        println!("  ?? s{:02}e{:02} not found in TVMaze: {}", p.season, p.ep, rel.display());
                        continue;
                    }
                }
            };
            let base = new_stem(p, &name, resolved_year);
            let dst = path.parent().unwrap().join(format!("{base}{}", ext_lower(&path.file_name().unwrap().to_string_lossy())));
            plans.push((path.clone(), dst));
            let old_stem = crate::tokens::stem_of(&path.file_name().unwrap().to_string_lossy()).to_string();
            plans.extend(sub_sibling_names(path.parent().unwrap(), &old_stem, &base));

            if needs_year {
                if let Some(resolved_year) = resolved_year {
                    if let Some(show_dir) = path.parent().and_then(|p| p.parent()) {
                        let dir_name = show_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        let new_dir_name = format!("{show} ({resolved_year})");
                        if dir_name != new_dir_name {
                            let new_show_dir = show_dir.parent().unwrap().join(&new_dir_name);
                            let pair = (show_dir.to_path_buf(), new_show_dir);
                            if !dir_renames.contains(&pair) {
                                dir_renames.push(pair);
                            }
                        }
                    }
                }
            }
        }
    }

    println!("{}", "-".repeat(70));
    let result = execute_moves(&plans, &root, args.apply, &dir_renames);
    let skipped = result.skipped + multi_skip;

    println!("{}", "-".repeat(70));
    let mut summary = format!(
        "{}: {}   already titled: {}   skipped: {}   no show match: {}   no episode: {}   no year: {}",
        if args.apply { "renamed" } else { "planned" },
        result.done,
        already_titled,
        skipped,
        no_show,
        no_episode,
        no_year
    );
    if result.errors > 0 {
        summary.push_str(&format!("   errors: {}", result.errors));
    }
    println!("{summary}");
    if !args.apply {
        println!("DRY RUN — re-run with --apply to execute");
    }

    Ok(if result.errors > 0 { 1 } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    struct FakeClient {
        shows: Vec<Show>,
        episodes: HashMap<u64, Vec<(u32, u32, String)>>,
    }

    impl Client for FakeClient {
        fn search_shows(&mut self, _title: &str) -> Vec<Show> {
            self.shows.clone()
        }
        fn episodes(&mut self, show_id: u64) -> Vec<(u32, u32, String)> {
            self.episodes.get(&show_id).cloned().unwrap_or_default()
        }
    }

    fn show(id: u64, name: &str, premiered: Option<&str>) -> Show {
        Show { id, name: Some(name.to_string()), premiered: premiered.map(str::to_string) }
    }

    #[test]
    fn resolve_show_picks_year_matching_candidate() {
        let mut client = FakeClient {
            shows: vec![
                show(1, "The Office", Some("2005-03-24")),
                show(2, "The Office", Some("2001-07-09")), // UK original
            ],
            episodes: HashMap::new(),
        };
        let result = resolve_show(&mut client, "The Office", 2005, 0.75);
        assert_eq!(result.show.unwrap().id, 1);
    }

    #[test]
    fn resolve_show_reports_nothing_below_threshold() {
        let mut client = FakeClient { shows: vec![show(1, "Completely Different", None)], episodes: HashMap::new() };
        let result = resolve_show(&mut client, "Severance", 0, 0.75);
        assert!(result.show.is_none());
        assert_eq!(result.candidates.len(), 1);
    }

    #[test]
    fn new_stem_uses_resolved_year_when_filename_has_none() {
        let p =
            ParsedFile { show: "Severance".into(), year: None, edition: String::new(), season: 1, ep: 1, ep2: None, title: None };
        assert_eq!(new_stem(&p, "Good News About Hell", Some(2022)), "Severance (2022) - s01e01 - Good News About Hell");
    }

    #[test]
    fn new_stem_sanitizes_unsafe_title_characters() {
        let p = ParsedFile {
            show: "Show".into(),
            year: Some(2020),
            edition: String::new(),
            season: 1,
            ep: 2,
            ep2: None,
            title: None,
        };
        assert_eq!(new_stem(&p, "Hide and Seek: Part 1/2", None), "Show (2020) - s01e02 - Hide and Seek Part 1-2");
    }

    #[test]
    fn parses_year_and_no_year_filenames() {
        let p = parse_file("Show (2020) - s01e01").unwrap();
        assert_eq!((p.show.as_str(), p.year, p.season, p.ep, p.title), ("Show", Some(2020), 1, 1, None));

        let p = parse_file_no_year("Show - s01e01 - Some Title").unwrap();
        assert_eq!(p.show, "Show");
        assert_eq!(p.title.as_deref(), Some("Some Title"));
    }

    #[test]
    fn end_to_end_backfills_title_and_year_via_fake_client() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let show_dir = root.join("Severance/Season 01");
        fs::create_dir_all(&show_dir).unwrap();
        let video = show_dir.join("Severance - s01e01.mkv");
        fs::write(&video, b"x").unwrap();

        let mut client = FakeClient {
            shows: vec![show(1, "Severance", Some("2022-02-18"))],
            episodes: HashMap::from([(1, vec![(1, 1, "Good News About Hell".to_string())])]),
        };

        let args = Args { root: root.to_path_buf(), apply: true, multi_ep_first: false, threshold: 0.75, timeout: 15 };
        let code = run_with_client(&args, &mut client).unwrap();
        assert_eq!(code, 0);

        let expected = root.join("Severance (2022)/Season 01/Severance (2022) - s01e01 - Good News About Hell.mkv");
        assert!(expected.exists(), "expected {} to exist", expected.display());
        assert!(!video.exists());
    }
}
