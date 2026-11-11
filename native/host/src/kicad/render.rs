//! app/render's `backplane --render MODEL` in Rust, with core/mech's paths: a part's standard views as PNG files beside it,
//! `renders/<stem>.<view>.png`. A STEP is meshed once (the models cache) and stood Z up; a GLB is read as it is. Each view is fitted to
//! the whole part and drawn by kicad/mesh.rs, 800 x 600, written through a `.part` file and a rename. `Ran` holds what the command
//! prints and its exit code, so Mech.error (files/misc.rs) reads it as it read the child.
use super::stepglb::{self, Mesher, Ran};
use super::{glb, mesh, png, solid};

pub const W: u32 = 800;
pub const H: u32 = 600;
/// More file than this is not read into memory for a render.
const MOST_FILE: u64 = 1 << 30;
/// Mech.views: a camera direction by name, on a part stood Z up
pub const VIEWS: [(&str, f32, f32); 4] = [("iso", 0.785398, 0.61548), ("front", 0.0, 0.0), ("top", 0.0, 1.570796), ("right", 1.570796, 0.0)];

/// Mech.last
/// (a separator is `/`, and on Windows `\\` too: `C:\\p\\a.step`'s renders go in `C:\\p\\renders`)
fn last(p: &str) -> &str { p.rsplit(crate::plat::fs::SEPS).next().unwrap_or(p) }
/// Mech.parent: the folder a path is in ("" for none)
pub fn parent(p: &str) -> &str { p[..p.len() - last(p).len()].strip_suffix(crate::plat::fs::SEPS).unwrap_or("") }
/// Mech.stem: a file's name without its extension
pub fn stem(p: &str) -> &str { let f = last(p); f.rsplit_once('.').map_or(f, |x| x.0) }
fn inside(dir: &str, f: &str) -> String { if dir.is_empty() { f.to_string() } else { format!("{dir}/{f}") } }
/// Mech.render: where a model's render from a view goes
pub fn path_of(model: &str, view: &str) -> String { inside(&inside(parent(model), "renders"), &format!("{}.{view}.png", stem(model))) }
/// Mech.is_step
pub fn is_step(p: &str) -> bool { let l = p.to_lowercase(); l.ends_with(".step") || l.ends_with(".stp") }

/// Parts.out: where the model's GLB is kept (a .glb is its own)
fn glb_path(src: &str, cache: &str) -> String {
    use crate::plat::fs::MetadataExt;
    if src.ends_with(".glb") { return src.to_string() }
    let size = std::fs::metadata(src).map_or("0".to_string(), |m| format!("{}.{}", m.len() as u32, m.mtime() as u32));
    std::path::Path::new(cache).join(format!("{}.glb", stepglb::fnv(&format!("step3|{src}|{size}")))).to_string_lossy().into_owned()
}
/// Parts.glb: the model's GLB, from the cache, else converted into it; Err: why it cannot be ("" for a GLB that is not there)
fn model_glb(src: &str, cache: &str, tools: &[Mesher]) -> Result<String, String> {
    let out = glb_path(src, cache);
    if stepglb::have(&out) { return Ok(out) }
    if src.ends_with(".glb") { return Err(String::new()) }
    let dir = stepglb::stage(&out, "part.").map_err(|e| format!("could not make a folder beside {out}: {e}"))?;
    let from = dir.join("model.glb");
    let r = stepglb::publish(stepglb::convert(tools, src, &from.to_string_lossy(), Some(stepglb::NORMAL), false), &from, &out);
    let _ = std::fs::remove_dir_all(&dir);
    if r.code == 0 { Ok(out) } else { Err(r.out.trim().to_string()) }
}
/// Render.write and Render.publish: the file appears whole or not at all
fn write(path: &str, bs: &[u8]) -> bool {
    let part = format!("{path}.part");
    let ok = std::fs::write(&part, bs).is_ok() && crate::plat::fs::rename(&part, path).is_ok();
    if !ok { let _ = std::fs::remove_file(&part); }
    ok
}
/// Render.view: every view fitted to the whole part; a view that cannot be written does not stop the others
fn views(ts: &[glb::Tri3], model: &str, say: &mut Vec<String>) -> bool {
    let (lo, hi) = mesh::bounds(ts); let cam = mesh::fit(lo, hi);
    let mut all = true;
    for (name, yaw, pitch) in VIEWS {
        let out = path_of(model, name);
        let ok = write(&out, &png::rgb(&mesh::render(W, H, ts, &cam.face(yaw, pitch), mesh::GROUND), W, H));
        say.push(if ok { out } else { format!("backplane --render: could not write {out}") });
        all &= ok;
    }
    all
}
/// Render.run: one model
fn one(model: &str, cwd: &str, cache: &str, tools: &[Mesher], say: &mut Vec<String>) -> bool {
    let path = if crate::plat::fs::is_abs(model) { model.to_string() } else { format!("{cwd}/{model}") };
    let mut failed = |why: String| { say.push(format!("backplane --render: {why}")); false };
    let _ = std::fs::create_dir_all(cache);
    let file = match model_glb(&path, cache, tools) { Ok(f) => f, Err(why) => {
        let why = if why.is_empty() { String::new() } else { format!(": {}", why.chars().take(600).collect::<String>()) };
        return failed(format!("could not mesh {path} (a STEP or GLB file; a large STEP needs FreeCAD installed){why}. Nothing was drawn.")) } };
    if std::fs::metadata(&file).map_or(0, |m| m.len()) > MOST_FILE { return failed(format!("{path} is too big to draw here. Nothing was drawn.")) }
    let Ok(mut ts) = glb::tris(&std::fs::read(&file).unwrap_or_default()) else { return failed(format!("{path} has too many triangles to draw here. Nothing was drawn.")) };
    if is_step(&path) { solid::upright(&mut ts) }
    if ts.is_empty() { return failed(format!("no renderable triangles in {path}. Nothing was drawn.")) }
    if let Err(e) = std::fs::create_dir_all(parent(&path_of(&path, "iso"))) { return failed(format!("could not create render directory for {path}: {e}")) }
    views(&ts, &path, say)
}
/// Render.main: every model, the rest going on after one that failed; exit 1 if any did.
pub fn run(models: &[&str], cwd: &str, tools: &[Mesher]) -> Ran {
    let (mut say, cache) = (vec![], stepglb::cache_dir());
    let mut ok = true;
    for m in models { ok &= one(m, cwd, &cache, tools, &mut say) }
    if !ok { say.push("backplane --render: one or more models could not be rendered".into()) }
    Ran { code: if ok { 0 } else { 1 }, out: say.join("\n") }
}

#[cfg(test)]
mod tests {
    use super::*;
    // law mech_render_path: a part's render from a view goes to renders/ beside it, named by view
    #[test] fn mech_render_path() { assert_eq!(path_of("mech/bracket.step", "iso"), "mech/renders/bracket.iso.png"); }
    #[test] fn paths_as_mech_says() {
        assert_eq!(path_of("/p/a.b.glb", "top"), "/p/renders/a.b.top.png"); assert_eq!(path_of("x", "top"), "renders/x.top.png"); assert_eq!(path_of("/x.glb", "top"), "renders/x.top.png");
        assert_eq!(parent("a/b/c.step"), "a/b"); assert_eq!(parent("c"), ""); assert_eq!(stem("a/.hid"), ""); assert_eq!(stem("a/noext"), "noext");
        assert!(is_step("/p/A.STEP") && is_step("a.Stp") && !is_step("a.glb"));
        #[cfg(windows)] { assert_eq!(path_of(r"C:\p dir\a.step", "iso"), r"C:\p dir/renders/a.iso.png"); assert_eq!(parent(r"C:\p\a.step"), r"C:\p"); }
    }
    fn work(tag: &str) -> String { let d = std::env::temp_dir().join(format!("bp-render-{tag}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d.to_string_lossy().into_owned() }
    fn cube() -> Vec<u8> {
        let v = |x: f32, y: f32, z: f32| [x, y, z];
        let (a, b, c, d, e, f, g, h) = (v(0.,0.,0.), v(1.,0.,0.), v(1.,1.,0.), v(0.,1.,0.), v(0.,0.,1.), v(1.,0.,1.), v(1.,1.,1.), v(0.,1.,1.));
        let q = [[a,c,b],[a,d,c],[e,f,g],[e,g,h],[a,b,f],[a,f,e],[b,c,g],[b,g,f],[c,d,h],[c,h,g],[d,a,e],[d,e,h]];
        let fs: Vec<f32> = q.iter().flatten().flatten().copied().collect();
        glb::tests::glb(&format!(r#"{{"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0}}],"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}}}}]}}],"accessors":[{{"bufferView":0,"count":{},"type":"VEC3"}}],"bufferViews":[{{}}]}}"#, fs.len() / 3), &glb::tests::floats(&fs))
    }
    #[test] fn a_model_gets_every_view_as_a_png_beside_it() {
        let d = work("ok"); std::fs::create_dir_all(format!("{d}/mech")).unwrap(); std::fs::write(format!("{d}/mech/box.glb"), cube()).unwrap();
        let r = run(&["mech/box.glb"], &d, &[]); assert_eq!(r.code, 0, "{}", r.out);
        let names: Vec<String> = VIEWS.iter().map(|v| format!("{d}/mech/renders/box.{}.png", v.0)).collect();
        assert_eq!(r.out, names.join("\n"));
        for n in names { let b = std::fs::read(&n).unwrap(); assert_eq!(&b[1..4], b"PNG"); assert!(!std::path::Path::new(&format!("{n}.part")).exists()) }
    }
    // law mech_render_failure_diagnostic and Mech.error read this: a failed model names itself, the others still run
    #[test] fn a_bad_model_is_said_and_the_rest_are_drawn() {
        let d = work("bad"); std::fs::write(format!("{d}/box.glb"), cube()).unwrap(); std::fs::write(format!("{d}/empty.glb"), "glTF").unwrap();
        let r = run(&["none.glb", "empty.glb", "box.glb", "x.step"], &d, &[]);
        assert_eq!(r.code, 1); let ls: Vec<&str> = r.out.lines().collect();
        assert_eq!(ls[0], format!("backplane --render: could not mesh {d}/none.glb (a STEP or GLB file; a large STEP needs FreeCAD installed). Nothing was drawn."));
        assert_eq!(ls[1], format!("backplane --render: no renderable triangles in {d}/empty.glb. Nothing was drawn."));
        assert_eq!(ls[2], format!("{d}/renders/box.iso.png"));
        assert!(ls[6].contains("could not mesh")); assert_eq!(ls[7], "backplane --render: one or more models could not be rendered");
    }
}
