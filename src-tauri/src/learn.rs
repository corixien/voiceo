//! Learns word corrections: diff of dictated text vs. user-corrected text, applied to future output.
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Learned {
    pub from: String,
    pub to: String,
    pub count: u32,
}

pub fn load(path: &Path) -> Vec<Learned> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, items: &[Learned]) {
    if let Ok(json) = serde_json::to_string_pretty(items) {
        let _ = fs::write(path, json);
    }
}

/// Adds a correction (from -> to) or, with an empty `from`, a plain term the LLM should watch for.
pub fn add(items: &mut Vec<Learned>, from: &str, to: &str) {
    let (from, to) = (from.trim(), to.trim());
    if to.is_empty() || from == to {
        return;
    }
    if let Some(it) = items.iter_mut().find(|i| i.from == from && i.to == to) {
        it.count += 1;
        return;
    }
    if !from.is_empty() {
        if let Some(it) = items.iter_mut().find(|i| i.from == from) {
            it.to = to.to_string();
            it.count += 1;
            return;
        }
    }
    items.push(Learned { from: from.to_string(), to: to.to_string(), count: 1 });
}

fn tokens(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Small word-level replacements between `old` and `new`.
pub fn diff_pairs(old: &str, new: &str) -> Vec<(String, String)> {
    let a = tokens(old);
    let b = tokens(new);
    let (n, m) = (a.len(), b.len());
    if n == 0 || m == 0 {
        return vec![];
    }
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] { dp[i + 1][j + 1] + 1 } else { dp[i + 1][j].max(dp[i][j + 1]) };
        }
    }
    let mut pairs = vec![];
    let (mut i, mut j) = (0, 0);
    let (mut dels, mut ins): (Vec<String>, Vec<String>) = (vec![], vec![]);
    let flush = |dels: &mut Vec<String>, ins: &mut Vec<String>, pairs: &mut Vec<(String, String)>| {
        if !dels.is_empty() && !ins.is_empty() && dels.len() <= 3 && ins.len() <= 3 {
            pairs.push((dels.join(" "), ins.join(" ")));
        }
        dels.clear();
        ins.clear();
    };
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            flush(&mut dels, &mut ins, &mut pairs);
            i += 1;
            j += 1;
        } else if j < m && (i == n || dp[i][j + 1] >= dp[i + 1][j]) {
            ins.push(b[j].clone());
            j += 1;
        } else {
            dels.push(a[i].clone());
            i += 1;
        }
    }
    flush(&mut dels, &mut ins, &mut pairs);
    pairs
}

fn lc(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn replace_ci(text: &str, from: &str, to: &str) -> String {
    let t: Vec<char> = text.chars().collect();
    let f: Vec<char> = from.chars().collect();
    if f.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < t.len() {
        let end = i + f.len();
        let hit = end <= t.len()
            && (0..f.len()).all(|k| lc(t[i + k]) == lc(f[k]))
            && (i == 0 || !t[i - 1].is_alphanumeric())
            && (end == t.len() || !t[end].is_alphanumeric());
        if hit {
            let up = t[i].is_uppercase();
            let mut chars = to.chars();
            match chars.next() {
                Some(c) if up && c.is_lowercase() => {
                    out.extend(c.to_uppercase());
                    out.push_str(chars.as_str());
                }
                _ => out.push_str(to),
            }
            i = end;
        } else {
            out.push(t[i]);
            i += 1;
        }
    }
    out
}

pub fn apply(text: &str, items: &[Learned]) -> String {
    let mut sorted: Vec<&Learned> = items.iter().filter(|i| !i.from.is_empty()).collect();
    sorted.sort_by_key(|i| std::cmp::Reverse(i.from.chars().count()));
    let mut out = text.to_string();
    for it in sorted {
        out = replace_ci(&out, &it.from, &it.to);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learns_and_applies() {
        let p = diff_pairs("Bitte schreibe dies in clod.ai und danke", "Bitte schreibe dies in Claude.ai und danke");
        assert_eq!(p, vec![("clod.ai".to_string(), "Claude.ai".to_string())]);
        let items = vec![Learned { from: "clod.ai".into(), to: "Claude.ai".into(), count: 1 }];
        assert_eq!(apply("Gehe zu clod.ai, bitte", &items), "Gehe zu Claude.ai, bitte");
    }
}
