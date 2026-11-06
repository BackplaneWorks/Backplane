//! bp-plot KIND FILE [INFOS]: the Rust plot reader as a command, printing what `backplane --plot KIND FILE INFOS` prints, for comparing
//! it with child and for timing it; exits 2 while the file is half written, 3 with a message when it cannot be read.
//! bp-plot --tree FILE and bp-plot --info FILE SIG IDX answer as `--plot-tree` and `--plot-info` do. Kinds 2, 3 and 4 read 3D models (kicad-cli from
//! BACKPLANE_KICAD_CLI, the mesher as the hub picks it, BACKPLANE_HOME for its managed FreeCAD); bp-plot --render MODEL... is `backplane --render`.
//! bp-plot --frame MODEL.glb OUT writes the model as the client's solid frame ({key, n, mesh}), past the hub's size limit: what `backplane-gpui --solid-demo FILE` measures.
#![allow(dead_code)]
// the process gate (proc.rs, copy.rs): the helper makes its git / kicad-cli / step children through it like the hub
#[path = "../copy.rs"] mod copy;
#[path = "../proc.rs"] mod proc;
#[path = "../kicad/fl.rs"] mod fl;
#[path = "../kicad/sexpr.rs"] mod sexpr;
#[path = "../kicad/strokefont.rs"] mod strokefont;
#[path = "../kicad/strokefont_data.rs"] mod strokefont_data;
#[path = "../kicad/board.rs"] mod board;
#[path = "../kicad/schematic.rs"] mod schematic;
#[path = "../kicad/plot.rs"] mod plot;
#[path = "../kicad/plotgen.rs"] mod plotgen;
#[path = "../kicad/plotread.rs"] mod plotread;
#[path = "../kicad/glb.rs"] mod glb;
#[path = "../kicad/mesh.rs"] mod mesh;
#[path = "../kicad/png.rs"] mod png;
#[path = "../kicad/render.rs"] mod render;
#[path = "../kicad/solid.rs"] mod solid;
#[path = "../kicad/solidread.rs"] mod solidread;
#[path = "../kicad/stepglb.rs"] mod stepglb;
use std::io::Write;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (Some(k), Some(file)) = (a.first(), a.get(1)) else { eprintln!("bp-plot KIND FILE [INFOS]"); std::process::exit(64) };
    if k == "--tree" { let t = plotread::tree(file); println!("{}", plotread::tree_json(&t)); return }
    if k == "--info" { let t = std::fs::read_to_string(file).unwrap_or_default(); println!("{}", plotgen::info_find(&t, a[2].parse().unwrap_or(0), a[3].parse().unwrap_or(0))); return }
    let tools = || stepglb::machine(std::path::Path::new(&std::env::var("BACKPLANE_HOME").unwrap_or_default()));
    if k == "--render" {
        let ms: Vec<&str> = a[1..].iter().map(String::as_str).collect();
        let cwd = std::env::current_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_else(|_| "/".into());
        let r = render::run(&ms, &cwd, &tools());
        if !r.out.is_empty() { println!("{}", r.out) }
        std::process::exit(r.code)
    }
    if k == "--frame" {
        let Some(out) = a.get(2) else { eprintln!("bp-plot --frame MODEL.glb OUT"); std::process::exit(64) };
        let bytes = std::fs::read(file).unwrap_or_default();
        let plot = solidread::plot_of(&bytes, false).unwrap_or_else(|_| { eprintln!("model too big"); std::process::exit(3) });
        let Some(so) = plot.solid else { eprintln!("no triangles"); std::process::exit(3) };
        let text = |f: &mut Vec<u8>, s: &str| { f.push(0x60 | s.len() as u8); f.extend(s.as_bytes()) };
        let mut f = vec![0xa3]; text(&mut f, "key"); text(&mut f, "2|bench"); text(&mut f, "n"); f.push(0x1a); f.extend(so.n.to_be_bytes());
        text(&mut f, "mesh"); f.push(0x5a); f.extend((so.d.len() as u32).to_be_bytes()); f.extend(&so.d);
        std::fs::write(out, f).unwrap(); println!("{} triangles", so.n);
        return
    }
    let kind: u32 = k.parse().unwrap_or(0);
    if kind >= 2 {
        let g = solidread::read(kind, file, &std::env::var("BACKPLANE_KICAD_CLI").unwrap_or_default(), &tools(), a.get(2).map_or("", String::as_str));
        let Some(p) = g.plot else { eprintln!("{}", g.text); std::process::exit(g.code) };
        std::io::stdout().write_all(&plotgen::lines(&p)).unwrap();
        println!();
        return
    }
    match plotread::read(kind, file) {
        plotgen::Read::Plot(p, infos) => {
            if let Some(i) = a.get(2) { plotgen::write_infos(i, &infos) }
            std::io::stdout().write_all(&plotgen::lines(&p)).unwrap();
            println!();
        }
        plotgen::Read::Half => std::process::exit(2),
        plotgen::Read::Fail(m) => { eprintln!("{m}"); std::process::exit(3) }
    }
}
