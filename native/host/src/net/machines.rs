//! Which machine a peer is: client's `Far.hit/machine` (pure). Nothing here pairs machines: a link is made only by a code the
//! person approves (`pairflow.rs`). The owner's machines are the switcher's JSON: `{"self":{name,url,domain},"peers":[{name,url,named?}]}`.
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

/// "sync.<name>" = "off": nothing goes to the machine or comes from it.
pub fn sync_key(name: &str) -> String { format!("sync.{}", name.to_lowercase()) }
/// A host on this computer only: localhost, 127.x, [::1].
pub fn loopback(url: &str) -> bool { let h = host(url).to_lowercase(); h == "localhost" || h.starts_with("127.") || hostport(url).starts_with("[::1]") }

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn demo(me: &str) -> Value { json!({"self":{"name":"laptop","url":me,"domain":"t.ts.net"},"peers":[{"name":"desk","url":"http://100.2.0.1:3787","version":"1"}]}) }

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
    #[test] fn mate_sync_key_is_lowercase() { assert_eq!(sync_key("Desk"), "sync.desk"); }
    #[test] fn loopback_hosts() { assert!(loopback("http://127.0.0.1:1") && loopback("http://localhost") && loopback("http://[::1]:3") && loopback("http://127.9.9.9:1")); assert!(!loopback("http://100.64.0.1:1") && !loopback("http://box.ts.net:1")); }
    fn farj() -> Value { json!({"self":{"name":"desk","url":"http://100.64.0.1:3787","domain":"tail1.ts.net"},"peers":[{"name":"Box","url":"http://100.64.0.2:3787"}]}) }
    #[test] fn far_machine_magicdns() { assert_eq!(machine(&farj(), "lab", "http://box.tail1.ts.net:3787"), Some("http://100.64.0.2:3787".into())); }
    #[test] fn far_machine_self() { assert_eq!(machine(&farj(), "desk", "http://100.64.0.1:3787"), Some(String::new())); }
    #[test] fn far_machine_other_port() { assert_eq!(machine(&farj(), "box", "http://box.tail1.ts.net:4000"), None); }
    #[test] fn far_machine_stranger() { assert_eq!(machine(&farj(), "box", "http://box.tail9.ts.net:3787"), None); }
}
