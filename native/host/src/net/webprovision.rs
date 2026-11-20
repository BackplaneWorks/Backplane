//! Interpret the status of a foreground node Serve mapping. The keeper holds
//! its backend socket until a successful status read confirms its removal.
use serde_json::{Map, Value};

#[derive(Debug, PartialEq, Eq)]
pub enum Mapping {
    Present,
    Absent,
    Unconfirmed,
}

/// A caller must distinguish a failed CLI invocation from a successful read of
/// the empty configuration. An unrecognized mapping to our backend also keeps
/// the reservation: another URL may still be forwarding to it.
pub fn mapping_status(raw: &str, authority: &str, proxy: &str) -> Mapping {
    let Ok(Value::Object(config)) = serde_json::from_str(raw) else {
        return Mapping::Unconfirmed;
    };
    fn inspect(config: &Map<String, Value>, authority: &str, proxy: &str) -> Option<bool> {
        if config.keys().any(|k| {
            !matches!(
                k.as_str(),
                "Web" | "TCP" | "Services" | "AllowFunnel" | "Foreground" | "Tun"
            )
        }) {
            return None;
        }
        let endpoint = proxy.strip_prefix("http://")?;
        if let Some(tcp) = config.get("TCP") {
            for handler in tcp.as_object()?.values() {
                if handler
                    .as_object()?
                    .get("TCPForward")
                    .is_some_and(|target| target.as_str() == Some(endpoint))
                {
                    return None;
                }
            }
        }
        let mut present = false;
        if let Some(web) = config.get("Web") {
            for (host, entry) in web.as_object()? {
                for (path, handler) in entry.get("Handlers")?.as_object()? {
                    let handler = handler.as_object()?;
                    if let Some(target) = handler.get("Proxy") {
                        if target.as_str()? == proxy {
                            if host != authority || path != "/" {
                                return None;
                            }
                            present = true;
                        }
                    }
                }
            }
        }
        for key in ["Foreground", "Services"] {
            if let Some(scopes) = config.get(key) {
                for child in scopes.as_object()?.values() {
                    present |= inspect(child.as_object()?, authority, proxy)?;
                }
            }
        }
        Some(present)
    }
    match inspect(&config, authority, proxy) {
        Some(true) => Mapping::Present,
        Some(false) => Mapping::Absent,
        None => Mapping::Unconfirmed,
    }
}

/// Every directory used to resolve the persistent Serve target must be
/// protected from replacement by other accounts, including after hub exit.
pub fn protected_directory(path: &std::path::Path) -> std::io::Result<()> {
    use std::{io, os::unix::fs::MetadataExt};
    let uid = crate::plat::effective_uid();
    if !path.is_absolute() || path.canonicalize()? != path {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "canonical absolute directory required",
        ));
    }
    for ancestor in path.ancestors() {
        let meta = std::fs::symlink_metadata(ancestor)?;
        if !meta.is_dir()
            || (meta.uid() != 0 && meta.uid() != uid)
            || meta.mode() & 0o022 != 0
            || (ancestor == path && (meta.uid() != uid || meta.mode() & 0o777 != 0o700))
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "private socket path required",
            ));
        }
    }
    Ok(())
}

/// Read the kernel identity of the daemon at the system LocalAPI socket.
/// No HTTP request, login header, or hub credential is sent to this socket.
pub async fn system_proxy_identity() -> std::io::Result<super::webtransport::ProxyIdentity> {
    use std::{io, time::Duration};
    let stream = tokio::time::timeout(
        Duration::from_secs(2),
        tokio::net::UnixStream::connect("/run/tailscale/tailscaled.sock"),
    )
    .await??;
    let peer = stream.peer_cred()?;
    let pid = peer
        .pid()
        .filter(|pid| *pid > 0 && peer.uid() == 0)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "system Tailscale daemon required",
            )
        })?;
    Ok(super::webtransport::ProxyIdentity {
        uid: peer.uid(),
        pid,
    })
}

pub async fn bind_private_backend(
    directory: &std::path::Path,
    authority: String,
) -> std::io::Result<super::webtransport::BrowserListener> {
    protected_directory(directory)?;
    let proxy = system_proxy_identity().await?;
    super::webtransport::BrowserListener::bind(directory, authority, proxy)
}

/// Pick a spare node HTTPS port from the status snapshot. Recheck before
/// applying a configuration change; this selection alone grants no ownership.
pub fn spare_node_port(config: &Value) -> Option<u16> {
    fn occupied(scope: &Value, ports: &mut std::collections::BTreeSet<u16>) -> Option<()> {
        let scope = scope.as_object()?;
        for (key, value) in scope {
            match key.as_str() {
                "TCP" => {
                    for port in value.as_object()?.keys() {
                        ports.insert(port.parse().ok()?);
                    }
                }
                "Web" | "AllowFunnel" => {
                    for authority in value.as_object()?.keys() {
                        ports.insert(authority.rsplit_once(':')?.1.parse().ok()?);
                    }
                }
                "Foreground" | "Services" => {
                    for child in value.as_object()?.values() {
                        occupied(child, ports)?;
                    }
                }
                "Tun" => {
                    value.as_bool()?;
                }
                _ => return None,
            }
        }
        Some(())
    }
    let mut ports = std::collections::BTreeSet::new();
    occupied(config, &mut ports)?;
    (8443..=u16::MAX).find(|p| !ports.contains(p))
}

/// Arguments for the system-installed Tailscale executable. Pass directly
/// to the process API so a path containing spaces stays a single argument.
pub fn private_serve_args(
    service: Option<&str>,
    port: u16,
    socket: &std::path::Path,
) -> Option<Vec<String>> {
    if port == 0 || (service.is_none() && port == 443) || !socket.is_absolute() {
        return None;
    }
    let path = socket.to_str()?;
    if path.chars().any(char::is_control) {
        return None;
    }
    let mut args = vec![String::from("serve"), String::from("--bg")];
    if let Some(name) = service {
        if name.is_empty()
            || name.len() > 63
            || name.starts_with('-')
            || name.ends_with('-')
            || !name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return None;
        }
        args.push(format!("--service=svc:{name}"));
    }
    args.push(format!("--https={port}"));
    args.push(format!("unix:{path}"));
    Some(args)
}

/// Inspect the exact node or Service scope before showing its URL. The
/// caller separately verifies reachability and the identity of the proxy.
pub fn private_route_matches(
    config: &Value,
    service: Option<&str>,
    authority: &str,
    port: u16,
    socket: &std::path::Path,
) -> bool {
    let Some(path) = socket.to_str().filter(|_| socket.is_absolute()) else {
        return false;
    };
    let scope = match service {
        Some(name) => &config["Services"][name],
        None => config,
    };
    let host_port = format!("{authority}:{port}");
    let handlers = &scope["Web"][&host_port]["Handlers"];
    let funnel_off = |scope: &Value| match scope.get("AllowFunnel") {
        None => true,
        Some(map) => map
            .as_object()
            .is_some_and(|map| map.get(&host_port).is_none_or(|flag| flag == false)),
    };
    funnel_off(config)
        && funnel_off(scope)
        && scope["TCP"][port.to_string()] == serde_json::json!({"HTTPS":true})
        && *handlers == serde_json::json!({"/":{"Proxy":format!("unix:{path}")}})
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn private_socket_directory_rejects_a_writable_parent() {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let parent =
            std::env::temp_dir().join(format!("bp-web-unsafe-parent-{}", std::process::id()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&parent)
            .unwrap();
        let child = parent.join("private");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&child)
            .unwrap();
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o770)).unwrap();
        assert_eq!(
            protected_directory(&child.canonicalize().unwrap())
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
        std::fs::remove_dir(child).unwrap();
        std::fs::remove_dir(parent).unwrap();
    }

    #[test]
    fn node_port_selection_preserves_existing_and_foreground_routes() {
        let config = json!({"TCP":{"8443":{"HTTPS":true}},
            "Foreground":{"test":{"Web":{"hub.example.ts.net:8444":{"Handlers":{}}}}}});
        assert_eq!(spare_node_port(&config), Some(8445));
        assert_eq!(spare_node_port(&json!({})), Some(8443));
        assert_eq!(spare_node_port(&json!({"TCP":null})), None);
        assert_eq!(spare_node_port(&json!({"FutureConfig":{}})), None);
    }

    #[test]
    fn setup_targets_an_exclusive_unix_socket_in_the_selected_address_mode() {
        let path = std::path::Path::new("/run/private dir/browser.sock");
        assert_eq!(
            private_serve_args(None, 8443, path).unwrap(),
            [
                "serve",
                "--bg",
                "--https=8443",
                "unix:/run/private dir/browser.sock"
            ]
        );
        assert_eq!(
            private_serve_args(Some("bp"), 443, path).unwrap(),
            [
                "serve",
                "--bg",
                "--service=svc:bp",
                "--https=443",
                "unix:/run/private dir/browser.sock"
            ]
        );
        assert!(private_serve_args(None, 443, path).is_none());
        assert!(private_serve_args(Some("--other"), 443, path).is_none());
        assert!(private_serve_args(None, 8443, std::path::Path::new("relative")).is_none());
    }

    #[test]
    fn private_route_requires_exclusive_https_and_the_expected_scope() {
        let name = "bp.example.ts.net";
        let socket = std::path::Path::new("/run/example/browser.sock");
        let route = json!({"TCP":{"8443":{"HTTPS":true}},
            "Web":{"bp.example.ts.net:8443":{"Handlers":{"/":{"Proxy":"unix:/run/example/browser.sock"}}}}});
        assert!(private_route_matches(&route, None, name, 8443, socket));
        let mut config = json!({"Services":{"svc:bp":route}});
        assert!(private_route_matches(
            &config,
            Some("svc:bp"),
            name,
            8443,
            socket
        ));
        assert!(!private_route_matches(&config, None, name, 8443, socket));
        config["AllowFunnel"] = json!({"bp.example.ts.net:8443":true});
        assert!(!private_route_matches(
            &config,
            Some("svc:bp"),
            name,
            8443,
            socket
        ));
        let mut shared = route;
        shared["Web"]["bp.example.ts.net:8443"]["Handlers"]["/other"] =
            json!({"Text":"another app"});
        assert!(!private_route_matches(&shared, None, name, 8443, socket));
    }

    #[test]
    fn keeper_requires_confirmed_removal_of_its_own_foreground_mapping() {
        let host = "hub.example.ts.net:8443";
        let backend = "http://127.0.0.1:3876";
        let config = |host: &str, proxy: &str| {
            json!({"Foreground": {"session": {
                "Web": {(host): {"Handlers": {"/": {"Proxy": proxy}}}}
            }}})
        };
        let status = |v: Value| mapping_status(&v.to_string(), host, backend);
        assert_eq!(status(config(host, backend)), Mapping::Present);
        assert_eq!(
            status(config(host, "http://127.0.0.1:3877")),
            Mapping::Absent
        );
        assert_eq!(
            status(config("another.example.ts.net:8443", backend)),
            Mapping::Unconfirmed
        );
        assert_eq!(status(json!({})), Mapping::Absent);
        assert_eq!(status(json!({"Unexpected":{}})), Mapping::Unconfirmed);
        assert_eq!(
            status(json!({"TCP":{"9443":{"TCPForward":"127.0.0.1:3876"}}})),
            Mapping::Unconfirmed
        );
        for invalid in ["", "null", "[]", "{", r#"{"Foreground":[]}"#] {
            assert_eq!(mapping_status(invalid, host, backend), Mapping::Unconfirmed);
        }
    }
}
