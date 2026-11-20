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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
