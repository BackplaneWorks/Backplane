//! The session against a stand-in helper (a shell script that speaks the helper's NDJSON): no microphone, no network, no model.
use super::*;
use crate::{hub::{Cfg, Out}, presenter::Canned};
use crate::plat::fs::PermissionsExt;

const HELPER: &str = r#"#!/bin/sh
here=$(dirname "$0")
case "$1" in
probe) exit 0;;
devices) echo '{"event":"devices","default":"m","devices":[{"name":"m","label":"Desk mic"}]}'; echo '{"event":"end"}';;
local-status) echo '{"event":"local","platform":"linux-x64","supported":true,"runtime":false,"vad":false,"models":[{"id":"parakeet-v2","label":"Parakeet v2","size":1000000,"installed":false}]}'; echo '{"event":"end"}';;
local-install)
  echo '{"event":"progress","what":"Parakeet v2","done":500000,"total":1000000,"phase":"download"}'
  if [ -e "$here/fail-install" ]; then echo '{"event":"error","error":"checksum mismatch"}'; echo '{"event":"end"}'; exit 1; fi
  echo '{"event":"end"}';;
serve)
  echo "$*" > "$here/serve-args"
  echo '{"event":"loaded","ms":5,"rss_kb":716800,"model":"parakeet-v2"}'
  cat > /dev/null
  echo stopped > "$here/serve-stopped";;
file)
  echo "$*" > "$here/file-args"; cp "$2" "$here/form"
  echo '{"event":"ready"}'; echo '{"event":"done","item":"file","text":"Open key cad now.","segments":[[12,180,87,"Open key cad now."]]}'; echo '{"event":"end"}';;
live)
  echo "$*" > "$here/args"
  case "$*" in *--always*)
    echo "$*" >> "$here/always-args"
    echo '{"event":"ready","recorder":"pw-record","device":"m","label":"Desk mic"}'; echo '{"event":"loaded","ms":5,"resident":true}'
    echo '{"event":"turn","item":"l0","p":0.2,"ms":30}'; echo '{"event":"done","item":"l0","text":"","words":[["Um.",0,20,90]]}'
    echo '{"event":"turn","item":"l1","p":0.95,"ms":30}'; echo '{"event":"done","item":"l1","text":"","words":[["Ship",100,120,99],["it",120,130,99],["today.",130,160,99]]}'
    cat > /dev/null; echo '{"event":"stopping"}'; echo '{"event":"end"}'; exit 0;;
  esac
  if [ -e "$here/eot" ]; then
    echo '{"event":"ready","recorder":"pw-record","device":"m","label":"Desk mic"}'; echo '{"event":"loaded","ms":5,"resident":true}'
    echo '{"event":"turn","item":"l0","p":0.93,"ms":30}'
    echo '{"event":"done","item":"l0","text":"","words":[["Ship",0,20,99],["it",20,30,99],["today.",30,60,99]]}'
    cat > /dev/null; echo '{"event":"stopping"}'; echo '{"event":"end"}'; exit 0
  fi
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
/// Writes the stand-in helper inside `home/helper` and points `BACKPLANE_VOICE_BIN` at it (serialized by `ONE`).
fn setup_bin(home: &Path) -> PathBuf {
    let d = home.join("helper"); std::fs::create_dir_all(&d).unwrap();
    let f = d.join("backplane-voice"); std::fs::write(&f, HELPER).unwrap(); std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
    // Another test's fork while the script was open for writing keeps it open in that child until its exec, and an exec meanwhile fails
    // "Text file busy". Wait until one exec works: no writer is left after that, so no later exec can fail so.
    for _ in 0..1000 {
        match crate::proc::std_cmd(crate::proc::Kind::Local, &f).arg("probe").output() {
            Err(e) if e.raw_os_error() == Some(crate::plat::errno::ETXTBSY) => std::thread::sleep(std::time::Duration::from_millis(5)),
            r => { r.expect("the stand-in helper runs"); break }
        }
    }
    std::env::set_var("BACKPLANE_VOICE_BIN", &f); d
}
/// The tests share `BACKPLANE_VOICE_BIN`: one at a time.
static ONE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
struct Rig { hub: Arc<Hub>, v: Arc<Voice>, out: crate::hub::Outbox, bin: PathBuf, home: crate::plat::tmp::TempDir }
async fn rig(tag: &str) -> Rig {
    let home = crate::plat::tmp::dir(&format!("voice-{tag}"));
    let bin = setup_bin(&home);
    let hub = Hub::start(Cfg { home: home.to_path_buf(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let out = hub.attach(1); let v = start(&hub);
    Rig { hub, v, out, bin, home }
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
    for f in fs { if f["ev"] == "edit" { let e = crate::voice::place::Edit { del: f["del"].as_u64().unwrap() as usize, ins: f["ins"].as_str().unwrap().into(), adv: f["adv"].as_u64().unwrap() as usize, to: f["to"].as_u64().unwrap() as usize, back: f["back"].as_u64().unwrap_or(0) as usize }; (t, m, _) = crate::voice::place::apply(&t, m, &e); } }
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
    let args = std::fs::read_to_string(r.bin.join("args")).unwrap(); assert!(args.contains("--key-file") && args.contains("--keywords-file") && !args.contains("sk-"), "{args}");
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
    assert_eq!(std::fs::read(r.bin.join("audio")).unwrap(), b"pcm-bytes"); assert!(std::fs::read_to_string(r.bin.join("args")).unwrap().contains("--source stdin --rate 16000"));
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
    assert_eq!(crate::plat::fs::mode_bits(std::fs::metadata(key_path(&r.home)).unwrap().permissions().mode() & 0o777, 0o600), 0o600);
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
    std::fs::write(r.bin.join("fail-install"), "").unwrap();
    r.rpc(json!({"op":"install","model":"parakeet-v2"})).await;
    let fs = r.until(|v| v["ev"] == "page" && v["rows"][1]["note"] == "Download failed: checksum mismatch").await; let _ = std::fs::remove_file(r.bin.join("fail-install"));
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
#[tokio::test(flavor = "multi_thread")] async fn a_recording_is_heard_with_the_dictionary_with_its_segments() {
    let _g = ONE.lock().await; let r = rig("ring").await;
    let p = r.home.join("rec.m4a"); std::fs::write(&p, b"bytes \xff").unwrap();
    assert_eq!(hear_file(&r.hub, &p, "audio/mp4", "").await.unwrap_err(), "no OpenAI key in Settings, Voice", "no engine: nothing heard, and why");
    assert!(hear_file(&r.hub, &p, "audio/mp4", "local:parakeet-v3").await.unwrap_err().contains("parakeet-v3 is not downloaded"));
    r.key(); commit(&r.hub, &[("voice.e.kicad".to_string(), "KiCad = key cad".to_string())]).await.unwrap();
    let h = hear_file(&r.hub, &p, "audio/mp4", "").await.unwrap();
    assert_eq!((h.text.as_str(), h.model.as_str(), h.conf), ("Open KiCad now.", "gpt-live-transcribe", Some(87))); assert_eq!(h.segs, vec![(12, 180, 87, "Open key cad now.".to_string())]);
    assert_eq!(std::fs::read(r.bin.join("form")).unwrap(), b"bytes \xff"); assert!(std::fs::read_to_string(r.bin.join("file-args")).unwrap().contains("--content-type audio/mp4"));
    assert!(p.exists(), "the recording stays where it is kept");
}
#[test] fn a_hearing_picks_its_engine() {
    let p: Prefs = [("voice.model".to_string(), "parakeet-v3".to_string())].into_iter().collect(); let home = std::path::Path::new("/nonexistent");
    assert_eq!(choose(&p, home, ""), ("openai", "parakeet-v3".to_string())); assert_eq!(choose(&p, home, "local"), ("local", "parakeet-v3".to_string()));
    assert_eq!(choose(&p, home, "local:parakeet-v2"), ("local", "parakeet-v2".to_string())); assert_eq!(choose(&p, home, "openai").0, "openai");
}
#[test] fn the_engine_a_setting_picks() { assert_eq!([engine_of("auto", true), engine_of("auto", false), engine_of("local", false), engine_of("cloud", true), engine_of("cloud", false)], ["local", "openai", "local", "openai", "openai"]); }
#[tokio::test(flavor = "multi_thread")] async fn the_local_engine_keeps_one_recognizer_loaded_for_every_session_and_lets_it_go_with_the_cloud() {
    let _g = ONE.lock().await; let mut r = rig("resident").await; let _ = std::fs::remove_file(r.bin.join("serve-stopped"));
    let res = resident::of(&r.home).unwrap();
    let up = |r: &Rig| r.hub.info_now()["voice.resident"].as_str().map_or(false, |s| s.contains("\"loaded\""));
    tokio::time::sleep(Duration::from_millis(200)).await; assert!(!up(&r), "no model: nothing loads");
    std::fs::create_dir_all(local_dir(&r.home)).unwrap(); std::fs::write(local_dir(&r.home).join("ready-parakeet-v2"), "ok").unwrap(); res.kick();
    for _ in 0..100 { if up(&r) { break } tokio::time::sleep(Duration::from_millis(50)).await }
    assert!(up(&r), "{}", r.hub.info_now()); assert!(r.hub.info_now()["voice.resident"].as_str().unwrap().contains("\"rss_mb\":700"));
    assert!(std::fs::read_to_string(r.bin.join("serve-args")).unwrap().contains(&format!("--socket {}", r.home.join("voice/engine.sock").display())));
    // a dictation and a ring's recording both decode through it
    r.rpc(json!({"op":"start","thread":"t1","draft":"","src":"hub"})).await; r.until(|v| v["ev"] == "edit").await; r.rpc(json!({"op":"stop"})).await; r.until(off).await;
    assert!(std::fs::read_to_string(r.bin.join("args")).unwrap().contains("--engine local") && std::fs::read_to_string(r.bin.join("args")).unwrap().contains("--server "), "live goes to the server");
    let p = r.home.join("note.m4a"); std::fs::write(&p, b"a").unwrap(); let _ = hear_file(&r.hub, &p, "audio/mp4", "").await;
    assert!(std::fs::read_to_string(r.bin.join("file-args")).unwrap().contains("--server "), "and the ring's file too");
    // the cloud chosen: let go
    assert_eq!(r.rpc(json!({"op":"page-act","action":"setting","value":"voice.engine=cloud"})).await["ok"], true);
    for _ in 0..100 { if r.bin.join("serve-stopped").exists() { break } tokio::time::sleep(Duration::from_millis(50)).await }
    assert!(r.bin.join("serve-stopped").exists(), "the server's stdin closed"); 
    for _ in 0..100 { if r.hub.info_now().get("voice.resident").is_none() { break } tokio::time::sleep(Duration::from_millis(50)).await }
    assert_eq!(r.hub.info_now().get("voice.resident"), None);
}

/// Jev's stand-in: answers every request with `p`, and keeps what it was sent.
async fn jev_mock(p: f64) -> (String, Arc<Mutex<Vec<String>>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let url = format!("http://{}", l.local_addr().unwrap());
    let got = Arc::new(Mutex::new(vec![])); let g = got.clone();
    tokio::spawn(async move { loop {
        let Ok((mut s, _)) = l.accept().await else { return };
        let mut buf = vec![0u8; 65536]; let mut n = 0;
        loop { let k = s.read(&mut buf[n..]).await.unwrap_or(0); if k == 0 { break } n += k; let t = String::from_utf8_lossy(&buf[..n]).to_string();
            if let Some(h) = t.find("\r\n\r\n") { let len = t.to_lowercase().split("content-length:").nth(1).and_then(|x| x.trim().split('\r').next()?.trim().parse::<usize>().ok()).unwrap_or(0); if n >= h + 4 + len { break } } }
        g.lock().unwrap().push(String::from_utf8_lossy(&buf[..n]).to_string());
        let body = json!({"model": "jev-1.14.0", "answers": {"done": {"type": "noul", "noul": p}}}).to_string();
        let _ = s.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes()).await;
    } });
    (url, got)
}
#[tokio::test(flavor = "multi_thread")] async fn a_finished_sentence_is_sent_by_jevs_word_after_the_countdown_and_an_undo_teaches_patience() {
    let _g = ONE.lock().await; let mut r = rig("eot").await; std::fs::write(r.bin.join("eot"), "").unwrap();
    std::fs::create_dir_all(local_dir(&r.home)).unwrap(); std::fs::write(local_dir(&r.home).join("ready-parakeet-v2"), "ok").unwrap(); // the local engine: timed words
    let (url, got) = jev_mock(0.8).await; std::env::set_var("BACKPLANE_JEV_URL", &url); *super::super::jev::TEST_KEY.lock().unwrap() = Some("ts-test-key".into());
    commit(&r.hub, &[("voice.eot.jev".to_string(), "on".to_string())]).await.unwrap();
    r.rpc(json!({"op":"start","thread":"t1","draft":"","src":"hub"})).await;
    let fs = r.until(|v| v["ev"] == "command").await;
    let cd = fs.iter().find(|f| f["ev"] == "countdown").expect("a countdown"); assert_eq!((cd["ms"].as_u64(), cd["by"].as_str()), (Some(1500), Some("jev")));
    assert_eq!(fs.last().unwrap()["cmd"], "send");
    let req = got.lock().unwrap().join("\n"); assert!(req.contains("Bearer ts-test-key") && req.contains(r#"Transcript so far: \"Ship it today.\""#) && req.contains("gives 0.93"), "{req}");
    assert!(r.hub.info_now()["voice.eot.last"].as_str().unwrap().contains("\"by\":\"jev\""));
    r.rpc(json!({"op":"act","thread":"t1","action":"voice-undid","value":"","draft":""})).await;
    let l = crate::voice::eot::Learned::parse(&r.hub.setting("voice.eot.learned").await.unwrap_or_default()); assert_eq!((l.sends, l.undone), (1, 1)); assert!(l.nudge > 0.0);
    r.rpc(json!({"op":"stop"})).await; r.until(off).await;
    let _ = std::fs::remove_file(r.bin.join("eot")); std::env::remove_var("BACKPLANE_JEV_URL"); *super::super::jev::TEST_KEY.lock().unwrap() = None;
}

#[tokio::test(flavor = "multi_thread")] async fn always_on_writes_only_real_words_sends_by_end_of_turn_and_stays_off_once_muted() {
    let _g = ONE.lock().await; let mut r = rig("always").await;
    std::fs::create_dir_all(local_dir(&r.home)).unwrap(); std::fs::write(local_dir(&r.home).join("ready-parakeet-v2"), "ok").unwrap();
    commit(&r.hub, &[("voice.always".to_string(), "on".to_string())]).await.unwrap();
    assert_eq!(r.rpc(json!({"op":"always"})).await["want"], true);
    r.rpc(json!({"op":"start","thread":"t1","always":true,"src":"hub"})).await;
    let fs = r.until(|v| v["ev"] == "command").await;
    assert_eq!(replay(&fs, ""), "Ship it today.", "the filler before it was never written");
    assert!(fs.iter().any(|f| f["ev"] == "countdown" && f["by"] == "audio"), "the on-device model decided (Jev is off)");
    assert!(fs.iter().any(|f| f["ev"] == "view" && f["view"]["always"]["on"] == true && f["view"]["status"] == "Always on · listening"));
    assert!(std::fs::read_to_string(r.bin.join("always-args")).unwrap().contains("--always"));
    // muted: the session ends and does not start again
    r.rpc(json!({"op":"stop"})).await; r.until(|v| off(v) && v["thread"] == "t1").await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(r.v.st.lock().unwrap().live.is_none(), "muted stays muted"); assert_eq!(std::fs::read_to_string(r.bin.join("always-args")).unwrap().lines().count(), 1);
}
