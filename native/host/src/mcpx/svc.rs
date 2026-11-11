//! The hub's MCP list in use: read and written here (one writer, the list's lock), what each server last did (its check, its last use, its
//! last failure: `<home>/mcp/state.json`), the vault's values put in for the wrapper and the checks, and `info["mcp"]` for the pages.
use super::{gallery, gen, health::{self, Report, Resolved}, import, model::{List, Server}};
use crate::{agents::Agents, hub::Hub, settings::vault};
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet}, path::PathBuf, sync::{Arc, Mutex, OnceLock, Weak}};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct St {
    #[serde(default)] pub report: Option<Report>,
    #[serde(default)] pub used_at: u64, #[serde(default)] pub used_tool: String, #[serde(default)] pub used_n: u64,
    #[serde(default)] pub err_at: u64, #[serde(default)] pub err: String, #[serde(default)] pub stderr: String,
}

pub struct Mcpx {
    pub hub: Arc<Hub>, ag: Weak<Agents>, pub home: PathBuf, pub user_home: PathBuf,
    list: Mutex<List>, st: Mutex<HashMap<String, St>>, checking: Mutex<HashSet<String>>,
    /// the last import scan, for the page
    scan: Mutex<Value>,
    /// the last refusal of a desktop press, said at the top of the page until the next press
    notice: Mutex<String>,
}
static ME: OnceLock<Arc<Mcpx>> = OnceLock::new();
pub fn get() -> Option<Arc<Mcpx>> { ME.get().cloned() }
/// This machine's name, for "runs on <name>".
pub fn machine() -> String { std::fs::read_to_string("/etc/hostname").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).or_else(|| std::env::var("HOSTNAME").ok()).or_else(|| std::env::var("COMPUTERNAME").ok()).unwrap_or_else(|| "this machine".into()) }
pub fn now_s() -> u64 { crate::hub::now_ms() / 1000 }
/// The setting that lets a project's agents add and turn on servers without asking.
pub fn allow_key(project: &str) -> String { format!("mcp.agents.{project}") }
pub const PCBPARTS_KEY: &str = "mcp.pcbparts";

/// Where a command is found for an agent of this hub (the PATH agents run with): its path, or None.
pub fn which(cmd: &str) -> Option<PathBuf> {
    let p = std::path::Path::new(cmd);
    if p.is_absolute() || cmd.contains('/') { return p.is_file().then(|| p.to_path_buf()) }
    crate::agents::proc::search_dirs().iter().flat_map(|d| [d.join(cmd), d.join(format!("{cmd}.exe")), d.join(format!("{cmd}.cmd"))]).find(|c| c.is_file())
}
fn path_env() -> Option<(String, String)> { std::env::join_paths(crate::agents::proc::search_dirs()).ok().map(|j| ("PATH".to_string(), j.to_string_lossy().into_owned())) }

pub fn start(hub: &Arc<Hub>, ag: &Arc<Agents>, user_home: PathBuf) -> Arc<Mcpx> {
    let home = hub.cfg.home.clone();
    let st: HashMap<String, St> = std::fs::read_to_string(home.join("mcp/state.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let m = Arc::new(Mcpx { hub: hub.clone(), ag: Arc::downgrade(ag), list: Mutex::new(List::load(&home)), home, user_home, st: Mutex::new(st), checking: Default::default(), scan: Mutex::new(Value::Null), notice: Default::default() });
    let _ = ME.set(m.clone());
    hub.register_rpc(Arc::new(super::act::PageRpc));
    ag.add_tools(Arc::new(super::tools::McpTools));
    { let m = m.clone(); tokio::spawn(async move { m.sync_imports().await; m.publish().await; }); }
    m
}

impl Mcpx {
    pub fn agents(&self) -> Option<Arc<Agents>> { self.ag.upgrade() }
    pub fn list(&self) -> List { self.list.lock().unwrap().clone() }
    pub fn st(&self, id: &str) -> St { self.st.lock().unwrap().get(id).cloned().unwrap_or_default() }
    pub fn checking(&self, id: &str) -> bool { self.checking.lock().unwrap().contains(id) }
    fn save_st(&self) { let t = serde_json::to_string(&*self.st.lock().unwrap()).unwrap_or_default(); let p = self.home.join("mcp/state.json"); let _ = std::fs::create_dir_all(self.home.join("mcp")); let tmp = p.with_extension("tmp"); if std::fs::write(&tmp, t).is_ok() { let _ = std::fs::rename(tmp, p); } }

    /// Changes the list: `f` edits a copy; it is kept only when every server is valid, then saved and shown.
    pub async fn change<T>(&self, f: impl FnOnce(&mut List) -> Result<T, String>) -> Result<T, String> {
        let r = {
            let mut l = self.list.lock().unwrap(); let mut copy = l.clone();
            let r = f(&mut copy)?;
            let mut seen = HashSet::new();
            for s in &copy.servers { if let Some(p) = s.problem() { return Err(format!("{}: {p}", s.title())) } if !seen.insert(s.id.clone()) { return Err(format!("{} is in the list twice", s.id)) } }
            copy.save(&self.home).map_err(|e| format!("could not save the MCP list: {e}"))?;
            *l = copy; r
        };
        { let ids = self.list().ids(); self.st.lock().unwrap().retain(|k, _| ids.contains(k)); }
        self.publish().await; Ok(r)
    }
    /// Stores secrets that came with a paste or an import; Err names the first the vault refused.
    pub fn store_secrets(&self, plain: &[(String, String)], purpose: &str, by: &str) -> Result<(), String> {
        for (n, v) in plain {
            let bad = crate::settings::secret::bad_value(v.trim()); if !bad.is_empty() { return Err(format!("{n}: {bad}")) }
            let r = vault::put(n, v.trim(), purpose, by); if !crate::settings::secret::saved_is(&r) { return Err(format!("the vault did not take {n}: {}", r.trim_start_matches("fail "))) }
        }
        Ok(())
    }
    pub async fn pcbparts(&self) -> bool { self.hub.setting(PCBPARTS_KEY).await.as_deref() != Some("off") }

    /// A server ready to start, the vault's values put in; Err lists the secrets the vault does not hold.
    pub fn resolve(&self, s: &Server) -> Result<Resolved, String> {
        let missing = s.missing(&|n| vault::person_get(n).is_some());
        if !missing.is_empty() { return Err(format!("the vault has no {} (set {} in Settings > MCP)", missing.join(", "), if missing.len() == 1 { "it" } else { "them" })) }
        let get = |n: &str| vault::person_get(n);
        let kv = |xs: &[super::model::Kv]| xs.iter().filter_map(|k| gen::value(k, &get).map(|v| (k.k.clone(), v))).collect::<Vec<_>>();
        let mut env = kv(&s.env); if !env.iter().any(|(k, _)| k == "PATH") { if let Some(p) = path_env() { env.push(p) } }
        let command = if s.transport == "stdio" { which(&s.command).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| s.command.clone()) } else { String::new() };
        Ok(Resolved { transport: s.transport.clone(), command, args: s.args.clone(), env, cwd: s.cwd.clone(), url: s.url.clone(), headers: kv(&s.headers) })
    }

    /// What a thread is, for the scope: (provider, project, bot).
    pub async fn thread_ctx(&self, thread: &str) -> (String, String, String) {
        let Some(ag) = self.agents() else { return Default::default() };
        let w = ag.world().await; let t = w.threads.get(thread);
        (t.map(|t| t.provider.clone()).unwrap_or_default(), t.map(|t| t.project.clone()).unwrap_or_default(), w.bot(thread).to_string())
    }
    /// The servers an agent of this thread gets now (launch).
    pub fn for_thread(&self, provider: &str, project: &str, bot: &str) -> Vec<Server> { self.list.lock().unwrap().for_thread(provider, project, bot).into_iter().cloned().collect() }
    /// The wrapper's ask (`backplane/mcp_spec`): only for a server this thread gets.
    pub async fn spec(&self, thread: &str, id: &str) -> Result<Value, String> {
        let (p, pr, b) = self.thread_ctx(thread).await;
        let s = self.for_thread(&p, &pr, &b).into_iter().find(|s| s.id == id).ok_or_else(|| format!("{id} is not one of this thread's MCP servers (Settings > MCP)"))?;
        if s.remote() { return Err(format!("{id} is a remote server")) }
        let r = self.resolve(&s).map_err(|e| { self.failed(id, &e, ""); e })?;
        Ok(json!({"command": r.command, "args": r.args, "env": r.env.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(), "cwd": r.cwd, "disabled_tools": s.disabled_tools}))
    }
    /// Claude's headersHelper (`backplane/mcp_headers`).
    pub async fn headers(&self, thread: &str, id: &str) -> Result<Value, String> {
        let (p, pr, b) = self.thread_ctx(thread).await;
        let s = self.for_thread(&p, &pr, &b).into_iter().find(|s| s.id == id).ok_or_else(|| format!("{id} is not one of this thread's MCP servers"))?;
        let r = self.resolve(&s)?; Ok(Value::Object(r.headers.into_iter().map(|(k, v)| (k, json!(v))).collect()))
    }
    pub fn used(&self, id: &str, tool: &str, ok: bool, err: &str) {
        { let mut m = self.st.lock().unwrap(); let e = m.entry(id.to_string()).or_default(); e.used_at = now_s(); e.used_tool = tool.into(); e.used_n += 1;
          if !ok { e.err_at = now_s(); e.err = format!("{tool}: {err}").chars().take(300).collect(); } }
        self.save_st(); self.publish_soon();
    }
    pub fn failed(&self, id: &str, err: &str, stderr: &str) {
        { let mut m = self.st.lock().unwrap(); let e = m.entry(id.to_string()).or_default(); e.err_at = now_s(); e.err = err.chars().take(300).collect(); e.stderr = health::tail(stderr); }
        self.save_st(); self.publish_soon();
    }
    fn publish_soon(&self) { if let Some(m) = get() { tokio::spawn(async move { m.publish().await }); } }

    /// The live check; its report is kept for the card.
    pub async fn test(self: &Arc<Self>, id: &str) -> Result<Report, String> {
        let s = self.list().get(id).cloned().ok_or_else(|| format!("no server {id}"))?;
        if !self.checking.lock().unwrap().insert(id.to_string()) { return Err("a check is already running".into()) }
        self.publish().await;
        let r = match self.resolve(&s) { Ok(res) => health::check(&res, now_s()).await, Err(e) => Report { at: now_s(), error: e, ..Default::default() } };
        self.checking.lock().unwrap().remove(id);
        { let mut m = self.st.lock().unwrap(); let e = m.entry(id.to_string()).or_default(); e.report = Some(r.clone()); if r.ok { e.err.clear(); e.stderr.clear(); } }
        self.save_st(); self.publish().await; Ok(r)
    }
    pub fn test_bg(self: &Arc<Self>, id: &str) { let (m, id) = (self.clone(), id.to_string()); tokio::spawn(async move { let _ = m.test(&id).await; }); }

    /// The desktop tools' configs read again; synced servers follow them.
    pub async fn scan_imports(&self) -> Vec<(import::Scan, Vec<Option<String>>)> {
        let roots: Vec<String> = match self.agents() { Some(ag) => ag.world().await.roots.values().cloned().collect(), None => vec![] };
        let (home, list) = (self.user_home.clone(), self.list());
        let sc = tokio::task::spawn_blocking(move || import::scan(&home, &roots, &list)).await.unwrap_or_default();
        *self.scan.lock().unwrap() = json!(sc.iter().map(|(s, here)| json!({"tool": s.tool, "path": s.path, "error": s.error,
            "servers": s.found.servers.iter().zip(here).map(|(x, h)| json!({"server": x, "here": h, "secret": x.secrets().len()})).collect::<Vec<_>>()})).collect::<Vec<_>>());
        sc
    }
    pub async fn sync_imports(&self) {
        if !self.list().servers.iter().any(|s| s.sync) { return }
        let sc = self.scan_imports().await; let scans: Vec<import::Scan> = sc.into_iter().map(|x| x.0).collect();
        let _ = self.change(|l| Ok(import::sync(l, &scans))).await;
    }
    pub fn set_notice(&self, n: &str) { *self.notice.lock().unwrap() = n.to_string() }
    pub fn last_scan(&self) -> Value { self.scan.lock().unwrap().clone() }

    /// `info["mcp"]`: everything the pages draw (no secret value: vault names and whether each is set).
    pub async fn info(&self) -> Value {
        let list = self.list();
        let (projects, bots) = match self.agents() { Some(ag) => { let w = ag.world().await;
            (w.projects.iter().filter(|(_, (_, gone))| !gone).map(|(id, (t, _))| json!([id, t])).collect::<Vec<_>>(), w.bots.iter().filter(|(_, (_, _, gone))| !gone).map(|(id, (n, _, _))| json!([id, n])).collect::<Vec<_>>()) }
            None => (vec![], vec![]) };
        let allow: Vec<Value> = { let mut v = vec![]; for p in &projects { let id = p[0].as_str().unwrap_or(""); if self.hub.setting(&allow_key(id)).await.as_deref() == Some("allow") { v.push(json!(id)) } } v };
        let servers: Vec<Value> = list.servers.iter().map(|s| {
            let st = self.st(&s.id);
            let secrets: Vec<Value> = s.env.iter().chain(s.headers.iter()).filter(|k| k.secret).map(|k| json!({"key": k.k, "name": k.v, "set": vault::person_get(&k.v).is_some()})).collect();
            let found = if s.remote() { true } else { which(&s.command).is_some() };
            json!({"server": s, "st": st, "checking": self.checking(&s.id), "secrets": secrets, "found": found})
        }).collect();
        let mut projects = projects; projects.sort_by(|a, b| a[1].as_str().cmp(&b[1].as_str()));
        let mut bots = bots; bots.sort_by(|a, b| a[1].as_str().cmp(&b[1].as_str()));
        json!({"servers": servers, "projects": projects, "bots": bots, "allow": allow, "pcbparts": self.pcbparts().await, "scan": self.last_scan(), "notice": self.notice.lock().unwrap().clone(),
            "gallery_added": list.servers.iter().filter_map(|s| s.source.strip_prefix("gallery:").map(str::to_string)).collect::<Vec<_>>(), "machine": machine()})
    }
    pub async fn publish(&self) { let v = self.info().await; self.hub.set_info("mcp", &v.to_string()).await; }

    /// Adds servers (secrets that came with them go to the vault first). Returns the ids.
    pub async fn add(&self, mut servers: Vec<Server>, plain: &[(String, String)], by: &str) -> Result<Vec<String>, String> {
        self.store_secrets(plain, "MCP server", by)?;
        let now = now_s(); for s in servers.iter_mut() { if s.added_at == 0 { s.added_at = now } }
        let ids = self.change(|l| { let mut ids = vec![]; for mut s in servers { s.id = super::model::unique(&s.id, &l.ids()); ids.push(s.id.clone()); l.servers.push(s) } Ok(ids) }).await?;
        if let Some(m) = get() { for id in &ids { m.test_bg(id) } }
        Ok(ids)
    }
    pub fn gallery_server(&self, id: &str, arg: &str) -> Result<Server, String> {
        let e = gallery::get(id).ok_or("no such gallery entry")?;
        if e.signin { return Err(format!("{} needs a sign-in Backplane cannot do yet", e.name)) }
        e.server(&self.list().ids(), arg, now_s()).ok_or_else(|| e.needs.to_string())
    }
}

/// What an agent process is started with: each provider's form of the thread's servers.
#[derive(Clone, Debug, Default)]
pub struct Launch { pub claude: String, pub disallowed: Vec<String>, pub acp: Value, pub codex_args: Vec<String>, pub codex_env: Vec<(String, String)>, pub fingerprint: String, pub pcbparts: bool }
/// The hub's own program, for the wrapper (a replaced binary's " (deleted)" is dropped: the new one is at the same path).
pub fn hub_exe() -> String { std::env::current_exe().map(|p| p.to_string_lossy().trim_end_matches(" (deleted)").to_string()).unwrap_or_default() }
/// The servers of `thread` (a `provider` agent) as its launch needs them. No hub MCP list (tests, a hub without it): Backplane's own and pcbparts only.
pub async fn launch(thread: &str, provider: &str, url: &str) -> Launch {
    let (servers, pcb) = match get() { Some(m) => { let (_, pr, b) = m.thread_ctx(thread).await; (m.for_thread(provider, &pr, &b), m.pcbparts().await) } None => (vec![], true) };
    let refs: Vec<&Server> = servers.iter().collect();
    let c = gen::Ctx { exe: hub_exe(), url: url.to_string(), pcbparts: pcb };
    let vault_get = |n: &str| vault::person_get(n);
    let (codex_args, codex_env) = gen::codex(&refs, &c, &vault_get);
    Launch { claude: gen::claude(&refs, &c, &vault_get).to_string(), disallowed: gen::claude_disallowed(&refs), acp: gen::acp(&refs, &c, &vault_get), codex_args, codex_env, fingerprint: gen::fingerprint(&refs, pcb), pcbparts: pcb }
}
/// The launch's fingerprint alone (what decides whether a live Claude is started again).
pub async fn fingerprint(thread: &str, provider: &str) -> String {
    match get() { Some(m) => { let (_, pr, b) = m.thread_ctx(thread).await; let s = m.for_thread(provider, &pr, &b); gen::fingerprint(&s.iter().collect::<Vec<_>>(), m.pcbparts().await) } None => gen::fingerprint(&[], true) }
}
