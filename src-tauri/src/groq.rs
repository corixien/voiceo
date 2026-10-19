use crate::config::Settings;
use reqwest::blocking::{multipart, Client};
use serde_json::{json, Value};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const BASE: &str = "https://api.groq.com/openai/v1";

fn api_error(resp: reqwest::blocking::Response) -> String {
    let status = resp.status();
    let body: Value = resp.json().unwrap_or(Value::Null);
    let msg = body["error"]["message"].as_str().unwrap_or("unbekannter Fehler");
    match status.as_u16() {
        401 => "Groq: API-Key ungueltig".into(),
        429 => format!("Groq: Rate-Limit erreicht, spaeter erneut ({msg})"),
        s => format!("Groq {s}: {msg}"),
    }
}

/// Receives short status texts while a request is being retried (shown in the overlay).
pub static NOTIFY: OnceLock<Box<dyn Fn(&str) + Send + Sync>> = OnceLock::new();
const RETRY_BUDGET: Duration = Duration::from_secs(80);
const RETRY_MAX: u32 = 9;

fn notify(msg: &str) {
    if let Some(f) = NOTIFY.get() {
        f(msg);
    }
}

fn retry_after(resp: &reqwest::blocking::Response, attempt: u32) -> Duration {
    let hdr = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<f64>().ok());
    let secs = hdr.unwrap_or_else(|| (1u64 << attempt.min(4)) as f64);
    Duration::from_secs_f64(secs.clamp(1.5, 20.0) + 0.5)
}

/// Sends a request, retrying on rate limits (429), server errors and network problems.
/// `build` is called once per attempt (multipart bodies cannot be reused).
fn send_retry(
    mut build: impl FnMut() -> reqwest::blocking::RequestBuilder,
) -> Result<reqwest::blocking::Response, String> {
    let start = Instant::now();
    let mut attempt = 0u32;
    loop {
        match build().send() {
            Ok(r) => {
                let st = r.status().as_u16();
                if (st == 429 || st >= 500) && attempt < RETRY_MAX && start.elapsed() < RETRY_BUDGET {
                    let wait = retry_after(&r, attempt);
                    notify(if st == 429 { "Groq ausgelastet, versuche erneut …" } else { "Groq antwortet nicht, versuche erneut …" });
                    std::thread::sleep(wait);
                    attempt += 1;
                    continue;
                }
                return Ok(r);
            }
            Err(e) => {
                if attempt < RETRY_MAX && start.elapsed() < RETRY_BUDGET {
                    notify("Keine Verbindung, versuche erneut …");
                    std::thread::sleep(Duration::from_secs((1u64 << attempt.min(3)).min(8)));
                    attempt += 1;
                    continue;
                }
                return Err(format!("Netzwerk: {e}"));
            }
        }
    }
}

/// True for errors that are worth retrying later with the same audio.
pub fn is_transient(err: &str) -> bool {
    err.contains("Rate-Limit") || err.contains("Netzwerk") || err.contains("Groq 5")
}

pub fn client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(8))
        .build()
        .expect("http client")
}

pub fn check_key(http: &Client, key: &str) -> Result<usize, String> {
    let resp = http
        .get(format!("{BASE}/models"))
        .bearer_auth(key)
        .send()
        .map_err(|e| format!("Netzwerk: {e}"))?;
    if !resp.status().is_success() {
        return Err(api_error(resp));
    }
    let v: Value = resp.json().map_err(|e| e.to_string())?;
    Ok(v["data"].as_array().map(|a| a.len()).unwrap_or(0))
}

/// Neutral, informal, bilingual style sample. No questions or greetings: on near-silence Whisper
/// tends to echo the prompt, and `is_prompt_echo` drops such output.
pub const PRIMER: &str = "Ja, also ich hab das gestern mal ausprobiert, und es funktioniert eigentlich ganz gut. Danach muss ich noch das Deployment checken, weil die Pipeline manchmal hängt. Okay, passt.";

/// Default tech/office vocabulary for the Whisper prompt (measured: roughly halves WER on loanwords).
const DEFAULT_VOCAB: &[&str] = &[
    "Meeting", "Call", "Deadline", "Ticket", "Sprint", "Review", "Pull Request", "Commit", "Merge", "Branch",
    "Feature Flag", "Deployment", "Pipeline", "Build", "Release", "Bug", "Fix", "Backend", "Frontend", "API",
    "JSON", "Server", "Cache", "Config", "Docker", "Kubernetes", "GitHub",
];

pub fn is_prompt_echo(text: &str) -> bool {
    let norm = |s: &str| -> Vec<String> {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 2)
            .map(String::from)
            .collect()
    };
    let t = norm(text);
    if t.is_empty() {
        return false;
    }
    let p = norm(PRIMER);
    let hits = t.iter().filter(|w| p.contains(w)).count();
    t.len() >= 3 && hits * 10 >= t.len() * 7
}

/// Whisper prompt: vocabulary hint plus a punctuated bilingual primer (steers punctuation style).
/// Whisper only reads the last ~224 tokens, so the primer goes last.
fn whisper_prompt(s: &Settings, terms: &[String]) -> String {
    let mut vocab: Vec<String> = s.dictionary.clone();
    vocab.extend(terms.iter().cloned());
    vocab.extend(DEFAULT_VOCAB.iter().map(|t| t.to_string()));
    let mut seen = std::collections::HashSet::new();
    vocab.retain(|t| seen.insert(t.to_lowercase()));
    let mut p = String::new();
    if !vocab.is_empty() {
        p.push_str("Glossar: ");
        let mut list = vocab.join(", ");
        if list.chars().count() > 520 {
            list = list.chars().take(520).collect();
        }
        p.push_str(&list);
        p.push_str(". ");
    }
    if !s.whisper_prompt.trim().is_empty() {
        p.push_str(s.whisper_prompt.trim());
        p.push(' ');
    }
    p.push_str(PRIMER);
    p
}

pub fn transcribe(http: &Client, s: &Settings, wav: Vec<u8>, terms: &[String]) -> Result<String, String> {
    let (text, lang) = transcribe_once(http, s, wav.clone(), terms, None)?;
    // auto mode: Whisper sometimes mis-detects the language on short or casual speech.
    // Only German/English are expected, otherwise redo it with German pinned.
    if s.language == "auto" {
        let l = lang.to_lowercase();
        if !l.is_empty() && !l.contains("german") && !l.contains("english") && l != "de" && l != "en" {
            return transcribe_once(http, s, wav, terms, Some("de")).map(|r| r.0);
        }
    }
    Ok(text)
}

fn transcribe_once(
    http: &Client,
    s: &Settings,
    wav: Vec<u8>,
    terms: &[String],
    force_lang: Option<&str>,
) -> Result<(String, String), String> {
    let prompt = whisper_prompt(s, terms);
    let resp = send_retry(|| {
        let part = multipart::Part::bytes(wav.clone())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .expect("mime");
        let mut form = multipart::Form::new()
            .part("file", part)
            .text("model", s.stt_model.clone())
            .text("temperature", s.stt_temperature.to_string())
            .text("response_format", "verbose_json")
            .text("prompt", prompt.clone());
        if let Some(l) = force_lang {
            form = form.text("language", l.to_string());
        } else if s.language != "auto" && !s.language.is_empty() {
            form = form.text("language", s.language.clone());
        }
        http.post(format!("{BASE}/audio/transcriptions")).bearer_auth(&s.api_key).multipart(form)
    })?;
    if !resp.status().is_success() {
        return Err(api_error(resp));
    }
    let v: Value = resp.json().map_err(|e| e.to_string())?;
    // drop segments Whisper itself flags as non-speech / hallucinated
    if let Some(segs) = v["segments"].as_array() {
        let kept: Vec<&str> = segs
            .iter()
            .filter(|sg| {
                let nsp = sg["no_speech_prob"].as_f64().unwrap_or(0.0);
                let lp = sg["avg_logprob"].as_f64().unwrap_or(0.0);
                let cr = sg["compression_ratio"].as_f64().unwrap_or(1.0);
                !((nsp > 0.5 && lp < -0.8) || nsp > 0.85 || lp < -1.3 || cr > 2.4)
            })
            .filter_map(|sg| sg["text"].as_str())
            .map(str::trim)
            .collect();
        let lang = v["language"].as_str().unwrap_or("").to_string();
        return Ok((kept.join(" ").trim().to_string(), lang));
    }
    Ok((v["text"].as_str().unwrap_or("").trim().to_string(), v["language"].as_str().unwrap_or("").to_string()))
}

const BASE_PROMPT: &str = include_str!("../data/prompt_base.txt");

fn mode_prompt(s: &Settings) -> Option<String> {
    let extra = match s.mode.as_str() {
        "raw" => return None,
        "clean" => "Style: keep the original wording and tone as close as possible. Only clean up.",
        "message" => "Style: casual chat message. Short sentences, natural tone, no greeting or sign-off unless spoken.",
        "email" => "Style: professional email body. Polite, clear, structured in short paragraphs. Add a greeting or closing only if the speaker implied one. Keep it in the speaker's language.",
        "prompt" => "Style: rewrite as a clear, precise instruction for an AI assistant or developer. Imperative, unambiguous, keep technical terms, file names and identifiers exactly. No pleasantries.",
        "notes" => "Style: concise bullet-point notes. One idea per bullet, start each bullet with '- '.",
        "custom" => s.custom_prompt.trim(),
        _ => "Style: keep the original wording.",
    };
    let mut p = format!("{BASE_PROMPT}\n\n{extra}");
    if !s.dictionary.is_empty() {
        p.push_str(&format!(
            "\n\nPreferred spellings (use exactly): {}.",
            s.dictionary.join(", ")
        ));
    }
    Some(p)
}

const FALLBACK_MODELS: &[&str] = &["openai/gpt-oss-120b", "qwen/qwen3.8-27b", "openai/gpt-oss-20b"];

fn chat(http: &Client, s: &Settings, model: &str, system: &str, text: &str) -> Result<Value, (u16, String)> {
    let mut body = json!({
        "model": model,
        "temperature": s.llm_temperature,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": format!("<transcript>\n{text}\n</transcript>")}
        ]
    });
    if model.starts_with("openai/gpt-oss") {
        body["reasoning_effort"] = json!("low");
    }
    let resp = send_retry(|| http.post(format!("{BASE}/chat/completions")).bearer_auth(&s.api_key).json(&body))
        .map_err(|e| (0, e))?;
    if resp.status().is_success() {
        return resp.json().map_err(|e| (0, e.to_string()));
    }
    let code = resp.status().as_u16();
    Err((code, api_error(resp)))
}

/// Returns None when cleanup is disabled for this mode or the result is unusable.
/// `hints` = (heard, suggested term) pairs from the glossary.
pub fn cleanup(
    http: &Client,
    s: &Settings,
    text: &str,
    hints: &[(String, String)],
    terms: &[String],
) -> Result<Option<String>, String> {
    let Some(mut system) = mode_prompt(s) else {
        return Ok(None);
    };
    if !terms.is_empty() {
        let list: Vec<&str> = terms.iter().take(80).map(String::as_str).collect();
        system.push_str(&format!("\nKnown names/terms (exact spelling): {}.", list.join(", ")));
    }
    if !hints.is_empty() {
        let list: Vec<String> = hints.iter().map(|(h, t)| format!("\"{h}\" may be \"{t}\"")).collect();
        system.push_str(&format!(
            "\nPossibly misheard terms (apply ONLY if it clearly fits the context): {}.",
            list.join("; ")
        ));
    }

    let mut models: Vec<&str> = vec![s.llm_model.as_str()];
    models.extend(FALLBACK_MODELS.iter().copied().filter(|m| *m != s.llm_model));
    let mut last_err = String::new();
    let mut result = None;
    for m in models {
        match chat(http, s, m, &system, text) {
            Ok(v) => {
                result = Some(v);
                break;
            }
            Err((404, e)) | Err((400, e)) => last_err = e, // model gone / unsupported: try next
            Err((_, e)) => return Err(e),
        }
    }
    let v = result.ok_or(last_err)?;
    let mut out = v["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    if let (Some(a), Some(b)) = (out.find("<think>"), out.find("</think>")) {
        if a < b {
            out.replace_range(a..b + 8, "");
        }
    }
    for tag in ["<transcript>", "</transcript>"] {
        out = out.replace(tag, "");
    }
    let out = out.trim().to_string();
    // Sanity: fall back to raw when the model rambled or returned nothing.
    if out.is_empty() || out.chars().count() > text.chars().count() * 3 + 80 {
        return Ok(None);
    }
    Ok(Some(out))
}

/// Asks the LLM which systematic recognition fixes the user made when editing a dictation.
/// Returns (wrong, right) pairs.
pub fn extract_learning(
    http: &Client,
    s: &Settings,
    original: &str,
    corrected: &str,
) -> Result<Vec<(String, String)>, String> {
    let system = "You compare a dictated text (original, produced by speech recognition) with the \
user's final edited version. Return a JSON object {\"entries\":[{\"from\":\"...\",\"to\":\"...\"}]}.\n\
Include ONLY systematic recognition fixes the user made: misrecognized names, brand names, technical \
terms or specific words. \"from\" is the wrongly written form, \"to\" the correct form (max 3 words each, \
exact spelling and capitalization of the final text).\n\
Ignore: rewording, added or removed content, grammar or style changes, punctuation, one-off typos. \
At most 8 entries. If nothing qualifies return {\"entries\":[]}.";
    let user = format!("ORIGINAL:\n{original}\n\nFINAL:\n{corrected}");
    let mut body = json!({
        "model": s.llm_model,
        "temperature": 0,
        "response_format": {"type": "json_object"},
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ]
    });
    if s.llm_model.starts_with("openai/gpt-oss") {
        body["reasoning_effort"] = json!("low");
    }
    let resp = send_retry(|| http.post(format!("{BASE}/chat/completions")).bearer_auth(&s.api_key).json(&body))?;
    if !resp.status().is_success() {
        return Err(api_error(resp));
    }
    let v: Value = resp.json().map_err(|e| e.to_string())?;
    let content = v["choices"][0]["message"]["content"].as_str().unwrap_or("{}");
    let start = content.find('{').unwrap_or(0);
    let end = content.rfind('}').map(|i| i + 1).unwrap_or(content.len());
    let parsed: Value = serde_json::from_str(&content[start..end]).map_err(|e| e.to_string())?;
    let mut out = vec![];
    for e in parsed["entries"].as_array().cloned().unwrap_or_default() {
        let from = e["from"].as_str().unwrap_or("").trim().to_string();
        let to = e["to"].as_str().unwrap_or("").trim().to_string();
        if !from.is_empty() && !to.is_empty() && from != to && from.split_whitespace().count() <= 3 {
            out.push((from, to));
        }
    }
    Ok(out)
}
