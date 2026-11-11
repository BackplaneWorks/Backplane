//! The MCP servers already set up in the person's desktop tools: Claude Code (`~/.claude.json`, its projects, a project's `.mcp.json`), Codex
//! (`~/.codex/config.toml`), Cursor, VS Code, Claude Desktop, Windsurf, Gemini CLI and OpenCode. Read only; never written. Import copies the
//! chosen ones into the list (a real key found in one goes to the vault); a server imported with sync on follows its file (`sync`).
use super::model::{List, Server};
use super::parse::{self, Found};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Source { pub tool: &'static str, pub path: PathBuf }
#[derive(Debug, Default)]
pub struct Scan { pub tool: &'static str, pub path: String, pub found: Found, pub error: String }

/// Every place a tool keeps its servers, for this home and these project roots (the files that exist).
pub fn sources(home: &Path, roots: &[String]) -> Vec<Source> {
    let mut v: Vec<(&'static str, PathBuf)> = vec![("Claude Code", home.join(".claude.json"))];
    let codex_home = std::env::var("CODEX_HOME").ok().filter(|s| !s.is_empty()).map(PathBuf::from).filter(|p| p.starts_with(home)).unwrap_or_else(|| home.join(".codex"));
    v.push(("Codex", codex_home.join("config.toml")));
    v.push(("Cursor", home.join(".cursor/mcp.json")));
    for base in [home.join(".config"), home.join("Library/Application Support"), home.join("AppData/Roaming")] {
        for code in ["Code", "Code - Insiders", "VSCodium"] { v.push(("VS Code", base.join(code).join("User/mcp.json"))); v.push(("VS Code", base.join(code).join("User/settings.json"))); }
        v.push(("Claude Desktop", base.join("Claude/claude_desktop_config.json")));
    }
    v.push(("Windsurf", home.join(".codeium/windsurf/mcp_config.json")));
    v.push(("Gemini CLI", home.join(".gemini/settings.json")));
    v.push(("OpenCode", home.join(".config/opencode/opencode.json")));
    for r in roots.iter().filter(|r| !r.is_empty()) {
        let r = Path::new(r);
        v.push(("Claude Code", r.join(".mcp.json"))); v.push(("Cursor", r.join(".cursor/mcp.json"))); v.push(("VS Code", r.join(".vscode/mcp.json"))); v.push(("Gemini CLI", r.join(".gemini/settings.json")));
    }
    let mut seen = vec![];
    v.into_iter().filter(|(_, p)| p.is_file()).filter(|(_, p)| { let k = p.clone(); if seen.contains(&k) { false } else { seen.push(k); true } }).map(|(tool, path)| Source { tool, path }).collect()
}

/// A file's servers. `~/.claude.json` keeps them at the top and per project (`projects.<path>.mcpServers`), each read.
pub fn read(src: &Source, taken: &[String]) -> Scan {
    let mut sc = Scan { tool: src.tool, path: src.path.to_string_lossy().into_owned(), ..Default::default() };
    let text = match std::fs::read_to_string(&src.path) { Ok(t) => t, Err(e) => { sc.error = e.to_string(); return sc } };
    if text.len() > 32 << 20 { sc.error = "too large to read".into(); return sc }
    let r = if src.path.extension().map_or(false, |e| e == "toml") { super::toml::parse(&text).map(|v| parse::codex(&v, taken)) }
        else { serde_json::from_str::<Value>(&parse::strip_jsonc(&text)).map_err(|e| e.to_string()).map(|v| doc(&v, taken)) };
    match r { Ok(mut f) => { for s in f.servers.iter_mut() { s.source = format!("import:{}", sc.path); } sc.found = f } Err(e) => sc.error = e }
    sc
}
fn doc(v: &Value, taken: &[String]) -> Found {
    let mut all = Found { format: "", ..Default::default() };
    let mut ids = taken.to_vec();
    let mut add = |f: Result<Found, String>, all: &mut Found, ids: &mut Vec<String>| if let Ok(f) = f { all.format = f.format; for s in f.servers { ids.push(s.id.clone()); all.servers.push(s) } all.plain.extend(f.plain); all.notes.extend(f.notes) };
    let top = v.get("mcpServers").is_some() || v.get("servers").is_some() || v.pointer("/mcp/servers").is_some() || v.get("mcp").is_some();
    if top { add(parse::json_doc(v, &ids), &mut all, &mut ids) }
    if let Some(ps) = v.get("projects").and_then(Value::as_object) { // ~/.claude.json: per project
        for (path, p) in ps { if let Some(m) = p.get("mcpServers").filter(|m| m.as_object().map_or(false, |o| !o.is_empty())) {
            let f = parse::json_doc(&serde_json::json!({"mcpServers": m}), &ids).map(|mut f| { for s in f.servers.iter_mut() { if s.name == s.id { s.name = format!("{} ({})", s.name, Path::new(path).file_name().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default()) } } f });
            add(f, &mut all, &mut ids)
        } }
    }
    all
}

/// Every source read, the servers already in the list marked (`here`: the id of the one it matches).
pub fn scan(home: &Path, roots: &[String], list: &List) -> Vec<(Scan, Vec<Option<String>>)> {
    let mut taken = list.ids();
    sources(home, roots).iter().map(|s| { let sc = read(s, &taken); taken.extend(sc.found.servers.iter().map(|x| x.id.clone()));
        let here = sc.found.servers.iter().map(|x| list.servers.iter().find(|y| y.same_as(x)).map(|y| y.id.clone())).collect(); (sc, here) }).collect()
}

/// Synced servers brought up to date with their files: the command, arguments, env, URL and headers follow the file; the person's choices (on,
/// scope, tools, name) stay. A server gone from its file is left as it is. True when anything changed.
pub fn sync(list: &mut List, scans: &[Scan]) -> bool {
    let mut changed = false;
    for s in list.servers.iter_mut().filter(|s| s.sync) {
        let Some(path) = s.source.strip_prefix("import:") else { continue };
        let Some(sc) = scans.iter().find(|x| x.path == path) else { continue };
        let name_base = |x: &Server| x.name.split(" (").next().unwrap_or("").to_string();
        let Some(n) = sc.found.servers.iter().find(|x| name_base(x) == name_base(s) || x.same_as(s)) else { continue };
        let before = s.clone();
        s.transport = n.transport.clone(); s.command = n.command.clone(); s.args = n.args.clone(); s.env = n.env.iter().map(|k| { let mut k = k.clone(); if k.secret { k.v = super::model::vault_name(&s.id, &k.k) } k }).collect(); s.cwd = n.cwd.clone();
        s.url = n.url.clone(); s.headers = n.headers.iter().map(|k| { let mut k = k.clone(); if k.secret { k.v = super::model::vault_name(&s.id, &k.k) } k }).collect();
        if *s != before { changed = true }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcpx::model::Kv;
    fn tmp(n: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("bp-mcpx-imp-{n}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }
    #[test] fn a_sample_home_is_found_and_read() {
        let h = tmp("home"); let proj = h.join("proj"); std::fs::create_dir_all(proj.join(".vscode")).unwrap(); std::fs::create_dir_all(h.join(".codex")).unwrap(); std::fs::create_dir_all(h.join(".config/Claude")).unwrap();
        std::fs::write(h.join(".claude.json"), r#"{"numStartups":3,"mcpServers":{"fetch":{"type":"stdio","command":"uvx","args":["mcp-server-fetch"],"env":{}}},
            "projects":{"/home/x/board":{"allowedTools":[],"mcpServers":{"kicad":{"command":"python3","args":["-m","kicad_mcp"],"env":{"KICAD_TOKEN":"real-token-123"}}}},"/home/x/empty":{"mcpServers":{}}}}"#).unwrap();
        std::fs::write(h.join(".codex/config.toml"), "model = \"gpt-6\"\n[mcp_servers.everything]\ncommand = \"npx\"\nargs = [\"-y\", \"@modelcontextprotocol/server-everything\"]\n[mcp_servers.fetch]\ncommand = \"uvx\"\nargs = [\"mcp-server-fetch\"]\n").unwrap();
        std::fs::write(h.join(".config/Claude/claude_desktop_config.json"), r#"{"mcpServers":{"fs":{"command":"npx","args":["-y","@modelcontextprotocol/server-filesystem","/home/x"]}}}"#).unwrap();
        std::fs::write(proj.join(".vscode/mcp.json"), "{ // comment\n \"servers\": {\"docs\": {\"type\": \"http\", \"url\": \"https://d.example/mcp\"},},}").unwrap();
        std::fs::write(proj.join(".mcp.json"), "not json").unwrap();
        let src = sources(&h, &[proj.to_string_lossy().into_owned()]);
        let tools: Vec<&str> = src.iter().map(|s| s.tool).collect(); assert_eq!(tools, ["Claude Code", "Codex", "Claude Desktop", "Claude Code", "VS Code"]);
        let mut list = List::default(); list.servers.push(Server { id: "fetch".into(), transport: "stdio".into(), command: "uvx".into(), args: vec!["mcp-server-fetch".into()], enabled: true, ..Default::default() });
        let sc = scan(&h, &[proj.to_string_lossy().into_owned()], &list);
        let claude = &sc[0]; assert_eq!(claude.0.found.servers.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["fetch", "kicad (board)"]);
        assert_eq!(claude.1, vec![Some("fetch".to_string()), None], "fetch is already here");
        assert_eq!(claude.0.found.servers[0].id, "fetch-2", "an import never takes an id in use");
        assert_eq!(claude.0.found.servers[1].env, vec![Kv::sec("KICAD_TOKEN", "mcp-kicad-kicad_token", "")]); assert_eq!(claude.0.found.plain[0].1, "real-token-123");
        assert!(claude.0.found.servers[1].source.ends_with(".claude.json"));
        let codex = &sc[1]; assert_eq!(codex.0.found.servers.len(), 2); assert_eq!(codex.1, vec![None, Some("fetch".to_string())]);
        assert_eq!(sc[2].0.found.servers[0].args.last().unwrap(), "/home/x");
        assert!(!sc[3].0.error.is_empty(), "a broken .mcp.json says why");
        assert_eq!(sc[4].0.found.servers[0].transport, "http");
        let _ = std::fs::remove_dir_all(&h);
    }
    #[test] fn a_synced_server_follows_its_file_and_keeps_the_persons_choices() {
        let h = tmp("sync"); let f = h.join(".cursor/mcp.json"); std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, r#"{"mcpServers":{"docs":{"command":"npx","args":["-y","docs@1"]}}}"#).unwrap();
        let src = Source { tool: "Cursor", path: f.clone() }; let mut s = read(&src, &[]).found.servers.remove(0);
        s.sync = true; s.enabled = false; s.providers = vec!["codex".into()]; let mut list = List { servers: vec![s] };
        assert!(!sync(&mut list, &[read(&src, &[])]), "unchanged");
        std::fs::write(&f, r#"{"mcpServers":{"docs":{"command":"npx","args":["-y","docs@2"],"env":{"API_KEY":"${K}"}}}}"#).unwrap();
        assert!(sync(&mut list, &[read(&src, &[])]));
        let s = &list.servers[0]; assert_eq!(s.args, ["-y", "docs@2"]); assert!(!s.enabled && s.providers == ["codex"]); assert_eq!(s.env, vec![Kv::sec("API_KEY", "mcp-docs-api_key", "")]);
        let _ = std::fs::remove_dir_all(&h);
    }
}
