//! `skill.install {url, provider?}`: a SKILL.md from a link into the person's skills folder (Claude ~/.claude/skills, Codex ~/.agents/skills). The composer asks the person
//! to confirm first (the `$install?` row); only SKILL.md is fetched, over https (http only to this machine), at most 256 KB, and an existing skill of that name is not overwritten.
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

pub fn url_ok(u: &str) -> bool {
    if u.starts_with("https://") && !u.contains(char::is_whitespace) { return true }
    ["http://127.0.0.1", "http://localhost"].iter().any(|h| u.strip_prefix(h).map_or(false, |r| r.starts_with(':') || r.starts_with('/'))) && !u.contains(char::is_whitespace)
}

/// Fetch, check and write; the name and where it went, or why not.
pub fn install(home: &str, url: &str, provider: &str, replace: bool) -> Result<(String, String), String> {
    if crate::copy::on() { return Err(crate::copy::HOME_ONLY.into()) }
    let raw = crate::agents::skills::raw_url(url);
    if !url_ok(&raw) { return Err("only an https link can be installed from".into()) }
    let o = crate::proc::std_cmd(crate::proc::Kind::Net, "curl").args(["-sSL", "--fail", "--max-time", "20", "--max-filesize", "262144", "--proto", "=http,https", "--proto-redir", "=https", "--max-redirs", "4", "--"]).arg(&raw)
        .stdin(std::process::Stdio::null()).output().map_err(|e| format!("curl: {e}"))?;
    if !o.status.success() { return Err("the link could not be fetched".into()) }
    let body = String::from_utf8(o.stdout).map_err(|_| "not a text file".to_string())?;
    let (name, _d) = crate::agents::skills::check_download(&body, &raw)?;
    let dir = crate::agents::skills::install_dir(home, provider).join(&name);
    if !replace && dir.join("SKILL.md").exists() { return Err(format!("{name} is already installed")) }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?; std::fs::write(dir.join("SKILL.md"), body).map_err(|e| e.to_string())?;
    Ok((name, dir.to_string_lossy().into_owned()))
}

pub struct SkillRpc;
impl RpcHandler for SkillRpc {
    fn methods(&self) -> &'static [&'static str] { &["skill.install"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let (url, prov) = (p["url"].as_str().unwrap_or("").to_string(), p["provider"].as_str().unwrap_or("claude").to_string());
            let replace = p["replace"].as_bool().unwrap_or(false); let home = std::env::var("HOME").unwrap_or_default();
            let r = tokio::task::spawn_blocking(move || install(&home, &url, &prov, replace)).await.unwrap_or_else(|_| Err("failed".into()));
            vec![match r { Ok((name, path)) => json!({"t":"reply","id":id,"ok":true,"result":{"name":name,"path":path}}), Err(e) => json!({"t":"reply","id":id,"ok":false,"error":e}) }]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn serve(body: &'static str) -> String {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || { use std::io::{Read, Write}; for _ in 0..5 { let Ok((mut s, _)) = l.accept() else { return }; let mut b = [0u8; 2048]; let _ = s.read(&mut b); let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()); } });
        format!("http://127.0.0.1:{port}/skills/unslop-zq/SKILL.md")
    }
    #[test] fn a_skill_link_installs_once_into_the_providers_folder() {
        if crate::proc::std_cmd(crate::proc::Kind::Net, "curl").arg("--version").output().is_err() { eprintln!("no curl: skipped"); return }
        let h = std::env::temp_dir().join(format!("bp-inst-{}", std::process::id())); let _ = std::fs::remove_dir_all(&h); let hs = h.to_string_lossy().into_owned();
        let url = serve("---\nname: unslop-zq\ndescription: cut filler\n---\nSteps.");
        let (n, p) = install(&hs, &url, "claude", false).unwrap(); assert_eq!(n, "unslop-zq"); assert!(std::fs::read_to_string(format!("{p}/SKILL.md")).unwrap().contains("Steps."));
        assert!(p.replace('\\', "/").contains("/.claude/skills/")); assert!(install(&hs, &url, "claude", false).unwrap_err().contains("already installed"));
        assert!(install(&hs, "http://example.com/x/SKILL.md", "claude", false).is_err(), "plain http elsewhere is refused");
        assert!(install(&hs, &url, "claude", true).is_ok(), "a confirmed replace overwrites");
        let (_, p2) = install(&hs, &url, "codex", false).unwrap(); assert!(p2.replace('\\', "/").contains("/.agents/skills/"));
        std::fs::remove_dir_all(&h).ok();
    }
}
