//! Law-named tests for the pairing registry. hub's file is
//! read and written by both hubs, so the first test reads a registry the JS store wrote.
use super::*;
use std::sync::{atomic::{AtomicU64, Ordering}, Arc};

struct Env { dir: crate::plat::tmp::TempDir, t: Arc<AtomicU64>, r: Arc<AtomicU64> }
impl Env {
    fn new(tag: &str) -> Env {
        let dir = crate::plat::tmp::dir(&format!("net-{tag}"));
        Env { dir, t: Arc::new(AtomicU64::new(1_000_000)), r: Arc::new(AtomicU64::new(1)) }
    }
    fn reg(&self) -> Registry {
        let (t, r) = (self.t.clone(), self.r.clone());
        Registry::with(&self.dir, "root-token", Box::new(move || t.load(Ordering::SeqCst)),
            Box::new(move |n| { let s = r.fetch_add(1, Ordering::SeqCst); (0..n).map(|i| ((s * 31 + i as u64 * 7) % 251) as u8).collect() })).unwrap()
    }
    fn advance(&self, ms: u64) { self.t.fetch_add(ms, Ordering::SeqCst); }
}
fn routes() -> Vec<String> { vec!["https://m.tail.ts.net".into()] }

#[test] fn pair_enroll_once() {
    let e = Env::new("once"); let r = e.reg();
    let inv = r.create(&routes());
    let link = inv["link"].as_str().unwrap();
    let secret = link.split("invite=").nth(1).unwrap().split('&').next().unwrap();
    let dev = r.enroll("t", secret, "Phone");
    assert_eq!(dev["token"].as_str().unwrap().len(), 32);
    assert_eq!(r.enroll("t", secret, "Again")["error"], "used_code", "an invitation enrolls once");
    assert_eq!(r.resolve(dev["token"].as_str().unwrap(), 1, "/x"), dev["token"].as_str().unwrap());
}
#[test] fn pair_code_matches_secret_forms() {
    let e = Env::new("code"); let r = e.reg();
    let inv = r.create(&routes());
    let code = inv["code"].as_str().unwrap().to_lowercase();
    let spaced = format!("{}-{}", &code[..4], &code[4..]);
    assert!(r.enroll("t", &spaced, "x")["token"].is_string(), "the short code is typed in any case, with a dash or a space");
}
#[test] fn pair_unknown_and_expired_and_revoked() {
    let e = Env::new("states"); let r = e.reg();
    assert_eq!(r.enroll("t", "nope", "x")["error"], "wrong_code");
    let a = r.create(&routes()); let code_a = a["code"].as_str().unwrap().to_string();
    e.advance(600_001);
    assert_eq!(r.enroll("t", &code_a, "x")["error"], "expired_code");
    let b = r.create(&routes()); let id_b = b["id"].as_str().unwrap().to_string(); let code_b = b["code"].as_str().unwrap().to_string();
    assert_eq!(r.uninvite(&id_b)["ok"], true);
    assert_eq!(r.enroll("t", &code_b, "x")["error"], "revoked_code");
}
#[test] fn pair_guessing_is_bounded_per_client() {
    let e = Env::new("rate"); let r = e.reg();
    for _ in 0..10 { assert_eq!(r.enroll("t", "guess", "x")["error"], "wrong_code"); }
    assert_eq!(r.enroll("t", "guess", "x")["error"], "rate_limited");
    let inv = r.create(&routes());
    assert_eq!(r.enroll("t", inv["code"].as_str().unwrap(), "x")["error"], "rate_limited", "even the right code waits");
    e.advance(60_001);
    assert!(r.enroll("t", inv["code"].as_str().unwrap(), "x")["token"].is_string());
}
#[test] fn pair_one_client_cannot_rate_limit_another() {
    let e = Env::new("rate-each"); let r = e.reg();
    for _ in 0..10 { assert_eq!(r.enroll("1.1.1.1", "guess", "x")["error"], "wrong_code"); }
    assert_eq!(r.enroll("1.1.1.1", "guess", "x")["error"], "rate_limited");
    let inv = r.create(&routes());
    assert!(r.enroll("2.2.2.2", inv["code"].as_str().unwrap(), "x")["token"].is_string(), "another address still pairs");
    for _ in 0..10 { assert_eq!(r.machine_claim("1.1.1.1", "guess")["error"], "wrong_code"); }
    assert_eq!(r.machine_claim("1.1.1.1", "guess")["error"], "rate_limited");
    assert_eq!(r.machine_claim("2.2.2.2", "guess")["error"], "wrong_code", "claims are kept per client too");
}
#[test] fn pair_hub_wide_cap_backstops_many_clients() {
    let e = Env::new("rate-all"); let r = e.reg();
    for i in 0..super::GUESS_ALL { assert_eq!(r.enroll(&format!("10.0.0.{i}"), "guess", "x")["error"], "wrong_code"); }
    assert_eq!(r.enroll("192.168.0.1", "guess", "x")["error"], "rate_limited", "a fresh address meets the hub-wide cap");
    e.advance(60_001);
    assert_eq!(r.enroll("192.168.0.1", "guess", "x")["error"], "wrong_code");
}
#[test] fn pair_budget_map_prunes_expired_clients() {
    let mut b = super::Budget::default();
    for i in 0..50 { assert!(b.take(&format!("c{i}"), 1000)); }
    assert_eq!(b.by.len(), 50);
    assert!(b.take("late", 1000 + 60_001));
    assert_eq!((b.by.len(), b.all.len()), (1, 1), "expired clients leave the map");
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
    let dev = r.enroll("t", inv["code"].as_str().unwrap(), "Phone");
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
    let dev = r.enroll("t", inv["code"].as_str().unwrap(), "Phone"); let tok = dev["token"].as_str().unwrap().to_string();
    r.resolve(&tok, 3, "/ws");
    let out = r.uninvite(inv["id"].as_str().unwrap());
    assert_eq!(out["clients"], json!([3]));
    assert_eq!(r.resolve(&tok, 4, "/x"), "");
    assert_eq!(r.uninvite("nope")["error"], "unknown_invitation");
}
#[test] fn pair_registry_survives_a_restart_and_keeps_hashes_only() {
    let e = Env::new("persist"); let tok;
    { let r = e.reg(); let inv = r.create(&routes()); tok = r.enroll("t", inv["code"].as_str().unwrap(), "Phone")["token"].as_str().unwrap().to_string(); }
    let text = std::fs::read_to_string(e.dir.join("devices-v1.json")).unwrap();
    assert!(!text.contains(&tok), "the credential is not stored");
    assert!(text.contains(&digest(&tok)));
    use crate::plat::fs::PermissionsExt;
    assert_eq!(crate::plat::fs::mode_bits(std::fs::metadata(e.dir.join("devices-v1.json")).unwrap().permissions().mode() & 0o777, 0o600), 0o600);
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
    assert_eq!(r.enroll("t", "anything", "x")["error"], "wrong_code");
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
    let inv = r.create(&routes()); let tok = r.enroll("t", inv["code"].as_str().unwrap(), "P")["token"].as_str().unwrap().to_string();
    e.advance(61_000);
    // the folder can no longer take a new file: the stamp cannot be written
    std::fs::remove_dir_all(&e.dir).unwrap();
    assert_eq!(r.resolve(&tok, 1, "/x"), "", "pairing_resolve's catch returns nothing");
}

// ---- the docking layouts follow the registry's own liveness and every retirement
fn keep_layout(home: &Path, device: &str) {
    let store = crate::data::layout::global(home);
    let mut e = crate::data::layout::Entry::default();
    // a layout of two tiles (a one-tile layout is the page, and is not kept)
    let tile = |id: &str, kind: &str| json!({"type":"tile","id":id,"kind":kind,"source":"","explicit":false});
    let two = json!({"version":1,"root":{"type":"split","id":"s1","axis":"row","ratio":0.5,"a":tile("t1","board"),"b":tile("t2","gerbers")},"focus":"t1","maximized":null});
    e.drafts.insert("@view.layout:t".into(), json!(two.to_string()));
    e.rev = 1;
    store.put(device, "t", e);
    store.flush();
}
fn has_layout(home: &Path, device: &str) -> bool { crate::data::layout::global(home).entry(device, "t").is_some() }

#[test] fn device_ids_use_the_registrys_own_revoked_rule() {
    let e = Env::new("ids-truthy"); let r = e.reg();
    let mut ids = vec![];
    for n in 0..4 { let inv = r.create(&routes()); ids.push(r.enroll("t", inv["code"].as_str().unwrap(), &format!("P{n}"))["id"].as_str().unwrap().to_string()); }
    assert_eq!(r.device_ids().len(), 4);
    // the file is read back by the next hub: `"yes"`, `1`, `true` are all revoked (never narrower than the JS rule); `false`, `0`, `""`, null are not
    let file = e.dir.join("devices-v1.json");
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let marks = [json!("yes"), json!(1), json!(true), json!(0)];
    for (d, m) in v["devices"].as_array_mut().unwrap().iter_mut().zip(marks) { d["revoked"] = m; }
    std::fs::write(&file, v.to_string()).unwrap();
    let again = e.reg();
    assert_eq!(again.device_ids(), vec![ids[3].clone()], "only the device whose `revoked` is 0 is live");
}
#[test] fn a_retired_device_loses_its_layouts_on_every_path_and_a_disconnect_keeps_them() {
    let e = Env::new("retire-layouts"); let r = e.reg();
    let pair = |n: &str| { let inv = r.create(&routes()); let d = r.enroll("t", inv["code"].as_str().unwrap(), n); (d["id"].as_str().unwrap().to_string(), d["token"].as_str().unwrap().to_string(), inv["id"].as_str().unwrap().to_string()) };
    let (a, ta, _) = pair("A"); let (b, tb, inv_b) = pair("B"); let (c, _, _) = pair("C");
    for d in [&a, &b, &c] { keep_layout(&e.dir, d); }
    // a disconnect keeps the layout (the person comes back)
    r.resolve(&ta, 1, "/ws"); r.forget(1);
    assert!(has_layout(&e.dir, &a));
    // revoke(device), a browser's logout (it is `revoke`), and the revoke of a used invitation (which retires its device without `revoke`): a device with no connection too
    r.resolve(&tb, 2, "/ws");
    assert_eq!(r.uninvite(&inv_b)["ok"], true);
    assert!(!has_layout(&e.dir, &b), "the used invitation's device is retired: its layouts go");
    assert_eq!(r.revoke(&a)["ok"], true);
    assert!(!has_layout(&e.dir, &a) && has_layout(&e.dir, &c), "only the retired device's");
    // ... and from the FILE: a store reopened from it holds nothing of them
    let reopened = crate::data::layout::Store::open(Some(e.dir.join("viewer-layouts.json")));
    assert!(reopened.entry(&a, "t").is_none() && reopened.entry(&b, "t").is_none() && reopened.entry(&c, "t").is_some());
    // an unused invitation has no device; a machine invitation retires none
    let open_inv = r.create(&routes());
    assert_eq!(r.uninvite(open_inv["id"].as_str().unwrap())["ok"], true);
    assert!(has_layout(&e.dir, &c));
}
