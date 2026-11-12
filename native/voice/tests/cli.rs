//! The helper as the hub runs it, against the mock API; never calls OpenAI.
//! Each `voice.test.ts` behaviour has its test here.

mod common;
use backplane_voice::util::which;
use base64::Engine as _;
use common::*;
use serde_json::{json, Value};
use std::io::Read;
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::time::Duration;

fn empty() -> Bins {
    Bins::new(&[])
}

#[test]
fn live_from_stdin_session_appends_commit_after_quiet_deltas_and_done() {
    let m = Mock::start();
    let t = tempfile::tempdir().unwrap();
    let kw = write_file(t.path(), "dictionary.txt", b"  AC-42 \n\npremium plan\n");
    let mut r = Run::new(&["live", "--source", "stdin", "--keywords-file", &kw, "--delay", "medium", "--language", "en", "--prompt", "KiCad talk"], &m.base, &[]);
    let pcm = [quiet(500), tone(1500, 8000.0), quiet(1000), tone(1000, 8000.0)].concat();
    r.write(&pcm);
    r.close_stdin();
    assert_eq!(r.done(), 0);

    let ws = m.ws();
    let s = &ws[0];
    assert_eq!(s["type"], "session.update");
    assert_eq!(s["session"]["type"], "transcription");
    assert_eq!(s["session"]["audio"]["input"]["format"], json!({"type": "audio/pcm", "rate": 24000}));
    assert!(s["session"]["audio"]["input"]["turn_detection"].is_null());
    assert!(s["session"]["audio"]["input"].as_object().unwrap().contains_key("turn_detection"));
    assert_eq!(
        s["session"]["audio"]["input"]["transcription"],
        json!({"model": "gpt-live-transcribe", "prompt": "KiCad talk", "keywords": ["AC-42", "premium plan"], "languages": ["en"], "delay": "medium"})
    );

    let appends = m.of_type("input_audio_buffer.append");
    assert!(!appends.is_empty());
    let mut sent = 0;
    for a in &appends {
        let b = base64::engine::general_purpose::STANDARD.decode(a["audio"].as_str().unwrap()).unwrap();
        assert_eq!(b.len() % 2, 0);
        sent += b.len();
    }
    assert!(sent < pcm.len()); // leading silence beyond the pre-roll is not sent
    assert_eq!(m.of_type("input_audio_buffer.commit").len(), 2);

    let ev = r.names();
    assert_eq!(ev[0], "ready");
    assert_eq!(r.events()[0], json!({"event": "ready", "recorder": "stdin", "device": "", "label": ""}));
    assert_eq!(ev.last().unwrap(), "end");
    assert!(ev.contains(&"stopping".to_string()));
    assert!(!ev.contains(&"error".to_string()));
    let dones = r.all("done");
    assert_eq!(dones.iter().map(|d| d["item"].as_str().unwrap()).collect::<Vec<_>>(), vec!["item_1", "item_2"]);
    let text = dones[0]["text"].as_str().unwrap();
    assert!(text.starts_with("seg1 ") && text.ends_with("ms"), "{text}");
    let deltas: String = r.all("delta").iter().filter(|e| e["item"] == "item_1").map(|e| e["text"].as_str().unwrap().to_string()).collect();
    assert_eq!(deltas, text);
}

#[test]
fn the_raw_ready_line_keeps_its_field_order() {
    let m = Mock::start();
    let o = Command::new(EXE)
        .args(["live", "--source", "stdin"])
        .env("OPENAI_API_KEY", "good")
        .env("OPENAI_BASE_URL", &m.base)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert_eq!(out.lines().next().unwrap(), r#"{"event":"ready","recorder":"stdin","device":"","label":""}"#);
}

fn stopped_how(sigterm: bool) {
    let m = Mock::start();
    let bin = fake_recorder("arecord", None);
    let mut r = Run::new(&["live"], &m.base, &[("PATH", Some(bin.path()))]);
    r.next("ready");
    // stop once the helper has heard the tone (a level above 0): by then it has more than the 0.1 s a commit needs, however slow the machine
    r.next_where("level", |e| e["v"].as_i64().unwrap_or(0) > 0);
    if sigterm {
        r.term();
    } else {
        r.write(b"stop\n");
    }
    assert_eq!(r.done(), 0);
    let ev = r.names();
    assert_eq!(ev[..2], ["ready", "stopping"]);
    assert_eq!(ev.last().unwrap(), "end");
    assert_eq!(m.of_type("input_audio_buffer.commit").len(), 1);
    assert!(r.all("done")[0]["text"].as_str().unwrap().starts_with("seg1"));
}

#[test]
fn live_from_a_recorder_stopped_by_a_stop_line_commits_and_ends_cleanly() {
    stopped_how(false);
}

#[test]
fn live_from_a_recorder_stopped_by_sigterm_commits_and_ends_cleanly() {
    stopped_how(true);
}

#[test]
fn live_from_a_recorder_stopped_by_sigint_or_stdins_end() {
    let m = Mock::start();
    let bin = fake_recorder("arecord", None);
    let mut r = Run::new(&["live"], &m.base, &[("PATH", Some(bin.path()))]);
    r.next("ready");
    unsafe { libc::kill(r.pid() as i32, libc::SIGINT) };
    assert_eq!(r.done(), 0);
    assert!(r.names().contains(&"stopping".to_string()));

    let mut r = Run::new(&["live"], &m.base, &[("PATH", Some(bin.path()))]);
    r.next("ready");
    r.close_stdin();
    assert_eq!(r.done(), 0);
    assert!(r.names().contains(&"stopping".to_string()));
}

#[test]
fn devices_from_pactl_nothing_to_ask_is_an_empty_list_and_an_error() {
    let m = Mock::start();
    let bin = Bins::new(&[("pactl", fake_pactl())]);
    let mut r = Run::new(&["devices"], &m.base, &[("PATH", Some(bin.path()))]);
    assert_eq!(r.done(), 0);
    assert_eq!(
        r.events(),
        vec![
            json!({"event": "devices", "default": "alsa_input.usb-mic.mono", "devices": [
                {"name": "alsa_input.usb-mic.mono", "label": "USB Mic Mono"},
                {"name": "alsa_input.webcam.stereo", "label": "Webcam Stereo"},
            ]}),
            json!({"event": "end"}),
        ]
    );
    let none = empty();
    let mut r2 = Run::new(&["devices"], &m.base, &[("PATH", Some(none.path()))]);
    assert_eq!(r2.done(), 1);
    assert_eq!(r2.names(), vec!["devices", "error", "end"]);
    assert_eq!(r2.events()[0]["devices"], json!([]));
}

#[test]
fn devices_fall_back_to_arecord() {
    let m = Mock::start();
    let bin = Bins::new(&[("arecord", "echo default\necho '    Default ALSA'\n".to_string())]);
    let mut r = Run::new(&["devices"], &m.base, &[("PATH", Some(bin.path()))]);
    assert_eq!(r.done(), 0);
    assert_eq!(r.events()[0], json!({"event": "devices", "default": "default", "devices": [{"name": "default", "label": "Default ALSA"}]}));
}

#[test]
fn live_ready_names_the_recorder_and_default_device_device_reaches_the_recorder() {
    let m = Mock::start();
    let bin = fake_recorder("arecord", None);
    bin.script("pactl", &fake_pactl());
    let mut r = Run::new(&["live"], &m.base, &[("PATH", Some(bin.path()))]);
    assert_eq!(r.next("ready"), json!({"event": "ready", "recorder": "arecord", "device": "alsa_input.usb-mic.mono", "label": "USB Mic Mono"}));
    r.term();
    assert_eq!(r.done(), 0);
    assert!(!bin.args().contains("-D"));

    let mut r2 = Run::new(&["live", "--device", "alsa_input.webcam.stereo"], &m.base, &[("PATH", Some(bin.path()))]);
    assert_eq!(r2.next("ready"), json!({"event": "ready", "recorder": "arecord", "device": "alsa_input.webcam.stereo", "label": "Webcam Stereo"}));
    r2.term();
    assert_eq!(r2.done(), 0);
    assert!(bin.args().trim().ends_with("-D alsa_input.webcam.stereo"));

    let pw = fake_recorder("pw-record", None);
    let mut r3 = Run::new(&["live", "--source", "pw-record", "--device", "mic2"], &m.base, &[("PATH", Some(pw.path()))]);
    let e = r3.next("ready");
    assert_eq!((&e["recorder"], &e["device"], &e["label"]), (&json!("pw-record"), &json!("mic2"), &json!("")));
    r3.term();
    assert_eq!(r3.done(), 0);
    assert!(pw.args().trim().ends_with("--target mic2 -"));
}

#[test]
fn ready_comes_again_when_the_name_is_late() {
    let m = Mock::start();
    let bin = fake_recorder("arecord", None);
    // pactl that takes longer than the 150 ms ready waits for it
    bin.script("pactl", &format!("/bin/sleep 0.4\n{}", fake_pactl()));
    let mut r = Run::new(&["live"], &m.base, &[("PATH", Some(bin.path()))]);
    // the second ready comes when pactl answers: waited for, not timed, so a loaded machine only makes it later
    r.next_n("ready", 2);
    r.term();
    assert_eq!(r.done(), 0);
    let readies = r.all("ready");
    assert_eq!(readies.len(), 2, "{readies:?}");
    assert_eq!(readies[0]["device"], "");
    assert_eq!(readies[1], json!({"event": "ready", "recorder": "arecord", "device": "alsa_input.usb-mic.mono", "label": "USB Mic Mono"}));
}

#[test]
fn live_a_recorder_that_gives_nothing_gets_the_no_audio_hint() {
    let m = Mock::start();
    let bin = fake_recorder("arecord", Some(":"));
    let mut r = Run::new(&["live"], &m.base, &[("PATH", Some(bin.path()))]);
    r.next("ready");
    assert_eq!(r.next("hint"), json!({"event": "hint", "hint": "no-audio"}));
    r.term();
    assert_eq!(r.done(), 0);
    let levels = r.all("level");
    assert!(levels.len() >= 10, "{}", levels.len());
    assert!(levels.iter().all(|e| e["v"] == 0));
}

#[test]
fn live_silence_gets_the_silent_hint_then_speech_ok_levels_follow_the_sound() {
    let m = Mock::start();
    let bin = Bins::new(&[]);
    let q = bin.pcm("quiet5.pcm", &quiet(5000));
    let sp = bin.pcm("speech2.pcm", &tone(1500, 8000.0));
    // audio at about real time: a quiet second a second, then speech, then quiet: a commit
    let (q, sp) = (q.display(), sp.display());
    let play = format!("for i in 1 2 3 4 5; do /usr/bin/head -c 48000 '{q}'; /bin/sleep 1; done\n/bin/cat '{sp}'\n/usr/bin/head -c 48000 '{q}'");
    bin.script("arecord", &recorder_body(&play));
    let mut r = Run::new(&["live"], &m.base, &[("PATH", Some(bin.path()))]);
    r.next("ready");
    assert_eq!(r.next("hint"), json!({"event": "hint", "hint": "silent"}));
    r.next("done");
    for _ in 0..200 {
        if r.all("level").iter().any(|e| e["v"].as_i64().unwrap() > 0) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    r.term();
    assert_eq!(r.done(), 0);
    let hints: Vec<Value> = r.all("hint").iter().map(|e| e["hint"].clone()).collect();
    assert_eq!(hints, vec![json!("silent"), json!("ok")]);
    let vs: Vec<i64> = r.all("level").iter().map(|e| e["v"].as_i64().unwrap()).collect();
    assert!(vs.iter().all(|v| (0..=100).contains(v)));
    assert_eq!(vs.iter().max(), Some(&75)); // a sine peaking at 8000 is about -15 dBFS
    assert!(vs[..8].iter().all(|v| *v == 0));
}

#[test]
fn live_with_no_recorder_is_an_error_event() {
    let m = Mock::start();
    let none = empty();
    let mut r = Run::new(&["live"], &m.base, &[("PATH", Some(none.path()))]);
    assert_eq!(r.done(), 1);
    assert_eq!(r.names(), vec!["error", "end"]);
    assert_eq!(r.events()[0]["error"], "no recorder found (pw-record, parecord or arecord)");
    let mut r = Run::new(&["live", "--source", "arecord"], &m.base, &[("PATH", Some(none.path()))]);
    assert_eq!(r.done(), 1);
    assert_eq!(r.events()[0]["error"], "arecord not found");
}

#[test]
fn a_recorder_that_fails_at_once_says_why_and_the_next_is_tried() {
    let m = Mock::start();
    let bin = fake_recorder("arecord", None);
    bin.script("pw-record", "echo 'no pipewire here' >&2\nexit 3");
    let mut r = Run::new(&["live"], &m.base, &[("PATH", Some(bin.path()))]);
    assert_eq!(r.next("ready")["recorder"], "arecord");
    r.term();
    assert_eq!(r.done(), 0);

    let bad = Bins::new(&[("arecord", "echo 'device busy' >&2\nexit 1".to_string())]);
    let mut r = Run::new(&["live"], &m.base, &[("PATH", Some(bad.path()))]);
    assert_eq!(r.done(), 1);
    assert_eq!(r.names(), vec!["error", "end"]);
    assert_eq!(r.events()[0]["error"], "could not record from the microphone (arecord: device busy)");
}

#[test]
fn a_recorder_that_dies_mid_session_is_an_error() {
    let m = Mock::start();
    let bin = Bins::new(&[]);
    let p = bin.pcm("a.pcm", &quiet(600));
    bin.script("arecord", &format!("/bin/cat '{}'\necho 'overrun' >&2\nexit 5", p.display()));
    let mut r = Run::new(&["live"], &m.base, &[("PATH", Some(bin.path()))]);
    assert_eq!(r.done(), 1);
    let e = r.next("error");
    assert_eq!(e["error"], "the recorder stopped (arecord: overrun)");
    assert_eq!(r.names().last().unwrap(), "end");
}

#[test]
fn a_bad_key_is_a_401_error_event_recording_starts_before_the_session_opens() {
    let m = Mock::start();
    let mut r = Run::new(&["live", "--source", "stdin"], &m.base, &[("OPENAI_API_KEY", Some("bad"))]);
    r.close_stdin();
    assert_eq!(r.done(), 1);
    let ev = r.names();
    assert_eq!(ev.iter().filter(|e| *e == "error").count(), 1);
    assert_eq!(ev.last().unwrap(), "end");
    assert_eq!(r.next("error")["error"], "OpenAI rejected the API key (401)");
}

#[test]
fn a_refused_upgrade_with_a_good_key_is_explained_by_asking_models() {
    let m = Mock::start();
    m.s.refuse_ws.store(true, std::sync::atomic::Ordering::SeqCst);
    let mut r = Run::new(&["live", "--source", "stdin"], &m.base, &[]);
    r.close_stdin();
    assert_eq!(r.done(), 1);
    assert_eq!(r.next("error")["error"], "could not open the OpenAI realtime session");
}

#[test]
fn nothing_listening_reads_could_not_reach_openai() {
    let free = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let mut r = Run::new(&["live", "--source", "stdin"], &format!("http://{free}/v1"), &[]);
    r.close_stdin();
    assert_eq!(r.done(), 1);
    assert!(r.next("error")["error"].as_str().unwrap().starts_with("could not reach OpenAI: "));
}

#[test]
fn live_ready_goes_out_before_the_openai_session_opens() {
    let mut r = Run::new(&["live", "--source", "stdin"], "http://10.255.255.1:9/v1", &[]);
    assert_eq!(r.next("ready")["recorder"], "stdin");
    r.close_stdin();
}

#[test]
fn audio_waits_for_the_session_and_goes_when_it_opens() {
    // a listener that answers only after a while: everything sent before is kept
    let m = Mock::start();
    let front = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = front.local_addr().unwrap();
    let back = m.base.trim_start_matches("http://").trim_end_matches("/v1").to_string();
    std::thread::spawn(move || {
        if let Ok((mut c, _)) = front.accept() {
            std::thread::sleep(Duration::from_millis(700));
            let mut b = std::net::TcpStream::connect(back).unwrap();
            let (mut c2, mut b2) = (c.try_clone().unwrap(), b.try_clone().unwrap());
            std::thread::spawn(move || std::io::copy(&mut c2, &mut b2));
            let _ = std::io::copy(&mut b, &mut c);
        }
    });
    let mut r = Run::new(&["live", "--source", "stdin"], &format!("http://{addr}/v1"), &[]);
    r.next("ready");
    r.write(&[quiet(300), tone(1000, 8000.0), quiet(900)].concat());
    r.close_stdin();
    assert_eq!(r.done(), 0);
    assert_eq!(m.of_type("input_audio_buffer.commit").len(), 1);
    assert_eq!(r.all("done").len(), 1);
}

#[test]
fn audio_at_another_rate_is_converted() {
    let m = Mock::start();
    // 1 s at 48 kHz is 1 s at 24 kHz, half the bytes
    let mut r = Run::new(&["live", "--source", "stdin", "--rate", "48000"], &m.base, &[]);
    let at_48k: Vec<u8> = tone(1000, 8000.0).chunks(2).flat_map(|s| [s[0], s[1], s[0], s[1]]).collect();
    r.write(&at_48k);
    r.close_stdin();
    assert_eq!(r.done(), 0);
    let text = r.all("done")[0]["text"].as_str().unwrap().to_string();
    let ms: f64 = text.split(' ').nth(1).unwrap().trim_end_matches("ms").parse().unwrap();
    assert!((900.0..=1100.0).contains(&ms), "{text}");
}

#[test]
fn no_key_is_an_error_event_key_file_wins_over_the_environment() {
    let m = Mock::start();
    let mut r = Run::new(&["live", "--source", "stdin"], &m.base, &[("OPENAI_API_KEY", None)]);
    r.close_stdin();
    assert_eq!(r.done(), 1);
    assert_eq!(r.names(), vec!["error", "end"]);
    assert_eq!(r.events()[0]["error"], "no OpenAI API key (set one in Backplane's settings)");

    let t = tempfile::tempdir().unwrap();
    let kf = write_file(t.path(), "openai.key", b"good\n");
    let mut r2 = Run::new(&["live", "--source", "stdin", "--key-file", &kf], &m.base, &[("OPENAI_API_KEY", Some("bad"))]);
    r2.close_stdin();
    assert_eq!(r2.done(), 0);
    assert_eq!(r2.names(), vec!["ready", "stopping", "end"]);

    let mut r3 = Run::new(&["live", "--source", "stdin", "--key-file", "/nonexistent/key"], &m.base, &[]);
    r3.close_stdin();
    assert_eq!(r3.done(), 1);
    assert_eq!(r3.events()[0]["error"], "cannot read the key file /nonexistent/key");
}

#[test]
fn a_keywords_file_that_cannot_be_read_goes_without() {
    let m = Mock::start();
    let mut r = Run::new(&["live", "--source", "stdin", "--keywords-file", "/nonexistent/kw"], &m.base, &[]);
    r.close_stdin();
    assert_eq!(r.done(), 0);
    assert_eq!(m.ws_wait(1)[0]["session"]["audio"]["input"]["transcription"], json!({"model": "gpt-live-transcribe", "delay": "low"}));
}

#[test]
fn file_multipart_audio_part_through_the_realtime_path_with_ffmpeg() {
    if which("ffmpeg").is_none() {
        eprintln!("ffmpeg not found: skipped");
        return;
    }
    let m = Mock::start();
    let t = tempfile::tempdir().unwrap();
    let (body, ct) = pebble_body(true);
    let path = write_file(t.path(), "body.bin", &body);
    let mut r = Run::new(&["file", &path, "--content-type", &ct], &m.base, &[]);
    assert_eq!(r.done(), 0);
    assert_eq!(r.names(), vec!["ready", "done", "end"]);
    let e = &r.events()[1];
    assert_eq!(e["item"], "file");
    let text = e["text"].as_str().unwrap();
    assert!(text.starts_with("seg1 3") && text.ends_with("ms") && text.len() == "seg1 3300ms".len(), "{text}");
    assert_eq!(m.ws_wait(1)[0]["session"]["audio"]["input"]["transcription"]["model"], "gpt-live-transcribe");
    assert!(m.rest().is_empty());
}

#[test]
fn file_a_long_recording_goes_in_pieces_of_at_most_15_s_joined_with_a_space() {
    if which("ffmpeg").is_none() {
        eprintln!("ffmpeg not found: skipped");
        return;
    }
    let m = Mock::start();
    let t = tempfile::tempdir().unwrap();
    let p = write_file(t.path(), "long.wav", &wav(&tone(20000, 8000.0), RATE as u32));
    let mut r = Run::new(&["file", &p], &m.base, &[]);
    assert_eq!(r.done(), 0);
    assert_eq!(m.of_type("input_audio_buffer.commit").len(), 2);
    let text = r.all("done")[0]["text"].as_str().unwrap().to_string();
    assert_eq!(text, "seg1 10000ms seg2 10000ms");
}

#[test]
fn file_without_ffmpeg_rest_gpt_transcribe_with_keywords_and_languages() {
    let m = Mock::start();
    let t = tempfile::tempdir().unwrap();
    let (body, ct) = pebble_body(true);
    let path = write_file(t.path(), "body.bin", &body);
    let kw = write_file(t.path(), "kw.txt", b"AC-42\npremium plan\n");
    let none = empty();
    let mut r = Run::new(&["file", &path, "--content-type", &ct, "--keywords-file", &kw, "--language", "en", "--prompt", "p"], &m.base, &[("PATH", Some(none.path()))]);
    assert_eq!(r.done(), 0);
    assert_eq!(r.names(), vec!["ready", "done", "end"]);
    let text = r.events()[1]["text"].as_str().unwrap().to_string();
    assert!(text.starts_with("rest ") && text.ends_with(" bytes"), "{text}");
    let f = &m.rest()[0];
    assert_eq!(field_text(f, "model").unwrap(), "gpt-transcribe");
    assert_eq!(field_text(f, "response_format").unwrap(), "json");
    assert_eq!(field_text(f, "prompt").unwrap(), "p");
    let kws: Vec<String> = fields(f, "keywords[]").iter().map(|p| String::from_utf8_lossy(&p.data).into_owned()).collect();
    assert_eq!(kws, vec!["AC-42", "premium plan"]);
    assert_eq!(fields(f, "languages[]").len(), 1);
    let file = fields(f, "file")[0];
    assert_eq!(file.filename.as_deref(), Some("note.wav"));
    assert_eq!(file.content_type, "audio/wav");
}

#[test]
fn file_realtime_refused_falls_back_to_rest_a_400_about_keywords_retries_without_them() {
    use std::sync::atomic::Ordering;
    if which("ffmpeg").is_none() {
        eprintln!("ffmpeg not found: skipped");
        return;
    }
    let m = Mock::start();
    m.s.refuse_ws.store(true, Ordering::SeqCst);
    m.s.reject_hints.store(true, Ordering::SeqCst);
    let t = tempfile::tempdir().unwrap();
    let p = write_file(t.path(), "raw.wav", &wav(&tone(1000, 8000.0), RATE as u32));
    let kw = write_file(t.path(), "kw2.txt", b"AC-42\n");
    let mut r = Run::new(&["file", &p, "--keywords-file", &kw], &m.base, &[]);
    assert_eq!(r.done(), 0);
    assert_eq!(r.names(), vec!["ready", "done", "end"]);
    let rest = m.rest();
    assert_eq!(rest.len(), 2);
    assert_eq!(fields(&rest[0], "keywords[]").len(), 1);
    assert!(fields(&rest[1], "keywords[]").is_empty());
}

#[test]
fn file_a_bad_key_over_rest_reads_the_same() {
    let m = Mock::start();
    let t = tempfile::tempdir().unwrap();
    let p = write_file(t.path(), "raw.wav", &wav(&tone(200, 8000.0), RATE as u32));
    let none = empty();
    let mut r = Run::new(&["file", &p], &m.base, &[("PATH", Some(none.path())), ("OPENAI_API_KEY", Some("bad"))]);
    assert_eq!(r.done(), 1);
    assert_eq!(r.next("error")["error"], "OpenAI rejected the API key (401)");
}

#[test]
fn file_a_multipart_body_without_an_audio_part() {
    let m = Mock::start();
    let t = tempfile::tempdir().unwrap();
    let (body, ct) = pebble_body(false);
    let path = write_file(t.path(), "body.bin", &body);
    let mut r = Run::new(&["file", &path, "--content-type", &ct], &m.base, &[]);
    assert_eq!(r.done(), 1);
    assert_eq!(r.names(), vec!["ready", "error", "end"]);
    assert_eq!(r.events()[1]["error"], "no audio part");
}

#[test]
fn file_a_broken_body_and_a_missing_file() {
    let m = Mock::start();
    let t = tempfile::tempdir().unwrap();
    let path = write_file(t.path(), "body.bin", b"nothing like a form");
    let mut r = Run::new(&["file", &path, "--content-type", "multipart/form-data; boundary=x"], &m.base, &[]);
    assert_eq!(r.done(), 1);
    assert_eq!(r.next("error")["error"], "bad multipart body: FormData parse error missing final boundary");
    let mut r = Run::new(&["file", "/nonexistent/a.wav"], &m.base, &[]);
    assert_eq!(r.done(), 1);
    assert_eq!(r.names(), vec!["ready", "error", "end"]);
    assert_eq!(r.next("error")["error"], "no such file: /nonexistent/a.wav");
}

#[test]
fn usage_no_arguments_exits_2_with_nothing_on_stdout() {
    let mut r = Run::new(&[], "http://127.0.0.1:9/v1", &[]);
    assert_eq!(r.done(), 2);
    assert!(r.events().is_empty());
    let mut r = Run::new(&["live", "--source", "nope"], "http://127.0.0.1:9/v1", &[]);
    assert_eq!(r.done(), 2);
    assert!(r.events().is_empty());
}

#[test]
fn https_and_wss_really_speak_tls() {
    // a TLS ClientHello opens with a handshake record: 0x16 0x03
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let t = std::thread::spawn(move || {
        let mut firsts = vec![];
        for _ in 0..2 {
            let (mut c, _) = l.accept().unwrap();
            let mut b = [0u8; 2];
            c.read_exact(&mut b).unwrap();
            firsts.push(b);
        }
        firsts
    });
    let mut r = Run::new(&["live", "--source", "stdin"], &format!("https://{addr}/v1"), &[]);
    r.close_stdin();
    let firsts = t.join().unwrap();
    assert_eq!(firsts, vec![[0x16, 0x03], [0x16, 0x03]]); // the socket, then /models
    assert_eq!(r.done(), 1);
    assert!(r.next("error")["error"].as_str().unwrap().starts_with("could not reach OpenAI: "));
}

// ---- the local engine, without the engine

#[test]
fn local_with_nothing_downloaded_says_so() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().to_str().unwrap();
    let mut r = Run::new(&["live", "--engine", "local", "--local-dir", dir, "--source", "stdin"], "http://127.0.0.1:9/v1", &[]);
    assert_eq!(r.done(), 1);
    assert_eq!(r.names(), vec!["error", "end"]);
    assert_eq!(r.events()[0]["error"], "the local model is not downloaded (Settings → Voice)");
    let mut r = Run::new(&["file", "x.wav", "--engine", "local", "--local-dir", dir], "http://127.0.0.1:9/v1", &[]);
    assert_eq!(r.done(), 1);
    assert_eq!(r.names(), vec!["error", "end"]);
    let mut r = Run::new(&["live", "--engine", "local", "--local-dir", dir, "--model", "nope"], "http://127.0.0.1:9/v1", &[]);
    assert_eq!(r.done(), 1);
    assert_eq!(r.events()[0]["error"], "unknown model nope");
}

#[test]
fn local_status_and_remove_report_the_folder() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().to_str().unwrap();
    std::fs::copy(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("vad/silero_vad.onnx"), t.path().join("silero_vad.onnx")).unwrap();
    let mut r = Run::new(&["local-status", "--local-dir", dir], "http://127.0.0.1:9/v1", &[]);
    assert_eq!(r.done(), 0);
    let e = &r.events()[0];
    let keys: Vec<&String> = e.as_object().unwrap().keys().collect();
    assert_eq!(keys, vec!["event", "platform", "supported", "runtime", "vad", "vadBundled", "turn", "models", "bytes"]);
    assert_eq!(e["turn"], json!(false), "no end-of-turn model fetched");
    assert_eq!(e["vadBundled"], json!(false), "no release copy beside a test build");
    assert_eq!((&e["event"], &e["vad"], &e["runtime"], &e["bytes"]), (&json!("local"), &json!(true), &json!(false), &json!(643854)));
    assert_eq!(e["models"][0], json!({"id": "parakeet-v2", "label": "Parakeet TDT 0.6B v2 (English)", "size": 482440461, "installed": false}));
    assert_eq!(r.names(), vec!["local", "end"]);
    let mut r = Run::new(&["local-remove", "--local-dir", dir], "http://127.0.0.1:9/v1", &[]);
    assert_eq!(r.done(), 0);
    assert_eq!(r.events()[0]["vad"], json!(false)); // the VAD goes with the last model
}

#[cfg(target_os = "linux")]
#[test]
fn a_runtime_of_another_version_is_refused_with_a_plain_error() {
    use backplane_voice::local::{c_library, onnx_library, runtime_dir, vad_path};
    if which("cc").is_none() {
        eprintln!("no C compiler: skipped");
        return;
    }
    let t = tempfile::tempdir().unwrap();
    let rt = runtime_dir(t.path());
    std::fs::create_dir_all(&rt).unwrap();
    // a stand-in library whose only function is the version; anything else called would fail to link
    let c = t.path().join("v.c");
    std::fs::write(&c, "const char *SherpaOnnxGetVersionStr() { return \"9.9.9\"; }\n").unwrap();
    for lib in [c_library(), onnx_library()] {
        let s = Command::new("cc").arg("-shared").arg("-fPIC").arg(&c).arg("-o").arg(rt.join(lib)).status().unwrap();
        assert!(s.success());
    }
    std::fs::copy(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("vad/silero_vad.onnx"), vad_path(t.path())).unwrap();
    let m = t.path().join("sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8");
    std::fs::create_dir_all(&m).unwrap();
    // the pinned sizes, sparse
    for (f, n) in [("encoder.int8.onnx", 652282309), ("decoder.int8.onnx", 7257753), ("joiner.int8.onnx", 1739080), ("tokens.txt", 9384)] {
        std::fs::File::create(m.join(f)).unwrap().set_len(n).unwrap();
    }
    let dir = t.path().to_str().unwrap();
    let mut r = Run::new(&["live", "--engine", "local", "--local-dir", dir, "--source", "stdin"], "http://127.0.0.1:9/v1", &[]);
    let e = r.next("error");
    r.close_stdin();
    assert_eq!(r.done(), 1);
    let m = e["error"].as_str().unwrap();
    assert!(m.starts_with("could not load the local model (the speech runtime is version 9.9.9, this helper needs 2.0.1"), "{m}");
    assert_eq!(r.names().last().unwrap(), "end");
    assert!(!r.names().contains(&"loaded".to_string()));
}

// ---- record

struct Raw {
    child: std::process::Child,
}

impl Raw {
    fn new(args: &[&str], path: &str) -> Raw {
        let child = Command::new(EXE).args(args).env_clear().env("PATH", path).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        Raw { child }
    }

    fn term(&self) {
        unsafe { libc::kill(self.child.id() as i32, libc::SIGTERM) };
    }

    /// Reads N bytes of stdout.
    fn take(&mut self, n: usize) -> Vec<u8> {
        let mut b = vec![0u8; n];
        self.child.stdout.as_mut().unwrap().read_exact(&mut b).unwrap();
        b
    }

    /// The rest of stdout, stderr and the exit code. stdin stays open until
    /// the exit (its end would be a stop); drop it first to ask for one.
    fn finish(mut self) -> (Vec<u8>, String, i32) {
        let stdin = self.child.stdin.take();
        let o = self.child.wait_with_output().unwrap();
        drop(stdin);
        (o.stdout, String::from_utf8_lossy(&o.stderr).into_owned(), o.status.code().unwrap_or(-1))
    }
}

#[test]
fn record_writes_the_recorders_pcm_to_stdout_until_sigterm() {
    let bin = Bins::new(&[]);
    let audio = tone(500, 8000.0);
    let p = bin.pcm("a.pcm", &audio);
    bin.script("arecord", &recorder_body(&format!("/bin/cat '{}'", p.display())));
    let mut r = Raw::new(&["record", "--rate", "8000", "--device", "hw:1"], bin.path());
    assert_eq!(r.take(audio.len()), audio);
    r.term();
    let (rest, err, code) = r.finish();
    assert_eq!(code, 0);
    assert!(rest.is_empty(), "{} more bytes", rest.len());
    assert!(err.contains("recording from arecord at 8000 Hz"), "{err}");
    let args = bin.args();
    assert!(args.contains("-r 8000") && args.contains("-c 1") && args.trim().ends_with("-D hw:1"), "{args}");
}

#[test]
fn record_defaults_to_16_khz_and_stops_at_stdins_end() {
    let bin = fake_recorder("pw-record", None);
    let mut r = Raw::new(&["record"], bin.path());
    r.take(1000);
    drop(r.child.stdin.take());
    let (_, _, code) = r.finish();
    assert_eq!(code, 0);
    assert!(bin.args().contains("--rate 16000"), "{}", bin.args());
}

#[test]
fn record_with_no_recorder_fails_on_stderr_and_keeps_stdout_clean() {
    let none = empty();
    let (out, err, code) = Raw::new(&["record"], none.path()).finish();
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains("no recorder found (pw-record, parecord or arecord)"), "{err}");
}

#[test]
fn record_when_the_recorder_dies_exits_1() {
    let bin = Bins::new(&[]);
    let p = bin.pcm("a.pcm", &quiet(600));
    bin.script("arecord", &format!("/bin/cat '{}'\necho 'overrun' >&2\nexit 5", p.display()));
    let (out, err, code) = Raw::new(&["record"], bin.path()).finish();
    assert_eq!(code, 1);
    assert_eq!(out.len(), quiet(600).len());
    assert!(err.contains("the recorder stopped (arecord: overrun)"), "{err}");
}

#[test]
fn record_from_stdin_passes_it_through() {
    use std::io::Write;
    let none = empty();
    let mut r = Raw::new(&["record", "--source", "stdin"], none.path());
    r.child.stdin.as_mut().unwrap().write_all(b"abcd").unwrap();
    drop(r.child.stdin.take());
    let (out, _, code) = r.finish();
    assert_eq!((out.as_slice(), code), (b"abcd".as_slice(), 0));
}

#[test]
fn record_ends_when_its_reader_goes_away() {
    let bin = Bins::new(&[]);
    let p = bin.pcm("a.pcm", &quiet(100));
    bin.script("arecord", &recorder_body(&format!("while :; do /bin/cat '{}'; /bin/sleep 0.05; done", p.display())));
    let mut r = Raw::new(&["record"], bin.path());
    r.take(100);
    drop(r.child.stdout.take());
    let s = r.child.wait().unwrap();
    assert_eq!(s.code(), Some(0));
}

#[test]
fn record_a_bad_rate_is_usage() {
    let none = empty();
    let (out, _, code) = Raw::new(&["record", "--rate", "fast"], none.path()).finish();
    assert_eq!((out.len(), code), (0, 2));
}
