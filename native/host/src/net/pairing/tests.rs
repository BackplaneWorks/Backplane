//! Law-named tests for the pairing registry. hub's file is
//! read and written by both hubs, so the first test reads a registry the JS store wrote.
use super::*;
use std::sync::{atomic::{AtomicU64, Ordering}, Arc};

struct Env { dir: PathBuf, t: Arc<AtomicU64>, r: Arc<AtomicU64> }
impl Env {
    fn new(tag: &str) -> Env {
        let dir = std::env::temp_dir().join(format!("bp-net-{tag}-{}-{}", std::process::id(), hex::encode(urandom(4))));
        std::fs::create_dir_all(&dir).unwrap();
        Env { dir, t: Arc::new(AtomicU64::new(1_000_000)), r: Arc::new(AtomicU64::new(1)) }
    }
    fn reg(&self) -> Registry {
        let (t, r) = (self.t.clone(), self.r.clone());
        Registry::with(&self.dir, "root-token", Box::new(move || t.load(Ordering::SeqCst)),
            Box::new(move |n| { let s = r.fetch_add(1, Ordering::SeqCst); (0..n).map(|i| ((s * 31 + i as u64 * 7) % 251) as u8).collect() })).unwrap()
    }
    fn advance(&self, ms: u64) { self.t.fetch_add(ms, Ordering::SeqCst); }
}
impl Drop for Env { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.dir); } }
fn routes() -> Vec<String> { vec!["https://m.tail.ts.net".into()] }

#[test] fn pair_enroll_once() {
    let e = Env::new("once"); let r = e.reg();
    let inv = r.create(&routes());
    let link = inv["link"].as_str().unwrap();
    let secret = link.split("invite=").nth(1).unwrap().split('&').next().unwrap();
    let dev = r.enroll(secret, "Phone");
    assert_eq!(dev["token"].as_str().unwrap().len(), 32);
    assert_eq!(r.enroll(secret, "Again")["error"], "used_code", "an invitation enrolls once");
    assert_eq!(r.resolve(dev["token"].as_str().unwrap(), 1, "/x"), dev["token"].as_str().unwrap());
}
#[test] fn pair_code_matches_secret_forms() {
    let e = Env::new("code"); let r = e.reg();
    let inv = r.create(&routes());
    let code = inv["code"].as_str().unwrap().to_lowercase();
    let spaced = format!("{}-{}", &code[..4], &code[4..]);
    assert!(r.enroll(&spaced, "x")["token"].is_string(), "the short code is typed in any case, with a dash or a space");
}
#[test] fn pair_unknown_and_expired_and_revoked() {
    let e = Env::new("states"); let r = e.reg();
    assert_eq!(r.enroll("nope", "x")["error"], "wrong_code");
    let a = r.create(&routes()); let code_a = a["code"].as_str().unwrap().to_string();
    e.advance(600_001);
    assert_eq!(r.enroll(&code_a, "x")["error"], "expired_code");
    let b = r.create(&routes()); let id_b = b["id"].as_str().unwrap().to_string(); let code_b = b["code"].as_str().unwrap().to_string();
    assert_eq!(r.uninvite(&id_b)["ok"], true);
    assert_eq!(r.enroll(&code_b, "x")["error"], "revoked_code");
}
#[test] fn pair_guessing_is_bounded_hub_wide() {
    let e = Env::new("rate"); let r = e.reg();
    for _ in 0..10 { assert_eq!(r.enroll("guess", "x")["error"], "wrong_code"); }
    assert_eq!(r.enroll("guess", "x")["error"], "rate_limited");
    let inv = r.create(&routes());
    assert_eq!(r.enroll(inv["code"].as_str().unwrap(), "x")["error"], "rate_limited", "even the right code waits");
    e.advance(60_001);
    assert!(r.enroll(inv["code"].as_str().unwrap(), "x")["token"].is_string());
}
#[test] fn pair_open_invitations_are_capped() {
    let e = Env::new("cap"); let r = e.reg();
    for _ in 0..16 { assert!(r.create(&routes())["id"].is_string()); }
    assert_eq!(r.create(&routes())["error"], "too_many_invitations");
}
#[test] fn pair_routes_are_vetted() {
    let e = Env::new("routes"); let r = e.reg();
    assert_eq!(r.create(&["ftp://x".into(), "http://u:p@h".into(), "http://h/#f".into(), "http://h/?q".into(), "http://".into()])["error"], "no_reachable_address");
    let ok = r.create(&["http://127.0.0.1:3787".into(), "http://127.0.0.1:3787".into()]);
    assert_eq!(ok["routes"].as_array().unwrap().len(), 1, "duplicates collapse");
    assert_eq!(ok["localOnly"], true);
    let l = ok["link"].as_str().unwrap();
    assert!(l.starts_with("http://127.0.0.1:3787/pair#invite=") && l.contains("&routes=%5B%22http%3A%2F%2F127.0.0.1%3A3787%22%5D"), "{l}");
}
#[test] fn pair_revoked_device_stops_and_its_clients_are_named() {
    let e = Env::new("revoke"); let r = e.reg();
    let inv = r.create(&routes());
    let dev = r.enroll(inv["code"].as_str().unwrap(), "Phone");
    let (tok, id) = (dev["token"].as_str().unwrap().to_string(), dev["id"].as_str().unwrap().to_string());
    assert_eq!(r.resolve(&tok, 7, "/ws"), tok);
    assert!(r.active(7));
    let out = r.revoke(&id);
    assert_eq!(out["clients"], json!([7]));
    assert_eq!(r.resolve(&tok, 8, "/ws"), "", "a revoked credential resolves to nothing");
    assert!(!r.active(7), "a connection that raced the revocation is not published");
    assert_eq!(r.revoke("ffffffffffffffffffffffffffffffff")["error"], "unknown_device");
}
#[test] fn pair_revoking_a_used_invitation_revokes_its_device() {
    let e = Env::new("uninv"); let r = e.reg();
    let inv = r.create(&routes());
    let dev = r.enroll(inv["code"].as_str().unwrap(), "Phone"); let tok = dev["token"].as_str().unwrap().to_string();
    r.resolve(&tok, 3, "/ws");
    let out = r.uninvite(inv["id"].as_str().unwrap());
    assert_eq!(out["clients"], json!([3]));
    assert_eq!(r.resolve(&tok, 4, "/x"), "");
    assert_eq!(r.uninvite("nope")["error"], "unknown_invitation");
}
#[test] fn pair_registry_survives_a_restart_and_keeps_hashes_only() {
    let e = Env::new("persist"); let tok;
    { let r = e.reg(); let inv = r.create(&routes()); tok = r.enroll(inv["code"].as_str().unwrap(), "Phone")["token"].as_str().unwrap().to_string(); }
    let text = std::fs::read_to_string(e.dir.join("devices-v1.json")).unwrap();
    assert!(!text.contains(&tok), "the credential is not stored");
    assert!(text.contains(&digest(&tok)));
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(e.dir.join("devices-v1.json")).unwrap().permissions().mode() & 0o777, 0o600);
    let r2 = e.reg(); assert_eq!(r2.resolve(&tok, 1, "/x"), tok);
    assert!(r2.list()["devices"][0].get("hash").is_none(), "a listing never carries a hash");
}
#[test] fn pair_registry_of_another_hub_is_refused_and_kept() {
    let e = Env::new("owner"); { let r = e.reg(); r.create(&routes()); }
    let before = std::fs::read_to_string(e.dir.join("devices-v1.json")).unwrap();
    assert!(Registry::open(&e.dir, "someone-else").is_err());
    std::fs::write(e.dir.join("devices-v1.json"), "{not json").unwrap();
    assert!(Registry::open(&e.dir, "root-token").is_err());
    std::fs::write(e.dir.join("devices-v1.json"), &before).unwrap();
    assert!(Registry::open(&e.dir, "root-token").is_ok());
}
#[test] fn pair_reads_a_registry_the_js_hub_wrote() {
    let e = Env::new("js");
    let (tok, dev) = ("0123456789abcdef0123456789abcdef", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let body = json!({"version":1,"owner":digest("root-token"),"devices":[{"id":dev,"hash":digest(tok),"name":"Pixel","createdAt":1,"lastSeen":2,"revoked":false}],
        "invitations":[{"id":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","hash":digest("x"),"codeHash":digest("Y"),"createdAt":1,"expiresAt":2,"used":true,"revoked":false,"device":dev,"usedAt":2},
        {"id":"cccccccccccccccccccccccccccccccc","kind":"machine","peer":"p0123456789abcdef","hash":digest("m"),"codeHash":digest("n"),"createdAt":3,"expiresAt":4,"used":true,"revoked":false,"name":"desk","usedAt":3}]});
    std::fs::write(e.dir.join("devices-v1.json"), body.to_string()).unwrap();
    let r = e.reg();
    assert_eq!(r.resolve(tok, 1, "/x"), tok);
    let l = r.list();
    assert_eq!(l["invitations"][1]["deviceName"], "Pixel");
    assert_eq!(l["invitations"][0]["kind"], "machine");
    assert_eq!(l["invitations"][0]["device"], "p0123456789abcdef");
}
#[test] fn pair_machine_invites_link_once_and_enroll_nothing() {
    let e = Env::new("machine"); let r = e.reg();
    assert_eq!(r.machine("nope")["error"], "invalid_request");
    let m = r.machine("p0123456789abcdef"); assert_eq!(m["ok"], true);
    assert_eq!(r.machine("p0123456789abcdef")["error"], "invalid_request", "one invite per link");
    assert_eq!(r.enroll("anything", "x")["error"], "wrong_code");
    assert_eq!(r.link("p0123456789abcdef", "desk")["ok"], true);
    assert_eq!(r.link("p0123456789abcdef", "desk")["error"], "used_code");
    assert_eq!(r.link("pffffffffffffffff", "x")["error"], "unknown_invitation");
    let out = r.uninvite(m["id"].as_str().unwrap());
    assert_eq!(out["peer"], "p0123456789abcdef", "revoking a used machine invite unlinks the machine");
}
#[test] fn pair_closed_invitations_are_trimmed_but_open_ones_never() {
    let e = Env::new("trim"); let r = e.reg();
    let open_one = r.create(&routes());
    for _ in 0..120 { e.advance(1); let m = r.create(&routes()); r.uninvite(m["id"].as_str().unwrap()); }
    let l = r.list(); let n = l["invitations"].as_array().unwrap().len();
    assert!(n <= 101 + 16 && n >= 100, "{n}");
    assert!(l["invitations"].as_array().unwrap().iter().any(|i| i["id"] == open_one["id"]));
}
#[test] fn pair_known_origins() {
    let machines = json!({"self":{"url":"https://me.tail.ts.net","name":"me","domain":"tail.ts.net."},"peers":[{"url":"http://100.1.1.1:3787","name":"desk"}]});
    let k = |o: &str| known(o, &["http://127.0.0.1:3787".into()], &machines, &["https://boat.example".into()]);
    assert!(k("https://me.tail.ts.net")); assert!(k("http://100.1.1.1:3787")); assert!(k("https://boat.example")); assert!(k("http://127.0.0.1:3787"));
    assert!(k("https://desk.tail.ts.net:8443"), "a machine's MagicDNS name in the owner's domain");
    assert!(!k("https://desk.evil.net")); assert!(!k("https://other.tail.ts.net")); assert!(!k("http://100.1.1.1:3788"));
    assert!(!k("http://u@100.1.1.1:3787")); assert!(!k("ftp://me.tail.ts.net")); assert!(!k("not a url"));
}

#[test] fn pair_known_refuses_what_new_url_throws() {
    let machines = json!({"self":{"url":"https://me.tail.ts.net","name":"me","domain":"tail.ts.net"},"peers":[]});
    let k = |o: &str| known(o, &[], &machines, &[]);
    assert!(k("https://me.tail.ts.net") && k("https://me.tail.ts.net:443") && k("HTTPS://ME.tail.ts.net/x"), "default port and case are the same origin");
    for bad in ["https://me.tail.ts.net:99999", "https://me.x:1.tail.ts.net", "https://me.a|b.tail.ts.net", "https://me.a b.tail.ts.net", "https://.tail.ts.net:x", "https://[::1", "https://me.tail.ts.net:123456"] {
        assert!(!k(bad), "{bad}");
    }
    assert!(authority("[::1]:3787").is_some() && authority("[zz]").is_none() && authority("a:65535").is_some() && authority("a:65536").is_none());
    let e = Env::new("badroute"); let r = e.reg();
    assert_eq!(r.create(&["https://h:99999".into(), "https://a|b".into()])["error"], "no_reachable_address");
}
#[test] fn pair_revoked_is_js_truthy() {
    let e = Env::new("truthy");
    let tok = "0123456789abcdef0123456789abcdef";
    for (i, v) in [json!(1), json!("yes"), json!(true), json!({}), json!([])].into_iter().enumerate() {
        let body = json!({"version":1,"owner":digest("root-token"),"devices":[{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","hash":digest(tok),"name":"x","createdAt":1,"lastSeen":2,"revoked":v}]});
        std::fs::write(e.dir.join("devices-v1.json"), body.to_string()).unwrap();
        assert_eq!(e.reg().resolve(tok, 1, "/x"), "", "case {i}");
    }
    for v in [json!(0), json!(""), json!(null), json!(false)] {
        let body = json!({"version":1,"owner":digest("root-token"),"devices":[{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","hash":digest(tok),"name":"x","createdAt":1,"lastSeen":2,"revoked":v}]});
        std::fs::write(e.dir.join("devices-v1.json"), body.to_string()).unwrap();
        assert_eq!(e.reg().resolve(tok, 1, "/x"), tok);
    }
}
#[test] fn pair_unwritable_stamp_refuses_the_device() {
    let e = Env::new("stamp"); let r = e.reg();
    let inv = r.create(&routes()); let tok = r.enroll(inv["code"].as_str().unwrap(), "P")["token"].as_str().unwrap().to_string();
    e.advance(61_000);
    // the folder can no longer take a new file: the stamp cannot be written
    std::fs::remove_dir_all(&e.dir).unwrap();
    assert_eq!(r.resolve(&tok, 1, "/x"), "", "pairing_resolve's catch returns nothing");
}
