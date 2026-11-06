//! Which machine a peer is, and whether this hub pairs with it: client.bend's `Far.hit/machine`, `Mate.*`
//! (pure). The owner's machines are the switcher's JSON: `{"self":{name,url,domain},"peers":[{name,url,named?}]}`.
//! Laws `far_machine_*`, `mate_*`.
use serde_json::Value;

/// A url's host:port ("https://box.ts.net:3787/x" → "box.ts.net:3787").
pub fn hostport(url: &str) -> &str {
    let r = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).unwrap_or(url);
    r.split('/').next().unwrap_or("")
}
pub fn host(url: &str) -> &str { hostport(url).split(':').next().unwrap_or("") }
/// "" when the url names no port.
pub fn port(url: &str) -> &str { hostport(url).split(':').nth(1).unwrap_or("") }
/// A host's first label, lowercased (a MagicDNS name's machine).
fn label(url: &str) -> String { host(url).split('.').next().unwrap_or("").to_lowercase() }
/// Whether a url's host is a machine on the owner's tailnet (domain "" when unknown, then none is).
fn ours(purl: &str, domain: &str) -> bool { !domain.is_empty() && host(purl).to_lowercase().ends_with(&format!(".{}", domain.to_lowercase())) }

/// Whether the machine named `n` at `url` is the peer called `pname` at `purl`: the same host:port, or a
/// machine on the owner's tailnet whose name is the peer's MagicDNS label or its own, on the same port. A name
/// alone is not enough: someone else's "laptop" is not the owner's.
pub fn hit(n: &str, url: &str, pname: &str, purl: &str, domain: &str) -> bool {
    (!url.is_empty() && hostport(url) == hostport(purl))
        || (!n.is_empty() && ours(purl, domain) && (port(purl).is_empty() || port(url) == port(purl))
            && (n.to_lowercase() == label(purl) || n.to_lowercase() == pname.to_lowercase()))
}
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }

/// Which of the owner's machines is the peer: Some("") for this one, Some(url) for another, None when it is none.
pub fn machine(j: &Value, pname: &str, purl: &str) -> Option<String> {
    let me = &j["self"]; let domain = s(me, "domain");
    if hit(s(me, "name"), s(me, "url"), pname, purl, domain) { return Some(String::new()) }
    j["peers"].as_array()?.iter().find(|p| hit(s(p, "name"), s(p, "url"), pname, purl, domain)).map(|p| s(p, "url").to_string())
}

pub struct Peer { pub id: String, pub name: String, pub url: String }

/// Mate.known: some other machine (not this one, "" is this one).
pub fn known(m: &Option<String>) -> bool { m.as_ref().map_or(false, |u| !u.is_empty()) }
/// "sync.<name>" = "off": nothing goes to the machine or comes from it.
pub fn sync_key(name: &str) -> String { format!("sync.{}", name.to_lowercase()) }
/// Mate.linked: a live link already reaches the machine (and the owner has not said it is not theirs).
pub fn linked(peers: &[Peer], own_no: &dyn Fn(&str) -> bool, n: &str, url: &str, domain: &str) -> bool {
    peers.iter().any(|p| hit(n, url, &p.name, &p.url, domain) && !own_no(&p.id))
}
/// What this hub answers a machine that asks to pair: 0 an invite, 1 no (sync with it is off here), 2 no (a live
/// link already reaches it), 3 no (it is none of the machines this hub found: a link must reach back).
pub fn answer(j: &Value, peers: &[Peer], sync_off: &dyn Fn(&str) -> bool, own_no: &dyn Fn(&str) -> bool, n: &str, url: &str) -> u8 {
    if !known(&machine(j, n, url)) { 3 }
    else if sync_off(n) { 1 }
    else if linked(peers, own_no, n, url, s(&j["self"], "domain")) { 2 } else { 0 }
}
/// Mate.me / Mate.local: a hub reached on this machine's loopback is compared with this one's loopback address.
fn local(url: &str) -> bool { host(url) == "127.0.0.1" || host(url) == "localhost" }
fn me_for(me: &str, url: &str) -> String { if local(url) { format!("http://127.0.0.1:{}", port(me)) } else { me.to_string() } }
/// Mate.wants: this one's address is the lower (so of two hubs one asks), neither link nor setting says otherwise.
pub fn wants(n: &str, url: &str, peers: &[Peer], sync_off: &dyn Fn(&str) -> bool, own_no: &dyn Fn(&str) -> bool, me: &str, domain: &str) -> bool {
    !url.is_empty() && !me.is_empty() && me_for(me, url).as_str() < url && !sync_off(n) && !linked(peers, own_no, n, url, domain)
}
/// Mate.due: the machines (name, url) this hub should ask to pair now. A hub started with --no-tailscale pairs only
/// with those BACKPLANE_PEERS names ("named"): a throwaway hub never links itself to the owner's real ones.
pub fn due(tailnet: bool, j: &Value, peers: &[Peer], sync_off: &dyn Fn(&str) -> bool, own_no: &dyn Fn(&str) -> bool) -> Vec<(String, String)> {
    let (me, domain) = (s(&j["self"], "url"), s(&j["self"], "domain"));
    j["peers"].as_array().into_iter().flatten()
        .filter(|m| (tailnet || m["named"].as_bool().unwrap_or(false)) && wants(s(m, "name"), s(m, "url"), peers, sync_off, own_no, me, domain))
        .map(|m| (s(m, "name").to_string(), s(m, "url").to_string())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn demo(me: &str) -> Value { json!({"self":{"name":"laptop","url":me,"domain":"t.ts.net"},"peers":[{"name":"desk","url":"http://100.2.0.1:3787","version":"1"}]}) }
    fn no(_: &str) -> bool { false }

    #[test] fn far_url_parts() {
        assert_eq!((hostport("https://box.ts.net:3787/x"), host("https://box.ts.net:3787/x"), port("https://box.ts.net:3787/x")), ("box.ts.net:3787", "box.ts.net", "3787"));
        assert_eq!((hostport("http://h"), port("http://h"), label("http://Box.ts.net:1")), ("h", "", "box".to_string()));
    }
    #[test] fn far_machine_matches_by_address_or_by_owned_name() {
        let j = demo("http://100.1.0.1:3787");
        assert_eq!(machine(&j, "x", "http://100.2.0.1:3787"), Some("http://100.2.0.1:3787".into()));
        assert_eq!(machine(&j, "x", "http://100.1.0.1:3787"), Some(String::new()), "this machine");
        assert_eq!(machine(&j, "x", "http://100.9.9.9:3787"), None);
        // a name alone is not enough: someone else's "desk" is not the owner's
        assert_eq!(machine(&j, "desk", "http://evil.example:3787"), None);
        // the owner's tailnet name, on the same port
        assert_eq!(machine(&j, "desk", "http://desk.t.ts.net:3787"), Some("http://100.2.0.1:3787".into()), "the owner's MagicDNS name of a machine it found");
        let j2 = json!({"self":{"name":"laptop","url":"http://laptop.t.ts.net:3787","domain":"t.ts.net"},"peers":[{"name":"desk","url":"http://desk.t.ts.net:3787"}]});
        assert_eq!(machine(&j2, "Desk", "http://desk.t.ts.net:3787"), Some("http://desk.t.ts.net:3787".into()));
        assert_eq!(machine(&j2, "desk", "http://desk.t.ts.net:9999"), None, "another port");
    }
    #[test] fn mate_answer_codes() {
        let j = demo("http://100.1.0.1:3787");
        let peers = vec![Peer { id: "p1".into(), name: "desk".into(), url: "http://100.2.0.1:3787".into() }];
        assert_eq!(answer(&j, &[], &no, &no, "desk", "http://100.2.0.1:3787"), 0);
        assert_eq!(answer(&j, &[], &no, &no, "x", "http://6.6.6.6:3787"), 3, "none of the machines this hub found");
        assert_eq!(answer(&j, &[], &no, &no, "laptop", "http://100.1.0.1:3787"), 3, "this machine is not another");
        assert_eq!(answer(&j, &[], &|n| n == "desk", &no, "desk", "http://100.2.0.1:3787"), 1);
        assert_eq!(answer(&j, &peers, &no, &no, "desk", "http://100.2.0.1:3787"), 2);
        assert_eq!(answer(&j, &peers, &no, &|id| id == "p1", "desk", "http://100.2.0.1:3787"), 0, "a link the owner said is not theirs does not count");
    }
    #[test] fn mate_sync_key_is_lowercase() { assert_eq!(sync_key("Desk"), "sync.desk"); }
    #[test] fn mate_of_two_hubs_the_lower_asks() {
        let j = json!({"self":{"name":"a","url":"http://100.1.0.1:3787","domain":"t.ts.net"},"peers":[{"name":"b","url":"http://100.2.0.1:3787"},{"name":"c","url":"http://100.0.0.9:3787"}]});
        let d = due(true, &j, &[], &no, &no);
        assert_eq!(d, vec![("b".to_string(), "http://100.2.0.1:3787".to_string())], "100.1.. asks 100.2.., not 100.0..");
    }
    #[test] fn mate_no_tailscale_pairs_only_named() {
        let j = json!({"self":{"name":"box","url":"http://100.9.0.1:4001","domain":"t.ts.net"},"peers":[{"name":"127.0.0.1","url":"http://127.0.0.1:4002","named":true},{"name":"desk","url":"http://100.9.0.2:3787"}]});
        assert_eq!(due(false, &j, &[], &no, &no), vec![("127.0.0.1".to_string(), "http://127.0.0.1:4002".to_string())]);
        assert_eq!(due(true, &j, &[], &no, &no).len(), 2);
    }
    #[test] fn mate_loopback_peer_compares_with_this_ones_loopback_address() {
        // 4001 < 4002 on the loopback comparison, though this hub's tailnet address sorts above
        let j = json!({"self":{"name":"box","url":"http://100.99.0.1:4001","domain":"t.ts.net"},"peers":[{"name":"127.0.0.1","url":"http://127.0.0.1:4002","named":true}]});
        assert_eq!(due(false, &j, &[], &no, &no).len(), 1);
        let j = json!({"self":{"name":"box","url":"http://100.0.0.1:4003","domain":"t.ts.net"},"peers":[{"name":"127.0.0.1","url":"http://127.0.0.1:4002","named":true}]});
        assert_eq!(due(false, &j, &[], &no, &no).len(), 0, "the other one is lower: it asks");
    }
    #[test] fn mate_off_and_linked_are_not_asked() {
        let j = demo("http://100.1.0.1:3787");
        let j = json!({"self":j["self"],"peers":[{"name":"desk","url":"http://100.2.0.1:3787"}]});
        assert!(due(true, &j, &[], &|n| n == "desk", &no).is_empty());
        let ps = vec![Peer { id: "p".into(), name: "desk".into(), url: "http://100.2.0.1:3787".into() }];
        assert!(due(true, &j, &ps, &no, &no).is_empty());
    }

    // the laws' own fixtures (client.bend Mate.demo / Mate.demo.lo)
    fn lo(port: &str) -> Value { json!({"self":{"name":"box","url":format!("http://100.9.0.1:{port}"),"domain":"t.ts.net"},"peers":[{"name":"127.0.0.1","url":"http://127.0.0.1:4002","version":"1","named":true}]}) }
    fn p1() -> Vec<Peer> { vec![Peer { id: "p1".into(), name: "desk".into(), url: "http://desk.t.ts.net:3787".into() }] }
    fn desk_row() -> Vec<(String, String)> { vec![("desk".into(), "http://100.2.0.1:3787".into())] }
    #[test] fn mate_lower_asks() { assert_eq!(due(true, &demo("http://100.1.0.1:3787"), &[], &no, &no), desk_row()); }
    #[test] fn mate_higher_waits() { assert!(due(true, &demo("http://100.3.0.1:3787"), &[], &no, &no).is_empty()); }
    #[test] fn mate_linked_never_asks() { assert!(due(true, &demo("http://100.1.0.1:3787"), &p1(), &no, &no).is_empty()); }
    #[test] fn mate_off_never_asks() { assert!(due(true, &demo("http://100.1.0.1:3787"), &[], &|n| n == "desk", &no).is_empty()); }
    #[test] fn mate_no_self_never_asks() { assert!(due(true, &demo(""), &[], &no, &no).is_empty()); }
    #[test] fn mate_unnamed_without_tailnet() { assert!(due(false, &demo("http://100.1.0.1:3787"), &[], &no, &no).is_empty()); }
    #[test] fn mate_answer_stranger() { assert_eq!(answer(&demo("http://100.3.0.1:3787"), &[], &no, &no, "desk", "http://127.0.0.1:9999"), 3); }
    #[test] fn mate_answer_invites() { assert_eq!(answer(&demo("http://100.3.0.1:3787"), &[], &no, &no, "desk", "http://desk.t.ts.net:3787"), 0); }
    #[test] fn mate_answer_off() { assert_eq!(answer(&demo("http://100.3.0.1:3787"), &[], &|n| n == "desk", &no, "desk", "http://desk.t.ts.net:3787"), 1); }
    #[test] fn mate_answer_linked() { assert_eq!(answer(&demo("http://100.3.0.1:3787"), &p1(), &no, &no, "desk", "http://desk.t.ts.net:3787"), 2); }
    #[test] fn mate_loopback_lower_asks() { assert_eq!(due(false, &lo("4001"), &[], &no, &no), vec![("127.0.0.1".to_string(), "http://127.0.0.1:4002".to_string())]); }
    #[test] fn mate_loopback_higher_waits() { assert!(due(false, &lo("4003"), &[], &no, &no).is_empty()); }
    fn farj() -> Value { json!({"self":{"name":"desk","url":"http://100.64.0.1:3787","domain":"tail1.ts.net"},"peers":[{"name":"Box","url":"http://100.64.0.2:3787"}]}) }
    #[test] fn far_machine_magicdns() { assert_eq!(machine(&farj(), "lab", "http://box.tail1.ts.net:3787"), Some("http://100.64.0.2:3787".into())); }
    #[test] fn far_machine_self() { assert_eq!(machine(&farj(), "desk", "http://100.64.0.1:3787"), Some(String::new())); }
    #[test] fn far_machine_other_port() { assert_eq!(machine(&farj(), "box", "http://box.tail1.ts.net:4000"), None); }
    #[test] fn far_machine_stranger() { assert_eq!(machine(&farj(), "box", "http://box.tail9.ts.net:3787"), None); }
}
