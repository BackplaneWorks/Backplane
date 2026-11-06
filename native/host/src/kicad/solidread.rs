//! server/plot's `Pv.child.solid` in Rust: a 3D source read to a plot. A .glb is its own model; a board's is exported by the
//! runtime's kicad-cli and a STEP's meshed by a host tool (kicad/stepglb.rs), each cached by the content of the source (solidread::content) and moved
//! into the cache only when it worked. Kinds: 2 a board with its parts, 3 bare, 4 with its outer copper too.
//! The solid made of a cached GLB is kept beside it (`.solid1`), so a model read again after a restart or from another worktree is not parsed again.
use super::plot::Plot;
use super::stepglb::{self, Mesher, Ran};
use super::{glb, plotgen, solid};

#[derive(Debug)]
pub struct Got { pub code: i32, pub text: String, pub plot: Option<Plot> }
fn fail(text: String) -> Got { Got { code: 3, text, plot: None } }

/// Pv.is_step
pub fn is_step(p: &str) -> bool { [".step", ".stp", ".STEP", ".STP"].iter().any(|e| p.ends_with(e)) }
/// FS.revision with Pv.cache.revision: the exact revision of a file as a name ("" when it has none). Names the cache from before
/// models were kept by content (`legacy`).
fn revision(path: &str) -> String {
    use std::os::unix::fs::MetadataExt;
    match std::fs::metadata(path) { Ok(m) => format!("{}.{}.{}.{}.{}.{}.{}", m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec()), Err(_) => String::new() }
}
/// A file's content as a name: the digest of its bytes (plot::digest) and its length, kept per revision so a big STEP is read once
/// while it does not change ("" when it cannot be read).
pub fn content(path: &str) -> String {
    static SEEN: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, (String, String)>>> = std::sync::OnceLock::new();
    let rev = revision(path); if rev.is_empty() { return String::new() }
    let m = SEEN.get_or_init(Default::default);
    if let Some((r, c)) = m.lock().unwrap().get(path) { if *r == rev { return c.clone() } }
    let Ok(b) = std::fs::read(path) else { return String::new() };
    let (a, z) = super::plot::digest(&b); let c = format!("{a:016x}{z:016x}-{}", b.len());
    let mut g = m.lock().unwrap(); if g.len() > 512 { g.clear() } g.insert(path.to_string(), (rev, c.clone()));
    c
}
/// Pv.step.out, Pv.board.out.recipe: a model's name in the cache, by the recipe and the source's content: the same board in another
/// worktree, or checked out again unchanged, has its model already (its 3D model files are taken to be the same: they are found by the
/// same paths from the project and the libraries).
pub fn cached(dir: &str, path: &str, rev: &str, step: bool, bare: bool, full: bool) -> String {
    let _ = path;
    if step { format!("{dir}/step4-{rev}.normal.glb") }
    else if full { format!("{dir}/board-full-outer-v2-{}{rev}.glb", if bare { "bare-" } else { "parts-" }) }
    else { format!("{dir}/board-v2-{}{rev}.glb", if bare { "bare-" } else { "parts-" }) }
}
/// The name a model had before (path and inode revision): a model made then is taken over rather than made again.
fn legacy(dir: &str, path: &str, step: bool, bare: bool, full: bool) -> String {
    let rev = revision(path);
    if step { format!("{dir}/step3-{}.{rev}.normal.glb", stepglb::fnv(path)) }
    else if full { format!("{dir}/board-full-outer-v1-{}{}.{rev}.glb", if bare { "bare-" } else { "parts-" }, stepglb::fnv(path)) }
    else { format!("{dir}/board-{}.{rev}.glb", stepglb::fnv(&format!("{}{path}", if bare { "bare3|" } else { "parts3|" }))) }
}
/// Pv.solid.out: where a model's GLB comes from
pub fn glb_of(kind: u32, path: &str) -> String {
    if path.ends_with(".glb") { return path.to_string() }
    cached(&stepglb::cache_dir(), path, &content(path), is_step(path), kind == 3, kind == 4)
}
/// Pv.solid.known: the size of a model's GLB when it is already there, else 0
pub fn known(kind: u32, path: &str) -> u64 { std::fs::metadata(glb_of(kind, path)).map(|m| m.len()).unwrap_or(0) }
/// The model is made (its GLB is in the cache, or is the file itself): reading it is a matter of a second, not an export.
pub fn ready(kind: u32, path: &str) -> bool { path.ends_with(".glb") || known(kind, path) > 0 || stepglb::have(&legacy(&stepglb::cache_dir(), path, is_step(path), kind == 3, kind == 4)) }

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
/// Pv.glb.staged: the export, in a folder of its own, promoted to `out` only when it worked; the folder goes either way
fn export(step: bool, bare: bool, full: bool, path: &str, out: &str, cli: &str, tools: &[Mesher]) -> Ran {
    if !step && cli.is_empty() { return Ran { code: 127, out: "KiCad is not installed on this machine (no kicad-cli): get it from kicad.org/download or its package manager".into() } }
    if let Some(d) = std::path::Path::new(out).parent() { let _ = std::fs::create_dir_all(d); }
    let Ok(dir) = stepglb::stage(out, "export-") else { return Ran { code: 3, out: "Unable to create model staging directory".into() } };
    let stage = dir.join("model.glb"); let at = stage.to_string_lossy().into_owned();
    let r = if step { stepglb::convert(tools, path, &at, Some(stepglb::NORMAL), true) } else {
        let a = flags(bare, full, &at, path);
        let mut argv = vec![cli]; argv.extend(a.iter().map(String::as_str));
        stepglb::run_merged(&argv, true)
    };
    let r = stepglb::publish(r, &stage, out);
    let _ = std::fs::remove_dir_all(&dir);
    r
}
/// Pv.solid.parse, without the cached text: a model's plot from its GLB
pub fn plot_of(bytes: &[u8], step: bool) -> Result<Plot, glb::TooBig> {
    let mut ts = glb::tris(bytes)?;
    if step { solid::metres(&mut ts); solid::upright(&mut ts) } // a part from STEP stands Z up, in metres
    let ts = solid::fit(ts, solid::TRIS);
    Ok(solid::plot(solid::of(&ts)))
}
/// The solid made of a cached GLB, kept beside it (`.solid1`: n, box, mesh): a model read again after a restart, or in another
/// worktree, is not parsed and thinned again. A GLB of the project's own is not written beside.
fn solid_file(glb: &str) -> String { format!("{glb}.solid1") }
fn solid_load(glb: &str) -> Option<Plot> {
    let b = std::fs::read(solid_file(glb)).ok()?;
    let w = |i: usize| b.get(4 + 4 * i..8 + 4 * i).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]));
    if b.get(..4)? != b"bps1" { return None }
    let (n, bx) = (w(0)?, [w(1)?, w(2)?, w(3)?, w(4)?, w(5)?, w(6)?]);
    Some(solid::plot(Some(super::plot::Solid { n, b: bx, d: b.get(32..)?.to_vec() })))
}
fn solid_save(glb: &str, p: &Plot) {
    let Some(so) = &p.solid else { return };
    let mut o = b"bps1".to_vec(); o.extend(so.n.to_le_bytes()); for v in so.b { o.extend(v.to_le_bytes()) } o.extend(&so.d);
    let (f, part) = (solid_file(glb), format!("{}.part", solid_file(glb)));
    if std::fs::write(&part, &o).is_ok() { let _ = std::fs::rename(&part, &f); }
}
/// Pv.child.solid: what `--plot KIND FILE INFOS` does for a 3D model. The note (parts without a model) goes to `infos`, as the child left it.
pub fn read(kind: u32, path: &str, cli: &str, tools: &[Mesher], infos: &str) -> Got {
    let (isglb, step) = (path.ends_with(".glb"), is_step(path));
    let out = glb_of(kind, path);
    if !isglb && !stepglb::have(&out) { // made under the name it had before: taken over
        let old = legacy(&stepglb::cache_dir(), path, step, kind == 3, kind == 4);
        if stepglb::have(&old) && std::fs::hard_link(&old, &out).is_err() { let _ = std::fs::copy(&old, &out); }
    }
    let r = if isglb || stepglb::have(&out) { Ran::default() } else { export(step, kind == 3, kind == 4, path, &out, cli, tools) };
    if r.code != 0 { return fail(format!("3D export failed: {}", r.out.trim().chars().take(160).collect::<String>())) }
    plotgen::write_infos(infos, &missing_note(&r.out));
    let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    if !solid::fits(size) { return fail(solid::big(size)) }
    if !isglb { if let Some(p) = solid_load(&out) { return Got { code: 0, text: String::new(), plot: Some(p) } } }
    match plot_of(&std::fs::read(&out).unwrap_or_default(), step) { Ok(p) => { if !isglb { solid_save(&out, &p) } Got { code: 0, text: String::new(), plot: Some(p) } } Err(_) => fail(solid::big(size)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_model_is_cached_by_its_content_and_recipe() {
        let r = "0123456789abcdef0123456789abcdef-7";
        assert_eq!(cached("/c", "/p/b.kicad_pcb", r, false, false, false), format!("/c/board-v2-parts-{r}.glb"));
        assert_eq!(cached("/c", "/p/b.kicad_pcb", r, false, true, false), format!("/c/board-v2-bare-{r}.glb"));
        assert_eq!(cached("/c", "/p/b.kicad_pcb", r, false, false, true), format!("/c/board-full-outer-v2-parts-{r}.glb"));
        assert_eq!(cached("/c", "/p/a.step", r, true, false, false), format!("/c/step4-{r}.normal.glb"));
        assert_eq!(glb_of(2, "/p/own.glb"), "/p/own.glb");
        let d = work("content"); for w in ["a", "b"] { std::fs::write(d.join(w), "(kicad_pcb)").unwrap() }
        let (a, b) = (d.join("a").to_string_lossy().into_owned(), d.join("b").to_string_lossy().into_owned());
        assert_eq!(glb_of(2, &a), glb_of(2, &b), "the same board in two folders: one model"); assert_ne!(glb_of(2, &a), glb_of(3, &a));
        std::fs::write(&b, "(kicad_pcb )").unwrap(); assert_ne!(glb_of(2, &a), glb_of(2, &b), "other bytes: another model");
    }
    #[test] fn a_made_solid_is_kept_beside_its_model() {
        let d = work("keep"); let g = d.join("m.glb").to_string_lossy().into_owned();
        let p = solid::plot(Some(super::super::plot::Solid { n: 2, b: [1, 2, 3, 4, 5, u32::MAX], d: vec![9, 8, 7] }));
        assert!(solid_load(&g).is_none()); solid_save(&g, &p); assert_eq!(solid_load(&g), Some(p));
        std::fs::write(solid_file(&g), b"junk").unwrap(); assert!(solid_load(&g).is_none());
    }
    #[test] fn the_export_flags_are_the_childs() {
        assert_eq!(flags(false, false, "o", "b").join(" "), "pcb export glb --subst-models --include-pads --include-soldermask --include-silkscreen -f -o o b");
        assert_eq!(flags(true, true, "o", "b").join(" "), "pcb export glb --no-components --include-pads --include-soldermask --include-silkscreen --include-tracks --include-zones --cut-vias-in-body -f -o o b");
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
        let big = std::fs::File::create(&f).unwrap(); big.set_len(solid::MOST + 1).unwrap(); drop(big); // sparse: no 64 MB written
        let p = f.to_string_lossy();
        let g = read(2, &p, "", &[], &infos); assert_eq!((g.code, g.text.as_str()), (3, "This 3D model is too big to show here (65 MB; the hub reads models up to 64 MB)"));
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
