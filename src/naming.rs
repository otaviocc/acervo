// SPDX-License-Identifier: MIT
//! Path-component sanitizing and light title-casing, shared by all three subcommands.

use regex::Regex;
use std::sync::LazyLock;

static UNSAFE_DASH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[/\\|]").unwrap());
static UNSAFE_DROP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"[:?"*<>\x00-\x1f]"#).unwrap());
static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

pub static SMALL_WORDS: &[&str] =
    &["a", "an", "the", "of", "and", "or", "in", "on", "to", "for", "vs", "at", "by", "with", "from", "as", "but", "nor"];

pub fn safe_component(name: &str) -> String {
    let name = UNSAFE_DASH_RE.replace_all(name, "-");
    let name = UNSAFE_DROP_RE.replace_all(&name, "");
    let name = WHITESPACE_RE.replace_all(&name, " ");
    let trimmed = name.trim_matches(|c: char| c == ' ' || c == '.');
    if trimmed.is_empty() { "_".to_string() } else { trimmed.to_string() }
}

pub fn smart_title(text: &str) -> String {
    let all_caps = is_upper(text);
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut out = Vec::with_capacity(words.len());
    for (i, w) in words.iter().enumerate() {
        if !all_caps && w.chars().any(|c| c.is_uppercase()) {
            out.push(w.to_string());
        } else if i != 0 && SMALL_WORDS.contains(&w.to_lowercase().as_str()) {
            out.push(w.to_lowercase());
        } else {
            let mut chars = w.chars();
            let first = chars.next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
            let rest: String = chars.as_str().into();
            let rest = if all_caps { rest.to_lowercase() } else { rest };
            out.push(format!("{first}{rest}"));
        }
    }
    out.join(" ")
}

fn is_upper(text: &str) -> bool {
    let mut has_cased = false;
    for c in text.chars() {
        if c.is_lowercase() {
            return false;
        }
        if c.is_uppercase() {
            has_cased = true;
        }
    }
    has_cased
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_and_rewrites_unsafe_chars() {
        assert_eq!(safe_component("Part 1/2"), "Part 1-2");
        assert_eq!(safe_component(r#"Chapter 4: The Trial"#), "Chapter 4 The Trial");
        assert_eq!(safe_component("  spaced   out  "), "spaced out");
        assert_eq!(safe_component("..."), "_");
        assert_eq!(safe_component(""), "_");
    }

    #[test]
    fn title_cases_all_caps() {
        assert_eq!(smart_title("THE DARK KNIGHT"), "The Dark Knight");
        assert_eq!(smart_title("john wick"), "John Wick");
    }

    #[test]
    fn leaves_mixed_case_words_alone() {
        assert_eq!(smart_title("john McClane story"), "John McClane Story");
    }

    #[test]
    fn lowercases_small_words_except_first() {
        assert_eq!(smart_title("the lord of the rings"), "The Lord of the Rings");
    }
}
