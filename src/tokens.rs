// SPDX-License-Identifier: MIT
//! Shared regex tokens: release-junk detection, editions, language codes.

use regex::Regex;
use std::sync::LazyLock;

pub static ALREADY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<title>.+?) \((?P<year>\d{4})\)(?: \{edition-(?P<ed>[^}]+)\})?$").unwrap());

pub static QUALITY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:\d{3,4}p|4k|2160p|1080i|bluray|blu-ray|brrip|bdrip|bdremux|remux|\
web[- ]?dl|webrip|hdtv|hdrip|dvdrip|dvdscr|hdts|telesync|hdcam|\
x264|x265|h\s?264|h\s?265|hevc|avc|xvid|divx|\
aac|ac3|dd5|ddp5|ddp|dts|truehd|atmos|flac|mp3|opus|\
10bit|8bit|hdr10|hdr|sdr|yify|yts|vostfr)\b",
    )
    .unwrap()
});

pub static EDITION_PATTERNS: LazyLock<Vec<(Regex, Option<&'static str>)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"(?i)director'?s[ .]?cut").unwrap(), Some("Director's Cut")),
        (Regex::new(r"(?i)\bfinal[ .]?cut\b").unwrap(), Some("Final Cut")),
        (Regex::new(r"(?i)\bextended(?:[ .]?(?:cut|edition))?\b").unwrap(), Some("Extended")),
        (Regex::new(r"(?i)\bunrated\b").unwrap(), Some("Unrated")),
        (Regex::new(r"(?i)\btheatrical(?:[ .]?cut)?\b").unwrap(), Some("Theatrical")),
        (Regex::new(r"(?i)\bimax\b").unwrap(), Some("IMAX")),
        (Regex::new(r"(?i)\bremastered\b").unwrap(), None),
        (Regex::new(r"(?i)\brestored\b").unwrap(), None),
    ]
});

pub static NOISE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:proper|repack|internal|retail|limited|multi|complete|hybrid|\
open\s?matte|uncut|dubbed|subbed)\b",
    )
    .unwrap()
});

pub static YEAR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(?:19|20)\d{2}\b").unwrap());

pub const LANG_CODES: &str = concat!(
    "eng|english|en|por|portuguese|pt|spa|spanish|es|fre|french|fr|",
    "ger|german|de|ita|italian|it|jpn|japanese|ja|kor|korean|ko|",
    "chi|chinese|zh|nld|dutch|nl|swe|swedish|sv|nor|norwegian|no|",
    "dan|danish|da|fin|finnish|fi|pol|polish|pl|rus|russian|ru|",
    "tur|turkish|tr|ara|arabic|heb|hebrew|he",
);

pub static LANG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"(?i)[ ._-]({LANG_CODES})$")).unwrap());

pub static SUB_LANG_DOT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"(?i)\.(?:{LANG_CODES})$")).unwrap());

pub static PART_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:cd|dvd|disc|disk|part|pt)\s*0*([1-8])\b").unwrap());

pub static SPLIT_MARKER_TRAILING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s*-\s*(?:cd|dvd|disc|disk|part|pt)\s*0*[1-8]\s*$").unwrap());

pub fn ext_of(filename: &str) -> String {
    match filename.rfind('.') {
        Some(pos) if pos > 0 => filename[pos..].to_lowercase(),
        _ => String::new(),
    }
}

pub fn stem_of(filename: &str) -> &str {
    match filename.rfind('.') {
        Some(pos) if pos > 0 => &filename[..pos],
        _ => filename,
    }
}
