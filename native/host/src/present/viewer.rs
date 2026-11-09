//! The viewer panel beside a thread, for the pages the GPUI desktop reads from `thread.viewer`: Board, Schematic and 3D,
//! with the plot itself riding its own channel (`kicad.watch`). Pure functions over the session and `State`. `None`: a case not ported (a project with several boards, a
//! version, a project home, a thread elsewhere). The Files tab is drawn here: the listing is read in-process (`files::dir`), not asked of the hub. Diff and Review are session/pages.rs's. Keys GPUI does not read (`hist`, `bg`, `look`...) keep the idle template.
use crate::{session::{pages, patch}, state::State};
use serde_json::{json, Value};

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn jstr(x: &str) -> String { serde_json::to_string(x).unwrap_or_else(|_| "\"\"".into()) }
fn drawing(kind: &str) -> bool { matches!(kind, "board" | "schematic" | "3d" | "split" | "mech") }

/// Mech's listing (`mech.list`, files/scripts/mech_list.sh): every STEP/STP/GLB under `<root>/mech`, four folders deep, sorted bytewise, at most 400, as paths from the root.
pub fn mech_parts(root: &str) -> Vec<String> {
    fn walk(base: &std::path::Path, rel: &str, depth: usize, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(base) else { return };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') { continue }
            let (r, ft) = (format!("{rel}/{name}"), e.file_type());
            match ft { Ok(t) if t.is_dir() && depth < 3 => walk(&e.path(), &r, depth + 1, out), Ok(t) if t.is_file() => { let l = name.to_ascii_lowercase(); if [".step", ".stp", ".glb"].iter().any(|x| l.ends_with(x)) { out.push(r.trim_start_matches('/').to_string()) } }, _ => {} }
        }
    }
    if root.is_empty() { return vec![] }
    let mut out = vec![]; walk(&std::path::Path::new(root).join("mech"), "mech", 0, &mut out);
    out.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes())); out.truncate(400); out
}
fn stem(p: &str) -> String { let f = p.rsplit('/').next().unwrap_or(p); f.rsplit_once('.').map_or(f, |x| x.0).to_string() }
/// Mech.is_asm / Mech.default: the part Mech opens on when none was asked for: an assembly (a name with "assembl" in it or ending "-asm"), else the first.
fn default_part(ps: &[String]) -> Option<&String> {
    ps.iter().find(|p| { let n = stem(p).to_ascii_lowercase(); n.contains("assembl") || n.ends_with("-asm") }).or_else(|| ps.first())
}
/// The part on show: the one chosen (`@mech.part:<thread>`) while the listing still has it, else the default.
pub fn mech_current(ps: &[String], chosen: &str) -> Option<String> { ps.iter().find(|p| p.as_str() == chosen).or_else(|| default_part(ps)).cloned() }
fn abs_of(root: &str, rel: &str) -> String { if crate::plat::fs::is_abs(rel) { rel.to_string() } else { format!("{root}/{rel}") } }
/// The source a Mech page asks the hub for: the part as a 3D model ("" with no part).
/// A project's parts: listed from its folder here; a project on another machine is not on this disk, so only the part an agent showed is known.
fn parts_of(root: &str, sel: &str, chosen: &str) -> Vec<String> { if sel.contains('~') { if chosen.is_empty() { vec![] } else { vec![chosen.to_string()] } } else { mech_parts(root) } }
fn mech_want(root: &str, ps: &[String], chosen: &str) -> (String, String) {
    match mech_current(ps, chosen) { Some(p) if !root.is_empty() => { let a = abs_of(root, &p); (format!("2|{root}|{a}"), a) }, _ => (String::new(), String::new()) }
}
/// a kind this panel can show: a drawing, or the Browser (the page rides the browser helper, not `kicad.watch`)
fn shows(kind: &str) -> bool { drawing(kind) || kind == "browser" || files_kind(kind) }
/// The Files tab and what it opened: the folder, or an image, a PDF or a text note
fn files_kind(kind: &str) -> bool { matches!(kind, "files" | "image" | "pdf" | "text") }

/// Ui.root: the thread's worktree, else its project's folder
fn root_of(st: &State, sel: &str) -> String {
    let Some(t) = st.threads.get(sel) else { return String::new() };
    if !t.worktree.is_empty() { return t.worktree.clone() }
    st.projects.get(&t.project).map(|p| p.root.clone()).unwrap_or_default()
}
/// View.key: the source a plot carries ("" with no root or no drawing kind)
fn key(kind: &str, root: &str) -> String {
    if root.is_empty() { return String::new() }
    match kind { "board" => format!("0|{root}|"), "schematic" => format!("1|{root}|"), "3d" => format!("2|{root}|"), "split" => format!("split|{root}|"), _ => String::new() }
}

/// A pure case this port draws: no hierarchy of boards, no history, no Review page behind it (a file opened from the Files tab is `@view.file`)
fn simple(st: &State, session: &Value, sel: &str) -> bool {
    let (d, info) = (&session["drafts"], &session["info"]);
    // (Diff and Review, which pages.rs draws, may be on show: a drawing or a close puts them away)
    let own = !pages::kind_of(session, sel).is_empty();
    !sel.is_empty() && !sel.starts_with("project-home-") && !sel.starts_with('@') && st.threads.contains_key(sel)
        && s(d, "@view.bare").is_empty()
        && (own || (s(info, &format!("view.path:{sel}")).is_empty() && s(info, &format!("view.kind:{sel}")).is_empty()))
}

pub struct ViewOut {
    /// the session patch (drafts, rid)
    pub patch: Value,
    /// the commands before the project card's look (`kicad.watch`)
    pub before: Vec<Value>,
    /// the project's boards are asked for after it (`boards.list`) when the viewer opens on a drawing from anything else
    pub boards_look: bool,
}

/// The board the person picked in the Boards menu (`@view.board:<thread>`, a path from the project's folder), kept across Board, Schematic, 3D and Split and
/// across closing the viewer (0.14 kept the board picked, Bd.id): "" for the project's own. A path that leaves the folder is no choice.
fn chosen_board(d: &Value, sel: &str) -> String {
    let b = s(d, &format!("@view.board:{sel}"));
    if b.is_empty() || b.contains("..") || b.starts_with('/') || !b.ends_with(".kicad_pcb") { String::new() } else { b.to_string() }
}
/// The file a page of `kind` shows for the board `rel` (its schematic beside it on the Schematic page); "" for none.
fn board_file(kind: &str, root: &str, rel: &str) -> String {
    if rel.is_empty() || root.is_empty() { return String::new() }
    let abs = format!("{root}/{rel}");
    match kind { "schematic" => abs.strip_suffix(".kicad_pcb").map(|b| format!("{b}.kicad_sch")).unwrap_or_default(), "board" | "3d" | "split" => abs, _ => String::new() }
}

/// The `view` action for a drawing kind, or "" to close (the phones' idempotent `view`).
pub fn view_action(st: &State, hub: &str, session: &Value, kind: &str) -> Option<ViewOut> {
    // "mech\t<part>": an agent's viewer_show of a model opens Mech on that part (a path from the project's root, or absolute under it)
    let (kind, part) = kind.split_once('\t').unwrap_or((kind, ""));
    if !(shows(kind) || kind.is_empty() || kind == "review") { return None }
    let sel = s(session, "sel");
    // the Review tab: its first page (pages.rs asks for it; no source to watch)
    if kind == "review" {
        if sel.is_empty() || sel.contains('~') || sel.starts_with("project-home-") || sel.starts_with('@') || !st.threads.contains_key(sel) || !s(&session["drafts"], &format!("@view.file:{sel}")).is_empty() { return None }
        let (patch, before) = pages::open_review(session, hub, sel, "bom");
        return Some(ViewOut { patch, before, boards_look: false });
    }
    if !simple(st, session, sel) { return None }
    let d = &session["drafts"]; let prev = s(d, &format!("@view:{sel}"));
    if !(prev.is_empty() || shows(prev)) { return None }
    // Files pressed while the Files tab is on show closes it (every other press opens its kind, and "" closes): the hub decides, never the button
    let kind = if kind == "files" && files_kind(prev) { "" } else { kind };
    let root = root_of(st, sel);
    // what the hub is asked to watch: the kind and path View.ask sends (Mech's part is a 3D model; a split watches three sources)
    // Mech opened from the rail (no part named) shows the part chosen last in this thread, as it was left
    let kept_part = s(d, &format!("@mech.part:{sel}")).to_string();
    let part = if kind == "mech" && part.is_empty() { kept_part.as_str() } else { part };
    // (a board picked in the Boards menu stays the one shown on Board, Schematic, 3D and Split: its file is the source)
    let picked = if matches!(kind, "board" | "schematic" | "3d" | "split") { board_file(kind, &root, &chosen_board(d, sel)) } else { String::new() };
    let (want, wkind, wpath, board) = if kind == "mech" { let part = part.strip_prefix(&format!("{root}/")).unwrap_or(part); let ps = parts_of(&root, sel, part); let (w, a) = mech_want(&root, &ps, part); let k = if w.is_empty() { "" } else { "3d" }; (w, k.to_string(), a, String::new()) }
        else { let base = key(kind, &root); let want = if base.is_empty() { base } else { format!("{base}{picked}") };
            // (Split watches its three sources from the board: the path stays its schematic's sheet, "" the root)
            if kind == "split" { (want, kind.to_string(), String::new(), picked.clone()) } else { (want, kind.to_string(), picked.clone(), String::new()) } };
    let at = s(d, "@view.at");
    let mut rid = session["rid"].as_u64().unwrap_or(0);
    let mut before = Vec::new();
    let mut drafts = serde_json::Map::new();
    drafts.insert(format!("@view.file:{sel}"), json!(if kind == "split" { "" } else { picked.as_str() })); drafts.insert(format!("@view:{sel}"), json!(kind));
    drafts.insert(format!("@view.sheet:{sel}"), json!(""));
    if kind == "mech" { drafts.insert(format!("@mech.part:{sel}"), json!(part.strip_prefix(&format!("{root}/")).unwrap_or(part))); } // another page leaves Mech's choice as it is
    for k in ["@pick", "@card", "@pick.rid"] { drafts.insert(k.into(), json!("")); }
    if want != at {
        // View.ask: the kind, the root, the path ("" for the project's own file) and the board tree ("")
        // a thread of another machine (`<link>~<id>`, drawn from that machine's mirror: its root is a folder over there): the watch goes to it (`on`)
        let on = if sel.contains('~') { format!(",\"on\":{}", jstr(sel.split('~').next().unwrap_or(""))) } else { String::new() };
        let bd = if board.is_empty() { String::new() } else { format!(",\"board\":{}", jstr(&board)) };
        let p = format!("{{\"kind\":{},\"root\":{},\"path\":{},\"tree\":\"\"{bd}{on}}}", jstr(if want.is_empty() { "" } else { &wkind }), jstr(&root), jstr(&wpath));
        before.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"kicad.watch\",\"p\":{p}}}")}));
        rid += 1; drafts.insert("@view.at".into(), json!(want));
    }
    let mut out = json!({"rid": rid, "drafts": drafts});
    patch::merge(&mut out, &pages::leave(session, sel));
    Some(ViewOut { patch: out, before, boards_look: drawing(kind) && kind != "mech" && !drawing(prev) })
}

/// Settings opened: the viewer beside the thread shuts (0.14's Settings.viewer.shut), its watch stopped, Diff or Review put away; the board picked stays.
/// The patch and the commands; an empty patch with nothing open.
pub fn shut(hub: &str, session: &Value) -> (Value, Vec<Value>) {
    let sel = s(session, "sel"); let d = &session["drafts"];
    if sel.is_empty() { return (json!({}), vec![]) }
    let mut out = json!({});
    if !s(d, &format!("@view:{sel}")).is_empty() {
        let mut drafts = serde_json::Map::new();
        for k in ["@view", "@view.file", "@view.sheet"] { drafts.insert(format!("{k}:{sel}"), json!("")); }
        for k in ["@pick", "@card", "@pick.rid"] { drafts.insert(k.into(), json!("")); }
        out = json!({"drafts": drafts});
    }
    let mut before = vec![];
    if !s(d, "@view.at").is_empty() {
        let rid = session["rid"].as_u64().unwrap_or(0);
        before.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"kicad.watch\",\"p\":{{\"kind\":\"\",\"root\":\"\",\"path\":\"\",\"tree\":\"\"}}}}")}));
        patch::merge(&mut out, &json!({"rid": rid + 1, "drafts": {"@view.at": ""}}));
    }
    patch::merge(&mut out, &pages::leave(session, sel));
    (out, before)
}

/// The project's boards as the Boards menu lists them (View.board.rows): two or more, the one on show marked, the project's own marked as the default. None for one board or none.
fn boards_menu(session: &Value, sel: &str, root: &str) -> Option<Value> {
    let list: Vec<&str> = s(&session["info"], &format!("fab.boards:{sel}")).lines().filter(|l| !l.trim().is_empty()).collect();
    if list.len() < 2 { return None }
    let own = s(&session["info"], &format!("fab.board:{sel}"));
    let chosen = chosen_board(&session["drafts"], sel);
    let on_abs = s(&session["drafts"], &format!("@view.file:{sel}"));
    let on = if !chosen.is_empty() { chosen } else if !on_abs.is_empty() && on_abs.ends_with(".kicad_pcb") { on_abs.strip_prefix(&format!("{root}/")).unwrap_or(on_abs).to_string() } else { own.to_string() };
    let on = if on.is_empty() { list[0].to_string() } else { on };
    Some(Value::Array(list.iter().map(|p| json!({"label": p.strip_suffix(".kicad_pcb").unwrap_or(p), "value": p, "on": **p == *on, "default": !own.is_empty() && *p == own})).collect()))
}
/// `view-board <path from the root>`: another of the project's boards on the Board, Schematic, 3D or Split page (its schematic is the one beside it), kept as the
/// thread's board (`@view.board`) for the other pages and the next opening; watched as the Files tab watches a file.
pub fn board_action(st: &State, hub: &str, session: &Value, rel: &str) -> Option<ViewOut> {
    let sel = s(session, "sel"); let d = &session["drafts"];
    if !simple(st, session, sel) { return None }
    let kind = s(d, &format!("@view:{sel}"));
    if !matches!(kind, "board" | "schematic" | "3d" | "split") { return None }
    if !s(&session["info"], &format!("fab.boards:{sel}")).lines().any(|l| l == rel) { return None }
    let root = root_of(st, sel); if root.is_empty() { return None }
    // the project's own board is no choice: the pages follow the project again (its file may be renamed or moved)
    let own = s(&session["info"], &format!("fab.board:{sel}")) == rel;
    let file = board_file(kind, &root, rel); if file.is_empty() { return None }
    let want = if kind == "split" { format!("split|{root}|{file}") } else { format!("{}{file}", key(kind, &root)) };
    let mut rid = session["rid"].as_u64().unwrap_or(0); let mut before = vec![]; let mut drafts = serde_json::Map::new();
    drafts.insert(format!("@view.board:{sel}"), json!(if own { "" } else { rel }));
    drafts.insert(format!("@view.file:{sel}"), json!(if kind == "split" { "" } else { file.as_str() })); drafts.insert(format!("@view.sheet:{sel}"), json!(""));
    for k in ["@pick", "@card", "@pick.rid"] { drafts.insert(k.into(), json!("")); }
    if want != s(d, "@view.at") {
        let p = if kind == "split" { format!("{{\"kind\":\"split\",\"root\":{},\"path\":\"\",\"tree\":\"\",\"board\":{}}}", jstr(&root), jstr(&file)) }
            else { format!("{{\"kind\":{},\"root\":{},\"path\":{},\"tree\":\"\"}}", jstr(kind), jstr(&root), jstr(&file)) };
        before.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"kicad.watch\",\"p\":{p}}}")}));
        rid += 1; drafts.insert("@view.at".into(), json!(want));
    }
    Some(ViewOut { patch: json!({"rid": rid, "drafts": drafts}), before, boards_look: false })
}
/// `view-boards`: the Boards menu opened, so the project's boards are read again (0.14's data-on-open "board-list"): a board added or renamed since shows at once.
pub fn boards_look(st: &State, hub: &str, session: &Value) -> Option<Vec<Value>> {
    let sel = s(session, "sel");
    if !simple(st, session, sel) || !matches!(s(&session["drafts"], &format!("@view:{sel}")), "board" | "schematic" | "3d" | "split") { return None }
    let rid = session["rid"].as_u64().unwrap_or(0);
    Some(vec![json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"boards.list\",\"p\":{{\"thread\":{}}}}}", jstr(sel))})])
}

/// `mech-part <path from the root>`: another part on the Mech page, watched as a 3D model (Mpage.part.k). None off the Mech page; `refresh` ("mech-refresh") lists again and follows the listing.
pub fn mech_action(st: &State, hub: &str, session: &Value, rel: &str, refresh: bool) -> Option<ViewOut> {
    let sel = s(session, "sel");
    if !simple(st, session, sel) || s(&session["drafts"], &format!("@view:{sel}")) != "mech" { return None }
    let root = root_of(st, sel); let ps = mech_parts(&root);
    let rel = rel.strip_prefix(&format!("{root}/")).unwrap_or(rel); // viewer_show names a part by its absolute path
    let chosen = if refresh { s(&session["drafts"], &format!("@mech.part:{sel}")) } else { rel };
    let (want, abs) = mech_want(&root, &ps, chosen);
    let mut rid = session["rid"].as_u64().unwrap_or(0); let mut before = vec![]; let mut drafts = serde_json::Map::new();
    if !refresh { drafts.insert(format!("@mech.part:{sel}"), json!(rel)); }
    if want != s(&session["drafts"], "@view.at") {
        let p = format!("{{\"kind\":{},\"root\":{},\"path\":{},\"tree\":\"\"}}", jstr(if want.is_empty() { "" } else { "3d" }), jstr(&root), jstr(&abs));
        before.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"kicad.watch\",\"p\":{p}}}")}));
        rid += 1; drafts.insert("@view.at".into(), json!(want));
    }
    Some(ViewOut { patch: json!({"rid": rid, "drafts": drafts}), before, boards_look: false })
}


/// core/layers's focus (KiCad's high contrast): one active layer and how the others show (0 as they are, 1 dimmed, 2 hidden), kept per kind as "<active>|<mode>".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Focus { pub active: u32, pub mode: u32 }
const NONE: u32 = 255;
impl Focus {
    pub fn read(t: &str) -> Focus { let mut p = t.split('|'); let a = p.next().and_then(|x| x.parse().ok()).unwrap_or(NONE); let m = p.next().and_then(|x| x.parse::<u32>().ok()).unwrap_or(0).min(2); if t.contains('|') { Focus { active: a, mode: m } } else { Focus { active: NONE, mode: 0 } } }
    pub fn show(&self) -> String { format!("{}|{}", self.active, self.mode) }
    pub fn on(&self) -> bool { self.mode > 0 && self.active != NONE }
}
fn bit(l: u32) -> u32 { 1u32 << l.min(31) }
fn kept(sch: bool, f: Focus) -> u32 {
    let copper = !sch && (5..=12).contains(&f.active);
    bit(f.active) | if copper { bit(13) | bit(23) | bit(24) } else { 0 } | if sch { 0 } else { bit(22) } | bit(26) | bit(27) | bit(28)
}
/// Lyr.masks: the layers not drawn and those drawn dimmed, from the person's own hidden layers and the focus.
pub fn masks(sch: bool, off: u32, f: Focus) -> (u32, u32) {
    if !f.on() { return (off, 0) }
    let rest = 536870911u32 & !kept(sch, f); let act = !bit(f.active);
    if f.mode == 1 { (off & act, rest & !off) } else { ((off | rest) & act, 0) }
}
/// Lyr.pick: a layer's name makes it the active one and the others dim; again puts every layer back.
pub fn pick(f: Focus, l: u32) -> Focus { if f.on() && f.active == l { Focus { active: l, mode: 0 } } else { Focus { active: l, mode: if f.mode == 0 { 1 } else { f.mode } } } }
/// Lyr.first by what the plot draws: the first layer in the list's order with art on it (`present`, a bit each, 0 when the plot is not known), else front copper or the schematic's first layer.
fn first_art(sch: bool, present: u32) -> u32 {
    let has = |l: &u32| present & bit(*l) != 0;
    let first = if sch { (0..SCH_LAYERS.len() as u32).find(has) } else { BOARD_LAYERS.iter().map(|x| x.0).find(has) };
    first.unwrap_or(if sch { 0 } else { 12 })
}
/// Lyr.set: All / Dim / Only (the first layer with art is active when none is, or when the one kept has no art on this plot).
pub fn set_mode(sch: bool, f: Focus, mode: u32, present: u32) -> Focus {
    let bare = f.active == NONE || (present != 0 && present & bit(f.active) == 0);
    Focus { active: if bare { first_art(sch, present) } else { f.active }, mode: mode.min(2) }
}
/// Lyr.cycle: Ctrl+H, as in KiCad: the layers other than the active one as they are, dimmed, hidden, and around again.
pub fn cycle(sch: bool, f: Focus, present: u32) -> Focus { set_mode(sch, f, if f.mode == 2 { 0 } else { f.mode + 1 }, present) }
/// A layer action's value and, after a space, the layers the plot on show has (`view-layer-mode`, `view-layer-cycle`): ("2", bits); a value without them is the value and 0.
fn with_art(value: &str) -> (&str, u32) {
    match value.rsplit_once(' ').map(|(a, b)| (a, b.parse::<u32>())) { Some((a, Ok(bits))) => (a.trim(), bits), _ => (value.trim(), 0) }
}
/// Rv.bits: the layers a comparison's marks keep to, the active one and those kept with it (0 when no layer stands out, and always on a schematic).
pub fn compare_bits(sch: bool, f: Focus) -> u32 { if f.on() && !sch { kept(sch, f) } else { 0 } }

/// view-layer (hide or show a layer), view-layer-pick (the active layer), view-layer-mode (All/Dim/Only), view-layer-cycle (Ctrl+H), view-layers-reset (every layer as it is by default, no focus)
/// on the Board or Schematic page: the session patch, None elsewhere. On Split, whose panes show each kind as its tab does, Ctrl+H acts on the pane `value` names ("schematic", else the board).
pub fn layer_action(st: &State, session: &Value, action: &str, value: &str) -> Option<Value> {
    let sel = s(session, "sel"); let d = &session["drafts"]; let (value, present) = with_art(value);
    let kind = match s(d, &format!("@view:{sel}")) { "split" if action == "view-layer-cycle" => if value == "schematic" { "schematic" } else { "board" }, k => k };
    if !simple(st, session, sel) || !matches!(kind, "board" | "schematic") { return None }
    let sch = kind == "schematic"; let v: u32 = value.trim().parse().unwrap_or(if action == "view-layer" { 31 } else { NONE });
    let (offk, fk) = (format!("@view.off.{kind}"), format!("@view.focus.{kind}"));
    let mut dr = serde_json::Map::new();
    match action {
        "view-layer" => { let off: u32 = s(d, &offk).parse().unwrap_or(0); dr.insert(offk, json!((off ^ bit(v)).to_string())); }
        "view-layer-pick" => { dr.insert(fk.clone(), json!(pick(Focus::read(s(d, &fk)), v).show())); }
        "view-layer-mode" => { dr.insert(fk.clone(), json!(set_mode(sch, Focus::read(s(d, &fk)), if value.trim().parse::<u32>().is_ok() { v } else { 0 }, present).show())); }
        "view-layer-cycle" => { dr.insert(fk.clone(), json!(cycle(sch, Focus::read(s(d, &fk)), present).show())); }
        "view-layers-reset" => { dr.insert(offk, json!("")); dr.insert(fk, json!("")); }
        _ => return None,
    }
    for k in ["@pick", "@card", "@pick.rid"] { dr.insert(k.into(), json!("")); }
    Some(json!({"drafts": dr}))
}

/// `view-sheet <file>`: the schematic page shows another sheet of the design (its file, absolute; "" the root sheet), watched like the root (`View.ask` with the sheet as the path).
pub fn sheet_action(st: &State, hub: &str, session: &Value, file: &str) -> Option<ViewOut> {
    let sel = s(session, "sel"); let d = &session["drafts"];
    if !simple(st, session, sel) || s(d, &format!("@view:{sel}")) != "schematic" { return None }
    let root = root_of(st, sel);
    if !file.is_empty() && !(file.starts_with(&format!("{root}/")) && file.ends_with(".kicad_sch") && !file.contains("..")) { return None }
    let want = format!("1|{root}|{file}");
    let mut rid = session["rid"].as_u64().unwrap_or(0); let mut before = vec![]; let mut drafts = serde_json::Map::new();
    drafts.insert(format!("@view.sheet:{sel}"), json!(file));
    for k in ["@pick", "@card", "@pick.rid"] { drafts.insert(k.into(), json!("")); }
    if want != s(d, "@view.at") {
        let p = format!("{{\"kind\":\"schematic\",\"root\":{},\"path\":{},\"tree\":\"\"}}", jstr(&root), jstr(file));
        before.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"kicad.watch\",\"p\":{p}}}")}));
        rid += 1; drafts.insert("@view.at".into(), json!(want));
    }
    Some(ViewOut { patch: json!({"rid": rid, "drafts": drafts}), before, boards_look: false })
}

const BOARD_LAYERS: [(u32, &str); 17] = [(12, "F.Cu"), (11, "In1.Cu"), (10, "In2.Cu"), (9, "In3.Cu"), (8, "In4.Cu"), (7, "In5.Cu"), (6, "In6.Cu"), (5, "B.Cu"), (13, "Pads"), (23, "Vias"), (24, "Holes"),
    (16, "F.Silkscreen"), (3, "B.Silkscreen"), (19, "User.Drawings"), (20, "User.Comments"), (22, "Edge.Cuts"), (21, "Margin")];
/// A comparison's marks (Look.names); a plot has them only while it compares.
const MARK_LAYERS: [(u32, &str); 3] = [(26, "Removed"), (27, "Changed"), (28, "Added")];
const SCH_LAYERS: [&str; 24] = ["page", "sheet fill", "body fill", "sheet", "graphic", "note", "body", "pin", "pin name", "pin number", "field", "field other", "bus", "bus entry", "wire", "no connect",
    "junction", "label", "global label", "hier label", "sheet name", "sheet file", "sheet pin", "dnp"];

/// One drawing page as `View.json.single` writes it: open, key, the layers it draws, the layer list.
fn page(template: &Value, kind: &str, root: &str, drafts: &Value, sel: &str) -> Value {
    // View.key.at: a schematic's sheet is part of its source key
    let sheet = if kind == "schematic" && !sel.is_empty() { s(drafts, &format!("@view.sheet:{sel}")) } else { "" };
    // a file opened from the Files tab is the source's path ("" for the project's own)
    let file = if !sel.is_empty() && matches!(kind, "board" | "schematic" | "3d") { s(drafts, &format!("@view.file:{sel}")) } else { "" };
    let k = format!("{}{}", key(kind, root), if file.is_empty() { sheet } else { file });
    let mut v = template.clone();
    let Some(o) = v.as_object_mut() else { return v };
    o.insert("open".into(), json!(kind)); o.insert("key".into(), json!(k));
    // View.layers: a 3D view draws the board's layers on its faces
    o.insert("layers".into(), json!(if kind == "3d" { if file.is_empty() { key("board", root) } else { String::new() } } else { k }));
    // View.masks / View.lmode / View.lhead: what the person hid, the focus, and what the canvas draws from them
    let draws = matches!(kind, "board" | "schematic");
    let off: u32 = if draws { s(drafts, &format!("@view.off.{kind}")).parse().unwrap_or(0) } else { 0 };
    let f = if draws { Focus::read(s(drafts, &format!("@view.focus.{kind}"))) } else { Focus { active: NONE, mode: 0 } };
    layer_fields(o, kind, off, f);
    v
}

/// What a page's Layers menu and canvas are drawn from, for the desktop's picture and the phones' `viewer.layers` alike.
fn layer_fields(o: &mut serde_json::Map<String, Value>, kind: &str, off: u32, f: Focus) {
    o.insert("layerModes".into(), json!(kind == "board"));
    let sch = kind == "schematic"; let draws = matches!(kind, "board" | "schematic");
    let (moff, mdim) = masks(sch, off, f);
    let row = |n: u32, name: &str| json!({"layer": n, "name": name, "on": off & bit(n) == 0, "active": f.on() && f.active == n});
    let rows: Vec<Value> = match kind {
        "board" => BOARD_LAYERS.iter().map(|(n, name)| row(*n, name)).collect(),
        "schematic" => SCH_LAYERS.iter().enumerate().map(|(n, name)| row(n as u32, name)).collect(),
        _ => vec![],
    };
    // Lyr.order lists a comparison's marks after the kind's own; which plot compares is the client's, so they ride apart and show when the plot has them
    let marks: Vec<Value> = if draws { MARK_LAYERS.iter().map(|(n, name)| row(*n, name)).collect() } else { vec![] };
    o.insert("layerMarks".into(), Value::Array(marks));
    o.insert("layerList".into(), Value::Array(rows));
    o.insert("off".into(), json!(moff)); o.insert("dim".into(), json!(mdim)); o.insert("lift".into(), json!(if f.on() { f.active } else { NONE }));
    o.insert("layerMode".into(), json!(if f.on() { f.mode } else { 0 }));
    // Side.list's reset: offered while anything differs from the default; Rv.bits: what a comparison on this page keeps its marks to
    o.insert("layerReset".into(), json!(off != 0 || f.on()));
    o.insert("layerBits".into(), json!(compare_bits(sch, f)));
    if f.on() {
        let nm = if sch { SCH_LAYERS.get(f.active as usize).map(|x| x.to_string()) } else { BOARD_LAYERS.iter().find(|(n, _)| *n == f.active).map(|(_, x)| x.to_string()) };
        let nm = nm.or_else(|| MARK_LAYERS.iter().find(|(n, _)| *n == f.active).map(|(_, x)| x.to_string()));
        if let Some(nm) = nm { o.insert("layerHead".into(), json!(format!("Layer: {nm}"))); }
    }
}

/// `viewer.layers`: the phones keep `off` (what the person hid) and `focus` ("<active>|<mode>") and hold no rule: this answers what a press does to them and what to draw.
/// `action`: "" (as they are), toggle, pick, mode, cycle or reset; `present`: the layers the plot on show has, a bit each. None for another kind or action.
pub fn layer_reply(kind: &str, present: u32, off: u32, focus: &str, action: &str, value: &str) -> Option<Value> {
    if !matches!(kind, "board" | "schematic") { return None }
    let sch = kind == "schematic"; let (mut off, mut f) = (off, Focus::read(focus)); let v = value.trim().parse::<u32>();
    match action {
        "" => {}
        "toggle" => off ^= bit(v.ok().filter(|l| *l < 31)?),
        "pick" => f = pick(f, v.ok()?),
        "mode" => f = set_mode(sch, f, v.ok()?, present),
        "cycle" => f = cycle(sch, f, present),
        "reset" => { off = 0; f = Focus { active: NONE, mode: 0 }; }
        _ => return None,
    }
    let mut o = serde_json::Map::new(); o.insert("own".into(), json!(off)); o.insert("focus".into(), json!(f.show())); o.insert("layerHead".into(), json!("Layers"));
    layer_fields(&mut o, kind, off, f);
    Some(Value::Object(o))
}

/// The versions a comparison may take, from `history.refs`' lines (kind, label, commit), as the desktop lists them.
pub fn ref_rows(refs: &str) -> Vec<(String, String, String)> {
    refs.lines().filter_map(|l| { let f: Vec<&str> = l.split('\t').collect(); if f.len() < 3 || f[2].is_empty() || !matches!(f[0], "head" | "base" | "branch" | "tag" | "commit") { return None } Some((f[0].to_string(), f[1].to_string(), f[2].to_string())) }).take(60).collect()
}
/// core/hist's comparison of `file` with `commit`: "diff\n<the version>\n<the live file>", then `bits`, the layers its marks keep to (0 every layer). None for a file outside `top`.
pub fn compare_path(git: &str, top: &str, commit: &str, file: &str, bits: u32) -> Option<String> {
    let rel = file.strip_prefix(&format!("{top}/"))?;
    let old = format!("git\t{git}\t{commit}\t{}\t{top}/{rel}", top.chars().count());
    Some(if bits == 0 { format!("diff\n{old}\n{file}") } else { format!("diff\n{old}\n{file}\n{bits}") })
}


// ------------------------------------------------------------------ the Files tab

fn pct(s: &str, slash: bool) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) || (slash && b == b'/') { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}
/// Fm.size: a file's size as a person reads it.
pub fn size_text(n: u64) -> String {
    let tenths = |unit: u64, name: &str| format!("{}.{} {name}", n / unit, (n % unit) / (unit / 10));
    if n < 1024 { format!("{n} B") } else if n < 1 << 20 { format!("{} KB", (n + 512) / 1024) } else if n < 1 << 30 { tenths(1 << 20, "MB") } else { tenths(1 << 30, "GB") }
}
fn icon_of(kind: &str) -> &'static str {
    match kind { "folder" => "folder", "board" => "circuit-board", "schematic" => "cpu", "3d" => "box", "image" => "image", "pdf" => "file-text", _ => "file" }
}
/// Fm.zip.name: `<folder or project title>.zip`, nothing a file system refuses in the name.
pub fn zip_name(title: &str, dir: &str) -> String {
    let base = if dir.is_empty() { title } else { crate::files::fm::last(dir) };
    let c: String = base.chars().map(|c| if (c as u32) < 32 || "/\\:?*<>|\"".contains(c) { '_' } else { c }).collect();
    format!("{}.zip", if c.is_empty() || c == "." || c == ".." { "project" } else { &c })
}
fn down_url(thread: &str, rel: &str, folder: bool) -> String {
    if thread.is_empty() || crate::files::ops::zip_within(rel) { return String::new() }
    format!("/files/{}?thread={}&path={}", if folder { "zip" } else { "get" }, pct(thread, false), pct(rel, true))
}
/// The listing of a folder as Files.rows: folders first, then files, each row the view action it takes. `Err` is the words the page shows instead.
pub fn file_rows(root: &str, thread: &str, dir: &str) -> Result<Vec<Value>, String> {
    if root.is_empty() { return Err("This thread has no project folder.".into()) }
    let text = crate::files::dir::list(std::path::Path::new(root), dir).map_err(|e| e.to_string())?;
    Ok(rows_of(&text, thread, dir))
}
fn rows_of(text: &str, thread: &str, dir: &str) -> Vec<Value> {
    use crate::files::fm;
    text.lines().filter_map(|l| {
        let mut it = l.splitn(3, '\t'); let (k, size, n) = (it.next()?, it.next()?.parse::<u64>().unwrap_or(0), it.next()?);
        let value = fm::join(dir, n);
        Some(if k == "d" {
            json!({"label": n, "action": "fm-cd", "value": value, "detail": "", "kind": "folder", "icon": "folder", "down": down_url(thread, &value, true), "downName": zip_name("", &value)})
        } else {
            let kind = fm::preview(n);
            json!({"label": n, "action": if kind.is_empty() { "" } else { "fm-open" }, "value": value, "detail": size_text(size), "kind": kind, "icon": icon_of(kind), "down": down_url(thread, &value, false), "downName": n})
        })
    }).collect()
}
/// Fm.trail: the project, then each folder down to the one on show.
pub fn files_trail(dir: &str) -> Vec<Value> {
    let mut v = vec![json!({"label": "Project", "action": "fm-cd", "value": ""})]; let mut at = String::new();
    for p in dir.split('/').filter(|p| !p.is_empty()) { at = crate::files::fm::join(&at, p); v.push(json!({"label": p, "action": "fm-cd", "value": at})); }
    v
}
fn project_title(st: &State, sel: &str) -> String { st.threads.get(sel).and_then(|t| st.projects.get(&t.project)).map(|p| p.title.clone()).unwrap_or_default() }
/// The Files page: trail, rows, the folder's zip (Files.json).
fn files_page(st: &State, root: &str, sel: &str, dir: &str) -> Value {
    // the folder is watched on disk while this page shows it (files/watch_dir.rs): a change there makes the hub tell the views to draw it again
    // a project on another machine (`<link>~<id>`): its folder is listed there, not read from this disk
    let listed = if sel.contains('~') { Err("This project is on another machine: its folder is shown there.".to_string()) } else if root.is_empty() { Err("This thread has no project folder.".to_string()) } else { crate::files::dir::list(std::path::Path::new(root), dir).map_err(|e| e.to_string()) };
    if let Ok(t) = &listed { crate::files::watch_dir::touch(root, dir, crate::files::watch_dir::signature(t)); }
    let (rows, err) = match listed { Ok(t) => (rows_of(&t, sel, dir), String::new()), Err(e) => (vec![], e) };
    json!({"trail": files_trail(dir), "rows": rows, "ready": true, "error": err, "zip": down_url(sel, dir, true), "zipName": zip_name(&project_title(st, sel), dir)})
}
/// File.json: an opened image, PDF or note: its name, its kind, where the hub serves it (`/img` takes an absolute path).
fn file_page(kind: &str, abs: &str) -> Value {
    let mut v = json!({"name": crate::files::fm::last(abs), "kind": kind, "url": format!("/img?path={}", pct(abs, true))});
    // a Markdown note is sent as the chat's nodes (`Md.render`), so the clients draw it rendered; plain text stays the client's to fetch
    let l = abs.to_ascii_lowercase();
    if kind == "text" && (l.ends_with(".md") || l.ends_with(".markdown")) {
        use std::io::Read;
        let mut b = Vec::new();
        if std::fs::File::open(abs).map(|f| f.take(256 << 10).read_to_end(&mut b)).is_ok() { v["blocks"] = Value::Array(crate::md::nodes_json(&crate::md::render(&String::from_utf8_lossy(&b)))); }
    }
    v
}

/// `fm`, `fm-cd`, `fm-up`, `fm-open` (Fm.act): the Files page on a folder, or an opened file. None for a path outside the project or a case not ported; a file the tab does not preview changes nothing.
pub fn files_action(st: &State, hub: &str, session: &Value, action: &str, value: &str) -> Option<ViewOut> {
    use crate::files::fm;
    let sel = s(session, "sel");
    if !simple(st, session, sel) { return None }
    let d = &session["drafts"]; let prev = s(d, &format!("@view:{sel}"));
    if !(prev.is_empty() || shows(prev)) { return None }
    let root = root_of(st, sel); if root.is_empty() { return None }
    let cur = s(d, &format!("@files.dir:{sel}")).to_string();
    let (kind, dir, file): (String, String, String) = match action {
        "fm" => ("files".into(), cur, String::new()),
        "fm-up" => ("files".into(), fm::parent(&cur), String::new()),
        "fm-cd" => { if !fm::files_safe(value) { return None } ("files".into(), value.to_string(), String::new()) }
        _ => { // fm-open
            if value.is_empty() || !fm::files_safe(value) { return None }
            let k = fm::preview(fm::last(value));
            if k.is_empty() { return Some(ViewOut { patch: json!({}), before: vec![], boards_look: false }) }
            // the file must be a plain file inside the project, as the listing shows them
            let parent = std::path::Path::new(&root).join(fm::parent(value));
            let abs = fm::contained(std::path::Path::new(&root), &fm::parent(value))?.join(fm::last(value));
            let _ = parent; if !std::fs::symlink_metadata(&abs).map_or(false, |m| m.is_file()) { return None }
            (k.into(), fm::parent(value), abs.to_string_lossy().into_owned())
        }
    };
    let n = match kind.as_str() { "board" => "0", "schematic" => "1", "3d" => "2", _ => "" };
    let want = if n.is_empty() { String::new() } else { format!("{n}|{root}|{file}") };
    let wkind = match kind.as_str() { "board" | "schematic" | "3d" => kind.as_str(), _ => "" };
    let at = s(d, "@view.at");
    let mut rid = session["rid"].as_u64().unwrap_or(0); let mut before = vec![]; let mut drafts = serde_json::Map::new();
    drafts.insert(format!("@view:{sel}"), json!(kind)); drafts.insert(format!("@view.file:{sel}"), json!(file)); drafts.insert(format!("@files.dir:{sel}"), json!(dir));
    drafts.insert(format!("@view.sheet:{sel}"), json!("")); drafts.insert(format!("@mech.part:{sel}"), json!(""));
    for k in ["@pick", "@card", "@pick.rid"] { drafts.insert(k.into(), json!("")); }
    if want != at {
        let p = format!("{{\"kind\":{},\"root\":{},\"path\":{},\"tree\":\"\"}}", jstr(wkind), jstr(&root), jstr(&file));
        before.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"kicad.watch\",\"p\":{p}}}")}));
        rid += 1; drafts.insert("@view.at".into(), json!(want));
    }
    Some(ViewOut { patch: json!({"rid": rid, "drafts": drafts}), before, boards_look: false })
}

/// `thread.viewer`: the idle template with the page the thread's viewer has open. None for a page not ported.
pub fn viewer_json(st: &State, template: &Value, session: &Value) -> Option<Value> {
    let sel = s(session, "sel");
    let kind = s(&session["drafts"], &format!("@view:{sel}"));
    // a panel the helper opened (Diff, Files, Mech, Review: view.kind/view.path in the info, panel on) is the helper's page, not an idle viewer
    let info = &session["info"];
    // Diff and Review are pages.rs's: the thread carries their object beside the viewer
    match pages::kind_of(session, sel) {
        "diff" => return Some(template.clone()),
        "review" => {
            let mut v = template.clone(); let o = v.as_object_mut()?;
            o.insert("open".into(), json!("review")); o.insert("review".into(), pages::review_json(session, sel));
            let chips: Vec<Value> = [("Board", "board"), ("Schematic", "schematic"), ("3D", "3d"), ("Mech", "mech"), ("Files", "files"), ("Review", "review")].iter().map(|(l, val)| json!({"label": l, "value": val, "on": *val == "review"})).collect();
            o.insert("choices".into(), Value::Array(chips));
            return Some(v);
        }
        _ => {}
    }
    // (the session's `panel` alone is no page: it stays on from another thread's Diff when a new thread is made or another one picked, and this thread has nothing open)
    if !s(info, &format!("view.kind:{sel}")).is_empty() || !s(info, &format!("view.path:{sel}")).is_empty() { return None }
    if kind.is_empty() { return Some(template.clone()) }
    if !shows(kind) || !simple(st, session, sel) { return None }
    let root = root_of(st, sel);
    let mut v = match kind {
        // Mech: the part on show as a 3D model (View.want: "2|<root>|<abs part>"), the parts as chips (Mech.json)
        "mech" => {
            let ps = parts_of(&root, sel, s(&session["drafts"], &format!("@mech.part:{sel}"))); let cur = mech_current(&ps, s(&session["drafts"], &format!("@mech.part:{sel}")));
            let (want, _) = mech_want(&root, &ps, s(&session["drafts"], &format!("@mech.part:{sel}")));
            let mut v = template.clone(); let o = v.as_object_mut()?;
            o.insert("open".into(), json!("mech")); o.insert("key".into(), json!(want));
            let chips: Vec<Value> = ps.iter().map(|p| json!({"label": stem(p), "value": p, "on": cur.as_deref() == Some(p.as_str())})).collect();
            o.insert("mech".into(), json!({"say": if root.is_empty() { "Loading…" } else if sel.contains('~') && ps.is_empty() { "This project is on another machine: its parts are listed there. A part an agent shows opens here." } else if ps.is_empty() { "No parts yet. Ask an agent to model one: it designs it in FreeCAD, keeps it in mech/, and renders it here." } else { "" },
                "note": "", "parts": chips, "rel": cur.clone().unwrap_or_default(), "busy": false, "render": "Create renders", "err": "",
                "name": cur.as_deref().map(stem).unwrap_or_default(), "path": cur.as_deref().map(|p| abs_of(&root, p)).unwrap_or_default(), "shots": [], "empty": ""}));
            v
        }
        // Split (Split.*): the schematic, the board and its 3D model, each a page of its own
        "split" => {
            let mut v = template.clone(); let o = v.as_object_mut()?;
            o.insert("open".into(), json!("split")); o.insert("key".into(), json!(key("split", &root)));
            // a board picked in the Boards menu: each pane's source is that board's file (its schematic beside it), as kicad.watch's split keys them
            let picked = board_file("board", &root, &chosen_board(&session["drafts"], sel));
            o.insert("panes".into(), Value::Array(["schematic", "board", "3d"].iter().map(|k| { let mut p = page(template, k, &root, &session["drafts"], "");
                if !picked.is_empty() { let f = if *k == "schematic" { board_file("schematic", &root, &chosen_board(&session["drafts"], sel)) } else { picked.clone() }; let kk = format!("{}{f}", key(k, &root));
                    if *k != "3d" { p["layers"] = json!(kk); } else { p["layers"] = json!(""); } p["key"] = json!(kk); }
                p }).collect()));
            if let Some(b) = boards_menu(session, sel, &root) { o.insert("boards".into(), b); }
            v
        }
        // Files: the folder on show, or an image, a PDF or a note opened from it (Fm.tab: they belong to Files)
        "files" => {
            let mut v = template.clone(); let o = v.as_object_mut()?;
            o.insert("open".into(), json!("files")); o.insert("key".into(), json!(""));
            o.insert("files".into(), files_page(st, &root, sel, s(&session["drafts"], &format!("@files.dir:{sel}"))));
            v
        }
        "image" | "pdf" | "text" => {
            let mut v = template.clone(); let o = v.as_object_mut()?;
            o.insert("open".into(), json!(kind)); o.insert("key".into(), json!(""));
            o.insert("file".into(), file_page(kind, s(&session["drafts"], &format!("@view.file:{sel}"))));
            v
        }
        _ => { let mut v = page(template, kind, &root, &session["drafts"], sel); if matches!(kind, "board" | "schematic" | "3d") { if let Some(b) = boards_menu(session, sel, &root) { v["boards"] = b; } } v }
    };
    // the folder the Boards menu's paths are from (0.14's "Folder: <root>")
    if v["boards"].is_array() { v["boardsRoot"] = json!(root); }
    let tab = if matches!(kind, "image" | "pdf" | "text") { "files" } else { kind };
    let chips: Vec<Value> = [("Board", "board"), ("Schematic", "schematic"), ("3D", "3d"), ("Mech", "mech"), ("Files", "files"), ("Review", "review")].iter().map(|(l, val)| json!({"label": l, "value": val, "on": *val == tab})).collect();
    v.as_object_mut()?.insert("choices".into(), Value::Array(chips));
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fx() -> Value { serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/panel.json"))).unwrap() }
    fn state() -> (State, String) {
        let mut st = State::new();
        for l in include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-log.jsonl")).lines() { st.apply_line(l); }
        (st, "t1700000003-8".into())
    }
    #[test] fn the_threads_browser_is_a_page_with_nothing_to_watch() {
        let (st, th) = state();
        let template: Value = serde_json::from_str::<Value>(include_str!("thread_static.json")).unwrap()["viewer"].clone();
        let session = json!({"sel": th, "rid": 1, "drafts": {}, "open": {}, "adding": false, "panel": false, "info": {}});
        let o = view_action(&st, "h", &session, "browser").unwrap();
        assert!(o.before.is_empty() && !o.boards_look, "no kicad.watch for a page of frames");
        assert_eq!(o.patch["drafts"][format!("@view:{th}")], json!("browser"));
        let mut s2 = session.clone(); crate::session::patch::merge(&mut s2, &o.patch);
        let v = viewer_json(&st, &template, &s2).unwrap();
        assert_eq!((v["open"].clone(), v["key"].clone()), (json!("browser"), json!("")));
        // from a board: the watch is stopped
        let board = json!({"sel": th, "rid": 5, "drafts": {format!("@view:{th}"): "board", "@view.at": "0|/tmp/soak2|"}, "open": {}, "adding": false, "panel": false, "info": {}});
        assert_eq!(view_action(&st, "h", &board, "browser").unwrap().before.len(), 1);
    }
    /// A thread of another machine (drawn from its mirror, `<link>~<id>`; its project's folder is a folder over there): the viewer opens, and the watch goes to that machine.
    #[test] fn a_thread_of_another_machine_opens_the_viewer_and_its_watch_goes_there() {
        let mut st = State::new();
        for l in [r#"{"$":"ProjectCreated","id":"L~p1","title":"Board","root":"/desk/board","at":1}"#, r#"{"$":"ThreadCreated","id":"L~t1","project":"L~p1","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}"#] { st.apply_line(l); }
        let template: Value = serde_json::from_str::<Value>(include_str!("thread_static.json")).unwrap()["viewer"].clone();
        let session = json!({"sel": "L~t1", "rid": 1, "drafts": {}, "open": {}, "adding": false, "panel": false, "info": {}});
        let o = view_action(&st, "h", &session, "board").expect("a far thread opens the viewer");
        let w: Value = serde_json::from_str(o.before[0]["text"].as_str().unwrap()).unwrap();
        assert_eq!((w["m"].as_str(), w["p"]["on"].as_str(), w["p"]["root"].as_str(), w["p"]["kind"].as_str()), (Some("kicad.watch"), Some("L"), Some("/desk/board"), Some("board")));
        let mut s2 = session.clone(); crate::session::patch::merge(&mut s2, &o.patch);
        let v = viewer_json(&st, &template, &s2).expect("and draws it");
        assert_eq!((v["open"].as_str(), v["key"].as_str()), (Some("board"), Some("0|/desk/board|")));
        // this machine's own thread asks for no machine
        let (mine, th) = state(); let own = json!({"sel": th, "rid": 1, "drafts": {}, "open": {}, "adding": false, "panel": false, "info": {}});
        assert!(!view_action(&mine, "h", &own, "board").unwrap().before[0]["text"].as_str().unwrap().contains("\"on\""));
    }
    #[test] fn a_part_an_agent_showed_on_another_machine_opens_in_mech_there() {
        let mut st = State::new();
        for l in [r#"{"$":"ProjectCreated","id":"L~p1","title":"Board","root":"/desk/board","at":1}"#, r#"{"$":"ThreadCreated","id":"L~t1","project":"L~p1","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}"#] { st.apply_line(l); }
        let session = json!({"sel": "L~t1", "rid": 1, "drafts": {}, "open": {}, "adding": false, "panel": false, "info": {}});
        let o = view_action(&st, "h", &session, "mech\t/desk/board/mech/a.step").unwrap();
        let w: Value = serde_json::from_str(o.before[0]["text"].as_str().unwrap()).unwrap();
        assert_eq!((w["p"]["kind"].as_str(), w["p"]["path"].as_str(), w["p"]["on"].as_str()), (Some("3d"), Some("/desk/board/mech/a.step"), Some("L")));
    }
    #[test] fn a_panel_the_helper_opened_is_not_drawn_as_an_idle_viewer() {
        let (st, th) = state();
        let template: Value = serde_json::from_str::<Value>(include_str!("thread_static.json")).unwrap()["viewer"].clone();
        let idle = json!({"sel": th, "rid": 1, "drafts": {}, "open": {}, "adding": false, "panel": false, "info": {}});
        assert!(viewer_json(&st, &template, &idle).is_some(), "nothing open: the idle template");
        let mut diff = idle.clone(); diff["panel"] = json!(true); diff["info"] = json!({format!("view.kind:{th}"): "diff"});
        assert!(viewer_json(&st, &template, &diff).is_some(), "Diff on show is pages.rs's: the thread carries its diff beside the viewer");
        let mut kind = idle.clone(); kind["info"] = json!({format!("view.kind:{th}"): "mech"});
        assert!(viewer_json(&st, &template, &kind).is_none(), "a view.kind in the info alone is the helper's too");
        // Diff open on another thread leaves the session's panel on: this thread, with nothing of its own open, is still drawn (a new thread made from the + after Diff was refused)
        let mut other = idle.clone(); other["panel"] = json!(true); other["info"] = json!({"view.kind:t-other": "diff", "panel:t-other": "1"});
        assert_eq!(viewer_json(&st, &template, &other), Some(template.clone()), "the panel left on by another thread: the idle viewer");
        let mut board = other.clone(); board["drafts"] = json!({format!("@view:{th}"): "board", "@view.at": "0|/tmp/soak2|"});
        assert!(viewer_json(&st, &template, &board).is_some(), "and its own drawing is drawn");
    }
    #[test] fn the_view_action_and_the_viewer_of_board_schematic_and_3d_are_the_helpers() {
        let (st, th) = state(); let f = fx();
        let template: Value = serde_json::from_str::<Value>(include_str!("thread_static.json")).unwrap()["viewer"].clone();
        for kind in ["board", "schematic", "3d"] {
            let want = &f[kind];
            let session = json!({"sel": th, "rid": 1, "drafts": {}, "open": {}, "adding": false, "panel": false, "info": {}});
            let o = view_action(&st, "h", &session, kind).unwrap();
            assert_eq!(o.before, want["cmds"].as_array().unwrap()[..1].to_vec(), "{kind}: kicad.watch");
            assert!(o.boards_look, "{kind}");
            for (k, v) in want["drafts"].as_object().unwrap() { if k != "@fab.at" { assert_eq!(&o.patch["drafts"][k], v, "{kind}: {k}"); } }
            // the viewer on the session the action answered
            let mut s2 = session.clone(); crate::session::patch::merge(&mut s2, &o.patch);
            let got = viewer_json(&st, &template, &s2).unwrap();
            for (k, v) in want["viewer"].as_object().unwrap() { assert_eq!(got.get(k), Some(v), "{kind}: viewer.{k}"); }
        }
        // closing sends the stop, and a second open of the same kind sends nothing
        let s_open = json!({"sel": th, "rid": 5, "drafts": {"@view:t1700000003-8": "board", "@view.at": "0|/tmp/soak2|"}, "open": {}, "adding": false, "panel": false, "info": {}});
        assert!(view_action(&st, "h", &s_open, "board").unwrap().before.is_empty());
        let close = view_action(&st, "h", &s_open, "").unwrap();
        assert_eq!(close.before.len(), 1); assert!(!close.boards_look);
        assert!(close.before[0]["text"].as_str().unwrap().contains("\"kind\":\"\""));
        // Files is ported: opening it from a board stops the watch (a drawing's source is not the folder's), and Review is ported (session/pages.rs)
        let f = view_action(&st, "h", &s_open, "files").unwrap();
        assert_eq!(f.before.len(), 1); assert!(f.before[0]["text"].as_str().unwrap().contains("\"kind\":\"\"")); assert!(!f.boards_look);
        assert_eq!(f.patch["drafts"][format!("@view:{th}")], "files");
        assert!(view_action(&st, "h", &s_open, "review").is_some(), "Review is drawn from the session (session/pages.rs)");
    }


    fn files_world(tag: &str) -> (State, String, std::path::PathBuf) {
        let d = std::env::temp_dir().join(format!("bp-fm-pres-{}-{tag}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(d.join("docs/deep")).unwrap(); std::fs::create_dir_all(d.join("mech")).unwrap(); let d = crate::plat::fs::canonicalize(&d).unwrap();
        for (f, c) in [("notes.md", "# n"), ("a.kicad_pcb", "(kicad_pcb)"), ("pic.png", "x"), ("doc.pdf", "x"), ("m.step", "x"), ("plain.bin", "x"), ("docs/a.txt", "a"), (".env", "k"), ("docs/server.key", "k")] { std::fs::write(d.join(f), c).unwrap(); }
        let mut st = State::new(); for l in include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-log.jsonl")).lines() { st.apply_line(l); }
        let th = "t1700000003-8".to_string(); let proj = st.threads.get(&th).map(|t| t.project.clone()).unwrap();
        if let Some(p) = st.projects.get_mut(&proj) { p.root = d.to_string_lossy().into_owned(); p.title = "My: Board".into(); }
        (st, th, d)
    }
    #[test] fn pressing_files_while_files_is_on_show_closes_it() {
        let (st, th, d) = files_world("toggle");
        let mut s1 = sess(&th, json!({}));
        let o = view_action(&st, "h", &s1, "files").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        assert_eq!(s1["drafts"][format!("@view:{th}")], json!("files"), "the first press opens");
        let o = view_action(&st, "h", &s1, "files").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        assert_eq!(s1["drafts"][format!("@view:{th}")], json!(""), "the second press closes");
        let o = view_action(&st, "h", &s1, "files").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        assert_eq!(s1["drafts"][format!("@view:{th}")], json!("files"), "and the next opens again");
        let o = view_action(&st, "h", &s1, "board").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        assert_eq!(s1["drafts"][format!("@view:{th}")], json!("board"), "another tab is never a toggle");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn a_project_with_several_boards_lists_them_and_a_pick_watches_that_board() {
        let (st, th, d) = files_world("boards"); let root = d.to_string_lossy().into_owned();
        let template: Value = serde_json::from_str::<Value>(include_str!("thread_static.json")).unwrap()["viewer"].clone();
        let mut s1 = sess(&th, json!({})); let o = view_action(&st, "h", &s1, "board").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        assert!(viewer_json(&st, &template, &s1).unwrap().get("boards").is_none(), "one board: no menu");
        crate::session::patch::merge(&mut s1, &json!({"info": {format!("fab.boards:{th}"): "a/a.kicad_pcb\nb/b.kicad_pcb", format!("fab.board:{th}"): "b/b.kicad_pcb"}}));
        let b = viewer_json(&st, &template, &s1).unwrap()["boards"].clone();
        assert_eq!((b[0]["on"].as_bool(), b[1]["on"].as_bool(), b[1]["label"].as_str()), (Some(false), Some(true), Some("b/b")));
        assert!(board_action(&st, "h", &s1, "c/c.kicad_pcb").is_none(), "only a listed board");
        let o = board_action(&st, "h", &s1, "a/a.kicad_pcb").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        assert!(o.before[0]["text"].as_str().unwrap().contains(&format!("{root}/a/a.kicad_pcb")), "{:?}", o.before);
        assert_eq!(viewer_json(&st, &template, &s1).unwrap()["boards"][0]["on"], true);
        let _ = std::fs::remove_dir_all(&d);
    }
    /// a command's request params (the JSON text it sends), so paths compare as values whatever the platform's separators
    fn sent(c: &Value) -> Value { serde_json::from_str::<Value>(c["text"].as_str().unwrap_or("")).unwrap_or(Value::Null)["p"].clone() }
    #[test] fn a_picked_board_stays_across_board_schematic_3d_split_and_reopening() {
        let (st, th, d) = files_world("keep"); let root = d.to_string_lossy().into_owned();
        let template: Value = serde_json::from_str::<Value>(include_str!("thread_static.json")).unwrap()["viewer"].clone();
        let mut s1 = sess(&th, json!({})); let o = view_action(&st, "h", &s1, "board").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        crate::session::patch::merge(&mut s1, &json!({"info": {format!("fab.boards:{th}"): "a/a.kicad_pcb\nb/b.kicad_pcb", format!("fab.board:{th}"): "b/b.kicad_pcb"}}));
        assert_eq!(viewer_json(&st, &template, &s1).unwrap()["boards"][1]["default"], true, "the project's own board is marked");
        assert_eq!(viewer_json(&st, &template, &s1).unwrap()["boardsRoot"], json!(root));
        let o = board_action(&st, "h", &s1, "a/a.kicad_pcb").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        let o = view_action(&st, "h", &s1, "schematic").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        assert_eq!(sent(&o.before[0])["path"], json!(format!("{root}/a/a.kicad_sch")), "{:?}", o.before);
        assert_eq!(viewer_json(&st, &template, &s1).unwrap()["boards"][0]["on"], true, "the schematic page names the board picked");
        let o = view_action(&st, "h", &s1, "split").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        assert_eq!(sent(&o.before[0])["board"], json!(format!("{root}/a/a.kicad_pcb")), "{:?}", o.before);
        let v = viewer_json(&st, &template, &s1).unwrap();
        assert_eq!(v["panes"][1]["key"], json!(format!("0|{root}|{root}/a/a.kicad_pcb"))); assert_eq!(v["panes"][0]["key"], json!(format!("1|{root}|{root}/a/a.kicad_sch")));
        assert_eq!(v["boards"][0]["on"], true, "Split has the Boards menu too");
        let o = view_action(&st, "h", &s1, "").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        let o = view_action(&st, "h", &s1, "3d").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        assert_eq!(sent(&o.before[0])["path"], json!(format!("{root}/a/a.kicad_pcb")), "closed and opened again: still the board picked");
        // the project's own board picked again: no choice kept, the pages follow the project
        let o = board_action(&st, "h", &s1, "b/b.kicad_pcb").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        assert_eq!(s1["drafts"][format!("@view.board:{th}")], json!(""));
        assert!(boards_look(&st, "h", &s1).unwrap()[0]["text"].as_str().unwrap().contains("boards.list"));
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn opening_settings_shuts_the_viewer_and_keeps_the_board_picked() {
        let (st, th, d) = files_world("shut");
        let mut s1 = sess(&th, json!({})); let o = view_action(&st, "h", &s1, "board").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        crate::session::patch::merge(&mut s1, &json!({"drafts": {format!("@view.board:{th}"): "a/a.kicad_pcb"}}));
        let (p, c) = shut("h", &s1); crate::session::patch::merge(&mut s1, &p);
        assert_eq!(s1["drafts"][format!("@view:{th}")], json!("")); assert_eq!(s1["drafts"]["@view.at"], json!(""));
        assert!(c[0]["text"].as_str().unwrap().contains("\"kind\":\"\""), "the watch stops");
        assert_eq!(s1["drafts"][format!("@view.board:{th}")], json!("a/a.kicad_pcb"));
        assert!(shut("h", &s1).1.is_empty(), "nothing open: nothing sent");
        let _ = std::fs::remove_dir_all(&d);
    }
    fn sess(th: &str, drafts: Value) -> Value { json!({"sel": th, "rid": 1, "drafts": drafts, "open": {}, "adding": false, "panel": false, "info": {}}) }
    #[test] fn the_files_page_lists_rows_trail_and_downloads_as_core_fm_does() {
        let (st, th, d) = files_world("rows"); let root = d.to_string_lossy().into_owned();
        let rows = file_rows(&root, &th, "").unwrap();
        let by = |n: &str| rows.iter().find(|r| r["label"] == n).unwrap().clone();
        assert_eq!(rows[0]["label"], "docs"); assert_eq!(rows[1]["label"], "mech"); // folders first
        assert_eq!((by("docs")["action"].as_str(), by("docs")["kind"].as_str(), by("docs")["icon"].as_str()), (Some("fm-cd"), Some("folder"), Some("folder")));
        assert_eq!((by("notes.md")["action"].as_str(), by("notes.md")["kind"].as_str(), by("notes.md")["detail"].as_str()), (Some("fm-open"), Some("text"), Some("3 B")));
        assert_eq!(by("a.kicad_pcb")["kind"], "board"); assert_eq!(by("a.kicad_pcb")["icon"], "circuit-board"); assert_eq!(by("m.step")["kind"], "3d"); assert_eq!(by("pic.png")["icon"], "image"); assert_eq!(by("doc.pdf")["icon"], "file-text");
        assert_eq!((by("plain.bin")["action"].as_str(), by("plain.bin")["icon"].as_str()), (Some(""), Some("file")), "a file the tab cannot show is listed only");
        assert!(rows.iter().all(|r| r["label"] != ".env"), "dotfiles are not listed");
        assert_eq!(by("notes.md")["down"], format!("/files/get?thread={th}&path=notes.md")); assert_eq!(by("docs")["down"], format!("/files/zip?thread={th}&path=docs")); assert_eq!(by("docs")["downName"], "docs.zip");
        let sub = file_rows(&root, &th, "docs").unwrap();
        assert_eq!(sub.iter().map(|r| r["label"].as_str().unwrap()).collect::<Vec<_>>(), ["deep", "a.txt", "server.key"]);
        assert_eq!(sub[2]["down"], "", "a download never serves what a zip leaves out"); assert_eq!(sub[1]["value"], "docs/a.txt");
        assert_eq!(files_trail("docs/deep"), vec![json!({"label":"Project","action":"fm-cd","value":""}), json!({"label":"docs","action":"fm-cd","value":"docs"}), json!({"label":"deep","action":"fm-cd","value":"docs/deep"})]);
        assert_eq!(zip_name("My: Board", ""), "My_ Board.zip"); assert_eq!(zip_name("", ".."), "project.zip"); assert_eq!(files_page(&st, &root, &th, "")["zipName"], "My_ Board.zip");
        assert_eq!(size_text(1023), "1023 B"); assert_eq!(size_text(1024), "1 KB"); assert_eq!(size_text(1536), "2 KB"); assert_eq!(size_text(5 << 20), "5.0 MB"); assert_eq!(size_text(3 << 30), "3.0 GB");
        assert!(file_rows(&root, &th, "../x").is_err() && file_rows("", &th, "").is_err() && file_rows(&root, &th, "nope").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn fm_actions_walk_folders_open_files_and_refuse_escapes() {
        let (st, th, d) = files_world("act"); let root = d.to_string_lossy().into_owned();
        let template: Value = serde_json::from_str::<Value>(include_str!("thread_static.json")).unwrap()["viewer"].clone();
        let run = |sess: &Value, a: &str, v: &str| files_action(&st, "h", sess, a, v);
        let mut s1 = sess(&th, json!({})); let o = run(&s1, "fm-cd", "docs").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        assert_eq!(s1["drafts"][format!("@files.dir:{th}")], "docs"); assert_eq!(s1["drafts"][format!("@view:{th}")], "files");
        let v = viewer_json(&st, &template, &s1).unwrap();
        assert_eq!(v["open"], "files"); assert_eq!(v["files"]["trail"].as_array().unwrap().len(), 2); assert!(v["files"]["rows"].as_array().unwrap().iter().any(|r| r["label"] == "a.txt"));
        assert_eq!(v["choices"].as_array().unwrap().iter().find(|c| c["on"] == true).unwrap()["value"], "files");
        let o = run(&s1, "fm-up", "").unwrap(); crate::session::patch::merge(&mut s1, &o.patch); assert_eq!(s1["drafts"][format!("@files.dir:{th}")], "");
        assert!(run(&s1, "fm-cd", "..").is_none() && run(&s1, "fm-cd", "/etc").is_none() && run(&s1, "fm-open", "../x.png").is_none() && run(&s1, "fm-open", "nope.png").is_none());
        // a board opens in the Board viewer on its own file, and the hub is asked to watch that path
        let o = run(&s1, "fm-open", "a.kicad_pcb").unwrap(); let mut s2 = s1.clone(); crate::session::patch::merge(&mut s2, &o.patch);
        let w = o.before[0]["text"].as_str().unwrap(); assert!(w.contains("\"kind\":\"board\"") && w.contains(&format!("{root}/a.kicad_pcb")), "{w}");
        let v = viewer_json(&st, &template, &s2).unwrap(); assert_eq!(v["open"], "board"); assert_eq!(v["key"], format!("0|{root}|{root}/a.kicad_pcb"));
        // an image, a PDF and a note open as a file page of the Files tab; the watch is stopped
        for (n, k) in [("pic.png", "image"), ("doc.pdf", "pdf"), ("notes.md", "text")] {
            let o = run(&s2, "fm-open", n).unwrap(); let mut s3 = s2.clone(); crate::session::patch::merge(&mut s3, &o.patch);
            assert!(o.before[0]["text"].as_str().unwrap().contains("\"kind\":\"\""), "{n}");
            let v = viewer_json(&st, &template, &s3).unwrap(); assert_eq!(v["open"], k); assert_eq!(v["file"]["name"], n); assert_eq!(v["file"]["kind"], k);
            assert_eq!(v["file"]["url"], format!("/img?path={}", pct(&format!("{root}/{n}"), true)));
            assert_eq!(v["choices"].as_array().unwrap().iter().find(|c| c["on"] == true).unwrap()["value"], "files", "{n}: Files stays lit");
            // back to the folder (fm), and from there another tab
            let o = run(&s3, "fm", "").unwrap(); let mut s4 = s3.clone(); crate::session::patch::merge(&mut s4, &o.patch); assert_eq!(viewer_json(&st, &template, &s4).unwrap()["open"], "files");
            let o = view_action(&st, "h", &s3, "board").unwrap(); let mut s5 = s3.clone(); crate::session::patch::merge(&mut s5, &o.patch); assert_eq!(viewer_json(&st, &template, &s5).unwrap()["key"], format!("0|{root}|"), "a tab press shows the project's own board again");
        }
        // a Markdown note carries its rendered nodes; a plain note does not
        std::fs::write(d.join("read.txt"), "plain").unwrap();
        let o = run(&s2, "fm-open", "notes.md").unwrap(); let mut s3 = s2.clone(); crate::session::patch::merge(&mut s3, &o.patch);
        let blocks = viewer_json(&st, &template, &s3).unwrap()["file"]["blocks"].clone(); assert!(blocks.as_array().map_or(false, |b| !b.is_empty()), "{blocks}"); assert!(blocks.to_string().contains("\"tag\":\"h"), "a heading node: {blocks}");
        let o = run(&s2, "fm-open", "read.txt").unwrap(); let mut s3 = s2.clone(); crate::session::patch::merge(&mut s3, &o.patch); assert!(viewer_json(&st, &template, &s3).unwrap()["file"].get("blocks").is_none());
        // a file the tab cannot show changes nothing
        let o = run(&s1, "fm-open", "plain.bin").unwrap(); assert!(o.before.is_empty() && o.patch == json!({}));
        // Mech and Browser keep working beside it
        assert!(view_action(&st, "h", &s1, "mech").is_some() && view_action(&st, "h", &s1, "browser").is_some());
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn mech_lists_models_picks_the_assembly_and_watches_the_part_as_3d() {
        let d = std::env::temp_dir().join(format!("bp-mech-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("mech/sub")).unwrap();
        for f in ["mech/bracket.step", "mech/main-assembly.stp", "mech/sub/lid.glb", "mech/notes.txt", "mech/.hidden.step"] { std::fs::write(d.join(f), "x").unwrap(); }
        let root = d.to_string_lossy().into_owned(); let ps = mech_parts(&root);
        assert_eq!(ps, vec!["mech/bracket.step", "mech/main-assembly.stp", "mech/sub/lid.glb"]);
        assert_eq!(mech_current(&ps, "").as_deref(), Some("mech/main-assembly.stp"), "the assembly is the default");
        assert_eq!(mech_current(&ps, "mech/sub/lid.glb").as_deref(), Some("mech/sub/lid.glb"));
        assert_eq!(mech_current(&ps, "mech/gone.step").as_deref(), Some("mech/main-assembly.stp"), "a part no longer listed falls back");
        let (want, abs) = mech_want(&root, &ps, "mech/bracket.step");
        assert_eq!(want, format!("2|{root}|{root}/mech/bracket.step")); assert_eq!(abs, format!("{root}/mech/bracket.step"));
        assert_eq!(mech_want(&root, &[], ""), (String::new(), String::new()));
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn split_is_three_pages_and_a_key_of_its_own() {
        let template: Value = serde_json::from_str::<Value>(include_str!("thread_static.json")).unwrap()["viewer"].clone();
        let p = page(&template, "schematic", "/p", &json!({}), ""); assert_eq!(p["key"], "1|/p|");
        assert_eq!(page(&template, "3d", "/p", &json!({}), "")["layers"], "0|/p|");
        assert_eq!(key("split", "/p"), "split|/p|"); assert_eq!(key("split", ""), "");
    }

    #[test] fn layer_focus_masks_pick_and_modes_follow_core_layers() {
        let f = Focus::read("12|1");
        assert_eq!(masks(false, 0, f), (0, 536870911 & !kept(false, f)), "dim: the rest drawn at a quarter, nothing hidden");
        let only = Focus::read("12|2"); let (off, dim) = masks(false, 0, only); assert_eq!(dim, 0); assert_eq!(off & bit(12), 0, "the active layer is never hidden"); assert!(off & bit(5) != 0);
        assert_eq!(masks(false, 4, Focus::read("")), (4, 0), "no focus: only the person's own");
        assert_eq!(pick(Focus::read(""), 5), Focus { active: 5, mode: 1 }); assert_eq!(pick(Focus { active: 5, mode: 1 }, 5), Focus { active: 5, mode: 0 }, "again puts all back");
        assert_eq!(set_mode(false, Focus::read(""), 2, 0), Focus { active: 12, mode: 2 }); assert_eq!(set_mode(true, Focus::read(""), 1, 0).active, 0);
        assert_eq!(Focus::read("7|2").show(), "7|2"); assert!(!Focus::read("7|0").on());
    }

    #[test] fn ctrl_h_cycles_the_others_as_they_are_dimmed_hidden_and_around_again() {
        let a = cycle(false, Focus::read(""), 0); assert_eq!(a, Focus { active: 12, mode: 1 }, "no active layer: front copper, dimmed");
        let b = cycle(false, a, 0); assert_eq!(b, Focus { active: 12, mode: 2 });
        let c = cycle(false, b, 0); assert_eq!(c, Focus { active: 12, mode: 0 }); assert!(!c.on(), "every layer as it was");
        assert_eq!(cycle(false, c, 0), a, "and around again");
        assert_eq!(cycle(true, Focus::read(""), 0), Focus { active: 0, mode: 1 }, "a schematic starts at its first layer");
        assert_eq!(cycle(false, pick(Focus::read(""), 5), 0), Focus { active: 5, mode: 2 }, "a picked layer stays the active one");
        let (off, _) = masks(false, bit(5), Focus { active: 5, mode: 2 }); assert_eq!(off & bit(5), 0, "the active layer shows though the person hid it");
    }

    #[test] fn the_focus_starts_on_the_first_layer_with_art() {
        let no_front = bit(5) | bit(13) | bit(16) | bit(22); let both = no_front | bit(12);
        assert_eq!(cycle(false, Focus::read(""), no_front).active, 5, "no F.Cu art: B.Cu, the next in the list's order");
        assert_eq!(cycle(false, Focus::read(""), both).active, 12, "F.Cu has art: it stays first");
        assert_eq!(cycle(false, Focus::read(""), bit(16) | bit(22)).active, 16, "no copper at all: the first layer that has art");
        assert_eq!(set_mode(false, Focus::read(""), 2, no_front), Focus { active: 5, mode: 2 }, "All/Dim/Only start there too");
        assert_eq!(cycle(false, Focus::read("12|0"), no_front).active, 5, "a focus left on a layer this plot lacks starts over");
        assert_eq!(cycle(false, Focus::read("16|1"), no_front), Focus { active: 16, mode: 2 }, "the active layer stays while it has art");
        assert_eq!(pick(Focus::read(""), 12).active, 12, "a pick is the person's, as in core/layers");
        assert_eq!(cycle(true, Focus::read(""), bit(6) | bit(14)).active, 6, "a schematic's first layer with art");
        assert_eq!(first_art(false, bit(26)), 12, "marks alone name no layer of the list");
    }
    #[test] fn an_action_carries_the_plots_layers_after_a_space() {
        assert_eq!(with_art("2 4128"), ("2", 4128)); assert_eq!(with_art(" 32"), ("", 32)); assert_eq!(with_art("schematic 32"), ("schematic", 32));
        assert_eq!(with_art("2"), ("2", 0)); assert_eq!(with_art(""), ("", 0)); assert_eq!(with_art("schematic"), ("schematic", 0));
    }
    #[test] fn a_phone_asks_the_hub_what_a_press_does() {
        let art = bit(5) | bit(13) | bit(22);
        let r = layer_reply("board", art, 0, "", "cycle", "").unwrap();
        assert_eq!((r["focus"].as_str(), r["layerMode"].as_u64(), r["layerHead"].as_str(), r["lift"].as_u64()), (Some("5|1"), Some(1), Some("Layer: B.Cu"), Some(5)));
        assert_eq!(r["layerReset"], true); assert!(r["dim"].as_u64().unwrap() & bit(16) as u64 != 0 && r["dim"].as_u64().unwrap() & bit(13) as u64 == 0, "pads stay with copper, as on the desktop");
        assert_eq!(r["layerBits"], kept(false, Focus::read("5|1")));
        let h = layer_reply("board", art, 0, "5|1", "mode", "2").unwrap(); assert!(h["off"].as_u64().unwrap() & bit(16) as u64 != 0); assert_eq!(h["dim"], 0);
        let t = layer_reply("board", art, 0, "", "toggle", "16").unwrap(); assert_eq!(t["own"], bit(16)); assert_eq!(t["off"], bit(16)); assert_eq!(t["layerHead"], "Layers");
        let p = layer_reply("board", art, 0, "", "pick", "5").unwrap(); assert_eq!(p["focus"], "5|1");
        let z = layer_reply("board", art, bit(16), "5|2", "reset", "").unwrap(); assert_eq!((z["own"].as_u64(), z["focus"].as_str(), z["layerReset"].as_bool(), z["off"].as_u64()), (Some(0), Some("255|0"), Some(false), Some(0)));
        assert_eq!(layer_reply("board", art, 0, "5|1", "", "").unwrap()["focus"], "5|1", "no action: as they are");
        assert_eq!(layer_reply("schematic", bit(6), 0, "", "cycle", "").unwrap()["layerModes"], false);
        assert!(layer_reply("3d", 0, 0, "", "cycle", "").is_none()); assert!(layer_reply("board", 0, 0, "", "nope", "").is_none()); assert!(layer_reply("board", 0, 0, "", "toggle", "x").is_none());
    }
    #[test] fn a_phones_comparison_is_the_desktops_path_with_its_layers() {
        assert_eq!(compare_path("/p/.git", "/p", "abc", "/p/b.kicad_pcb", 0).unwrap(), "diff\ngit\t/p/.git\tabc\t2\t/p/b.kicad_pcb\n/p/b.kicad_pcb");
        assert_eq!(compare_path("/p/.git", "/p", "abc", "/p/b.kicad_pcb", 4096).unwrap(), "diff\ngit\t/p/.git\tabc\t2\t/p/b.kicad_pcb\n/p/b.kicad_pcb\n4096");
        assert!(compare_path("/p/.git", "/p", "abc", "/x/b.kicad_pcb", 0).is_none(), "a file outside the top folder");
        assert_eq!(ref_rows("head\tHEAD\tabc\nnope\tX\tdef\nbranch\tmain\t\ncommit\tfirst\tdef"), [("head".to_string(), "HEAD".to_string(), "abc".to_string()), ("commit".to_string(), "first".to_string(), "def".to_string())]);
    }
    #[test] fn a_comparison_keeps_to_the_active_layer_on_a_board_only() {
        assert_eq!(compare_bits(false, Focus::read("")), 0); assert_eq!(compare_bits(false, Focus::read("12|0")), 0, "no layer stands out");
        let b = compare_bits(false, Focus::read("12|1")); assert_eq!(b, kept(false, Focus::read("12|1"))); assert!(b & bit(12) != 0 && b & bit(23) != 0 && b & bit(5) == 0);
        assert_eq!(compare_bits(true, Focus::read("3|1")), 0, "a schematic's comparison marks every layer");
    }

    #[test] fn the_layer_page_says_what_can_be_reset_and_keeps_the_marks_apart() {
        let template: Value = serde_json::from_str::<Value>(include_str!("thread_static.json")).unwrap()["viewer"].clone();
        let p = |d: Value| page(&template, "board", "/p", &d, "");
        assert_eq!(p(json!({}))["layerReset"], false); assert_eq!(p(json!({"@view.off.board": "32"}))["layerReset"], true);
        assert_eq!(p(json!({"@view.focus.board": "12|1"}))["layerReset"], true); assert_eq!(p(json!({"@view.focus.board": "12|0"}))["layerReset"], false);
        let q = p(json!({"@view.focus.board": "27|1"})); let l = q["layerMarks"].as_array().unwrap();
        assert_eq!(l.iter().map(|r| r["name"].as_str().unwrap()).collect::<Vec<_>>(), ["Removed", "Changed", "Added"]); assert_eq!(l[1]["active"], true);
        assert!(q["layerList"].as_array().unwrap().iter().all(|r| r["layer"].as_u64().unwrap() < 26), "a plain plot's list is Lyr.order's");
        assert_eq!(q["layerHead"], "Layer: Changed");
        assert!(page(&template, "3d", "/p", &json!({}), "")["layerList"].as_array().unwrap().is_empty(), "a 3D mesh has no layers");
    }

    fn thread_on(page_kind: &str) -> (State, Value, String) {
        let mut st = State::new(); for l in include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-log.jsonl")).lines() { st.apply_line(l); }
        let th = "t1700000003-8".to_string();
        let session = json!({"sel": th, "rid": 1, "drafts": {format!("@view:{th}"): page_kind}, "open": {}, "adding": false, "panel": false, "info": {}});
        (st, session, th)
    }
    #[test] fn ctrl_h_acts_on_the_page_or_the_split_pane_and_nowhere_else() {
        let (st, session, _) = thread_on("board");
        assert_eq!(layer_action(&st, &session, "view-layer-cycle", "").unwrap()["drafts"]["@view.focus.board"], "12|1");
        assert_eq!(layer_action(&st, &session, "view-layer-cycle", " 32").unwrap()["drafts"]["@view.focus.board"], "5|1", "the plot's layers ride after a space");
        assert_eq!(layer_action(&st, &session, "view-layer-mode", "2 32").unwrap()["drafts"]["@view.focus.board"], "5|2");
        let (st, session, _) = thread_on("schematic");
        assert_eq!(layer_action(&st, &session, "view-layer-cycle", "").unwrap()["drafts"]["@view.focus.schematic"], "0|1");
        let (st, session, _) = thread_on("split");
        assert_eq!(layer_action(&st, &session, "view-layer-cycle", "").unwrap()["drafts"]["@view.focus.board"], "12|1");
        assert_eq!(layer_action(&st, &session, "view-layer-cycle", "schematic").unwrap()["drafts"]["@view.focus.schematic"], "0|1");
        assert_eq!(layer_action(&st, &session, "view-layer-cycle", "schematic 64").unwrap()["drafts"]["@view.focus.schematic"], "6|1");
        assert!(layer_action(&st, &session, "view-layer", "5").is_none(), "Split has no Layers menu");
        let (st, session, _) = thread_on("3d"); assert!(layer_action(&st, &session, "view-layer-cycle", "").is_none());
    }
    #[test] fn reset_clears_what_the_person_hid_and_the_focus() {
        let (st, mut session, th) = thread_on("board");
        session["drafts"] = json!({format!("@view:{th}"): "board", "@view.off.board": "32", "@view.focus.board": "5|2"});
        let p = layer_action(&st, &session, "view-layers-reset", "").unwrap();
        assert_eq!(p["drafts"]["@view.off.board"], ""); assert_eq!(p["drafts"]["@view.focus.board"], "");
    }

    #[test] fn a_viewer_show_of_a_part_opens_mech_on_it() {
        let d = std::env::temp_dir().join(format!("bp-mech2-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(d.join("mech")).unwrap();
        for f in ["mech/a.step", "mech/b.step"] { std::fs::write(d.join(f), "x").unwrap(); }
        let root = d.to_string_lossy().into_owned();
        let mut st = State::new(); for l in include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-log.jsonl")).lines() { st.apply_line(l); }
        let th = "t1700000003-8"; let proj = st.threads.get(th).map(|t| t.project.clone()).unwrap();
        if let Some(p) = st.projects.get_mut(&proj) { p.root = root.clone(); }
        let session = json!({"sel": th, "rid": 1, "drafts": {}, "open": {}, "adding": false, "panel": false, "info": {}});
        let o = view_action(&st, "h", &session, &format!("mech\t{root}/mech/b.step")).unwrap();
        assert_eq!(o.patch["drafts"][format!("@mech.part:{th}")], "mech/b.step");
        let w = o.before[0]["text"].as_str().unwrap(); assert!(w.contains("\"kind\":\"3d\"") && w.contains(&format!("{root}/mech/b.step")), "{w}");
        // another page, then Mech again from the rail (no part named): the part chosen last, not the first
        let session = json!({"sel": th, "rid": 2, "drafts": {format!("@view:{th}"): "mech", format!("@mech.part:{th}"): "mech/b.step"}, "open": {}, "adding": false, "panel": false, "info": {}});
        let o = view_action(&st, "h", &session, "3d").unwrap();
        assert!(o.patch["drafts"].get(format!("@mech.part:{th}")).is_none(), "3D leaves Mech's choice");
        let session = json!({"sel": th, "rid": 3, "drafts": {format!("@view:{th}"): "3d", format!("@mech.part:{th}"): "mech/b.step"}, "open": {}, "adding": false, "panel": false, "info": {}});
        let o = view_action(&st, "h", &session, "mech").unwrap();
        assert_eq!(o.patch["drafts"][format!("@mech.part:{th}")], "mech/b.step");
        let w = o.before[0]["text"].as_str().unwrap(); assert!(w.contains(&format!("{root}/mech/b.step")), "{w}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test] fn a_sheet_is_part_of_the_schematics_source_and_is_watched_by_its_file() {
        let template: Value = serde_json::from_str::<Value>(include_str!("thread_static.json")).unwrap()["viewer"].clone();
        let drafts = json!({"@view.sheet:t1": "/p/amp.kicad_sch"});
        assert_eq!(page(&template, "schematic", "/p", &drafts, "t1")["key"], "1|/p|/p/amp.kicad_sch");
        assert_eq!(page(&template, "schematic", "/p", &drafts, "t2")["key"], "1|/p|", "another thread's sheet is not this one's");
        assert_eq!(page(&template, "board", "/p", &drafts, "t1")["key"], "0|/p|");
    }
}
