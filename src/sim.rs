// SPDX-License-Identifier: MIT
//! A hand-port of `difflib.SequenceMatcher(None, a, b).ratio()` (Ratcliff/Obershelp).

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

    #[allow(clippy::needless_range_loop)]
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
        let r = ratio("the office", "the office us");
        assert!((r - 0.8695652173913043).abs() < 1e-9, "got {r}");
    }

    #[test]
    fn unrelated_strings() {
        let r = ratio("breaking bad", "the sopranos");
        assert!((r - 0.25).abs() < 1e-9, "got {r}");
    }
}
