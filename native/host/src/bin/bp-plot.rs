//! bp-plot KIND FILE [INFOS]: the Rust plot reader as a command, printing what `backplane --plot KIND FILE INFOS` prints, for comparing
//! it with the Bend child and for timing it; exits 2 while the file is half written, 3 with a message when it cannot be read.
//! bp-plot --tree FILE and bp-plot --info FILE SIG IDX answer as `--plot-tree` and `--plot-info` do.
#![allow(dead_code)]
#[path = "../kicad/fl.rs"] mod fl;
#[path = "../kicad/sexpr.rs"] mod sexpr;
#[path = "../kicad/strokefont.rs"] mod strokefont;
#[path = "../kicad/strokefont_data.rs"] mod strokefont_data;
#[path = "../kicad/board.rs"] mod board;
#[path = "../kicad/schematic.rs"] mod schematic;
#[path = "../kicad/plot.rs"] mod plot;
#[path = "../kicad/plotgen.rs"] mod plotgen;
#[path = "../kicad/plotread.rs"] mod plotread;
use std::io::Write;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (Some(k), Some(file)) = (a.first(), a.get(1)) else { eprintln!("bp-plot KIND FILE [INFOS]"); std::process::exit(64) };
    if k == "--tree" { let t = plotread::tree(file); println!("{}", plotread::tree_json(&t)); return }
    if k == "--info" { let t = std::fs::read_to_string(file).unwrap_or_default(); println!("{}", plotgen::info_find(&t, a[2].parse().unwrap_or(0), a[3].parse().unwrap_or(0))); return }
    let kind: u32 = k.parse().unwrap_or(0);
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
