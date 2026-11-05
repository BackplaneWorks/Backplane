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

    // ----- push: this hub shares a thread; the JS hub takes it and writes its own mirror file, which we read -----
    let feed = Feed::new(); let rx = feed.pushes(); feed.start(&b.hub).await; let _ = fs.feed.set(feed.clone());
    tokio::spawn(outbox::run(b.hub.clone(), b.p.clone(), outbox::Outbox::new(), rx));
    b.hub.commit_many(&[json!({"$":"ThreadCreated","id":"trs1","project":"","title":"A Rust thread","env":"","branch":"","worktree":"","provider":"claude","model":"","at":at(40)}),
        json!({"$":"MessagePosted","thread":"trs1","msg":"r1","role":"user","text":"hello from the Rust hub","at":at(41)})]).await.unwrap();
    let jfile = jhome.join(format!("far/{me_link}.jsonl"));
    let wrote = wait_for(40, || std::fs::read_to_string(&jfile).map_or(false, |t| t.contains("hello from the Rust hub"))).await;
    assert!(wrote, "the JS hub took our push and kept it in far/{me_link}.jsonl");
    // Rust reads the file the JS hub wrote (the Bend format), as a mirror of this hub's own history
    let copy = root.join("copy"); std::fs::create_dir_all(copy.join("far")).unwrap(); std::fs::copy(&jfile, copy.join(format!("far/{me_link}.jsonl"))).unwrap();
    let again = FarStore::open(&copy).unwrap();
    assert!(again.has(&format!("{me_link}~trs1")), "FarStore reads a mirror file written by the Bend hub");
    let _ = js.kill().await;
    let _ = std::fs::remove_dir_all(&root);
}
