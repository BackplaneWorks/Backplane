//! Merging an action's `session_patch` into the request's session: one helper for every lane that answers an action
//! (composer::act, select::select, commands::actions::act). A patch is `{sel?, rid?, panel?, adding?, wantNew?, error?, note?, drafts?, info?, open?, outbox?}`: scalars are set, the four maps are
//! merged key by key (a key set to a value replaces it; the maps keep every key the patch does not name).
use serde_json::{Map, Value};

pub fn merge(session: &mut Value, patch: &Value) {
    let (Some(s), Some(p)) = (session.as_object_mut(), patch.as_object()) else { return };
    for (k, v) in p {
        match (k.as_str(), v) {
            ("drafts" | "info" | "open" | "outbox", Value::Object(m)) => {
                let slot = s.entry(k.clone()).or_insert_with(|| Value::Object(Map::new()));
                if !slot.is_object() { *slot = Value::Object(Map::new()) }
                let dst = slot.as_object_mut().unwrap();
                for (mk, mv) in m { dst.insert(mk.clone(), mv.clone()); }
            }
            _ => { s.insert(k.clone(), v.clone()); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test] fn scalars_set_and_maps_merge_key_by_key() {
        let mut s = json!({"sel": "", "rid": 1, "drafts": {"@boot": "", "t": "x"}, "info": {"a": "1"}});
        merge(&mut s, &json!({"sel": "t9", "rid": 2, "drafts": {"t": "", "@fab.at": "k"}, "info": {"panel:": ""}, "open": {"settings": false}, "outbox": {"2": "{}"}}));
        assert_eq!(s, json!({"sel": "t9", "rid": 2, "drafts": {"@boot": "", "t": "", "@fab.at": "k"}, "info": {"a": "1", "panel:": ""}, "open": {"settings": false}, "outbox": {"2": "{}"}}));
    }
    #[test] fn a_missing_or_odd_session_is_left_alone() {
        let mut s = json!(null); merge(&mut s, &json!({"sel": "x"})); assert_eq!(s, json!(null));
        let mut o = json!({"drafts": 5}); merge(&mut o, &json!({"drafts": {"a": "b"}})); assert_eq!(o, json!({"drafts": {"a": "b"}}));
    }
}
