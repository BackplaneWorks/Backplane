//! server/plot's `Pv.child.solid` in Rust: a 3D source read to a plot. A .glb is its own model; a board's is exported by the
//! runtime's kicad-cli and a STEP's meshed by a host tool (kicad/stepglb.rs), each cached by the exact revision of the source and moved
//! into the cache only when it worked. Kinds: 2 a board with its parts, 3 bare, 4 with its outer copper too.
//! Not kept from the child: `<glb>.<build>.plot`, the plot cached beside a GLB (a read here takes milliseconds, not seconds).
use super::plot::Plot;
use super::stepglb::{self, Mesher, Ran};
use super::{glb, plotgen, solid};

#[derive(Debug)]
pub struct Got { pub code: i32, pub text: String, pub plot: Option<Plot> }
fn fail(text: String) -> Got { Got { code: 3, text, plot: None } }

/// Pv.is_step
pub fn is_step(p: &str) -> bool { [".step", ".stp", ".STEP", ".STP"].iter().any(|e| p.ends_with(e)) }
/// FS.revision with Pv.cache.revision: the exact revision of a file as a name ("" when it has none)
fn revision(path: &str) -> String {
    use crate::plat::fs::MetadataExt;
    match std::fs::metadata(path) { Ok(m) => format!("{}.{}.{}.{}.{}.{}.{}", m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec()), Err(_) => String::new() }
}
/// `name` in the folder `dir`, with the system's separator (a Windows cache is `C:\\...\\models\\name`, one path to the hub and its tools alike)
fn in_dir(dir: &str, name: &str) -> String { std::path::Path::new(dir).join(name).to_string_lossy().into_owned() }
/// Pv.step.out, Pv.board.out.recipe: a model's name in the cache, with the source's exact revision unhashed
pub fn cached(dir: &str, path: &str, rev: &str, step: bool, bare: bool, full: bool) -> String {
    let name = if step { format!("step7p-{}.{rev}.normal.glb", stepglb::fnv(path)) }
        else if full { format!("board-full-outer-v1-{}{}.{rev}.glb", if bare { "bare-" } else { "parts-" }, stepglb::fnv(path)) }
        else { format!("board-{}.{rev}.glb", stepglb::fnv(&format!("{}{path}", if bare { "bare3|" } else { "parts3|" }))) };
    in_dir(dir, &name)
}
/// Pv.solid.out: where a model's GLB comes from. A STEP's is named by its content (`step_by_content`), so every copy of a part (the same
/// file in each of a project's worktrees) finds the mesh made once; one meshed under 0.14's name (path and revision) is taken over.
pub fn glb_of(kind: u32, path: &str) -> String { glb_at(kind, path, None) }
/// `glb_of` at a detail: a STEP refined around a box (`stepglb::Detail::refined`) is a mesh of its own beside the first, named by the box.
pub fn glb_at(kind: u32, path: &str, fine: Option<[f64; 6]>) -> String {
    if path.ends_with(".glb") { return path.to_string() }
    if let (Some(b), true) = (fine, is_step(path)) {
        let dir = stepglb::cache_dir();
        let tag = stepglb::fnv(&b.map(|v| format!("{v:.3}")).join(","));
        if let Some(own) = step_by_content(&dir, path) { return own.replace(".normal.glb", &format!(".fine-{tag:08x}.glb")) }
    }
    let dir = stepglb::cache_dir();
    let legacy = cached(&dir, path, &revision(path), is_step(path), kind == 3, kind == 4);
    if !is_step(path) { return legacy }
    let Some(own) = step_by_content(&dir, path) else { return legacy };
    if !stepglb::have(&own) && stepglb::have(&legacy) { take_over(&legacy, &own) }
    own
}
/// A STEP's mesh in the cache by the file's content: the sha256 of its bytes, read once a revision of the file (memoized by its exact
/// revision for the process).
fn step_by_content(dir: &str, path: &str) -> Option<String> {
    use sha2::{Digest, Sha256};
    static KEYS: std::sync::Mutex<Option<std::collections::HashMap<(String, String), String>>> = std::sync::Mutex::new(None);
    let at = (path.to_string(), revision(path));
    if at.1.is_empty() { return None }
    if let Some(k) = KEYS.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&at)) { return Some(in_dir(dir, &format!("step7-{k}.normal.glb"))) }
    let bytes = std::fs::read(path).ok()?;
    let k: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    let mut g = KEYS.lock().unwrap_or_else(|e| e.into_inner()); let m = g.get_or_insert_with(Default::default);
    if m.len() > 4096 { m.clear() }
    m.insert(at, k.clone());
    Some(in_dir(dir, &format!("step7-{k}.normal.glb")))
}
/// A mesh made under its old name, given its content name too: a hard link, else a copy put in place whole.
fn take_over(from: &str, to: &str) {
    if std::fs::hard_link(from, to).is_ok() { return }
    let tmp = format!("{to}.{}.part", std::process::id());
    if std::fs::copy(from, &tmp).is_err() || crate::plat::fs::rename(&tmp, to).is_err() { let _ = std::fs::remove_file(&tmp); }
}
/// Pv.solid.known: the size of a model's GLB when it is already there, else 0
pub fn known(kind: u32, path: &str) -> u64 { std::fs::metadata(glb_of(kind, path)).map(|m| m.len()).unwrap_or(0) }

/// Pv.glb.flags: compact board meshes keep pads, mask and silk (the phone draws copper from its plot); the original model exports outer
/// routing, zones and real via cuts
pub fn flags(bare: bool, full: bool, out: &str, path: &str) -> Vec<String> {
    let mut a: Vec<&str> = vec!["pcb", "export", "glb", if bare { "--no-components" } else { "--subst-models" }, "--include-pads", "--include-soldermask", "--include-silkscreen"];
    if full { a.extend(["--include-tracks", "--include-zones", "--cut-vias-in-body"]) }
    a.extend(["-f", "-o", out, path]);
    a.into_iter().map(str::to_string).collect()
}
/// Pv.missing and Pv.missing.note: how many parts kicad-cli found no 3D model for, said so a bare board does not look like a bug
pub fn missing_note(log: &str) -> String {
    let n = log.lines().filter(|l| l.starts_with("Could not add 3D model")).count();
    if n == 0 { return String::new() }
    format!("{n} {} no 3D model on the hub's machine (KiCad's 3D library is not installed there, or the paths are wrong)", if n == 1 { "part has" } else { "parts have" })
}
/// The staged export's file name: the board's own, because kicad-cli writes its output's stem as the model's `pcb_name`, which names the
/// board's layers (`<pcb_name>_soldermask`...). Other characters than a file name's plain ones become `_`.
pub fn stage_name(path: &str) -> String {
    let stem = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let stem: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' }).collect();
    format!("{}.glb", if stem.is_empty() { "model" } else { &stem })
}
/// Pv.glb.staged: the export, in a folder of its own, promoted to `out` only when it worked; the folder goes either way
fn export(step: bool, bare: bool, full: bool, path: &str, out: &str, cli: &str, tools: &[Mesher], detail: stepglb::Detail, on: Option<&mut dyn FnMut(Streamed)>) -> Ran {
    if !step && cli.is_empty() { return Ran { code: 127, out: "KiCad is not installed on this machine (no kicad-cli): get it from kicad.org/download or its package manager".into() } }
    if let Some(d) = std::path::Path::new(out).parent() { let _ = std::fs::create_dir_all(d); }
    let Ok(dir) = stepglb::stage(out, "export-") else { return Ran { code: 3, out: "Unable to create model staging directory".into() } };
    let stage = dir.join(stage_name(path)); let at = stage.to_string_lossy().into_owned();
    let r = if step {
        // each batch read as it is said (its file goes when the tool is done) and given on as the model's triangles
        let mut on = on;
        let mut each = |e: stepglb::Event| if let Some(f) = on.as_mut() { match e {
            stepglb::Event::Of { leaves, .. } => f(Streamed::Of(leaves)),
            stepglb::Event::Part { leaves, glb, .. } => if let Some((ts, faces)) = std::fs::read(&glb).ok().and_then(|b| part_tris(&b)) { f(Streamed::Part { tris: ts, faces, leaves }) },
        } };
        stepglb::convert_streaming(tools, path, &at, Some(detail), true, Some(&mut each))
    } else {
        let a = flags(bare, full, &at, path);
        let mut argv = vec![cli]; argv.extend(a.iter().map(String::as_str));
        stepglb::run_merged(&argv, true)
    };
    let r = stepglb::publish(r, &stage, out);
    let _ = std::fs::remove_dir_all(&dir);
    r
}
/// What a STEP's conversion gives as it goes: how many placed solids it has, then a batch of them meshed (triangles as `plot_of`'s, in
/// metres and upright, not thinned), in the order they were done. The whole model comes after, as always.
#[derive(Debug)]
pub enum Streamed { Of(u32), Part { tris: Vec<glb::Tri3>, faces: Vec<u8>, leaves: u32 } }
/// A streamed batch's GLB as triangles in the axes and units of the STEP's whole model (`plot_of`), and its faces' runs; None when it has none.
pub fn part_tris(bytes: &[u8]) -> Option<(Vec<glb::Tri3>, Vec<u8>)> {
    let (mut ts, faces) = glb::tris_faces(bytes).ok()?;
    if ts.is_empty() { return None }
    solid::metres(&mut ts); solid::upright(&mut ts);
    let runs = solid::face_runs(&faces, ts.len());
    Some((ts, runs))
}
/// Pv.solid.parse, without the cached text: a model's plot from its GLB
pub fn plot_of(bytes: &[u8], step: bool) -> Result<Plot, glb::TooBig> {
    let (mut ts, faces) = glb::tris_faces(bytes)?;
    if step { solid::metres(&mut ts); solid::upright(&mut ts) } // a part from STEP stands Z up, in metres
    // a STEP is sent as meshed while it fits (the grid of `thin` made small holes in big assemblies jagged), with its B-rep faces when
    // its mesher said them and nothing was thinned; a board as always
    let ts = if step { solid::fit(ts, solid::STEP_TRIS) } else { solid::thin(ts) };
    let runs = if step { solid::face_runs(&faces, ts.len()) } else { vec![] };
    let mut so = solid::of(&ts); if let Some(s) = so.as_mut() { s.faces = runs }
    Ok(solid::plot(so))
}
/// A STEP part meshed into the models cache ahead of its showing (Mech's read-ahead): whether it was meshed now (false: already there,
/// not a STEP, too big a file to mesh unasked, or it failed; a failure is meshed, and said, when the part is shown).
pub fn warm(path: &str, tools: &[Mesher]) -> bool {
    if !is_step(path) || std::fs::metadata(path).map_or(true, |m| m.len() > 48 << 20) { return false }
    let out = glb_of(2, path);
    if stepglb::have(&out) { return false }
    export(true, false, false, path, &out, "", tools, stepglb::NORMAL, None).code == 0
}
/// Pv.child.solid: what `--plot KIND FILE INFOS` does for a 3D model. The note (parts without a model) goes to `infos`, as the child left it.
pub fn read(kind: u32, path: &str, cli: &str, tools: &[Mesher], infos: &str) -> Got { read_streaming(kind, path, cli, tools, infos, None, None) }
/// `read`, a STEP not meshed yet streaming its parts to `on` while it is (a cached one, a board or a GLB says nothing there). `fine`: a
/// STEP refined around that box (its solids there meshed finer: `stepglb::Detail::refined`), kept as a mesh of its own.
pub fn read_streaming(kind: u32, path: &str, cli: &str, tools: &[Mesher], infos: &str, fine: Option<[f64; 6]>, on: Option<&mut dyn FnMut(Streamed)>) -> Got {
    let (isglb, step) = (path.ends_with(".glb"), is_step(path));
    let out = glb_at(kind, path, fine);
    let detail = fine.map_or(stepglb::NORMAL, stepglb::Detail::refined);
    let r = if isglb || stepglb::have(&out) { Ran::default() } else { export(step, kind == 3, kind == 4, path, &out, cli, tools, detail, on) };
    if r.code != 0 { return fail(format!("3D export failed: {}", r.out.trim().chars().take(600).collect::<String>())) }
    plotgen::write_infos(infos, &missing_note(&r.out));
    let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    if !solid::fits_for(size, step) { return fail(solid::big(size)) }
    match plot_of(&std::fs::read(&out).unwrap_or_default(), step) { Ok(p) => Got { code: 0, text: String::new(), plot: Some(p) }, Err(_) => fail(solid::big(size)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_model_is_cached_by_its_exact_revision_and_recipe() {
        let r = "1.2.3.4.5.6.7";
        let c = |n: String| in_dir("/c", &n);
        assert_eq!(cached("/c", "/p/b.kicad_pcb", r, false, false, false), c(format!("board-{}.{r}.glb", stepglb::fnv("parts3|/p/b.kicad_pcb"))));
        assert_eq!(cached("/c", "/p/b.kicad_pcb", r, false, true, false), c(format!("board-{}.{r}.glb", stepglb::fnv("bare3|/p/b.kicad_pcb"))));
        assert_eq!(cached("/c", "/p/b.kicad_pcb", r, false, false, true), c(format!("board-full-outer-v1-parts-{}.{r}.glb", stepglb::fnv("/p/b.kicad_pcb"))));
        assert_eq!(cached("/c", "/p/a.step", r, true, false, false), c(format!("step7p-{}.{r}.normal.glb", stepglb::fnv("/p/a.step"))));
        assert_eq!(glb_of(2, "/p/own.glb"), "/p/own.glb");
    }
    #[test] fn a_steps_mesh_is_found_by_its_content_from_any_copy_and_an_old_name_is_taken_over() {
        let d = std::env::temp_dir().join(format!("bp-stepc-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d);
        for w in ["a/mech", "b/mech"] { std::fs::create_dir_all(d.join(w)).unwrap(); std::fs::write(d.join(w).join("lid.step"), b"ISO-10303-21; lid").unwrap(); }
        let (a, b) = (d.join("a/mech/lid.step").to_string_lossy().into_owned(), d.join("b/mech/lid.step").to_string_lossy().into_owned());
        let dir = d.join("cache").to_string_lossy().into_owned(); std::fs::create_dir_all(&dir).unwrap();
        let (ca, cb) = (step_by_content(&dir, &a).unwrap(), step_by_content(&dir, &b).unwrap());
        assert_eq!(ca, cb, "two worktrees' copies: one mesh");
        let legacy = cached(&dir, &a, &revision(&a), true, false, false); std::fs::write(&legacy, b"glb").unwrap();
        take_over(&legacy, &ca); assert_eq!(std::fs::read(&cb).unwrap(), b"glb", "0.14's mesh serves the copy too");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn the_export_flags_are_the_childs() {
        assert_eq!(flags(false, false, "o", "b").join(" "), "pcb export glb --subst-models --include-pads --include-soldermask --include-silkscreen -f -o o b");
        assert_eq!(flags(true, true, "o", "b").join(" "), "pcb export glb --no-components --include-pads --include-soldermask --include-silkscreen --include-tracks --include-zones --cut-vias-in-body -f -o o b");
    }
    #[test] fn only_a_step_on_disk_is_meshed_ahead() {
        assert!(!warm("/p/b.kicad_pcb", &[]) && !warm("/p/m.glb", &[]) && !warm("/no/such/part.step", &[]));
    }
    #[test] fn the_staged_export_is_named_for_the_board() {
        assert_eq!(stage_name("/p/GNSS/design/Tracker_nRF9251/Tracker_nRF9251.kicad_pcb"), "Tracker_nRF9251.glb");
        assert_eq!(stage_name("/p/a b/my board.kicad_pcb"), "my_board.glb");
        assert_eq!(stage_name("/p/part.step"), "part.glb");
        assert_eq!(stage_name(""), "model.glb");
    }
    /// A real board through the hub's export to what a phone receives. Opt in with the board and a KiCad that opens it:
    /// BACKPLANE_TEST_BOARD=<.kicad_pcb> BACKPLANE_KICAD_CLI=<kicad-cli> cargo test real_board -- --ignored --nocapture
    /// (the GNSS tracker: design/Tracker_nRF9251/Tracker_nRF9251.kicad_pcb with KiCad 10.99).
    #[test] #[ignore] fn a_real_board_exports_to_a_model_a_phone_takes() {
        let (Ok(board), Ok(cli)) = (std::env::var("BACKPLANE_TEST_BOARD"), std::env::var("BACKPLANE_KICAD_CLI")) else { eprintln!("skipped: no BACKPLANE_TEST_BOARD / BACKPLANE_KICAD_CLI"); return };
        let dir = std::env::temp_dir().join(format!("bp-realboard-{}", std::process::id())); std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("board.glb").to_string_lossy().into_owned();
        let t = std::time::Instant::now();
        let r = export(false, false, false, &board, &out, &cli, &[], stepglb::NORMAL, None);
        assert_eq!(r.code, 0, "export: {}", r.out);
        let bytes = std::fs::read(&out).unwrap(); let _ = std::fs::remove_dir_all(&dir);
        let stem = std::path::Path::new(&board).file_stem().unwrap().to_string_lossy().into_owned();
        assert_eq!(glb::kicad_board(&bytes), stem, "the board's layers are known, so their colours are decoded");
        assert!(solid::fits(bytes.len() as u64), "{} bytes: the hub reads it", bytes.len());
        let so = plot_of(&bytes, false).unwrap().solid.expect("triangles");
        let frame = super::super::plot::solid_frame("2|/r|", &so);
        eprintln!("export {:?}, GLB {} B, {} triangles to the phone, frame {} B", t.elapsed(), bytes.len(), so.n, frame.len());
        assert!(so.n as usize <= (64 << 20) / 84, "within the phones' triangle limit (Plot.swift, Plot.kt)");
        assert!(so.d.len() >= so.n as usize * 10, "the phones' sanity bound: at least 10 bytes a triangle");
        assert!(frame.len() < crate_outbox_max() && frame.len() < 16 << 20, "fits a phone's socket (16 MB) and the hub's outbox");
    }
    fn crate_outbox_max() -> usize { 8 << 20 } // hub.rs OUTBOX_MAX (this module also builds into bp-plot, without the hub)
    /// The polygon a round hole is drawn as, at the first detail and zoomed in on (kicad.refine): the top rim of a 3 mm cylinder
    /// (test/files/fixtures/mesh/cylinder_r3.step) through the hub's own read, counted as the distinct corners on it. Opt in with the
    /// mesher: BP_STEP2GLB=<backplane-step2glb> cargo test a_cylinder_is_round -- --ignored --nocapture
    #[test] #[ignore] fn a_cylinder_is_round_enough_at_first_and_rounder_zoomed_in() {
        let Ok(tool) = std::env::var("BP_STEP2GLB") else { eprintln!("skipped: no BP_STEP2GLB"); return };
        let d = work("cyl"); let src = d.join("cylinder_r3.step");
        std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/files/fixtures/mesh/cylinder_r3.step"), &src).unwrap();
        std::env::set_var("XDG_CACHE_HOME", d.join("cache")); // its own models cache: nothing made before is read
        let tools = [Mesher::Step2glb(tool)]; let infos = d.join("i").to_string_lossy().into_owned();
        let rim = |fine: Option<[f64; 6]>| -> usize {
            let g = read_streaming(2, &src.to_string_lossy(), "", &tools, &infos, fine, None);
            assert_eq!(g.code, 0, "{}", g.text);
            let ts = glb::tris(&std::fs::read(glb_at(2, &src.to_string_lossy(), fine)).unwrap()).unwrap();
            let mut pts: Vec<(i64, i64)> = ts.iter().flat_map(|t| [t.a, t.b, t.c]).filter(|p| (p[2] - 10.0).abs() < 1e-4 && ((p[0] * p[0] + p[1] * p[1]).sqrt() - 3.0).abs() < 1e-3)
                .map(|p| ((p[0] * 1e4).round() as i64, (p[1] * 1e4).round() as i64)).collect();
            pts.sort(); pts.dedup(); pts.len()
        };
        let (first, zoomed) = (rim(None), rim(Some([-5.0, -5.0, 0.0, 5.0, 5.0, 12.0])));
        eprintln!("a 3 mm cylinder's rim: {first} segments at first, {zoomed} zoomed in");
        assert!(first >= 63, "at first, 5.7 degrees a segment at most: {first}");
        assert!(zoomed >= 2 * first - 2, "zoomed in, twice as round (2.9 degrees a segment): {zoomed}");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn parts_without_a_model_are_said() {
        assert_eq!(missing_note("ok\n"), "");
        assert_eq!(missing_note("Could not add 3D model for R1.\n"), "1 part has no 3D model on the hub's machine (KiCad's 3D library is not installed there, or the paths are wrong)");
        assert!(missing_note("Could not add 3D model for R1.\nx\nCould not add 3D model for R2.").starts_with("2 parts have no 3D model"));
    }
    #[test] fn steps_by_suffix() { assert!(is_step("a.step") && is_step("a.STP") && !is_step("a.Step") && !is_step("a.glb")); }
    fn work(tag: &str) -> std::path::PathBuf { let d = std::env::temp_dir().join(format!("bp-solidread-{tag}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }
    #[test] fn a_glb_too_big_is_refused_saying_so_and_garbage_is_no_model() {
        let d = work("big"); let f = d.join("m.glb"); let infos = d.join("i").to_string_lossy().into_owned();
        let big = vec![0u8; solid::MOST as usize + 1]; std::fs::write(&f, &big).unwrap();
        let p = f.to_string_lossy();
        let g = read(2, &p, "", &[], &infos); assert_eq!((g.code, g.text.as_str()), (3, "This 3D model is too big to show here (13 MB; the hub reads models up to 12 MB)"));
        std::fs::write(&f, b"not a model").unwrap();
        let g = read(2, &p, "", &[], &infos); assert_eq!(g.code, 0); let pl = g.plot.unwrap(); assert_eq!((pl.solid, pl.thick), (None, 0));
    }
    #[test] fn an_export_with_no_kicad_says_so_and_a_step_with_no_tool_too() {
        let d = work("none"); let infos = d.join("i").to_string_lossy().into_owned();
        let b = d.join("b.kicad_pcb"); std::fs::write(&b, "x").unwrap();
        assert!(read(2, &b.to_string_lossy(), "", &[], &infos).text.starts_with("3D export failed: KiCad is not installed"));
        let s = d.join("a.step"); std::fs::write(&s, "x").unwrap();
        assert_eq!(read(2, &s.to_string_lossy(), "", &[], &infos).text, format!("3D export failed: {}", stepglb::NO_MESHER));
    }
}
