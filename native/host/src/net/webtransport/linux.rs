use crate::plat::browser_peer::{accepts, host_namespaces};
use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

/// Binds only loopback. Provisioning must keep the listener alive for the entire
/// lifetime of its Serve mapping.
pub struct LoopbackListener {
    listener: tokio::net::TcpListener,
    authority: String,
}

impl LoopbackListener {
    pub async fn bind(authority: String) -> io::Result<Self> {
        super::validate_authority(&authority)?;
        if !host_namespaces() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "browser access requires host namespaces",
            ));
        }
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        Ok(Self {
            listener,
            authority,
        })
    }

    pub fn address(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    pub async fn run(self, hub: std::sync::Arc<crate::hub::Hub>) -> io::Result<()> {
        loop {
            let stream = match self.listener.accept().await {
                Ok((stream, _)) => stream,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::Interrupted | io::ErrorKind::ConnectionAborted
                    ) =>
                {
                    continue
                }
                Err(e) if matches!(e.raw_os_error(), Some(libc::EMFILE | libc::ENFILE)) => {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
                Err(e) => return Err(e),
            };
            // One outstanding lookup per listener bounds blocking work under local load.
            let lookup = tokio::task::spawn_blocking(move || accepts(&stream).then_some(stream));
            if let Ok(Ok(Some(stream))) =
                tokio::time::timeout(Duration::from_millis(300), lookup).await
            {
                let authority = self.authority.clone();
                let hub = hub.clone();
                tokio::spawn(async move {
                    let _ = super::super::webingress::serve(hub, stream, &authority).await;
                });
            }
        }
    }
}
