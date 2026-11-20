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
/// A model's path as Mech keeps it: from the root with `/` when it is under the root (either separator, as Windows writes it), else as given (absolute).
pub fn rel_in(root: &str, p: &str) -> String {
    if root.is_empty() { return p.to_string() }
    let (r, q) = (root.replace('\\', "/"), p.replace('\\', "/"));
    let r = r.trim_end_matches('/');
    let eq = |a: &str, b: &str| if cfg!(windows) { a.eq_ignore_ascii_case(b) } else { a == b };
    match q.get(..r.len()) { Some(head) if eq(head, r) && q[r.len()..].starts_with('/') => q[r.len() + 1..].to_string(), _ => p.to_string() }
}
/// A file Mech draws: a STEP, STP or GLB, any case.
pub fn is_model(p: &str) -> bool { let l = p.to_ascii_lowercase(); [".step", ".stp", ".glb"].iter().any(|x| l.ends_with(x)) }
fn abs_of(root: &str, rel: &str) -> String { if crate::plat::fs::is_abs(rel) { rel.to_string() } else { format!("{root}/{rel}") } }
/// viewer_show's check before it answers (the client's `shown` decides the page from the same kind and path): why this cannot be shown, None
/// when it can. A model for Mech must be a STEP, STP or GLB file that is there; a kind the viewer has no page for is refused, never acknowledged
/// while another part is drawn. A relative path is from one of `roots` (the thread's design folder and its own).
pub fn show_refusal(kind: &str, path: &str, roots: &[String]) -> Option<String> {
    let l = path.to_ascii_lowercase();
    let there = || if crate::plat::fs::is_abs(path) { std::path::Path::new(path).is_file() } else { roots.iter().any(|r| !r.is_empty() && std::path::Path::new(r).join(path).is_file()) };
    match kind {
        "mech" if path.is_empty() => None,
        "mech" | "3d" if is_model(path) => (!there()).then(|| format!("no such file: {path} (Mech shows a STEP, STP or GLB file that exists)")),
        "mech" => Some(format!("Mech shows STEP, STP and GLB files, not {path}")),
        "board" | "pcb" | "schematic" | "sch" | "3d" | "split" | "browser" | "files" | "review" | "gerbers" => None,
        "" if is_model(path) => (!there()).then(|| format!("no such file: {path}")),
        "" if l.ends_with(".kicad_pcb") || l.ends_with(".kicad_sch") || l.ends_with(".pcbdoc") || l.ends_with(".schdoc") => None,
        "" => Some(format!("the viewer has no page for {path}: give a kind (board, schematic, 3d, mech, files) or a .kicad_pcb, .kicad_sch, .step or .glb")),
        k => Some(format!("unknown viewer kind {k:?} (board, schematic, 3d, mech, split, files, review)")),
    }
}
/// The source a Mech page asks the hub for: the part as a 3D model ("" with no part).
/// A project's parts: listed from its folder here; a project on another machine from that machine's listing of its `mech/` (net/farfiles.rs:
/// the first draw has none yet, the next has them), with the part an agent showed kept even before then. Here too a model asked for
/// outside `mech/` (a board's GLB in build/, a vendor STEP anywhere) is kept first while it is there: viewer_show shows what it names (issue 1102).
fn parts_of(root: &str, sel: &str, chosen: &str) -> Vec<String> {
    if !sel.contains('~') {
        let mut ps = mech_parts(root);
        if !chosen.is_empty() && is_model(chosen) && !ps.iter().any(|p| p == chosen) && std::path::Path::new(&abs_of(root, chosen)).is_file() { ps.insert(0, chosen.to_string()) }
        return ps
    }
    let mut ps = match crate::net::farfiles::listing(sel, "mech") { Some(Ok(t)) => parts_in_listing(&t), _ => vec![] };
    if !chosen.is_empty() && !ps.iter().any(|p| p == chosen) { ps.insert(0, chosen.to_string()) }
    ps
}
/// The models in a `files::dir::list` listing of `mech/` (its files, not its folders), as `mech/<name>`, in byte order.
fn parts_in_listing(t: &str) -> Vec<String> {
    let mut v: Vec<String> = t.lines().filter_map(|l| { let mut it = l.splitn(3, '\t'); let (k, _, n) = (it.next()?, it.next()?, it.next()?);
        let low = n.to_ascii_lowercase(); (k != "d" && !n.starts_with('.') && [".step", ".stp", ".glb"].iter().any(|x| low.ends_with(x))).then(|| format!("mech/{n}")) }).collect();
    v.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes())); v
}
fn mech_want(root: &str, ps: &[String], chosen: &str) -> (String, String) {
    match mech_current(ps, chosen) { Some(p) if !root.is_empty() => { let a = abs_of(root, &p); (format!("2|{root}|{a}"), a) }, _ => (String::new(), String::new()) }
}
/// a kind this panel can show: a drawing, or the Browser (the page rides the browser helper, not `kicad.watch`)
fn shows(kind: &str) -> bool { drawing(kind) || kind == "browser" || files_kind(kind) }
/// The Files tab and what it opened: the folder, or an image, a PDF or a text note
fn files_kind(kind: &str) -> bool { matches!(kind, "files" | "image" | "pdf" | "text") }

/// Ui.root: the thread's worktree, else its project's folder; a hardware project's design is
/// always shown from its main checkout (kicad/canonical.rs `design_root`), whichever worktree the thread has
fn root_of(st: &State, sel: &str) -> String {
    let Some(t) = st.threads.get(sel) else { return String::new() };
    if !t.worktree.is_empty() { return crate::files::kicad::canonical::design_root(&t.worktree) }
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
fn chosen_board(d: &Value, sel: &str, root: &str) -> String {
    let b = viewer_identity::relative(root, s(d, &format!("@view.board:{sel}"))).unwrap_or_default();
    if b.is_empty() || b.contains("..") || b.starts_with('/') || !(b.ends_with(".kicad_pcb") || b.to_ascii_lowercase().ends_with(".pcbdoc")) { String::new() } else { b.to_string() }
}
fn schematic_for_board(board: &str) -> String {
    board.strip_suffix(".kicad_pcb").map(|stem| format!("{stem}.kicad_sch")).unwrap_or_default()
}

/// The file a page of `kind` shows for the board `rel` (its schematic beside it on the Schematic page); "" for none.
fn board_file(kind: &str, root: &str, rel: &str) -> String {
    if rel.is_empty() || root.is_empty() { return String::new() }
    let abs = format!("{root}/{rel}");
    match kind { "schematic" => schematic_for_board(&abs), "board" | "3d" | "split" => abs, _ => String::new() }
}

/// Minimum interval (ms) between accepted `"retry"` actions in one session (1 s spam guard).
pub const RETRY_COOLDOWN_MS: u64 = 1000;
/// Pure spam guard: whether a `"retry"` at `now_ms` is allowed given the session's last retry timestamp (`@view.retry.at`).
pub fn retry_allowed(last_ms: u64, now_ms: u64) -> bool {
    now_ms == 0 || last_ms == 0 || now_ms.saturating_sub(last_ms) >= RETRY_COOLDOWN_MS
}

/// The `view` action for a drawing kind, or "" to close (the phones' idempotent `view`).
/// `"retry"` re-sends the current page's complete `kicad.watch` with `force: true` (including Split's board/sheet and a far thread's `on`), rate-limited to once per second per session.
pub fn view_action(st: &State, hub: &str, session: &Value, kind: &str) -> Option<ViewOut> {
    view_action_at(st, hub, session, kind, 0)
}
pub fn view_action_at(st: &State, hub: &str, session: &Value, kind: &str, now_ms: u64) -> Option<ViewOut> {
    match view_action_for(st, hub, session, None, "en", kind, now_ms)? {
        LayoutOut::Ok(o) => Some(o),
        LayoutOut::Refused { .. } => None,
    }
}
/// Session-aware `view` action (rail press, close, Split, retry) applying the hub's KiCad guard
/// on every mutation path and returning localized catalog refusals when blocked.
pub fn view_action_for(
    st: &State,
    hub: &str,
    session: &Value,
    client_id: Option<u64>,
    locale: &str,
    kind: &str,
    now_ms: u64,
) -> Option<LayoutOut> {
    let sel = s(session, "sel");
    let d = &session["drafts"];
    let force_retry = kind == "retry";
    // "retry": re-watch whatever page is currently open through the same presenter path
    let (kind, part) = if force_retry {
        if !simple(st, session, sel) { return None }
        let cur = s(d, &format!("@view:{sel}"));
        if cur.is_empty() { return Some(LayoutOut::Ok(ViewOut { patch: json!({}), before: vec![], boards_look: false })) }
        if cur == "board" && s(d, &format!("@view.gerbers:{sel}")) == "1" {
            let f = s(d, &format!("@view.file:{sel}"));
            if f.starts_with(crate::files::kicad::gerber::CMP) || f.starts_with("noboard\n") { ("gerbers", "board") } else { ("gerbers", "") }
        } else { (cur, "") }
    } else {
        // "mech\t<part>": an agent's viewer_show of a model opens Mech on that part (a path from the project's root, or absolute under it)
        kind.split_once('\t').unwrap_or((kind, ""))
    };
    // Gerbers: the Board page drawing the project's fab output (its newest set under mfg/: kicad/gerber), the page marked as theirs
    let gerbers = kind == "gerbers";
    // "gerbers\tboard": the set on show compared with its board (kicad/gerber/compare.rs); "gerbers\t" (or none) the set alone
    let gerbers_vs_board = gerbers && part == "board";
    let kind = if gerbers { "board" } else { kind };
    if !(shows(kind) || kind.is_empty() || kind == "review") { return None }

    let linked = sel.contains('~');
    let eff_root = if linked {
        st.threads.get(sel).map(|t| if !t.worktree.is_empty() { t.worktree.clone() } else { st.projects.get(&t.project).map(|p| p.root.clone()).unwrap_or_default() }).unwrap_or_default()
    } else {
        root_of(st, sel)
    };
    let stored = parse_stored_layout(d, sel);
    // Hub-resolved KiCad guard on EVERY mutation path (rail press, close, Split, Review).
    if !force_retry {
        let cur_for_guard = stored.clone().unwrap_or_else(|| {
            let open = s(d, &format!("@view:{sel}"));
            if open.is_empty() { super::layout::Layout::empty() } else {
                let picked_board = chosen_board(d, sel, &eff_root);
                super::layout::derive_layout(&super::layout::DeriveInput {
                    root: &eff_root, open, diff_on: pages::diff_on(session, sel),
                    gerbers: s(d, &format!("@view.gerbers:{sel}")) == "1",
                    file: s(d, &format!("@view.file:{sel}")), sheet: s(d, &format!("@view.sheet:{sel}")),
                    picked_board: &picked_board, ..Default::default()
                })
            }
        });
        let picked_abs = board_file("board", &eff_root, &chosen_board(d, sel, &eff_root));
        let guard = hub_kicad_guard(&cur_for_guard, &eff_root, &picked_abs, &session["info"], client_id, None, None, linked);
        let locked_ref = guard.locked.as_deref();

        // Multi-tile session layout: `view ""` closes the FOCUSED tile only; rail press routes through `layout::rail_press`
        // (or `layout::viewer_show` when an explicit path is given), and commits via the single `commit_layout` writer.
        if let Some(cur_layout) = &stored {
            if cur_layout.tiles().len() >= 2 && kind != "split" {
                let next_res = if kind.is_empty() {
                    let fid = cur_layout.focus.as_deref().unwrap_or("");
                    super::layout::close(cur_layout, fid, locked_ref)
                } else {
                    let target_kind = if gerbers {
                        super::layout::PageKind::Gerbers
                    } else {
                        super::layout::PageKind::from_page_str(kind).unwrap_or(super::layout::PageKind::Board)
                    };
                    let src = if !part.is_empty() && !gerbers_vs_board {
                        super::layout::sanitize_rel(&eff_root, part)
                    } else {
                        String::new()
                    };
                    if !part.is_empty() && !gerbers_vs_board {
                        super::layout::viewer_show(cur_layout, target_kind, src, locked_ref)
                    } else {
                        super::layout::rail_press(cur_layout, target_kind, src, |_| true, locked_ref)
                    }
                };
                return Some(match next_res {
                    Err(r) => LayoutOut::Refused { code: r.code(), key: r.i18n_key(), text: r.text_for(locale) },
                    Ok(next_layout) => LayoutOut::Ok(commit_layout(st, hub, session, sel, &eff_root, &next_layout, false)),
                });
            }
        }
        // Even on a single-tile or Split layout, if the holder's KiCad tile is locked, refuse close/switch/Split.
        if let Some(lid) = locked_ref {
            if cur_for_guard.contains_tile(lid) {
                let target_kind = if kind.is_empty() { None } else if gerbers { Some(super::layout::PageKind::Gerbers) } else { super::layout::PageKind::from_page_str(kind) };
                let same_tile = cur_for_guard.tiles().len() == 1
                    && cur_for_guard.tile(lid).is_some_and(|t| Some(t.kind) == target_kind)
                    && kind != "split";
                if !same_tile {
                    let r = super::layout::Refusal::Locked;
                    return Some(LayoutOut::Refused { code: r.code(), key: r.i18n_key(), text: r.text_for(locale) });
                }
            }
        }
    }

    // the Review tab: its first page (pages.rs asks for it; no source to watch)
    if kind == "review" {
        if sel.is_empty() || sel.contains('~') || sel.starts_with("project-home-") || sel.starts_with('@') || !st.threads.contains_key(sel) || !s(&session["drafts"], &format!("@view.file:{sel}")).is_empty() { return None }
        let (mut patch, before) = pages::open_review(session, hub, sel, "bom", has_management(&root_of(st, sel)));
        if let Some(d) = patch["drafts"].as_object_mut() { d.insert(format!("@view.layout:{sel}"), json!("")); }
        return Some(LayoutOut::Ok(ViewOut { patch, before, boards_look: false }));
    }
    if !simple(st, session, sel) { return None }
    let prev = s(d, &format!("@view:{sel}"));
    if !(prev.is_empty() || shows(prev)) { return None }
    // Files pressed while the Files tab is on show closes it (every other press opens its kind, and "" closes): the hub decides, never the button
    let kind = if !force_retry && kind == "files" && files_kind(prev) { "" } else { kind };
    let root = root_of(st, sel);
    // what the hub is asked to watch: the kind and path View.ask sends (Mech's part is a 3D model; a split watches three sources)
    // Mech opened from the rail (no part named) shows the part chosen last in this thread, as it was left
    let kept_part = s(d, &format!("@mech.part:{sel}")).to_string();
    let part = if kind == "mech" && part.is_empty() { kept_part.as_str() } else { part };
    let kept_sheet = if force_retry { s(d, &format!("@view.sheet:{sel}")).to_string() } else { String::new() };
    // (a board picked in the Boards menu stays the one shown on Board, Schematic, 3D and Split: its file is the source)
    let mut gerbers_no_board = false;
    let picked = if gerbers {
        use crate::files::kicad::gerber as g;
        // the set on show (a set or a file opened from Files), else the project's newest; none yet opens the empty Gerbers page instead of refusing the press
        let shown = s(d, &format!("@view.file:{sel}"));
        let shown_set = shown.strip_prefix("noboard\n").unwrap_or(shown);
        let set = if s(d, &format!("@view.gerbers:{sel}")) == "1" && !shown_set.is_empty() { Some(g::set_of(shown_set).to_string()) } else { g::default_set(std::path::Path::new(&root)).map(|p| p.to_string_lossy().into_owned()) };
        match set {
            Some(set) if gerbers_vs_board => match g::board_for_watch(std::path::Path::new(&set), std::path::Path::new(&root)) {
                Some(b) => g::cmp_path(&set, &b.to_string_lossy()),
                None => { gerbers_no_board = true; format!("noboard\n{set}") }
            },
            Some(set) => set,
            None => String::new(),
        }
    }
        else if matches!(kind, "board" | "schematic" | "3d" | "split") { board_file(kind, &root, &chosen_board(d, sel, &root)) } else { String::new() };
    let (want, wkind, wpath, board) = if kind == "mech" { let part = rel_in(&root, part); let ps = parts_of(&root, sel, &part); let (w, a) = mech_want(&root, &ps, &part); let k = if w.is_empty() { "" } else { "3d" }; (w, k.to_string(), a, String::new()) }
        else if gerbers && (picked.is_empty() || gerbers_no_board) { (String::new(), String::new(), String::new(), String::new()) }
        else { let base = key(kind, &root);
            // (Split watches its three sources from the board: the path stays its schematic's sheet, "" the root)
            if kind == "split" {
                let want = if base.is_empty() { base } else if kept_sheet.is_empty() { format!("{base}{picked}") } else { format!("{base}{picked}|{kept_sheet}") };
                (want, kind.to_string(), kept_sheet.clone(), picked.clone())
            } else {
                let p = if kind == "schematic" && !kept_sheet.is_empty() { kept_sheet.clone() } else { picked.clone() };
                let want = if base.is_empty() { base } else { format!("{base}{p}") };
                (want, kind.to_string(), p, String::new())
            }
        };
    if force_retry {
        let last_ms: u64 = s(d, "@view.retry.at").parse().unwrap_or(0);
        if !retry_allowed(last_ms, now_ms) {
            return Some(LayoutOut::Ok(ViewOut { patch: json!({}), before: vec![], boards_look: false }));
        }
    }
    let at = s(d, "@view.at");
    let mut rid = session["rid"].as_u64().unwrap_or(0);
    let mut before = Vec::new();
    let mut drafts = serde_json::Map::new();
    drafts.insert(format!("@view.file:{sel}"), json!(if kind == "split" { "" } else { picked.as_str() })); drafts.insert(format!("@view:{sel}"), json!(kind));
    drafts.insert(format!("@view.gerbers:{sel}"), json!(if gerbers { "1" } else { "" }));
    if !force_retry {
        // Legacy single-page / close / Split transitions clear `@view.layout:<sel>` so `viewer_json`
        // derives the fresh layout cleanly (multi-tile `rail_press` returned early above).
        drafts.insert(format!("@view.layout:{sel}"), json!(""));
    }
    // vs board starts on one layer's differences (F.Cu, the others dimmed) unless a layer is already in focus: the list picks another
    if gerbers_vs_board && !gerbers_no_board && !Focus::read(s(d, "@view.focus.board")).on() { drafts.insert("@view.focus.board".into(), json!(Focus { active: 12, mode: 1 }.show())); }
    if !force_retry { drafts.insert(format!("@view.sheet:{sel}"), json!("")); }
    if kind == "mech" { drafts.insert(format!("@mech.part:{sel}"), json!(rel_in(&root, part))); } // another page leaves Mech's choice as it is
    for k in ["@pick", "@card", "@pick.rid"] { drafts.insert(k.into(), json!("")); }
    if want != at || (force_retry && !want.is_empty()) {
        // View.ask: the kind, the root, the path ("" for the project's own file) and the board tree ("")
        // a thread of another machine (`<link>~<id>`, drawn from that machine's mirror: its root is a folder over there): the watch goes to it (`on`)
        let on = if sel.contains('~') { format!(",\"on\":{}", jstr(sel.split('~').next().unwrap_or(""))) } else { String::new() };
        let bd = if board.is_empty() { String::new() } else { format!(",\"board\":{}", jstr(&board)) };
        let fc = if force_retry { ",\"force\":true" } else { "" };
        let p = format!("{{\"kind\":{},\"root\":{},\"path\":{},\"tree\":\"\"{bd}{on}{fc}}}", jstr(if want.is_empty() { "" } else { &wkind }), jstr(&root), jstr(&wpath));
        before.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"kicad.watch\",\"p\":{p}}}")}));
        rid += 1; drafts.insert("@view.at".into(), json!(want));
        if force_retry && now_ms > 0 { drafts.insert("@view.retry.at".into(), json!(now_ms.to_string())); }
    }
    let mut out = json!({"rid": rid, "drafts": drafts});
    patch::merge(&mut out, &pages::leave(session, sel));
    Some(LayoutOut::Ok(ViewOut { patch: out, before, boards_look: !force_retry && drawing(kind) && kind != "mech" && !drawing(prev) }))
}

/// Where the hub keeps a board's probe (0.14's Probe.key "probe:<root>|<board>"): the board picked, else the project's own; the same for every page of it.
fn probe_key(session: &Value, sel: &str, root: &str) -> String {
    let b = chosen_board(&session["drafts"], sel, root);
    let b = if b.is_empty() { s(&session["info"], &format!("fab.board:{sel}")).to_string() } else { b };
    format!("probe:{root}|{b}")
}
/// The probe kept for the board on show ("" none): the tapped piece as kicad.info's text ("title;k=v;...").
fn probe_of(st: &State, session: &Value, sel: &str, root: &str) -> String { st.settings.get(&probe_key(session, sel, root)).cloned().unwrap_or_default() }
/// `view-probe <info>`: a piece tapped on any page (or "" for none): kept as the hub's setting for this board, so Board, Schematic, Split and every
/// opening (and the phones) light it again. Only a clear is accepted off a drawing page. Probe text is one line, at most 2000 bytes.
pub fn probe_action(st: &State, hub: &str, session: &Value, value: &str) -> Option<Vec<Value>> {
    let sel = s(session, "sel");
    if !simple(st, session, sel) { return None }
    if !value.is_empty() && !matches!(s(&session["drafts"], &format!("@view:{sel}")), "board" | "schematic" | "3d" | "split") { return None }
    if value.len() > 2000 || value.contains('\n') { return None }
    let root = root_of(st, sel); if root.is_empty() { return None }
    let key = probe_key(session, sel, &root);
    if st.settings.get(&key).map(String::as_str).unwrap_or("") == value { return Some(vec![]) }
    let rid = session["rid"].as_u64().unwrap_or(0);
    Some(vec![json!({"type": "send", "hub": hub, "text": json!({"id": rid, "m": "setting.set", "p": {"key": key, "value": value}}).to_string()})])
}

/// When an empty Gerbers page is open and fab files appear on disk, start its `kicad.watch` from the presenter and record `@view.at` so closing always stops it.
pub fn sync_gerbers(st: &State, hub: &str, session: &mut Value, cmds: &mut Vec<Value>) {
    let sel = s(session, "sel").to_string();
    if sel.is_empty() || sel.contains('~') { return }
    let d = &session["drafts"];
    if s(d, &format!("@view:{sel}")) != "board" || s(d, &format!("@view.gerbers:{sel}")) != "1" || !s(d, &format!("@view.file:{sel}")).is_empty() { return }
    let root = root_of(st, &sel);
    if root.is_empty() { return }
    let Some(set) = crate::files::kicad::gerber::default_set(std::path::Path::new(&root)) else { return };
    let set_s = set.to_string_lossy().into_owned();
    let want = format!("{}{set_s}", key("board", &root));
    let at = s(d, "@view.at").to_string();
    if let Some(m) = session["drafts"].as_object_mut() { m.insert(format!("@view.file:{sel}"), json!(set_s)); }
    if want != at {
        let rid = session["rid"].as_u64().unwrap_or(0);
        let p = format!("{{\"kind\":\"board\",\"root\":{},\"path\":{},\"tree\":\"\"}}", jstr(&root), jstr(&set_s));
        cmds.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"kicad.watch\",\"p\":{p}}}")}));
        session["rid"] = json!(rid + 1);
        if let Some(m) = session["drafts"].as_object_mut() { m.insert("@view.at".into(), json!(want)); }
    }
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
    let chosen = chosen_board(&session["drafts"], sel, root);
    let on_abs = s(&session["drafts"], &format!("@view.file:{sel}"));
    let on = if !chosen.is_empty() { chosen } else if !on_abs.is_empty() && on_abs.ends_with(".kicad_pcb") { on_abs.strip_prefix(&format!("{root}/")).unwrap_or(on_abs).to_string() } else { own.to_string() };
    // nothing known of the board on show (the project's own not read yet): no row is lit and the picker says "Select board". It lit the first
    // board by name, which was not the board the page drew (Nov 16: "it shows a board other than the one picked there on top")
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
    let root = root_of(st, sel);
    let rel = rel_in(&root, rel); let rel = rel.as_str(); // viewer_show names a part by its absolute path
    let chosen = if refresh { s(&session["drafts"], &format!("@mech.part:{sel}")) } else { rel };
    let ps = parts_of(&root, sel, chosen);
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
    first_art_kind(if sch { "schematic" } else { "board" }, present)
}
fn first_art_kind(kind: &str, present: u32) -> u32 {
    let has = |l: &u32| present & bit(*l) != 0;
    let first = match kind {
        "schematic" => (0..SCH_LAYERS.len() as u32).find(has),
        "gerbers" => GERBER_LAYERS.iter().chain(GERBER_MORE.iter()).map(|x| x.0).find(has),
        _ => BOARD_LAYERS.iter().map(|x| x.0).find(has),
    };
    first.unwrap_or(if kind == "schematic" { 0 } else { 12 })
}
fn set_mode_kind(kind: &str, f: Focus, mode: u32, present: u32) -> Focus {
    let bare = f.active == NONE || (present != 0 && present & bit(f.active) == 0);
    Focus { active: if bare { first_art_kind(kind, present) } else { f.active }, mode: mode.min(2) }
}
/// Lyr.set: All / Dim / Only (the first layer with art is active when none is, or when the one kept has no art on this plot).
pub fn set_mode(sch: bool, f: Focus, mode: u32, present: u32) -> Focus {
    set_mode_kind(if sch { "schematic" } else { "board" }, f, mode, present)
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

/// `view-sheet <file>`: the schematic page (or Split's schematic pane) shows another sheet of the design (its file, absolute; "" the root sheet), watched like
/// the root (`View.ask` with the sheet as the path; Split watches its three sources again with that sheet).
pub fn sheet_action(st: &State, hub: &str, session: &Value, file: &str) -> Option<ViewOut> {
    let sel = s(session, "sel"); let d = &session["drafts"];
    let kind = s(d, &format!("@view:{sel}"));
    if !simple(st, session, sel) || !matches!(kind, "schematic" | "split") { return None }
    let root = root_of(st, sel);
    if !file.is_empty() && !(file.starts_with(&format!("{root}/")) && (file.ends_with(".kicad_sch") || file.to_ascii_lowercase().ends_with(".schdoc")) && !file.contains("..")) { return None }
    let board = if kind == "split" { board_file("split", &root, &chosen_board(d, sel, &root)) } else { String::new() };
    let want = if kind == "split" { format!("split|{root}|{board}|{file}") } else { format!("1|{root}|{file}") };
    let mut rid = session["rid"].as_u64().unwrap_or(0); let mut before = vec![]; let mut drafts = serde_json::Map::new();
    drafts.insert(format!("@view.sheet:{sel}"), json!(file));
    for k in ["@pick", "@card", "@pick.rid"] { drafts.insert(k.into(), json!("")); }
    if want != s(d, "@view.at") {
        let p = if kind == "split" { let bd = if board.is_empty() { String::new() } else { format!(",\"board\":{}", jstr(&board)) }; format!("{{\"kind\":\"split\",\"root\":{},\"path\":{},\"tree\":\"\"{bd}}}", jstr(&root), jstr(file)) }
            else { format!("{{\"kind\":\"schematic\",\"root\":{},\"path\":{},\"tree\":\"\"}}", jstr(&root), jstr(file)) };
        before.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"kicad.watch\",\"p\":{p}}}")}));
        rid += 1; drafts.insert("@view.at".into(), json!(want));
    }
    Some(ViewOut { patch: json!({"rid": rid, "drafts": drafts}), before, boards_look: false })
}

const BOARD_LAYERS: [(u32, &str); 17] = [(12, "F.Cu"), (11, "In1.Cu"), (10, "In2.Cu"), (9, "In3.Cu"), (8, "In4.Cu"), (7, "In5.Cu"), (6, "In6.Cu"), (5, "B.Cu"), (13, "Pads"), (23, "Vias"), (24, "Holes"),
    (16, "F.Silkscreen"), (3, "B.Silkscreen"), (19, "User.Drawings"), (20, "User.Comments"), (22, "Edge.Cuts"), (21, "Margin")];
/// A Gerber set's layers (kicad/gerber, plotgen's look_gerber): the board's, and the fab layers a board view leaves out.
const GERBER_LAYERS: [(u32, &str); 18] = [(12, "F.Cu"), (11, "In1.Cu"), (10, "In2.Cu"), (9, "In3.Cu"), (8, "In4.Cu"), (7, "In5.Cu"), (6, "In6.Cu"), (5, "B.Cu"), (24, "Drills"),
    (14, "F.Mask"), (4, "B.Mask"), (15, "F.Paste"), (2, "B.Paste"), (16, "F.Silkscreen"), (3, "B.Silkscreen"), (17, "F.Courtyard"), (1, "B.Courtyard"), (22, "Edge.Cuts")];
const GERBER_MORE: [(u32, &str); 3] = [(18, "F.Fab"), (0, "B.Fab"), (19, "User.Drawings")];
/// A comparison's marks (Look.names); a plot has them only while it compares.
const MARK_LAYERS: [(u32, &str); 3] = [(26, "Removed"), (27, "Changed"), (28, "Added")];
const SCH_LAYERS: [&str; 24] = ["page", "sheet fill", "body fill", "sheet", "graphic", "note", "body", "pin", "pin name", "pin number", "field", "field other", "bus", "bus entry", "wire", "no connect",
    "junction", "label", "global label", "hier label", "sheet name", "sheet file", "sheet pin", "dnp"];

#[derive(Default)]
struct ShownGerbers {
    path: String,
    made_at: Option<u64>,
    has: Option<bool>,
}

/// One drawing page, including the source facts from its GerbView listing.
fn page(template: &Value, kind: &str, root: &str, drafts: &Value, sel: &str, sources: &RailSources, shown: &mut ShownGerbers) -> Value {
    // View.key.at: a schematic's sheet is part of its source key
    let sheet = if kind == "schematic" && !sel.is_empty() { s(drafts, &format!("@view.sheet:{sel}")) } else { "" };
    // a file opened from the Files tab is the source's path ("" for the project's own)
    let file = if !sel.is_empty() && matches!(kind, "board" | "schematic" | "3d") { s(drafts, &format!("@view.file:{sel}")) } else { "" };
    let k = format!("{}{}", key(kind, root), if file.is_empty() { sheet } else { file });
    let mut v = template.clone();
    let Some(o) = v.as_object_mut() else { return v };
    o.insert("open".into(), json!(kind)); o.insert("key".into(), json!(k));
    // whether the project has fab output for a Gerbers tab (phones list the tab only then; the desktop's rail has it always)
    if matches!(kind, "board" | "schematic" | "3d") && !root.is_empty() { o.insert("hasGerbers".into(), json!(sources.gerbers)); }
    // the Board page drawing a Gerber set (kicad/gerber): the rail lights Gerbers, the layer list is the set's
    let mut k = k;
    if kind == "board" && !sel.is_empty() && s(drafts, &format!("@view.gerbers:{sel}")) == "1" {
        o.insert("gerbers".into(), json!(true));
        let no_board = file.starts_with("noboard\n");
        let raw_file = file.strip_prefix("noboard\n").unwrap_or(file);
        // compared with its board, or not (the bar's toggle)
        o.insert("gerbersVsBoard".into(), json!(no_board || raw_file.starts_with(crate::files::kicad::gerber::CMP)));
        // what "Open in GerbView" opens (the set's job, else its files): the desktop offers it where its KiCad has GerbView in process
        let set = crate::files::kicad::gerber::set_of(raw_file);
        let set = if set.is_empty() { sources.newest.as_ref().map(|(p, _)| p.clone()) }
            else { viewer_identity::relative(root, set).map(|rel| std::path::Path::new(root).join(rel)) };
        if no_board {
            k.clear();
            o.insert("key".into(), json!(""));
            o.insert("empty".into(), json!("No board to compare these Gerbers against."));
            o.insert("emptyNext".into(), json!("Add a board to this project to compare them."));
        } else if set.is_none() {
            k.clear();
            o.insert("key".into(), json!(""));
            o.insert("empty".into(), json!("No Gerber files in this project yet"));
            o.insert("emptyNext".into(), json!("Ask an agent to export fabrication files into mfg/."));
        } else if file.is_empty() {
            // fab files appeared on disk while the empty Gerbers page was open: key the page to the new Gerber set (never the plain board)
            let s_path = set.as_ref().unwrap().to_string_lossy().into_owned();
            k = format!("{}{s_path}", key("board", root));
            o.insert("key".into(), json!(k));
        }
        let mut files = Vec::new();
        if let Some(p) = set {
            shown.path = p.to_string_lossy().into_owned();
            if !sel.contains('~') && !root.is_empty() {
                let time = sources.newest.as_ref().filter(|(path, _)| path == &p).map(|(_, t)| *t);
                (files, shown.made_at) = crate::files::kicad::gerber::shown_set(std::path::Path::new(root), &p, time);
            }
        }
        shown.has = (!sel.contains('~') && !root.is_empty()).then_some(!files.is_empty());
        o.insert("gerberFiles".into(), json!(files));
    }
    // View.layers: a 3D view draws the board's layers on its faces
    o.insert("layers".into(), json!(if kind == "3d" { if file.is_empty() { key("board", root) } else { String::new() } } else { k }));
    // View.masks / View.lmode / View.lhead: what the person hid, the focus, and what the canvas draws from them
    let draws = matches!(kind, "board" | "schematic");
    let off: u32 = if draws { s(drafts, &format!("@view.off.{kind}")).parse().unwrap_or(0) } else { 0 };
    let f = if draws { Focus::read(s(drafts, &format!("@view.focus.{kind}"))) } else { Focus { active: NONE, mode: 0 } };
    layer_fields(o, kind, off, f);
    if o.get("gerbers") == Some(&json!(true)) {
        // compared with its board: each layer's differences in its row ("F.Cu · 154"), from the last comparison of this set
        let counts = if file.starts_with(crate::files::kicad::gerber::CMP) { crate::files::kicad::gerber::counts_of(crate::files::kicad::gerber::set_of(file)) } else { vec![] };
        let named = |n: &str| { let c = counts.iter().find(|c| c.0 == n || (n == "Drills" && c.0 == "Drills")); match c { Some((_, a, b)) => format!("{n} · {}", a + b), None => n.to_string() } };
        let rows: Vec<Value> = GERBER_LAYERS.iter().chain(GERBER_MORE.iter()).map(|(n, name)| json!({"layer": n, "name": named(name), "on": off & bit(*n) == 0, "active": f.on() && f.active == *n})).collect();
        o.insert("layerList".into(), Value::Array(rows));
        if f.on() { if let Some((_, nm)) = GERBER_LAYERS.iter().chain(GERBER_MORE.iter()).find(|(n, _)| *n == f.active) { o.insert("layerHead".into(), json!(format!("Layer: {nm}"))); } }
    }
    v
}

/// What a page's Layers menu and canvas are drawn from, for the desktop's picture and the phones' `viewer.layers` alike.
fn layer_fields(o: &mut serde_json::Map<String, Value>, kind: &str, off: u32, f: Focus) {
    o.insert("layerModes".into(), json!(matches!(kind, "board" | "gerbers")));
    let sch = kind == "schematic"; let draws = matches!(kind, "board" | "schematic" | "gerbers");
    let (moff, mdim) = masks(sch, off, f);
    let row = |n: u32, name: &str| json!({"layer": n, "name": name, "on": off & bit(n) == 0, "active": f.on() && f.active == n});
    let rows: Vec<Value> = match kind {
        "board" => BOARD_LAYERS.iter().map(|(n, name)| row(*n, name)).collect(),
        "gerbers" => GERBER_LAYERS.iter().chain(GERBER_MORE.iter()).map(|(n, name)| row(*n, name)).collect(),
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
        let nm = if sch { SCH_LAYERS.get(f.active as usize).map(|x| x.to_string()) }
            else if kind == "gerbers" { GERBER_LAYERS.iter().chain(GERBER_MORE.iter()).find(|(n, _)| *n == f.active).map(|(_, x)| x.to_string()) }
            else { BOARD_LAYERS.iter().find(|(n, _)| *n == f.active).map(|(_, x)| x.to_string()) };
        let nm = nm.or_else(|| MARK_LAYERS.iter().find(|(n, _)| *n == f.active).map(|(_, x)| x.to_string()));
        if let Some(nm) = nm { o.insert("layerHead".into(), json!(format!("Layer: {nm}"))); }
    }
}

/// `viewer.layers`: the phones keep `off` (what the person hid) and `focus` ("<active>|<mode>") and hold no rule: this answers what a press does to them and what to draw.
/// `action`: "" (as they are), toggle, pick, mode, cycle or reset; `present`: the layers the plot on show has, a bit each. None for another kind or action.
pub fn layer_reply(kind: &str, present: u32, off: u32, focus: &str, action: &str, value: &str) -> Option<Value> {
    if !matches!(kind, "board" | "schematic" | "gerbers") { return None }
    let (mut off, mut f) = (off, Focus::read(focus)); let v = value.trim().parse::<u32>();
    match action {
        "" => {}
        "toggle" => off ^= bit(v.ok().filter(|l| *l < 31)?),
        "pick" => f = pick(f, v.ok()?),
        "mode" => f = set_mode_kind(kind, f, v.ok()?, present),
        "cycle" => f = set_mode_kind(kind, f, if f.mode == 2 { 0 } else { f.mode + 1 }, present),
        "reset" => { off = 0; f = Focus { active: NONE, mode: 0 }; }
        _ => return None,
    }
    let mut o = serde_json::Map::new(); o.insert("own".into(), json!(off)); o.insert("focus".into(), json!(f.show())); o.insert("layerHead".into(), json!("Layers"));
    layer_fields(&mut o, kind, off, f);
    Some(Value::Object(o))
}

/// The versions a comparison may take, from `history.refs`' lines (kind, label, commit), as the desktop lists them.
pub fn ref_rows(refs: &str) -> Vec<(String, String, String)> {
    // a turn's snapshot (files/snapshot.rs, refs.sh `snap`): "Before turn N", the newest few first so they are not cut off
    let snap = |l: &str| { let f: Vec<&str> = l.split('\t').collect(); (f.len() >= 3 && f[0] == "snap" && !f[2].is_empty()).then(|| ("snap".to_string(), format!("Before turn {}", f[1].rsplit('/').next().unwrap_or("")), f[2].to_string())) };
    let snaps: Vec<_> = refs.lines().filter_map(snap).take(10).collect();
    let rest = refs.lines().filter_map(|l| { let f: Vec<&str> = l.split('\t').collect(); if f.len() < 3 || f[2].is_empty() || !matches!(f[0], "head" | "base" | "branch" | "tag" | "commit") { return None } Some((f[0].to_string(), f[1].to_string(), f[2].to_string())) });
    snaps.into_iter().chain(rest).take(60).collect()
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
    // a project on another machine (`<link>~<id>`): its folder is listed there and the listing comes over (net/farfiles.rs); downloads are that machine's
    let far = sel.contains('~');
    let listed = if far { match crate::net::farfiles::listing(sel, dir) { Some(r) => r, None => return json!({"trail": files_trail(dir), "rows": [], "ready": false, "error": "", "zip": "", "zipName": ""}) } }
        else if root.is_empty() { Err("This thread has no project folder.".to_string()) } else { crate::files::dir::list(std::path::Path::new(root), dir).map_err(|e| crate::plat::privacy_block(root).unwrap_or_else(|| e.to_string())) };
    if let (false, Ok(t)) = (far, &listed) { crate::files::watch_dir::touch(root, dir, crate::files::watch_dir::signature(t)); }
    let (mut rows, err) = match listed { Ok(t) => (rows_of(&t, sel, dir), String::new()), Err(e) => (vec![], e) };
    if far { for r in rows.iter_mut() { r["down"] = json!(""); } }
    json!({"trail": files_trail(dir), "rows": rows, "ready": true, "error": err, "zip": if far { String::new() } else { down_url(sel, dir, true) }, "zipName": zip_name(&project_title(st, sel), dir)})
}
/// File.json: an opened image, PDF or note: its name, its kind, where the hub serves it (`/img` takes an absolute path).
fn file_page(kind: &str, abs: &str) -> Value {
    let mut v = json!({"name": crate::files::fm::last(abs), "kind": kind, "url": format!("/img?path={}", pct(abs, true))});
    // a Markdown note is sent as a document's nodes (md/doc.rs: GitHub's Markdown, its pictures beside it), so the clients draw it rendered; plain text stays the client's to fetch
    let l = abs.to_ascii_lowercase();
    if kind == "text" && (l.ends_with(".md") || l.ends_with(".markdown")) {
        use std::io::Read;
        let mut b = Vec::new();
        if std::fs::File::open(abs).map(|f| f.take(256 << 10).read_to_end(&mut b)).is_ok() {
            let base = crate::md::doc::Base::for_file(std::path::Path::new(abs));
            v["blocks"] = Value::Array(crate::md::nodes_json(&crate::md::doc::render(&String::from_utf8_lossy(&b), &base)));
        }
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
            // a project on another machine: the file is there (its path there; a drawing is made there, an image or note is copied here to show)
            if sel.contains('~') { (k.into(), fm::parent(value), format!("{}/{value}", root.trim_end_matches('/'))) } else {
            // the file must be a plain file inside the project, as the listing shows them
            let abs = fm::contained(std::path::Path::new(&root), &fm::parent(value))?.join(fm::last(value));
            if !std::fs::symlink_metadata(&abs).map_or(false, |m| m.is_file()) { return None }
            (k.into(), fm::parent(value), abs.to_string_lossy().into_owned()) }
        }
    };
    let n = match kind.as_str() { "board" => "0", "schematic" => "1", "3d" => "2", _ => "" };
    let want = if n.is_empty() { String::new() } else { format!("{n}|{root}|{file}") };
    let wkind = match kind.as_str() { "board" | "schematic" | "3d" => kind.as_str(), _ => "" };
    let at = s(d, "@view.at");
    let mut rid = session["rid"].as_u64().unwrap_or(0); let mut before = vec![]; let mut drafts = serde_json::Map::new();
    drafts.insert(format!("@view:{sel}"), json!(kind)); drafts.insert(format!("@view.file:{sel}"), json!(file)); drafts.insert(format!("@files.dir:{sel}"), json!(dir));
    drafts.insert(format!("@view.gerbers:{sel}"), json!(if kind == "board" && crate::files::kicad::gerber::is_fab_file(fm::last(value)) { "1" } else { "" }));
    drafts.insert(format!("@view.sheet:{sel}"), json!("")); drafts.insert(format!("@mech.part:{sel}"), json!(""));
    for k in ["@pick", "@card", "@pick.rid"] { drafts.insert(k.into(), json!("")); }
    if want != at {
        let on = if sel.contains('~') { format!(",\"on\":{}", jstr(sel.split('~').next().unwrap_or(""))) } else { String::new() };
        let p = format!("{{\"kind\":{},\"root\":{},\"path\":{},\"tree\":\"\"{on}}}", jstr(wkind), jstr(&root), jstr(&file));
        before.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"kicad.watch\",\"p\":{p}}}")}));
        rid += 1; drafts.insert("@view.at".into(), json!(want));
    }
    Some(ViewOut { patch: json!({"rid": rid, "drafts": drafts}), before, boards_look: false })
}

use super::rail;

#[derive(Clone, Default)]
pub struct RailSources {
    pub board: bool,
    pub schematic: bool,
    pub mech: bool,
    pub gerbers: bool,
    pub altium: bool,
    pub pcb: String,
    pub sch: String,
    pub gerber_set: String,
    docs: Value,
    root_sheet: String,
    newest: Option<(std::path::PathBuf, std::time::SystemTime)>,
}

pub fn rail_sources(root: &str) -> RailSources {
    rail_sources_for(root, "", "", "", "")
}

// A history watch names a render request. The selected working document still
// supplies the rail, management panels and embedded-editor availability.
fn working_file(file: &str) -> &str {
    if file.starts_with("git\t") || file.starts_with("diff\n") { "" } else { file }
}

fn rail_sources_for(root: &str, selected: &str, kind: &str, file: &str, sheet: &str) -> RailSources {
    use std::path::Path;
    if root.is_empty() { return RailSources::default(); }
    let files = crate::files::kicad::project::read(root);
    let file = working_file(file);
    let board_path = if matches!(kind,"board"|"3d") && !file.is_empty() { file } else if selected.is_empty() { &files.pcb } else { selected };
    let root_sheet = if selected.is_empty() || files.sch.to_ascii_lowercase().ends_with(".schdoc") {
        files.sch.clone()
    } else { schematic_for_board(selected) };
    let sheet_path = if matches!(kind, "schematic"|"split") && !sheet.is_empty() { sheet }
        else if kind == "schematic" && !file.is_empty() { file } else { &root_sheet };
    let doc = |path| {
        let rel = viewer_identity::relative(root,path).unwrap_or_default();
        let exists = !rel.is_empty() && Path::new(root).join(&rel).is_file();
        (rel, exists)
    };
    let (pcb, board) = doc(board_path);
    let (sch, schematic) = doc(sheet_path);
    let newest = crate::files::kicad::gerber::newest_set(Path::new(root)).map(|(rel, time)| (Path::new(root).join(rel), time));
    let gerber_set = newest.as_ref().map(|(path, _)| path.to_string_lossy().into_owned()).unwrap_or_default();
    let docs = json!({"board":pcb, "hasBoard":board, "sheet":sch, "hasSheet":schematic});
    RailSources {
        board, schematic, docs, root_sheet,
        mech: !mech_parts(root).is_empty(), gerbers: newest.is_some(), newest,
        altium: crate::files::kicad::project::is_altium(&files),
        pcb: files.pcb, sch: files.sch, gerber_set,
    }
}

/// Whether a Split sub-pane (`"schematic"`, `"board"`, `"3d"`) has no local source file to draw: `(empty, next)` when missing, `None` when present or on a far machine.
#[cfg(test)]
fn split_pane_empty(kind: &str, root: &str, picked_board: &str, sheet: &str, far: bool) -> Option<(&'static str, &'static str)> {
    if far || root.is_empty() { return None }
    pane_empty(kind, &rail_sources_for(root, picked_board, "split", "", sheet))
}

fn pane_empty(kind: &str, sources: &RailSources) -> Option<(&'static str, &'static str)> {
    match kind {
        "schematic" if !sources.schematic => Some(("No schematic in this project yet", "Ask an agent to draw one.")),
        "board" | "3d" if !sources.board => Some(("No board in this project yet", "Ask an agent to lay one out.")),
        _ => None,
    }
}

/// `thread.viewer`: the idle template with the page the thread's viewer has open. None for a page not ported.
pub fn viewer_json(st: &State, template: &Value, session: &Value) -> Option<Value> {
    let sel = s(session, "sel");
    // the rail's Simulation entry (desktop: KiCad's Simulator in the pane), whatever page is on show: the frame it opens, where this
    // hub's KiCad embeds it and the thread's project has a schematic (kicad/embed.rs rail_simulation); absent otherwise, and for
    // another machine's thread (nothing to embed here)
    let root = root_of(st, sel);
    let local = !sel.is_empty() && !sel.contains('~');
    let drafts = &session["drafts"];
    let draft = |key: &str| s(drafts, &format!("{key}:{sel}"));
    let kind = if draft("@view.gerbers") == "1" { "gerbers" } else { draft("@view") };
    let sources = std::cell::LazyCell::new(|| if local {
        rail_sources_for(&root, &chosen_board(drafts, sel, &root), kind, draft("@view.file"), draft("@view.sheet"))
    } else { RailSources::default() });
    let mut shown = ShownGerbers::default();
    let mut v = viewer_page(st, template, session, &root, || &*sources, &mut shown)?;
    // Follow agent: the thread's setting and the last edit's focus (the phones read the same; kicad/follow.rs)
    if !sel.is_empty() && !sel.contains('~') && v["open"].as_str().is_some_and(|o| matches!(o, "board" | "schematic" | "split")) {
        if let Some(o) = v.as_object_mut() { o.insert("follow".into(), crate::files::kicad::follow::picture(sel)); }
    }
    if local {
        if (sources.board || sources.schematic) && !sources.altium && matches!(s(&v, "open"), "review" | "board" | "schematic" | "3d" | "split") {
            let picked = board_file("board", &root, &chosen_board(&session["drafts"], sel, &root));
            let versions = if v["open"] == "review" { s(&session["drafts"], &format!("@rp.versions:{sel}")) } else { "" };
            v["designPanels"] = design_panels(if picked.is_empty() { &root } else { &picked }, versions);
        }
        if v["open"] == "review" && !sources.schematic && v["review"]["tabs"].as_array().is_some_and(|tabs| tabs.iter().any(|t| t["page"] == "bom" && t["on"] == true))
            && s(&session["drafts"], &format!("@rp.versions:{sel}")).is_empty() {
            v["review"]["empty"] = json!({"title":"Nothing to review yet", "next":"Add a schematic to see its parts here."});
        }
        if sources.altium { v["cad"] = json!("altium"); }
        if let Some(f) = crate::files::kicad::embed::rail_simulation(&root) { if let Some(o) = v.as_object_mut() { o.insert("simFrame".into(), json!(f)); } }
    }
    // A restored template can already show the default set before the thread has a page draft.
    if local && v["gerbers"] == true && shown.has.is_none() && !root.is_empty() {
        shown.has = Some(sources.gerbers);
        if let Some((path, time)) = &sources.newest {
            shown.path = path.to_string_lossy().into_owned();
            shown.made_at = time.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs());
        }
    }
    // `viewer.layout` + `viewer.tiles`: per-session layout (`@view.layout:<sel>`) parsed once per
    // picture, falling back to derivation from legacy single-page / Split drafts. Zero extra
    // filesystem discovery; linked-machine threads use mirrored values only.
    let mut active_layout = super::layout::Layout::empty();
    let mut had_stored_layout = false;
    if !sel.is_empty() {
        let linked = sel.contains('~');
        let (eff_root_local, default_pcb, default_sch, default_gerber_set) = if local {
            (root.as_str(), sources.pcb.as_str(), sources.sch.as_str(), sources.gerber_set.as_str())
        } else { ("", "", "", "") };
        let linked_root = if linked {
            st.threads.get(sel).map(|t| {
                if !t.worktree.is_empty() { t.worktree.clone() }
                else { st.projects.get(&t.project).map(|p| p.root.clone()).unwrap_or_default() }
            }).unwrap_or_default()
        } else {
            String::new()
        };
        let eff_root = if linked { linked_root.as_str() } else { eff_root_local };
        let drafts = &session["drafts"];
        let picked_board = chosen_board(drafts, sel, eff_root);
        let open = s(&v, "open");
        let picked_sch = if picked_board.is_empty() || linked {
            String::new()
        } else if open == "split" {
            v["panes"].as_array().and_then(|ps| ps.first()).and_then(|sch_pane| {
                if sch_pane["empty"].as_str().is_some_and(|e| !e.is_empty()) {
                    return None;
                }
                let k = sch_pane["key"].as_str().unwrap_or("");
                let prefix = format!("1|{eff_root}|");
                k.strip_prefix(&prefix).filter(|p| !p.is_empty()).map(str::to_string)
            }).unwrap_or_default()
        } else if open == "schematic" {
            let k = v["key"].as_str().unwrap_or("");
            let prefix = format!("1|{eff_root}|");
            if v["empty"].as_str().is_some_and(|e| !e.is_empty()) {
                String::new()
            } else {
                k.strip_prefix(&prefix).filter(|p| !p.is_empty()).map(str::to_string).unwrap_or_default()
            }
        } else {
            String::new()
        };
        if let Some(stored) = parse_stored_layout(drafts, sel) {
            had_stored_layout = true;
            active_layout = stored;
        } else {
            active_layout = super::layout::derive_layout(&super::layout::DeriveInput {
                root: eff_root,
                open,
                diff_on: pages::diff_on(session, sel),
                gerbers: v["gerbers"] == true,
                gerbers_vs_board: v["gerbersVsBoard"] == true,
                file: s(drafts, &format!("@view.file:{sel}")),
                sheet: s(drafts, &format!("@view.sheet:{sel}")),
                picked_board: &picked_board,
                picked_sch: &picked_sch,
                mech_rel: s(&v["mech"], "rel"),
                files_dir: s(drafts, &format!("@files.dir:{sel}")),
                default_pcb,
                default_sch,
                default_gerber_set,
            });
        }
        v["layout"] = serde_json::to_value(&active_layout).unwrap_or(Value::Null);
        let src_ref = local.then_some(&*sources);
        v["tiles"] = build_tiles_picture(&active_layout, eff_root, &v, drafts, src_ref, &shown, linked);
    }
    if local {
        let selected = v["choices"].as_array().and_then(|rows| rows.iter().find(|r| r["on"] == true)).and_then(|r| r["value"].as_str());
        let focused_kind = active_layout
            .focus
            .as_deref()
            .and_then(|fid| active_layout.tile(fid))
            .map(|t| {
                if t.kind == super::layout::PageKind::Board
                    && t.overlay.as_ref().is_some_and(|o| o.kind == super::layout::PageKind::Gerbers)
                {
                    "gerbers"
                } else {
                    t.kind.as_str()
                }
            });
        let page = if had_stored_layout {
            focused_kind.unwrap_or("")
        } else if v["gerbers"] == true {
            "gerbers"
        } else {
            selected.unwrap_or_else(|| s(&v, "open"))
        };
        let present: Vec<&str> = active_layout.tiles().iter().map(|t| {
            if t.kind == super::layout::PageKind::Board
                && t.overlay.as_ref().is_some_and(|o| o.kind == super::layout::PageKind::Gerbers)
            {
                "gerbers"
            } else {
                t.kind.as_str()
            }
        }).collect();
        let input = rail::RailInput {
            board: sources.board, schematic: sources.schematic, mech: sources.mech || v["mech"]["parts"].as_array().is_some_and(|p| !p.is_empty()),
            gerbers: v["hasGerbers"].as_bool().unwrap_or(sources.gerbers),
            simulation: v["simFrame"].is_string(), altium: sources.altium, page, diff: pages::diff_on(session, sel),
            present: &present,
        };
        let has = json!({"board":input.board, "schematic":input.schematic, "mech":input.mech, "gerbers":input.gerbers});
        let rail_val = rail::rail(input);
        v["rail"] = rail_val;
        v["has"] = has;
    }
    if let identity @ Value::Object(_) = header_identity(st, session, &v, &root, &sources, &shown) { v["identity"] = identity; }
    Some(v)
}
#[path="viewer_identity.rs"]
mod viewer_identity;

/// Gather source facts for the page, then format them without filesystem access.
fn header_identity(st: &State, session: &Value, viewer: &Value, root: &str, rail: &RailSources, shown: &ShownGerbers) -> Value {
    let sel = s(session,"sel");
    let gerbers = viewer["gerbers"] == true;
    let page = if pages::diff_on(session,sel) { "diff" } else if gerbers { "gerbers" } else { s(viewer,"open") };
    if page.is_empty() { return Value::Null }
    let far = sel.contains('~');
    let draft = |key: &str| s(&session["drafts"],&format!("{key}:{sel}"));
    let file = working_file(draft("@view.file"));
    let sheet = draft("@view.sheet");
    let selected = chosen_board(&session["drafts"], sel, &root);
    let context = super::identity_context::facts(st, session, sel, &rail.docs);
    let docs = if far { &context } else { &rail.docs };
    let root_sheet = if !far { rail.root_sheet.clone() } else if selected.is_empty() { s(docs,"sheet").to_owned() } else {
        schematic_for_board(&selected)
    };
    let mut board: &str = if selected.is_empty() { s(docs,"board") } else { &selected };
    let mut schematic = root_sheet.as_str();
    if !file.is_empty() {
        match page { "board"|"3d"=>board=file, "schematic"=>schematic=file, _=>{} }
    }
    if !sheet.is_empty() { schematic=sheet; }
    let history = !gerbers && matches!(page, "board" | "schematic" | "3d" | "diff");
    let historical = if history { s(&context,"historyRel") } else { "" };
    if !historical.is_empty() {
        match page { "board" | "3d" => board = historical, "schematic" => schematic = historical, _ => {} }
    }
    let sources = viewer_identity::Sources { root:&root, board, schematic, root_sheet:&root_sheet,
        has_board:docs["hasBoard"].as_bool(), has_sheet:docs["hasSheet"].as_bool(), has_set:shown.has,
        file, sheet, directory:draft("@files.dir"), set:&shown.path, made_at:shown.made_at };
    let mut id = viewer_identity::identity(page,viewer,&sources,&|rel| {
        if !historical.is_empty() && historical == rel { return Some(true) }
        if page == "gerbers" { return shown.has }
        if docs["board"] == rel { return docs["hasBoard"].as_bool() }
        if docs["sheet"] == rel { return docs["hasSheet"].as_bool() }
        if far { return super::identity_context::far_exists(sel,rel) }
        if page == "mech" { return Some(viewer["mech"]["parts"].as_array()?.iter().any(|p| p["value"] == rel)) }
        if page == "files" { return (viewer["files"]["ready"] == true).then(|| viewer["files"]["error"] == "") }
        // fm-open checked the local file when it selected this preview.
        if matches!(page, "text" | "image" | "pdf") { return Some(viewer["file"]["name"].is_string()) }
        None
    });
    if history && !s(&context,"version").is_empty() { id["version"] = context["version"].clone(); }
    if history && context["comparison"].is_object() {
        let pair = &context["comparison"];
        id["scope"] = json!(format!("{} → {}",s(pair,"from"),s(pair,"to")));
    }
    if page == "browser" {
        for (value,field) in [("browserTitle","title"),("browserUrl","url")] {
            if !s(&context,value).is_empty() { id[field] = context[value].clone(); }
        }
    }
    id
}

/// Outcome of a `layout-*` or `viewer_show_action` call.
pub enum LayoutOut {
    Ok(ViewOut),
    Refused {
        code: &'static str,
        key: &'static str,
        text: &'static str,
    },
}

fn parse_stored_layout(drafts: &Value, sel: &str) -> Option<super::layout::Layout> {
    let raw = s(drafts, &format!("@view.layout:{sel}"));
    if raw.is_empty() {
        return None;
    }
    serde_json::from_str::<super::layout::Layout>(raw).ok()
}

fn is_exact_split_preset(layout: &super::layout::Layout) -> bool {
    use super::layout::{Axis, Node, PageKind};
    let Some(Node::Split { axis: Axis::Row, a, b, .. }) = &layout.root else {
        return false;
    };
    let Node::Tile(sch) = a.as_ref() else {
        return false;
    };
    let Node::Split { axis: Axis::Col, a: pcb_node, b: solid_node, .. } = b.as_ref() else {
        return false;
    };
    let (Node::Tile(pcb), Node::Tile(solid)) = (pcb_node.as_ref(), solid_node.as_ref()) else {
        return false;
    };
    sch.kind == PageKind::Schematic
        && pcb.kind == PageKind::Board
        && solid.kind == PageKind::Solid3d
        && sch.version.is_none()
        && sch.overlay.is_none()
        && pcb.version.is_none()
        && pcb.overlay.is_none()
        && solid.version.is_none()
        && solid.overlay.is_none()
        && pcb.source == solid.source
}

/// Resolve the hub's KiCad guard for this session on `root`/`picked_board_abs` from live `embed`
/// runtime state + `session_info`. Non-holders cannot influence `locked`, `kicad_tile`, or `client_has_kicad`.
fn hub_kicad_guard(
    layout: &super::layout::Layout,
    root: &str,
    picked_board_abs: &str,
    session_info: &Value,
    client_id: Option<u64>,
    client_locked: Option<&str>,
    client_kicad_tile: Option<&str>,
    linked: bool,
) -> super::layout::KicadGuard {
    let (hub_rt, hub_dialog_open) = if !linked && !root.is_empty() {
        let rt = crate::files::kicad::embed::runtime_picture(root, picked_board_abs);
        let pro = crate::files::kicad::embed::resolve_pro(if picked_board_abs.is_empty() { root } else { picked_board_abs }).ok();
        let dlg = pro
            .as_deref()
            .map(crate::files::kicad::embed::session_state)
            .is_some_and(|st| st["dialogs"].as_array().is_some_and(|ds| !ds.is_empty()));
        (Some(rt), dlg)
    } else {
        (None, false)
    };
    super::layout::resolve_kicad_guard(
        layout,
        hub_rt.as_ref(),
        hub_dialog_open,
        session_info,
        client_id,
        client_locked,
        client_kicad_tile,
    )
}

/// Build a project-scoped `VersionSourceMap` for `root` (`.git` and `.backplane/history.git`).
fn project_version_resolver(root: &str) -> super::layout::VersionSourceMap {
    let mut map = super::layout::VersionSourceMap::new();
    let top = root.trim_end_matches('/');
    if !top.is_empty() {
        map.register(format!("{top}/.git"), top);
        map.register(format!("{top}/.backplane/history.git"), top);
    }
    map
}

/// The ONE writer for per-session `@view.layout:<sel>`, its legacy `@view:<sel>` projection,
/// Diff/Review `pages.rs` transitions, and the focused tile's `kicad.watch` transition + `@view.at`.
///
/// When `geometry_only` is `true` (`layout-resize`, `layout-maximize`, `layout-restore`, or a
/// structural move/swap that keeps the focused tile identical), emits ZERO watch commands and
/// does NOT call `pages::leave` (so resizing with Diff or Review up never dismisses it).
fn commit_layout(
    st: &State,
    hub: &str,
    session: &Value,
    sel: &str,
    root: &str,
    next: &super::layout::Layout,
    geometry_only: bool,
) -> ViewOut {
    use super::layout::{OverlayMode, PageKind};
    let d = &session["drafts"];
    let mut drafts = serde_json::Map::new();
    let mut rid = session["rid"].as_u64().unwrap_or(0);
    let mut before = Vec::new();
    let abs = |rel: &str| if rel.is_empty() || root.is_empty() { String::new() } else { format!("{}/{rel}", root.trim_end_matches('/')) };

    if next.is_empty() {
        drafts.insert(format!("@view.layout:{sel}"), json!(""));
        drafts.insert(format!("@view:{sel}"), json!(""));
        drafts.insert(format!("@view.file:{sel}"), json!(""));
        drafts.insert(format!("@view.gerbers:{sel}"), json!(""));
        drafts.insert(format!("@view.sheet:{sel}"), json!(""));
        for k in ["@pick", "@card", "@pick.rid"] { drafts.insert(k.into(), json!("")); }
        let at = s(d, "@view.at");
        if !at.is_empty() {
            let on = if sel.contains('~') { format!(",\"on\":{}", jstr(sel.split('~').next().unwrap_or(""))) } else { String::new() };
            let p = format!("{{\"kind\":\"\",\"root\":{},\"path\":\"\",\"tree\":\"\"{on}}}", jstr(root));
            before.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"kicad.watch\",\"p\":{p}}}")}));
            rid += 1;
            drafts.insert("@view.at".into(), json!(""));
        }
        let mut patch = json!({"rid": rid, "drafts": drafts});
        patch::merge(&mut patch, &pages::leave(session, sel));
        return ViewOut { patch, before, boards_look: false };
    }

    drafts.insert(format!("@view.layout:{sel}"), json!(serde_json::to_string(next).unwrap_or_default()));
    if geometry_only {
        return ViewOut { patch: json!({"rid": rid, "drafts": drafts}), before: vec![], boards_look: false };
    }

    for k in ["@pick", "@card", "@pick.rid"] { drafts.insert(k.into(), json!("")); }

    // 1. Exact Split preset
    if is_exact_split_preset(next) {
        let ts = next.tiles();
        let (sch_t, pcb_t) = (ts[0], ts[1]);
        let sheet_abs = if sch_t.explicit && !sch_t.source.is_empty() { abs(&sch_t.source) } else { String::new() };
        let board_abs = if pcb_t.explicit && !pcb_t.source.is_empty() { abs(&pcb_t.source) } else { String::new() };
        drafts.insert(format!("@view:{sel}"), json!("split"));
        drafts.insert(format!("@view.gerbers:{sel}"), json!(""));
        drafts.insert(format!("@view.file:{sel}"), json!(""));
        drafts.insert(format!("@view.sheet:{sel}"), json!(sheet_abs));
        if pcb_t.explicit && !pcb_t.source.is_empty() {
            drafts.insert(format!("@view.board:{sel}"), json!(pcb_t.source));
        }
        let base = key("split", root);
        let want = if base.is_empty() { base } else if sheet_abs.is_empty() { format!("{base}{board_abs}") } else { format!("{base}{board_abs}|{sheet_abs}") };
        let at = s(d, "@view.at");
        if want != at {
            let on = if sel.contains('~') { format!(",\"on\":{}", jstr(sel.split('~').next().unwrap_or(""))) } else { String::new() };
            let bd = if board_abs.is_empty() { String::new() } else { format!(",\"board\":{}", jstr(&board_abs)) };
            let p = format!("{{\"kind\":{},\"root\":{},\"path\":{},\"tree\":\"\"{bd}{on}}}", jstr(if want.is_empty() { "" } else { "split" }), jstr(root), jstr(&sheet_abs));
            before.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"kicad.watch\",\"p\":{p}}}")}));
            rid += 1;
            drafts.insert("@view.at".into(), json!(want));
        }
        let mut patch = json!({"rid": rid, "drafts": drafts});
        patch::merge(&mut patch, &pages::leave(session, sel));
        return ViewOut { patch, before, boards_look: false };
    }

    let Some(ft) = next.focus.as_deref().and_then(|id| next.tile(id)).or_else(|| next.tiles().into_iter().next()) else {
        return ViewOut { patch: json!({"rid": rid, "drafts": drafts}), before, boards_look: false };
    };

    // 2. Review & Diff pages (pages.rs state) enter through the same writer without `pages::leave`
    if ft.kind == PageKind::Review {
        let (mut p_rev, mut rev_cmds) = pages::open_review(session, hub, sel, "bom", !sel.contains('~') && has_management(root));
        if let Some(pd) = p_rev["drafts"].as_object_mut() {
            pd.insert(format!("@view.layout:{sel}"), json!(serde_json::to_string(next).unwrap_or_default()));
        }
        let at = s(d, "@view.at");
        if !at.is_empty() {
            let p = format!("{{\"kind\":\"\",\"root\":{},\"path\":\"\",\"tree\":\"\"}}", jstr(root));
            rev_cmds.insert(0, json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"kicad.watch\",\"p\":{p}}}")}));
            if let Some(pd) = p_rev["drafts"].as_object_mut() { pd.insert("@view.at".into(), json!("")); }
        }
        return ViewOut { patch: p_rev, before: rev_cmds, boards_look: false };
    }
    if ft.kind == PageKind::Diff {
        drafts.insert(format!("@view:{sel}"), json!(""));
        drafts.insert(format!("@view.gerbers:{sel}"), json!(""));
        drafts.insert(format!("@view.file:{sel}"), json!(""));
        let at = s(d, "@view.at");
        if !at.is_empty() {
            let on = if sel.contains('~') { format!(",\"on\":{}", jstr(sel.split('~').next().unwrap_or(""))) } else { String::new() };
            let p = format!("{{\"kind\":\"\",\"root\":{},\"path\":\"\",\"tree\":\"\"{on}}}", jstr(root));
            before.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"kicad.watch\",\"p\":{p}}}")}));
            rid += 1;
            drafts.insert("@view.at".into(), json!(""));
        }
        let patch = json!({
            "rid": rid,
            "panel": true,
            "info": { format!("view.kind:{sel}"): "diff", format!("panel:{sel}"): "1" },
            "drafts": drafts
        });
        return ViewOut { patch, before, boards_look: false };
    }

    // 3. Drawings, Gerbers, Mech, Files, Browser, Simulation
    let (want, wkind, wpath): (String, &str, String) = match ft.kind {
        PageKind::Board if ft.overlay.as_ref().is_some_and(|o| o.kind == PageKind::Gerbers) => {
            let ov = ft.overlay.as_ref().unwrap();
            let set_abs = abs(&ov.source);
            let board_abs = abs(&ft.source);
            let (file, w, wk, wp) = match ov.mode {
                OverlayMode::Diff => {
                    if board_abs.is_empty() {
                        (if set_abs.is_empty() { String::new() } else { format!("noboard\n{set_abs}") }, String::new(), "", String::new())
                    } else {
                        let cmp = crate::files::kicad::gerber::cmp_path(&set_abs, &board_abs);
                        (cmp.clone(), format!("0|{root}|{cmp}"), "board", cmp)
                    }
                }
                OverlayMode::Old => (set_abs.clone(), if set_abs.is_empty() { String::new() } else { format!("0|{root}|{set_abs}") }, if set_abs.is_empty() { "" } else { "board" }, set_abs),
                OverlayMode::New => {
                    let wp = if ft.explicit { board_abs.clone() } else { String::new() };
                    (wp.clone(), format!("0|{root}|{wp}"), "board", wp)
                }
            };
            drafts.insert(format!("@view:{sel}"), json!("board"));
            drafts.insert(format!("@view.gerbers:{sel}"), json!("1"));
            drafts.insert(format!("@view.file:{sel}"), json!(file));
            drafts.insert(format!("@view.sheet:{sel}"), json!(""));
            (w, wk, wp)
        }
        PageKind::Gerbers => {
            let set_abs = abs(&ft.source);
            drafts.insert(format!("@view:{sel}"), json!("board"));
            drafts.insert(format!("@view.gerbers:{sel}"), json!("1"));
            drafts.insert(format!("@view.file:{sel}"), json!(set_abs));
            drafts.insert(format!("@view.sheet:{sel}"), json!(""));
            if set_abs.is_empty() || root.is_empty() {
                (String::new(), "", String::new())
            } else {
                (format!("0|{root}|{set_abs}"), "board", set_abs)
            }
        }
        PageKind::Mech => {
            let ps = parts_of(root, sel, &ft.source);
            let (w, a) = mech_want(root, &ps, &ft.source);
            drafts.insert(format!("@view:{sel}"), json!("mech"));
            drafts.insert(format!("@view.gerbers:{sel}"), json!(""));
            drafts.insert(format!("@view.file:{sel}"), json!(""));
            drafts.insert(format!("@view.sheet:{sel}"), json!(""));
            drafts.insert(format!("@mech.part:{sel}"), json!(ft.source));
            (w.clone(), if w.is_empty() { "" } else { "3d" }, a)
        }
        PageKind::Files => {
            let preview_k = if !ft.source.is_empty() { crate::files::fm::preview(crate::files::fm::last(&ft.source)) } else { "" };
            drafts.insert(format!("@view.gerbers:{sel}"), json!(""));
            drafts.insert(format!("@view.sheet:{sel}"), json!(""));
            if !preview_k.is_empty() {
                drafts.insert(format!("@view:{sel}"), json!(preview_k));
                drafts.insert(format!("@view.file:{sel}"), json!(abs(&ft.source)));
                drafts.insert(format!("@files.dir:{sel}"), json!(crate::files::fm::parent(&ft.source)));
            } else {
                drafts.insert(format!("@view:{sel}"), json!("files"));
                drafts.insert(format!("@files.dir:{sel}"), json!(ft.source));
                drafts.insert(format!("@view.file:{sel}"), json!(""));
            }
            (String::new(), "", String::new())
        }
        PageKind::Schematic => {
            let wp = if ft.explicit { abs(&ft.source) } else { String::new() };
            drafts.insert(format!("@view:{sel}"), json!("schematic"));
            drafts.insert(format!("@view.gerbers:{sel}"), json!(""));
            drafts.insert(format!("@view.sheet:{sel}"), json!(wp));
            drafts.insert(format!("@view.file:{sel}"), json!(""));
            let w = if root.is_empty() { String::new() } else { format!("1|{root}|{wp}") };
            (w.clone(), if w.is_empty() { "" } else { "schematic" }, wp)
        }
        PageKind::Board => {
            let wp = if ft.explicit { abs(&ft.source) } else { String::new() };
            drafts.insert(format!("@view:{sel}"), json!("board"));
            drafts.insert(format!("@view.gerbers:{sel}"), json!(""));
            drafts.insert(format!("@view.file:{sel}"), json!(wp));
            drafts.insert(format!("@view.sheet:{sel}"), json!(""));
            let w = if root.is_empty() { String::new() } else { format!("0|{root}|{wp}") };
            (w.clone(), if w.is_empty() { "" } else { "board" }, wp)
        }
        PageKind::Solid3d => {
            let wp = if ft.explicit { abs(&ft.source) } else { String::new() };
            drafts.insert(format!("@view:{sel}"), json!("3d"));
            drafts.insert(format!("@view.gerbers:{sel}"), json!(""));
            drafts.insert(format!("@view.file:{sel}"), json!(wp));
            drafts.insert(format!("@view.sheet:{sel}"), json!(""));
            let w = if root.is_empty() { String::new() } else { format!("2|{root}|{wp}") };
            (w.clone(), if w.is_empty() { "" } else { "3d" }, wp)
        }
        k => {
            drafts.insert(format!("@view:{sel}"), json!(k.as_str()));
            drafts.insert(format!("@view.gerbers:{sel}"), json!(""));
            drafts.insert(format!("@view.file:{sel}"), json!(""));
            drafts.insert(format!("@view.sheet:{sel}"), json!(""));
            (String::new(), "", String::new())
        }
    };

    let at = s(d, "@view.at");
    if want != at {
        let on = if sel.contains('~') { format!(",\"on\":{}", jstr(sel.split('~').next().unwrap_or(""))) } else { String::new() };
        let p = format!("{{\"kind\":{},\"root\":{},\"path\":{},\"tree\":\"\"{on}}}", jstr(if want.is_empty() { "" } else { wkind }), jstr(root), jstr(&wpath));
        before.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"kicad.watch\",\"p\":{p}}}")}));
        rid += 1;
        drafts.insert("@view.at".into(), json!(want));
    }
    let mut patch = json!({"rid": rid, "drafts": drafts});
    patch::merge(&mut patch, &pages::leave(session, sel));
    ViewOut { patch, before, boards_look: false }
}

/// Build the per-tile `identity` object by reusing `viewer_identity::identity` (PR #1411)
/// with already-known tile & project facts (zero filesystem access).
fn tile_identity(
    tile: &super::layout::Tile,
    root: &str,
    viewer: &Value,
    sources: Option<&RailSources>,
    shown: &ShownGerbers,
    source_missing: bool,
) -> Value {
    use super::layout::PageKind;
    let is_gvb = tile.kind == PageKind::Board
        && tile.overlay.as_ref().is_some_and(|o| o.kind == PageKind::Gerbers);
    let page = if is_gvb { "gerbers" } else { tile.kind.as_str() };
    let rel = if is_gvb {
        tile.overlay.as_ref().map(|o| o.source.as_str()).unwrap_or("")
    } else {
        tile.source.as_str()
    };
    let root_sheet = sources.map(|s| s.root_sheet.as_str()).unwrap_or("");
    let made_at = shown.made_at.or_else(|| {
        sources
            .and_then(|s| s.newest.as_ref())
            .and_then(|(_, t)| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
    });
    let id_sources = viewer_identity::Sources {
        root,
        board: if matches!(page, "board" | "3d") { rel } else { "" },
        schematic: if matches!(page, "schematic" | "simulation") { rel } else { "" },
        file: if matches!(page, "image" | "pdf" | "text") { rel } else { "" },
        sheet: if page == "schematic" && tile.explicit { rel } else { "" },
        directory: if page == "files" { rel } else { "" },
        root_sheet,
        has_board: sources.map(|s| s.board),
        has_sheet: sources.map(|s| s.schematic),
        has_set: if source_missing && page == "gerbers" { Some(false) } else { shown.has.or_else(|| sources.map(|s| s.gerbers)) },
        set: if page == "gerbers" { rel } else { "" },
        made_at: if source_missing { None } else { made_at },
    };
    let mut id = viewer_identity::identity(page, viewer, &id_sources, &|r| {
        if r.is_empty() || source_missing { return Some(false) }
        if tile.version.is_some() { return Some(true) }
        if let Some(src) = sources {
            if page == "gerbers" { return Some(src.gerbers && !source_missing) }
            if src.docs["board"] == r { return src.docs["hasBoard"].as_bool() }
            if src.docs["sheet"] == r { return src.docs["hasSheet"].as_bool() }
        }
        Some(true)
    });
    if let Some(ver) = &tile.version {
        id["version"] = json!(ver.label);
    }
    if let Some(ov) = &tile.overlay {
        if let (Some(ov_ver), Some(base_ver)) = (&ov.version, &tile.version) {
            id["scope"] = json!(format!("{} → {}", ov_ver.label, base_ver.label));
        }
    }
    id
}

/// Build `viewer.tiles`: per-tile map keyed by `TileId` carrying today's page values plus `identity`
/// (from `viewer_identity::identity`).
///
/// - Versioned tiles and history overlays resolve through `layout::plan_watches` + `VersionSourceMap`;
///   an unresolved or malformed version renders as the tile's error state (`error`, `key: ""`), never live artwork.
/// - Vanished Gerber sets show the Gerbers empty state and `identity.missing = true`.
/// - Unfocused Mech and Files tiles carry their own `rel`/`dir` without leaking new absolute paths
///   (plot `key` strings keep today's existing `kind|root|path` wire format).
fn build_tiles_picture(
    layout: &super::layout::Layout,
    root: &str,
    viewer: &Value,
    drafts: &Value,
    sources: Option<&RailSources>,
    shown: &ShownGerbers,
    linked: bool,
) -> Value {
    use super::layout::{self, PageKind};
    let mut map = serde_json::Map::new();
    let probe = s(viewer, "probe");
    let default_pcb = sources.map(|s| s.pcb.as_str()).unwrap_or("");
    let default_sch = sources.map(|s| s.sch.as_str()).unwrap_or("");
    let default_gerber_rel = sources.map(|s| layout::sanitize_rel(root, &s.gerber_set)).unwrap_or_default();
    let watch_ctx = layout::WatchContext {
        root: root.to_string(),
        default_pcb: default_pcb.to_string(),
        default_sch: default_sch.to_string(),
    };
    let resolver = project_version_resolver(root);

    for tile in layout.tiles() {
        let mut o = serde_json::Map::new();
        o.insert("id".into(), json!(tile.id));
        o.insert("kind".into(), json!(tile.kind.as_str()));
        o.insert("source".into(), json!(tile.source));
        if let Some(v) = &tile.version {
            o.insert("version".into(), serde_json::to_value(v).unwrap_or(Value::Null));
        }
        if let Some(ov) = &tile.overlay {
            o.insert("overlay".into(), serde_json::to_value(ov).unwrap_or(Value::Null));
        }
        let is_gvb = tile.kind == PageKind::Board
            && tile.overlay.as_ref().is_some_and(|ov| ov.kind == PageKind::Gerbers);
        let is_gerbers = tile.kind == PageKind::Gerbers || is_gvb;
        let is_versioned = tile.unresolved_version
            || tile.version.is_some()
            || tile.overlay.as_ref().is_some_and(|ov| ov.version.is_some());

        // Check vanished Gerber set or missing explicit source (using already-known `sources` facts)
        let gerber_vanished = !linked && is_gerbers && sources.is_some_and(|src| {
            let set_rel = if is_gvb {
                tile.overlay.as_ref().map(|ov| ov.source.as_str()).unwrap_or("")
            } else {
                tile.source.as_str()
            };
            !src.gerbers || (!set_rel.is_empty() && !default_gerber_rel.is_empty() && set_rel != default_gerber_rel && shown.path != format!("{}/{set_rel}", root.trim_end_matches('/')))
        });
        let explicit_missing = !linked && tile.explicit && !tile.source.is_empty() && sources.is_some_and(|src| {
            match tile.kind {
                PageKind::Board | PageKind::Solid3d => src.docs["board"] == tile.source && src.docs["hasBoard"] == false,
                PageKind::Schematic => src.docs["sheet"] == tile.source && src.docs["hasSheet"] == false,
                _ => false,
            }
        });
        let source_missing = gerber_vanished || explicit_missing;

        let mut version_error = false;
        let plot_key = if is_versioned {
            if let Ok(single) = layout::Layout::single(tile.clone()) {
                match layout::plan_watches(&single, &watch_ctx, &resolver) {
                    Ok(specs) => specs.first().map(|sp| sp.key.clone()).unwrap_or_default(),
                    Err(_) => {
                        version_error = true;
                        String::new()
                    }
                }
            } else {
                version_error = true;
                String::new()
            }
        } else if source_missing {
            String::new()
        } else if let Ok(single) = layout::Layout::single(tile.clone()) {
            layout::plan_watches(&single, &watch_ctx, &resolver)
                .ok()
                .and_then(|specs| specs.into_iter().next().map(|sp| sp.key))
                .unwrap_or_default()
        } else {
            String::new()
        };

        if version_error {
            o.insert("error".into(), json!("Unresolved version"));
        }
        o.insert("key".into(), json!(plot_key));
        let layers_key = if tile.kind == PageKind::Solid3d {
            if version_error || source_missing { String::new() }
            else if !root.is_empty() && !tile.explicit && tile.version.is_none() { format!("0|{root}|") }
            else { String::new() }
        } else {
            plot_key.clone()
        };
        o.insert("layers".into(), json!(layers_key));

        if matches!(tile.kind, PageKind::Board | PageKind::Schematic | PageKind::Solid3d | PageKind::Gerbers) {
            o.insert("probe".into(), json!(probe));
        }
        if matches!(tile.kind, PageKind::Board | PageKind::Schematic | PageKind::Gerbers) {
            let lkind = if tile.kind == PageKind::Schematic { "schematic" } else { "board" };
            let off: u32 = s(drafts, &format!("@view.off.{lkind}")).parse().unwrap_or(0);
            let f = Focus::read(s(drafts, &format!("@view.focus.{lkind}")));
            layer_fields(&mut o, lkind, off, f);
            if is_gerbers {
                o.insert("gerbers".into(), json!(true));
                o.insert("gerbersVsBoard".into(), json!(is_gvb));
                let rows: Vec<Value> = GERBER_LAYERS.iter().chain(GERBER_MORE.iter())
                    .map(|(n, name)| json!({"layer": n, "name": name, "on": off & bit(*n) == 0, "active": f.on() && f.active == *n}))
                    .collect();
                o.insert("layerList".into(), Value::Array(rows));
            } else if tile.kind == PageKind::Board {
                o.insert("gerbers".into(), json!(false));
            }
        }
        if tile.kind == PageKind::Mech {
            if viewer["mech"].is_object() && s(&viewer["mech"], "rel") == tile.source {
                o.insert("mech".into(), viewer["mech"].clone());
            } else {
                o.insert("mech".into(), json!({"rel": tile.source, "name": stem(&tile.source)}));
            }
        }
        if tile.kind == PageKind::Files {
            if viewer["files"].is_object() && s(&viewer["files"], "dir") == tile.source {
                o.insert("files".into(), viewer["files"].clone());
            } else {
                o.insert("files".into(), json!({"dir": tile.source, "trail": files_trail(&tile.source)}));
            }
        }
        if !linked {
            if gerber_vanished {
                o.insert("empty".into(), json!("No Gerber files in this project yet"));
                o.insert("emptyNext".into(), json!("Ask an agent to export fabrication files into mfg/."));
            } else if let Some(src) = sources {
                match tile.kind {
                    PageKind::Board if !is_gvb && !src.board && tile.source.is_empty() => {
                        o.insert("empty".into(), json!("No board in this project yet"));
                        o.insert("emptyNext".into(), json!("Ask an agent to lay one out."));
                    }
                    PageKind::Board if is_gvb && tile.source.is_empty() => {
                        o.insert("empty".into(), json!("No board to compare these Gerbers against."));
                        o.insert("emptyNext".into(), json!("Add a board to this project to compare them."));
                    }
                    PageKind::Schematic if !src.schematic && tile.source.is_empty() => {
                        o.insert("empty".into(), json!("No schematic in this project yet"));
                        o.insert("emptyNext".into(), json!("Ask an agent to draw one."));
                    }
                    PageKind::Solid3d if !src.board && tile.source.is_empty() => {
                        o.insert("empty".into(), json!("No board in this project yet"));
                        o.insert("emptyNext".into(), json!("Ask an agent to lay one out."));
                    }
                    PageKind::Gerbers if !src.gerbers && tile.source.is_empty() => {
                        o.insert("empty".into(), json!("No Gerber files in this project yet"));
                        o.insert("emptyNext".into(), json!("Ask an agent to export fabrication files into mfg/."));
                    }
                    _ => {}
                }
            }
        }
        o.insert("identity".into(), tile_identity(tile, root, viewer, sources, shown, source_missing));
        map.insert(tile.id.clone(), Value::Object(o));
    }
    Value::Object(map)
}

fn parse_side(s: &str) -> Option<super::layout::Side> {
    match s.to_ascii_lowercase().as_str() {
        "left" => Some(super::layout::Side::Left),
        "right" => Some(super::layout::Side::Right),
        "top" => Some(super::layout::Side::Top),
        "bottom" => Some(super::layout::Side::Bottom),
        _ => None,
    }
}

fn parse_overlay_mode(s: &str) -> Option<super::layout::OverlayMode> {
    match s.to_ascii_lowercase().as_str() {
        "old" => Some(super::layout::OverlayMode::Old),
        "diff" => Some(super::layout::OverlayMode::Diff),
        "new" => Some(super::layout::OverlayMode::New),
        _ => None,
    }
}

/// Thin hub action handler over `present::layout` pure operations (per-session `@view.layout:<sel>`).
///
/// Handles: `layout-add`, `layout-close`, `layout-focus`, `layout-resize`, `layout-swap`,
/// `layout-move`, `layout-overlay`, `layout-unsplit`, `layout-maximize`, `layout-restore`.
/// Every malformed JSON, kind, side, mode, or field returns `LayoutOut::Refused` (`invalidInput`)
/// localized for `locale` with layout, drafts, and watches untouched.
/// `layout-resize` is geometry-only (never changes watches and never dismisses Diff/Review).
pub fn layout_action(
    st: &State,
    hub: &str,
    session: &Value,
    client_id: Option<u64>,
    locale: &str,
    action: &str,
    value: &str,
) -> Option<LayoutOut> {
    use super::layout::{self, Overlay, PageKind, Refusal, Tile, TileVersion};
    let sel = s(session, "sel");
    if sel.is_empty() || !st.threads.contains_key(sel) {
        return None;
    }
    let refuse = |r: Refusal| Some(LayoutOut::Refused { code: r.code(), key: r.i18n_key(), text: r.text_for(locale) });

    let payload: Value = if value.trim().is_empty() {
        json!({})
    } else {
        match serde_json::from_str::<Value>(value) {
            Ok(v) if v.is_object() => v,
            _ => return refuse(Refusal::InvalidInput),
        }
    };
    let linked = sel.contains('~');
    let root = if linked {
        st.threads.get(sel).map(|t| if !t.worktree.is_empty() { t.worktree.clone() } else { st.projects.get(&t.project).map(|p| p.root.clone()).unwrap_or_default() }).unwrap_or_default()
    } else {
        root_of(st, sel)
    };
    let drafts = &session["drafts"];
    let picked_board = chosen_board(drafts, sel, &root);
    let sources = if linked { RailSources::default() } else { rail_sources_for(&root, &picked_board, s(drafts, &format!("@view:{sel}")), s(drafts, &format!("@view.file:{sel}")), s(drafts, &format!("@view.sheet:{sel}"))) };
    let cur = parse_stored_layout(drafts, sel).unwrap_or_else(|| {
        let picked_sch = if picked_board.is_empty() || linked {
            String::new()
        } else {
            sources.root_sheet.clone()
        };
        layout::derive_layout(&layout::DeriveInput {
            root: &root,
            open: s(drafts, &format!("@view:{sel}")),
            diff_on: pages::diff_on(session, sel),
            gerbers: s(drafts, &format!("@view.gerbers:{sel}")) == "1",
            gerbers_vs_board: {
                let f = s(drafts, &format!("@view.file:{sel}"));
                f.starts_with(crate::files::kicad::gerber::CMP) || f.starts_with("noboard\n")
            },
            file: s(drafts, &format!("@view.file:{sel}")),
            sheet: s(drafts, &format!("@view.sheet:{sel}")),
            picked_board: &picked_board,
            picked_sch: &picked_sch,
            mech_rel: s(drafts, &format!("@mech.part:{sel}")),
            files_dir: s(drafts, &format!("@files.dir:{sel}")),
            default_pcb: &sources.pcb,
            default_sch: &sources.sch,
            default_gerber_set: &sources.gerber_set,
        })
    });

    let picked_abs = board_file("board", &root, &picked_board);
    let guard = hub_kicad_guard(
        &cur,
        &root,
        &picked_abs,
        &session["info"],
        client_id,
        payload["locked"].as_str(),
        payload["kicadTile"].as_str(),
        linked,
    );
    let locked_ref = guard.locked.as_deref();
    let kicad_tile_ref = guard.kicad_tile.as_deref();

    let mut geometry_only = false;
    let res = match action {
        "layout-add" => {
            let Some(kind) = payload["kind"].as_str().and_then(PageKind::from_page_str) else {
                return refuse(Refusal::InvalidInput);
            };
            let side = match payload.get("side") {
                None | Some(Value::Null) => layout::Side::Right,
                Some(v) => match v.as_str().and_then(parse_side) {
                    Some(s) => s,
                    None => return refuse(Refusal::InvalidInput),
                },
            };
            let target = match payload.get("target") {
                None | Some(Value::Null) => None,
                Some(v) => match v.as_str() {
                    Some(s) if !s.is_empty() => Some(s),
                    Some(_) => None,
                    None => return refuse(Refusal::InvalidInput),
                },
            };
            let explicit_source = match payload.get("source") {
                None | Some(Value::Null) => "",
                Some(v) => match v.as_str() {
                    Some(s) => s,
                    None => return refuse(Refusal::InvalidInput),
                },
            };
            let source = if !explicit_source.is_empty() {
                explicit_source.to_string()
            } else {
                match kind {
                    PageKind::Board | PageKind::Solid3d => layout::sanitize_rel(&root, &sources.pcb),
                    PageKind::Schematic | PageKind::Simulation => layout::sanitize_rel(&root, &sources.sch),
                    PageKind::Gerbers => layout::sanitize_rel(&root, &sources.gerber_set),
                    _ => String::new(),
                }
            };
            let version: Option<TileVersion> = match payload.get("version") {
                None | Some(Value::Null) => None,
                Some(v) => match serde_json::from_value(v.clone()) {
                    Ok(ver) => Some(ver),
                    Err(_) => return refuse(Refusal::InvalidInput),
                },
            };
            let tile = Tile {
                id: cur.next_tile_id(),
                kind,
                source,
                explicit: !explicit_source.is_empty(),
                version,
                unresolved_version: false,
                overlay: None,
            };
            layout::add(&cur, target, side, tile, guard.client_has_kicad, locked_ref)
        }
        "layout-close" => {
            let Some(tile) = payload["tile"].as_str().filter(|s| !s.is_empty()) else {
                return refuse(Refusal::InvalidInput);
            };
            layout::close(&cur, tile, locked_ref)
        }
        "layout-focus" => {
            let Some(tile) = payload["tile"].as_str().filter(|s| !s.is_empty()) else {
                return refuse(Refusal::InvalidInput);
            };
            layout::focus_tile(&cur, tile, locked_ref)
        }
        "layout-resize" => {
            geometry_only = true;
            let Some(split) = payload["split"].as_str().filter(|s| !s.is_empty()) else {
                return refuse(Refusal::InvalidInput);
            };
            let Some(ratio) = payload["ratio"].as_f64().map(|r| r as f32) else {
                return refuse(Refusal::InvalidInput);
            };
            let Some(bounds) = payload["bounds"].as_array().and_then(|a| {
                if a.len() == 2 { Some((a[0].as_f64()? as f32, a[1].as_f64()? as f32)) } else { None }
            }) else {
                return refuse(Refusal::InvalidInput);
            };
            layout::resize(&cur, split, ratio, bounds, layout::default_min_for(kicad_tile_ref), locked_ref)
        }
        "layout-swap" => {
            let (Some(a), Some(b)) = (payload["a"].as_str().filter(|s| !s.is_empty()), payload["b"].as_str().filter(|s| !s.is_empty())) else {
                return refuse(Refusal::InvalidInput);
            };
            geometry_only = true; // swap preserves focus id and tile payload
            layout::swap(&cur, a, b, locked_ref)
        }
        "layout-move" => {
            let (Some(tile), Some(target)) = (payload["tile"].as_str().filter(|s| !s.is_empty()), payload["target"].as_str().filter(|s| !s.is_empty())) else {
                return refuse(Refusal::InvalidInput);
            };
            let side = match payload.get("side") {
                None | Some(Value::Null) => layout::Side::Right,
                Some(v) => match v.as_str().and_then(parse_side) {
                    Some(s) => s,
                    None => return refuse(Refusal::InvalidInput),
                },
            };
            let prev_focus = cur.focus.clone();
            let r = layout::move_tile(&cur, tile, target, side, locked_ref);
            if let Ok(ref next) = r {
                geometry_only = next.focus == prev_focus;
            }
            r
        }
        "layout-overlay" => {
            let Some(base) = payload["base"].as_str().or_else(|| payload["a"].as_str()).filter(|s| !s.is_empty()) else {
                return refuse(Refusal::InvalidInput);
            };
            let mode = match payload.get("mode") {
                None | Some(Value::Null) => layout::OverlayMode::Diff,
                Some(v) => match v.as_str().and_then(parse_overlay_mode) {
                    Some(m) => m,
                    None => return refuse(Refusal::InvalidInput),
                },
            };
            let ov_val = payload.get("overlay").or_else(|| payload.get("b"));
            match ov_val {
                Some(Value::String(ov_tile)) if !ov_tile.is_empty() => {
                    layout::merge_overlay(&cur, base, ov_tile.as_str(), mode, None, None, None, locked_ref)
                }
                Some(Value::Object(_)) => match serde_json::from_value::<Overlay>(ov_val.unwrap().clone()) {
                    Ok(ov) => layout::overlay(&cur, base, Some(ov), locked_ref),
                    Err(_) => return refuse(Refusal::InvalidInput),
                },
                Some(Value::Null) => layout::overlay(&cur, base, None, locked_ref),
                _ => return refuse(Refusal::InvalidInput),
            }
        }
        "layout-unsplit" => {
            let Some(tile) = payload["tile"].as_str().filter(|s| !s.is_empty()) else {
                return refuse(Refusal::InvalidInput);
            };
            let side = match payload.get("side") {
                None | Some(Value::Null) => layout::Side::Right,
                Some(v) => match v.as_str().and_then(parse_side) {
                    Some(s) => s,
                    None => return refuse(Refusal::InvalidInput),
                },
            };
            layout::split_overlay(&cur, tile, side, None, locked_ref).map(|o| o.layout)
        }
        "layout-maximize" => {
            let Some(tile) = payload["tile"].as_str().filter(|s| !s.is_empty()) else {
                return refuse(Refusal::InvalidInput);
            };
            let prev_focus = cur.focus.clone();
            let r = layout::maximize(&cur, tile, locked_ref);
            if let Ok(ref next) = r {
                geometry_only = next.focus == prev_focus;
            }
            r
        }
        "layout-restore" => {
            geometry_only = true;
            layout::restore(&cur, locked_ref)
        }
        _ => return None,
    };

    match res {
        Err(r) => refuse(r),
        Ok(next) => Some(LayoutOut::Ok(commit_layout(st, hub, session, sel, &root, &next, geometry_only))),
    }
}

/// Route an agent's `viewer_show(kind, path)` through `layout::viewer_show` on `@view.layout:<sel>`:
/// updates and focuses an existing tile of `kind`, else replaces the focused tile, else opens a 1-tile layout.
pub fn viewer_show_action(
    st: &State,
    hub: &str,
    session: &Value,
    client_id: Option<u64>,
    locale: &str,
    kind: &str,
    path: &str,
) -> Option<LayoutOut> {
    use super::layout::{self, PageKind};
    let sel = s(session, "sel");
    if sel.is_empty() || !st.threads.contains_key(sel) {
        return None;
    }
    let pk = PageKind::from_page_str(kind)?;
    let linked = sel.contains('~');
    let root = if linked {
        st.threads.get(sel).map(|t| if !t.worktree.is_empty() { t.worktree.clone() } else { st.projects.get(&t.project).map(|p| p.root.clone()).unwrap_or_default() }).unwrap_or_default()
    } else {
        root_of(st, sel)
    };
    let drafts = &session["drafts"];
    let cur = parse_stored_layout(drafts, sel).unwrap_or_else(|| {
        let sources = if linked { RailSources::default() } else { rail_sources(&root) };
        layout::derive_layout(&layout::DeriveInput {
            root: &root,
            open: s(drafts, &format!("@view:{sel}")),
            diff_on: pages::diff_on(session, sel),
            gerbers: s(drafts, &format!("@view.gerbers:{sel}")) == "1",
            file: s(drafts, &format!("@view.file:{sel}")),
            sheet: s(drafts, &format!("@view.sheet:{sel}")),
            default_pcb: &sources.pcb,
            default_sch: &sources.sch,
            default_gerber_set: &sources.gerber_set,
            ..Default::default()
        })
    });
    let rel = layout::sanitize_rel(&root, path);
    let picked_abs = board_file("board", &root, &chosen_board(drafts, sel, &root));
    let guard = hub_kicad_guard(&cur, &root, &picked_abs, &session["info"], client_id, None, None, linked);
    match layout::viewer_show(&cur, pk, rel, guard.locked.as_deref()) {
        Err(r) => Some(LayoutOut::Refused { code: r.code(), key: r.i18n_key(), text: r.text_for(locale) }),
        Ok(next) => Some(LayoutOut::Ok(commit_layout(st, hub, session, sel, &root, &next, false))),
    }
}

/// Choose the surface for this connection and the document the frame would open.
pub fn surface(st: &State, viewer: &Value, params: &Value, thread: &str) -> Value {
    use super::design_mode::*;
    let session = &params["session"];
    let page = s(viewer, "open");
    let linked = thread.contains('~');
    let document = if !matches!(page, "board" | "schematic") || viewer["gerbers"] == true || pages::diff_on(session, thread) {
        Document::OtherPage
    } else if viewer["has"][page].as_bool().is_none() {
        Document::Unknown
    } else {
        let present = !linked && viewer["has"][page] == true;
        if present { Document::Present } else { Document::Missing }
    };
    design_mode(Input {
        desktop: params["designDesktop"].as_u64(), linked, altium: viewer["cad"] == "altium", document,
        platform: Platform::host(), preference: Preference::read(st.settings.get(SETTING).map_or("", String::as_str)),
        visit: Visit::Normal, runtime: runtime_from_picture(&viewer["runtime"]), visit_started_after_stop: false,
        frame: match page { "board" => Some(Frame::Board), "schematic" => Some(Frame::Schematic), _ => None },
    }, AUTO_PLATFORMS)
}

#[cfg(test)]
#[test]
fn frame_switch_preserves_design_mode_and_unknown_documents_stay_distinct() {
    let st = State::new();
    for page in ["board", "schematic"] {
        for (has, why) in [(json!({"board":true,"schematic":true}), "runtimeUnknown"), (Value::Null, "documentUnknown"),
            (json!({"board":false,"schematic":false}), "missingDocument")] {
            assert_eq!(surface(&st, &json!({"open":page,"has":has}), &json!({"designDesktop":2}), "thread"),
                json!({"want":"viewer","why":why}));
        }
    }
}

pub(crate) fn has_management(root: &str) -> bool {
    let files = crate::files::kicad::project::read(root);
    !root.is_empty() && !crate::files::kicad::project::is_altium(&files)
        && [files.pcb, files.sch].iter().any(|p| std::path::Path::new(p).is_file())
}

/// Both doors name the same live management publication. A comparison's BOM is
/// a read-only report whose two labels come from the versions sent to review.bom.
pub(crate) fn design_panels(project: &str, versions: &str) -> Value {
    let Ok(project) = crate::files::kicad::embed::resolve_pro(project) else { return Value::Null };
    panel_scope(&project, versions)
}

fn panel_scope(project: &str, versions: &str) -> Value {
    let mut panels = json!({"bom":{"project":project,"title":"review.bom.working","refresh":"mgmt.refresh"}, "checks":{"project":project,"title":"review.checks.of","refresh":"kicad.checks.run"}});
    if let Some((old, new)) = versions.split_once('\n') {
        let revision = |source: &str| source.strip_prefix("git\t").and_then(|s| s.split('\t').nth(1))
            .map(|sha| sha.chars().take(8).collect::<String>()).unwrap_or_default();
        panels["bom"]["title"] = json!("review.bom.change");
        panels["bom"]["refresh"] = json!("rv-run");
        panels["bom"]["versions"] = json!([revision(old), revision(new)]);
    }
    panels
}
fn viewer_page<'a>(st: &State, template: &Value, session: &Value, root: &str, sources: impl FnOnce() -> &'a RailSources, shown: &mut ShownGerbers) -> Option<Value> {
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
    let sources = sources();
    let mut v = match kind {
        // Mech: the part on show as a 3D model (View.want: "2|<root>|<abs part>"), the parts as chips (Mech.json)
        "mech" => {
            let ps = parts_of(root, sel, s(&session["drafts"], &format!("@mech.part:{sel}"))); let cur = mech_current(&ps, s(&session["drafts"], &format!("@mech.part:{sel}")));
            let (want, _) = mech_want(root, &ps, s(&session["drafts"], &format!("@mech.part:{sel}")));
            let mut v = template.clone(); let o = v.as_object_mut()?;
            o.insert("open".into(), json!("mech")); o.insert("key".into(), json!(want));
            let chips: Vec<Value> = ps.iter().map(|p| json!({"label": stem(p), "value": p, "on": cur.as_deref() == Some(p.as_str())})).collect();
            let (say, note) = if root.is_empty() { ("Loading…", "") }
                else if sel.contains('~') && ps.is_empty() { ("No parts in its mech/ folder on that machine yet.", "") }
                else if ps.is_empty() { ("No mechanical parts in this project yet", "Ask an agent to model one, or add a STEP file to mech/.") }
                else { ("", "") };
            o.insert("mech".into(), json!({"say": say,
                "note": note, "parts": chips, "rel": cur.clone().unwrap_or_default(), "busy": false, "render": "Create renders", "err": "",
                "name": cur.as_deref().map(stem).unwrap_or_default(), "path": cur.as_deref().map(|p| abs_of(root, p)).unwrap_or_default(), "shots": [], "empty": ""}));
            v
        }
        // Split (Split.*): the schematic, the board and its 3D model, each a page of its own
        "split" => {
            let mut v = template.clone(); let o = v.as_object_mut()?;
            o.insert("open".into(), json!("split")); o.insert("key".into(), json!(key("split", root)));
            // a board picked in the Boards menu: each pane's source is that board's file (its schematic beside it), as kicad.watch's split keys them
            let chosen = chosen_board(&session["drafts"], sel, root);
            let picked = board_file("board", root, &chosen);
            // a sheet opened in the schematic pane is its source (`view-sheet`)
            let sheet = s(&session["drafts"], &format!("@view.sheet:{sel}")).to_string();
            let far = sel.contains('~');
            o.insert("panes".into(), Value::Array(["schematic", "board", "3d"].iter().map(|k| { let mut p = page(template, k, root, &session["drafts"], "", sources, shown);
                if !picked.is_empty() { let f = if *k == "schematic" { board_file("schematic", root, &chosen) } else { picked.clone() }; let kk = format!("{}{f}", key(k, root));
                    if *k != "3d" { p["layers"] = json!(kk); } else { p["layers"] = json!(""); } p["key"] = json!(kk); }
                if *k == "schematic" && !sheet.is_empty() { let kk = format!("{}{sheet}", key("schematic", root)); p["key"] = json!(kk); p["layers"] = json!(kk); }
                if let Some((empty, next)) = (!far && !root.is_empty()).then(|| pane_empty(k, sources)).flatten() {
                    p["empty"] = json!(empty);
                    p["emptyNext"] = json!(next);
                }
                p }).collect()));
            if let Some(b) = boards_menu(session, sel, root) { o.insert("boards".into(), b); }
            v
        }
        // Files: the folder on show, or an image, a PDF or a note opened from it (Fm.tab: they belong to Files)
        "files" => {
            let mut v = template.clone(); let o = v.as_object_mut()?;
            o.insert("open".into(), json!("files")); o.insert("key".into(), json!(""));
            o.insert("files".into(), files_page(st, root, sel, s(&session["drafts"], &format!("@files.dir:{sel}"))));
            v
        }
        "image" | "pdf" | "text" => {
            let mut v = template.clone(); let o = v.as_object_mut()?;
            o.insert("open".into(), json!(kind)); o.insert("key".into(), json!(""));
            let f = s(&session["drafts"], &format!("@view.file:{sel}"));
            // a file of a project on another machine is shown from this machine's copy of it (net/farfiles.rs)
            let page = if sel.contains('~') {
                let rel = f.strip_prefix(&format!("{}/", root.trim_end_matches('/'))).unwrap_or("");
                match crate::net::farfiles::copy(sel, rel) { Some(Ok(local)) => { let mut p = file_page(kind, &local); p["name"] = json!(crate::files::fm::last(rel)); p }, Some(Err(e)) => json!({"name": crate::files::fm::last(rel), "kind": kind, "url": "", "error": e}), None => json!({"name": crate::files::fm::last(rel), "kind": kind, "url": "", "loading": true}) }
            } else { file_page(kind, f) };
            o.insert("file".into(), page);
            v
        }
        _ => { let mut v = page(template, kind, root, &session["drafts"], sel, sources, shown); if matches!(kind, "board" | "schematic" | "3d") { if let Some(b) = boards_menu(session, sel, root) { v["boards"] = b; } } v }
    };
    // the folder the Boards menu's paths are from (0.14's "Folder: <root>")
    if v["boards"].is_array() { v["boardsRoot"] = json!(root); }
    // the probe kept for this board, on the page and on each of Split's panes (each canvas lights it on its own source)
    if matches!(kind, "board" | "schematic" | "3d" | "split") {
        let pr = probe_of(st, session, sel, root);
        if let Some(ps) = v["panes"].as_array_mut() { for p in ps.iter_mut() { p["probe"] = json!(pr); } }
        v["probe"] = json!(pr);
    }
    let tab = if matches!(kind, "image" | "pdf" | "text") { "files" } else if v["gerbers"] == true { "gerbers" } else { kind };
    // (Gerbers is a chip while it is on show: a project's fab output, when there is some, is the Board page's file)
    let chips: Vec<Value> = [("Board", "board"), ("Schematic", "schematic"), ("3D", "3d"), ("Gerbers", "gerbers"), ("Mech", "mech"), ("Files", "files"), ("Review", "review")].iter()
        .filter(|(_, val)| *val != "gerbers" || tab == "gerbers").map(|(l, val)| json!({"label": l, "value": val, "on": *val == tab})).collect();
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
    #[test]
    fn empty_gerbers_page_has_a_lit_rail_entry_until_closed() {
        let (st, th) = state();
        let session = json!({"sel":th});
        let template = json!({"open":"board", "gerbers":true, "hasGerbers":false});
        let v = viewer_json(&st, &template, &session).unwrap();
        let rows = v["rail"].as_array().unwrap();
        assert!(rows.iter().any(|r| r["id"] == "gerbers" && r["state"] == "on"));
        assert_eq!(rows.iter().filter(|r| r["state"] == "on").count(), 1);
        let closed = viewer_json(&st, &json!({"open":"", "hasGerbers":false}), &session).unwrap();
        assert!(closed["rail"].as_array().unwrap().iter().all(|r| r["id"] != "gerbers"));
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
        assert!(v.get("rail").is_none() && v.get("has").is_none(), "linked project files are on the other machine");
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
    #[test] fn another_machines_parts_and_files_come_from_its_listing() {
        assert_eq!(parts_in_listing("d\t0\told\nf\t10\tb.STEP\nf\t3\tnotes.md\nf\t9\ta.glb\nf\t1\t.hidden.step\n"), ["mech/a.glb", "mech/b.STEP"]);
        let mut st = State::new();
        for l in [r#"{"$":"ProjectCreated","id":"L~p1","title":"Board","root":"/desk/board","at":1}"#, r#"{"$":"ThreadCreated","id":"L~t1","project":"L~p1","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}"#] { st.apply_line(l); }
        // (no machine answers in a test: the page waits for the listing, and offers no download from this machine)
        let f = files_page(&st, "/desk/board", "L~t1", "");
        assert_eq!((f["ready"].as_bool(), f["zip"].as_str(), f["error"].as_str()), (Some(false), Some(""), Some("")));
        assert_eq!(parts_of("/desk/board", "L~t1", "mech/x.step"), ["mech/x.step"], "a part an agent showed is kept before the listing comes");
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
        assert_eq!(viewer_json(&st, &template, &other), viewer_json(&st, &template, &idle), "the panel left on by another thread: the idle viewer");
        let mut board = other.clone(); board["drafts"] = json!({format!("@view:{th}"): "board", "@view.at": "0|/tmp/soak2|"});
        assert!(viewer_json(&st, &template, &board).is_some(), "and its own drawing is drawn");
    }
    /// Follow agent (issue 1229): an agent's saved kicad_edit in a thread with Follow on is a focus (page, sheet, box) in the viewer's picture; not with it
    /// off or paused, and a person's pause and resume are the thread's own state, the same for every client.
    #[test] fn an_agents_edit_is_a_focus_in_the_viewer_picture_only_while_following() {
        use crate::files::kicad::follow;
        let (st, th) = state();
        let template: Value = serde_json::from_str::<Value>(include_str!("thread_static.json")).unwrap()["viewer"].clone();
        let o = view_action(&st, "h", &json!({"sel": th, "rid": 1, "drafts": {}, "open": {}, "adding": false, "panel": false, "info": {}}), "board").unwrap();
        let mut session = json!({"sel": th, "rid": 1, "drafts": {}, "open": {}, "adding": false, "panel": false, "info": {}}); crate::session::patch::merge(&mut session, &o.patch);
        let edit = json!({"saved": "/p/b.kicad_pcb", "results": [{"routed": [{"net": "+3V3", "length": 9.0, "vias": 0, "layers": ["F.Cu"], "box": [10.0, 20.0, 30.0, 24.0]}]}]});
        let pic = |s: &Value| viewer_json(&st, &template, s).unwrap()["follow"].clone();
        assert_eq!(pic(&session)["on"], json!(false), "off until the person turns it on");
        assert!(follow::record(&th, "/p/b.kicad_pcb", "/p/b.kicad_sch", &edit).is_none());
        assert_eq!(pic(&session)["focus"], Value::Null, "off: no focus in the picture");
        follow::act(&th, "on");
        follow::record(&th, "/p/b.kicad_pcb", "/p/b.kicad_sch", &edit).expect("on");
        let p = pic(&session);
        assert_eq!((p["on"].clone(), p["seq"].clone(), p["focus"]["page"].clone(), p["focus"]["sheet"].clone(), p["focus"]["net"].clone()), (json!(true), json!(1), json!("board"), json!(""), json!("+3V3")));
        assert_eq!(p["focus"]["box"].as_array().map(|b| b.len()), Some(4));
        follow::act(&th, "pause");
        assert!(follow::record(&th, "/p/b.kicad_pcb", "/p/b.kicad_sch", &edit).is_none(), "paused: nothing new");
        let p = pic(&session); assert_eq!((p["paused"].clone(), p["seq"].clone()), (json!(true), json!(1)));
        follow::act(&th, "resume");
        follow::record(&th, "/p/b.kicad_sch", "/p/b.kicad_sch", &json!({"results": [{"updated": [{"ref": "U5", "at": [1.0, 2.0]}]}]})).expect("resumed");
        let p = pic(&session); assert_eq!((p["seq"].clone(), p["focus"]["page"].clone()), (json!(2), json!("schematic")));
        follow::act(&th, "off");
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


    fn files_world(tag: &str) -> (State, String, crate::plat::tmp::TempDir) {
        let d = crate::plat::tmp::canonical(&format!("fm-pres-{tag}")); std::fs::create_dir_all(d.join("docs/deep")).unwrap(); std::fs::create_dir_all(d.join("mech")).unwrap();
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
        let mut unknown = s1.clone(); crate::session::patch::merge(&mut unknown, &json!({"info": {format!("fab.board:{th}"): ""}}));
        let u = viewer_json(&st, &template, &unknown).unwrap()["boards"].clone();
        assert!(u.as_array().unwrap().iter().all(|r| r["on"] == false), "the board on show not known yet: none is lit (the first by name was not the one drawn): {u}");
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
    #[test] fn a_probe_is_kept_per_board_and_shown_on_every_page_of_it() {
        let (mut st, th, d) = files_world("probe"); let root = d.to_string_lossy().into_owned();
        let template: Value = serde_json::from_str::<Value>(include_str!("thread_static.json")).unwrap()["viewer"].clone();
        let mut s1 = sess(&th, json!({})); let o = view_action(&st, "h", &s1, "board").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        let c = probe_action(&st, "h", &s1, "fp;ref=C5;value=1u").unwrap();
        assert_eq!(sent(&c[0])["key"], json!(format!("probe:{root}|")), "{c:?}");
        st.settings.insert(format!("probe:{root}|"), "fp;ref=C5;value=1u".into());
        let closed = sess(&th, json!({}));
        assert!(probe_action(&st, "linked", &closed, "fp;ref=C5").is_none());
        let clear = probe_action(&st, "linked", &closed, "").unwrap();
        assert_eq!(clear.len(), 1);
        assert_eq!(clear[0]["hub"], "linked");
        assert_eq!(sent(&clear[0]), json!({"key": format!("probe:{root}|"), "value": ""}));
        assert_eq!(viewer_json(&st, &template, &s1).unwrap()["probe"], "fp;ref=C5;value=1u");
        assert!(probe_action(&st, "h", &s1, "fp;ref=C5;value=1u").unwrap().is_empty(), "the same probe again sends nothing");
        let o = view_action(&st, "h", &s1, "split").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        assert_eq!(viewer_json(&st, &template, &s1).unwrap()["panes"][0]["probe"], "fp;ref=C5;value=1u", "Split's panes light it too");
        // another board picked: its own probe (none yet)
        crate::session::patch::merge(&mut s1, &json!({"drafts": {format!("@view.board:{th}"): "a/a.kicad_pcb"}}));
        assert_eq!(viewer_json(&st, &template, &s1).unwrap()["probe"], "");
        assert!(probe_action(&st, "h", &s1, "two\nlines").is_none());
        assert!(crate::state::fam::policy::lean_keeps(&format!("probe:{root}|a/a.kicad_pcb")), "the presenter's lean copy keeps the probe");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn splits_schematic_pane_opens_another_sheet() {
        let (st, th, d) = files_world("split-sheet"); let root = d.to_string_lossy().into_owned();
        let template: Value = serde_json::from_str::<Value>(include_str!("thread_static.json")).unwrap()["viewer"].clone();
        let mut s1 = sess(&th, json!({})); let o = view_action(&st, "h", &s1, "split").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        let sub = format!("{root}/power.kicad_sch");
        let o = sheet_action(&st, "h", &s1, &sub).unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        let p = sent(&o.before[0]); assert_eq!((p["kind"].as_str(), p["path"].as_str()), (Some("split"), Some(sub.as_str())), "{p}");
        assert_eq!(viewer_json(&st, &template, &s1).unwrap()["panes"][0]["key"], json!(format!("1|{root}|{sub}")));
        let o = sheet_action(&st, "h", &s1, "").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        assert_eq!(viewer_json(&st, &template, &s1).unwrap()["panes"][0]["key"], json!(format!("1|{root}|")), "back to the root sheet");
        let alt = format!("{root}/Power.SchDoc"); // an Altium sheet is a sheet too
        let o = sheet_action(&st, "h", &s1, &alt).unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        assert_eq!(viewer_json(&st, &template, &s1).unwrap()["panes"][0]["key"], json!(format!("1|{root}|{alt}")));
        assert!(sheet_action(&st, "h", &s1, &format!("{root}/notes.txt")).is_none());
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
        let d = crate::plat::tmp::dir("mech");
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
        let p = page(&template, "schematic", "/p", &json!({}), "", &RailSources::default(), &mut ShownGerbers::default()); assert_eq!(p["key"], "1|/p|");
        assert_eq!(page(&template, "3d", "/p", &json!({}), "", &RailSources::default(), &mut ShownGerbers::default())["layers"], "0|/p|");
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
        let p = |d: Value| page(&template, "board", "/p", &d, "", &RailSources::default(), &mut ShownGerbers::default());
        assert_eq!(p(json!({}))["layerReset"], false); assert_eq!(p(json!({"@view.off.board": "32"}))["layerReset"], true);
        assert_eq!(p(json!({"@view.focus.board": "12|1"}))["layerReset"], true); assert_eq!(p(json!({"@view.focus.board": "12|0"}))["layerReset"], false);
        let q = p(json!({"@view.focus.board": "27|1"})); let l = q["layerMarks"].as_array().unwrap();
        assert_eq!(l.iter().map(|r| r["name"].as_str().unwrap()).collect::<Vec<_>>(), ["Removed", "Changed", "Added"]); assert_eq!(l[1]["active"], true);
        assert!(q["layerList"].as_array().unwrap().iter().all(|r| r["layer"].as_u64().unwrap() < 26), "a plain plot's list is Lyr.order's");
        assert_eq!(q["layerHead"], "Layer: Changed");
        assert!(page(&template, "3d", "/p", &json!({}), "", &RailSources::default(), &mut ShownGerbers::default())["layerList"].as_array().unwrap().is_empty(), "a 3D mesh has no layers");
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
        let d = crate::plat::tmp::dir("mech2"); std::fs::create_dir_all(d.join("mech")).unwrap();
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
    }

    #[test]
    fn linked_schematic_waits_for_peer_evidence_without_a_local_root() {
        let (mut st, th) = state();
        let peer = "peer~pending-document";
        let mut thread = st.threads[&th].clone();
        thread.project.clear();
        thread.worktree.clear();
        st.threads.insert(peer.into(), thread);
        for selected in ["", "boards/second.kicad_pcb"] {
            let session = sess(peer, json!({format!("@view:{peer}"):"schematic", format!("@view.board:{peer}"):selected}));
            let v = viewer_json(&st, &json!({}), &session).unwrap();
            assert_eq!(v["identity"]["page"], "schematic");
            assert!(v["identity"].get("missing").is_none());
            assert!(v["identity"].get("source").is_none());
        }
        assert_eq!(schematic_for_board("boards/second.kicad_pcb"), "boards/second.kicad_sch");
    }

    #[test]
    fn split_and_schematic_share_selected_document_facts() {
        let (st, th, root) = files_world("identity-selected");
        for name in ["docs/main.kicad_pcb", "docs/main.kicad_sch", "docs/deep/child.kicad_sch"] {
            std::fs::write(root.join(name), "").unwrap();
        }
        let board = root.join("docs/main.kicad_pcb");
        let mut session = sess(&th, json!({
            format!("@view:{th}"): "schematic",
            format!("@view.board:{th}"): board,
            format!("@view.sheet:{th}"): root.join("docs/main.kicad_sch"),
        }));
        let v = viewer_json(&st, &json!({}), &session).unwrap();
        assert_eq!(v["identity"]["source"]["rel"], "docs/main.kicad_sch");
        assert!(v["identity"].get("sheet").is_none());

        session["drafts"][format!("@view:{th}")] = json!("split");
        session["drafts"][format!("@view.sheet:{th}")] = json!(root.join("docs/deep/child.kicad_sch"));
        std::fs::remove_file(board).unwrap();
        let v = viewer_json(&st, &json!({}), &session).unwrap();
        assert_eq!(v["has"]["board"], false);
        assert!(v["identity"].get("missing").is_none());
        let panes = &v["identity"]["panes"];
        assert_eq!(panes[0]["sheet"], "docs/deep/child.kicad_sch");
        assert_eq!(panes[1]["missing"], true);
        assert_eq!(panes[2]["missing"], true);
    }

    #[test]
    fn explicit_gerber_set_keeps_its_time_and_drops_board_history() {
        use std::time::{Duration, UNIX_EPOCH};
        let (st, th, root) = files_world("identity-gerbers");
        for (dir, time) in [("older", 1_600_000_000), ("newer", 1_600_000_100)] {
            let set = root.join("mfg").join(dir);
            std::fs::create_dir_all(&set).unwrap();
            std::fs::File::create(set.join("copper.gbr")).unwrap()
                .set_modified(UNIX_EPOCH + Duration::from_secs(time)).unwrap();
        }
        let mut session = sess(&th, json!({
            format!("@view:{th}"): "board", format!("@view.gerbers:{th}"): "1",
        }));
        session["watchedHistory"] = json!({"kind":"board","historyRel":"a.kicad_pcb",
            "version":"1234abcd","comparison":{"from":"first","to":"last"}});
        for (file, set, time) in [("", "mfg/newer", 1_600_000_100), ("mfg/older", "mfg/older", 1_600_000_000)] {
            session["drafts"][format!("@view.file:{th}")] = json!(file);
            let v = viewer_json(&st, &json!({}), &session).unwrap();
            assert_eq!(v["identity"]["set"], json!({"rel":set,"madeAt":time}));
            assert!(v["identity"].get("version").is_none());
            assert!(v["identity"].get("scope").is_none());
        }
        std::fs::remove_dir_all(root.join("mfg")).unwrap();
        assert_eq!(viewer_json(&st, &json!({}), &session).unwrap()["identity"]["missing"], true);
    }

    #[cfg(unix)]
    #[test]
    fn comparison_watch_keeps_both_operands_under_the_registered_root_alias() {
        let (mut st, th, root) = files_world("identity-gerber-root-alias");
        let aliases = crate::plat::tmp::dir("identity-root-alias");
        let alias = aliases.join("project");
        std::os::unix::fs::symlink(&*root, &alias).unwrap();
        let project = st.threads[&th].project.clone();
        st.projects.get_mut(&project).unwrap().root = alias.to_string_lossy().into_owned();
        std::fs::create_dir(root.join("mfg")).unwrap();
        std::fs::write(root.join("mfg/copper.gbr"), "").unwrap();
        std::fs::write(root.join("a.kicad_pro"), "{}").unwrap();
        let session = sess(&th, json!({}));
        let action = view_action(&st, "h", &session, "gerbers\tboard").unwrap();
        let watch: Value = serde_json::from_str(action.before[0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(watch["p"]["path"], format!("gerbercmp\n{0}/mfg\n{0}/a.kicad_pcb", alias.display()));
    }

    #[cfg(unix)]
    #[test]
    fn escaping_gerber_set_is_missing_for_default_and_explicit_selection() {
        let (st, th, root) = files_world("identity-gerber-link");
        let outside = crate::plat::tmp::dir("identity-external-set");
        std::fs::write(outside.join("copper.gbr"), "").unwrap();
        std::fs::create_dir(root.join("mfg")).unwrap();
        std::os::unix::fs::symlink(&*outside, root.join("mfg/out")).unwrap();
        for selected in ["", "mfg/out"] {
            let session = sess(&th, json!({format!("@view:{th}"):"board",
                format!("@view.gerbers:{th}"):"1", format!("@view.file:{th}"):selected}));
            let v = viewer_json(&st, &json!({}), &session).unwrap();
            assert_eq!(v["identity"]["missing"], true);
            assert!(v["identity"].get("set").is_none());
        }
    }

    #[test] fn the_gerbers_page_is_the_board_page_on_the_newest_set() {
        let d = crate::plat::tmp::dir("gerb-view"); std::fs::create_dir_all(d.join("mfg/b/gerber")).unwrap();
        std::fs::write(d.join("mfg/b/gerber/b-F_Cu.gbr"), "%FSLAX46Y46*%M02*").unwrap();
        let root = d.to_string_lossy().into_owned();
        let mut st = State::new(); for l in include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-log.jsonl")).lines() { st.apply_line(l); }
        let th = "t1700000003-8"; let proj = st.threads.get(th).map(|t| t.project.clone()).unwrap();
        if let Some(p) = st.projects.get_mut(&proj) { p.root = root.clone(); }
        let session = json!({"sel": th, "rid": 1, "drafts": {}, "open": {}, "adding": false, "panel": false, "info": {}});
        let o = view_action(&st, "h", &session, "gerbers").unwrap();
        use crate::plat::tmp::sl; let want = sl(format!("{root}/mfg/b/gerber"));
        assert_eq!((o.patch["drafts"][format!("@view:{th}")].as_str(), o.patch["drafts"][format!("@view.file:{th}")].as_str().map(sl), o.patch["drafts"][format!("@view.gerbers:{th}")].as_str()), (Some("board"), Some(want.clone()), Some("1")));
        let w = o.before[0]["text"].as_str().unwrap(); assert!(w.contains("\"kind\":\"board\"") && sl(w.replace("\\\\", "\\")).contains(&want), "{w}");
        let set = o.patch["drafts"][format!("@view.file:{th}")].as_str().unwrap().to_string(); // the set as the hub names it (Windows: with "\\")
        let mut s2 = session.clone(); crate::session::patch::merge(&mut s2, &o.patch);
        let v = viewer_json(&st, &json!({}), &s2).unwrap();
        assert_eq!((v["open"].as_str(), v["gerbers"].as_bool(), v["hasGerbers"].as_bool()), (Some("board"), Some(true), Some(true)));
        assert!(v["choices"].as_array().unwrap().iter().any(|c| c["value"] == "gerbers" && c["on"] == true), "the Gerbers tab is the one on");
        let names: Vec<&str> = v["layerList"].as_array().unwrap().iter().map(|r| r["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"F.Mask") && names.contains(&"B.Paste") && names.contains(&"Drills") && !names.contains(&"Vias"), "{names:?}");
        // compared with its board: the set and the board side by side in the watched path, marked so
        std::fs::write(d.join("b.kicad_pro"), "{}").unwrap(); std::fs::write(d.join("b.kicad_pcb"), "(kicad_pcb)").unwrap();
        let o2 = view_action(&st, "h", &s2, "gerbers\tboard").unwrap();
        let f = o2.patch["drafts"][format!("@view.file:{th}")].as_str().unwrap().to_string();
        assert_eq!(sl(&f), sl(format!("gerbercmp\n{set}\n{root}/b.kicad_pcb")));
        let mut s3 = s2.clone(); crate::session::patch::merge(&mut s3, &o2.patch);
        assert_eq!(viewer_json(&st, &json!({}), &s3).unwrap()["gerbersVsBoard"], true);
        assert_eq!(o2.patch["drafts"]["@view.focus.board"], "12|1", "on F.Cu's differences, the other layers dimmed");
        crate::files::kicad::gerber::set_counts(&set, vec![("F.Cu".into(), 3, 1)]);
        let v3 = viewer_json(&st, &json!({}), &s3).unwrap();
        assert!(v3["layerList"].as_array().unwrap().iter().any(|r| r["name"] == "F.Cu · 4"), "{}", v3["layerList"]);
        // and back to the set alone
        let o3 = view_action(&st, "h", &s3, "gerbers").unwrap();
        assert_eq!(o3.patch["drafts"][format!("@view.file:{th}")], set.as_str());
        // the Board page again: its own board, not marked
        let o = view_action(&st, "h", &s2, "board").unwrap();
        assert_eq!(o.patch["drafts"][format!("@view.gerbers:{th}")], "");
        // a project with no fab output: Gerbers opens its empty state instead of refusing with PRESENTER_UNAVAILABLE
        std::fs::remove_dir_all(d.join("mfg")).unwrap();
        let o_empty = view_action(&st, "h", &session, "gerbers").expect("empty Gerbers page opens");
        assert!(o_empty.before.is_empty(), "nothing to watch when there is no Gerber set");
        let mut s_empty = session.clone(); crate::session::patch::merge(&mut s_empty, &o_empty.patch);
        let v_empty = viewer_json(&st, &json!({}), &s_empty).unwrap();
        assert_eq!((v_empty["open"].as_str(), v_empty["gerbers"].as_bool(), v_empty["hasGerbers"].as_bool(), v_empty["key"].as_str()), (Some("board"), Some(true), Some(false), Some("")));
        assert_eq!((v_empty["empty"].as_str(), v_empty["emptyNext"].as_str()), (Some("No Gerber files in this project yet"), Some("Ask an agent to export fabrication files into mfg/.")));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Issue 1102: a GLB or STEP shown from outside mech/ (build/f1_model/picobiscuit.glb) is the part on show, not mech/'s first.
    #[test] fn a_model_shown_from_outside_mech_is_the_one_drawn() {
        let d = crate::plat::tmp::named("mech-out", "my project");
        std::fs::create_dir_all(d.join("mech")).unwrap(); std::fs::create_dir_all(d.join("build/f1 model")).unwrap();
        for f in ["mech/littelfuse.step", "build/f1 model/pico biscuit.glb"] { std::fs::write(d.join(f), "x").unwrap(); }
        let root = d.to_string_lossy().into_owned();
        let mut st = State::new(); for l in include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-log.jsonl")).lines() { st.apply_line(l); }
        let th = "t1700000003-8"; let proj = st.threads.get(th).map(|t| t.project.clone()).unwrap();
        if let Some(p) = st.projects.get_mut(&proj) { p.root = root.clone(); }
        let glb = d.join("build/f1 model/pico biscuit.glb").to_string_lossy().into_owned();
        let session = json!({"sel": th, "rid": 1, "drafts": {}, "open": {}, "adding": false, "panel": false, "info": {}});
        let o = view_action(&st, "h", &session, &format!("mech\t{glb}")).unwrap();
        assert_eq!(o.patch["drafts"][format!("@mech.part:{th}")], "build/f1 model/pico biscuit.glb");
        let w: Value = serde_json::from_str(o.before[0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(w["p"]["path"].as_str().map(|p| p.replace('\\', "/")), Some(glb.replace('\\', "/")), "the GLB asked for is watched, not mech/littelfuse.step");
        let mut s2 = session.clone(); crate::session::patch::merge(&mut s2, &o.patch);
        let template: Value = serde_json::from_str::<Value>(include_str!("thread_static.json")).unwrap()["viewer"].clone();
        let v = viewer_json(&st, &template, &s2).unwrap();
        assert_eq!(v["mech"]["rel"], "build/f1 model/pico biscuit.glb"); assert_eq!(v["mech"]["parts"][0]["on"], true); assert_eq!(v["mech"]["parts"][1]["on"], false);
        // a chip pressed (mech-part) and a refresh keep it too; one outside the project is kept by its absolute path
        let o = mech_action(&st, "h", &s2, &glb, true).unwrap(); assert!(o.before.is_empty(), "refresh: still that GLB");
        let far = crate::plat::tmp::file("mech-far", "far.step"); std::fs::write(&far, "x").unwrap(); let fs = far.to_string_lossy().into_owned();
        assert_eq!(parts_of(&root, th, &fs)[0], fs);
        assert_eq!(parts_of(&root, th, "mech/gone.step"), ["mech/littelfuse.step"], "a file that is not there is not listed");
    }
    #[test] fn paths_are_kept_from_the_root_whatever_the_separator() {
        assert_eq!(rel_in("/p/x", "/p/x/mech/a.step"), "mech/a.step"); assert_eq!(rel_in("/p/x/", "/p/x/build/b.glb"), "build/b.glb");
        assert_eq!(rel_in("/p/x", "/p/xy/a.step"), "/p/xy/a.step", "a sibling folder is not the root"); assert_eq!(rel_in("/p/x", "mech/a.step"), "mech/a.step");
        assert_eq!(rel_in(r"C:\Users\a b\proj", r"C:\Users\a b\proj\build\m.glb"), "build/m.glb"); assert_eq!(rel_in("C:/p", r"C:\p\mech\a.step"), "mech/a.step");
        assert_eq!(rel_in(r"C:\p", r"D:\q\a.step"), r"D:\q\a.step");
    }
    #[test] fn viewer_show_refuses_what_it_cannot_show() {
        let d = crate::plat::tmp::dir("vs-ref"); std::fs::create_dir_all(d.join("mech")).unwrap(); std::fs::write(d.join("mech/a.STEP"), "x").unwrap();
        let (root, abs) = (d.to_string_lossy().into_owned(), d.join("mech/a.STEP").to_string_lossy().into_owned()); let roots = [root.clone()];
        for (k, p) in [("mech", abs.as_str()), ("mech", "mech/a.STEP"), ("3d", abs.as_str()), ("", abs.as_str()), ("mech", ""), ("board", ""), ("schematic", "x.kicad_sch"), ("", "b.kicad_pcb"), ("files", ""), ("3d", "")] {
            assert_eq!(show_refusal(k, p, &roots), None, "{k} {p}");
        }
        for (k, p) in [("mech", "mech/gone.step"), ("mech", "board.kicad_pcb"), ("", "notes.md"), ("teapot", ""), ("3d", "mech/gone.glb")] {
            assert!(show_refusal(k, p, &roots).is_some(), "{k} {p}");
        }
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn a_sheet_is_part_of_the_schematics_source_and_is_watched_by_its_file() {
        let template: Value = serde_json::from_str::<Value>(include_str!("thread_static.json")).unwrap()["viewer"].clone();
        let drafts = json!({"@view.sheet:t1": "/p/amp.kicad_sch"});
        assert_eq!(page(&template, "schematic", "/p", &drafts, "t1", &RailSources::default(), &mut ShownGerbers::default())["key"], "1|/p|/p/amp.kicad_sch");
        assert_eq!(page(&template, "schematic", "/p", &drafts, "t2", &RailSources::default(), &mut ShownGerbers::default())["key"], "1|/p|", "another thread's sheet is not this one's");
        assert_eq!(page(&template, "board", "/p", &drafts, "t1", &RailSources::default(), &mut ShownGerbers::default())["key"], "0|/p|");
    }

    #[test] fn split_pane_empty_reports_only_missing_sources_and_ignores_far_machines() {
        let d = crate::plat::tmp::dir("split-empty"); let root = d.to_string_lossy().into_owned();
        // empty project: all three panes missing
        assert_eq!(split_pane_empty("schematic", &root, "", "", false), Some(("No schematic in this project yet", "Ask an agent to draw one.")));
        assert_eq!(split_pane_empty("board", &root, "", "", false), Some(("No board in this project yet", "Ask an agent to lay one out.")));
        assert_eq!(split_pane_empty("3d", &root, "", "", false), Some(("No board in this project yet", "Ask an agent to lay one out.")));
        // far machine: never reports local missing
        assert_eq!(split_pane_empty("board", &root, "", "", true), None);
        // schematic only: board & 3d missing, schematic present
        std::fs::write(d.join("p.kicad_pro"), "{}").unwrap();
        let sch = d.join("p.kicad_sch"); std::fs::write(&sch, "(kicad_sch)").unwrap();
        assert_eq!(split_pane_empty("schematic", &root, "", &sch.to_string_lossy(), false), None);
        if !crate::files::kicad::project::read(&root).sch.is_empty() {
            assert_eq!(split_pane_empty("schematic", &root, "", "", false), None);
            assert!(split_pane_empty("board", &root, "", "", false).is_some());
            assert!(split_pane_empty("3d", &root, "", "", false).is_some());
            // board added: all three present
            std::fs::write(d.join("p.kicad_pcb"), "(kicad_pcb)").unwrap();
            for k in ["schematic", "board", "3d"] { assert_eq!(split_pane_empty(k, &root, "", "", false), None, "{k}"); }
        } else {
            std::fs::write(d.join("p.kicad_pcb"), "(kicad_pcb)").unwrap();
            assert_eq!(split_pane_empty("board", &root, "p.kicad_pcb", "", false), None);
            assert_eq!(split_pane_empty("3d", &root, "p.kicad_pcb", "", false), None);
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test] fn retry_re_sends_complete_watch_for_split_and_far_threads() {
        assert!(retry_allowed(0, 10_000));
        assert!(!retry_allowed(10_000, 10_000));
        assert!(!retry_allowed(10_000, 10_999));
        assert!(retry_allowed(10_000, 11_000));
        let (st, th, d) = files_world("retry"); let root = d.to_string_lossy().into_owned();
        let mut s1 = sess(&th, json!({}));
        let o = view_action(&st, "h", &s1, "split").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        let sub = format!("{root}/power.kicad_sch");
        let o = sheet_action(&st, "h", &s1, &sub).unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        // First retry at t=10_000 ms emits forced Split watch preserving sheet and records @view.retry.at
        let r = view_action_at(&st, "h", &s1, "retry", 10_000).unwrap();
        assert_eq!(r.before.len(), 1, "retry re-sends forced kicad.watch");
        let p = sent(&r.before[0]);
        assert_eq!((p["kind"].as_str(), p["path"].as_str(), p["force"].as_bool()), (Some("split"), Some(sub.as_str()), Some(true)), "Split retry preserves kind, sheet and force: {p}");
        crate::session::patch::merge(&mut s1, &r.patch);
        // Spammed retry within 1 s is ignored; after 1 s it is accepted again
        assert!(view_action_at(&st, "h", &s1, "retry", 10_500).unwrap().before.is_empty(), "spammed retry within 1 s is ignored");
        let r2 = view_action_at(&st, "h", &s1, "retry", 11_000).unwrap();
        assert_eq!(r2.before.len(), 1, "retry accepted again after 1 s");
        // Standalone schematic with a picked board AND active subsheet: retry keeps the active subsheet (not the picked board's root schematic)
        let mut s_sch = sess(&th, json!({format!("@view.board:{th}"): "sub/b.kicad_pcb"}));
        let o = view_action(&st, "h", &s_sch, "schematic").unwrap(); crate::session::patch::merge(&mut s_sch, &o.patch);
        let o = sheet_action(&st, "h", &s_sch, &sub).unwrap(); crate::session::patch::merge(&mut s_sch, &o.patch);
        let rs = view_action_at(&st, "h", &s_sch, "retry", 10_000).unwrap();
        assert_eq!(rs.before.len(), 1);
        assert_eq!((sent(&rs.before[0])["path"].as_str(), sent(&rs.before[0])["force"].as_bool()), (Some(sub.as_str()), Some(true)), "standalone schematic retry keeps active subsheet even with picked board");
        // far thread retry preserves `on` and `force: true`
        let mut far_st = State::new();
        for l in [r#"{"$":"ProjectCreated","id":"L~p1","title":"Board","root":"/desk/board","at":1}"#, r#"{"$":"ThreadCreated","id":"L~t1","project":"L~p1","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}"#] { far_st.apply_line(l); }
        let mut fs = sess("L~t1", json!({}));
        let o = view_action(&far_st, "h", &fs, "board").unwrap(); crate::session::patch::merge(&mut fs, &o.patch);
        let fr = view_action_at(&far_st, "h", &fs, "retry", 10_000).unwrap();
        assert_eq!(fr.before.len(), 1);
        assert_eq!((sent(&fr.before[0])["on"].as_str(), sent(&fr.before[0])["force"].as_bool()), (Some("L"), Some(true)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test] fn gerbers_empty_recovers_when_files_appear_and_vs_board_says_when_no_board() {
        let d = crate::plat::tmp::dir("gerb-coh"); let root = d.to_string_lossy().into_owned();
        let mut st = State::new(); for l in include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-log.jsonl")).lines() { st.apply_line(l); }
        let th = "t1700000003-8"; let proj = st.threads.get(th).map(|t| t.project.clone()).unwrap();
        if let Some(p) = st.projects.get_mut(&proj) { p.root = root.clone(); }
        let mut s1 = sess(th, json!({}));
        // 1. Open Gerbers when no fab output exists
        let o = view_action(&st, "h", &s1, "gerbers").unwrap(); crate::session::patch::merge(&mut s1, &o.patch);
        let v1 = viewer_json(&st, &json!({}), &s1).unwrap();
        assert_eq!((v1["gerbers"].as_bool(), v1["key"].as_str(), v1["empty"].as_str()), (Some(true), Some(""), Some("No Gerber files in this project yet")));
        assert_eq!(s(&s1["drafts"], "@view.at"), "");
        // 2. Fab files appear on disk while the empty Gerbers page is open: presenter sync starts the watch and records @view.at
        std::fs::create_dir_all(d.join("mfg/b/gerber")).unwrap();
        std::fs::write(d.join("mfg/b/gerber/b-F_Cu.gbr"), "%FSLAX46Y46*%M02*").unwrap();
        let mut cmds = vec![];
        sync_gerbers(&st, "h", &mut s1, &mut cmds);
        assert_eq!(cmds.len(), 1, "presenter starts Gerber watch once files appear");
        let wp = sent(&cmds[0]);
        assert_eq!(wp["kind"].as_str(), Some("board"));
        assert!(wp["path"].as_str().unwrap_or("").contains("gerber"), "watch targets Gerber set: {wp}");
        assert!(s1["drafts"]["@view.at"].as_str().unwrap_or("").contains("gerber"), "@view.at recorded");
        let mut cmds2 = vec![];
        sync_gerbers(&st, "h", &mut s1, &mut cmds2);
        assert!(cmds2.is_empty(), "sync_gerbers is idempotent");
        let v2 = viewer_json(&st, &json!({}), &s1).unwrap();
        let k2 = v2["key"].as_str().unwrap();
        assert!(k2.contains("mfg") && k2.contains("gerber"), "expected Gerber set key, got {k2:?}");
        assert!(v2.get("empty").is_none());
        // 3. Closing the recovered Gerbers page stops the watch (no subscription left)
        let close = view_action(&st, "h", &s1, "").unwrap();
        assert_eq!(close.before.len(), 1, "closing stops the presenter-started Gerber watch");
        assert_eq!(sent(&close.before[0])["kind"].as_str(), Some(""));
        assert_eq!(close.patch["drafts"]["@view.at"], "");
        // 4. "vs board" with a Gerber set and no board says so instead of silently showing the plain set
        let o_vs = view_action(&st, "h", &s1, "gerbers\tboard").unwrap();
        let mut s_vs = s1.clone(); crate::session::patch::merge(&mut s_vs, &o_vs.patch);
        let v_vs = viewer_json(&st, &json!({}), &s_vs).unwrap();
        assert_eq!((v_vs["gerbers"].as_bool(), v_vs["gerbersVsBoard"].as_bool(), v_vs["key"].as_str()), (Some(true), Some(true), Some("")));
        assert_eq!(v_vs["empty"].as_str(), Some("No board to compare these Gerbers against."));
        // 5. Nested project with Gerbers and a parent project's board outside its root still refuses "vs board"
        let nested = d.join("nested");
        std::fs::create_dir_all(nested.join("mfg")).unwrap();
        std::fs::write(nested.join("mfg/n-F_Cu.gbr"), "%FSLAX46Y46*%M02*").unwrap();
        std::fs::write(d.join("parent.kicad_pro"), "{}").unwrap();
        std::fs::write(d.join("parent.kicad_pcb"), "(kicad_pcb)").unwrap();
        if let Some(p) = st.projects.get_mut(&proj) { p.root = nested.to_string_lossy().into_owned(); }
        let mut sn = sess(th, json!({}));
        let o_ng = view_action(&st, "h", &sn, "gerbers").unwrap(); crate::session::patch::merge(&mut sn, &o_ng.patch);
        let o_nvs = view_action(&st, "h", &sn, "gerbers\tboard").unwrap();
        crate::session::patch::merge(&mut sn, &o_nvs.patch);
        let v_nvs = viewer_json(&st, &json!({}), &sn).unwrap();
        assert_eq!((v_nvs["gerbers"].as_bool(), v_nvs["gerbersVsBoard"].as_bool(), v_nvs["key"].as_str()), (Some(true), Some(true), Some("")));
        assert_eq!(v_nvs["empty"].as_str(), Some("No board to compare these Gerbers against."));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn gerbers_layer_reply_exposes_gerber_layers_and_modes() {
        let present = (1 << 14) | (1 << 12) | (1 << 24);
        let r = layer_reply("gerbers", present, 0, "", "", "").expect("gerbers layer reply");
        assert_eq!(r["layerModes"], json!(true));
        let names: Vec<&str> = r["layerList"].as_array().unwrap().iter().filter_map(|row| row["name"].as_str()).collect();
        assert!(names.contains(&"F.Mask") && names.contains(&"Drills") && names.contains(&"F.Fab"), "{names:?}");
        // `present` controls which layer cycle/mode activates (same as board: absent bits are skipped).
        let only_mask = 1 << 14;
        let cycled = layer_reply("gerbers", only_mask, 0, "", "cycle", "").unwrap();
        assert_eq!(cycled["lift"], json!(14), "cycle picks only a present layer bit");
        assert_eq!(cycled["layerHead"], json!("Layer: F.Mask"));
        let picked = layer_reply("gerbers", present, 0, "", "pick", "14").unwrap();
        assert_eq!(picked["layerHead"], json!("Layer: F.Mask"));
        assert_eq!(picked["lift"], json!(14));
        let toggled = layer_reply("gerbers", present, 0, "", "toggle", "14").unwrap();
        assert_eq!(toggled["off"], json!(1 << 14));
        // Existing phones sending kind="board" continue to work unchanged.
        assert!(layer_reply("board", present, 0, "", "", "").is_some());
    }
}
