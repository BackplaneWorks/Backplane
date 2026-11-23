//! Persisted favorite models setting helpers (`models.favorites`), shared between settings, data, sync, and standalone audit bins.
use serde_json::Value;

/// Persisted favorite models setting key (`["provider/model", ...]`), synced across machines and clients.
pub const FAVORITES_KEY: &str = "models.favorites";
pub const MAX_FAVORITES: usize = 64;
pub const MAX_FAV_LEN: usize = 200;
pub const MAX_FAVORITES_BYTES: usize = 4096;

/// Normalize a `(provider, model)` favorite pair. Rejects empty, whitespace/slash in provider, control characters, or overlong entries.
pub fn norm_fav_pair(provider: &str, model: &str) -> Option<(String, String)> {
    if provider.chars().any(char::is_control) || model.chars().any(char::is_control) {
        return None;
    }
    let p = provider.trim().trim_start_matches('@').to_ascii_lowercase();
    let m = model.trim();
    if p.is_empty() || m.is_empty() || p.contains('/') || p.chars().any(char::is_whitespace) {
        return None;
    }
    if p.len() + 1 + m.len() > MAX_FAV_LEN {
        return None;
    }
    Some((p, m.to_string()))
}

pub fn fav_key(provider: &str, model: &str) -> Option<String> {
    norm_fav_pair(provider, model).map(|(p, m)| format!("{p}/{m}"))
}

/// Parse `models.favorites` JSON string into ordered, deduplicated `(provider, model)` pairs.
pub fn parse_favorites(raw: &str) -> Vec<(String, String)> {
    let s = raw.trim();
    if s.is_empty() {
        return vec![];
    }
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(s) else {
        return vec![];
    };
    let mut out: Vec<(String, String)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for item in items {
        let pair = match item {
            Value::String(text) => text.split_once('/').and_then(|(p, m)| norm_fav_pair(p, m)),
            Value::Object(ref obj) => {
                let p = obj.get("provider").and_then(Value::as_str).unwrap_or("");
                let m = obj.get("model").and_then(Value::as_str).unwrap_or("");
                norm_fav_pair(p, m)
            }
            _ => None,
        };
        if let Some((p, m)) = pair {
            let key = format!("{p}/{m}");
            if seen.insert(key) {
                out.push((p, m));
                if out.len() >= MAX_FAVORITES {
                    break;
                }
            }
        }
    }
    out
}

pub fn parse_favorite_keys(raw: &str) -> Vec<String> {
    parse_favorites(raw).into_iter().map(|(p, m)| format!("{p}/{m}")).collect()
}

/// Format favorite `(provider, model)` pairs for `SettingSet`. Empty list produces `""` so the setting clears cleanly.
pub fn format_favorites(favs: &[(String, String)]) -> String {
    let mut norm: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (p, m) in favs {
        if let Some((np, nm)) = norm_fav_pair(p, m) {
            let k = format!("{np}/{nm}");
            if seen.insert(k.clone()) {
                norm.push(k);
                let encoded = serde_json::to_string(&norm).unwrap_or_default();
                if encoded.len() > MAX_FAVORITES_BYTES {
                    norm.pop();
                    break;
                }
                if norm.len() >= MAX_FAVORITES {
                    break;
                }
            }
        }
    }
    if norm.is_empty() {
        String::new()
    } else {
        serde_json::to_string(&norm).unwrap_or_default()
    }
}

/// Validate a `models.favorites` setting value before storing or syncing.
pub fn favorites_value_ok(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    if value.len() > MAX_FAVORITES_BYTES {
        return false;
    }
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(value) else {
        return false;
    };
    if items.len() > MAX_FAVORITES {
        return false;
    }
    items.iter().all(|item| {
        item.as_str()
            .and_then(|s| s.split_once('/'))
            .and_then(|(p, m)| norm_fav_pair(p, m))
            .is_some()
    })
}

/// Toggle or explicitly set `(provider, model)` in `raw`, returning `Ok((next_setting_value, now_starred, canonical_keys))` or an error reason.
pub fn toggle_favorite(raw: &str, provider: &str, model: &str, on: Option<bool>) -> Result<(String, bool, Vec<String>), &'static str> {
    let Some((np, nm)) = norm_fav_pair(provider, model) else {
        return Err("invalid provider or model");
    };
    let mut list = parse_favorites(raw);
    let idx = list.iter().position(|(p, m)| *p == np && *m == nm);
    let want = on.unwrap_or_else(|| idx.is_none());
    match (idx, want) {
        (Some(i), false) => {
            list.remove(i);
        }
        (None, true) => {
            if list.len() >= MAX_FAVORITES {
                return Err("too many favorite models (max 64)");
            }
            list.push((np, nm));
            let keys: Vec<String> = list.iter().map(|(p, m)| format!("{p}/{m}")).collect();
            let next = serde_json::to_string(&keys).unwrap_or_default();
            if !favorites_value_ok(&next) {
                return Err("favorite models list is full");
            }
            return Ok((next, true, keys));
        }
        _ => {}
    }
    let next = format_favorites(&list);
    let keys = parse_favorite_keys(&next);
    Ok((next, want, keys))
}
