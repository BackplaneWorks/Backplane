//! Settings > MCP, the hub's half: the page's rows for every client (a pure function of `info["mcp"]`, what is typed and the page's own
//! state), the presses that only move the page (`step`), and the presses that change the list (`act`). The desktop draws it through the presenter
//! (present/settings.rs, its drafts `@mcp.*` and `@f.m*`); the phones ask `mcp.page {ui, fields}` and press `mcp.page.act {action, value, ui, fields}`
//! and keep the `ui` they are given back.
//!
//! Rows are the usual `{label, note, buttons, fields}` plus, for clients that draw them, `kind`:
//! - `card`: a server. `icon {text, bg}`, `title`, `sub` (its command or URL, monospace), `pill {text, tone}` (ok, warn, bad, busy, dim),
//!   `meta` (tool count, last use), `toggle {on, action, value}`, `press {action, value}` (opens it), `open` (drawn opened), `badge` ("Community").
//! - `tool`: one of a server's tools under its card: `label`, `note`, `toggle`.
//! - `code`: monospace text (`text`), a server's stderr.
//! - `group`: a small label over the cards that follow.
//! - `notice`: a line with a `tone`.
//! A field may have `lines` (a box of that many lines: the paste box).
use super::{gallery, model::{Kv, Server}, parse};
use serde_json::{json, Map, Value};

/// What the page shows and what is typed: `@mcp.view` etc. on the desktop, `ui` + `fields` from a phone.
pub struct Ui<'a> { pub get: &'a (dyn Fn(&str) -> Option<String> + Sync) }
impl<'a> Ui<'a> {
    fn s(&self, k: &str) -> String { (self.get)(k).unwrap_or_default() }
    fn has(&self, k: &str) -> bool { (self.get)(k).is_some() }
}
/// The page's own state keys (not sent to the hub as a change) and the typed fields.
pub const UI_KEYS: [&str; 8] = ["view", "mode", "rtrans", "rm", "skip", "imp", "isync", "err"];
pub const FIELDS: [&str; 9] = ["mpaste", "mname", "mcmd", "menv", "mcwd", "murl", "mhdr", "mgarg", "msec"];

fn btn(label: &str, action: &str, value: &str, on: bool) -> Value { json!({"label": label, "action": action, "value": value, "on": on}) }
fn row(label: &str, note: &str, buttons: Vec<Value>) -> Value { json!({"label": label, "note": note, "buttons": buttons}) }
fn field(name: &str, hint: &str, value: &str, lines: u32) -> Value { let mut f = json!({"name": name, "hint": hint, "secret": false, "value": value}); if lines > 1 { f["lines"] = json!(lines) } f }
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn plural(n: usize, one: &str) -> String { format!("{n} {one}{}", if n == 1 { "" } else { "s" }) }
pub fn ago(at: u64, now: u64) -> String {
    let d = now.saturating_sub(at);
    if d < 60 { "just now".into() } else if d < 3600 { format!("{} min ago", d / 60) } else if d < 86400 { format!("{} h ago", d / 3600) } else { format!("{} d ago", d / 86400) }
}

/// The tile a server is drawn with: its vendor's colour for the gallery's, a colour from its name otherwise, and one or two letters.
pub fn icon(s: &Server) -> Value {
    let key = s.source.strip_prefix("gallery:").unwrap_or(&s.icon);
    let known = |k: &str| -> Option<(&'static str, &'static str)> { Some(match k {
        "microchip-docs" | "microchip-products" => ("Mc", "#c8102e"), "esp-components" | "espressif-docs" => ("Es", "#e7352c"), "stm32-docs" => ("ST", "#03234b"),
        "nordic" => ("N", "#00a9ce"), "nxp-docs" => ("NX", "#f9b500"), "silabs" => ("SL", "#d91e2a"), "kicad-pro" => ("Ki", "#314cb0"), "deepwiki" => ("DW", "#1f6feb"),
        "context7" => ("C7", "#00c48c"), "github" => ("GH", "#24292f"), "fetch" | "filesystem" | "git" | "time" | "memory" | "everything" => ("", "#d97757"), _ => return None }) };
    let (text, bg) = known(key).unwrap_or(("", ""));
    let text = if text.is_empty() { s.title().chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into()) } else { text.to_string() };
    let bg = if bg.is_empty() { let h = s.id.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32)) % 360; hsl(h) } else { bg.to_string() };
    json!({"text": text, "bg": bg})
}
fn hsl(h: u32) -> String { // a muted colour of hue h (s 45%, l 42%)
    let (s, l) = (0.45f32, 0.42f32); let c = (1. - (2. * l - 1.).abs()) * s; let hp = h as f32 / 60.; let x = c * (1. - (hp % 2. - 1.).abs());
    let (r, g, b) = match hp as u32 { 0 => (c, x, 0.), 1 => (x, c, 0.), 2 => (0., c, x), 3 => (0., x, c), 4 => (x, 0., c), _ => (c, 0., x) };
    let m = l - c / 2.; format!("#{:02x}{:02x}{:02x}", ((r + m) * 255.) as u8, ((g + m) * 255.) as u8, ((b + m) * 255.) as u8)
}
fn gallery_icon(e: &gallery::Entry) -> Value { icon(&Server { id: e.id.into(), name: e.name.into(), source: format!("gallery:{}", e.id), ..Default::default() }) }
pub fn transport_label(t: &str) -> &'static str { match t { "stdio" => "stdio", "sse" => "SSE", _ => "HTTP" } }

/// A server's status: (pill text, tone).
pub fn status(x: &Value, machine: &str) -> (String, &'static str) {
    let srv = &x["server"]; let st = &x["st"]; let rep = &st["report"];
    if srv["enabled"] == false { return ("Off".into(), "dim") }
    if x["checking"] == true { return ("Checking".into(), "busy") }
    if x["secrets"].as_array().map_or(false, |a| a.iter().any(|k| k["set"] == false)) { return ("Needs a key".into(), "warn") }
    if x["found"] == false { return (format!("Not on {machine}"), "bad") }
    let rep_at = rep["at"].as_u64().unwrap_or(0);
    if st["err_at"].as_u64().unwrap_or(0) > rep_at && !s(st, "err").is_empty() { return ("Error".into(), "bad") }
    if rep.is_object() { if rep["ok"] == true { ("Ready".into(), "ok") } else { ("Error".into(), "bad") } } else { ("Not checked".into(), "dim") }
}

/// The page.
pub fn rows(info: &Value, ui: &Ui, now: u64) -> Vec<Value> {
    let view = ui.s("view");
    let servers: Vec<Value> = info["servers"].as_array().cloned().unwrap_or_default();
    let machine = s(info, "machine");
    let mut v = vec![];
    let n = servers.len();
    let lead = if n == 0 { "Every agent Backplane runs gets the servers here: Claude, Codex, Antigravity, Grok, OpenCode. Changes apply from each thread's next turn.".to_string() }
        else { format!("{} for every agent Backplane runs. Changes apply from each thread's next turn.", plural(n, "server")) };
    v.push(json!({"kind": "head", "label": "MCP servers", "note": lead, "buttons": [btn("Add", "mcp-view", if view == "add" { "" } else { "add" }, view == "add"), btn("Import", "mcp-view", if view == "import" { "" } else { "import" }, view == "import"), btn("Gallery", "mcp-view", if view == "gallery" { "" } else { "gallery" }, view == "gallery")], "primary": 0}));
    let err = { let e = ui.s("err"); if e.is_empty() { s(info, "notice").to_string() } else { e } }; if !err.is_empty() { v.push(json!({"kind": "notice", "label": err, "note": "", "tone": "bad", "buttons": [btn("Dismiss", "mcp-err", "", false)]})) }
    match view.as_str() { "add" => add_rows(&mut v, info, ui), "import" => import_rows(&mut v, info, ui), "gallery" => gallery_rows(&mut v, info, ui), _ => {} }
    if let Some(id) = view.strip_prefix("edit:") { if let Some(x) = servers.iter().find(|x| x["server"]["id"] == id) { edit_rows(&mut v, &x["server"], ui) } }

    // the servers
    v.push(json!({"kind": "group", "label": "Servers", "note": "", "buttons": []}));
    if servers.is_empty() && view.is_empty() {
        v.push(json!({"kind": "empty", "label": "No servers of yours yet", "note": "Paste the snippet a vendor's docs give, add one from the gallery, or bring in what Claude Code, Codex, Cursor or VS Code already have.",
            "buttons": [btn("Paste a config", "mcp-view", "add", false), btn("Gallery", "mcp-view", "gallery", false), btn("Import", "mcp-view", "import", false)]}));
    }
    for x in &servers { card_rows(&mut v, x, info, ui, now, machine) }
    v.push(json!({"kind": "group", "label": "Built in", "note": "", "buttons": []}));
    v.push(json!({"kind": "card", "label": "Backplane", "note": "Built in. Threads, viewers, KiCad, secrets, work and locks.", "title": "Backplane", "sub": "Built in", "icon": {"text": "B", "bg": "#2f6f5e"}, "compact": true,
        "pill": {"text": "Always on", "tone": "ok"}, "meta": "", "buttons": []}));
    let pcb = info["pcbparts"] != false;
    v.push(json!({"kind": "card", "label": "pcbparts", "note": "Parts search: the fab's stock, Mouser, DigiKey.", "title": "pcbparts", "sub": "https://pcbparts.dev/mcp", "icon": {"text": "P", "bg": "#5a6b2f"}, "compact": true,
        "pill": if pcb { json!({"text": "On", "tone": "ok"}) } else { json!({"text": "Off", "tone": "dim"}) }, "meta": "HTTP · built in",
        "toggle": {"on": pcb, "action": "mcp-pcbparts", "value": if pcb { "off" } else { "on" }}, "buttons": [btn(if pcb { "on" } else { "off" }, "mcp-pcbparts", if pcb { "off" } else { "on" }, pcb)]}));

    // what agents may do
    let projects = info["projects"].as_array().cloned().unwrap_or_default();
    if !projects.is_empty() {
        v.push(json!({"kind": "group", "label": "Agents adding servers", "note": "", "buttons": []}));
        let allow: Vec<&str> = info["allow"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        v.push(row("Ask me first", "An agent can add, change or turn on a server with its mcp_ tools. A server runs code on this machine, so each one waits for your one-tap OK unless its project allows it.", vec![]));
        for p in projects.iter().take(40) {
            let (id, title) = (p[0].as_str().unwrap_or(""), p[1].as_str().unwrap_or(""));
            let on = allow.contains(&id);
            v.push(row(title, if on { "Its agents add servers without asking." } else { "Its agents ask first." }, vec![btn("ask", "mcp-allow", &format!("{id}=ask"), !on), btn("allow", "mcp-allow", &format!("{id}=allow"), on)]));
        }
    }
    v
}

fn card_rows(v: &mut Vec<Value>, x: &Value, info: &Value, ui: &Ui, now: u64, machine: &str) {
    let srv = &x["server"]; let st = &x["st"]; let rep = &st["report"];
    let server: Server = serde_json::from_value(srv.clone()).unwrap_or_default();
    let id = server.id.as_str();
    let (pill, tone) = status(x, machine);
    let mut meta = vec![transport_label(&server.transport).to_string()];
    if rep["ok"] == true { let t = rep["tools"].as_array().map_or(0, Vec::len); let off = server.disabled_tools.len(); meta.push(if off > 0 { format!("{} of {}", t.saturating_sub(off), plural(t, "tool")) } else { plural(t, "tool") });
        let r = rep["resources"].as_u64().unwrap_or(0) as usize; if r > 0 { meta.push(plural(r, "resource")) } let p = rep["prompts"].as_u64().unwrap_or(0) as usize; if p > 0 { meta.push(plural(p, "prompt")) } }
    if st["used_at"].as_u64().unwrap_or(0) > 0 { meta.push(format!("used {}", ago(st["used_at"].as_u64().unwrap_or(0), now))) }
    let scope = scope_line(&server, info);
    if !scope.is_empty() { meta.push(scope) }
    let open = ui.s("view") == format!("open:{id}") || ui.s("view").starts_with(&format!("secret:{id}:"));
    let community = server.source.strip_prefix("gallery:").and_then(gallery::get).map_or(false, |e| e.community);
    v.push(json!({"kind": "card", "id": id, "label": server.title(), "note": format!("{pill} · {}", meta.join(" · ")), "title": server.title(), "sub": server.line(), "icon": icon(&server),
        "pill": {"text": pill, "tone": tone}, "meta": meta.join(" · "), "open": open, "badge": if community { "Community" } else { "" },
        "press": {"action": "mcp-view", "value": if open { String::new() } else { format!("open:{id}") }},
        "toggle": {"on": server.enabled, "action": "mcp-toggle", "value": id},
        "buttons": [btn(if server.enabled { "on" } else { "off" }, "mcp-toggle", id, server.enabled), btn(if open { "Close" } else { "Open" }, "mcp-view", &if open { String::new() } else { format!("open:{id}") }, open)]}));
    if !open { return }
    // opened: secrets, the check, scope, tools, the actions, drawn inside the card ("in")
    let from = v.len();
    for k in x["secrets"].as_array().into_iter().flatten() {
        let (key, name, set) = (s(k, "key"), s(k, "name"), k["set"] == true);
        if set { v.push(json!({"label": format!("{key}"), "note": format!("In the vault as {name}. Its value is put in when an agent starts the server."), "tone": "", "buttons": [btn("Replace", "mcp-view", &format!("secret:{id}:{name}"), false)]})) }
        if !set || ui.s("view") == format!("secret:{id}:{name}") {
            v.push(json!({"label": key, "note": format!("{} Typed here, it goes straight to the vault as {name}; it is never written to a config file.", if set { "A new value." } else { "This server needs it." }),
                "fields": [{"name": "msec", "hint": format!("{key}"), "secret": true, "vault": true, "value": ""}], "buttons": [btn("Save to vault", "secret-save", &format!("{name}\u{1f}MCP server {id}: {key}\u{1f}"), true)]}));
        }
    }
    if x["found"] == false && server.transport == "stdio" { v.push(json!({"kind": "notice", "label": format!("{} is not found on {machine}.", server.command), "note": "Agents run it where they run. Install it, or give its full path in Edit.", "tone": "bad", "buttons": []})) }
    let err = s(st, "err"); let err_at = st["err_at"].as_u64().unwrap_or(0);
    if rep.is_object() {
        let at = rep["at"].as_u64().unwrap_or(0);
        if rep["ok"] == true {
            let name = s(rep, "server"); let ver = s(rep, "version");
            v.push(json!({"kind": "notice", "label": format!("Answered in {} ms{}", rep["ms"].as_u64().unwrap_or(0), if name.is_empty() { String::new() } else { format!(" as {name}{}", if ver.is_empty() { String::new() } else { format!(" {ver}") }) }), "note": format!("Checked {}", ago(at, now)), "tone": "ok", "buttons": []}));
        } else { v.push(json!({"kind": "notice", "label": s(rep, "error"), "note": format!("Checked {}", ago(at, now)), "tone": "bad", "buttons": []})) }
    }
    if err_at > rep["at"].as_u64().unwrap_or(0) && !err.is_empty() { v.push(json!({"kind": "notice", "label": err, "note": format!("While an agent used it, {}", ago(err_at, now)), "tone": "bad", "buttons": []})) }
    let stderr = if !s(st, "stderr").is_empty() && err_at > rep["at"].as_u64().unwrap_or(0) { s(st, "stderr") } else { s(rep, "stderr") };
    if !stderr.is_empty() { v.push(json!({"kind": "code", "label": "stderr", "note": "", "text": stderr, "buttons": []})) }
    if st["used_at"].as_u64().unwrap_or(0) > 0 { v.push(row("Last used", &format!("{} by an agent ({}), {} in all", ago(st["used_at"].as_u64().unwrap_or(0), now), s(st, "used_tool"), plural(st["used_n"].as_u64().unwrap_or(0) as usize, "call")), vec![])) }
    // scope
    let mut provs = vec![btn("all", "mcp-scope", &format!("{id}|prov|"), server.providers.is_empty())];
    for p in ["claude", "codex", "antigravity", "grok", "opencode", "muse"] { provs.push(btn(p, "mcp-scope", &format!("{id}|prov|{p}"), server.providers.iter().any(|x| x == p))) }
    v.push(row("Agents", "Which providers get it.", provs));
    let projects = info["projects"].as_array().cloned().unwrap_or_default();
    if !projects.is_empty() {
        let mut b = vec![btn("all", "mcp-scope", &format!("{id}|proj|"), server.projects.is_empty())];
        for p in projects.iter().take(24) { let pid = p[0].as_str().unwrap_or(""); b.push(btn(p[1].as_str().unwrap_or(pid), "mcp-scope", &format!("{id}|proj|{pid}"), server.projects.iter().any(|x| x == pid))) }
        v.push(row("Projects", "Which projects' threads get it.", b));
    }
    let bots = info["bots"].as_array().cloned().unwrap_or_default();
    if !bots.is_empty() {
        let mut b = vec![btn("all", "mcp-scope", &format!("{id}|bot|*"), server.bots.is_none()), btn("none", "mcp-scope", &format!("{id}|bot|-"), server.bots.as_ref().map_or(false, Vec::is_empty))];
        for p in bots.iter().take(24) { let bid = p[0].as_str().unwrap_or(""); b.push(btn(p[1].as_str().unwrap_or(bid), "mcp-scope", &format!("{id}|bot|{bid}"), server.bots.as_ref().map_or(false, |x| x.iter().any(|y| y == bid)))) }
        v.push(row("Bots", "Which bots' agents get it.", b));
    }
    // tools
    let tools = rep["tools"].as_array().cloned().unwrap_or_default();
    if !tools.is_empty() {
        v.push(json!({"kind": "group", "label": format!("Tools ({})", tools.len()), "note": "", "buttons": []}));
        for t in &tools { let name = s(t, "name"); let on = !server.disabled_tools.iter().any(|d| d == name);
            v.push(json!({"kind": "tool", "label": name, "note": s(t, "description"), "toggle": {"on": on, "action": "mcp-tool", "value": format!("{id}|{name}")}, "buttons": [btn(if on { "on" } else { "off" }, "mcp-tool", &format!("{id}|{name}"), on)]})) }
    } else if rep["ok"] != true { v.push(row("Tools", "Test it to see its tools.", vec![])) }
    let rm = ui.s("rm") == id;
    v.push(json!({"kind": "actions", "label": "", "note": if rm { "Remove it from every agent? Press Remove again." } else { "" }, "buttons": [
        btn(if x["checking"] == true { "Testing" } else { "Test" }, "mcp-test", id, false), btn("Edit", "mcp-view", &format!("edit:{id}"), false), btn("Duplicate", "mcp-dup", id, false), btn(if rm { "Remove for good" } else { "Remove" }, "mcp-remove", id, rm)]}));
    for r in v[from..].iter_mut() { r["in"] = json!(true) }
}

/// A path with the home folder said as ~.
fn tilde(p: &str, home: &str) -> String { if !home.is_empty() && p.starts_with(home) { format!("~{}", &p[home.len()..]) } else { p.to_string() } }
fn scope_line(s: &Server, info: &Value) -> String {
    let mut p = vec![];
    if !s.providers.is_empty() { p.push(s.providers.join(", ")) }
    if !s.projects.is_empty() { let names: Vec<String> = s.projects.iter().map(|id| info["projects"].as_array().into_iter().flatten().find(|x| x[0] == id.as_str()).and_then(|x| x[1].as_str()).unwrap_or(id).to_string()).collect(); p.push(names.join(", ")) }
    if s.bots.as_ref().map_or(false, Vec::is_empty) { p.push("no bots".into()) }
    p.join(" · ")
}

/// What Add makes from the form, or why not (pure: the presenter and the hub both call it).
pub fn form_servers(ui: &Ui, taken: &[String]) -> Result<parse::Found, String> {
    match ui.s("mode").as_str() {
        "command" => {
            let words = parse::shell_words(&ui.s("mcmd"))?; if words.is_empty() { return Err("Type the command that starts it".into()) }
            let mut env = Map::new(); for l in ui.s("menv").lines().map(str::trim).filter(|l| !l.is_empty()) { let (k, x) = l.split_once('=').ok_or_else(|| format!("{l:?}: one KEY=value a line"))?; env.insert(k.trim().into(), json!(x.trim())); }
            let mut o = json!({"command": words[0], "args": words[1..].to_vec(), "env": env}); if !ui.s("mcwd").trim().is_empty() { o["cwd"] = json!(ui.s("mcwd").trim()) }
            let name = if ui.s("mname").trim().is_empty() { String::new() } else { ui.s("mname").trim().to_string() };
            let mut f = parse::json_doc(&if name.is_empty() { o } else { json!({"mcpServers": {name: o}}) }, taken)?; f.format = "command"; Ok(f)
        }
        "remote" => {
            let url = ui.s("murl").trim().to_string(); if url.is_empty() { return Err("Type the server's URL".into()) }
            let mut h = Map::new(); for l in ui.s("mhdr").lines().map(str::trim).filter(|l| !l.is_empty()) { let (k, x) = l.split_once(':').ok_or_else(|| format!("{l:?}: one Name: value a line"))?; h.insert(k.trim().into(), json!(x.trim())); }
            let ty = if ui.s("rtrans") == "sse" { "sse" } else { "http" };
            let o = json!({"type": ty, "url": url, "headers": h});
            let name = if ui.s("mname").trim().is_empty() { parse::host_name(&url) } else { ui.s("mname").trim().to_string() };
            let mut f = parse::json_doc(&json!({"mcpServers": {name: o}}), taken)?; f.format = "remote"; Ok(f)
        }
        _ => { let t = ui.s("mpaste"); if t.trim().is_empty() { Err(String::new()) } else { parse::paste(&t, taken) } }
    }
}
fn taken(info: &Value) -> Vec<String> { info["servers"].as_array().into_iter().flatten().filter_map(|x| x["server"]["id"].as_str().map(str::to_string)).collect() }
fn skip_set(ui: &Ui) -> Vec<usize> { ui.s("skip").split(',').filter_map(|x| x.parse().ok()).collect() }

fn preview(v: &mut Vec<Value>, f: &parse::Found, skip: &[usize], pick: bool) {
    for (i, s) in f.servers.iter().enumerate() {
        let on = !skip.contains(&i);
        let secrets: Vec<String> = s.env.iter().chain(s.headers.iter()).filter(|k| k.secret).map(|k| k.k.clone()).collect();
        let given: Vec<&String> = f.plain.iter().map(|(n, _)| n).collect();
        let sec_note = if secrets.is_empty() { String::new() } else { let with: Vec<&str> = s.env.iter().chain(s.headers.iter()).filter(|k| k.secret && given.contains(&&k.v)).map(|k| k.k.as_str()).collect();
            if with.is_empty() { format!("{} from the vault: you set {} after adding", secrets.join(", "), if secrets.len() == 1 { "it" } else { "them" }) } else { format!("{} goes to the vault, never into a config file", secrets.join(", ")) } };
        let mut card = json!({"kind": "card", "label": s.title(), "note": sec_note, "title": s.title(), "sub": s.line(), "icon": icon(s), "pill": {"text": transport_label(&s.transport), "tone": "dim"}, "meta": sec_note, "buttons": []});
        if pick && f.servers.len() > 1 { card["toggle"] = json!({"on": on, "action": "mcp-skip", "value": i.to_string(), "style": "check"}); card["buttons"] = json!([btn(if on { "add" } else { "skip" }, "mcp-skip", &i.to_string(), on)]) }
        v.push(card);
    }
}

fn add_rows(v: &mut Vec<Value>, info: &Value, ui: &Ui) {
    let mode = { let m = ui.s("mode"); if m.is_empty() { "paste".to_string() } else { m } };
    v.push(json!({"kind": "panel", "label": "Add a server", "note": "", "buttons": [btn("Paste", "mcp-mode", "paste", mode == "paste"), btn("Command", "mcp-mode", "command", mode == "command"), btn("Remote URL", "mcp-mode", "remote", mode == "remote")]}));
    match mode.as_str() {
        "command" => {
            v.push(json!({"label": "Name", "note": "What agents call it (mcp__name__tool).", "fields": [field("mname", "stm32-docs", &ui.s("mname"), 1)], "buttons": []}));
            v.push(json!({"label": "Command", "note": "As you would type it in a shell. It runs where the agent runs.", "fields": [field("mcmd", "npx -y @modelcontextprotocol/server-everything", &ui.s("mcmd"), 1)], "buttons": []}));
            v.push(json!({"label": "Environment", "note": "KEY=value, one a line. A key, token or password goes to the vault.", "fields": [field("menv", "API_KEY=…", &ui.s("menv"), 3)], "buttons": []}));
            v.push(json!({"label": "Folder", "note": "Where it starts. Empty: the thread's folder.", "fields": [field("mcwd", "~/work", &ui.s("mcwd"), 1)], "buttons": []}));
        }
        "remote" => {
            let sse = ui.s("rtrans") == "sse";
            v.push(json!({"label": "URL", "note": "Streamable HTTP, or the older SSE.", "fields": [field("murl", "https://example.com/mcp", &ui.s("murl"), 1)], "buttons": [btn("HTTP", "mcp-rtrans", "http", !sse), btn("SSE", "mcp-rtrans", "sse", sse)]}));
            v.push(json!({"label": "Name", "note": "Empty: from its address.", "fields": [field("mname", "docs", &ui.s("mname"), 1)], "buttons": []}));
            v.push(json!({"label": "Headers", "note": "Name: value, one a line. Authorization and API keys go to the vault. Sign-in (OAuth) servers are not supported yet.", "fields": [field("mhdr", "Authorization: Bearer …", &ui.s("mhdr"), 2)], "buttons": []}));
        }
        _ => {
            v.push(json!({"kind": "paste", "label": "Paste", "note": "A vendor's mcpServers JSON, a Codex [mcp_servers] table, VS Code or Cursor settings, a claude mcp add line or a command.",
                "fields": [field("mpaste", "{ \"mcpServers\": { … } }", &ui.s("mpaste"), 7)], "buttons": []}));
        }
    }
    let taken = taken(info);
    match form_servers(ui, &taken) {
        Ok(f) => {
            let skip = skip_set(ui); let n = f.servers.iter().enumerate().filter(|(i, _)| !skip.contains(i)).count();
            v.push(json!({"kind": "notice", "label": if mode == "paste" { format!("Found {} in {}", plural(f.servers.len(), "server"), f.format) } else { "Ready to add".to_string() }, "note": f.notes.join("; "), "tone": "ok", "buttons": []}));
            preview(v, &f, &skip, true);
            v.push(json!({"kind": "actions", "label": "", "note": "It is tested once added.", "buttons": [btn(&if n == 1 { "Add server".to_string() } else { format!("Add {n} servers") }, "mcp-add", "", true), btn("Cancel", "mcp-view", "", false)]}));
        }
        Err(e) if e.is_empty() => {}
        Err(e) => v.push(json!({"kind": "notice", "label": e, "note": "", "tone": "warn", "buttons": []})),
    }
}

fn edit_rows(v: &mut Vec<Value>, srv: &Value, ui: &Ui) {
    let s: Server = serde_json::from_value(srv.clone()).unwrap_or_default();
    let val = |k: &str, cur: String| if ui.has(k) { ui.s(k) } else { cur };
    let kvs = |xs: &[Kv], sep: &str| xs.iter().map(|k| if k.secret { format!("{}{sep}{}${{vault:{}}}", k.k, k.pre, k.v) } else { format!("{}{sep}{}", k.k, k.v) }).collect::<Vec<_>>().join("\n");
    v.push(json!({"kind": "panel", "label": format!("Edit {}", s.title()), "note": "A value written ${vault:name} is the vault's secret of that name.", "buttons": []}));
    v.push(json!({"label": "Name", "note": "Shown on its card.", "fields": [field("mname", &s.id, &val("mname", s.title().to_string()), 1)], "buttons": []}));
    if s.remote() {
        v.push(json!({"label": "URL", "note": "", "fields": [field("murl", "https://…", &val("murl", s.url.clone()), 1)], "buttons": []}));
        v.push(json!({"label": "Headers", "note": "Name: value, one a line.", "fields": [field("mhdr", "", &val("mhdr", kvs(&s.headers, ": ")), 3)], "buttons": []}));
    } else {
        let cmd = std::iter::once(s.command.clone()).chain(s.args.iter().map(|a| if a.is_empty() || a.contains([' ', '"', '\'', '$']) { format!("'{}'", a.replace('\'', "'\\''")) } else { a.clone() })).collect::<Vec<_>>().join(" ");
        v.push(json!({"label": "Command", "note": "", "fields": [field("mcmd", "", &val("mcmd", cmd), 1)], "buttons": []}));
        v.push(json!({"label": "Environment", "note": "KEY=value, one a line.", "fields": [field("menv", "", &val("menv", kvs(&s.env, "=")), 3)], "buttons": []}));
        v.push(json!({"label": "Folder", "note": "", "fields": [field("mcwd", "", &val("mcwd", s.cwd.clone()), 1)], "buttons": []}));
    }
    v.push(json!({"kind": "actions", "label": "", "note": "", "buttons": [btn("Save", "mcp-save", &s.id, true), btn("Cancel", "mcp-view", &format!("open:{}", s.id), false)]}));
}

/// Edit's Save: the server with what was typed (a `${vault:name}` value stays a reference; a new secret-looking value goes to the vault: `plain`).
pub fn edited(old: &Server, ui: &Ui) -> Result<(Server, Vec<(String, String)>), String> {
    let mut s = old.clone(); let mut plain = vec![];
    if ui.has("mname") { let n = ui.s("mname").trim().to_string(); s.name = if n == s.id { String::new() } else { n } }
    let lines = |t: String, sep: char, what: &str| -> Result<Vec<(String, String)>, String> { t.lines().map(str::trim).filter(|l| !l.is_empty()).map(|l| l.split_once(sep).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).ok_or_else(|| format!("{l:?}: one {what} a line"))).collect() };
    let to_kv = |k: &str, v: &str, plain: &mut Vec<(String, String)>| -> Kv {
        if let Some((pre, rest)) = v.split_once("${vault:") { if let Some(name) = rest.strip_suffix('}') { return Kv::sec(k, name, pre) } }
        let probe = json!({k: v}); let mut p = vec![]; let kv = parse_kv(&s.id, k, &probe[k], &mut p); plain.extend(p); kv
    };
    if s.remote() {
        if ui.has("murl") { s.url = ui.s("murl").trim().into() }
        if ui.has("mhdr") { s.headers = lines(ui.s("mhdr"), ':', "Name: value")?.iter().map(|(k, v)| to_kv(k, v, &mut plain)).collect() }
    } else {
        if ui.has("mcmd") { let w = parse::shell_words(&ui.s("mcmd"))?; if w.is_empty() { return Err("Type the command that starts it".into()) } s.command = w[0].clone(); s.args = w[1..].to_vec() }
        if ui.has("menv") { s.env = lines(ui.s("menv"), '=', "KEY=value")?.iter().map(|(k, v)| to_kv(k, v, &mut plain)).collect() }
        if ui.has("mcwd") { s.cwd = ui.s("mcwd").trim().into() }
    }
    s.problem().map_or(Ok((s, plain)), Err)
}
fn parse_kv(id: &str, k: &str, v: &Value, plain: &mut Vec<(String, String)>) -> Kv {
    // the same rule as a paste: a secret-looking name or a placeholder is a vault reference
    let f = parse::json_doc(&json!({"mcpServers": {id: {"command": "x", "env": {k: v}}}}), &[]).ok();
    match f { Some(mut f) => { plain.extend(f.plain.drain(..)); f.servers.pop().and_then(|s| s.env.into_iter().next()).map(|mut kv| { if kv.secret { kv.v = super::model::vault_name(id, k) } kv }).unwrap_or_else(|| Kv::lit(k, v.as_str().unwrap_or(""))) }
        None => Kv::lit(k, v.as_str().unwrap_or("")) }
}

fn import_rows(v: &mut Vec<Value>, info: &Value, ui: &Ui) {
    let scan = info["scan"].as_array().cloned();
    v.push(json!({"kind": "panel", "label": "Import from your tools", "note": "Claude Code, Codex, Cursor, VS Code, Claude Desktop, Windsurf, Gemini CLI and OpenCode, read where they keep their servers. Nothing of theirs is changed.", "buttons": [btn("Look again", "mcp-scan", "", false)]}));
    let Some(scan) = scan else { v.push(json!({"kind": "notice", "label": "Looking…", "note": "", "tone": "busy", "buttons": []})); return };
    let skipped: Vec<String> = ui.s("imp").split(',').filter(|x| !x.is_empty()).map(str::to_string).collect(); // every new one is picked until it is unticked
    let mut any = false; let mut n = 0;
    for src in &scan {
        let servers = src["servers"].as_array().cloned().unwrap_or_default();
        if servers.is_empty() && s(src, "error").is_empty() { continue }
        v.push(json!({"kind": "group", "label": s(src, "tool"), "note": tilde(s(src, "path"), s(info, "home")), "buttons": []}));
        if !s(src, "error").is_empty() { v.push(json!({"kind": "notice", "label": format!("Could not read it: {}", s(src, "error")), "note": "", "tone": "warn", "buttons": []})) }
        for (i, x) in servers.iter().enumerate() {
            any = true;
            let srv: Server = serde_json::from_value(x["server"].clone()).unwrap_or_default();
            let key = format!("{}|{i}", s(src, "path")); let here = x["here"].as_str();
            let on = here.is_none() && !skipped.contains(&key); if on { n += 1 }
            let mut card = json!({"kind": "card", "label": srv.title(), "note": "", "title": srv.title(), "sub": srv.line(), "icon": icon(&srv), "pill": {"text": transport_label(&srv.transport), "tone": "dim"},
                "meta": match here { Some(h) => format!("Already here as {h}"), None => if x["secret"].as_u64().unwrap_or(0) > 0 { "Its keys go to the vault".to_string() } else { String::new() } }, "buttons": []});
            if here.is_none() { card["toggle"] = json!({"on": on, "action": "mcp-imp", "value": key, "style": "check"}); card["buttons"] = json!([btn(if on { "import" } else { "skip" }, "mcp-imp", &key, on)]) }
            v.push(card);
        }
    }
    if !any { v.push(json!({"kind": "empty", "label": "Nothing to import", "note": "None of those tools has an MCP server set up on this machine.", "buttons": []})); return }
    let sync = ui.s("isync") != "off";
    v.push(json!({"kind": "actions", "label": "", "note": "Keep in sync: an imported server follows changes to its file; your on/off, scope and tools stay yours.", "buttons": [btn(&if n == 1 { "Import 1 server".to_string() } else { format!("Import {n} servers") }, "mcp-import", "", n > 0), btn("Keep in sync", "mcp-isync", if sync { "off" } else { "on" }, sync), btn("Cancel", "mcp-view", "", false)]}));
}

/// The scan's servers to import: every one not already here, less those unticked (`imp`).
pub fn import_picks(scan: &Value, skipped: &str) -> Vec<String> {
    let skip: Vec<&str> = skipped.split(',').filter(|x| !x.is_empty()).collect();
    scan.as_array().into_iter().flatten().flat_map(|src| src["servers"].as_array().into_iter().flatten().enumerate().filter(|(_, x)| x["here"].is_null()).map(move |(i, _)| format!("{}|{i}", s(src, "path")))).filter(|k| !skip.contains(&k.as_str())).collect()
}
fn gallery_rows(v: &mut Vec<Value>, info: &Value, ui: &Ui) {
    let added: Vec<&str> = info["gallery_added"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    v.push(json!({"kind": "panel", "label": "Gallery", "note": "Checked to exist, one press to add. Community means it is not made by the vendor.", "buttons": []}));
    let mut group = "";
    for e in gallery::ENTRIES {
        if e.group != group { group = e.group; v.push(json!({"kind": "group", "label": group, "note": "", "buttons": []})) }
        let here = added.contains(&e.id);
        let (pill, tone) = if e.signin { ("Needs sign-in", "dim") } else if here { ("Added", "ok") } else { (transport_label(if e.remote() { "http" } else { "stdio" }), "dim") };
        let mut b = vec![];
        if !e.signin && e.needs.is_empty() && !here { b.push(btn("Add", "mcp-gallery", e.id, true)) }
        b.push(btn("Docs", "open-url", e.home, false));
        let mut card = json!({"kind": "card", "label": e.name, "note": format!("{} · {}", e.by, e.about), "title": e.name, "sub": format!("{} · {}", e.by, e.about), "icon": gallery_icon(e),
            "pill": {"text": pill, "tone": tone}, "meta": if e.signin { "OAuth sign-in is not supported yet".to_string() } else if let Some((k, _, req)) = e.secret { format!("{k} {}", if req { "from the vault" } else { "optional, from the vault" }) } else { String::new() },
            "badge": if e.community { "Community" } else { "" }, "buttons": b});
        if !e.needs.is_empty() { card["fields"] = json!([field("mgarg", e.needs, &ui.s("mgarg"), 1)]); card["buttons"].as_array_mut().unwrap().insert(0, btn("Add", "mcp-gallery", e.id, true)) }
        v.push(card);
    }
}

/// The presses that only move the page: the new page state, or None for a press that changes the list.
pub fn step(ui: &Ui, action: &str, value: &str) -> Option<Map<String, Value>> {
    let mut m = Map::new(); let mut put = |k: &str, v: &str| { m.insert(k.into(), json!(v)); };
    match action {
        "mcp-view" => { put("view", value); put("rm", ""); put("err", ""); if value.starts_with("edit:") || value.is_empty() || value == "add" { for f in ["mname", "mcmd", "menv", "mcwd", "murl", "mhdr"] { m.insert(f.into(), Value::Null); } } }
        "mcp-mode" => { put("mode", value); put("skip", "") }
        "mcp-rtrans" => put("rtrans", value),
        "mcp-err" => put("err", ""),
        "mcp-isync" => put("isync", value),
        "mcp-skip" => { let mut s = skip_set(ui); let i: usize = value.parse().ok()?; if let Some(p) = s.iter().position(|x| *x == i) { s.remove(p); } else { s.push(i) } put("skip", &s.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")) }
        "mcp-imp" => { let mut p: Vec<String> = ui.s("imp").split(',').filter(|x| !x.is_empty()).map(str::to_string).collect(); if let Some(i) = p.iter().position(|x| x == value) { p.remove(i); } else { p.push(value.into()) } put("imp", &p.join(",")) }
        "mcp-remove" if ui.s("rm") != value => put("rm", value),
        _ => return None,
    }
    Some(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    fn ui<'a>(m: &'a HashMap<&'a str, &'a str>) -> impl Fn(&str) -> Option<String> + Sync + 'a { move |k| m.get(k).map(|x| x.to_string()) }
    fn info() -> Value {
        let s = Server { id: "fetch".into(), transport: "stdio".into(), command: "uvx".into(), args: vec!["mcp-server-fetch".into()], enabled: true, ..Default::default() };
        let g = Server { id: "gh".into(), name: "GitHub".into(), transport: "http".into(), url: "https://api.githubcopilot.com/mcp/".into(), headers: vec![Kv::sec("Authorization", "mcp-gh-authorization", "Bearer ")], enabled: true, source: "gallery:github".into(), ..Default::default() };
        json!({"machine": "desk", "pcbparts": true, "projects": [["p1", "Board"]], "bots": [["b1", "Scout"]], "allow": [], "scan": null, "gallery_added": ["github"], "servers": [
            {"server": s, "st": {"report": {"ok": true, "at": 100, "ms": 900, "server": "fetch", "tools": [{"name": "fetch", "description": "Fetches"}], "resources": 0, "prompts": 0}, "used_at": 150, "used_tool": "fetch", "used_n": 2}, "checking": false, "secrets": [], "found": true},
            {"server": g, "st": {}, "checking": false, "secrets": [{"key": "Authorization", "name": "mcp-gh-authorization", "set": false}], "found": true}]})
    }
    fn kinds(rows: &[Value]) -> Vec<String> { rows.iter().map(|r| format!("{}:{}", s(r, "kind"), s(r, "label"))).collect() }
    #[test] fn the_list_shows_built_ins_then_cards_with_their_status() {
        let m = HashMap::new(); let get = ui(&m); let r = rows(&info(), &Ui { get: &get }, 160);
        let k = kinds(&r); assert_eq!(&k[..7], ["head:MCP servers", "group:Servers", "card:fetch", "card:GitHub", "group:Built in", "card:Backplane", "card:pcbparts"]);
        assert_eq!(r[2]["pill"], json!({"text":"Ready","tone":"ok"})); assert_eq!(r[2]["meta"], "stdio · 1 tool · used just now"); assert_eq!(r[2]["sub"], "uvx mcp-server-fetch");
        assert_eq!(r[3]["pill"], json!({"text":"Needs a key","tone":"warn"})); assert_eq!(r[3]["icon"], json!({"text":"GH","bg":"#24292f"}));
        assert!(r.iter().all(|x| x["label"].is_string() && x["buttons"].is_array()), "every row also reads as a plain row for an older client");
        assert!(k.contains(&"group:Agents adding servers".to_string()));
    }
    #[test] fn an_opened_card_shows_secrets_scope_tools_and_actions() {
        let m: HashMap<&str, &str> = [("view", "open:gh")].into(); let get = ui(&m); let r = rows(&info(), &Ui { get: &get }, 160);
        let sec = r.iter().find(|x| x["fields"][0]["vault"] == true).unwrap(); assert_eq!(sec["buttons"][0]["action"], "secret-save"); assert!(sec["buttons"][0]["value"].as_str().unwrap().starts_with("mcp-gh-authorization\u{1f}"));
        assert!(r.iter().any(|x| x["label"] == "Agents" && x["buttons"][0]["on"] == true)); assert!(r.iter().any(|x| x["label"] == "Bots"));
        let m: HashMap<&str, &str> = [("view", "open:fetch")].into(); let get = ui(&m); let r = rows(&info(), &Ui { get: &get }, 160);
        let t = r.iter().find(|x| x["kind"] == "tool").unwrap(); assert_eq!((t["label"].as_str(), t["toggle"]["value"].as_str()), (Some("fetch"), Some("fetch|fetch")));
        assert!(r.iter().any(|x| x["kind"] == "actions" && x["buttons"][0]["action"] == "mcp-test" && x["in"] == true), "the details are drawn inside the card");
    }
    #[test] fn add_parses_as_you_paste() {
        let m: HashMap<&str, &str> = [("view", "add"), ("mpaste", r#"{"mcpServers":{"a":{"command":"x"},"b":{"url":"https://b.example/mcp"}}}"#), ("skip", "1")].into(); let get = ui(&m);
        let r = rows(&info(), &Ui { get: &get }, 0); let k = kinds(&r);
        assert!(k.contains(&"notice:Found 2 servers in mcpServers".to_string()), "{k:?}");
        let add = r.iter().find(|x| x["kind"] == "actions").unwrap(); assert_eq!(add["buttons"][0]["label"], "Add server");
        let m: HashMap<&str, &str> = [("view", "add"), ("mpaste", "{ broken")].into(); let get = ui(&m); let r = rows(&info(), &Ui { get: &get }, 0);
        assert!(r.iter().any(|x| x["kind"] == "notice" && x["tone"] == "warn"));
        let m: HashMap<&str, &str> = [("view", "add"), ("mode", "command"), ("mcmd", "npx -y @modelcontextprotocol/server-everything"), ("menv", "API_KEY=sk-1234567890")].into(); let get = ui(&m);
        let f = form_servers(&Ui { get: &get }, &[]).unwrap(); assert_eq!(f.servers[0].id, "everything"); assert_eq!(f.plain.len(), 1);
        let m: HashMap<&str, &str> = [("mode", "remote"), ("murl", "https://mcp.deepwiki.com/mcp")].into(); let get = ui(&m); assert_eq!(form_servers(&Ui { get: &get }, &[]).unwrap().servers[0].id, "deepwiki");
    }
    #[test] fn empty_state_offers_three_ways_in() {
        let m = HashMap::new(); let get = ui(&m); let mut i = info(); i["servers"] = json!([]); i["projects"] = json!([]);
        let r = rows(&i, &Ui { get: &get }, 0); let e = r.iter().find(|x| x["kind"] == "empty").unwrap(); assert_eq!(e["buttons"].as_array().unwrap().len(), 3);
    }
    #[test] fn gallery_and_import_views() {
        let m: HashMap<&str, &str> = [("view", "gallery")].into(); let get = ui(&m); let r = rows(&info(), &Ui { get: &get }, 0);
        let gh = r.iter().find(|x| x["kind"] == "card" && x["title"] == "GitHub" && x["sub"].as_str().unwrap().starts_with("GitHub")).unwrap(); assert_eq!(gh["pill"]["text"], "Added");
        assert!(r.iter().any(|x| x["title"] == "Nordic Semiconductor" && x["pill"]["text"] == "Needs sign-in"));
        assert!(r.iter().any(|x| x["title"] == "STM32 Docs" && x["badge"] == "Community"));
        let mut i = info(); i["scan"] = json!([{"tool": "Codex", "path": "/h/.codex/config.toml", "error": "", "servers": [{"server": Server { id: "x".into(), transport: "stdio".into(), command: "x".into(), ..Default::default() }, "here": null, "secret": 0}, {"server": Server { id: "f2".into(), transport: "stdio".into(), command: "uvx".into(), ..Default::default() }, "here": "fetch", "secret": 0}]}]);
        let m: HashMap<&str, &str> = [("view", "import")].into(); let get = ui(&m); let r = rows(&i, &Ui { get: &get }, 0);
        assert!(r.iter().any(|x| x["meta"] == "Already here as fetch")); assert_eq!(r.iter().find(|x| x["kind"] == "actions").unwrap()["buttons"][0]["label"], "Import 1 server");
    }
    #[test] fn steps_move_the_page_only() {
        let m: HashMap<&str, &str> = [("skip", "0,2")].into(); let get = ui(&m); let u = Ui { get: &get };
        assert_eq!(step(&u, "mcp-skip", "2").unwrap()["skip"], "0"); assert_eq!(step(&u, "mcp-skip", "1").unwrap()["skip"], "0,2,1");
        assert_eq!(step(&u, "mcp-remove", "fetch").unwrap()["rm"], "fetch");
        let m: HashMap<&str, &str> = [("rm", "fetch")].into(); let get = ui(&m); assert!(step(&Ui { get: &get }, "mcp-remove", "fetch").is_none(), "the second press removes");
        assert!(step(&u, "mcp-add", "").is_none() && step(&u, "mcp-toggle", "x").is_none());
    }
    #[test] fn edit_keeps_vault_references_and_moves_new_secrets_to_the_vault() {
        let old = Server { id: "a".into(), transport: "stdio".into(), command: "x".into(), env: vec![Kv::sec("TOKEN", "mcp-a-token", "")], enabled: true, ..Default::default() };
        let m: HashMap<&str, &str> = [("mcmd", "npx -y 'a b'"), ("menv", "TOKEN=${vault:mcp-a-token}\nAPI_KEY=sk-newvalue-123\nLOG=1")].into(); let get = ui(&m);
        let (s, plain) = edited(&old, &Ui { get: &get }).unwrap();
        assert_eq!((s.command.as_str(), s.args.clone()), ("npx", vec!["-y".to_string(), "a b".to_string()]));
        assert_eq!(s.env, vec![Kv::sec("TOKEN", "mcp-a-token", ""), Kv::sec("API_KEY", "mcp-a-api_key", ""), Kv::lit("LOG", "1")]); assert_eq!(plain, vec![("mcp-a-api_key".to_string(), "sk-newvalue-123".to_string())]);
        let m: HashMap<&str, &str> = [("mcmd", " ")].into(); let get = ui(&m); assert!(edited(&old, &Ui { get: &get }).is_err());
    }
}
