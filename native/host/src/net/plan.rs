//! Where an invitation may point (Settings → Devices → Add a device, `POST /auth/invitations`): the tailnet, each local
//! network address, this computer, in that order, each saying whether this hub answers there now (0.14's
//! `effects/pairing.js` `pairingPlan`). Pure: the interfaces, the listener's routes and the local network addresses the
//! person let the hub answer on are given.
//!
//! The tailnet's addresses are only listener-confirmed origins (the tailnet listener's, Serve's), the tailnet address a
//! listener on every interface covers, or the one the request came through. A local network address answers when the
//! main listener covers it (`--host 0.0.0.0` or that address) or the person let the hub answer there too (`net::lan`).
//! The default (the fields older clients read: `link`, `routes`, `localOnly`) is the first remote address that answers.
//! Each address's routes: itself, then the other remote ones that answer; loopback is never a hint for another device
//! (it would reach that device's own hub).

/// One address an invitation can be shown with: `kind` is "tailnet", "lan" or "local".
#[derive(Clone, Debug, PartialEq)]
pub struct Choice { pub kind: &'static str, pub url: String, pub listening: bool, pub routes: Vec<String> }

/// 100.64.0.0/10, the addresses Tailscale hands out.
pub fn tailnet_ip(ip: &str) -> bool {
    let p: Vec<&str> = ip.split('.').collect();
    p.len() == 4 && p.iter().all(|x| x.parse::<u8>().is_ok()) && p[0] == "100" && p[1].parse::<u8>().map_or(false, |b| (64..=127).contains(&b))
}
/// A container's, a VM's or a VPN's interface: no other device on the network reaches its address.
pub fn virtual_iface(name: &str) -> bool {
    ["docker", "br-", "veth", "virbr", "vboxnet", "vmnet", "tailscale", "podman", "cni", "flannel", "lxcbr", "lxdbr", "zt", "utun", "wg"].iter().any(|p| name.starts_with(p))
}
fn v4(ip: &str) -> bool { ip.parse::<std::net::Ipv4Addr>().is_ok() }
/// This machine's local network addresses from (interface name, IPv4) pairs: not loopback or link-local, not the tailnet's,
/// not a container's or a VM's bridge. Each once, in the order given.
pub fn lan(ifaces: &[(String, String)]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for (name, ip) in ifaces {
        if virtual_iface(name) || !v4(ip) || ip.starts_with("127.") || ip.starts_with("169.254.") || tailnet_ip(ip) || out.contains(ip) { continue }
        out.push(ip.clone());
    }
    out
}
fn host_of(url: &str) -> &str {
    let r = url.strip_prefix("http://").or_else(|| url.strip_prefix("https://")).unwrap_or("");
    let a = r.split(|c| c == '/' || c == '?' || c == '#').next().unwrap_or("");
    if let Some(v6) = a.strip_prefix('[') { return v6.split(']').next().unwrap_or("") }
    a.rsplit_once(':').map_or(a, |(h, _)| h)
}
fn loopback(url: &str) -> bool {
    let r = url.strip_prefix("http://").or_else(|| url.strip_prefix("https://")).unwrap_or("");
    r.starts_with("localhost") || r.starts_with("127.") || r.starts_with("[::1]")
}
fn tailnet_url(url: &str) -> bool { let h = host_of(url); tailnet_ip(h) || h.to_ascii_lowercase().ends_with(".ts.net") }
fn push(v: &mut Vec<String>, x: String) { if !x.is_empty() && !v.contains(&x) { v.push(x) } }

/// The plan: (the default routes, every address). `routes`: the listener-confirmed origins (tailnet listener, Serve);
/// `host`/`port`: the main listener; `authority`: the Host the request came with ("" for one made inside the hub);
/// `ifaces`: (interface, IPv4) pairs; `bound`: the local network addresses the hub was let answer on and listens on now.
pub fn plan(routes: &[String], host: &str, port: u16, authority: &str, https: bool, ifaces: &[(String, String)], bound: &[String]) -> (Vec<String>, Vec<Choice>) {
    let all = host == "0.0.0.0";
    let at = |ip: &str| format!("http://{ip}:{port}");
    let loop_host = all || ["127.0.0.1", "localhost", "::1"].contains(&host);
    // this exact origin already authenticated the issuing client
    let asked = if !authority.is_empty() && authority.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'[' | b']' | b':')) {
        format!("{}{authority}", if https { "https://" } else { "http://" })
    } else { String::new() };
    let lans = lan(ifaces);
    let mut tails: Vec<String> = vec![];
    for r in routes { push(&mut tails, r.clone()) }
    if all { for (_, ip) in ifaces { if tailnet_ip(ip) { push(&mut tails, at(ip)) } } }
    if tailnet_ip(host) { push(&mut tails, at(host)) }
    if !asked.is_empty() && tailnet_url(&asked) { push(&mut tails, asked.clone()) }
    let mut choices: Vec<Choice> = vec![];
    if let Some(t) = tails.first() { choices.push(Choice { kind: "tailnet", url: t.clone(), listening: true, routes: vec![] }) }
    for ip in &lans {
        if !asked.is_empty() && host_of(&asked) == ip { choices.push(Choice { kind: "lan", url: asked.clone(), listening: true, routes: vec![] }) }
        else { choices.push(Choice { kind: "lan", url: at(ip), listening: all || host == ip || bound.contains(ip), routes: vec![] }) }
    }
    let local_asked = !asked.is_empty() && loopback(&asked);
    choices.push(Choice { kind: "local", url: if local_asked { asked.clone() } else { at("127.0.0.1") }, listening: loop_host || local_asked, routes: vec![] });
    // other origins this hub answers on, hints after the chosen one: more tailnet ones, the request's own and the main
    // listener's when they are none of the above
    let mut hints: Vec<String> = tails.iter().skip(1).cloned().collect();
    if !asked.is_empty() && !loopback(&asked) && !tailnet_url(&asked) && !lans.iter().any(|l| l == host_of(&asked)) { push(&mut hints, asked.clone()) }
    if !loop_host && !tailnet_ip(host) && !lans.iter().any(|l| l == host) { push(&mut hints, at(host)) }
    let remote: Vec<String> = choices.iter().filter(|c| c.kind != "local" && c.listening).map(|c| c.url.clone()).collect();
    for c in choices.iter_mut() {
        let mut rs = vec![c.url.clone()];
        for x in remote.iter().chain(hints.iter()) { push(&mut rs, x.clone()) }
        c.routes = rs;
    }
    let default = match choices.iter().find(|c| c.kind != "local" && c.listening) {
        Some(c) => c.routes.clone(),
        None if !hints.is_empty() => hints,
        None => choices.last().map(|c| c.routes.clone()).unwrap_or_default(),
    };
    (default, choices)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ifs(x: &[(&str, &str)]) -> Vec<(String, String)> { x.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect() }
    fn kinds(c: &[Choice]) -> Vec<(&str, &str, bool)> { c.iter().map(|c| (c.kind, c.url.as_str(), c.listening)).collect() }
    const HOME: &[(&str, &str)] = &[("lo", "127.0.0.1"), ("enp9s0", "192.168.77.2"), ("docker0", "172.17.0.1"), ("tailscale0", "100.101.102.103"), ("wlan0", "169.254.3.4"), ("wlp2s0", "10.0.0.5")];

    #[test] fn tailnet_addresses_are_100_64_slash_10() {
        for ip in ["100.64.0.1", "100.101.102.103", "100.127.255.255"] { assert!(tailnet_ip(ip), "{ip}") }
        for ip in ["100.63.0.1", "100.128.0.1", "10.0.0.1", "100.64.0", "nope"] { assert!(!tailnet_ip(ip), "{ip}") }
    }
    #[test] fn local_network_skips_loopback_link_local_tailnet_and_bridges() {
        assert_eq!(lan(&ifs(HOME)), ["192.168.77.2", "10.0.0.5"]);
        assert_eq!(lan(&ifs(&[("br-1a2b", "172.18.0.1"), ("veth0", "10.1.1.1"), ("virbr0", "192.168.122.1"), ("eth0", "192.168.1.9"), ("eth0", "192.168.1.9")])), ["192.168.1.9"]);
    }
    /// The default hub (loopback main listener, tailnet listener up): Tailscale answers, the local network does not until the person lets it.
    #[test] fn a_loopback_hub_with_its_tailnet_listener() {
        let (def, c) = plan(&["http://100.101.102.103:3871".into()], "127.0.0.1", 3871, "", false, &ifs(HOME), &[]);
        assert_eq!(kinds(&c), [("tailnet", "http://100.101.102.103:3871", true), ("lan", "http://192.168.77.2:3871", false), ("lan", "http://10.0.0.5:3871", false), ("local", "http://127.0.0.1:3871", true)]);
        assert_eq!(def, ["http://100.101.102.103:3871"]);
        assert_eq!(c[1].routes, ["http://192.168.77.2:3871", "http://100.101.102.103:3871"], "its own address first, then the remote ones that answer; never loopback");
        assert_eq!(c[3].routes, ["http://127.0.0.1:3871", "http://100.101.102.103:3871"]);
    }
    #[test] fn a_local_network_address_the_person_let_it_answer_on() {
        let (def, c) = plan(&["http://100.101.102.103:3871".into()], "127.0.0.1", 3871, "127.0.0.1:3871", false, &ifs(HOME), &["192.168.77.2".into()]);
        assert_eq!(kinds(&c)[1], ("lan", "http://192.168.77.2:3871", true));
        assert_eq!(def, ["http://100.101.102.103:3871", "http://192.168.77.2:3871"]);
        assert_eq!(c[1].routes, ["http://192.168.77.2:3871", "http://100.101.102.103:3871"]);
    }
    #[test] fn no_tailnet_and_only_loopback_falls_back_to_this_computer() {
        let (def, c) = plan(&[], "127.0.0.1", 3797, "127.0.0.1:3797", false, &ifs(&[("lo", "127.0.0.1"), ("eth0", "192.168.1.9")]), &[]);
        assert_eq!(kinds(&c), [("lan", "http://192.168.1.9:3797", false), ("local", "http://127.0.0.1:3797", true)]);
        assert_eq!(def, ["http://127.0.0.1:3797"]);
    }
    #[test] fn a_hub_on_every_interface_answers_everywhere() {
        let (def, c) = plan(&[], "0.0.0.0", 3787, "", false, &ifs(HOME), &[]);
        assert_eq!(kinds(&c), [("tailnet", "http://100.101.102.103:3787", true), ("lan", "http://192.168.77.2:3787", true), ("lan", "http://10.0.0.5:3787", true), ("local", "http://127.0.0.1:3787", true)]);
        assert_eq!(def, ["http://100.101.102.103:3787", "http://192.168.77.2:3787", "http://10.0.0.5:3787"]);
    }
    #[test] fn the_request_origin_counts() {
        // asked through the MagicDNS name over Serve: that is a tailnet address; through a LAN address: that one answers
        let (_, c) = plan(&[], "127.0.0.1", 3787, "box.tail1.ts.net", true, &ifs(HOME), &[]);
        assert_eq!(kinds(&c)[0], ("tailnet", "https://box.tail1.ts.net", true));
        let (def, c) = plan(&[], "192.168.77.2", 3787, "192.168.77.2:3787", false, &ifs(HOME), &[]);
        assert_eq!(kinds(&c)[0], ("lan", "http://192.168.77.2:3787", true));
        assert_eq!(def, ["http://192.168.77.2:3787"]);
        // a header that is not an authority is ignored
        let (_, c) = plan(&[], "127.0.0.1", 3787, "evil/x y", false, &ifs(&[("lo", "127.0.0.1")]), &[]);
        assert_eq!(kinds(&c), [("local", "http://127.0.0.1:3787", true)]);
    }
    #[test] fn a_main_listener_on_another_address_is_a_hint() {
        let (def, c) = plan(&[], "203.0.113.5", 3787, "", false, &ifs(&[("lo", "127.0.0.1")]), &[]);
        assert_eq!(kinds(&c), [("local", "http://127.0.0.1:3787", false)]);
        assert_eq!(def, ["http://203.0.113.5:3787"]);
    }
}
