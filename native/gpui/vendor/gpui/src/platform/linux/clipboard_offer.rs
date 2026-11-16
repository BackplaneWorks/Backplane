//! What a copied item offers other programs on Linux, X11 and Wayland alike (Backplane patch, PATCH.md). Upstream offered every item as
//! text only, so a copied picture put an empty string on the clipboard: a paste into a browser, GIMP or a chat app got nothing.
//!
//! A picture is offered as `image/png` first (transcoded when it is not a PNG: what every program reads), then in its own format; its strings
//! as text. No `text/uri-list` for a picture's file: Firefox takes a uri-list over the image and pastes the file, which a snap cannot read
//! from a hidden folder (Backplane's attachments are under one): it pasted a picture it could not decode.
use crate::{ClipboardEntry, ClipboardItem, ImageFormat};

pub(crate) const PNG: &str = "image/png";
/// The item's text, under the backends' own names for text (`TEXT_MIME_TYPES`, X11's `UTF8_STRING` and its aliases).
pub(crate) const TEXT: &str = "text/plain;charset=utf-8";

/// `(mime type, bytes)` for each form the item is offered in, the most specific first. Only the first picture is offered.
pub(crate) fn offers(item: &ClipboardItem) -> Vec<(&'static str, Vec<u8>)> {
    let mut out: Vec<(&'static str, Vec<u8>)> = Vec::new();
    let mut text: Option<String> = None;
    let mut pictured = false;
    for entry in item.entries() {
        match entry {
            ClipboardEntry::Image(img) if !pictured && !img.bytes.is_empty() => {
                pictured = true;
                if img.format == ImageFormat::Png {
                    out.push((PNG, img.bytes.clone()));
                } else {
                    match to_png(&img.bytes, img.format) {
                        Some(png) => out.push((PNG, png)),
                        None => log::warn!("clipboard: a {:?} picture could not be made a PNG; offered as it is", img.format),
                    }
                    out.push((img.format.mime_type(), img.bytes.clone()));
                }
            }
            ClipboardEntry::Image(_) => {}
            ClipboardEntry::String(s) => text.get_or_insert_with(String::new).push_str(s.text()),
        }
    }
    if let Some(t) = text {
        out.push((TEXT, t.into_bytes()));
    }
    out
}

/// The bytes offered under `mime`: the text under any of the text names.
pub(crate) fn bytes_for<'a>(offers: &'a [(&'static str, Vec<u8>)], mime: &str) -> Option<&'a [u8]> {
    let text = matches!(mime, "text/plain;charset=utf-8" | "text/plain;charset=UTF-8" | "text/plain" | "UTF8_STRING" | "STRING" | "TEXT");
    offers.iter().find(|(m, _)| *m == mime || (text && *m == TEXT)).map(|(_, b)| b.as_slice())
}

fn to_png(bytes: &[u8], format: ImageFormat) -> Option<Vec<u8>> {
    let img = match format {
        ImageFormat::Png => return Some(bytes.to_vec()),
        ImageFormat::Svg => svg_rgba(bytes)?,
        other => {
            let f = match other {
                ImageFormat::Jpeg => image::ImageFormat::Jpeg,
                ImageFormat::Webp => image::ImageFormat::WebP,
                ImageFormat::Gif => image::ImageFormat::Gif,
                ImageFormat::Bmp => image::ImageFormat::Bmp,
                ImageFormat::Tiff => image::ImageFormat::Tiff,
                ImageFormat::Png | ImageFormat::Svg => unreachable!(),
            };
            image::load_from_memory_with_format(bytes, f).ok()?
        }
    };
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).ok()?;
    Some(out)
}

/// An SVG drawn at its own size (text in it without fonts: the clipboard has no font database).
fn svg_rgba(bytes: &[u8]) -> Option<image::DynamicImage> {
    let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default()).ok()?;
    let size = tree.size().to_int_size();
    let mut pm = resvg::tiny_skia::Pixmap::new(size.width(), size.height())?;
    resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pm.as_mut());
    let rgba: Vec<u8> = pm.pixels().iter().flat_map(|p| { let c = p.demultiply(); [c.red(), c.green(), c.blue(), c.alpha()] }).collect();
    image::RgbaImage::from_raw(size.width(), size.height(), rgba).map(image::DynamicImage::ImageRgba8)
}

// Tested from the client (native/gpui/src/screens/images.rs), through `gpui::linux_clipboard_offers`: this vendored crate's own tests do not run.
