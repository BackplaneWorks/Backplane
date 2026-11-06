//! Compiles the files stream's pure modules by path, so their tests run without the rest of the hub.
pub mod api { pub struct HttpReply { pub status: &'static str, pub ctype: String, pub extra: String, pub body: Vec<u8> }
  impl HttpReply { pub fn text(status: &'static str, body: &str) -> HttpReply { HttpReply { status, ctype: "text/plain; charset=utf-8".into(), extra: String::new(), body: body.as_bytes().to_vec() } } } }
#[path = "../../../../../native/host/src/copy.rs"] pub mod copy;
pub mod files {
  #[path = "../../../../../../native/host/src/files/fm.rs"] pub mod fm;
  #[path = "../../../../../../native/host/src/files/dir.rs"] pub mod dir;
  #[path = "../../../../../../native/host/src/files/serve.rs"] pub mod serve;
  #[path = "../../../../../../native/host/src/files/git.rs"] pub mod git;
  #[path = "../../../../../../native/host/src/files/attach.rs"] pub mod attach;
  #[path = "../../../../../../native/host/src/files/mimg.rs"] pub mod mimg;
  #[path = "../../../../../../native/host/src/files/threadpath.rs"] pub mod threadpath;
}
pub mod kicad {
  #[path = "../../../../../../native/host/src/kicad/plot.rs"] pub mod plot;
  #[path = "../../../../../../native/host/src/kicad/project.rs"] pub mod project;
  #[path = "../../../../../../native/host/src/kicad/boards.rs"] pub mod boards;
  #[path = "../../../../../../native/host/src/kicad/kserve.rs"] pub mod kserve;
  #[path = "../../../../../../native/host/src/kicad/fl.rs"] pub mod fl;
  #[path = "../../../../../../native/host/src/kicad/sexpr.rs"] pub mod sexpr;
  #[path = "../../../../../../native/host/src/kicad/strokefont.rs"] pub mod strokefont;
  #[path = "../../../../../../native/host/src/kicad/strokefont_data.rs"] pub mod strokefont_data;
  #[path = "../../../../../../native/host/src/kicad/board.rs"] pub mod board;
  #[path = "../../../../../../native/host/src/kicad/schematic.rs"] pub mod schematic;
  #[path = "../../../../../../native/host/src/kicad/plotgen.rs"] pub mod plotgen;
  #[path = "../../../../../../native/host/src/kicad/plotread.rs"] pub mod plotread;
  #[path = "../../../../../../native/host/src/kicad/glb.rs"] pub mod glb;
  #[path = "../../../../../../native/host/src/kicad/mesh.rs"] pub mod mesh;
  #[path = "../../../../../../native/host/src/kicad/png.rs"] pub mod png;
  #[path = "../../../../../../native/host/src/kicad/render.rs"] pub mod render;
  #[path = "../../../../../../native/host/src/kicad/solid.rs"] pub mod solid;
  #[path = "../../../../../../native/host/src/kicad/solidread.rs"] pub mod solidread;
  #[path = "../../../../../../native/host/src/kicad/stepglb.rs"] pub mod stepglb;
  #[path = "../../../../../../native/host/src/kicad/bj.rs"] pub mod bj;
  #[path = "../../../../../../native/host/src/kicad/cad.rs"] pub mod cad;
  #[path = "../../../../../../native/host/src/kicad/bom.rs"] pub mod bom;
  #[path = "../../../../../../native/host/src/kicad/checks.rs"] pub mod checks;
  #[path = "../../../../../../native/host/src/kicad/release.rs"] pub mod release;
  #[path = "../../../../../../native/host/src/kicad/review.rs"] pub mod review;
  #[path = "../../../../../../native/host/src/kicad/kexport.rs"] pub mod kexport;
}

// The hub's process gate itself (native/host/src/proc.rs, copy.rs): the files modules call crate::proc.
#[path = "../../../../../native/host/src/proc.rs"] pub mod proc;
