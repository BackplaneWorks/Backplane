//! Backplane KiCad's GUI on the computer's own C library and graphics drivers (Linux).
//!
//! The runtime brings its own loader and glibc (`lib/ld-linux-*.so.*`, `lib/libc.so.6`) and its wrapper `bin/kicad` starts KiCad on
//! them. The graphics driver is the computer's (`libGLX_nvidia.so.0`, Mesa's `libGLX_mesa.so.0`), built for the computer's glibc: on
//! a computer whose glibc is newer than the runtime's, no driver loads (`libm.so.6: version 'GLIBC_2.44' not found`, or the host's
//! `librt.so.1` asking for a private symbol of its own libc), KiCad says "Could not use OpenGL, falling back to software rendering",
//! and the 3D viewer, which has no software path, draws nothing.
//!
//! So where the computer's glibc is at least the runtime's, the GUI is started on the computer's loader with a view of the runtime's
//! `lib/` that leaves out what must be the computer's: glibc itself, the GL dispatch libraries and what they talk to the driver with,
//! and the compiler runtime when the computer's is at least as new (Mesa's LLVM needs the newer libstdc++). Everything else stays the
//! runtime's. The view is a folder of links beside a copy of the wrapper whose last line names the computer's loader; `--library-path`
//! is not inherited, so what KiCad starts (a browser, a Python) runs as it does today.
//!
//! Anything unexpected (another wrapper, an older glibc here, a library that does not resolve) leaves the runtime's own wrapper in use.
use std::path::{Path, PathBuf};

/// glibc's own libraries: always the computer's in the view.
const GLIBC: &[&str] = &["libc.so.6", "libm.so.6", "libmvec.so.1", "libresolv.so.2", "libpthread.so.0", "libdl.so.2", "librt.so.1", "libutil.so.1", "libanl.so.1", "libnsl.so.1", "libthread_db.so.1", "libBrokenLocale.so.1", "libc_malloc_debug.so.0"];
/// What loads the graphics driver, and what the driver shares with the program: always the computer's in the view.
const GRAPHICS: &[&str] = &["libGL.so.1", "libGLX.so.0", "libEGL.so.1", "libGLdispatch.so.0", "libOpenGL.so.0", "libGLESv2.so.2", "libdrm.so.2", "libgbm.so.1"];
/// The compiler's runtime: the computer's when it is at least as new as the runtime's.
const GCC: &[&str] = &["libstdc++.so.6", "libgcc_s.so.1", "libatomic.so.1", "libgomp.so.1"];

/// Whether `name` (a file of the runtime's `lib/`) is left out of the view. `host_gcc`: the computer's compiler runtime is used.
pub fn left_out(name: &str, host_gcc: bool) -> bool {
    name.starts_with("ld-linux") || GLIBC.contains(&name) || GRAPHICS.contains(&name) || name.starts_with("libwayland-") || name.starts_with("libnss_") || (host_gcc && GCC.contains(&name))
}

/// The highest `<prefix><number>` in `bytes` as (major, minor) for "GLIBC_2." -> (2, n), or (0, n) for a prefix that ends in its only dot.
pub fn highest(bytes: &[u8], prefix: &str) -> Option<u32> {
    let p = prefix.as_bytes();
    let mut best = None;
    let mut i = 0;
    while i + p.len() < bytes.len() {
        if &bytes[i..i + p.len()] == p {
            let mut j = i + p.len(); let mut n: u32 = 0; let mut any = false;
            while j < bytes.len() && bytes[j].is_ascii_digit() && n < 100_000 { n = n * 10 + (bytes[j] - b'0') as u32; j += 1; any = true; }
            if any && best.map_or(true, |b| n > b) { best = Some(n); }
            i = j;
        } else { i += 1; }
    }
    best
}
fn highest_in(file: &Path, prefix: &str) -> Option<u32> { highest(&std::fs::read(file).ok()?, prefix) }

/// The wrapper's text with the runtime's place written in and its last line starting KiCad on `loader` with `lib`. None when the wrapper
/// is not the one known (no `root=`, `program=` or `exec "$root/lib/ld-linux` line): then it is used as it is.
pub fn rewrap(wrapper: &str, root: &str, loader: &str, lib: &str) -> Option<String> {
    let q = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let (mut r, mut p, mut e) = (false, false, false);
    let lines: Vec<String> = wrapper.lines().map(|l| {
        if l.starts_with("root=") { r = true; format!("root={}", q(root)) }
        else if l.starts_with("program=") { p = true; "program=kicad".to_string() }
        else if l.starts_with("exec \"$root/lib/ld-linux") && l.contains("\"$root/bin/$program.real\" \"$@\"") { e = true; format!("exec {} --library-path {} \"$root/bin/$program.real\" \"$@\"", q(loader), q(lib)) }
        else { l.to_string() }
    }).collect();
    (r && p && e).then(|| lines.join("\n") + "\n")
}

/// The computer's loader for this build's architecture, when it is there.
fn host_loader() -> Option<&'static str> {
    let l = if cfg!(target_arch = "x86_64") { "/lib64/ld-linux-x86-64.so.2" } else if cfg!(target_arch = "aarch64") { "/lib/ld-linux-aarch64.so.1" } else { return None };
    Path::new(l).is_file().then_some(l)
}
/// The computer's glibc minor version (2.N), from the running hub's own libc.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn host_glibc() -> Option<u32> {
    let v = unsafe { std::ffi::CStr::from_ptr(libc::gnu_get_libc_version()) }.to_str().ok()?;
    v.strip_prefix("2.")?.split('.').next()?.parse().ok()
}
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn host_glibc() -> Option<u32> { None }
/// The computer's libstdc++, where distributions keep it.
fn host_stdcxx() -> Option<PathBuf> {
    let arch = if cfg!(target_arch = "aarch64") { "aarch64-linux-gnu" } else { "x86_64-linux-gnu" };
    [format!("/usr/lib/{arch}/libstdc++.so.6"), format!("/lib/{arch}/libstdc++.so.6"), "/usr/lib64/libstdc++.so.6".into(), "/usr/lib/libstdc++.so.6".into()].into_iter().map(PathBuf::from).find(|p| p.is_file())
}

/// The program to start instead of `<root>/bin/kicad`: the wrapper's copy in the view (made or refreshed here), or None to start the
/// runtime's own. `cache`: a folder of the hub's to keep views in (one per runtime version).
pub fn gui_program(root: &Path, cache: &Path) -> Option<PathBuf> {
    if !cfg!(target_os = "linux") || std::env::var("BACKPLANE_KICAD_HOSTLIBS").is_ok_and(|v| v == "0") { return None }
    let lib = root.join("lib");
    let bundled = highest_in(&lib.join("libc.so.6"), "GLIBC_2.")?; // a runtime with no glibc of its own already runs on the computer's
    let loader = host_loader()?;
    if host_glibc()? < bundled { return None }
    let host_gcc = match (host_stdcxx().and_then(|p| highest_in(&p, "GLIBCXX_3.4.")), highest_in(&lib.join("libstdc++.so.6"), "GLIBCXX_3.4.")) { (Some(h), Some(b)) => h >= b, (Some(_), None) => true, _ => false };
    let wrapper = std::fs::read_to_string(root.join("bin").join("kicad")).ok()?;
    let view = cache.join(root.file_name()?);
    let (vlib, prog, mark) = (view.join("lib"), view.join("kicad"), view.join("made"));
    let text = rewrap(&wrapper, &root.to_string_lossy(), loader, &vlib.to_string_lossy())?;
    let mut names: Vec<String> = std::fs::read_dir(&lib).ok()?.flatten().filter_map(|e| e.file_name().into_string().ok()).filter(|n| !left_out(n, host_gcc)).collect();
    names.sort();
    // what the view was made from: the same again is used as it is (a refused view stays refused until the runtime or the computer changes)
    let stamp = format!("{}\n{loader}\nglibc {:?}>={bundled} gcc {host_gcc}\n{}\n{text}", root.display(), host_glibc(), names.join(" "));
    match std::fs::read_to_string(&mark) {
        Ok(m) if m == format!("ok\n{stamp}") && prog.is_file() => return Some(prog),
        Ok(m) if m == format!("refused\n{stamp}") => return None,
        _ => {}
    }
    let _ = std::fs::remove_dir_all(&view);
    std::fs::create_dir_all(&vlib).ok()?;
    #[cfg(unix)]
    for n in &names { std::os::unix::fs::symlink(lib.join(n), vlib.join(n)).ok()?; }
    std::fs::write(&prog, &text).ok()?;
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&prog, std::fs::Permissions::from_mode(0o755)).ok()?; }
    // every library KiCad and its editors name resolves in the view: else the runtime's own wrapper (software drawing, as before)
    let bin = root.join("bin");
    let ok = ["kicad.real", "_pcbnew.kiface", "_eeschema.kiface"].iter().map(|f| bin.join(f)).filter(|f| f.is_file()).all(|f| {
        crate::proc::std_cmd(crate::proc::Kind::Local, loader).arg("--library-path").arg(&vlib).arg("--list").arg(&f).stdin(std::process::Stdio::null()).output()
            .is_ok_and(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).contains("not found") && !String::from_utf8_lossy(&o.stderr).contains("not found"))
    });
    let _ = std::fs::write(&mark, format!("{}\n{stamp}", if ok { "ok" } else { "refused" }));
    if !ok { eprintln!("kicad embed: {} does not resolve on this computer's libraries: started on its own (software drawing)", root.display()); return None }
    eprintln!("kicad embed: {} starts on this computer's loader and graphics driver (glibc 2.{} here, 2.{bundled} in the runtime)", root.display(), host_glibc().unwrap_or(0));
    Some(prog)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_view_leaves_out_glibc_the_gl_dispatch_and_an_older_compiler_runtime() {
        for n in ["ld-linux-x86-64.so.2", "libc.so.6", "libm.so.6", "libGL.so.1", "libGLX.so.0", "libEGL.so.1", "libGLdispatch.so.0", "libdrm.so.2", "libwayland-client.so.0"] { assert!(left_out(n, false), "{n}"); }
        for n in ["libwx_gtk3u_gl-3.2.so.0", "libGLU.so.1", "libX11.so.6", "libnng.so.1", "libnss3.so", "kicad"] { assert!(!left_out(n, true), "{n}"); }
        assert!(left_out("libstdc++.so.6", true) && !left_out("libstdc++.so.6", false), "the compiler runtime only when the computer's is as new");
    }
    #[test]
    fn the_highest_version_named_in_a_library() {
        assert_eq!(highest(b"xx GLIBC_2.4\0GLIBC_2.43\0GLIBC_2.17\0GLIBC_PRIVATE", "GLIBC_2."), Some(43));
        assert_eq!(highest(b"GLIBCXX_3.4.9\0GLIBCXX_3.4.36\0GLIBCXX_3.4", "GLIBCXX_3.4."), Some(36));
        assert_eq!(highest(b"nothing here", "GLIBC_2."), None);
    }
    #[test]
    fn the_wrapper_is_rewritten_only_when_it_is_the_known_one() {
        let w = "#!/usr/bin/env sh\nset -eu\nroot=$(CDPATH= cd -- \"$(dirname -- \"$0\")/..\" && pwd)\nexport KICAD_KIFACE_HOME=\"$root/bin\"\nprogram=$(basename \"$0\")\nexec \"$root/lib/ld-linux-x86-64.so.2\" --library-path \"$root/lib\" \"$root/bin/$program.real\" \"$@\"\n";
        let t = rewrap(w, "/opt/it's/rt", "/lib64/ld-linux-x86-64.so.2", "/c/v/lib").unwrap();
        assert!(t.contains("root='/opt/it'\\''s/rt'\n") && t.contains("program=kicad\n") && t.contains("export KICAD_KIFACE_HOME=\"$root/bin\"\n"), "{t}");
        assert!(t.ends_with("exec '/lib64/ld-linux-x86-64.so.2' --library-path '/c/v/lib' \"$root/bin/$program.real\" \"$@\"\n"), "{t}");
        assert!(rewrap("#!/bin/sh\nexec /usr/bin/kicad \"$@\"\n", "/r", "/l", "/v").is_none(), "another wrapper is used as it is");
    }
}
