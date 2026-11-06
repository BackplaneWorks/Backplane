//! multipart/form-data: reading a whole body (a ring's webhook, the mock's
//! view of a request) and writing the transcription request. The wording of
//! the errors is Bun's, which reported them before.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    pub name: String,
    pub filename: Option<String>, // Some makes it a file; a part without one is text
    pub content_type: String,
    pub data: Vec<u8>,
}

const BAD_TYPE: &str = "Can't decode form data from body because of incorrect MIME type/boundary";
const NO_END: &str = "FormData parse error missing final boundary";

/// A `; key=value` parameter of a header, quoted or not.
pub fn param(header: &str, key: &str) -> Option<String> {
    let b = header.as_bytes();
    let mut i = header.find(';')?;
    while i < b.len() {
        i += 1; // the ';'
        while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
            i += 1;
        }
        let start = i;
        while i < b.len() && b[i] != b'=' && b[i] != b';' {
            i += 1;
        }
        let k = header[start..i].trim();
        if i >= b.len() || b[i] == b';' {
            continue;
        }
        i += 1; // the '='
        let mut v = Vec::new();
        if i < b.len() && b[i] == b'"' {
            i += 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' && i + 1 < b.len() {
                    i += 1;
                }
                v.push(b[i]);
                i += 1;
            }
            i += 1; // the closing quote
            while i < b.len() && b[i] != b';' {
                i += 1;
            }
        } else {
            let s = i;
            while i < b.len() && b[i] != b';' {
                i += 1;
            }
            v.extend_from_slice(header[s..i].trim().as_bytes());
        }
        if k.eq_ignore_ascii_case(key) {
            return Some(String::from_utf8_lossy(&v).into_owned());
        }
    }
    None
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from > hay.len() {
        return None;
    }
    hay[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

pub fn parse(body: &[u8], content_type: &str) -> Result<Vec<Part>, String> {
    let boundary = param(content_type, "boundary").filter(|b| !b.is_empty()).ok_or(BAD_TYPE)?;
    let delim = format!("--{boundary}").into_bytes();
    let next = format!("\r\n--{boundary}").into_bytes();
    let mut at = if body.starts_with(&delim) { 0 } else { find(body, &next, 0).ok_or(NO_END)? + 2 };
    let mut parts = vec![];
    loop {
        at += delim.len();
        if body[at..].starts_with(b"--") {
            return Ok(parts);
        }
        // the rest of the boundary's line
        let line_end = find(body, b"\r\n", at).ok_or(NO_END)?;
        at = line_end + 2;
        let (head, data_at) = if body[at..].starts_with(b"\r\n") {
            ("".to_string(), at + 2)
        } else {
            let e = find(body, b"\r\n\r\n", at).ok_or(NO_END)?;
            (String::from_utf8_lossy(&body[at..e]).into_owned(), e + 4)
        };
        // a part ending right after its headers has its boundary on the blank line
        let end = find(body, &next, data_at - 2).ok_or(NO_END)?;
        let data = if end < data_at { vec![] } else { body[data_at..end].to_vec() };
        let (mut name, mut filename, mut ctype) = (None, None, String::new());
        for line in head.split("\r\n") {
            let Some((k, v)) = line.split_once(':') else { continue };
            if k.trim().eq_ignore_ascii_case("content-disposition") {
                name = param(v, "name");
                filename = param(v, "filename");
            } else if k.trim().eq_ignore_ascii_case("content-type") {
                ctype = v.trim().to_string();
            }
        }
        if let Some(name) = name {
            parts.push(Part { name, filename, content_type: ctype, data });
        }
        at = end + 2;
    }
}

/// The file part named `audio`: a text part of that name is not a recording.
pub fn audio_part(parts: Vec<Part>) -> Option<Part> {
    parts.into_iter().find(|p| p.name == "audio").filter(|p| p.filename.is_some())
}

fn quoted(s: &str) -> String {
    s.replace('"', "%22").replace('\r', "%0D").replace('\n', "%0A")
}

/// A request body, as FormData writes one.
pub struct Writer {
    boundary: String,
    out: Vec<u8>,
}

impl Writer {
    pub fn new(boundary: String) -> Self {
        Writer { boundary, out: vec![] }
    }

    pub fn content_type(&self) -> String {
        format!("multipart/form-data; boundary={}", self.boundary)
    }

    pub fn field(&mut self, name: &str, value: &str) {
        let head = format!("--{}\r\nContent-Disposition: form-data; name=\"{}\"\r\n\r\n", self.boundary, quoted(name));
        self.out.extend_from_slice(head.as_bytes());
        self.out.extend_from_slice(value.as_bytes());
        self.out.extend_from_slice(b"\r\n");
    }

    pub fn file(&mut self, name: &str, filename: &str, ctype: &str, data: &[u8]) {
        let ctype = if ctype.is_empty() { "application/octet-stream" } else { ctype };
        let head = format!("--{}\r\nContent-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\nContent-Type: {}\r\n\r\n", self.boundary, quoted(name), quoted(filename), ctype);
        self.out.extend_from_slice(head.as_bytes());
        self.out.extend_from_slice(data);
        self.out.extend_from_slice(b"\r\n");
    }

    pub fn finish(mut self) -> Vec<u8> {
        self.out.extend_from_slice(format!("--{}--\r\n", self.boundary).as_bytes());
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CT: &str = "multipart/form-data; boundary=abc";

    #[test]
    fn a_ring_body_finds_the_audio_part() {
        let mut b = b"--abc\r\nContent-Disposition: form-data; name=\"audio\"; filename=\"note.m4a\"\r\nContent-Type: audio/mp4\r\n\r\n".to_vec();
        let audio = [0u8, 255, 13, 10, 45, 45, 97, 13, 10]; // binary, with a line that looks close to a boundary
        b.extend_from_slice(&audio);
        b.extend_from_slice(b"\r\n--abc\r\nContent-Disposition: form-data; name=\"transcription\"\r\n\r\non-ring text\r\n--abc--\r\n");
        let parts = parse(&b, CT).unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1], Part { name: "transcription".into(), filename: None, content_type: "".into(), data: b"on-ring text".to_vec() });
        let a = audio_part(parts).unwrap();
        assert_eq!((a.filename.as_deref(), a.content_type.as_str()), (Some("note.m4a"), "audio/mp4"));
        assert_eq!(a.data, audio);
    }

    #[test]
    fn a_text_part_called_audio_is_no_recording() {
        let b = b"--abc\r\nContent-Disposition: form-data; name=\"audio\"\r\n\r\nTEXT\r\n--abc--\r\n";
        assert_eq!(audio_part(parse(b, CT).unwrap()), None);
    }

    #[test]
    fn errors_read_as_they_did() {
        assert_eq!(parse(b"x", "multipart/form-data").unwrap_err(), BAD_TYPE);
        assert_eq!(parse(b"no parts", CT).unwrap_err(), NO_END);
        let b = b"--abc\r\nContent-Disposition: form-data; name=\"audio\"; filename=\"\"\r\n\r\nD\r\n--abc\r\n";
        assert_eq!(parse(b, "multipart/form-data; boundary=\"abc\"").unwrap_err(), NO_END);
    }

    #[test]
    fn empty_parts_a_preamble_and_quoted_values() {
        let b = b"preamble\r\n--abc\r\nContent-Disposition: form-data; name=\"a\\\"b\"; filename=\"x;y.wav\"\r\n\r\n\r\n--abc\r\nContent-Disposition: form-data; name=\"c\"\r\n\r\nv\r\n--abc--";
        let p = parse(b, CT).unwrap();
        assert_eq!(p[0].name, "a\"b");
        assert_eq!(p[0].filename.as_deref(), Some("x;y.wav"));
        assert_eq!(p[0].data, b"");
        assert_eq!(p[1].data, b"v");
    }

    #[test]
    fn params_case_and_tokens() {
        assert_eq!(param("multipart/form-data; BOUNDARY=a1;x=y", "boundary").as_deref(), Some("a1"));
        assert_eq!(param("form-data; name=audio", "name").as_deref(), Some("audio"));
        assert_eq!(param("form-data; filename=\"n\"", "name"), None);
    }

    #[test]
    fn what_is_written_reads_back() {
        let mut w = Writer::new("zz".into());
        w.file("file", "a\"b.wav", "audio/wav", &[1, 2, 3]);
        w.field("keywords[]", "AC-42");
        let ct = w.content_type();
        let p = parse(&w.finish(), &ct).unwrap();
        assert_eq!(p[0].filename.as_deref(), Some("a%22b.wav"));
        assert_eq!(p[0].data, vec![1, 2, 3]);
        assert_eq!(p[1].name, "keywords[]");
        assert_eq!(p[1].data, b"AC-42");
    }
}
