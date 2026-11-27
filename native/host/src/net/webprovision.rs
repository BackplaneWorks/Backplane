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
        let endpoint = proxy.strip_prefix("http://").unwrap_or(proxy);
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
#[cfg(unix)]
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

/// On Windows, directory permissions are governed by NTFS user profile ACLs;
/// verify that the runtime directory is an absolute, non-symlink/non-junction directory.
#[cfg(windows)]
pub fn protected_directory(path: &std::path::Path) -> std::io::Result<()> {
    use std::io;
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "canonical absolute directory required",
        ));
    }
    let meta = std::fs::symlink_metadata(path)?;
    if !meta.is_dir() || !crate::plat::fs::no_link_on(path)? {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private socket path required",
        ));
    }
    Ok(())
}

fn ensure_protected_directory(directory: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt};
        let _ = std::fs::DirBuilder::new().mode(0o700).create(directory);
        let canonical = crate::plat::fs::canonicalize(directory)?;
        let meta = std::fs::symlink_metadata(&canonical)?;
        if !meta.is_dir() || meta.uid() != crate::plat::effective_uid() || meta.mode() & 0o777 != 0o700 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "private socket path required",
            ));
        }
        Ok(canonical)
    }
    #[cfg(not(unix))]
    {
        let _ = std::fs::create_dir(directory);
        let canonical = crate::plat::fs::canonicalize(directory)?;
        protected_directory(&canonical)?;
        Ok(canonical)
    }
}

/// Read the kernel identity of the daemon at the system LocalAPI socket.
/// No HTTP request, login header, or hub credential is sent to this socket.
#[cfg(unix)]
pub async fn system_proxy_identity() -> std::io::Result<super::webtransport::ProxyIdentity> {
    use std::{io, time::Duration};
    if crate::copy::on() {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "copy run"));
    }
    #[cfg(target_os = "linux")]
    let sock_path = "/run/tailscale/tailscaled.sock";
    #[cfg(target_os = "macos")]
    let sock_path = "/var/run/tailscaled.socket";
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    return Err(io::Error::new(io::ErrorKind::Unsupported, "unix daemon socket unsupported"));

    let stream = tokio::time::timeout(
        Duration::from_secs(2),
        tokio::net::UnixStream::connect(sock_path),
    )
    .await??;
    let peer = stream.peer_cred()?;
    #[cfg(target_os = "linux")]
    let trusted_uid = peer.uid() == 0;
    #[cfg(target_os = "macos")]
    let trusted_uid = peer.uid() == 0 || peer.uid() == crate::plat::effective_uid();
    let pid = peer
        .pid()
        .filter(|pid| *pid > 0 && trusted_uid)
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

#[cfg(unix)]
async fn remove_stale_browser_socket(directory: &std::path::Path) -> std::io::Result<()> {
    use std::{
        io,
        os::unix::fs::{FileTypeExt, MetadataExt},
        time::Duration,
    };

    let socket = directory.join("browser.sock");
    let meta = match std::fs::symlink_metadata(&socket) {
        Ok(meta) => meta,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    if !meta.file_type().is_socket() || meta.uid() != crate::plat::effective_uid() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "existing browser.sock is not a private socket",
        ));
    }
    match tokio::time::timeout(
        Duration::from_millis(500),
        tokio::net::UnixStream::connect(&socket),
    )
    .await
    {
        Ok(Err(err)) if err.kind() == io::ErrorKind::ConnectionRefused => {
            match std::fs::remove_file(&socket) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(err),
            }
        }
        Ok(Ok(_)) => Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "browser.sock is already active",
        )),
        Ok(Err(err)) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(Err(err)) => Err(err),
        Err(_) => Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "browser.sock liveness probe timed out",
        )),
    }
}

#[cfg(unix)]
async fn bind_private_socket_with(
    directory: &std::path::Path,
    authority: String,
    proxy: super::webtransport::ProxyIdentity,
) -> std::io::Result<super::webtransport::BrowserListener> {
    match super::webtransport::BrowserListener::bind(directory, authority.clone(), proxy) {
        Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
            remove_stale_browser_socket(directory).await?;
            super::webtransport::BrowserListener::bind(directory, authority, proxy)
        }
        res => res,
    }
}

#[cfg(unix)]
pub async fn bind_private_backend(
    directory: &std::path::Path,
    authority: String,
) -> std::io::Result<super::webtransport::BrowserListener> {
    protected_directory(directory)?;
    let proxy = system_proxy_identity().await?;
    bind_private_socket_with(directory, authority, proxy).await
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
    let Some(curl) = super::super::tailnet::curl_cli() else {
        return false;
    };
    let url = format!("https://{authority}/auth/web/setup");
    let probe = super::webprobe::Probe::start(home, authority);
    let mut cmd = crate::proc::tokio_cmd(crate::proc::Kind::Net, &curl);
    cmd.args([
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
    ]);
    #[cfg(windows)]
    cmd.arg("--ssl-revoke-best-effort");
    let Ok(mut child) = cmd
        .args([
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendTarget {
    Unix(std::path::PathBuf),
    Loopback(u16),
}

impl BackendTarget {
    pub fn proxy_url(&self) -> Option<String> {
        match self {
            Self::Unix(socket) => {
                let path = socket.to_str()?;
                if !path.starts_with('/') || path.chars().any(char::is_control) {
                    return None;
                }
                Some(format!("unix:{path}"))
            }
            Self::Loopback(port) => (*port != 0).then(|| format!("http://127.0.0.1:{port}")),
        }
    }

    pub fn matches_proxy_value(&self, value: &str) -> bool {
        let Some(url) = self.proxy_url() else {
            return false;
        };
        match self {
            Self::Unix(_) => value == url,
            Self::Loopback(_) => value == url || value == format!("{url}/"),
        }
    }
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

pub fn private_serve_args_for(
    service: Option<&str>,
    port: u16,
    target: &BackendTarget,
) -> Option<Vec<String>> {
    if port == 0 || (service.is_none() && port == 443) {
        return None;
    }
    let proxy = target.proxy_url()?;
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
    args.push(proxy);
    Some(args)
}

/// Arguments for the system-installed Tailscale executable. Pass directly
/// to the process API so a path containing spaces stays a single argument.
pub fn private_serve_args(
    service: Option<&str>,
    port: u16,
    socket: &std::path::Path,
) -> Option<Vec<String>> {
    private_serve_args_for(service, port, &BackendTarget::Unix(socket.to_path_buf()))
}

/// Inspect the exact node or Service scope before showing its URL. The
/// caller separately verifies reachability and the identity of the proxy.
pub fn private_route_matches_target(
    config: &Value,
    service: Option<&str>,
    authority: &str,
    port: u16,
    target: &BackendTarget,
) -> bool {
    if target.proxy_url().is_none() {
        return false;
    }
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
    let handler_matches = handlers.as_object().is_some_and(|map| {
        map.len() == 1
            && map
                .get("/")
                .and_then(|entry| entry.get("Proxy"))
                .and_then(Value::as_str)
                .is_some_and(|proxy| target.matches_proxy_value(proxy))
    });
    funnel_off(config)
        && funnel_off(scope)
        && scope["TCP"][port.to_string()] == serde_json::json!({"HTTPS":true})
        && handler_matches
}

pub fn private_route_matches(
    config: &Value,
    service: Option<&str>,
    authority: &str,
    port: u16,
    socket: &std::path::Path,
) -> bool {
    private_route_matches_target(
        config,
        service,
        authority,
        port,
        &BackendTarget::Unix(socket.to_path_buf()),
    )
}

/// Prefer a configured Service; fall back to this node's own HTTPS mapping.
fn configured_authority_for(
    config: &Value,
    service: &str,
    node: &str,
    target: &BackendTarget,
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
            if name == host && private_route_matches_target(config, scope, host, port, target) {
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

#[cfg(test)]
fn configured_authority(
    config: &Value,
    service: &str,
    node: &str,
    socket: &std::path::Path,
) -> Option<String> {
    configured_authority_for(config, service, node, &BackendTarget::Unix(socket.to_path_buf()))
}

// A setup request exists only while this hub's tailnet monitor is running.
type SetupRequests = std::collections::HashMap<std::path::PathBuf, tokio::sync::mpsc::Sender<String>>;
static SETUP_REQUESTS: std::sync::OnceLock<std::sync::Mutex<SetupRequests>> = std::sync::OnceLock::new();

pub fn request_setup(home: &std::path::Path, plan: &str) -> Result<(), &'static str> {
    if plan.len() > 4096 { return Err("invalid setup plan"); }
    let requests = SETUP_REQUESTS.get().ok_or("web setup unavailable")?.lock().unwrap();
    requests.get(home).ok_or("web setup unavailable")?.try_send(plan.to_owned()).map_err(|_| "setup already pending")
}

fn missing_setup_component() -> Option<&'static str> {
    let has_linux_socket = if cfg!(target_os = "linux") {
        use crate::plat::fs::FileTypeExt;
        std::fs::metadata("/run/tailscale/tailscaled.sock").is_ok_and(|file| file.file_type().is_socket())
    } else {
        true
    };
    let has_tailscale = if cfg!(target_os = "linux") {
        std::fs::metadata("/usr/bin/tailscale").is_ok_and(|f| crate::plat::fs::runnable(&f))
    } else {
        super::super::tailnet::tailscale_cli().is_some()
    };
    let has_curl = if cfg!(target_os = "linux") {
        std::fs::metadata("/usr/bin/curl").is_ok_and(|f| crate::plat::fs::runnable(&f))
    } else {
        super::super::tailnet::curl_cli().is_some()
    };
    setup_component_reason_for(std::env::consts::OS, has_linux_socket, has_tailscale, has_curl)
}

#[cfg(test)]
fn setup_component_reason(socket: bool, executable: impl Fn(&str) -> bool) -> Option<&'static str> {
    setup_component_reason_for(
        "linux",
        socket,
        executable("/usr/bin/tailscale"),
        executable("/usr/bin/curl"),
    )
}

pub fn setup_component_reason_for(
    os: &str,
    has_linux_socket: bool,
    has_tailscale: bool,
    has_curl: bool,
) -> Option<&'static str> {
    match os {
        "linux" => {
            if !has_linux_socket {
                return Some("Start Tailscale. Backplane needs its system socket at /run/tailscale/tailscaled.sock.");
            }
            if !has_tailscale {
                return Some("HTTPS setup requires the Tailscale command at /usr/bin/tailscale.");
            }
            if !has_curl {
                return Some("HTTPS verification requires curl at /usr/bin/curl.");
            }
            None
        }
        "macos" => {
            if !has_tailscale {
                return Some("Install Tailscale (from the App Store, tailscale.com, or Homebrew) to set up HTTPS forwarding.");
            }
            if !has_curl {
                return Some("HTTPS verification requires curl at /usr/bin/curl.");
            }
            None
        }
        "windows" => {
            if !has_tailscale {
                return Some("Install Tailscale to set up HTTPS forwarding.");
            }
            if !has_curl {
                return Some("HTTPS verification requires curl.exe.");
            }
            None
        }
        _ => {
            if !has_tailscale {
                return Some("Install Tailscale to set up HTTPS forwarding.");
            }
            if !has_curl {
                return Some("HTTPS verification requires curl.");
            }
            None
        }
    }
}

fn quote_posix_arg(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', "'\\''"))
}

fn quote_posix_program(prog: &str) -> String {
    if !prog.is_empty()
        && prog
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'))
    {
        prog.to_owned()
    } else {
        quote_posix_arg(prog)
    }
}

fn quote_windows_arg(arg: &str) -> String {
    if arg.is_empty() || arg.chars().any(|c| c.is_whitespace() || c == '"') {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_owned()
    }
}

pub fn format_command_for(os: &str, program: &str, args: &[String]) -> String {
    if os == "windows" {
        let mut parts = Vec::with_capacity(args.len() + 1);
        parts.push(quote_windows_arg(program));
        parts.extend(args.iter().map(|a| quote_windows_arg(a)));
        parts.join(" ")
    } else {
        let mut parts = Vec::with_capacity(args.len() + 1);
        parts.push(quote_posix_program(program));
        parts.extend(args.iter().map(|a| quote_posix_arg(a)));
        parts.join(" ")
    }
}

pub fn operator_guidance(os: &str, cli: &str, user_plan: &str) -> (String, &'static str) {
    match os {
        "linux" => (
            r#"sudo tailscale set --operator="$(id -un)""#.to_owned(),
            "This user can manage Tailscale on this computer without sudo.",
        ),
        "macos" => (
            format!(r#"sudo {} set --operator="$(id -un)""#, quote_posix_program(cli)),
            "This user can manage Tailscale on this computer without sudo.",
        ),
        "windows" => (
            user_plan.to_owned(),
            "Run this command once in an Administrator terminal to set up HTTPS forwarding.",
        ),
        _ => (
            user_plan.to_owned(),
            "Run this command with administrator privileges to set up HTTPS forwarding.",
        ),
    }
}

fn read_saved_port(directory: &std::path::Path) -> Option<u16> {
    std::fs::read_to_string(directory.join("backend-port"))
        .ok()?
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|p| *p != 0)
}

fn write_saved_port(directory: &std::path::Path, port: u16) -> std::io::Result<()> {
    use std::io::Write;
    let path = directory.join("backend-port");
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(path)?;
    writeln!(file, "{port}")
}

fn is_permission_refusal(stderr: &str) -> bool {
    let reason = stderr.to_ascii_lowercase();
    [
        "permission denied",
        "access denied",
        "requires root",
        "permission is denied",
        "not authorized",
        "must be run as",
        "requires elevation",
        "administrator",
    ]
    .iter()
    .any(|m| reason.contains(m))
}

fn use_unix_backend(directory: &std::path::Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        let _ = directory;
        true
    }
    #[cfg(target_os = "macos")]
    {
        super::super::tailnet::macos_has_brew_socket()
            && read_saved_port(directory).is_none()
            && directory.join("browser.sock").to_string_lossy().len() < 100
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = directory;
        false
    }
}

async fn enabled_changed(hub: &crate::hub::Hub, changes: &mut tokio::sync::broadcast::Receiver<u64>, enabled: bool) {
    loop {
        let _ = changes.recv().await;
        if (hub.setting("web.enabled").await.as_deref() == Some("1")) != enabled { return; }
    }
}

fn retry_interval(since_action: std::time::Duration, verification_failed: bool) -> std::time::Duration {
    use std::time::Duration;
    if !verification_failed && since_action < Duration::from_secs(60) { Duration::from_secs(2) }
    else { Duration::from_secs(60) }
}

async fn publish_status(hub: &crate::hub::Hub, previous: &mut Value, next: Value) {
    if *previous != next {
        hub.set_info("web.status", &next.to_string()).await;
        *previous = next;
    }
}

pub async fn monitor(hub: std::sync::Arc<crate::hub::Hub>, ts: &dyn super::Tailscale) {
    if crate::copy::on() {
        return;
    }
    let (sender, mut requests) = tokio::sync::mpsc::channel(1);
    SETUP_REQUESTS.get_or_init(Default::default).lock().unwrap().insert(hub.cfg.home.clone(), sender);
    let directory = hub.cfg.home.join("web");
    let mut cleanup_pending = directory.exists();
    let mut refused_plan = None;
    let mut changes = hub.subscribe();
    let mut requested = None;
    let mut previous = Value::Null;
    let mut last_action = std::time::Instant::now();
    let mut verification_failed = false;
    let mut reserved_listener: Option<std::sync::Arc<tokio::net::TcpListener>> = None;
    loop {
        let enabled = hub.setting("web.enabled").await.as_deref() == Some("1");
        let mut status_row = serde_json::json!({"canEnable":true});
        if enabled {
            cleanup_pending = true;
            if let Some(reason) = missing_setup_component() {
                status_row["reason"] = Value::from(reason);
            } else {
                let status: Value = serde_json::from_str(&ts.status().await).unwrap_or(Value::Null);
                let mut config: Value =
                    serde_json::from_str(&ts.serve_status().await).unwrap_or(Value::Null);
                let node = status["Self"]["DNSName"]
                    .as_str()
                    .unwrap_or("")
                    .trim_end_matches('.')
                    .to_ascii_lowercase();
                let service = hub
                    .setting("web.service")
                    .await
                    .unwrap_or_else(|| "bp".into());
                if node.is_empty() {
                    status_row["reason"] = Value::from("Start and sign in to Tailscale to set up HTTPS forwarding.");
                } else {
                    match ensure_protected_directory(&directory) {
                        Err(_) => {
                            status_row["reason"] = Value::from("Web UI runtime directory could not be prepared.");
                        }
                        Ok(canon_dir) => {
                            status_row["reason"] = Value::from("Waiting for a verified private Tailscale HTTPS mapping.");
                            #[cfg(target_os = "linux")]
                            let use_unix = true;
                            #[cfg(target_os = "macos")]
                            let use_unix = use_unix_backend(&canon_dir)
                                && protected_directory(&canon_dir).is_ok()
                                && system_proxy_identity().await.is_ok();
                            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
                            let use_unix = false;
                            let target = if use_unix {
                                reserved_listener = None;
                                if protected_directory(&canon_dir).is_err() {
                                    status_row["reason"] = Value::from("Web UI runtime directory permissions are too permissive.");
                                    None
                                } else {
                                    Some(BackendTarget::Unix(canon_dir.join("browser.sock")))
                                }
                            } else {
                                // Loopback Serve mappings persist across hub restarts so users do not need
                                // to re-approve forwarding each time; while running, `reserved_listener`
                                // keeps the port reserved until `Mapping::Absent` is confirmed on disable.
                                let port = if let Some(ref listener) = reserved_listener {
                                    listener.local_addr().ok().map(|a| a.port())
                                } else if let Some(saved) = read_saved_port(&canon_dir) {
                                    if let Ok(tcp) = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, saved)).await {
                                        reserved_listener = Some(std::sync::Arc::new(tcp));
                                        Some(saved)
                                    } else if configured_authority_for(&config, &service, &node, &BackendTarget::Loopback(saved)).is_some() {
                                        Some(saved)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };
                                if let Some(p) = port {
                                    Some(BackendTarget::Loopback(p))
                                } else if let Ok(tcp) = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await {
                                    let new_port = tcp.local_addr().ok().map(|a| a.port()).unwrap_or(0);
                                    if new_port != 0 && write_saved_port(&canon_dir, new_port).is_ok() {
                                        reserved_listener = Some(std::sync::Arc::new(tcp));
                                        Some(BackendTarget::Loopback(new_port))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            };
                            if let Some(target) = target {
                                let cli_path = super::super::tailnet::tailscale_cli();
                                let cli_str = cli_path
                                    .as_ref()
                                    .map(|p| p.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| "tailscale".to_owned());
                                if configured_authority_for(&config, &service, &node, &target).is_none() {
                                    if let Some(args) = spare_node_port(&config)
                                        .and_then(|port| private_serve_args_for(None, port, &target))
                                    {
                                        let user_plan = format_command_for(std::env::consts::OS, &cli_str, &args);
                                        if requested.as_ref() == Some(&user_plan) {
                                            #[cfg(unix)]
                                            let unix_ok = !matches!(target, BackendTarget::Unix(_))
                                                || (protected_directory(&canon_dir).is_ok()
                                                    && system_proxy_identity().await.is_ok());
                                            #[cfg(not(unix))]
                                            let unix_ok = true;
                                            if unix_ok {
                                                let mut cmd = crate::proc::tokio_cmd(crate::proc::Kind::Net, &cli_str);
                                                if let Ok(out) = cmd.args(&args).env("LC_ALL", "C").stdin(std::process::Stdio::null()).output().await {
                                                    let reason = String::from_utf8_lossy(&out.stderr);
                                                    if out.status.success() {
                                                        refused_plan = None;
                                                        config = serde_json::from_str(&ts.serve_status().await).unwrap_or(Value::Null);
                                                    } else if is_permission_refusal(&reason) {
                                                        refused_plan = Some(user_plan.clone());
                                                    } else {
                                                        status_row["reason"] = Value::from("Private HTTPS setup did not complete.");
                                                    }
                                                }
                                            }
                                        }
                                        if refused_plan.as_ref() == Some(&user_plan) {
                                            let (op_cmd, op_note) = operator_guidance(std::env::consts::OS, &cli_str, &user_plan);
                                            status_row["reason"] = Value::from("Tailscale refused setup. This account needs operator permission on this computer.");
                                            status_row["operatorRequired"] = Value::Bool(true);
                                            status_row["operatorCommand"] = Value::from(op_cmd);
                                            status_row["operatorNote"] = Value::from(op_note);
                                        }
                                        status_row["command"] = Value::from(user_plan);
                                    }
                                }
                                if let Some(authority) = configured_authority_for(&config, &service, &node, &target) {
                                    match target {
                                        #[cfg(unix)]
                                        BackendTarget::Unix(socket) => {
                                            match bind_private_backend(&canon_dir, authority.clone()).await {
                                                Ok(listener) => {
                                                    let mut task = tokio::spawn(listener.run(hub.clone()));
                                                    if https_address_answers(&hub.cfg.home, &authority).await {
                                                        let active = serde_json::json!({"available":true, "address":format!("https://{authority}")});
                                                        publish_status(&hub, &mut previous, active).await;
                                                        tokio::select! {
                                                            _ = &mut task => (),
                                                            _ = enabled_changed(&hub, &mut changes, true) => (),
                                                        }
                                                        task.abort();
                                                        let _ = task.await;
                                                        let _ = std::fs::remove_file(&socket);
                                                        requested = None;
                                                        verification_failed = false;
                                                        last_action = std::time::Instant::now();
                                                        continue;
                                                    }
                                                    verification_failed = true;
                                                    status_row["reason"] = Value::from("Private HTTPS forwarding could not be verified. Backplane will retry.");
                                                    if previous != status_row {
                                                        eprintln!("backplane: web ui HTTPS verification failed for https://{authority}");
                                                    }
                                                    task.abort();
                                                    let _ = task.await;
                                                    let _ = std::fs::remove_file(&socket);
                                                }
                                                Err(err) => {
                                                    status_row["reason"] = Value::from(if err.kind() == std::io::ErrorKind::AddrInUse {
                                                        "Local Web UI socket is in use by another process. Backplane will retry."
                                                    } else {
                                                        "Local Web UI socket could not be bound. Backplane will retry."
                                                    });
                                                    if previous != status_row {
                                                        eprintln!("backplane: web ui backend bind failed for {authority}: {err}");
                                                    }
                                                }
                                            }
                                        }
                                        #[cfg(not(unix))]
                                        BackendTarget::Unix(_) => {}
                                        BackendTarget::Loopback(port) => {
                                            if !reserved_listener.as_ref().is_some_and(|l| l.local_addr().is_ok_and(|a| a.port() == port)) {
                                                reserved_listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
                                                    .await
                                                    .ok()
                                                    .map(std::sync::Arc::new);
                                            }
                                            if let Some(ref tcp) = reserved_listener {
                                                if let Ok(listener) = super::webtransport::LoopbackBrowserListener::from_listener(std::sync::Arc::clone(tcp), authority.clone()) {
                                                    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
                                                    let mut task = tokio::spawn(listener.run(hub.clone(), stop_rx));
                                                    let verified = https_address_answers(&hub.cfg.home, &authority).await;
                                                    if verified {
                                                        let active = serde_json::json!({"available":true, "address":format!("https://{authority}")});
                                                        publish_status(&hub, &mut previous, active).await;
                                                        tokio::select! {
                                                            _ = &mut task => (),
                                                            _ = enabled_changed(&hub, &mut changes, true) => {
                                                                let _ = stop_tx.send(());
                                                                if tokio::time::timeout(std::time::Duration::from_secs(2), &mut task).await.is_err() {
                                                                    task.abort();
                                                                }
                                                            }
                                                        }
                                                        requested = None;
                                                        verification_failed = false;
                                                        last_action = std::time::Instant::now();
                                                        continue;
                                                    }
                                                    verification_failed = true;
                                                    status_row["reason"] = Value::from("Private HTTPS forwarding could not be verified. Backplane will retry.");
                                                    let _ = stop_tx.send(());
                                                    if tokio::time::timeout(std::time::Duration::from_secs(2), &mut task).await.is_err() {
                                                        task.abort();
                                                    }
                                                }
                                            } else {
                                                verification_failed = true;
                                                status_row["reason"] = Value::from("Local Web UI port is in use by another process. Turn Browser access off and back on to reset forwarding.");
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else if cleanup_pending {
            refused_plan = None;
            let service = hub.setting("web.service").await.unwrap_or_else(|| "bp".into());
            let dir = crate::plat::fs::canonicalize(&directory).unwrap_or_else(|_| directory.clone());
            let target = if let Some(port) = reserved_listener
                .as_ref()
                .and_then(|l| l.local_addr().ok().map(|a| a.port()))
                .or_else(|| read_saved_port(&dir))
            {
                BackendTarget::Loopback(port)
            } else {
                BackendTarget::Unix(dir.join("browser.sock"))
            };
            let ts_status_raw = ts.serve_status().await;
            let ts_config: Value = serde_json::from_str(&ts_status_raw).unwrap_or(Value::Null);
            cleanup_pending = !cleanup_disabled_target(ts, &service, &target, |scope, host, port, tgt| async move {
                match tgt {
                    #[cfg(target_os = "linux")]
                    BackendTarget::Unix(path) => remove_private_mapping(scope.as_deref(), &host, port, &path).await,
                    _ => remove_private_mapping_cli(&ts_config, scope.as_deref(), &host, port, &tgt).await,
                }
            }).await;
            if cleanup_pending {
                status_row["reason"] = Value::from("Browser access is off. Tailscale mapping removal could not be confirmed.");
            } else {
                reserved_listener = None;
                let _ = std::fs::remove_file(directory.join("backend-port"));
                let _ = std::fs::remove_file(directory.join("browser.sock"));
            }
        }
        publish_status(&hub, &mut previous, status_row).await;
        requested = None;
        tokio::select! {
            plan = requests.recv() => { requested = plan; }
            _ = enabled_changed(&hub, &mut changes, enabled) => (),
            _ = tokio::time::sleep(retry_interval(last_action.elapsed(), verification_failed)) => continue,
        }
        last_action = std::time::Instant::now();
        verification_failed = false;
    }
}

async fn cleanup_disabled_target<F, R>(
    ts: &dyn super::Tailscale,
    service: &str,
    target: &BackendTarget,
    remove: F,
) -> bool
where
    F: FnOnce(Option<String>, String, u16, BackendTarget) -> R,
    R: std::future::Future<Output = bool>,
{
    let Ok(status) = serde_json::from_str::<Value>(&ts.status().await) else { return false; };
    let Some(node) = status["Self"]["DNSName"]
        .as_str()
        .map(|s| s.trim_end_matches('.').to_ascii_lowercase())
        .filter(|s| !s.is_empty())
    else {
        return false;
    };
    let raw = ts.serve_status().await;
    let Ok(config) = serde_json::from_str::<Value>(&raw) else { return false; };
    if !config.is_object() { return false; }
    let Some(authority) = configured_authority_for(&config, service, &node, target) else {
        return private_backend_absent_target(&raw, target);
    };
    let (host, port) = authority.rsplit_once(':').map_or((authority.as_str(), 443), |(host, port)| (host, port.parse().unwrap_or(0)));
    let scope = (host != node).then(|| service.to_owned());
    let _ = remove(scope, host.to_owned(), port, target.clone()).await;
    // Read back on the next pass. The other address mode may also have a mapping.
    false
}

#[cfg(test)]
async fn cleanup_disabled<F, R>(ts: &dyn super::Tailscale, service: &str, socket: &std::path::Path, remove: F) -> bool
where F: FnOnce(Option<String>, String, u16, std::path::PathBuf) -> R, R: std::future::Future<Output=bool> {
    cleanup_disabled_target(ts, service, &BackendTarget::Unix(socket.to_path_buf()), |scope, host, port, target| async move {
        let BackendTarget::Unix(path) = target else { return false; };
        remove(scope, host, port, path).await
    }).await
}

// A shared handler cannot be removed automatically. Scan all scopes and paths:
// either Present or Unconfirmed keeps the disabled-state warning visible.
fn private_backend_absent_target(raw: &str, target: &BackendTarget) -> bool {
    let Some(url) = target.proxy_url() else {
        return false;
    };
    if mapping_status(raw, "", &url) != Mapping::Absent {
        return false;
    }
    if matches!(target, BackendTarget::Loopback(_))
        && mapping_status(raw, "", &format!("{url}/")) != Mapping::Absent
    {
        return false;
    }
    true
}

#[cfg(test)]
fn private_backend_absent(raw: &str, socket: &std::path::Path) -> bool {
    private_backend_absent_target(raw, &BackendTarget::Unix(socket.to_path_buf()))
}

pub fn other_authority_on_port(
    config: &Value,
    service: Option<&str>,
    hostname: &str,
    port: u16,
) -> bool {
    let scope = match service {
        Some(name) => &config["Services"][format!("svc:{name}")],
        None => config,
    };
    let Some(web) = scope.get("Web").and_then(Value::as_object) else {
        return false;
    };
    let own = format!("{hostname}:{port}");
    let suffix = format!(":{port}");
    web.keys().any(|k| k != &own && k.ends_with(&suffix))
}

pub fn private_remove_args_for(
    config: &Value,
    service: Option<&str>,
    hostname: &str,
    port: u16,
    target: &BackendTarget,
) -> Option<Vec<String>> {
    let mut args = private_serve_args_for(service, port, target)?;
    let scope = service.map(|name| format!("svc:{name}"));
    if !private_route_matches_target(config, scope.as_deref(), hostname, port, target) {
        return None;
    }
    args.retain(|arg| arg != "--bg");
    *args.last_mut()? = String::from("off");
    Some(args)
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
    private_remove_args_for(
        config,
        service,
        hostname,
        port,
        &BackendTarget::Unix(socket.to_path_buf()),
    )
}

/// Prepare the minimal Serve config update for a conditional LocalAPI write.
/// The caller must submit the original GET's ETag in If-Match.
#[allow(dead_code)]
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

#[cfg(target_os = "linux")]
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
    if input.write_all(updated.to_string().as_bytes()).await.is_err() { return false; };
    drop(input);
    child.wait().await.is_ok_and(|s| s.success())
}

async fn remove_private_mapping_cli(
    config: &Value,
    service: Option<&str>,
    host: &str,
    port: u16,
    target: &BackendTarget,
) -> bool {
    if crate::copy::on() || other_authority_on_port(config, service, host, port) {
        return false;
    }
    let Some(args) = private_remove_args_for(config, service, host, port, target) else {
        return false;
    };
    let cli = super::super::tailnet::tailscale_program();
    matches!(
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            crate::proc::tokio_cmd(crate::proc::Kind::Net, cli)
                .args(&args)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true)
                .status(),
        )
        .await,
        Ok(Ok(s)) if s.success()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn idle_hour_bounds_tailscale_status_reads() {
        use std::{sync::atomic::{AtomicUsize, Ordering}, time::Duration};
        use super::super::Tailscale;
        struct Count(AtomicUsize);
        impl Tailscale for Count {
            fn ip4(&self) -> super::super::Fut<String> { unreachable!() }
            fn whois(&self, _: String) -> super::super::Fut<String> { unreachable!() }
            fn status(&self) -> super::super::Fut<String> {
                self.0.fetch_add(1, Ordering::Relaxed);
                Box::pin(async { String::new() })
            }
            fn serve_status(&self) -> super::super::Fut<String> { self.status() }
        }
        let ts = Count(AtomicUsize::new(0));
        let mut elapsed = Duration::ZERO;
        while elapsed < Duration::from_secs(3600) {
            ts.status().await;
            ts.serve_status().await;
            elapsed += retry_interval(elapsed, false);
        }
        assert_eq!(ts.0.load(Ordering::Relaxed), 178);
        assert_eq!(retry_interval(Duration::ZERO, true), Duration::from_secs(60));
    }

    #[test]
    fn missing_components_name_the_path() {
        assert!(setup_component_reason(false, |_| true).unwrap().contains("/run/tailscale/tailscaled.sock"));
        for path in ["/usr/bin/tailscale", "/usr/bin/curl"] {
            assert!(setup_component_reason(true, |candidate| candidate != path).unwrap().contains(path));
        }
        assert!(setup_component_reason(true, |_| true).is_none());
    }

    #[tokio::test]
    async fn disabling_before_bind_cleans_up_a_mapping_installed_by_the_pending_prompt() {
        struct AfterPrompt;
        impl super::super::Tailscale for AfterPrompt {
            fn ip4(&self) -> super::super::Fut<String> { unreachable!() }
            fn whois(&self, _: String) -> super::super::Fut<String> { unreachable!() }
            fn status(&self) -> super::super::Fut<String> { Box::pin(async { json!({"Self":{"DNSName":"node.example.ts.net."}}).to_string() }) }
            fn serve_status(&self) -> super::super::Fut<String> { Box::pin(async { json!({"TCP":{"8443":{"HTTPS":true}},"Web":{"node.example.ts.net:8443":{"Handlers":{"/":{"Proxy":"unix:/private/browser.sock"}}}}}).to_string() }) }
        }
        let mut called = false;
        let path = std::path::Path::new("/private/browser.sock");
        let confirmed = cleanup_disabled(&AfterPrompt, "bp", path, |scope, host, port, backend| {
            called = true;
            assert_eq!((scope, host.as_str(), port, backend.as_path()), (None, "node.example.ts.net", 8443, path));
            std::future::ready(true)
        }).await;
        assert!(called);
        assert!(!confirmed, "removal still needs readback");
    }

    #[test]
    fn sibling_handler_keeps_cleanup_pending_until_private_backend_disappears() {
        let socket = std::path::Path::new("/private/browser.sock");
        let mut config = json!({"TCP":{"8443":{"HTTPS":true}},"Web":{"node.example.ts.net:8443":{"Handlers":{
            "/":{"Proxy":"unix:/private/browser.sock"}, "/other":{"Text":"keep"}
        }}}});
        assert!(configured_authority(&config, "bp", "node.example.ts.net", socket).is_none());
        assert!(!private_backend_absent(&config.to_string(), socket));
        config["Web"]["node.example.ts.net:8443"]["Handlers"].as_object_mut().unwrap().remove("/");
        assert!(private_backend_absent(&config.to_string(), socket));
    }

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

    #[cfg(unix)]
    #[test]
    fn private_socket_directory_rejects_a_writable_parent() {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let scratch = crate::plat::tmp::dir("web-unsafe-parent");
        let parent = scratch.join("parent");
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
    }

    #[test]
    fn ensure_protected_directory_creates_and_validates_directory() {
        let scratch = crate::plat::tmp::dir("web-ensure-dir");
        let dir = scratch.join("web");
        let canon = ensure_protected_directory(&dir).unwrap();
        assert!(canon.is_absolute());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&canon).unwrap().permissions().mode() & 0o777, 0o700);
        }
        #[cfg(windows)]
        assert!(protected_directory(&canon).is_ok());
        assert!(protected_directory(std::path::Path::new("relative")).is_err());
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

    #[test]
    fn loopback_target_supports_serve_args_route_matching_and_removal() {
        let target = BackendTarget::Loopback(49231);
        assert_eq!(
            private_serve_args_for(None, 8443, &target).unwrap(),
            ["serve", "--bg", "--https=8443", "http://127.0.0.1:49231"]
        );
        assert_eq!(
            private_serve_args_for(Some("bp"), 443, &target).unwrap(),
            ["serve", "--bg", "--service=svc:bp", "--https=443", "http://127.0.0.1:49231"]
        );
        assert!(private_serve_args_for(None, 8443, &BackendTarget::Loopback(0)).is_none());

        let node = "win-box.tail1.ts.net";
        let config = json!({
            "TCP": {"8443": {"HTTPS": true}},
            "Web": {"win-box.tail1.ts.net:8443": {"Handlers": {"/": {"Proxy": "http://127.0.0.1:49231/"}}}}
        });
        assert_eq!(
            configured_authority_for(&config, "bp", node, &target).as_deref(),
            Some("win-box.tail1.ts.net:8443")
        );
        assert_eq!(
            private_remove_args_for(&config, None, node, 8443, &target).unwrap(),
            ["serve", "--https=8443", "off"]
        );
        assert!(!other_authority_on_port(&config, None, node, 8443));

        let shared = json!({
            "TCP": {"8443": {"HTTPS": true}},
            "Web": {
                "win-box.tail1.ts.net:8443": {"Handlers": {"/": {"Proxy": "http://127.0.0.1:49231"}}},
                "other.tail1.ts.net:8443": {"Handlers": {"/": {"Text": "keep"}}}
            }
        });
        assert!(other_authority_on_port(&shared, None, node, 8443));
        assert!(!private_backend_absent_target(&config.to_string(), &target));
        assert!(private_backend_absent_target(&json!({}).to_string(), &target));
    }

    #[test]
    fn cross_platform_commands_missing_reasons_and_operator_guidance() {
        let args = vec![
            "serve".to_string(),
            "--bg".to_string(),
            "--https=8443".to_string(),
            "http://127.0.0.1:49231".to_string(),
        ];
        assert_eq!(
            format_command_for("macos", "/Applications/Tailscale.app/Contents/MacOS/Tailscale", &args),
            "/Applications/Tailscale.app/Contents/MacOS/Tailscale 'serve' '--bg' '--https=8443' 'http://127.0.0.1:49231'"
        );
        assert_eq!(
            format_command_for("windows", r"C:\Program Files\Tailscale\tailscale.exe", &args),
            r#""C:\Program Files\Tailscale\tailscale.exe" serve --bg --https=8443 http://127.0.0.1:49231"#
        );

        assert!(setup_component_reason_for("macos", false, true, true).is_none());
        assert!(setup_component_reason_for("macos", false, false, true).unwrap().contains("App Store"));
        assert!(setup_component_reason_for("windows", false, true, true).is_none());
        assert!(setup_component_reason_for("windows", false, true, false).unwrap().contains("curl.exe"));

        let (mac_cmd, mac_note) = operator_guidance("macos", "/Applications/Tailscale.app/Contents/MacOS/Tailscale", "plan");
        assert!(mac_cmd.contains("sudo /Applications/Tailscale.app/Contents/MacOS/Tailscale set --operator="));
        assert!(mac_note.contains("without sudo"));

        let (win_cmd, win_note) = operator_guidance("windows", r"C:\Program Files\Tailscale\tailscale.exe", "win-plan");
        assert_eq!(win_cmd, "win-plan");
        assert!(win_note.contains("Administrator"));
    }

    #[tokio::test]
    async fn disabling_loopback_target_triggers_removal_and_needs_readback() {
        struct LoopbackTs;
        impl super::super::Tailscale for LoopbackTs {
            fn ip4(&self) -> super::super::Fut<String> { unreachable!() }
            fn whois(&self, _: String) -> super::super::Fut<String> { unreachable!() }
            fn status(&self) -> super::super::Fut<String> {
                Box::pin(async { json!({"Self":{"DNSName":"win.example.ts.net."}}).to_string() })
            }
            fn serve_status(&self) -> super::super::Fut<String> {
                Box::pin(async {
                    json!({"TCP":{"8443":{"HTTPS":true}},"Web":{"win.example.ts.net:8443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:49231"}}}}}).to_string()
                })
            }
        }
        let target = BackendTarget::Loopback(49231);
        let mut called = false;
        let confirmed = cleanup_disabled_target(&LoopbackTs, "bp", &target, |scope, host, port, tgt| {
            called = true;
            assert_eq!((scope, host.as_str(), port, tgt), (None, "win.example.ts.net", 8443, BackendTarget::Loopback(49231)));
            std::future::ready(true)
        }).await;
        assert!(called);
        assert!(!confirmed);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stale_browser_socket_is_replaced_on_rebind() {
        use std::os::unix::fs::PermissionsExt;
        let dir = crate::plat::tmp::dir("web-stale-sock");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let proxy = super::super::webtransport::ProxyIdentity {
            uid: crate::plat::effective_uid(),
            pid: std::process::id() as i32,
        };
        let authority = "node.example.ts.net:8444";
        let first = bind_private_socket_with(&dir, authority.into(), proxy)
            .await
            .unwrap();
        drop(first);
        let sock_path = dir.join("browser.sock");
        assert!(sock_path.exists());
        let err = match super::super::webtransport::BrowserListener::bind(&dir, authority.into(), proxy) {
            Ok(_) => panic!("stale socket must cause plain BrowserListener::bind to fail"),
            Err(err) => err,
        };
        assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse);

        let second = bind_private_socket_with(&dir, authority.into(), proxy)
            .await
            .unwrap();
        let _client = tokio::net::UnixStream::connect(&sock_path).await.unwrap();
        drop(second);
        let _ = std::fs::remove_file(&sock_path);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn active_browser_socket_is_never_unlinked_on_rebind() {
        use std::os::unix::fs::PermissionsExt;
        let dir = crate::plat::tmp::dir("web-active-sock");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let proxy = super::super::webtransport::ProxyIdentity {
            uid: crate::plat::effective_uid(),
            pid: std::process::id() as i32,
        };
        let authority = "node.example.ts.net:8444";
        let live = bind_private_socket_with(&dir, authority.into(), proxy)
            .await
            .unwrap();
        let err = match bind_private_socket_with(&dir, authority.into(), proxy).await {
            Ok(_) => panic!("second bind must fail while first listener is alive"),
            Err(err) => err,
        };
        assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse);

        let sock_path = dir.join("browser.sock");
        let _client = tokio::net::UnixStream::connect(&sock_path).await.unwrap();
        drop(live);
        let _ = std::fs::remove_file(&sock_path);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn non_socket_at_browser_sock_path_is_refused_and_preserved() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = crate::plat::tmp::dir("web-nonsock");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let proxy = super::super::webtransport::ProxyIdentity {
            uid: crate::plat::effective_uid(),
            pid: std::process::id() as i32,
        };
        let authority = "node.example.ts.net:8444";
        let sock_path = dir.join("browser.sock");

        std::fs::write(&sock_path, b"keep").unwrap();
        let err = match bind_private_socket_with(&dir, authority.into(), proxy).await {
            Ok(_) => panic!("regular file at browser.sock must be refused"),
            Err(err) => err,
        };
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(std::fs::read(&sock_path).unwrap(), b"keep");
        std::fs::remove_file(&sock_path).unwrap();

        let target = dir.join("target-file");
        std::fs::write(&target, b"preserve").unwrap();
        symlink(&target, &sock_path).unwrap();
        let err = match bind_private_socket_with(&dir, authority.into(), proxy).await {
            Ok(_) => panic!("symlink at browser.sock must be refused"),
            Err(err) => err,
        };
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(std::fs::symlink_metadata(&sock_path).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read(&target).unwrap(), b"preserve");
    }

    #[test]
    fn configured_authority_finds_unix_socket_among_multiple_serve_entries() {
        let socket = std::path::Path::new("/private/home/web/browser.sock");
        let target = BackendTarget::Unix(socket.to_path_buf());
        let config = json!({
            "TCP": {
                "8443": {"HTTPS": true},
                "8444": {"HTTPS": true},
                "8447": {"HTTPS": true}
            },
            "Web": {
                "box.tail1.ts.net:8443": {"Handlers": {"/": {"Proxy": "http://127.0.0.1:8047"}}},
                "box.tail1.ts.net:8444": {"Handlers": {"/": {"Proxy": "unix:/private/home/web/browser.sock"}}},
                "box.tail1.ts.net:8447": {"Handlers": {"/": {"Path": "/srv/static"}}}
            }
        });
        assert_eq!(
            configured_authority_for(&config, "bp", "box.tail1.ts.net", &target).as_deref(),
            Some("box.tail1.ts.net:8444")
        );
        assert_eq!(spare_node_port(&config), Some(8445));
    }
}
