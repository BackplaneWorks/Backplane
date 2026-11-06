//! TLS and plain HTTP. rustls with the Mozilla roots, for the socket here and
//! inside ureq for REST; a blocking client, used from a blocking thread.
//! ureq takes HTTP(S)_PROXY and NO_PROXY from the environment by itself.

use std::sync::Arc;
use std::time::Duration;

pub fn tls_config() -> Arc<rustls::ClientConfig> {
    let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let c = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("the ring provider has TLS versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
    Arc::new(c)
}

/// An agent that hands back every status, for the caller to read.
pub fn agent(limit: Option<Duration>) -> ureq::Agent {
    ureq::Agent::config_builder().http_status_as_error(false).timeout_global(limit).build().new_agent()
}

/// A failed request's first line.
pub fn error_msg(e: &ureq::Error) -> String {
    e.to_string().lines().next().unwrap_or("network error").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tls_config_builds() {
        assert!(tls_config().alpn_protocols.is_empty());
    }
}
