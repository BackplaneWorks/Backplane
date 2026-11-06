//! Test-only: the outs driven by the hub's `alert.*` records (a push, a webhook, a desk notice) leave nothing on a copy of a log.
//!
//! What exists in Rust today: the alert records themselves (`state/turnend.rs` writes `alert.<thread>` into the copy's own log; nothing leaves) and the webhook
//! sender `settings::webhook::deliver` (gated twice: its own `copy::on()` and the process gate's `Kind::Net` refusal of curl). Nothing calls `deliver` yet (the
//! pipeline that decides *when* is net's, not ported). APNs push, the Live Activity/island sync and desk notices (`{"t":"notify"}`) are NOT PORTED: no code sends them.
//! The flag is read once per process, so the probe runs in a child, in both modes: with the flag the local listener sees nothing; without it the same child
//! posts and the listener sees the POST (so the harness can see a send at all).
use crate::net::peering::tests::node_extra;
use serde_json::json;
use std::sync::{atomic::{AtomicUsize, Ordering}, Arc};

#[tokio::test(flavor = "multi_thread")] async fn alert_outs_probe() {
    if std::env::var("BP_OUTS_PROBE").is_err() { return }
    let copy = crate::copy::on();
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let port = l.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0)); let h2 = hits.clone();
    std::thread::spawn(move || { use std::io::{Read, Write}; for s in l.incoming() { let Ok(mut s) = s else { return }; h2.fetch_add(1, Ordering::SeqCst); let mut b = [0u8; 4096]; let _ = s.read(&mut b); let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"); } });
    let n = node_extra("outs", false, false, None).await;
    n.hub.commit(&json!({"$":"SettingSet","key":"push.webhook.url","value":format!("http://127.0.0.1:{port}/hook"),"at":1})).await.unwrap();
    // the record a turn's end writes; it stays in this log
    n.hub.commit(&json!({"$":"SettingSet","key":"alert.t1","value":"done\tturn-t1\tm\t1.1\t","at":2})).await.unwrap();
    let r = crate::settings::webhook::deliver(&n.hub, "t1", "done", "completed", "", "", "hi", "T").await;
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    if copy {
        assert!(matches!(&r, Err(e) if e.contains("copy")), "{r:?}");
        assert_eq!(hits.load(Ordering::SeqCst), 0, "nothing reached the listener on a copy");
        // and a raw curl by the same gate is refused too
        let o = crate::proc::std_cmd(crate::proc::Kind::Net, "curl").arg(format!("http://127.0.0.1:{port}/")).output();
        assert!(o.is_err() || !o.unwrap().status.success(), "curl is refused on a copy");
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    } else {
        assert!(r.is_ok(), "{r:?}"); assert!(hits.load(Ordering::SeqCst) >= 1, "without the flag the same call posts");
    }
}
fn child(copy: bool) -> std::process::Output {
    let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, std::env::current_exe().unwrap());
    c.args(["net::copy_outs::alert_outs_probe", "--exact", "--nocapture", "--test-threads=1"]).env("BP_OUTS_PROBE", "1").env_remove("BACKPLANE_AGENTS");
    if copy { c.env("BACKPLANE_COPY", "1"); } else { c.env_remove("BACKPLANE_COPY"); }
    c.output().unwrap()
}
#[test] fn no_alert_driven_out_leaves_on_a_copy_and_the_same_probe_posts_without_the_flag() {
    for copy in [true, false] {
        let o = child(copy);
        assert!(o.status.success() && String::from_utf8_lossy(&o.stdout).contains("alert_outs_probe ... ok"), "copy={copy}: {}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    }
}
