//! Compiles the files stream's pure modules by path, so their tests run without the rest of the hub.
pub mod api { pub struct HttpReply { pub status: &'static str, pub ctype: String, pub extra: String, pub body: Vec<u8> }
  impl HttpReply { pub fn text(status: &'static str, body: &str) -> HttpReply { HttpReply { status, ctype: "text/plain; charset=utf-8".into(), extra: String::new(), body: body.as_bytes().to_vec() } } } }
pub mod files {
  #[path = "../../../../../../native/host/src/files/fm.rs"] pub mod fm;
  #[path = "../../../../../../native/host/src/files/dir.rs"] pub mod dir;
  #[path = "../../../../../../native/host/src/files/serve.rs"] pub mod serve;
}
