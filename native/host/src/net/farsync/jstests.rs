//! Against a REAL JS hub (the Bend hub compiled by Bun), on a temp home, with stand-in agents (none): the mirror built
//! from its `/far/log`, a page near the size limit, a push to it, and the mirror file format both ways. Ignored unless
//! BACKPLANE_JS_HUB names the binary (test/gpui/net/boat-js-peer.sh runs it on Boat). Ports 3915 (JS) and 3916 (here).
use super::*;
use crate::net::peering::tests::{node_extra, Extra, FIXED_PORT};
use crate::net::{botnet, farstore::FarStore, outbox, feed::Feed};
use std::{process::Stdio, sync::atomic::Ordering};

fn at(n: u64) -> u64 { 1_700_000_000 + n }
async fn wait_for<F: Fn() -> bool>(secs: u64, f: F) -> bool { for _ in 0..secs * 10 { if f() { return true } tokio::time::sleep(std::time::Duration::from_millis(100)).await } false }
fn line(v: Value) -> String { v.to_string() }

#[tokio::test] #[ignore] async fn a_rust_hub_and_a_js_hub_exchange_threads() {
    let Ok(bin) = std::env::var("BACKPLANE_JS_HUB") else { eprintln!("BACKPLANE_JS_HUB not set: skipped"); return };
    let (jp, rp) = (3915u16, 3916u16);
    let me_link = "pjsrustlink00001"; // the link as the JS hub names this machine
    let secret = botnet::fresh_secret();
    let root = std::env::temp_dir().join(format!("bp-jspeer-{}", std::process::id())); let _ = std::fs::remove_dir_all(&root);
    let (jhome, standins) = (root.join("js"), root.join("standins")); std::fs::create_dir_all(&jhome).unwrap(); std::fs::create_dir_all(&standins).unwrap();

    // ----- the Rust side: a hub behind a front that also answers GET /hello (the machine finder asks) -----
    FIXED_PORT.store(rp, Ordering::SeqCst);
    let slot: Arc<Mutex<Option<Arc<FarSync>>>> = Arc::new(Mutex::new(None)); let s2 = slot.clone();
    let extra: Extra = Arc::new(move |hub, req| { let s = s2.lock().unwrap().clone(); Box::pin(async move {
        if req.method == "GET" && req.route() == "/hello" { return Some(HttpReply::ok("application/cbor", crate::cbor::encode(&json!({"backplane":"0.14.0"})))) }
        let s = s?; if req.method != "POST" { return None }
        match req.route() { "/far/push" => Some(s.push(&hub, &req).await), "/far/log" => Some(s.log(&hub, &req).await), _ => None } }) });
    let b = node_extra("jsr", false, false, Some(extra)).await;
    FIXED_PORT.store(0, Ordering::SeqCst);
    assert_eq!(b.port, rp);
    let store = FarStore::open(&b.home).unwrap();
    let fs = FarSync::new(store.clone(), b.p.clone());
    *slot.lock().unwrap() = Some(fs.clone());

    // ----- the JS hub's home: a project, a thread and messages (two near the 400000-character page size), and this machine as a linked, verified one -----
    let big = |c: char, n: usize| c.to_string().repeat(n);
    let mut log = vec![
        line(json!({"$":"ProjectCreated","id":"proj1","title":"From JS","root":"/tmp","at":at(1)})),
        line(json!({"$":"ThreadCreated","id":"tjs1","project":"proj1","title":"A JS thread","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":at(2)})),
    ];
    for (i, (role, text)) in [("user", big('a', 390_000)), ("assistant", big('b', 390_000)), ("user", "short one".to_string()), ("assistant", big('c', 700_000))].into_iter().enumerate() {
        log.push(line(json!({"$":"MessagePosted","thread":"tjs1","msg":format!("m{}", i + 1),"role":role,"text":text,"at":at(10 + i as u64)})));
    }
    log.push(line(json!({"$":"PeerSet","id":me_link,"name":"rustbox","url":format!("http://127.0.0.1:{rp}"),"revoked":false,"at":at(20)})));
    log.push(line(json!({"$":"SettingSet","key":format!("peer.own.{me_link}"),"value":"owner","at":at(21)})));
    std::fs::write(jhome.join("events.jsonl"), log.join("\n") + "\n").unwrap();
    assert!(botnet::peer_save(&jhome, me_link, &secret));

    // ----- this side knows the JS hub the same way -----
    b.hub.commit_many(&[json!({"$":"PeerSet","id":me_link,"name":"jsbox","url":format!("http://127.0.0.1:{jp}"),"revoked":false,"at":at(30)}),
        json!({"$":"SettingSet","key":format!("peer.own.{me_link}"),"value":"owner","at":at(31)})]).await.unwrap();
    assert!(botnet::peer_save(&b.home, me_link, &secret));
    b.p.routes.machines(json!({"self":{"name":"rustbox","url":format!("http://127.0.0.1:{rp}"),"domain":""},"peers":[{"name":"jsbox","url":format!("http://127.0.0.1:{jp}"),"version":"1","named":true}]}), vec![]);

    // ----- start the JS hub (stand-in agents: none runnable; no update; its own home) -----
    let mut js = tokio::process::Command::new(&bin).args(["--home", jhome.to_str().unwrap(), "--port", &jp.to_string(), "--host", "127.0.0.1", "--no-tailscale", "--foreground"])
        .env("BACKPLANE_AGENTS", &standins).env("BACKPLANE_NO_UPDATE", "1").env("BACKPLANE_PEERS", format!("http://127.0.0.1:{rp}")).env("BACKPLANE_NAME", "jsbox")
        .env("BACKPLANE_URL", format!("http://127.0.0.1:{jp}")).env("HOME", &root).stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true).spawn().expect("start the JS hub");
    let up = wait_for(60, || std::net::TcpStream::connect(("127.0.0.1", jp)).is_ok()).await;
    assert!(up, "the JS hub listens");
    // (the JS hub's machines finder asks our /hello within moments; its feed is made on the first ask)

    // ----- pull: 503 not ready first, then the history in pages -----
    let mut caught = false;
    for _ in 0..40 { if fs.pull(&b.hub, me_link).await { caught = true; break } tokio::time::sleep(std::time::Duration::from_secs(2)).await }
    assert!(caught, "pulled from the JS hub's /far/log");
    use crate::api::FarView;
    let lid = format!("{me_link}~tjs1");
    assert!(store.has(&lid), "the JS hub's thread is mirrored");
    assert!(store.head(me_link) >= 6, "head {}", store.head(me_link));
    let p = store.page(&lid, 0, 32).unwrap();
    let mut lens: Vec<usize> = p.rows.iter().map(|r| r.text.chars().count()).collect(); lens.sort();
    assert_eq!(lens, vec![9, 390_000, 390_000, 700_000], "every message arrived whole, including the ones near the page size");
    assert_eq!(fs.catalog(me_link)[0]["id"], "proj1");
    assert!(store.epoch(me_link).starts_with("sha256="), "the JS hub's epoch is pinned: {}", store.epoch(me_link));
    // the mirror file is in the Bend format: header, items, head
    let text = std::fs::read_to_string(b.home.join(format!("far/{me_link}.jsonl"))).unwrap();
    assert!(text.lines().next().unwrap().contains("\"epoch\"") && text.lines().last().unwrap().starts_with("{\"head\":"));

    // ----- the other direction: this hub is the owner, with history the JS hub does not have, including pages at the size limit -----
    b.hub.commit_many(&[json!({"$":"ProjectCreated","id":"projrs","title":"From Rust","root":"/tmp","at":at(35)}),
        json!({"$":"ThreadCreated","id":"trs0","project":"projrs","title":"A Rust thread with history","env":"","branch":"","worktree":"","provider":"claude","model":"","at":at(36)}),
        json!({"$":"MessagePosted","thread":"trs0","msg":"h1","role":"user","text":big('x', 390_000),"at":at(37)}),
        json!({"$":"MessagePosted","thread":"trs0","msg":"h2","role":"assistant","text":big('y', 390_000),"at":at(38)}),
        json!({"$":"MessagePosted","thread":"trs0","msg":"h3","role":"user","text":"the short one from Rust","at":at(39)}),
        json!({"$":"MessagePosted","thread":"trs0","msg":"h4","role":"assistant","text":big('z', 700_000),"at":at(40)})]).await.unwrap();

    // ----- push: this hub shares a thread; the JS hub takes it and writes its own mirror file, which we read -----
    let feed = Feed::new(); let rx = feed.pushes(); feed.start(&b.hub).await; let _ = fs.feed.set(feed.clone());
    tokio::spawn(outbox::run(b.hub.clone(), b.p.clone(), outbox::Outbox::new(), rx));
    b.hub.commit_many(&[json!({"$":"ThreadCreated","id":"trs1","project":"","title":"A Rust thread","env":"","branch":"","worktree":"","provider":"claude","model":"","at":at(40)}),
        json!({"$":"MessagePosted","thread":"trs1","msg":"r1","role":"user","text":"hello from the Rust hub","at":at(41)})]).await.unwrap();
    let jfile = jhome.join(format!("far/{me_link}.jsonl"));
    let wrote = wait_for(60, || std::fs::read_to_string(&jfile).map_or(false, |t| t.contains("hello from the Rust hub") && t.contains("the short one from Rust"))).await;
    assert!(wrote, "the JS hub took our push (a gap) and pulled our /far/log, v:4 and all, keeping it in far/{me_link}.jsonl");
    // the JS hub's mirror holds every message of ours whole, the two near the page size and the 700k one included
    let mut lens: Vec<usize> = std::fs::read_to_string(&jfile).unwrap().lines().filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|o| o["c"]["$"] == "MessagePosted" && o["c"]["thread"] == "trs0").map(|o| o["c"]["text"].as_str().unwrap().chars().count()).collect();
    lens.sort();
    assert_eq!(lens, vec![23, 390_000, 390_000, 700_000], "built from our /far/log pages");
    // Rust reads the file the JS hub wrote (the Bend format), as a mirror of this hub's own history
    let copy = root.join("copy"); std::fs::create_dir_all(copy.join("far")).unwrap(); std::fs::copy(&jfile, copy.join(format!("far/{me_link}.jsonl"))).unwrap();
    let again = FarStore::open(&copy).unwrap();
    assert!(again.has(&format!("{me_link}~trs1")), "FarStore reads a mirror file written by the Bend hub");
    // ----- Bend reads a mirror file Rust wrote. A second JS hub starts on it; it reads each link's mirror back from disk and pulls from
    // the head it holds, so a stub peer that records what it is asked sees `since` = the file's head. A control: a file the Bend hub wrote. -----
    let _ = js.kill().await; tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let (j2, link2, link3) = (root.join("js2"), "pmirrorfrom0001", "pmirrorfrom0002");
    std::fs::create_dir_all(j2.join("far")).unwrap();
    let rust_file = b.home.join(format!("far/{me_link}.jsonl"));
    std::fs::copy(&rust_file, j2.join(format!("far/{link2}.jsonl"))).unwrap(); // written by Rust
    std::fs::copy(&jfile, j2.join(format!("far/{link3}.jsonl"))).unwrap();     // written by the Bend hub
    let head_of = |p: &std::path::Path| -> u64 { std::fs::read_to_string(p).unwrap().lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter_map(|o| o["head"].as_u64()).last().unwrap_or(0) };
    let (h2, h3) = (head_of(&j2.join(format!("far/{link2}.jsonl"))), head_of(&j2.join(format!("far/{link3}.jsonl"))));
    assert!(h2 > 0 && h3 > 0, "heads {h2} {h3}");
    let asked: Arc<Mutex<Vec<(String, u64, String)>>> = Arc::new(Mutex::new(vec![]));
    let stub = tokio::net::TcpListener::bind(("127.0.0.1", 3918)).await.unwrap(); let a2 = asked.clone();
    tokio::spawn(async move { use tokio::io::{AsyncReadExt, AsyncWriteExt}; loop {
        let Ok((mut s, _)) = stub.accept().await else { return }; let a3 = a2.clone();
        tokio::spawn(async move {
            let mut buf = vec![]; let mut chunk = [0u8; 8192];
            loop { let n = s.read(&mut chunk).await.unwrap_or(0); if n == 0 { break } buf.extend_from_slice(&chunk[..n]);
                if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") { let head = String::from_utf8_lossy(&buf[..at]).to_string();
                    let len = head.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
                    if buf.len() >= at + 4 + len { let peer = head.lines().find_map(|l| l.strip_prefix("X-Backplane-Peer: ")).unwrap_or("").to_string();
                        let since = serde_json::from_slice::<Value>(&buf[at + 4..]).ok().and_then(|v| v["since"].as_u64()).unwrap_or(u64::MAX);
                        if head.starts_with("POST /far/log") { a3.lock().unwrap().push((peer, since, head.replace('\r', "").replace('\n', " | ").chars().take(400).collect::<String>())); } break } } }
            let _ = s.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot ready").await; }); } });
    let mut seed = vec![];
    for l in [link2, link3] {
        seed.push(line(json!({"$":"PeerSet","id":l,"name":format!("n{l}"),"url":"http://127.0.0.1:3918","revoked":false,"at":at(50)})));
        seed.push(line(json!({"$":"SettingSet","key":format!("peer.own.{l}"),"value":"owner","at":at(51)})));
        assert!(botnet::peer_save(&j2, l, &secret));
    }
    std::fs::write(j2.join("events.jsonl"), seed.join("\n") + "\n").unwrap();
    let mut js2 = tokio::process::Command::new(&bin).args(["--home", j2.to_str().unwrap(), "--port", "3917", "--host", "127.0.0.1", "--no-tailscale", "--foreground"])
        .env("BACKPLANE_AGENTS", &standins).env("BACKPLANE_NO_UPDATE", "1").env("HOME", &root).stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true).spawn().expect("start the second JS hub");
    let both = wait_for(90, || { let g = asked.lock().unwrap(); g.iter().any(|(p, _, _)| p == link2) && g.iter().any(|(p, _, _)| p == link3) }).await;
    let got = asked.lock().unwrap().clone();
    assert!(both, "the second JS hub pulled both links (it asked: {got:?})");
    let since = |l: &str| got.iter().find(|(p, _, _)| p == l).map(|x| x.1).unwrap();
    assert_eq!(since(link3), h3, "CONTROL: the Bend hub pulls from the head of the file the Bend hub wrote: asked {got:?}");
    assert_eq!(since(link2), h2, "the Bend hub read the mirror file Rust wrote (it pulls from its head {h2}, not from 0): asked {got:?}");
    let _ = js2.kill().await;
    let _ = std::fs::remove_dir_all(&root);
}
