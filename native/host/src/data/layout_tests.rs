//! data/1 docking layout: the device store, its isolation from desktop sessions, the ops, the safety rules and the shapes.
use super::*;
use crate::present::layout::Refusal;

struct Fx { dir: crate::plat::tmp::TempDir, root: String, st: crate::state::State }
// a Files tile draws its folder, which a Files page registers for the hub's disk watch: leave nothing behind for another test's poll
impl Drop for Fx { fn drop(&mut self) { crate::files::watch_dir::forget_root(&self.root) } }

fn fx() -> Fx {
    let d = crate::plat::tmp::dir("data-layout");
    let root = d.to_string_lossy().into_owned();
    std::fs::write(d.join("unit.kicad_pcb"), "(kicad_pcb)").unwrap();
    std::fs::write(d.join("unit.kicad_sch"), "(kicad_sch)").unwrap();
    std::fs::create_dir_all(d.join("mfg/rev1")).unwrap();
    std::fs::write(d.join("mfg/rev1/unit-F_Cu.gbr"), "%FSLAX46Y46*%M02*").unwrap();
    std::fs::create_dir_all(d.join("mech")).unwrap();
    std::fs::write(d.join("mech/case.step"), "ISO-10303-21;").unwrap();
    let mut st = crate::state::State::new();
    st.apply_line(&json!({"$":"ProjectCreated","id":"p","title":"Board","root":root,"at":1}).to_string());
    st.apply_line(&json!({"$":"ThreadCreated","id":"t","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}).to_string());
    st.apply_line(&json!({"$":"ThreadCreated","id":"u","project":"p","title":"U","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":3}).to_string());
    Fx { dir: d, root, st }
}

const PHONE: (f32, f32) = (390.0, 844.0);
const TABLET: (f32, f32) = (820.0, 1180.0);
const DESKTOP: (f32, f32) = (1400.0, 900.0);

fn act(f: &Fx, store: &Store, device: &str, thread: &str, op: &str, p: Value, bounds: (f32, f32)) -> Result<Value, Refused> {
    let mut p = p;
    p["op"] = json!(op);
    super::act(&f.st, store, device, thread, op, &p, &f.root, "en", bounds)
}
fn tiles(r: &Value) -> Vec<String> { r["tiles"].as_array().unwrap().iter().map(|t| t["kind"].as_str().unwrap().to_string()).collect() }

/// Every string field of a reply that is not a plot key must hold no absolute path and not the project's folder.
fn no_absolute_paths(v: &Value, root: &str) {
    match v {
        Value::String(t) => { assert!(!t.starts_with('/') && !t.contains(root) && !t.contains(":\\"), "an absolute path in a reply: {t}") }
        Value::Array(a) => a.iter().for_each(|x| no_absolute_paths(x, root)),
        Value::Object(m) => m.values().for_each(|x| no_absolute_paths(x, root)),
        _ => {}
    }
}

#[test]
fn one_page_is_one_tile_and_an_add_is_two_tiles_fitted_to_the_device() {
    let f = fx();
    let store = Store::new();
    let r = read(&f.st, &store, "dev", "t", DESKTOP, "board", false, &f.root).unwrap();
    assert_eq!((r["mode"].as_str(), tiles(&r), r["rev"].as_u64()), (Some("full"), vec!["board".to_string()], Some(1)));
    let r = act(&f, &store, "dev", "t", "add", json!({"kind":"gerbers","side":"right","page":"board"}), DESKTOP).unwrap();
    assert_eq!(tiles(&r), vec!["board", "gerbers"]);
    assert_eq!((r["mode"].as_str(), r["changed"].as_bool(), r["rev"].as_u64()), (Some("full"), Some(true), Some(2)));
    assert_eq!(r["splits"].as_array().unwrap().len(), 1);
    assert_eq!(r["axis"], "row");
    no_absolute_paths(&r, &f.root);
    // the same tree, fitted to a tablet and a phone by each connection's own bounds
    let tab = read(&f.st, &store, "dev", "t", TABLET, "", false, &f.root).unwrap();
    assert_eq!((tab["mode"].as_str(), tab["axis"].as_str(), tab["rev"].as_u64()), (Some("twoUp"), Some("col"), Some(2)));
    assert_eq!(tab["tiles"].as_array().unwrap().iter().filter(|t| t["visible"] == true).count(), 2);
    let ph = read(&f.st, &store, "dev", "t", PHONE, "", false, &f.root).unwrap();
    assert_eq!(ph["mode"], "stack");
    assert_eq!(ph["tiles"].as_array().unwrap().iter().filter(|t| t["visible"] == true).count(), 1);
    assert_eq!(ph["order"].as_array().unwrap().len(), 2);
    assert!(ph["splits"].as_array().unwrap().is_empty());
}

#[test]
fn focus_swap_resize_maximize_restore_move_overlay_close_round_trip() {
    let f = fx();
    let store = Store::new();
    act(&f, &store, "d", "t", "add", json!({"kind":"gerbers","page":"board"}), DESKTOP).unwrap();
    let r = act(&f, &store, "d", "t", "add", json!({"kind":"mech","side":"bottom","target":"t2"}), DESKTOP).unwrap();
    assert_eq!(tiles(&r).len(), 3);
    let r = act(&f, &store, "d", "t", "focus", json!({"tile":"t3"}), PHONE).unwrap();
    assert_eq!((r["focus"].as_str(), r["order"][0].as_str()), (Some("t3"), Some("t3")));
    let r = act(&f, &store, "d", "t", "swap", json!({"a":"t1","b":"t2"}), DESKTOP).unwrap();
    assert_eq!(tiles(&r)[0], "gerbers");
    let r = act(&f, &store, "d", "t", "resize", json!({"split":"s1","ratio":0.6}), DESKTOP).unwrap();
    assert!((r["splits"][0]["ratio"].as_f64().unwrap() - 0.6).abs() < 1e-4);
    let r = act(&f, &store, "d", "t", "maximize", json!({"tile":"t1"}), DESKTOP).unwrap();
    assert_eq!(r["maximized"], "t1");
    assert_eq!(r["tiles"].as_array().unwrap().iter().filter(|t| t["visible"] == true).count(), 1);
    let r = act(&f, &store, "d", "t", "restore", json!({}), DESKTOP).unwrap();
    assert!(r["maximized"].is_null());
    let r = act(&f, &store, "d", "t", "move", json!({"tile":"t3","target":"t1","side":"left"}), DESKTOP).unwrap();
    assert_eq!(tiles(&r).len(), 3);
    let r = act(&f, &store, "d", "t", "close", json!({"tile":"t3"}), DESKTOP).unwrap();
    assert_eq!(tiles(&r).len(), 2);
    // an overlay: another tile merged over the base (Board with Gerbers), then split out again
    let ids: Vec<String> = r["tiles"].as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap().to_string()).collect();
    let board = r["tiles"].as_array().unwrap().iter().find(|t| t["kind"] == "board").unwrap()["id"].as_str().unwrap().to_string();
    let gerbers = ids.iter().find(|i| **i != board).unwrap().clone();
    let r = act(&f, &store, "d", "t", "overlay", json!({"tile":board,"overlay":gerbers,"mode":"diff"}), DESKTOP).unwrap();
    assert_eq!(tiles(&r), vec!["board"]);
    assert_eq!(r["tiles"][0]["overlay"]["kind"], "gerbers");
    no_absolute_paths(&r, &f.root);
    let r = act(&f, &store, "d", "t", "unsplit", json!({"tile":board}), DESKTOP).unwrap();
    assert_eq!(tiles(&r).len(), 2);
    // a tile that is not there, a split that is not there, and a repeated op that changes nothing
    assert_eq!(act(&f, &store, "d", "t", "close", json!({"tile":"nope"}), DESKTOP).unwrap_err().code, "notFound");
    let again = act(&f, &store, "d", "t", "focus", json!({"tile": r["focus"].as_str().unwrap()}), DESKTOP).unwrap();
    assert_eq!(again["changed"], false);
    assert_eq!(again["rev"], store.entry("d", "t").unwrap().rev);
}

#[test]
fn the_device_store_never_reads_or_writes_a_desktop_sessions_drafts() {
    let f = fx();
    let store = Store::new();
    // a desktop session on the same thread, with its own layout, page and a draft this module does not own
    let desktop = json!({"sel":"t","rid":7,"drafts":{"@view:t":"schematic","@view.at":format!("1|{}|", f.root),"@view.layout:t":"{\"version\":1,\"root\":null}","@composer:t":"secret words","other":"x"},"info":{"k":"v"},"panel":false});
    let before = desktop.clone();
    // 1. the synthetic session holds this device's own drafts only
    let s0 = session_for("t", None, "board", false);
    assert_eq!(s0["drafts"].as_object().unwrap().keys().cloned().collect::<Vec<_>>(), vec!["@view:t".to_string()]);
    assert!(s0["info"].as_object().unwrap().is_empty());
    let polluted = Entry { drafts: before["drafts"].as_object().unwrap().clone(), ..Default::default() };
    let s1 = session_for("t", Some(&polluted), "", false);
    let keys: Vec<_> = s1["drafts"].as_object().unwrap().keys().cloned().collect();
    assert!(keys.iter().all(|k| owned_key(k, "t")), "only the keys this module owns reach the hub's layout code: {keys:?}");
    assert!(!keys.contains(&"@composer:t".to_string()) && !keys.contains(&"other".to_string()) && !keys.contains(&"@view.at".to_string()));
    // 2. ops on the device's layout leave the desktop's session untouched, and what the store keeps is only its own keys
    act(&f, &store, "phone", "t", "add", json!({"kind":"gerbers","page":"board"}), DESKTOP).unwrap();
    act(&f, &store, "phone", "t", "add", json!({"kind":"mech"}), DESKTOP).unwrap();
    assert_eq!(desktop, before, "a device's ops never write a presenter session");
    let e = store.entry("phone", "t").unwrap();
    assert!(e.drafts.keys().all(|k| owned_key(k, "t")), "{:?}", e.drafts.keys().collect::<Vec<_>>());
    // 3. the desktop's own derived layout is not the device's: the device sees its layout, the desktop session still its schematic
    let dv = viewer::viewer_json(&f.st, &json!({}), &desktop).unwrap();
    assert_eq!(dv["layout"]["root"]["kind"], "schematic");
    assert_eq!(read(&f.st, &store, "phone", "t", DESKTOP, "", false, &f.root).unwrap()["tiles"].as_array().unwrap().len(), 3);
    // 4. another thread, another device: independent
    assert_eq!(read(&f.st, &store, "phone", "u", DESKTOP, "board", false, &f.root).unwrap()["tiles"].as_array().unwrap().len(), 1);
    assert_eq!(read(&f.st, &store, "tablet", "t", DESKTOP, "schematic", false, &f.root).unwrap()["tiles"].as_array().unwrap().len(), 1);
    // 5. a patch that tries to carry other drafts back is not stored
    let mut s = session_for("t", Some(&e), "", false);
    crate::session::patch::merge(&mut s, &json!({"drafts":{"@composer:t":"x","@view:other":"board"}}));
    assert!(s["drafts"].as_object().unwrap().contains_key("@composer:t"), "the merge itself is not the filter");
}

#[test]
fn two_connections_of_one_device_share_the_tree_and_are_told_of_a_change() {
    let f = fx();
    let store = Store::new();
    store.register_client(1, "dev");
    store.register_client(2, "dev");
    store.register_client(3, "other");
    store.watch("dev", "t", 1);
    store.watch("dev", "t", 2);
    store.watch("other", "t", 3);
    let r = act(&f, &store, "dev", "t", "add", json!({"kind":"gerbers","page":"board"}), PHONE).unwrap();
    let p = pushes(&store, "dev", "t", r["rev"].as_u64().unwrap());
    assert_eq!(p.iter().map(|(c, _)| *c).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(p[0].1, json!({"t":"layout","thread":"t","rev":2}));
    // each connection reads its own fit of the one tree; a client holding rev 1 sees rev 2 and asks again
    let a = read(&f.st, &store, "dev", "t", PHONE, "", false, &f.root).unwrap();
    let b = read(&f.st, &store, "dev", "t", DESKTOP, "", false, &f.root).unwrap();
    assert_eq!((a["rev"].clone(), b["rev"].clone()), (json!(2), json!(2)));
    assert_eq!((a["mode"].as_str(), b["mode"].as_str()), (Some("stack"), Some("full")));
    assert_eq!(read(&f.st, &store, "other", "t", PHONE, "board", false, &f.root).unwrap()["rev"], 1);
    store.forget_client(2);
    assert_eq!(store.watchers_of("dev", "t"), vec![1]);
    assert_eq!(store.device_of(1).as_deref(), Some("dev"));
    store.forget_client(1);
    assert!(store.device_of(1).is_none());
}

#[test]
fn multi_tile_layouts_survive_a_restart_and_go_with_a_revoked_or_expired_device() {
    let f = fx();
    let file = f.dir.join("viewer-layouts.json");
    let store = Store::open(Some(file.clone()));
    act(&f, &store, "d1", "t", "add", json!({"kind":"gerbers","page":"board"}), DESKTOP).unwrap();
    act(&f, &store, "d2", "t", "add", json!({"kind":"mech","source":"mech/case.step","page":"board"}), DESKTOP).unwrap();
    // a layout that is one tile again is the page: nothing of it stays on disk
    act(&f, &store, "d2", "t", "close", json!({"tile":"t2"}), DESKTOP).unwrap();
    store.flush();
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("d1") && !text.contains("d2"), "{text}");
    assert!(!text.contains(&f.root), "no absolute path on disk either: {text}");
    assert!(!text.contains("@view.file") && text.contains("@view.layout:t"), "only the layout and the picked board are kept: {text}");
    // a hub restart: the same tree, the same rev, the same recency
    let back = Store::open(Some(file.clone()));
    let (a, b) = (store.entry("d1", "t").unwrap(), back.entry("d1", "t").unwrap());
    assert_eq!((a.drafts.get("@view.layout:t").cloned(), a.recent, a.rev), (b.drafts.get("@view.layout:t").cloned(), b.recent, b.rev));
    assert_eq!(read(&f.st, &back, "d1", "t", DESKTOP, "", false, &f.root).unwrap()["tiles"].as_array().unwrap().len(), 2);
    assert!(back.entry("d2", "t").is_none());
    // revoked: its entries and its file entry go; the hub token's stay; expired devices go at the next prune
    back.put("hub", "t", Entry { drafts: store.entry("d1", "t").unwrap().drafts, rev: 1, ..Default::default() });
    act(&f, &back, "d3", "t", "add", json!({"kind":"mech","source":"mech/case.step","page":"board"}), DESKTOP).unwrap();
    back.forget_device("d1");
    assert!(!std::fs::read_to_string(&file).unwrap().contains("d1"));
    back.flush();
    assert!(!std::fs::read_to_string(&file).unwrap().contains("hub"), "the hub token's layout is in memory only");
    back.prune_to(&["d3".to_string()], 1_000_000);
    assert!(back.entry("d3", "t").is_some() && back.entry("hub", "t").is_some());
    back.prune_to(&[], 2_000_000);
    assert!(back.entry("d3", "t").is_none() && back.entry("hub", "t").is_some(), "an unknown device is dropped, the hub token never");
    // a file that is not ours, or holds a single tile, is not read as a layout
    std::fs::write(&file, r#"{"devices":{"x":{"t":{"drafts":{"@view:t":"board","@composer:t":"no"},"recent":[],"rev":3}}}}"#).unwrap();
    assert_eq!(Store::open(Some(file)).len(), 0);
}

#[test]
fn the_store_is_bounded() {
    let store = Store::new();
    let two = |t: &str| { let mut e = Entry::default(); e.drafts.insert(format!("@view.layout:{t}"), json!("x")); e.rev = 1; e };
    for i in 0..(THREADS_PER_DEVICE + 5) { store.put("d", &format!("t{i}"), two(&format!("t{i}"))); }
    assert!(store.entry("d", "t0").is_none() && store.entry("d", &format!("t{}", THREADS_PER_DEVICE + 4)).is_some());
    assert_eq!(store.len(), THREADS_PER_DEVICE);
    for i in 0..(DEVICES + 3) { store.put(&format!("dev{i}"), "t", two("t")); }
    let devices: std::collections::BTreeSet<String> = store.entries.lock().unwrap().keys().map(|(d, _)| d.clone()).collect();
    assert!(devices.len() <= DEVICES);
}

#[test]
fn sources_resolve_inside_the_project_or_are_refused() {
    let f = fx();
    let store = Store::new();
    for bad in ["../x", "a/../../x", "/etc/passwd", "C:\\x", "mfg\\rev1", "mfg/rev1\n", "mfg/rev1\0", "does/not/exist", "mfg/rev1/../../../x"] {
        let e = act(&f, &store, "d", "t", "add", json!({"kind":"gerbers","source":bad,"page":"board"}), DESKTOP).unwrap_err();
        assert_eq!(e.code, "invalidSource", "{bad:?}");
        let e = act(&f, &store, "d", "t", "overlay", json!({"tile":"t1","overlay":{"kind":"gerbers","source":bad}}), DESKTOP).unwrap_err();
        assert!(matches!(e.code, "invalidSource" | "notFound"), "{bad:?} {}", e.code);
        let e = act(&f, &store, "d", "t", "show", json!({"kind":"gerbers","source":bad,"page":"board"}), DESKTOP).unwrap_err();
        assert_eq!(e.code, "invalidSource", "{bad:?}");
    }
    // a symlink that leaves the project is a way out
    #[cfg(unix)]
    {
        let out = crate::plat::tmp::dir("data-layout-out");
        std::fs::write(out.join("f.gbr"), "x").unwrap();
        std::os::unix::fs::symlink(&*out, f.dir.join("escape")).unwrap();
        let e = act(&f, &store, "d", "t", "add", json!({"kind":"gerbers","source":"escape/f.gbr","page":"board"}), DESKTOP).unwrap_err();
        assert_eq!(e.code, "invalidSource");
    }
    // a file or folder inside the project is fine
    let ok = act(&f, &store, "d", "t", "add", json!({"kind":"gerbers","source":"mfg/rev1","page":"board"}), DESKTOP).unwrap();
    assert!(ok["tiles"].as_array().unwrap().iter().any(|t| t["source"] == "mfg/rev1"));
    no_absolute_paths(&ok, &f.root);
    // nothing was stored by a refusal
    assert_eq!(store.len(), 1);
}

#[test]
fn only_viewer_tiles_the_limit_linked_threads_and_malformed_requests_are_refused() {
    let f = fx();
    let store = Store::new();
    for kind in ["simulation", "browser", "terminal", "kicad", "zzz", ""] {
        let e = act(&f, &store, "d", "t", "add", json!({"kind":kind,"page":"board"}), DESKTOP).unwrap_err();
        assert!(matches!(e.code, "notOnDevice" | "invalidInput"), "{kind:?} {}", e.code);
        assert!(act(&f, &store, "d", "t", "show", json!({"kind":kind,"page":"board"}), DESKTOP).is_err(), "{kind:?}");
    }
    for ok in ["board", "schematic", "3d", "gerbers", "mech", "files", "diff", "review"] { assert!(TILE_KINDS.contains(&ok)); }
    // a linked machine's thread, an unknown thread, an unknown op
    // a linked machine's thread shows ONE derived tile and stores nothing; every act on it is refused
    let l = read(&f.st, &store, "d", "L~t1", DESKTOP, "board", false, &f.root).unwrap();
    assert!(l["tiles"].as_array().unwrap().len() <= 1 && l["rev"] == 1, "{l}");
    assert_eq!(store.len(), 0);
    for op in ["add", "close", "focus", "maximize", "restore", "resize", "swap", "move", "overlay", "unsplit", "show"] {
        assert_eq!(act(&f, &store, "d", "L~t1", op, json!({"kind":"mech","tile":"t1","page":"board"}), DESKTOP).unwrap_err().code, "linked", "{op}");
    }
    assert_eq!(act(&f, &store, "d", "L~t1", "add", json!({"kind":"mech"}), DESKTOP).unwrap_err().code, "linked");
    assert_eq!(read(&f.st, &store, "d", "nope", DESKTOP, "board", false, &f.root).unwrap_err().code, "notFound");
    assert_eq!(act(&f, &store, "d", "t", "explode", json!({}), DESKTOP).unwrap_err().code, "invalidInput");
    assert_eq!(act(&f, &store, "d", "t", "close", json!({}), DESKTOP).unwrap_err().code, "invalidInput");
    assert_eq!(act(&f, &store, "d", "t", "close", json!({"tile": 5}), DESKTOP).unwrap_err().code, "invalidInput");
    // the ninth tile
    act(&f, &store, "d", "t", "add", json!({"kind":"board","page":"board"}), DESKTOP).unwrap();
    for i in 0..(MAX_TILES - 2) { act(&f, &store, "d", "t", "add", json!({"kind":"files","side":if i % 2 == 0 { "bottom" } else { "right" }}), DESKTOP).unwrap_or_else(|e| panic!("{i}: {e:?}")); }
    let e = act(&f, &store, "d", "t", "add", json!({"kind":"mech"}), DESKTOP).unwrap_err();
    assert_eq!(e.code, "tooManyTiles");
    assert_eq!(read(&f.st, &store, "d", "t", DESKTOP, "", false, &f.root).unwrap()["tiles"].as_array().unwrap().len(), MAX_TILES);
}

#[test]
fn bounds_are_clamped_and_a_malformed_one_is_refused_before_any_fit() {
    for (b, ok) in [(json!([0, 0]), true), (json!([-5, 1e30]), true), (json!([390.5, 844]), true), (json!(null), false), (json!([1]), false), (json!([1, 2, 3]), false), (json!(["1", 2]), false), (json!({"w":1,"h":2}), false)] {
        assert_eq!(parse_bounds(&b).is_ok(), ok, "{b}");
    }
    let (w, h) = parse_bounds(&json!([-5, 1e30])).unwrap();
    assert!(w >= layout_fit::MIN_BOUND_PX && h <= layout_fit::MAX_BOUND_PX && w.is_finite() && h.is_finite());
    let f = fx();
    let store = Store::new();
    // a request with absurd bounds still answers, with finite rectangles
    let r = act(&f, &store, "d", "t", "add", json!({"kind":"gerbers","page":"board"}), parse_bounds(&json!([0, -1])).unwrap()).unwrap();
    assert_eq!(r["mode"], "stack");
    assert!(r["tiles"].as_array().unwrap().iter().all(|t| t["rect"].is_null() || t["rect"]["w"].as_f64().unwrap().is_finite()));
    // a ratio that is not a number is refused, never fed to the fit
    for ratio in [json!(null), json!("0.5"), json!(f64::NAN)] {
        let e = act(&f, &store, "d", "t", "resize", json!({"split":"s1","ratio":ratio}), DESKTOP).unwrap_err();
        assert_eq!(e.code, "invalidRatio");
    }
}

#[test]
fn a_data_client_cannot_hold_kicad_or_name_a_lock() {
    let f = fx();
    let store = Store::new();
    act(&f, &store, "d", "t", "add", json!({"kind":"gerbers","page":"board"}), DESKTOP).unwrap();
    // `locked` and `kicadTile` are the desktop's guard fields: whatever a client sends in them changes nothing
    let r = act(&f, &store, "d", "t", "close", json!({"tile":"t2","locked":"t2","kicadTile":"t2"}), DESKTOP).unwrap();
    assert_eq!(tiles(&r), vec!["board"]);
    let r = act(&f, &store, "d", "t", "unsplit", json!({"locked":"t1"}), DESKTOP);
    assert!(r.is_err() || r.is_ok());
    for code in ["oneKicad", "locked"] {
        let e = act(&f, &store, "d", "t", "add", json!({"kind":"simulation"}), DESKTOP).unwrap_err();
        assert_ne!(e.code, code, "Simulation is not a data/1 tile at all");
    }
}

#[test]
fn replies_hold_no_absolute_path_and_refusals_are_code_key_message() {
    let f = fx();
    let store = Store::new();
    let r = act(&f, &store, "d", "t", "add", json!({"kind":"gerbers","page":"board"}), TABLET).unwrap();
    no_absolute_paths(&r, &f.root);
    for k in ["thread", "rev", "mode", "axis", "bounds", "focus", "maximized", "order", "tiles", "splits", "limits", "changed"] { assert!(r.get(k).is_some(), "{k}"); }
    for t in r["tiles"].as_array().unwrap() { for k in ["id", "kind", "source", "explicit", "focused", "visible", "rect"] { assert!(t.get(k).is_some(), "{k}"); } }
    let e = act(&f, &store, "d", "t", "close", json!({"tile":"zz"}), TABLET).unwrap_err().to_value();
    assert_eq!((e["code"].as_str(), e["key"].as_str()), (Some("notFound"), Some("viewer.layout.refusal.notfound")));
    assert!(e["message"].as_str().is_some_and(|m| !m.is_empty() && m != "notFound"), "the message is the catalog's words: {e}");
    // the catalog's words come in the hub's language
    let ja = Refused::of_for(Refusal::NotFound, "ja");
    assert_ne!(ja.text, Refused::of(Refusal::NotFound).text);
}

#[test]
fn device_keys_are_paired_browser_or_hub() {
    assert_eq!(device_key(Some("d1".into()), None), "d1");
    assert_eq!(device_key(None, Some("b9")), "browser:b9");
    assert_eq!(device_key(None, None), "hub");
    assert_eq!(device_key(Some("d1".into()), Some("b9")), "browser:b9");
}

// ---- review round: passive reads, the source-aware show, the file, the watch set, the boundary

#[test]
fn a_read_is_passive_and_a_page_outside_the_protocol_is_refused() {
    let f = fx();
    let store = Store::new();
    let r = read(&f.st, &store, "d", "t", PHONE, "board", false, &f.root).unwrap();
    assert_eq!((r["rev"].clone(), store.len()), (json!(1), 0), "a read stores nothing");
    // a page this protocol cannot hold never seeds a layout, on a read or on an act, and changes no rev
    for bad in ["browser", "terminal", "simulation", "kicad", "zzz"] {
        assert_eq!(read(&f.st, &store, "d", "t", PHONE, bad, false, &f.root).unwrap_err().code, "notOnDevice", "{bad}");
        assert_eq!(act(&f, &store, "d", "t", "add", json!({"kind":"gerbers","page":bad}), PHONE).unwrap_err().code, "notOnDevice", "{bad}");
    }
    assert_eq!(store.len(), 0);
    // Diff takes precedence over the page it was opened from
    let d = read(&f.st, &store, "d", "t", PHONE, "board", true, &f.root).unwrap();
    assert_eq!(tiles(&d), vec!["diff"], "{d}");
    // `diff` alone is enough: a fresh device with no `page` reads Diff, on a read and for the watch's tree; a stored layout of several tiles is not replaced by it
    let d = read(&f.st, &store, "d", "t", PHONE, "", true, &f.root).unwrap();
    assert_eq!(tiles(&d), vec!["diff"], "diff with no page is the Diff tile, not idle: {d}");
    assert_eq!(read(&f.st, &store, "d", "t", PHONE, "", false, &f.root).unwrap()["tiles"].as_array().unwrap().len(), 0, "no page, no diff: idle");
    assert!(watch_set_sources(&f.st, &store, "d", "t", PHONE, &f.root, "", true, false).unwrap().is_empty(), "Diff draws no plot: an empty set, from the Diff tree");
    assert_eq!(store.len(), 0, "still passive");
    // reading again, with another page, with a layout of several tiles held: the tree and the rev are the device's, not the page's
    act(&f, &store, "d", "t", "add", json!({"kind":"gerbers","page":"board"}), DESKTOP).unwrap();
    let before = store.entry("d", "t").unwrap();
    let r = read(&f.st, &store, "d", "t", DESKTOP, "mech", false, &f.root).unwrap();
    assert_eq!((r["rev"].clone(), r["tiles"].as_array().unwrap().len()), (json!(2), 2));
    assert_eq!(store.entry("d", "t").unwrap(), before, "a passive read changes nothing that is stored");
}

#[test]
fn show_applies_its_source_and_never_closes() {
    let f = fx();
    let store = Store::new();
    act(&f, &store, "d", "t", "add", json!({"kind":"gerbers","page":"board"}), DESKTOP).unwrap();
    let a = act(&f, &store, "d", "t", "show", json!({"kind":"gerbers","source":"mfg/rev1"}), DESKTOP).unwrap();
    assert!(a["tiles"].as_array().unwrap().iter().any(|t| t["kind"] == "gerbers" && t["source"] == "mfg/rev1"), "the named source is applied: {a}");
    // a repeat keeps it (the rail's toggle would close it)
    let b = act(&f, &store, "d", "t", "show", json!({"kind":"gerbers","source":"mfg/rev1"}), DESKTOP).unwrap();
    assert!(b["tiles"].as_array().unwrap().iter().any(|t| t["kind"] == "gerbers" && t["source"] == "mfg/rev1"), "a repeat never closes: {b}");
    assert_eq!(b["tiles"].as_array().unwrap().len(), a["tiles"].as_array().unwrap().len());
    // a named board is the board shown, not the default
    let c = act(&f, &store, "d", "t", "show", json!({"kind":"board","source":"unit.kicad_pcb"}), DESKTOP).unwrap();
    assert!(c["tiles"].as_array().unwrap().iter().any(|t| t["kind"] == "board"), "{c}");
    no_absolute_paths(&c, &f.root);
}

#[test]
fn the_file_is_private_ordered_bounded_and_a_corrupt_one_is_kept() {
    let f = fx();
    let file = f.dir.join("viewer-layouts.json");
    let store = Store::open(Some(file.clone()));
    act(&f, &store, "d1", "t", "add", json!({"kind":"gerbers","page":"board"}), DESKTOP).unwrap();
    // memory only until a write: nothing is on disk from `act`
    assert!(!file.exists());
    store.flush();
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600, "device ids and relative paths are private"); }
    // nothing changed: nothing is written
    std::fs::remove_file(&file).unwrap();
    store.flush();
    assert!(!file.exists(), "an unchanged store writes nothing");
    // a later change writes the LATEST tree; a retired device is gone from the file the moment it is forgotten
    act(&f, &store, "d1", "t", "add", json!({"kind":"schematic"}), DESKTOP).unwrap();
    store.flush();
    assert_eq!(Store::open(Some(file.clone())).entry("d1", "t").unwrap().rev, store.entry("d1", "t").unwrap().rev);
    store.forget_device("d1");
    assert!(!std::fs::read_to_string(&file).unwrap().contains("d1"));
    // the limits hold for what is read back
    let two = r#"{"drafts":{"@view.layout:T":"{}"},"recent":[],"rev":1}"#;
    let mut devices = serde_json::Map::new();
    for i in 0..(DEVICES + 5) {
        let mut threads = serde_json::Map::new();
        for j in 0..(THREADS_PER_DEVICE + 3) { let t = format!("t{j}"); threads.insert(t.clone(), serde_json::from_str::<Value>(&two.replace("T", &t)).unwrap()); }
        devices.insert(format!("dev{i}"), Value::Object(threads));
    }
    let bin = f.dir.join("big.json");
    std::fs::write(&bin, json!({"version":1,"devices":devices}).to_string()).unwrap();
    let loaded = Store::open(Some(bin));
    assert!(loaded.len() <= DEVICES * THREADS_PER_DEVICE);
    let per: std::collections::BTreeMap<String, usize> = loaded.entries.lock().unwrap().keys().fold(Default::default(), |mut m, (d, _)| { *m.entry(d.clone()).or_default() += 1; m });
    assert!(per.len() <= DEVICES && per.values().all(|n| *n <= THREADS_PER_DEVICE), "{per:?}");
    // a file that is not JSON is copied aside, never silently dropped
    let broken = f.dir.join("broken.json");
    std::fs::write(&broken, "{\"devices\": ").unwrap();
    assert_eq!(Store::open(Some(broken.clone())).len(), 0);
    assert_eq!(std::fs::read_to_string(f.dir.join("broken.json.bad")).unwrap(), "{\"devices\": ");
}

#[test]
fn one_store_per_home_and_a_push_goes_only_to_the_devices_own_connections() {
    let (a, b) = (crate::plat::tmp::dir("data-layout-home-a"), crate::plat::tmp::dir("data-layout-home-b"));
    assert!(std::ptr::eq(global(&a), global(&a)) && !std::ptr::eq(global(&a), global(&b)), "each hub home has its own store");
    let store = Store::new();
    store.register_client(1, "dev"); store.register_client(2, "dev"); store.register_client(3, "other"); store.register_client(4, "hub");
    for (c, d) in [(1, "dev"), (2, "dev"), (3, "other"), (4, "hub")] { store.watch(d, "t", c); }
    // a device's change reaches its own watching connections (the sender included) and no other device's, nor the hub token's
    assert_eq!(pushes(&store, "dev", "t", 5).iter().map(|(c, _)| *c).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(pushes(&store, "hub", "t", 5).iter().map(|(c, _)| *c).collect::<Vec<_>>(), vec![4]);
    assert!(pushes(&store, "dev", "other-thread", 5).is_empty());
}

#[test]
fn a_saved_source_is_resolved_again_at_every_use_and_a_changed_link_is_not_followed() {
    let f = fx();
    let store = Store::new();
    // canonical: a link inside the project is stored as where it really is
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(f.dir.join("mfg/rev1"), f.dir.join("lnk")).unwrap();
        assert_eq!(safe_source(&f.root, "lnk").unwrap(), "mfg/rev1");
        assert_eq!(safe_source(&f.root, "./mfg/../mfg/rev1").unwrap_err().code, "invalidSource");
        // a layout kept with the link's own name (an older file, or one edited by hand), the link then pointed OUT of the project
        act(&f, &store, "d", "t", "add", json!({"kind":"gerbers","source":"mfg/rev1","page":"board"}), DESKTOP).unwrap();
        let mut e = store.entry("d", "t").unwrap();
        let text = e.drafts.get("@view.layout:t").unwrap().as_str().unwrap().replace("mfg/rev1", "swap");
        e.drafts.insert("@view.layout:t".into(), json!(text));
        store.put("d", "t", e);
        let out = crate::plat::tmp::dir("data-layout-out2");
        std::fs::write(out.join("f.gbr"), "x").unwrap();
        std::os::unix::fs::symlink(&*out, f.dir.join("swap")).unwrap();
        let r = read(&f.st, &store, "d", "t", DESKTOP, "", false, &f.root).unwrap();
        assert!(r["tiles"].as_array().unwrap().iter().any(|t| t["unavailable"] == true), "the tile whose source left the project says so: {r}");
        let set = watch_set_sources(&f.st, &store, "d", "t", DESKTOP, &f.root, "", false, false).unwrap();
        assert!(set.iter().all(|s| !s["path"].as_str().unwrap_or("").contains("swap") && !s["path"].as_str().unwrap_or("").contains(out.to_str().unwrap())), "never watched: {set:?}");
    }
}

#[test]
fn the_watch_set_is_the_visible_plot_tiles_empty_and_single_included() {
    let f = fx();
    let store = Store::new();
    // a phone stack: Board visible, then Files focused: the visible set is EMPTY, and is still the answer
    act(&f, &store, "d", "t", "add", json!({"kind":"files","page":"board"}), PHONE).unwrap();
    let set = |b, page: &str| watch_set_sources(&f.st, &store, "d", "t", b, &f.root, page, false, false).unwrap();
    let focus = |tile: &str| act(&f, &store, "d", "t", "focus", json!({"tile": tile}), PHONE).unwrap();
    let r = read(&f.st, &store, "d", "t", PHONE, "", false, &f.root).unwrap();
    let (board, files) = (r["tiles"][0]["id"].as_str().unwrap().to_string(), r["tiles"][1]["id"].as_str().unwrap().to_string());
    focus(&board);
    assert_eq!(set(PHONE, "").len(), 1, "the Board tile is the stack's one visible tile");
    focus(&files);
    assert!(set(PHONE, "").is_empty(), "Files draws no plot: the set is empty and is still sent");
    assert_eq!(set(DESKTOP, "").len(), 1, "both visible on a desktop: the Board's source");
    // a device with ONE Board tile (no layout of several) watches that tile
    let fresh = Store::new();
    let one = watch_set_sources(&f.st, &fresh, "d", "t", PHONE, &f.root, "board", false, false).unwrap();
    assert_eq!(one.len(), 1, "{one:?}");
    assert!(watch_set_sources(&f.st, &fresh, "d", "t", PHONE, &f.root, "", false, false).unwrap().is_empty());
    assert_eq!(watch_set_sources(&f.st, &fresh, "d", "t", PHONE, &f.root, "browser", false, false).unwrap_err().code, "notOnDevice");
}
