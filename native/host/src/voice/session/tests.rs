//! The session against a stand-in helper (a shell script that speaks the helper's NDJSON): no microphone, no network, no model.
use super::*;
use crate::{hub::{Cfg, Out}, presenter::Canned};
use std::{os::unix::fs::PermissionsExt, sync::OnceLock};

const HELPER: &str = r#"#!/bin/sh
here=$(dirname "$0")
case "$1" in
devices) echo '{"event":"devices","default":"m","devices":[{"name":"m","label":"Desk mic"}]}'; echo '{"event":"end"}';;
local-status) echo '{"event":"local","platform":"linux-x64","supported":true,"runtime":false,"vad":false,"models":[{"id":"parakeet-v2","label":"Parakeet v2","size":1000000,"installed":false}]}'; echo '{"event":"end"}';;
local-install)
  echo '{"event":"progress","what":"Parakeet v2","done":500000,"total":1000000,"phase":"download"}'
  if [ -e "$here/fail-install" ]; then echo '{"event":"error","error":"checksum mismatch"}'; echo '{"event":"end"}'; exit 1; fi
  echo '{"event":"end"}';;
file)
  echo "$*" > "$here/file-args"; cp "$2" "$here/form"
  echo '{"event":"ready"}'; echo '{"event":"done","item":"file","text":"Open key cad now."}'; echo '{"event":"end"}';;
live)
  echo "$*" > "$here/args"
  echo '{"event":"ready","recorder":"pw-record","device":"m","label":"Desk mic"}'
  echo '{"event":"level","v":40}'
  echo '{"event":"delta","item":"a","text":"Open key"}'
  echo '{"event":"delta","item":"a","text":" cad"}'
  cat > "$here/audio"
  echo '{"event":"stopping"}'
  echo '{"event":"done","item":"a","text":"Open key cad."}'
  echo '{"event":"end"}';;
esac
"#;
/// One folder for the whole test run: `BACKPLANE_VOICE_BIN` is read by every session test.
fn bin() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let d = std::env::temp_dir().join(format!("bp-voice-helper-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap();
        let f = d.join("backplane-voice"); std::fs::write(&f, HELPER).unwrap(); std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("BACKPLANE_VOICE_BIN", &f); d
    })
}
/// The tests share the helper's folder: one at a time.
static ONE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
struct Rig { hub: Arc<Hub>, v: Arc<Voice>, out: crate::hub::Outbox, home: PathBuf }
impl Drop for Rig { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.home); } }
async fn rig(tag: &str) -> Rig {
    bin();
    let home = std::env::temp_dir().join(format!("bp-voice-{tag}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
    let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let out = hub.attach(1); let v = start(&hub);
    Rig { hub, v, out, home }
}
impl Rig {
    async fn rpc(&self, p: Value) -> Value { let h = self.hub.rpc_for("bots.voice").unwrap(); h.call(&self.hub, &Caller { gen: "1".into(), client: 1 }, &json!(7), "bots.voice", &p).await.remove(0) }
    fn key(&self) { std::fs::create_dir_all(self.home.join("secrets")).unwrap(); std::fs::write(key_path(&self.home), "sk-abcdefghijklmnopqrstuvwxyz\n").unwrap(); }
    /// Frames until `done` says so, at most 10 s.
    async fn until(&mut self, mut done: impl FnMut(&Value) -> bool) -> Vec<Value> {
        let mut all = vec![];
        loop {
            let o = tokio::time::timeout(Duration::from_secs(10), self.out.recv()).await.unwrap_or_else(|_| panic!("no end: {all:?}"));
            if let Some(Out::Value(v)) = o { if v["t"] == "voice" { let stop = done(&v); all.push(v); if stop { return all } } }
        }
    }
}
fn off(v: &Value) -> bool { v["ev"] == "view" && v["view"]["on"] == false }
fn replay(fs: &[Value], draft: &str) -> String {
    let (mut t, mut m) = (draft.to_string(), words::len(draft));
    for f in fs { if f["ev"] == "edit" { let e = crate::voice::place::Edit { del: f["del"].as_u64().unwrap() as usize, ins: f["ins"].as_str().unwrap().into(), adv: f["adv"].as_u64().unwrap() as usize, to: f["to"].as_u64().unwrap() as usize }; (t, m, _) = crate::voice::place::apply(&t, m, &e); } }
    t
}

#[tokio::test(flavor = "multi_thread")] async fn no_engine_says_what_to_set_up_and_starts_nothing() {
    let _g = ONE.lock().await; let mut r = rig("none").await;
    let a = r.rpc(json!({"op":"start","thread":"t1","draft":"","src":"hub"})).await; assert_eq!(a["ok"], true); assert_eq!(a["path"], "");
    let fs = r.until(off).await; assert!(fs.last().unwrap()["view"]["note"].as_str().unwrap().contains("set up dictation in Settings, Voice"));
}
#[tokio::test(flavor = "multi_thread")] async fn a_session_writes_corrected_words_and_ends_when_stopped() {
    let _g = ONE.lock().await; let mut r = rig("live").await; r.key();
    commit(&r.hub, &[("voice.e.kicad".to_string(), "KiCad = key cad".to_string())]).await.unwrap();
    let a = r.rpc(json!({"op":"start","thread":"t1","draft":"see","caret":3,"src":"hub"})).await; assert_eq!(a["path"], "");
    let fs = r.until(|v| v["ev"] == "edit").await;
    assert_eq!(fs.iter().find(|f| f["ev"] == "view").unwrap()["view"]["state"], "starting");
    let again = r.rpc(json!({"op":"start","thread":"t2","draft":""})).await; assert_eq!(again["ok"], true);
    assert_eq!(r.rpc(json!({"op":"view","thread":"t2"})).await["view"]["note"], "Dictation: already listening: stop the other dictation first");
    let mut all = fs; assert_eq!(r.rpc(json!({"op":"stop"})).await["ok"], true);
    all.extend(r.until(|v| off(v) && v["thread"] == "t1").await);
    assert_eq!(replay(&all, "see"), "see open KiCad.");
    let last = all.last().unwrap(); assert_eq!(last["view"]["fixes"][0]["label"], "KiCad ← key cad ↶"); assert_eq!(last["view"]["bad"], false);
    let args = std::fs::read_to_string(bin().join("args")).unwrap(); assert!(args.contains("--key-file") && args.contains("--keywords-file") && !args.contains("sk-"), "{args}");
    assert_eq!(std::fs::read_to_string(keywords_path(&r.home)).unwrap(), "KiCad");
}
#[tokio::test(flavor = "multi_thread")] async fn a_clients_audio_goes_to_its_session_and_only_its_key_opens_it() {
    let _g = ONE.lock().await; let mut r = rig("up").await; r.key();
    let a = r.rpc(json!({"op":"start","thread":"t1","draft":"","src":"client"})).await; let path = a["path"].as_str().unwrap().to_string(); assert!(path.starts_with("/voice/in?s="), "{a}");
    let key = path.split('=').nth(1).unwrap().to_string();
    r.until(|v| v["ev"] == "edit").await;
    assert!(r.v.sink("").is_none() && r.v.sink("wrong").is_none(), "no key, no way in");
    let (id, tx) = r.v.sink(&key).unwrap(); tx.unwrap().send(b"pcm-bytes".to_vec()).await.unwrap(); r.v.close_audio(id);
    r.until(off).await;
    assert_eq!(std::fs::read(bin().join("audio")).unwrap(), b"pcm-bytes"); assert!(std::fs::read_to_string(bin().join("args")).unwrap().contains("--source stdin --rate 16000"));
    assert!(r.v.sink(&key).is_none(), "the session is gone, and its key with it");
}
#[tokio::test(flavor = "multi_thread")] async fn a_client_that_goes_away_stops_the_microphone() {
    let _g = ONE.lock().await; let mut r = rig("gone").await; r.key();
    r.rpc(json!({"op":"start","thread":"t1","draft":"","src":"hub"})).await; r.until(|v| v["ev"] == "edit").await;
    r.hub.detach(1);
    for _ in 0..100 { if r.v.st.lock().unwrap().live.is_none() { return } tokio::time::sleep(Duration::from_millis(100)).await }
    panic!("the session outlived its client");
}
#[tokio::test(flavor = "multi_thread")] async fn the_key_is_checked_kept_masked_and_never_answered() {
    let _g = ONE.lock().await; let r = rig("key").await;
    assert_eq!(r.rpc(json!({"op":"api-add","name":"@voice","value":"not a key"})).await["error"], "that is not an OpenAI API key (they start with sk-)");
    let a = r.rpc(json!({"op":"api-add","name":"@voice","value":"sk-abcdefghijklmnopqrstuvwxyz"})).await; assert_eq!(a["voiceKey"], "sk-…wxyz"); assert!(!a.to_string().contains("sk-abc"));
    assert_eq!(std::fs::metadata(key_path(&r.home)).unwrap().permissions().mode() & 0o777, 0o600);
    assert!(crate::settings::keys::names_kept(&r.home).is_empty(), "dictation's key is not one of the agents' API keys");
    assert_eq!(r.hub.info_now()["voice.key"], "sk-…wxyz");
    assert_eq!(r.rpc(json!({"op":"unkey"})).await["ok"], true); assert!(!key_path(&r.home).exists()); assert_eq!(r.hub.info_now().get("voice.key"), None);
    assert_eq!(r.rpc(json!({"op":"api-add","name":"OTHER_KEY","value":"v"})).await["apiKeys"], "OTHER_KEY", "the other API keys still answer here");
    assert_eq!(r.rpc(json!({"op":"nonsense"})).await["error"], "unknown voice request");
}
#[tokio::test(flavor = "multi_thread")] async fn status_reads_the_helper_and_the_page_shows_it() {
    let _g = ONE.lock().await; let r = rig("status").await;
    let a = r.rpc(json!({"op":"status"})).await; assert_eq!(a["local"]["platform"], "linux-x64"); assert!(a["voiceDevices"].as_str().unwrap().contains("Desk mic"));
    let p = r.rpc(json!({"op":"page"})).await; let rows = p["rows"].as_array().unwrap();
    assert_eq!(rows[1]["buttons"][2]["value"], "install:parakeet-v2"); assert_eq!(rows[3]["buttons"][1]["label"], "Desk mic");
}
#[tokio::test(flavor = "multi_thread")] async fn a_download_reports_its_progress_and_a_failure_stays_on_the_page() {
    let _g = ONE.lock().await; let mut r = rig("job").await;
    assert_eq!(r.rpc(json!({"op":"page-act","action":"voice-op","value":"install:parakeet-v2"})).await["ok"], true);
    let fs = r.until(|v| v["ev"] == "page" && v["rows"][1]["note"].as_str().map_or(false, |n| n.contains("Not downloaded"))).await;
    assert!(fs.iter().any(|f| f["rows"][1]["note"] == "Downloading Parakeet v2 · 0 / 1 MB (50%)"), "{fs:?}");
    std::fs::write(bin().join("fail-install"), "").unwrap();
    r.rpc(json!({"op":"install","model":"parakeet-v2"})).await;
    let fs = r.until(|v| v["ev"] == "page" && v["rows"][1]["note"] == "Download failed: checksum mismatch").await; let _ = std::fs::remove_file(bin().join("fail-install"));
    assert_eq!(fs.last().unwrap()["rows"][1]["buttons"][2]["label"], "Retry download");
}
#[tokio::test(flavor = "multi_thread")] async fn chips_change_the_draft_and_the_dictionary() {
    let _g = ONE.lock().await; let mut r = rig("act").await;
    r.v.with("t1", |d| d.unsure = vec![words::Unsure { word: "jason".into(), guess: "JSON".into() }]);
    assert_eq!(r.rpc(json!({"op":"act","thread":"t1","action":"voice-use","value":"0","draft":"send jason"})).await["ok"], true);
    let fs = r.until(|v| v["ev"] == "view").await; assert_eq!(fs[0]["ev"], "draft"); assert_eq!(fs[0]["text"], "send JSON"); assert!(fs[1]["view"]["ask"].is_null());
    assert_eq!(r.hub.setting("voice.l.jason").await.as_deref(), Some("JSON\t1\t0"));
    assert_eq!(r.rpc(json!({"op":"add","term":"KiCad"})).await["ok"], true); assert_eq!(r.hub.setting("voice.e.kicad").await.as_deref(), Some("KiCad"));
    assert_eq!(r.rpc(json!({"op":"page-act","action":"voice-term","value":"nRF52840\njason -> JSON"})).await["ok"], true); assert_eq!(r.hub.setting("voice.e.jason>").await.as_deref(), Some("jason -> JSON"));
    assert_eq!(r.rpc(json!({"op":"page-act","action":"setting","value":"voice.e.kicad="})).await["ok"], true); assert_eq!(dict_of(&prefs(&r.hub).await, &r.home).terms(), ["nRF52840".to_string()]);
    assert_eq!(r.rpc(json!({"op":"page-act","action":"setting","value":"onboard.done=1"})).await["error"], "not a voice setting");
}
#[tokio::test(flavor = "multi_thread")] async fn a_rings_recording_is_heard_with_the_dictionary_and_deleted() {
    let _g = ONE.lock().await; let r = rig("ring").await;
    let body = "--x\r\nContent-Disposition: form-data; name=\"transcription\"\r\n\r\nits own words\r\n--x--\r\n";
    assert_eq!(ring(&r.hub, "n1", "multipart/form-data; boundary=x", body, body.as_bytes()).await, body, "no engine: the form as it came");
    r.key(); commit(&r.hub, &[("voice.e.kicad".to_string(), "KiCad = key cad".to_string())]).await.unwrap();
    let f = ring(&r.hub, "n2", "multipart/form-data; boundary=x", body, b"bytes \xff").await;
    assert_eq!(crate::voice::ring::text(&f), "Open KiCad now."); assert_eq!(crate::voice::ring::part(&f, "heard"), "gpt-live-transcribe");
    assert_eq!(std::fs::read(bin().join("form")).unwrap(), b"bytes \xff"); assert!(std::fs::read_to_string(bin().join("file-args")).unwrap().contains("multipart/form-data; boundary=x"));
    assert!(!dir(&r.home).join("ring-n2.form").exists(), "the recording does not stay");
}
#[test] fn the_engine_a_setting_picks() { assert_eq!([engine_of("auto", true), engine_of("auto", false), engine_of("local", false), engine_of("cloud", true), engine_of("cloud", false)], ["local", "openai", "local", "openai", "openai"]); }
