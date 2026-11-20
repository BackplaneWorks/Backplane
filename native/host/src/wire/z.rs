//! Frame compression a client asks for with `z=<codecs>` on `/ws` (docs/PHONE-DATA.md, Compression): the first frame goes as it is and names the codec chosen
//! (`"z":"deflate"`), every later text or binary frame goes as a binary frame of raw DEFLATE, one stream for the connection's life (later frames refer back to
//! earlier ones), each frame ended with a sync flush so it decompresses whole on arrival. Control frames (ping, pong, close) are never touched.
//! The codec list is open: a client names its choices in order (`z=zstd,deflate`), the hub takes the first it has, so another codec can come later behind the same switch.
use flate2::{Compress, Compression, FlushCompress, Status};
use futures_util::Sink;
use std::{pin::Pin, task::{Context, Poll}};
use tokio_tungstenite::tungstenite::{Error, Message};

/// The codecs this hub has, by their wire names.
pub const CODECS: &[&str] = &["deflate"];
/// The codec to use for a request's `z` parameter: the first of its comma-separated names this hub has.
pub fn choose(param: &str) -> Option<&'static str> { param.split(',').map(str::trim).find_map(|w| CODECS.iter().copied().find(|c| *c == w)) }

/// One connection's compressor.
pub struct Deflater(Compress);
impl Deflater {
    /// Level 6 (zlib's default), raw DEFLATE, a 32 KiB window.
    pub fn new() -> Self { Self(Compress::new(Compression::new(6), false)) }
    /// One frame's bytes, compressed and sync-flushed: decompressing them on the connection's stream gives exactly `input`.
    pub fn frame(&mut self, input: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(input.len() / 3 + 64);
        let start = self.0.total_in();
        loop {
            let done = (self.0.total_in() - start) as usize;
            if out.capacity() - out.len() < 64 { out.reserve(out.capacity().max(1024)) }
            let st = self.0.compress_vec(&input[done..], &mut out, FlushCompress::Sync).unwrap_or(Status::StreamEnd);
            // done when every byte is in and the flush is out: the output was not filled up
            if (self.0.total_in() - start) as usize == input.len() && out.len() < out.capacity() { break }
            if st == Status::StreamEnd { break }
        }
        out
    }
}
impl Default for Deflater { fn default() -> Self { Self::new() } }

/// A WebSocket sink that compresses data frames once `arm`ed (after the uncompressed first frame); with no codec it passes everything through.
pub struct ZSink<S> { inner: S, z: Option<Deflater>, armed: bool }
impl<S> ZSink<S> {
    pub fn new(inner: S, codec: Option<&str>) -> Self { Self { inner, z: codec.filter(|c| *c == "deflate").map(|_| Deflater::new()), armed: false } }
    /// The first frame is out: compress from now on.
    pub fn arm(&mut self) { self.armed = true }
    pub fn compressing(&self) -> bool { self.z.is_some() }
    fn pack(&mut self, m: Message) -> Message {
        let (Some(z), true) = (self.z.as_mut(), self.armed) else { return m };
        match m { Message::Text(t) => Message::Binary(z.frame(t.as_bytes()).into()), Message::Binary(b) => Message::Binary(z.frame(&b).into()), other => other }
    }
}
impl<S: Sink<Message, Error = Error> + Unpin> Sink<Message> for ZSink<S> {
    type Error = Error;
    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> { Pin::new(&mut self.inner).poll_ready(cx) }
    fn start_send(mut self: Pin<&mut Self>, m: Message) -> Result<(), Error> { let m = self.pack(m); Pin::new(&mut self.inner).start_send(m) }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> { Pin::new(&mut self.inner).poll_flush(cx) }
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> { Pin::new(&mut self.inner).poll_close(cx) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Decompress, FlushDecompress};
    /// What a client does: one decompressor for the connection, each frame fed whole.
    struct Inflater(Decompress);
    impl Inflater {
        fn new() -> Self { Self(Decompress::new(false)) }
        fn frame(&mut self, input: &[u8]) -> Vec<u8> {
            let mut out = Vec::with_capacity(input.len() * 4 + 64); let start = self.0.total_in();
            loop {
                let done = (self.0.total_in() - start) as usize;
                if out.capacity() - out.len() < 64 { out.reserve(out.capacity()) }
                self.0.decompress_vec(&input[done..], &mut out, FlushDecompress::Sync).unwrap();
                if (self.0.total_in() - start) as usize == input.len() && out.len() < out.capacity() { break }
            }
            out
        }
    }
    #[test] fn a_client_names_its_codecs_in_order_and_gets_the_first_the_hub_has() {
        assert_eq!(choose("deflate"), Some("deflate")); assert_eq!(choose("zstd, deflate"), Some("deflate"), "an unknown codec first: the next one");
        assert_eq!(choose(""), None); assert_eq!(choose("zstd"), None); assert_eq!(choose("DEFLATE"), None, "names are exact");
    }
    #[test] fn each_frame_decompresses_whole_on_one_stream_and_repeats_cost_little() {
        let (mut z, mut u) = (Deflater::new(), Inflater::new());
        let page: String = (0..60).map(|i| format!("{{\"id\":\"m{i}\",\"kind\":\"assistant\",\"text\":\"Step {i}: the 3V3 rail is routed and DRC reports no errors.\"}}")).collect::<Vec<_>>().join(",");
        let frames = [page.clone(), "{\"t\":\"hb\"}".to_string(), page.clone(), String::new(), "x".repeat(300_000)];
        let mut sizes = vec![];
        for f in &frames { let c = z.frame(f.as_bytes()); assert!(c.ends_with(&[0, 0, 0xff, 0xff]), "sync-flushed"); assert_eq!(u.frame(&c), f.as_bytes()); sizes.push(c.len()); }
        assert!(sizes[0] * 5 < page.len(), "a page compresses: {} of {}", sizes[0], page.len());
        assert!(sizes[2] * 4 < sizes[0], "the same page again refers back to the first: {} after {}", sizes[2], sizes[0]);
        assert!(sizes[4] < 4096, "a long run is tiny: {}", sizes[4]);
    }
    #[tokio::test] async fn the_first_frame_goes_as_it_is_and_control_frames_are_never_touched() {
        use futures_util::SinkExt;
        let (tx, mut rx) = futures_channel_like();
        let mut s = ZSink::new(tx, choose("deflate"));
        s.send(Message::Text("{\"t\":\"hello\",\"z\":\"deflate\"}".into())).await.unwrap(); s.arm();
        s.send(Message::Text("{\"t\":\"reply\"}".into())).await.unwrap(); s.send(Message::Ping(vec![1].into())).await.unwrap();
        let got: Vec<Message> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert!(matches!(&got[0], Message::Text(t) if t.as_str().contains("hello")), "the hello as it is");
        let Message::Binary(b) = &got[1] else { panic!("a compressed frame is binary") }; assert_eq!(Inflater::new().frame(b), b"{\"t\":\"reply\"}");
        assert!(matches!(&got[2], Message::Ping(_)));
        let mut plain = ZSink::new(futures_channel_like().0, None); plain.arm(); assert!(!plain.compressing());
    }
    /// A sink over a std channel, for the test above.
    fn futures_channel_like() -> (Chan, std::sync::mpsc::Receiver<Message>) { let (t, r) = std::sync::mpsc::channel(); (Chan(t), r) }
    struct Chan(std::sync::mpsc::Sender<Message>);
    impl Sink<Message> for Chan {
        type Error = Error;
        fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> { Poll::Ready(Ok(())) }
        fn start_send(self: Pin<&mut Self>, m: Message) -> Result<(), Error> { let _ = self.0.send(m); Ok(()) }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> { Poll::Ready(Ok(())) }
        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> { Poll::Ready(Ok(())) }
    }
}
