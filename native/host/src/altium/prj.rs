//! .PrjPcb: an INI file. `[DocumentN]` sections name the project's documents (`DocumentPath=Sheet.SchDoc`, relative to
//! the project, Windows separators); `[Design]` and others hold settings this reader ignores.

/// The project's documents in file order, as paths relative to the project with '/' separators.
pub fn documents(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_doc = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') { in_doc = l.len() > 9 && l[1..].to_ascii_lowercase().starts_with("document"); continue }
        if !in_doc { continue }
        if let Some((k, v)) = l.split_once('=') {
            if k.trim().eq_ignore_ascii_case("DocumentPath") && !v.trim().is_empty() { out.push(v.trim().replace('\\', "/")) }
        }
    }
    out
}
/// The documents of one kind (by extension, any case).
pub fn of_kind(text: &str, k: super::Kind) -> Vec<String> { documents(text).into_iter().filter(|d| super::kind(d) == Some(k)).collect() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn lists_documents() {
        let t = "[Design]\r\nVersion=1.0\r\n[Document1]\r\nDocumentPath=Top.SchDoc\r\n[Document2]\r\nDocumentPath=sub\\Power.SchDoc\r\n[Document3]\r\nDocumentPath=Board.PcbDoc\r\n[Generic_SmartPDF]\r\nDocumentPath=x.SchDoc\r\n";
        assert_eq!(documents(t), ["Top.SchDoc", "sub/Power.SchDoc", "Board.PcbDoc"]);
        assert_eq!(of_kind(t, crate::files::altium::Kind::Board), ["Board.PcbDoc"]);
    }
}
