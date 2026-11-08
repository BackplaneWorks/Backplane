//! Settings, Voice (pure): the rows every client draws, the desktop through the presenter and the phones through `voice.page`, from the `voice.*` settings and what the
//! hub last found out (`info`: `voice.key`, `voice.local`, `voice.devices`, `voice.job`; JSON texts). A button is a `setting key=value`, a `voice-op` for the hub or a
//! `voice-term`; the OpenAI key is a vault field saved as the API key named `KEY_NAME`, so it leaves the client the way every API key does.
use super::vdict::{self, Learned};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The name a Save of the OpenAI key goes by (`bots.voice` `api-add`): it is kept as dictation's key, not among the agents' API keys.
pub const KEY_NAME: &str = "@voice";
pub type Prefs = BTreeMap<String, String>;

fn btn(label: &str, action: &str, value: &str, on: bool) -> Value { json!({"label": label, "action": action, "value": value, "on": on}) }
fn row(label: &str, note: &str, buttons: Vec<Value>) -> Value { json!({"label": label, "note": note, "buttons": buttons}) }
fn set<'a>(p: &'a Prefs, k: &str, dflt: &'a str) -> &'a str { p.get(k).map_or(dflt, String::as_str) }
fn parsed(info: &Value, k: &str) -> Value { info[k].as_str().and_then(|t| serde_json::from_str(t).ok()).unwrap_or(Value::Null) }
fn mb(b: u64) -> u64 { b / 1_000_000 }

fn ready(l: &Value, m: &Value) -> bool { m["installed"] == true && l["runtime"] == true && l["vad"] == true }
fn progress(j: &Value) -> String {
    let what = j["what"].as_str().unwrap_or(""); let (done, total) = (j["done"].as_u64().unwrap_or(0), j["total"].as_u64().unwrap_or(0));
    if j["busy"].as_str().unwrap_or("").starts_with("remove:") { return "Removing the local model…".into() }
    if what.is_empty() { return "Starting download… You can leave Settings while it runs.".into() }
    if j["phase"] == "unpack" { return format!("Verifying and unpacking {what}…") }
    if total == 0 { return format!("Downloading {what} · {} MB received", mb(done)) }
    format!("Downloading {what} · {} / {} MB ({}%)", mb(done), mb(total), (done * 100 / total.max(1)).min(100))
}
fn model_rows(p: &Prefs, info: &Value) -> Value {
    let (l, j) = (parsed(info, "voice.local"), parsed(info, "voice.job"));
    let id = vdict::model_id(set(p, "voice.model", ""));
    let m = l["models"].as_array().and_then(|a| a.iter().find(|m| m["id"] == id)).cloned().unwrap_or(Value::Null);
    let busy = j["busy"].as_str().map_or(false, |b| !b.is_empty());
    let err = j["error"].as_str().filter(|e| !e.is_empty()).or(l["error"].as_str().filter(|e| !e.is_empty()));
    let (name, size) = (m["label"].as_str().unwrap_or(id), m["size"].as_u64().unwrap_or(0));
    let note = if busy { progress(&j) }
        else if let Some(e) = err { format!("Download failed: {e}") }
        else if l["supported"] == false { format!("Local dictation is not available on {}.", l["platform"].as_str().unwrap_or("this machine")) }
        else if m["size"].is_null() { "Checking the voice helper on this machine…".into() }
        else if ready(&l, &m) { format!("{name} is ready.") }
        else if m["installed"] == true { "The model needs additional files before it can run. Download to finish setup.".into() }
        else { format!("Not downloaded. About {} MB, plus the speech runtime. Download once for private, offline dictation.", mb(size)) };
    if busy { return row("Local model", &note, vec![]) }
    let mut b = vec![btn("English", "setting", "voice.model=parakeet-v2", id == "parakeet-v2"), btn("25 languages", "setting", "voice.model=parakeet-v3", id == "parakeet-v3")];
    if m["size"].is_null() || l["supported"] == false { b.push(btn("Check availability", "voice-op", "status", false)) }
    else if ready(&l, &m) { b.push(btn("Remove", "voice-op", &format!("remove:{id}"), false)) }
    else { b.push(btn(&if err.is_some() { "Retry download".to_string() } else { format!("Download {name} ({} MB)", mb(size)) }, "voice-op", &format!("install:{id}"), false)) }
    row("Local model", &note, b)
}
fn key_row(info: &Value, err: &str) -> Value {
    let key = info["voice.key"].as_str().unwrap_or("");
    let note = if !err.is_empty() { err.to_string() } else if key.is_empty() { "For GPT-Live-Transcribe. Kept in this machine's secrets; never shown again.".into() } else { format!("Saved: {key}. Paste a new one to replace it.") };
    let mut b = vec![btn("Save", "secret-save", &format!("@apikey\u{1f}{KEY_NAME}"), true)]; if !key.is_empty() { b.push(btn("Remove", "voice-op", "unkey", false)) }
    let mut r = json!({"label": "OpenAI API key", "note": note, "fields": [{"name": "kval", "hint": "sk-…", "secret": true, "vault": true, "value": ""}], "buttons": b});
    if !err.is_empty() { r["tone"] = json!("bad") }
    r
}
fn mic_row(p: &Prefs, info: &Value) -> Value {
    let d = parsed(info, "voice.devices"); let cur = set(p, "voice.device", ""); let def = d["default"].as_str().unwrap_or("");
    let list = d["devices"].as_array().cloned().unwrap_or_default();
    let label = |name: &str| list.iter().find(|x| x["name"] == name).and_then(|x| x["label"].as_str().filter(|l| !l.is_empty())).unwrap_or(name).to_string();
    let now = if cur.is_empty() { if def.is_empty() { "default microphone".to_string() } else { format!("default microphone ({})", label(def)) } } else { label(cur) };
    let mut b = vec![btn("Default", "setting", "voice.device=", cur.is_empty())];
    for x in &list { let n = x["name"].as_str().unwrap_or(""); b.push(btn(&label(n), "setting", &format!("voice.device={n}"), n == cur)) }
    row("Microphone", &format!("A window on the hub's own machine records its {now}. Phones and windows on other machines record their own."), b)
}
/// The Voice page's rows. `term`: the Add a term field as typed; `key_err`: why the last key was refused.
pub fn rows(p: &Prefs, info: &Value, term: &str, key_err: &str) -> Vec<Value> {
    let eng = set(p, "voice.engine", "auto");
    let entries: Vec<(&String, &String)> = p.iter().filter(|(k, v)| k.starts_with(vdict::E_PRE) && !v.is_empty()).collect();
    let learned: Vec<(&String, &String)> = p.iter().filter(|(k, v)| k.starts_with(vdict::L_PRE) && !v.is_empty()).collect();
    let mut dict: Vec<Value> = vec![btn("Add", "voice-term", "", true)];
    dict.extend(entries.iter().map(|(k, v)| btn(&format!("{v} ×"), "setting", &format!("{k}="), false)));
    let fixes: Vec<Value> = learned.iter().map(|(k, v)| { let x = Learned::parse(v); btn(&format!("{} → {} · {}× ×", &k[vdict::L_PRE.len()..], x.out, x.yes), "setting", &format!("{k}="), x.trusted()) }).collect();
    let local_ready = { let l = parsed(info, "voice.local"); let id = vdict::model_id(set(p, "voice.model", "")); let m = l["models"].as_array().and_then(|a| a.iter().find(|m| m["id"] == id)).cloned().unwrap_or(Value::Null); ready(&l, &m) };
    let eng_note = if eng == "local" && !local_ready { "On this machine is chosen: it works once the local model below is downloaded. Until then dictation does not start." }
        else { "Where your speech becomes text. Auto uses the local model once it is downloaded, else OpenAI." };
    vec![
        row("Speech to text", eng_note, vec![btn("Auto", "setting", "voice.engine=auto", eng == "auto"), btn("On this machine", "setting", "voice.engine=local", eng == "local"), btn("OpenAI", "setting", "voice.engine=cloud", eng == "cloud")]),
        model_rows(p, info), key_row(info, key_err), mic_row(p, info),
        json!({"label": "Dictionary", "note": if entries.is_empty() { "No dictionary entries yet. One a line. KiCad = key cad (a term and how it sounds), jason -> JSON (a replacement), !um (a word to drop)." } else { "One a line. KiCad = key cad (a term and how it sounds), jason -> JSON (a replacement), !um (a word to drop)." },
            "fields": [{"name": "vterm", "hint": "Add a term", "secret": false, "value": term}], "buttons": dict}),
        row("Learned fixes", if learned.is_empty() { "No fixes learned yet. After dictating, use Fix on a misheard word or accept a suggested correction." } else { "Fixes you made after dictating. Each applies by itself once you have made it twice." }, fixes),
        row("Pebble ring", "Set its webhook to send the recording; it is transcribed here with this dictionary.", vec![]),
    ]
}
/// What Add does: a setting for each line typed (blank lines and comments are nothing).
pub fn term_settings(text: &str) -> Vec<(String, String)> { text.lines().filter(|l| vdict::line_ok(l)).map(super::dictation::entry_kv).collect() }

#[cfg(test)]
mod tests {
    use super::*;
    fn prefs(kv: &[(&str, &str)]) -> Prefs { kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() }
    fn local(installed: bool) -> Value { json!({"voice.local": json!({"platform":"linux-x64","supported":true,"runtime":installed,"vad":installed,"models":[{"id":"parakeet-v2","label":"Parakeet v2","size":482468385u64,"installed":installed}]}).to_string()}) }
    fn find<'a>(rows: &'a [Value], label: &str) -> &'a Value { rows.iter().find(|r| r["label"] == label).unwrap() }
    #[test] fn on_this_machine_chosen_before_the_model_is_downloaded_says_so() {
        let local = |installed: bool| json!({"voice.local": json!({"supported": true, "runtime": installed, "vad": installed, "models": [{"id": "parakeet-v2", "label": "P", "size": 1000000, "installed": installed}]}).to_string()});
        let note = |p: &Prefs, i: &Value| find(&rows(p, i, "", ""), "Speech to text")["note"].as_str().unwrap().to_string();
        assert!(note(&prefs(&[("voice.engine", "local")]), &local(false)).contains("works once the local model below is downloaded"));
        assert!(!note(&prefs(&[("voice.engine", "local")]), &local(true)).contains("downloaded. Until"), "ready: the plain note");
        assert!(!note(&prefs(&[("voice.engine", "cloud")]), &local(false)).contains("Until then"), "OpenAI needs no download");
        assert_eq!(find(&rows(&prefs(&[("voice.engine", "local")]), &local(false), "", ""), "Speech to text")["buttons"][1]["on"], true, "and the chip stays lit");
    }
    fn labels(r: &Value) -> Vec<String> { r["buttons"].as_array().unwrap().iter().map(|b| b["label"].as_str().unwrap().to_string()).collect() }
    #[test] fn the_engine_chips_say_the_setting() {
        let r = rows(&prefs(&[("voice.engine", "cloud")]), &json!({}), "", ""); let b = &r[0]["buttons"];
        assert_eq!(b[2]["value"], "voice.engine=cloud"); assert_eq!(b[2]["on"], true); assert_eq!(b[0]["on"], false);
    }
    #[test] fn the_model_row_follows_what_is_installed_and_what_is_running() {
        let none = rows(&prefs(&[]), &json!({}), "", ""); assert_eq!(find(&none, "Local model")["note"], "Checking the voice helper on this machine…"); assert!(labels(find(&none, "Local model")).contains(&"Check availability".to_string()));
        let r = rows(&prefs(&[]), &local(false), "", ""); let m = find(&r, "Local model"); assert!(m["note"].as_str().unwrap().starts_with("Not downloaded. About 482 MB")); assert_eq!(m["buttons"][2]["value"], "install:parakeet-v2"); assert_eq!(m["buttons"][2]["label"], "Download Parakeet v2 (482 MB)");
        let r = rows(&prefs(&[]), &local(true), "", ""); let m = find(&r, "Local model"); assert_eq!(m["note"], "Parakeet v2 is ready."); assert_eq!(m["buttons"][2]["value"], "remove:parakeet-v2");
        let mut i = local(false); i["voice.job"] = json!(json!({"busy":"install:parakeet-v2","what":"Parakeet v2","done":241234192u64,"total":482468385u64,"phase":"download"}).to_string());
        let r = rows(&prefs(&[]), &i, "", ""); let m = find(&r, "Local model"); assert_eq!(m["note"], "Downloading Parakeet v2 · 241 / 482 MB (49%)"); assert!(labels(m).is_empty(), "nothing to press while it runs");
        i["voice.job"] = json!(json!({"error":"checksum mismatch"}).to_string()); let r = rows(&prefs(&[]), &i, "", ""); let m = find(&r, "Local model"); assert_eq!(m["note"], "Download failed: checksum mismatch"); assert_eq!(m["buttons"][2]["label"], "Retry download");
        let i = json!({"voice.local": json!({"platform":"freebsd-x64","supported":false,"models":[{"id":"parakeet-v2","label":"P","size":1,"installed":false}]}).to_string()}); let r = rows(&prefs(&[]), &i, "", ""); assert_eq!(find(&r, "Local model")["note"], "Local dictation is not available on freebsd-x64.");
    }
    #[test] fn the_key_is_masked_and_goes_as_a_vault_field() {
        let r = rows(&prefs(&[]), &json!({"voice.key":"sk-…WXYZ"}), "", ""); let k = find(&r, "OpenAI API key");
        assert_eq!(k["note"], "Saved: sk-…WXYZ. Paste a new one to replace it."); assert_eq!(k["fields"][0]["vault"], true); assert_eq!(k["fields"][0]["value"], ""); assert_eq!(k["buttons"][0]["value"], "@apikey\u{1f}@voice"); assert_eq!(k["buttons"][1]["value"], "unkey");
        let r = rows(&prefs(&[]), &json!({}), "", "that is not an OpenAI API key"); assert_eq!(find(&r, "OpenAI API key")["tone"], "bad");
    }
    #[test] fn microphones_are_chips_by_their_labels() {
        let i = json!({"voice.devices": json!({"default":"a","devices":[{"name":"a","label":"Desk"},{"name":"b","label":""}]}).to_string()});
        let r = rows(&prefs(&[]), &i, "", ""); let m = find(&r, "Microphone"); assert_eq!(labels(m), ["Default", "Desk", "b"]); assert_eq!(m["note"], "A window on the hub's own machine records its default microphone (Desk). Phones and windows on other machines record their own."); assert_eq!(m["buttons"][0]["on"], true);
        let r = rows(&prefs(&[("voice.device", "b")]), &i, "", ""); assert_eq!(find(&r, "Microphone")["buttons"][2]["on"], true);
    }
    #[test] fn entries_and_learned_fixes_can_each_be_taken_out() {
        let r = rows(&prefs(&[("voice.e.kicad", "KiCad = key cad"), ("voice.e.gone", ""), ("voice.l.jason", "JSON\t2\t0"), ("voice.l.bend", "Bend\t1\t0")]), &json!({}), "typed", "");
        let d = find(&r, "Dictionary"); assert_eq!(labels(d), ["Add", "KiCad = key cad ×"]); assert_eq!(d["buttons"][1]["value"], "voice.e.kicad="); assert_eq!(d["fields"][0]["value"], "typed");
        let l = find(&r, "Learned fixes"); assert_eq!(labels(l), ["bend → Bend · 1× ×", "jason → JSON · 2× ×"]); assert_eq!(l["buttons"][1]["on"], true, "lit once it applies by itself");
    }
    #[test] fn add_takes_many_lines_at_once() {
        assert_eq!(term_settings("KiCad = key cad\n\n# no\njason -> JSON\n!um\n"), vec![("voice.e.kicad".to_string(), "KiCad = key cad".to_string()), ("voice.e.jason>".to_string(), "jason -> JSON".to_string()), ("voice.e.!um".to_string(), "!um".to_string())]);
    }
}
