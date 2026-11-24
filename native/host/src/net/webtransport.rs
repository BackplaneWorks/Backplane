//! Private backend for a separately configured HTTPS Serve proxy.
//! Provisioning supplies the authority and authenticated daemon identity.
//! The release startup path does not construct this listener.
use std::{io, net::{Ipv4Addr, SocketAddr}, sync::Arc, time::Duration};
#[cfg(unix)]
use std::{os::unix::fs::MetadataExt, path::Path};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

#[cfg(unix)]
pub(crate) struct ProxyIdentity {
    pub uid: u32,
    pub pid: i32,
}

#[cfg(unix)]
impl ProxyIdentity {
    fn accepts(&self, stream: &UnixStream) -> bool {
        self.pid > 0 && stream.peer_cred().is_ok_and(|peer| {
            peer.uid() == self.uid && peer.pid() == Some(self.pid)
        })
    }
}

#[cfg(unix)]
async fn refresh_for_peer(
    proxy: &mut ProxyIdentity,
    stream: &UnixStream,
    last_lookup: &mut Option<std::time::Instant>,
    lookup: impl std::future::Future<Output = io::Result<ProxyIdentity>>,
) -> bool {
    if !stream.peer_cred().is_ok_and(|peer| peer.uid() == proxy.uid)
        || last_lookup.is_some_and(|at| at.elapsed() < Duration::from_secs(1)) {
        return false;
    }
    *last_lookup = Some(std::time::Instant::now());
    let Ok(current) = lookup.await else { return false; };
    if current.uid != proxy.uid || !current.accepts(stream) { return false; }
    *proxy = current;
    true
}

#[cfg(unix)]
pub(crate) struct BrowserListener {
    listener: UnixListener,
    authority: String,
    proxy: ProxyIdentity,
}

pub(crate) fn validate_authority(authority: &str) -> io::Result<()> {
    let (host, port) = authority.split_once(':').map_or((authority, None), |(h,p)| (h, Some(p)));
    if !host.ends_with(".ts.net") || host.split('.').count() < 4 || host.len() > 253
        || host.split('.').any(|label| label.is_empty() || label.len() > 63 || label.starts_with('-')
            || label.ends_with('-') || !label.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'))
        || port.is_some_and(|p| p.parse::<u16>().map_or(true, |n| matches!(n, 0 | 443) || n.to_string() != p)) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid browser proxy configuration"));
    }
    Ok(())
}

#[cfg(unix)]
impl BrowserListener {
    /// Caller owns the private runtime directory and its socket lifecycle.
    /// An occupied path fails the bind, including after an unclean shutdown.
    pub fn bind(directory: &Path, authority: String, proxy: ProxyIdentity) -> io::Result<Self> {
        let meta = std::fs::symlink_metadata(directory)?;
        if !directory.is_absolute() || !meta.is_dir() || meta.mode() & 0o777 != 0o700
            || meta.uid() != crate::plat::effective_uid() {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "private browser runtime directory required"));
        }
        validate_authority(&authority)?;
        if proxy.pid <= 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid proxy pid"));
        }
        let listener = UnixListener::bind(directory.join("browser.sock"))?;
        Ok(Self { listener, authority, proxy })
    }

    pub async fn run(mut self, hub: Arc<crate::hub::Hub>) -> io::Result<()> {
        let mut last_lookup = None;
        loop {
            let stream = match self.listener.accept().await {
                Ok((stream, _)) => stream,
                Err(e) if matches!(e.kind(), io::ErrorKind::ConnectionAborted | io::ErrorKind::Interrupted) => continue,
                Err(e) if matches!(e.raw_os_error(), Some(crate::plat::errno::EMFILE | crate::plat::errno::ENFILE)) => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
                Err(e) => return Err(e),
            };
            let admitted = self.proxy.accepts(&stream)
                || refresh_for_peer(&mut self.proxy, &stream, &mut last_lookup,
                    super::webprovision::system_proxy_identity()).await;
            if admitted {
                let hub = hub.clone();
                let authority = self.authority.clone();
                tokio::spawn(async move { let _ = super::webingress::serve_unix(hub, stream, &authority).await; });
            }
        }
    }
}

/// Loopback TCP backend used when the OS Tailscale daemon serves via loopback
/// (Windows, and macOS sandboxed App Store Tailscale without a root Unix socket).
/// Retains and returns the bound `TcpListener` on stop so the port cannot be
/// claimed by another local process before Serve teardown completes.
pub(crate) struct LoopbackBrowserListener {
    listener: tokio::net::TcpListener,
    authority: String,
    backend_port: u16,
}

impl LoopbackBrowserListener {
    pub fn from_listener(listener: tokio::net::TcpListener, authority: String) -> io::Result<Self> {
        validate_authority(&authority)?;
        let addr = listener.local_addr()?;
        if addr.ip() != Ipv4Addr::LOCALHOST || addr.port() == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "loopback IPv4 listener required"));
        }
        Ok(Self {
            listener,
            authority,
            backend_port: addr.port(),
        })
    }

    pub async fn bind(authority: String, port: u16) -> io::Result<Self> {
        validate_authority(&authority)?;
        let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await?;
        Self::from_listener(listener, authority)
    }

    pub fn port(&self) -> u16 {
        self.backend_port
    }

    pub async fn run(
        self,
        hub: Arc<crate::hub::Hub>,
        stop: tokio::sync::oneshot::Receiver<()>,
    ) -> io::Result<tokio::net::TcpListener> {
        self.run_with(hub, stop, crate::plat::browser_loopback_peer_ok).await
    }

    pub(crate) async fn run_with<F>(
        self,
        hub: Arc<crate::hub::Hub>,
        mut stop: tokio::sync::oneshot::Receiver<()>,
        verify: F,
    ) -> io::Result<tokio::net::TcpListener>
    where
        F: Fn(&tokio::net::TcpStream) -> bool + Clone + Send + Sync + 'static,
    {
        loop {
            let stream = tokio::select! {
                _ = &mut stop => return Ok(self.listener),
                accepted = self.listener.accept() => match accepted {
                    Ok((stream, _)) => stream,
                    Err(e) if matches!(e.kind(), io::ErrorKind::ConnectionAborted | io::ErrorKind::Interrupted) => continue,
                    Err(e) if matches!(e.raw_os_error(), Some(crate::plat::errno::EMFILE | crate::plat::errno::ENFILE)) => {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            };
            let verify = verify.clone();
            let lookup = tokio::task::spawn_blocking(move || verify(&stream).then_some(stream));
            if let Ok(Ok(Some(stream))) = tokio::time::timeout(Duration::from_millis(300), lookup).await {
                let authority = self.authority.clone();
                let hub = hub.clone();
                let backend_port = self.backend_port;
                tokio::spawn(async move {
                    let _ = super::webingress::serve_loopback(hub, stream, &authority, backend_port).await;
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn restarted_proxy_must_match_fresh_daemon_credentials() {
        let (stream, _peer) = UnixStream::pair().unwrap();
        let uid = crate::plat::effective_uid();
        let pid = std::process::id() as i32;
        let mut cached = ProxyIdentity { uid, pid: pid + 1 };
        let mut last = None;
        assert!(!refresh_for_peer(&mut cached, &stream, &mut last,
            std::future::ready(Ok(ProxyIdentity { uid, pid: pid + 2 }))).await);
        assert!(!refresh_for_peer(&mut cached, &stream, &mut last,
            async { panic!("rate-limited lookup") }).await);
        assert!(!cached.accepts(&stream));
        last = None;
        assert!(refresh_for_peer(&mut cached, &stream, &mut last,
            std::future::ready(Ok(ProxyIdentity { uid, pid }))).await);
        assert!(cached.accepts(&stream));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bind_requires_a_private_directory_and_canonical_authority() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let tmp = crate::plat::tmp::dir("web-bind"); let dir = tmp.to_path_buf(); // removed when `tmp` drops, also when an assert fails
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let bind = |path: &Path, host: &str, pid| BrowserListener::bind(path, host.into(),
            ProxyIdentity { uid: crate::plat::effective_uid(), pid });
        let host = "hub.example.ts.net";
        assert!(bind(Path::new("."), host, 1).is_err());
        let link = dir.join("link");
        symlink(&dir, &link).unwrap();
        assert!(bind(&link, host, 1).is_err());
        std::fs::remove_file(link).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(bind(&dir, host, 1).is_err());
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(bind(&dir, host, 0).is_err());
        for invalid in ["example.org", "a.ts.net", "A.example.ts.net", "-a.example.ts.net",
            "a-.example.ts.net", "a.b.ts.net:0", "a.b.ts.net:08443", "a.b.ts.net:443"] {
            assert!(bind(&dir, invalid, 1).is_err(), "{invalid}");
        }
        let listener = bind(&dir, "hub.example.ts.net:8443", 1).unwrap();
        assert!(bind(&dir, host, 1).is_err());
        drop(listener);
        std::fs::remove_file(dir.join("browser.sock")).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn browser_transport_checks_peer_process_as_well_as_user() {
        let (stream, _peer) = UnixStream::pair().unwrap();
        let identity = ProxyIdentity { uid: crate::plat::effective_uid(), pid: std::process::id() as i32 };
        assert!(identity.accepts(&stream));
        let other_process = ProxyIdentity { pid: identity.pid + 1, ..identity };
        assert!(!other_process.accepts(&stream));
    }

    #[test]
    fn windows_trusted_tailscale_path_checks() {
        use crate::plat::is_trusted_tailscale_windows_path as trusted;
        assert!(trusted(r"C:\Program Files\Tailscale\tailscaled.exe", None, None));
        assert!(trusted(r"c:\program files\tailscale\tailscale-ipn.exe", None, None));
        assert!(trusted(r"D:\Apps\Tailscale\tailscale.exe", Some(r"D:\Apps"), None));
        assert!(trusted(r"C:\Program Files (x86)\Tailscale\tailscaled.exe", None, Some(r"C:\Program Files (x86)")));
        assert!(!trusted(r"C:\Users\bob\tailscaled.exe", None, None));
        assert!(!trusted(r"C:\Program Files\Tailscale\sub\tailscaled.exe", None, None));
        assert!(!trusted(r"C:\Program Files\Tailscale\other.exe", None, None));
    }

    #[tokio::test]
    async fn loopback_listener_enforces_peer_check_and_retains_port_on_stop() {
        use serde_json::json;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let home = crate::plat::tmp::dir("loopback-listener");
        std::fs::write(home.join("token"), "tok").unwrap();
        std::fs::write(
            home.join("events.jsonl"),
            "{\"$\":\"SettingSet\",\"key\":\"web.enabled\",\"value\":\"1\",\"at\":1}\n",
        )
        .unwrap();
        let cfg = crate::hub::Cfg {
            home: home.to_path_buf(),
            version: "0.16.4".into(),
            commit: "test".into(),
            listen: "127.0.0.1:0".into(),
            os: "windows".into(),
            arch: "x64".into(),
        };
        let hub = crate::hub::Hub::start(cfg, Arc::new(crate::presenter::Canned { reply: json!({}) }))
            .await
            .unwrap();
        let authority = "hub.example.ts.net:8443";
        assert!(LoopbackBrowserListener::bind("example.com".into(), 0).await.is_err());

        // Reject when peer verifier returns false
        let listener = LoopbackBrowserListener::bind(authority.into(), 0).await.unwrap();
        let port = listener.port();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(listener.run_with(hub.clone(), stop_rx, |_| false));
        let mut client = tokio::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await.unwrap();
        client
            .write_all(
                format!(
                    "GET /auth/web/info HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
                     X-Forwarded-Host: {authority}\r\nX-Forwarded-Proto: https\r\n\
                     Tailscale-User-Login: person@example.com\r\n\r\n"
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let mut buf = Vec::new();
        let _ = client.read_to_end(&mut buf).await;
        assert!(buf.is_empty(), "rejected peer must receive no HTTP response");
        let _ = stop_tx.send(());
        let kept = task.await.unwrap().unwrap();
        assert_eq!(kept.local_addr().unwrap().port(), port);

        // Admit when peer verifier returns true, using retained listener
        let listener = LoopbackBrowserListener::from_listener(kept, authority.into()).unwrap();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(listener.run_with(hub.clone(), stop_rx, |_| true));
        let mut client = tokio::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await.unwrap();
        client
            .write_all(
                format!(
                    "GET /auth/web/info HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
                     X-Forwarded-Host: {authority}\r\nX-Forwarded-Proto: https\r\n\
                     Tailscale-User-Login: person@example.com\r\n\r\n"
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let mut buf = Vec::new();
        client.read_to_end(&mut buf).await.unwrap();
        let text = String::from_utf8_lossy(&buf);
        assert!(text.starts_with("HTTP/1.1 200 OK"), "{text}");
        let _ = stop_tx.send(());
        let _kept = task.await.unwrap().unwrap();
    }
}

#[cfg(target_os = "linux")]
#[path = "webtransport/linux.rs"]
pub(crate) mod linux;
