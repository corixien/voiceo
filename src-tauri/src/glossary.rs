//! Bundled glossary of hard / technical terms (DE + EN). Finds likely misheard words by edit
//! distance and a rough phonetic skeleton; the LLM decides whether to use the suggestion.
use std::sync::OnceLock;

const CURATED: &str = include_str!("../data/curated.txt");
const AUTO: &str = include_str!("../data/auto.txt");

struct Term {
    text: String,
    key: String,
    skel: String,
}

fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// Rough consonant skeleton so "clod" and "Claude" collide.
fn skeleton(key: &str) -> String {
    let mut s = key.to_string();
    for (a, b) in [
        ("ph", "f"), ("ck", "k"), ("ß", "s"), ("ä", "e"), ("ö", "o"), ("ü", "u"), ("ae", "e"),
        ("oe", "o"), ("ue", "u"), ("ch", "k"), ("sch", "s"), ("th", "t"), ("c", "k"), ("q", "k"),
        ("v", "f"), ("w", "f"), ("z", "s"), ("y", "i"), ("x", "ks"),
    ] {
        s = s.replace(a, b);
    }
    let mut out = String::new();
    let mut prev = '\0';
    for (i, c) in s.chars().enumerate() {
        let vowel = "aeiou".contains(c);
        if vowel && i > 0 {
            continue;
        }
        if c != prev {
            out.push(c);
        }
        prev = c;
    }
    out
}

fn terms() -> &'static Vec<Term> {
    static T: OnceLock<Vec<Term>> = OnceLock::new();
    T.get_or_init(|| {
        let mut seen = std::collections::HashSet::new();
        CURATED
            .lines()
            .chain(AUTO.lines())
            .map(str::trim)
            .filter(|l| l.chars().count() >= 4)
            .filter_map(|l| {
                let key = norm(l);
                if key.chars().count() < 4 || !seen.insert(key.clone()) {
                    return None;
                }
                Some(Term { text: l.to_string(), skel: skeleton(&key), key })
            })
            .collect()
    })
}

fn edit_distance_le(a: &[char], b: &[char], max: usize) -> bool {
    if a.len().abs_diff(b.len()) > max {
        return false;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[b.len()] <= max
}

/// Returns (heard, suggested term) pairs for words in `text` that look like a glossary term but differ.
/// `extra` are user terms (dictionary / learned) matched with the same rules.
pub fn candidates(text: &str, extra: &[String]) -> Vec<(String, String)> {
    let words: Vec<&str> = text
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| !w.is_empty())
        .collect();
    let mut spans: Vec<String> = words.iter().map(|w| w.to_string()).collect();
    for p in words.windows(2) {
        spans.push(format!("{} {}", p[0], p[1]));
    }

    let user: Vec<Term> = extra
        .iter()
        .filter(|t| norm(t).chars().count() >= 3)
        .map(|t| {
            let key = norm(t);
            Term { text: t.clone(), skel: skeleton(&key), key }
        })
        .collect();

    let mut out: Vec<(String, String)> = vec![];
    for span in spans {
        let key = norm(&span);
        let kc: Vec<char> = key.chars().collect();
        if kc.len() < 4 {
            continue;
        }
        let skel = skeleton(&key);
        let max = if kc.len() >= 9 { 2 } else { 1 };
        let mut exact = false;
        let mut best: Option<&Term> = None;
        for t in terms().iter().chain(user.iter()) {
            if t.key == key {
                exact = true;
                break;
            }
            let tc: Vec<char> = t.key.chars().collect();
            let near = edit_distance_le(&kc, &tc, max);
            let phon = skel.chars().count() >= 3 && t.skel == skel && kc[0] == tc[0];
            if (near || phon) && best.is_none() {
                best = Some(t);
            }
        }
        if !exact {
            if let Some(t) = best {
                if !out.iter().any(|(_, s)| s == &t.text) {
                    out.push((span, t.text.clone()));
                }
            }
        }
        if out.len() >= 12 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_misheard_terms() {
        let c = candidates("Bitte schreibe dies in clod und starte den Kubernetis Cluster", &[]);
        let sug: Vec<&str> = c.iter().map(|(_, t)| t.as_str()).collect();
        assert!(sug.contains(&"Claude"), "{c:?}");
        assert!(sug.contains(&"Kubernetes"), "{c:?}");
    }

    #[test]
    fn exact_terms_are_left_alone() {
        let c = candidates("Wir nutzen Docker und PostgreSQL", &[]);
        assert!(c.is_empty(), "{c:?}");
    }
}
