//! Making the always-on Boat, in Rust, as `scripts/boat-hub.sh` does by hand: create (no auto-stop, no TTL, none of the account's stored logins),
//! the same build as this hub with the guard under systemd and the health timer, the network (the tailnet when this machine is on one, else
//! Boat's public HTTPS address in front of the hub's own auth), the hub started, linked to this hub with no code typed (this hub asks the Boat
//! for a machine code, pairs with it, and approves its own request there), and local dictation if asked. Every step can run again: Try again
//! starts at the step that failed. The Boat's id is kept (`alwayson.boat`) as soon as it exists, so Delete it always has something to delete.
use super::{boat::Boat, Opts, Shared};
use crate::hub::Hub;
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::Arc, time::Duration};

/// The guard's prefix on the Boat (builds/<id>, CURRENT, backplane, home/), as on boat-hub.
pub const R: &str = "/home/user/bp";
/// The hub's port on the Boat's loopback, and the untrusted listener Boat's public address forwards to (public network only).
pub const PORT: u16 = 3797;
pub const PUBLIC_PORT: u16 = 3798;

pub const STEPS: &[(&str, &str)] = &[
    ("create", "Make the Boat"), ("ready", "Start it"), ("build", "Install Backplane"), ("service", "Keep it running"),
    ("network", "Reach it"), ("start", "Start the hub"), ("link", "Link it to this computer"), ("voice", "Install dictation"),
];

fn step_i(id: &str) -> usize { STEPS.iter().position(|(s, _)| *s == id).unwrap_or(0) }

/// The run from `from` (a step id, "" from the start), one step after another, each told on the row as it goes.
pub async fn run(hub: Arc<Hub>, sh: Arc<Shared>, b: Boat, from: String) {
    let start = if from.is_empty() { 0 } else { step_i(&from) };
    for (i, (id, _)) in STEPS.iter().enumerate().skip(start) {
        let o = sh.opts();
        if *id == "voice" && !o.dictation { sh.step(id, "skip", "Off: dictation stays on this computer."); continue }
        if *id == "voice" {
            // a build without the voice helper (an upload of a folder that has none) cannot hear there: said, not failed
            let bid = b.sh(&sh.box_id(), &format!("head -1 {R}/CURRENT 2>/dev/null || true"), 30).await.unwrap_or_default();
            let has = b.sh(&sh.box_id(), &format!("test -x {R}/builds/{}/backplane-voice && echo yes || echo no", bid.trim()), 30).await.unwrap_or_default();
            if has.trim() == "no" { sh.step(id, "skip", "This build has no voice helper, so dictation stays on this computer."); continue }
        }
        sh.step(id, "run", ""); sh.publish(&hub).await;
        let r = match *id {
            "create" => create(&hub, &sh, &b, &o).await,
            "ready" => ready(&b, &sh.box_id()).await,
            "build" => build(&hub, &b, &sh.box_id(), &o.network).await,
            "service" => service(&hub, &b, &sh.box_id(), &o).await,
            "network" => network(&hub, &sh, &b, &o).await,
            "start" => start_hub(&b, &sh.box_id(), &o.network).await,
            "link" => link(&hub, &sh, &b).await,
            _ => voice(&b, &sh.box_id()).await,
        };
        match r {
            Ok(note) => {
                if *id == "build" { let n = note.clone(); super::save(&hub, &sh, |k| { k["build"] = json!(n); }).await; }
                sh.step(id, "done", &note); sh.publish(&hub).await
            }
            Err(e) => { sh.step(id, "fail", &e); sh.end(); sh.publish(&hub).await; return }
        }
    }
    sh.end();
    super::save(&hub, &sh, |k| { k["made"] = json!(true); }).await;
    let _ = hub.commit(&json!({"$":"SettingSet","key":super::CHOICE_KEY,"value":"boat","at":crate::hub::now_ms() / 1000})).await;
    sh.publish(&hub).await;
}

async fn create(hub: &Arc<Hub>, sh: &Arc<Shared>, b: &Boat, o: &Opts) -> Result<String, String> {
    if !sh.box_id().is_empty() {
        // made before: still there? (a Boat deleted on the dashboard is made again)
        match b.sandbox(&sh.box_id()).await {
            Ok(s) if !matches!(s["state"].as_str(), Some("deleted" | "deleting")) => { b.keep_on(&sh.box_id()).await.ok(); return Ok(format!("{}, {}", sh.box_id(), o.size)) }
            _ => super::save(hub, sh, |k| { *k = json!({}); }).await,
        }
    }
    let idem = format!("bp-{}", hex::encode(crate::net::pairing::urandom(8)));
    let s = b.create(&o.size, &o.name, &idem).await?;
    let id = s["id"].as_str().unwrap_or("").to_string();
    if id.is_empty() { return Err("Boat made a sandbox but named no id".into()) }
    let (name, size, created) = (o.name.clone(), o.size.clone(), s["createdAt"].as_str().unwrap_or("").to_string());
    super::save(hub, sh, |k| { *k = json!({"id": id, "name": name, "size": size, "created": created, "network": "", "url": "", "made": false}); }).await;
    Ok(format!("{}, {}", s["id"].as_str().unwrap_or(""), o.size))
}

/// Ready, idle or running (Boat's states that take commands), within five minutes; and no auto-stop, asked again.
async fn ready(b: &Boat, id: &str) -> Result<String, String> {
    for _ in 0..100 {
        match b.sandbox(id).await {
            Ok(s) => match s["state"].as_str().unwrap_or("") {
                "ready" | "idle" | "running" => { b.keep_on(id).await?; return Ok(String::new()) }
                "error" => return Err(format!("Boat says the sandbox failed to start{}", s["error"].as_str().map_or(String::new(), |e| format!(": {e}")))),
                "archived" => { b.resume(id, "").await?; }
                _ => {}
            },
            Err(e) if e.contains("401") || e.contains("refused the key") => return Err(e),
            Err(_) => {}
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    Err("the sandbox did not start within five minutes".into())
}

/// This hub's own build folder: `BACKPLANE_ALWAYSON_BUILD`, else the folder of the running program.
pub fn build_dir() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("BACKPLANE_ALWAYSON_BUILD") { if !d.trim().is_empty() { return Some(PathBuf::from(d.trim())) } }
    std::env::current_exe().ok()?.parent().map(Path::to_path_buf)
}
/// A build folder's name is the id the guard knows it by (`builds/<id>`): letters, digits, . _ - only.
pub fn build_id(dir: &Path) -> Option<String> {
    let n = dir.file_name()?.to_string_lossy().into_owned();
    (!n.is_empty() && n.len() <= 80 && n.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))).then_some(n)
}
/// What goes to the Boat: the hub, the guard, voice, its model's VAD, assets, COMMIT. The window, CEF and the browser stay here.
pub const SHIPPED: &[&str] = &["backplane-hub", "backplane", "backplane-voice", "COMMIT", "silero_vad.onnx", "assets"];

/// A Boat is Linux on x86_64: the release target it runs.
pub const BOAT_TARGET: &str = "linux-x64";
/// Where the Boat's build comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// This hub's own build folder, copied up: only a main build (the person's dev builds, `builds/<date>-<commit>`) on Linux x86_64, which no release carries.
    Upload,
    /// The release of this hub's version for linux-x64, fetched by the Boat itself and checked against the signed manifest this hub verified.
    Release,
}
/// The pure choice: `build`: this hub's build folder name, if it runs from one with the hub and the guard in it.
pub fn source_of(os: &str, arch: &str, build: Option<&str>) -> Source {
    let same = crate::update::rel::target_of(os, arch) == Some(BOAT_TARGET);
    let main_build = build.map_or(false, |b| !crate::update::rel::tag_ok(b));
    if same && main_build { Source::Upload } else { Source::Release }
}
pub fn source_here() -> (Source, Option<PathBuf>) { let (s, d, _) = source_why(); (s, d) }
/// `source_here`, and why, as the hub's log says it once per make ("release: no guard beside the hub in /x").
pub fn source_why() -> (Source, Option<PathBuf>, String) {
    let exe = build_dir();
    let dir = exe.clone().filter(|d| d.join("backplane-hub").is_file() && d.join("backplane").is_file());
    let id = dir.as_deref().and_then(build_id);
    // BACKPLANE_ALWAYSON_SOURCE=release: a test's way to take the release path from a main build (test/alwayson/e2e.sh SOURCE=release)
    if std::env::var("BACKPLANE_ALWAYSON_SOURCE").map_or(false, |v| v == "release") { return (Source::Release, dir, "release: BACKPLANE_ALWAYSON_SOURCE=release".into()) }
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    let src = source_of(os, arch, id.as_deref());
    let where_ = exe.as_deref().map_or("an unknown folder".to_string(), |d| d.display().to_string());
    let why = match (&src, &dir, &id) {
        (Source::Upload, _, Some(i)) => format!("upload: this hub's main build {i}"),
        _ if crate::update::rel::target_of(os, arch) != Some(BOAT_TARGET) => format!("release: this hub is {os}-{arch}, the Boat is {BOAT_TARGET}"),
        (_, None, _) => format!("release: no guard beside the hub in {where_}"),
        (_, _, Some(i)) => format!("release: {i} is a release build"),
        _ => format!("release: {where_} is not a build folder"),
    };
    (src, dir, why)
}
/// The release a Boat gets: this hub's version, or `BACKPLANE_ALWAYSON_RELEASE` (a test's published version).
pub fn release_version(own: &str) -> String { std::env::var("BACKPLANE_ALWAYSON_RELEASE").ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| own.to_string()) }
/// The guard's layout on the Boat, made by every step that writes there: no step relies on one before it having run (a fresh Boat has none of it).
pub fn layout() -> String { format!("mkdir -p {R}/builds {R}/home {R}/units") }

/// The release a Boat installs, as the updater checks one: this hub's version exactly, its signed files verified with this build's release key,
/// the manifest's tag, version and channel matching, and the linux-x64 archive's name, size and SHA-256 from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched { pub tag: String, pub url: String, pub sha256: String, pub size: u64, pub version: String, pub commit: String }
pub fn release_for(version: &str, list: &[crate::update::run::Listed], dir: &Path, get: crate::update::run::Get, keys: &[(String, [u8; 32])]) -> Result<Fetched, String> {
    use crate::update::{rel, run::SIGNED};
    let tag = format!("v{}", version.trim().trim_start_matches('v'));
    if !rel::tag_ok(&tag) { return Err(format!("this Backplane ({version}) is not a release, and only a release can be installed on a Boat from here")) }
    let l = list.iter().find(|l| l.rel.tag == tag && !l.rel.draft).ok_or_else(|| format!("no release {tag} was found to install on the Boat"))?;
    if !l.rel.signed { return Err(format!("the release {tag} is not fully published yet")) }
    let read = |n: &str| -> Result<Vec<u8>, String> { let p = dir.join(n); get(&l.urls[n], &p, 1 << 20)?; let b = std::fs::read(&p).map_err(|e| format!("{n}: {e}")); let _ = std::fs::remove_file(&p); b };
    let (body, sig, kid) = (read(SIGNED[0])?, read(SIGNED[1])?, read(SIGNED[2])?);
    let kid = String::from_utf8_lossy(&kid).to_string();
    rel::verify(&body, &sig, &kid, keys)?;
    let m = rel::parse(&body)?; rel::ceiling_ok(&kid, &m.version)?;
    let a = rel::pick(&m, &tag, rel::tag_channel(&tag), BOAT_TARGET)?;
    let url = l.urls.get(&a.name).ok_or("the release does not carry the file its manifest names")?.clone();
    Ok(Fetched { tag, url, sha256: a.sha256.clone(), size: a.size, version: m.version.clone(), commit: m.commit.clone() })
}
/// The Boat's commands for a release: download, the size and SHA-256 the verified manifest gives, the archive's members listed and refused
/// if any is a link or leaves the folder, unpacked owning nothing, the hub and the guard present, EXPECT written from the manifest, one rename.
pub fn release_script(f: &Fetched) -> String {
    let t = &f.tag;
    format!("set -e; {layout}; cd {R}; [ -x builds/{t}/backplane-hub ] && exit 0; rm -rf builds/.in-{t} .in-{t}.tgz; \
curl -fsSL --proto =https --max-filesize {size} -o .in-{t}.tgz '{url}'; \
[ \"$(stat -c %s .in-{t}.tgz)\" = {size} ] || {{ echo 'the download is the wrong size' >&2; exit 3; }}; \
echo '{sha}  .in-{t}.tgz' | sha256sum -c --status || {{ echo 'the checksum does not match the signed manifest' >&2; exit 3; }}; \
if tar -tvzf .in-{t}.tgz | grep -qE '^[lhbcp]' || tar -tzf .in-{t}.tgz | grep -qE '(^/|(^|/)\\.\\.(/|$))'; then echo 'the archive holds links or paths outside it' >&2; exit 3; fi; \
mkdir builds/.in-{t}; tar --no-same-owner -xzf .in-{t}.tgz -C builds/.in-{t}; rm -f .in-{t}.tgz; \
test -x builds/.in-{t}/backplane-hub && test -x builds/.in-{t}/backplane || {{ echo 'the release has no hub or guard' >&2; exit 3; }}; \
printf '%s\\n' '{expect}' > builds/.in-{t}/EXPECT; mv builds/.in-{t} builds/{t}",
        size = f.size, url = f.url.replace('\'', ""), sha = f.sha256, expect = json!({"version": f.version, "commit": f.commit}), layout = layout())
}

async fn build(hub: &Arc<Hub>, b: &Boat, id: &str, net: &str) -> Result<String, String> {
    let (src, dir, why) = source_why();
    eprintln!("backplane-hub: always-on Boat {id}: the build comes by {why}");
    b.sh(id, &layout(), 30).await?;
    let bid = if src == Source::Release {
        let (home, version) = (hub.cfg.home.clone(), release_version(&hub.cfg.version));
        let f = tokio::task::spawn_blocking(move || -> Result<Fetched, String> {
            let d = home.join("secrets").join("boat").join(format!("rel-{}", hex::encode(crate::net::pairing::urandom(4))));
            std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
            let r = (|| { let lp = d.join("releases.json"); crate::update::svc::curl(&crate::update::svc::feed(), &lp, 8 << 20)?;
                let list = crate::update::run::listing(&std::fs::read(&lp).map_err(|e| e.to_string())?)?;
                release_for(&version, &list, &d, &crate::update::svc::curl, &crate::update::rel::keys()) })();
            let _ = std::fs::remove_dir_all(&d); r
        }).await.map_err(|e| e.to_string())??;
        b.sh(id, &release_script(&f), 600).await.map_err(|e| format!("installing {}: {e}", f.tag))?;
        f.tag
    } else {
    let dir = dir.ok_or("this hub cannot find its own build folder")?;
    let bid = build_id(&dir).ok_or_else(|| format!("{} is not a build folder (builds/<id>)", dir.display()))?;
    let have = b.sh(id, &format!("test -x {R}/builds/{bid}/backplane-hub && echo yes || echo no"), 30).await?;
    if have.trim() != "yes" {
        let files: Vec<&str> = SHIPPED.iter().copied().filter(|f| dir.join(f).exists()).collect();
        let tmp = hub.cfg.home.join("secrets").join("boat");
        let _ = std::fs::create_dir_all(&tmp);
        let tgz = tmp.join(format!("build-{bid}.tgz"));
        let (d2, t2, f2) = (dir.clone(), tgz.clone(), files.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        let made = tokio::task::spawn_blocking(move || crate::proc::std_cmd(crate::proc::Kind::Local, "tar").arg("-czf").arg(&t2).arg("-C").arg(&d2).args(&f2).output()).await
            .map_err(|e| e.to_string())?.map_err(|e| format!("tar: {e}"))?;
        if !made.status.success() { let _ = std::fs::remove_file(&tgz); return Err(format!("could not pack the build: {}", String::from_utf8_lossy(&made.stderr).trim())) }
        b.sh(id, &format!("mkdir -p {R}/builds {R}/home && rm -rf {R}/builds/.in-{bid} {R}/.in-{bid}.tgz"), 30).await?;
        let up = b.upload(id, &tgz, &format!("{R}/.in-{bid}.tgz")).await;
        let _ = std::fs::remove_file(&tgz);
        up?;
        b.sh(id, &format!("set -e; cd {R}; mkdir -p builds/.in-{bid}; tar -xzf .in-{bid}.tgz -C builds/.in-{bid}; rm -f .in-{bid}.tgz; d=builds/.in-{bid}; \
            v=$($d/backplane-hub --version | awk '{{print $NF}}'); c=$(head -c 40 $d/COMMIT 2>/dev/null || true); printf '{{\"version\":\"%s\",\"commit\":\"%s\"}}\\n' \"$v\" \"$c\" > $d/EXPECT; \
            mv $d builds/{bid}"), 300).await?;
    }
    bid
    };
    // a public Boat needs the door (public.rs): a build without it is told now, seconds after the install, not after the hub has started
    if net == "public" && b.sh(id, &format!("grep -qa BACKPLANE_PUBLIC_PORT {R}/builds/{bid}/backplane-hub && echo door || echo none"), 60).await?.trim() != "door" {
        return Err(no_door(&bid))
    }
    // the first build: CURRENT, and the guard beside it (an update never replaces the program running it)
    b.sh(id, &format!("set -e; cd {R}; [ -s CURRENT ] || echo {bid} > CURRENT; cp builds/$(head -1 CURRENT)/backplane backplane.new && mv backplane.new backplane"), 60).await?;
    Ok(bid)
}

/// A made Boat follows the hub that made it: when this hub runs another build than the one the Boat was given (a main build: the person's
/// homes, which take every build main-build.sh publishes), that build goes up and the Boat's guard runs it on probation, going back by itself
/// if it fails its health check, as `scripts/boat-hub.sh update` does for bp-hub. A Boat on a release updates itself on its own channel instead.
/// Keeps the Boat's current build and the two before it. Ok(Some(build)) when it moved, Ok(None) when there was nothing to do.
pub async fn follow(hub: &Arc<Hub>, b: &Boat, id: &str, had: &str, net: &str) -> Result<Option<String>, String> {
    let net = net.to_string();
    let (src, dir) = source_here();
    if src != Source::Upload { return Ok(None) }
    let Some(bid) = dir.as_deref().and_then(build_id) else { return Ok(None) };
    if bid == had { return Ok(None) }
    let cur = b.sh(id, &format!("head -1 {R}/CURRENT 2>/dev/null || true"), 30).await?.trim().to_string();
    if cur == bid { return Ok(Some(bid)) }
    build(hub, b, id, &net).await?;
    b.sh(id, &format!("if systemctl is-active -q bp-hub; then BACKPLANE_HOME={R}/home {R}/backplane hub update {bid}; else echo {bid} > {R}/CURRENT; fi"), 120).await?;
    let _ = b.sh(id, &format!("cd {R}/builds && ls -1t | grep -v '^\\.' | tail -n +4 | while read -r d; do [ \"$d\" = \"$(head -1 {R}/CURRENT)\" ] || [ -e .pins/$d ] || rm -rf -- \"$d\"; done"), 120).await;
    Ok(Some(bid))
}

/// The units on the Boat: the guard under systemd (restarts on any exit, starts at boot) and a health check every 2 minutes that restarts the
/// service after 3 missed /healthz in a row, as boat-hub's. The hub's flags follow the network: the tailnet only, or loopback with the public listener.
pub fn units(net: &str, name: &str, url: &str, self_updates: bool) -> Vec<(&'static str, String)> { units_on(net, name, url, self_updates, "stable") }
/// `channel`: the channel of the hub that made the Boat; the Boat's hub starts on it (`BACKPLANE_UPDATE_CHANNEL`) until its own Settings say otherwise.
pub fn units_on(net: &str, name: &str, url: &str, self_updates: bool, channel: &str) -> Vec<(&'static str, String)> {
    let (env_net, flags) = if net == "public" {
        (format!(" BACKPLANE_URL={url} BACKPLANE_PUBLIC_PORT={PUBLIC_PORT}"), "--no-tailscale")
    } else { (" BACKPLANE_HTTPS_PORT=443".to_string(), "--tailnet-only") };
    let upd = if self_updates { if channel == "beta" { " BACKPLANE_UPDATE_CHANNEL=beta" } else { "" } } else { " BACKPLANE_NO_UPDATE=1" };
    let after = if net == "public" { "network-online.target" } else { "network-online.target tailscaled.service" };
    vec![
        ("bp-hub.service", format!("[Unit]\nDescription=Backplane hub (always-on)\nAfter={after}\nWants=network-online.target\n[Service]\nUser=user\nWorkingDirectory=/home/user\n\
Environment=HOME=/home/user BACKPLANE_NO_DESKTOP_APP=1 BACKPLANE_NAME={name}{env_net}{upd}\n\
ExecStart={R}/backplane hub run --home {R}/home --port {PORT} --host 127.0.0.1 --builds {R} --log {R}/hub.log --pidfile {R}/hub.pid -- {flags} --foreground --headless\n\
Restart=always\nRestartSec=5\nKillMode=mixed\nTimeoutStopSec=90\n[Install]\nWantedBy=multi-user.target\n")),
        ("bp-hub-health.service", format!("[Unit]\nDescription=Backplane hub health check\n[Service]\nType=oneshot\nUser=user\nExecStart=/bin/bash {R}/health.sh\n")),
        ("bp-hub-health.timer", "[Unit]\nDescription=Backplane hub health check every 2 minutes\n[Timer]\nOnBootSec=3min\nOnUnitActiveSec=2min\n[Install]\nWantedBy=timers.target\n".to_string()),
    ]
}
pub fn health_sh() -> String {
    format!("#!/bin/bash\n# bp-hub-health: /healthz on loopback; three misses in a row restart bp-hub (only when the service is meant to run)\n\
f={R}/health.fails; systemctl is-enabled -q bp-hub || exit 0\n\
if curl -fsS -m 10 -o /dev/null http://127.0.0.1:{PORT}/healthz; then echo 0 > $f; exit 0; fi\n\
n=$(( $(cat $f 2>/dev/null || echo 0) + 1 )); echo $n > $f; echo \"$(date -Is) healthz miss $n\" >> {R}/health.log\n\
[ $n -ge 3 ] && {{ echo \"$(date -Is) restarting bp-hub\" >> {R}/health.log; echo 0 > $f; sudo -n systemctl restart bp-hub; }}\nexit 0\n")
}
/// The Boat updates itself only when this hub does (a release on its channel); a hub that is updated by hand or by a package leaves the Boat to the same.
pub fn self_updates(info: &Value) -> bool { std::env::var("BACKPLANE_NO_UPDATE").map_or(true, |v| v != "1") && !matches!(info["update"].as_str().unwrap_or(""), "package" | "off" | "") }

async fn service(hub: &Arc<Hub>, b: &Boat, id: &str, o: &Opts) -> Result<String, String> {
    b.sh(id, "sudo -n true", 30).await.map_err(|_| "no sudo on the Boat".to_string())?;
    b.sh(id, &layout(), 30).await?;
    b.write(id, &format!("{R}/health.sh"), &health_sh()).await?;
    b.sh(id, &format!("chmod +x {R}/health.sh"), 30).await?;
    // the network's address is not known yet on a public Boat: the unit is written again once it is (network step)
    write_units(hub, b, id, &o.network, &o.host, "").await?;
    b.sh(id, "sudo -n systemctl enable --now bp-hub-health.timer > /dev/null 2>&1", 60).await?;
    Ok("guard under systemd, health check every 2 minutes".into())
}
async fn write_units(hub: &Arc<Hub>, b: &Boat, id: &str, net: &str, name: &str, url: &str) -> Result<(), String> {
    let channel = crate::update::rel::channel(&hub.setting("update.channel").await.unwrap_or_default()).to_string();
    // a Boat whose build this hub copies up (a main build) is fed by this hub (`follow`), never by a release
    let own = self_updates(&hub.info_now()) && source_here().0 == Source::Release;
    for (n, body) in units_on(net, name, url, own, &channel) {
        b.write(id, &format!("{R}/units/{n}"), &body).await?;
        b.sh(id, &format!("sudo -n install -m 644 {R}/units/{n} /etc/systemd/system/{n}"), 30).await?;
    }
    b.sh(id, "sudo -n systemctl daemon-reload", 60).await.map(|_| ())
}

/// The tailnet when this machine is on one (the key `tailscale.boat` from the vault, sent as a file in a private folder, used once and removed);
/// else Boat's public HTTPS address for the untrusted listener.
async fn network(hub: &Arc<Hub>, sh: &Arc<Shared>, b: &Boat, o: &Opts) -> Result<String, String> {
    let id = sh.box_id();
    let url = if o.network == "tailnet" {
        let Some(key) = crate::settings::vault::person_get(super::TS_KEY).filter(|k| !k.trim().is_empty()) else {
            return Err("needs a Tailscale auth key: paste one below (tailscale.boat), then Try again".into())
        };
        b.sh(&id, "command -v tailscale > /dev/null || (curl -fsSL https://tailscale.com/install.sh | sudo -n sh > /dev/null 2>&1); sudo -n systemctl enable --now tailscaled > /dev/null 2>&1; command -v tailscale", 300).await
            .map_err(|e| format!("could not install Tailscale there: {e}"))?;
        b.sh(&id, "umask 077; rm -rf /home/user/.bp-ts; mkdir -m 700 /home/user/.bp-ts", 30).await?;
        b.write(&id, "/home/user/.bp-ts/key", key.trim()).await?;
        let up = b.sh(&id, &format!("chmod 600 /home/user/.bp-ts/key; sudo -n tailscale up --auth-key=file:/home/user/.bp-ts/key --hostname={} --operator=user --accept-dns=true; rc=$?; rm -rf /home/user/.bp-ts; exit $rc", o.host), 120).await;
        b.sh(&id, "rm -rf /home/user/.bp-ts", 30).await.ok();
        up.map_err(|e| format!("tailscale up failed: {e}"))?;
        let st = b.sh(&id, "tailscale status --json --self --peers=false", 30).await?;
        let dns = serde_json::from_str::<Value>(&st).ok().and_then(|v| v["Self"]["DNSName"].as_str().map(|s| s.trim_end_matches('.').to_string())).unwrap_or_default();
        if dns.is_empty() { return Err("the Boat joined the tailnet but has no name there".into()) }
        format!("https://{dns}")
    } else {
        let u = b.host(&id, PUBLIC_PORT, true).await?;
        write_units(hub, b, &id, "public", &o.host, &u).await?;
        u
    };
    let (u2, n2) = (url.clone(), o.network.clone());
    super::save(hub, sh, |k| { k["url"] = json!(u2); k["network"] = json!(n2); }).await;
    Ok(url)
}

async fn start_hub(b: &Boat, id: &str, net: &str) -> Result<String, String> {
    b.sh(id, "sudo -n systemctl enable bp-hub > /dev/null 2>&1; sudo -n systemctl restart bp-hub", 90).await?;
    for _ in 0..60 {
        if let Ok(h) = b.sh(id, &format!("curl -fsS -m 5 http://127.0.0.1:{PORT}/healthz"), 30).await {
            if h.contains("\"ok\"") || h.contains("booted") || h.starts_with('{') {
                if net != "public" { return Ok(String::new()) }
                // the public address needs the door (public.rs); a release from before it has none, and nothing would answer there
                for _ in 0..10 {
                    let c = b.sh(id, &format!("curl -s -o /dev/null -m 5 -w '%{{http_code}}' http://127.0.0.1:{PUBLIC_PORT}/healthz; true"), 30).await.unwrap_or_default();
                    if matches!(c.trim(), "200" | "503") { return Ok(String::new()) }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                let cur = b.sh(id, &format!("head -1 {R}/CURRENT"), 30).await.unwrap_or_default();
                return Err(no_door(cur.trim()))
            }
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    let log = b.sh(id, &format!("tail -5 {R}/hub.log 2>/dev/null; systemctl is-active bp-hub"), 30).await.unwrap_or_default();
    Err(format!("the hub did not answer on the Boat within three minutes: {}", log.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(200).collect::<String>()))
}

/// What a public Boat says when its build has no door (a release from before it: v0.15.2 and older).
pub fn no_door(build: &str) -> String {
    format!("Backplane {build} on the Boat has no public address yet: it came with a later release. Use your tailnet, or wait for the next release.")
}

/// The machine code `backplane pair --machine` prints on the Boat: "On the other machine: backplane pair --join <address> <code>".
pub fn code_of(out: &str) -> Option<(String, String)> {
    let l = out.lines().find(|l| l.contains("pair --join"))?;
    let w: Vec<&str> = l.split_whitespace().collect();
    let i = w.iter().position(|x| *x == "--join")?;
    Some((w.get(i + 1)?.to_string(), w.get(i + 2)?.to_string()))
}
/// The request id `backplane pair --requests` lists for the check code `check` (each line: "<id>  <name>  <url>  check code <c> ...").
pub fn request_of(out: &str, check: &str) -> Option<String> {
    out.lines().find(|l| l.contains(&format!("check code {check} ")) || l.trim_end().ends_with(&format!("check code {check}"))).and_then(|l| l.split_whitespace().next()).map(str::to_string)
}

async fn link(hub: &Arc<Hub>, sh: &Arc<Shared>, b: &Boat) -> Result<String, String> {
    let id = sh.box_id();
    let url = sh.saved()["url"].as_str().unwrap_or("").to_string();
    if url.is_empty() { return Err("no address for the Boat yet: Try again from Reach it".into()) }
    let p = crate::net::start::peering().ok_or("this hub's machine links are not running")?;
    let hp = crate::net::machines::hostport(&url).to_string();
    if let Some(l) = p.table.with(|t| t.live().into_iter().find(|x| crate::net::machines::hostport(&x.url) == hp)) {
        let lid = l.id.clone(); super::save(hub, sh, |k| { k["link"] = json!(lid); }).await;
        return Ok(format!("linked as {}", l.name))
    }
    let pair = format!("BACKPLANE_HOME={R}/home {R}/backplane pair");
    let out = b.sh(&id, &format!("{pair} --machine"), 60).await?;
    let (said, code) = code_of(&out).ok_or("the Boat made no machine code")?;
    // the address this hub pairs with: the one recorded at Reach it, else the one the Boat's hub gives for itself, whichever answers first
    // (a tailnet name's HTTPS can take a minute to come up after the Boat joins)
    let mut tries: Vec<String> = vec![url.clone()];
    if crate::net::botnet::url_ok(&said) && !crate::net::machines::loopback(&said) && said != url { tries.push(said.clone()) }
    let mut url = String::new();
    'find: for _ in 0..30 {
        for t in &tries {
            let r = super::boat::send(&hub.cfg.home, super::boat::Call::new("GET", format!("{}/healthz", t.trim_end_matches('/')), "").secs(8)).await;
            if matches!(r.code.as_str(), "200" | "503") { url = t.clone(); break 'find }
        }
        tokio::time::sleep(Duration::from_secs(4)).await;
    }
    if url.is_empty() { return Err(format!("this computer cannot reach the Boat at {}", tries.join(" or "))) }
    let u2 = url.clone(); super::save(hub, sh, |k| { k["url"] = json!(u2); }).await;
    let hp = crate::net::machines::hostport(&url).to_string();
    let (attempt, check) = p.pair_with(hub, &url, &code).await?;
    // this hub made the request: it approves it there, after matching the check code
    let mut approved = false;
    for _ in 0..20 {
        let reqs = b.sh(&id, &format!("{pair} --requests"), 30).await.unwrap_or_default();
        if let Some(rid) = request_of(&reqs, &check) { b.sh(&id, &format!("{pair} --approve {rid}"), 30).await?; approved = true; break }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    if !approved { return Err("the Boat never saw this computer's request".into()) }
    for _ in 0..40 {
        let a = p.attempt(&attempt);
        match a["state"].as_str().unwrap_or("") {
            "linked" => {
                let lid = p.table.with(|t| t.live().into_iter().find(|x| crate::net::machines::hostport(&x.url) == hp).map(|x| x.id)).unwrap_or_default();
                super::save(hub, sh, |k| { k["link"] = json!(lid); }).await;
                return Ok(format!("linked as {}", a["name"].as_str().unwrap_or("")))
            }
            "refused" => return Err(a["error"].as_str().unwrap_or("refused").to_string()),
            _ => tokio::time::sleep(Duration::from_millis(1500)).await,
        }
    }
    Err("the link was approved but did not finish".into())
}

/// The local dictation runtime and model on the Boat, into its hub's own voice folder: its hub then hears locally (`voice::session::engine_of`:
/// a ready model is used unless the person chose the cloud there).
async fn voice(b: &Boat, id: &str) -> Result<String, String> {
    let bid = b.sh(id, &format!("head -1 {R}/CURRENT"), 30).await?.trim().to_string();
    let dir = format!("{R}/home/voice/local");
    if b.sh(id, &format!("cat {dir}/ready-parakeet-v2 2>/dev/null || true"), 30).await?.trim() == "ok" { return Ok("Parakeet v2, there already".into()) }
    b.sh(id, &format!("mkdir -p {dir}"), 30).await?;
    b.detach(id, &format!("{R}/builds/{bid}/backplane-voice local-install --local-dir {dir} --model parakeet-v2 > {R}/voice-install.log 2>&1")).await?;
    // the helper writes ready-<model> when the model and the runtime are in place; while it runs, it is in the process list
    let probe = format!("cat {dir}/ready-parakeet-v2 2>/dev/null; pgrep -f 'backplane-voic[e] local-install' > /dev/null && echo RUNNING; true");
    let mut gone = 0;
    for _ in 0..600 {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let Ok(out) = b.sh(id, &probe, 30).await else { continue };
        if out.lines().next().map(str::trim) == Some("ok") { return Ok("Parakeet v2".into()) }
        if out.contains("RUNNING") { gone = 0; continue }
        gone += 1; if gone < 3 { continue } // (the probe can land between the helper's steps)
        let tail = b.sh(id, &format!("tail -c 600 {R}/voice-install.log"), 30).await.unwrap_or_default();
        let last = tail.lines().rev().find(|l| l.contains("error")).unwrap_or(tail.lines().last().unwrap_or("")).to_string();
        return Err(format!("the install stopped: {}", last.chars().take(200).collect::<String>()))
    }
    Err("the dictation install did not finish in 50 minutes".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_units_run_the_guard_with_the_networks_flags() {
        let t = units("tailnet", "bp-x", "", false);
        assert_eq!(t.iter().map(|u| u.0).collect::<Vec<_>>(), ["bp-hub.service", "bp-hub-health.service", "bp-hub-health.timer"]);
        assert!(t[0].1.contains("-- --tailnet-only --foreground --headless") && t[0].1.contains("BACKPLANE_NO_UPDATE=1") && t[0].1.contains("tailscaled.service") && t[0].1.contains("Restart=always"));
        let p = units("public", "bp-x", "https://abc-3798.on.boat.dev", true);
        assert!(p[0].1.contains("--no-tailscale") && p[0].1.contains("BACKPLANE_URL=https://abc-3798.on.boat.dev") && p[0].1.contains("BACKPLANE_PUBLIC_PORT=3798") && !p[0].1.contains("NO_UPDATE"));
        assert!(p[0].1.contains("--host 127.0.0.1"), "the hub's own listener never leaves the Boat's loopback");
        assert!(units_on("public", "x", "https://u", true, "beta")[0].1.contains("BACKPLANE_UPDATE_CHANNEL=beta"), "a beta hub's Boat starts on beta");
        assert!(!units_on("public", "x", "https://u", true, "stable")[0].1.contains("UPDATE_CHANNEL"));
        assert!(units_on("public", "x", "https://u", false, "beta")[0].1.contains("BACKPLANE_NO_UPDATE=1"), "a Boat this hub feeds does not update itself");
        assert!(health_sh().contains("http://127.0.0.1:3797/healthz") && health_sh().contains("systemctl restart bp-hub"));
    }
    #[test] fn the_pair_commands_output_is_read() {
        let out = "Machine code: ABCD2345 (ten minutes, once)\nAddress:\n  https://bp.on.boat.dev\nOn the other machine: backplane pair --join https://bp.on.boat.dev ABCD2345\nThen approve its request here: backplane pair --requests\n";
        assert_eq!(code_of(out), Some(("https://bp.on.boat.dev".into(), "ABCD2345".into())));
        let reqs = "r1  desk  http://desk:3797  check code 123456  590s left   (backplane pair --approve r1 / --deny r1)\nr2  other  http://o:1  check code 654321  12s left   (x)\n";
        assert_eq!(request_of(reqs, "654321").as_deref(), Some("r2")); assert_eq!(request_of(reqs, "111111"), None);
        assert_eq!(code_of("No code"), None);
    }
    #[test] fn the_boats_build_comes_from_a_release_unless_this_is_a_linux_x64_main_build() {
        let main = Some("20261111-001717-483154793");
        assert_eq!(source_of("linux", "x86_64", main), Source::Upload, "the person's main builds: no release carries them");
        assert_eq!(source_of("linux", "x86_64", Some("v0.15.2")), Source::Release, "a release user's build is fetched by the Boat itself");
        assert_eq!(source_of("linux", "x86_64", None), Source::Release, "a hub from a system package has no build folder");
        assert_eq!(source_of("linux", "aarch64", main), Source::Release, "an arm64 build does not run on the Boat");
        assert_eq!(source_of("macos", "aarch64", main), Source::Release);
        assert_eq!(source_of("macos", "aarch64", Some("v0.15.2")), Source::Release);
        assert_eq!(source_of("windows", "x86_64", main), Source::Release);
        assert_eq!(source_of("freebsd", "x86_64", main), Source::Release);
    }
    #[test] fn the_release_is_this_hubs_version_verified_as_the_updater_verifies_it() {
        use crate::update::run::listing;
        use ed25519_compact::{KeyPair, Seed};
        let kp = KeyPair::from_seed(Seed::new([7; 32])); let keys = vec![("k".to_string(), *kp.pk)];
        let sha = "ab".repeat(32);
        let manifest = |tag: &str, ver: &str, ch: &str| json!({"schema": 1, "tag": tag, "version": ver, "channel": ch, "commit": "c0ffee",
            "assets": {"linux-x64": {"name": format!("backplane-{tag}-linux-x64-native.tar.gz"), "sha256": sha, "size": 1234}, "darwin-arm64": {"name": format!("backplane-{tag}-darwin-arm64-native.tar.gz"), "sha256": sha, "size": 99}}}).to_string().into_bytes();
        let files: std::cell::RefCell<std::collections::HashMap<String, Vec<u8>>> = Default::default();
        let mut add = |tag: &str, m: Vec<u8>, sig: Vec<u8>| { for (n, b) in [("manifest.json", m), ("manifest.json.sig", sig), ("manifest.json.keyid", b"k\n".to_vec())] { files.borrow_mut().insert(format!("mem://{tag}/{n}"), b); } };
        let m = manifest("v0.15.2", "0.15.2", "stable"); let s = kp.sk.sign(&m, None).as_ref().to_vec(); add("v0.15.2", m, s);
        let m = manifest("v0.16.0-beta.1", "0.16.0-beta.1", "beta"); let s = kp.sk.sign(&m, None).as_ref().to_vec(); add("v0.16.0-beta.1", m, s);
        let mut bad = manifest("v0.15.3", "0.15.3", "stable"); let s = kp.sk.sign(&bad, None).as_ref().to_vec(); let n = bad.len(); bad[n - 3] ^= 1; add("v0.15.3", bad, s);
        let rel = |tag: &str, pre: bool| format!(r#"{{"tag_name":"{tag}","prerelease":{pre},"draft":false,"assets":[{{"name":"manifest.json","browser_download_url":"mem://{tag}/manifest.json"}},{{"name":"manifest.json.sig","browser_download_url":"mem://{tag}/manifest.json.sig"}},{{"name":"manifest.json.keyid","browser_download_url":"mem://{tag}/manifest.json.keyid"}},{{"name":"backplane-{tag}-linux-x64-native.tar.gz","browser_download_url":"https://github.com/x/{tag}.tgz"}}]}}"#);
        let list = listing(format!("[{},{},{}]", rel("v0.15.2", false), rel("v0.16.0-beta.1", true), rel("v0.15.3", false)).as_bytes()).unwrap();
        let d = std::env::temp_dir().join(format!("bp-aon-rel-{}", std::process::id())); std::fs::create_dir_all(&d).unwrap();
        let get = |u: &str, p: &Path, _max: u64| -> Result<(), String> { std::fs::write(p, files.borrow().get(u).ok_or("404")?).map_err(|e| e.to_string()) };
        let f = release_for("0.15.2", &list, &d, &get, &keys).unwrap();
        assert_eq!((f.tag.as_str(), f.url.as_str(), f.sha256.as_str(), f.size, f.commit.as_str()), ("v0.15.2", "https://github.com/x/v0.15.2.tgz", sha.as_str(), 1234, "c0ffee"), "linux-x64, never this machine's own target");
        assert_eq!(release_for("0.16.0-beta.1", &list, &d, &get, &keys).unwrap().tag, "v0.16.0-beta.1", "a beta hub's Boat gets the same beta");
        assert!(release_for("0.15.3", &list, &d, &get, &keys).unwrap_err().contains("signature"), "a tampered manifest is refused");
        assert!(release_for("0.15.2", &list, &d, &get, &[("k".into(), [1; 32])]).is_err(), "only this build's key");
        assert!(release_for("0.17.0", &list, &d, &get, &keys).unwrap_err().contains("no release v0.17.0"));
        let sh = release_script(&f);
        assert!(sh.starts_with(&format!("set -e; mkdir -p {R}/builds {R}/home {R}/units; cd {R};")), "a fresh Boat has no layout: the install makes its own first");
        assert!(sh.contains(&format!("echo '{sha}  .in-v0.15.2.tgz' | sha256sum -c")) && sh.contains("--max-filesize 1234") && sh.contains("--no-same-owner") && sh.contains("mv builds/.in-v0.15.2 builds/v0.15.2"));
        assert!(sh.contains(r#"{"version":"0.15.2","commit":"c0ffee"}"#), "EXPECT from the signed manifest");
    }
    #[test] fn the_source_says_why() {
        let (src, _, why) = source_why();
        assert!(why.starts_with(if src == Source::Upload { "upload: " } else { "release: " }), "{why}");
        assert!(no_door("v0.15.0").starts_with("Backplane v0.15.0 on the Boat has no public address yet"));
    }
    #[test] fn a_build_folder_name_is_the_guards_id() {
        assert_eq!(build_id(Path::new("/x/builds/20261111-001717-483154793")).as_deref(), Some("20261111-001717-483154793"));
        assert_eq!(build_id(Path::new("/x/builds/a b")), None);
        assert!(STEPS.len() == 8 && step_i("link") == 6);
    }
}
