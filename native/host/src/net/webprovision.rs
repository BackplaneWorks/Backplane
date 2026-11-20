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

/// Called with the same authority that was validated when binding the socket.
/// The public address endpoint carries no credential. Leave proxy settings,
/// curl config files, redirects and user credentials out of the verification.
pub async fn https_address_answers(home: &std::path::Path, authority: &str) -> bool {
    use std::process::Stdio;
    use tokio::io::AsyncWriteExt;
    if crate::copy::on() {
        return false;
    }
    let url = format!("https://{authority}/auth/web/setup");
    let probe = super::webprobe::Probe::start(home, authority);
    let Ok(mut child) = crate::proc::tokio_cmd(crate::proc::Kind::Net, "/usr/bin/curl")
        .args([
            "-q",
            "--silent",
            "--fail",
            "--proto",
            "=https",
            "--noproxy",
            "*",
            "--max-time",
            "5",
            "--max-filesize",
            "1024",
            "--header",
            "@-",
            "--write-out",
            "%{http_code}",
            "--url",
            &url,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
    else {
        return false;
    };
    let Some(mut input) = child.stdin.take() else {
        return false;
    };
    if input.write_all(probe.header().as_bytes()).await.is_err() {
        return false;
    }
    drop(input);
    child
        .wait_with_output()
        .await
        .is_ok_and(|out| out.status.success() && out.stdout == b"204" && probe.consumed())
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

/// Prefer a configured Service; fall back to this node's own HTTPS mapping.
fn configured_authority(
    config: &Value,
    service: &str,
    node: &str,
    socket: &std::path::Path,
) -> Option<String> {
    let suffix = node.split_once('.')?.1;
    let service_host = format!("{service}.{suffix}");
    let scope_name = format!("svc:{service}");
    for (scope, host) in [
        (Some(scope_name.as_str()), service_host.as_str()),
        (None, node),
    ] {
        let settings = scope.map_or(config, |key| &config["Services"][key]);
        let Some(entries) = settings["Web"].as_object() else {
            continue;
        };
        for key in entries.keys() {
            let Some((name, port)) = key.rsplit_once(':') else {
                continue;
            };
            let Ok(port) = port.parse::<u16>() else {
                continue;
            };
            if name == host && private_route_matches(config, scope, host, port, socket) {
                return Some(if port == 443 {
                    host.to_owned()
                } else {
                    key.to_owned()
                });
            }
        }
    }
    None
}

// A setup request exists only while this hub's tailnet monitor is running.
type SetupRequests = std::collections::HashMap<std::path::PathBuf, tokio::sync::mpsc::Sender<String>>;
static SETUP_REQUESTS: std::sync::OnceLock<std::sync::Mutex<SetupRequests>> = std::sync::OnceLock::new();

pub fn request_setup(home: &std::path::Path, plan: &str) -> Result<(), &'static str> {
    if plan.len() > 4096 { return Err("invalid setup plan"); }
    let requests = SETUP_REQUESTS.get().ok_or("web setup unavailable")?.lock().unwrap();
    requests.get(home).ok_or("web setup unavailable")?.try_send(plan.to_owned()).map_err(|_| "setup already pending")
}

pub async fn monitor(hub: std::sync::Arc<crate::hub::Hub>, ts: &dyn super::Tailscale) {
    use std::os::unix::fs::DirBuilderExt;
    use std::time::Duration;
    if crate::copy::on() {
        return;
    }
    let (sender, mut requests) = tokio::sync::mpsc::channel(1);
    SETUP_REQUESTS.get_or_init(Default::default).lock().unwrap().insert(hub.cfg.home.clone(), sender);
    let directory = hub.cfg.home.join("web");
    let socket = directory.join("browser.sock");
    loop {
        let requested = requests.try_recv().ok();
        let mut status_row = serde_json::json!({"canEnable":true});
        if hub.setting("web.enabled").await.as_deref() == Some("1") {
            let status: Value = serde_json::from_str(&ts.status().await).unwrap_or(Value::Null);
            let config: Value =
                serde_json::from_str(&ts.serve_status().await).unwrap_or(Value::Null);
            let node = status["Self"]["DNSName"]
                .as_str()
                .unwrap_or("")
                .trim_end_matches('.');
            let service = hub
                .setting("web.service")
                .await
                .unwrap_or_else(|| "bp".into());
            status_row["reason"] = Value::from("Waiting for a verified private Tailscale HTTPS mapping.");
            if !node.is_empty() && configured_authority(&config, &service, node, &socket).is_none() {
                if let Some(args) = spare_node_port(&config).and_then(|port| private_serve_args(None, port, &socket)) {
                    let quoted = args.iter().map(|arg| format!("'{}'", arg.replace('\'', "'\\''"))).collect::<Vec<_>>();
                    let plan = format!("sudo /usr/bin/tailscale {}", quoted.join(" "));
                    if requested.as_ref() == Some(&plan) {
                        let _ = std::fs::DirBuilder::new().mode(0o700).create(&directory);
                        if protected_directory(&directory).is_ok() && system_proxy_identity().await.is_ok() {
                            let _ = crate::proc::tokio_cmd(crate::proc::Kind::Net, "/usr/bin/pkexec")
                                .arg("--disable-internal-agent").arg("/usr/bin/tailscale").args(&args)
                                .stdin(std::process::Stdio::null()).status().await;
                        }
                    }
                    status_row["command"] = Value::from(plan);
                }
            }
            if let Some(authority) = configured_authority(&config, &service, node, &socket) {
                let _ = std::fs::DirBuilder::new().mode(0o700).create(&directory);
                if let Ok(listener) = bind_private_backend(&directory, authority.clone()).await {
                    let mut task = tokio::spawn(listener.run(hub.clone()));
                    if https_address_answers(&hub.cfg.home, &authority).await {
                        let active = serde_json::json!({"available":true, "address":format!("https://{authority}")});
                        hub.set_info("web.status", &active.to_string()).await;
                        loop {
                            tokio::select! {
                                _ = &mut task => break,
                                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                                    if hub.setting("web.enabled").await.as_deref() != Some("1") {break;}
                                }
                            }
                        }
                    }
                    task.abort();
                    let _ = std::fs::remove_file(&socket);
                    if hub.setting("web.enabled").await.as_deref() != Some("1") {
                        let (host, port) = authority.rsplit_once(':').map_or((authority.as_str(), 443),
                            |(host, port)| (host, port.parse().unwrap_or(0)));
                        let scope = (host != node).then_some(service.as_str());
                        if !remove_private_mapping(scope, host, port, &socket).await {
                            status_row["reason"] = Value::from("Browser access is off. Tailscale mapping removal could not be confirmed.");
                        }
                    }
                }
            }
        }
        hub.set_info("web.status", &status_row.to_string()).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// An administrator may have reassigned the endpoint since setup. Only
/// produce a removal command when the fresh status still names our backend.
pub fn private_remove_args(
    config: &Value,
    service: Option<&str>,
    hostname: &str,
    port: u16,
    socket: &std::path::Path,
) -> Option<Vec<String>> {
    let mut args = private_serve_args(service, port, socket)?;
    let scope = service.map(|name| format!("svc:{name}"));
    if !private_route_matches(config, scope.as_deref(), hostname, port, socket) {
        return None;
    }
    args.retain(|arg| arg != "--bg");
    *args.last_mut()? = String::from("off");
    Some(args)
}

/// Prepare the minimal Serve config update for a conditional LocalAPI write.
/// The caller must submit the original GET's ETag in If-Match.
fn removal_update(mut config: Value, etag: &str, service: Option<&str>, hostname: &str,
                  port: u16, socket: &std::path::Path) -> Option<Value> {
    if etag.trim().is_empty() || etag == "*" { return None; }
    private_remove_args(&config, service, hostname, port, socket)?;
    let scope = match service {
        Some(name) => config.get_mut("Services")?.get_mut(format!("svc:{name}"))?,
        None => &mut config,
    };
    let key = format!("{hostname}:{port}");
    let web = scope.get_mut("Web")?.as_object_mut()?;
    // A port can have multiple host handlers. Preserve its HTTPS listener
    // when another host uses it.
    web.remove(&key)?;
    let suffix = format!(":{port}");
    if !web.keys().any(|authority| authority.ends_with(&suffix)) {
        scope.get_mut("TCP")?.as_object_mut()?.remove(&port.to_string())?;
    }
    Some(config)
}

async fn remove_private_mapping(service: Option<&str>, host: &str, port: u16, socket: &std::path::Path) -> bool {
    use std::process::Stdio;
    use tokio::io::AsyncWriteExt;
    if system_proxy_identity().await.is_err() { return false; }
    let local_api = || {
        let mut cmd = crate::proc::tokio_cmd(crate::proc::Kind::Net, "/usr/bin/curl");
        cmd.args(["-q", "--silent", "--fail", "--max-time", "5", "--max-filesize", "1048576",
            "--noproxy", "*", "--unix-socket", "/run/tailscale/tailscaled.sock",
            "http://local-tailscaled.sock/localapi/v0/serve-config"]);
        cmd
    };
    let Ok(reply) = local_api().args(["--write-out", "\n%header{etag}"]).output().await else { return false; };
    if !reply.status.success() { return false; }
    let Ok(text) = std::str::from_utf8(&reply.stdout) else { return false; };
    let Some((body, etag)) = text.rsplit_once('\n') else { return false; };
    if etag.bytes().any(|b| b.is_ascii_control()) { return false; }
    let Ok(config) = serde_json::from_str(body) else { return false; };
    let Some(updated) = removal_update(config, etag, service, host, port, socket) else { return false; };
    let Ok(mut child) = local_api().args(["--data-binary", "@-", "--header", "Content-Type: application/json",
        "--header", &format!("If-Match: {etag}")]).stdin(Stdio::piped()).stdout(Stdio::null()).kill_on_drop(true).spawn() else { return false; };
    let Some(mut input) = child.stdin.take() else { return false; };
    if input.write_all(updated.to_string().as_bytes()).await.is_err() { return false; }
    drop(input);
    child.wait().await.is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn conditional_removal_preserves_another_authority_on_the_port() {
        let socket = std::path::Path::new("/run/private/browser.sock");
        let config = json!({"TCP":{"8443":{"HTTPS":true}}, "Web":{
            "node.example.ts.net:8443":{"Handlers":{"/":{"Proxy":"unix:/run/private/browser.sock"}}},
            "another.example.ts.net:8443":{"Handlers":{"/":{"Text":"other"}}}
        }});
        for etag in ["", "*"] {
            assert!(removal_update(config.clone(), etag, None, "node.example.ts.net", 8443, socket).is_none());
        }
        let update = removal_update(config.clone(), "revision", None, "node.example.ts.net", 8443, socket).unwrap();
        assert_eq!(update["TCP"], config["TCP"]);
        assert_eq!(update["Web"].as_object().unwrap().len(), 1);
        assert_eq!(update["Web"]["another.example.ts.net:8443"], config["Web"]["another.example.ts.net:8443"]);
    }

    #[test]
    fn disable_preserves_a_reassigned_service_endpoint() {
        let host = "bp.example.ts.net";
        let socket = std::path::Path::new("/run/private/browser.sock");
        let mut state = json!({"Services":{"svc:bp":{"TCP":{"443":{"HTTPS":true}},
            "Web":{"bp.example.ts.net:443":{"Handlers":{"/":{"Proxy":"unix:/run/private/browser.sock"}}}}}}});
        assert_eq!(
            private_remove_args(&state, Some("bp"), host, 443, socket).unwrap(),
            ["serve", "--service=svc:bp", "--https=443", "off"]
        );
        state["Services"]["svc:bp"]["Web"]["bp.example.ts.net:443"]["Handlers"]["/"]["Proxy"] =
            json!("unix:/run/another/socket");
        assert!(private_remove_args(&state, Some("bp"), host, 443, socket).is_none());
    }

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
