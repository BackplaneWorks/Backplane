//! Private backend for a separately configured HTTPS Serve proxy.
//! Provisioning supplies the authority and authenticated daemon identity.
//! The release startup path does not construct this listener.
use std::{io, os::unix::fs::MetadataExt, path::Path, sync::Arc};
use tokio::net::{UnixListener, UnixStream};

pub(crate) struct ProxyIdentity {
    pub uid: u32,
    pub pid: i32,
}

impl ProxyIdentity {
    fn accepts(&self, stream: &UnixStream) -> bool {
        self.pid > 0 && stream.peer_cred().is_ok_and(|peer| {
            peer.uid() == self.uid && peer.pid() == Some(self.pid)
        })
    }
}

pub(crate) struct BrowserListener {
    listener: UnixListener,
    authority: String,
    proxy: ProxyIdentity,
}

impl BrowserListener {
    /// Caller owns the private runtime directory and its socket lifecycle.
    /// An occupied path fails the bind, including after an unclean shutdown.
    pub fn bind(directory: &Path, authority: String, proxy: ProxyIdentity) -> io::Result<Self> {
        let meta = std::fs::symlink_metadata(directory)?;
        if !directory.is_absolute() || !meta.is_dir() || meta.mode() & 0o777 != 0o700
            || meta.uid() != unsafe { libc::geteuid() } {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "private browser runtime directory required"));
        }
        let (host, port) = authority.split_once(':').map_or((authority.as_str(), None), |(h,p)| (h, Some(p)));
        if proxy.pid <= 0 || !host.ends_with(".ts.net") || host.split('.').count() < 4 || host.len() > 253
            || host.split('.').any(|label| label.is_empty() || label.len() > 63 || label.starts_with('-')
                || label.ends_with('-') || !label.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'))
            || port.is_some_and(|p| p.parse::<u16>().map_or(true, |n| matches!(n, 0 | 443) || n.to_string() != p)) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid browser proxy configuration"));
        }
        let listener = UnixListener::bind(directory.join("browser.sock"))?;
        Ok(Self { listener, authority, proxy })
    }

    pub async fn run(self, hub: Arc<crate::hub::Hub>) -> io::Result<()> {
        loop {
            let stream = match self.listener.accept().await {
                Ok((stream, _)) => stream,
                Err(e) if matches!(e.kind(), io::ErrorKind::ConnectionAborted | io::ErrorKind::Interrupted) => continue,
                Err(e) if matches!(e.raw_os_error(), Some(libc::EMFILE | libc::ENFILE)) => {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
                Err(e) => return Err(e),
            };
            if self.proxy.accepts(&stream) {
                let hub = hub.clone();
                let authority = self.authority.clone();
                tokio::spawn(async move { let _ = super::webingress::serve(hub, stream, &authority).await; });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bind_requires_a_private_directory_and_canonical_authority() {
        use std::os::unix::fs::{symlink, DirBuilderExt, PermissionsExt};
        let dir = std::env::temp_dir().join(format!("bp-web-bind-{}", std::process::id()));
        std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let bind = |path: &Path, host: &str, pid| BrowserListener::bind(path, host.into(),
            ProxyIdentity { uid: unsafe { libc::geteuid() }, pid });
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
        std::fs::remove_dir(dir).unwrap();
    }

    #[tokio::test]
    async fn browser_transport_checks_peer_process_as_well_as_user() {
        let (stream, _peer) = UnixStream::pair().unwrap();
        let identity = ProxyIdentity { uid: unsafe { libc::geteuid() }, pid: std::process::id() as i32 };
        assert!(identity.accepts(&stream));
        let other_process = ProxyIdentity { pid: identity.pid + 1, ..identity };
        assert!(!other_process.accepts(&stream));
    }
}

#[cfg(target_os = "linux")]
#[path = "webtransport/linux.rs"]
pub(crate) mod linux;
