//! Jev, TypeSafe's System One model (docs.typesafe.ai), asked whether a dictation is finished (eot.rs). `POST /v1/systemone` with the key from the
//! vault (`typesafe-api-key`, read when a request goes and kept nowhere else) in a 0600 curl config that is removed after (google/net.rs `send_in`):
//! never on a command line, in the log or in a setting. Only the current draft goes, with the pause, the audio model's probability and the
//! thread's last reply. `BACKPLANE_JEV_URL` moves the base (tests run a stand-in).
use super::eot;
use crate::google::net::{send_in, Req};
use std::path::Path;
use std::time::Instant;

pub fn base() -> String { std::env::var("BACKPLANE_JEV_URL").ok().map(|s| s.trim().trim_end_matches('/').to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| "https://api.typesafe.ai".into()) }

#[cfg(test)]
pub static TEST_KEY: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
/// The key, from the vault (a test's own when it set one).
pub fn key() -> Option<String> {
    #[cfg(test)]
    if let Some(k) = TEST_KEY.lock().unwrap().clone() { return Some(k) }
    crate::settings::vault::person_get(eot::KEY).map(|k| k.trim().to_string()).filter(|k| !k.is_empty())
}

/// Jev's probability that the speaker is done, and how long the round trip took (ms).
pub async fn ask(home: &Path, model: &str, state: &str, limit_secs: u64) -> Result<(f64, u64), String> {
    let Some(k) = key() else { return Err("no TypeSafe API key in the vault (typesafe-api-key)".into()) };
    let r = Req::json("POST", format!("{}/v1/systemone", base()), &k, &eot::request(model, state));
    let t = Instant::now();
    let resp = send_in(home, "typesafe", Req { secs: limit_secs.max(1), ..r }).await;
    let ms = t.elapsed().as_millis() as u64;
    if !resp.ok() { return Err(if resp.code == "000" { format!("TypeSafe did not answer ({})", resp.err) } else { format!("TypeSafe said {}: {}", resp.code, resp.text().chars().take(200).collect::<String>()) }) }
    eot::answer(&resp.json()).map(|p| (p, ms)).ok_or_else(|| "TypeSafe's answer had no probability".into())
}

/// Settings' Test: two drafts the answer should tell apart (the person's own check, 2026-11-11: 0.71 and 0.15), each with its round trip.
pub async fn test(home: &Path, model: &str) -> Result<Vec<(String, f64, u64)>, String> {
    let mut out = vec![];
    for d in ["Please add a reset button to the board and route it to pin PA0.", "Please add a reset button to the board and um"] {
        let (p, ms) = ask(home, model, &eot::state(d, 900, None, ""), 10).await?; out.push((d.to_string(), p, ms));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    /// The real Jev, through the hub's own request: `secret_run` puts the vault's key in a file (`BACKPLANE_JEV_KEY_FILE`); never in CI.
    ///   cargo test jev_smoke -- --ignored --nocapture
    #[tokio::test] #[ignore] async fn jev_smoke() {
        let k = std::fs::read_to_string(std::env::var("BACKPLANE_JEV_KEY_FILE").expect("BACKPLANE_JEV_KEY_FILE")).unwrap();
        *super::TEST_KEY.lock().unwrap() = Some(k.trim().to_string());
        let home = crate::plat::tmp::dir("jev-smoke");
        let r = super::test(&home, super::eot::MODEL_DEFAULT).await.expect("Jev answers");
        for (d, p, ms) in &r { eprintln!("jev: {p:.2} in {ms} ms for {d:?}") }
        assert!(r[0].1 > r[1].1, "a finished sentence scores above one ending in “um”");
        assert!(!home.join("secrets/typesafe").read_dir().map_or(false, |mut d| d.next().is_some()), "nothing left on disk");
        *super::TEST_KEY.lock().unwrap() = None;
    }
}
