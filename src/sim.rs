//! A faithful port of `difflib.SequenceMatcher(None, a, b).ratio()`
//! (Ratcliff/Obershelp), used by `titles` to score TVMaze show candidates.
//!
//! `strsim` and friends implement different algorithms (Levenshtein, Jaro-
//! Winkler, ...) that would silently shift which shows clear `--threshold`,
//! so this is hand-ported rather than swapped for a crate. Python's
//! `autojunk` heuristic is omitted: it only engages for a sequence of 200+
//! characters, and every string here is a normalized show title — far short
//! of that, so the omission changes nothing for real input.

use std::collections::HashMap;

fn find_longest_match(
    a: &[char],
    _b: &[char],
    alo: usize,
    ahi: usize,
    blo: usize,
    bhi: usize,
    b2j: &HashMap<char, Vec<usize>>,
) -> (usize, usize, usize) {
    let mut besti = alo;
    let mut bestj = blo;
    let mut bestsize = 0usize;
    let mut j2len: HashMap<usize, usize> = HashMap::new();

    for i in alo..ahi {
        let mut newj2len: HashMap<usize, usize> = HashMap::new();
        if let Some(js) = b2j.get(&a[i]) {
            for &j in js {
                if j < blo {
                    continue;
                }
                if j >= bhi {
                    break;
                }
                let prev = j.checked_sub(1).and_then(|jm1| j2len.get(&jm1)).copied().unwrap_or(0);
                let k = prev + 1;
                newj2len.insert(j, k);
                if k > bestsize {
                    besti = i + 1 - k;
                    bestj = j + 1 - k;
                    bestsize = k;
                }
            }
        }
        j2len = newj2len;
    }
    (besti, bestj, bestsize)
}

fn matching_blocks(a: &[char], b: &[char]) -> Vec<(usize, usize, usize)> {
    let mut b2j: HashMap<char, Vec<usize>> = HashMap::new();
    for (j, &c) in b.iter().enumerate() {
        b2j.entry(c).or_default().push(j);
    }

    let mut queue = vec![(0usize, a.len(), 0usize, b.len())];
    let mut blocks = Vec::new();
    while let Some((alo, ahi, blo, bhi)) = queue.pop() {
        let (i, j, k) = find_longest_match(a, b, alo, ahi, blo, bhi, &b2j);
        if k > 0 {
            blocks.push((i, j, k));
            if alo < i && blo < j {
                queue.push((alo, i, blo, j));
            }
            if i + k < ahi && j + k < bhi {
                queue.push((i + k, ahi, j + k, bhi));
            }
        }
    }
    blocks
}

/// Similarity ratio in `[0.0, 1.0]`, `2*M / T` where `M` is the total length
/// of matching blocks and `T` is the combined length of both strings.
pub fn ratio(a: &str, b: &str) -> f64 {
    let av: Vec<char> = a.chars().collect();
    let bv: Vec<char> = b.chars().collect();
    let total = av.len() + bv.len();
    if total == 0 {
        return 1.0;
    }
    let matches: usize = matching_blocks(&av, &bv).iter().map(|&(_, _, k)| k).sum();
    2.0 * matches as f64 / total as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected values cross-checked against CPython's difflib.SequenceMatcher.
    #[test]
    fn identical_strings() {
        assert_eq!(ratio("severance", "severance"), 1.0);
    }

    #[test]
    fn both_empty() {
        assert_eq!(ratio("", ""), 1.0);
    }

    #[test]
    fn one_empty() {
        assert_eq!(ratio("severance", ""), 0.0);
    }

    #[test]
    fn known_pair() {
        // difflib.SequenceMatcher(None, "the office", "the office us").ratio()
        let r = ratio("the office", "the office us");
        assert!((r - 0.8695652173913043).abs() < 1e-9, "got {r}");
    }

    #[test]
    fn unrelated_strings() {
        // difflib.SequenceMatcher(None, "breaking bad", "the sopranos").ratio()
        let r = ratio("breaking bad", "the sopranos");
        assert!((r - 0.25).abs() < 1e-9, "got {r}");
    }
}
