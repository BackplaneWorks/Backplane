//! server/plot's `Pv.child` and `Pv.child.tree`, with core/hist's comparison and core/hier's sheets, in Rust: what a viewer's
//! source path names (a live file, a file in a commit, a comparison of two versions) read to a plot, and a schematic's sheet tree.
use super::board::{Item, Prim};
use super::fl;
use super::plotgen::{self, Read};
use super::schematic::{head, sheet_prop};
use super::sexpr::{forms, parse, decode, Sx};

/// Hist.kind: 0 a live file, 1 a file in a commit, 2 a comparison
pub fn kind_of(p: &str) -> u8 { if p.starts_with("git\t") { 1 } else if p.starts_with("diff\n") { 2 } else { 0 } }
fn at<'p>(p: &'p str, sep: char, i: usize) -> &'p str { p.split(sep).nth(i).unwrap_or("") }
/// Hist.family: the live file a version or a comparison is of
pub fn family(p: &str) -> String {
    let abs = |q: &str| if kind_of(q) == 1 { at(q, '\t', 4).to_string() } else { q.to_string() };
    if kind_of(p) == 2 { abs(at(p, '\n', 2)) } else { abs(p) }
}
/// Hist.norm: a path inside a commit, "a/./b" and "a/x/../b" folded
fn norm(p: &str) -> String {
    let mut acc: Vec<&str> = vec![];
    for x in p.split('/') {
        if x.is_empty() || x == "." { continue }
        match acc.last() { Some(h) if x == ".." && *h != ".." => { acc.pop(); } _ => acc.push(x) }
    }
    acc.join("/")
}
/// Hist.git.obj: how `git show` names a file in a commit
fn git_obj(p: &str) -> String {
    let n = at(p, '\t', 3).chars().fold(0usize, |a, c| a.wrapping_mul(10).wrapping_add((c as u32).wrapping_sub(48) as usize));
    let rel: String = at(p, '\t', 4).chars().skip(n.saturating_add(1)).collect();
    format!("{}:{}", at(p, '\t', 2), norm(&rel))
}
/// Git.version: the bytes of a version the viewer names; empty when it cannot be read
pub fn version(path: &str) -> Vec<u8> { version_env(path, &[]) }
/// The same, with the hub's git settings. A commit word starting with "-" reads as nothing: to `git show` it is an option (--output writes a file).
pub fn version_env(path: &str, env: &[(&str, &str)]) -> Vec<u8> {
    if kind_of(path) != 1 { return std::fs::read(path).unwrap_or_default() }
    let obj = git_obj(path);
    if obj.starts_with('-') { return vec![] }
    let o = crate::proc::std_cmd(crate::proc::Kind::Local, "git").arg(format!("--git-dir={}", at(path, '\t', 1))).arg("show").arg(obj).current_dir("/").envs(env.iter().copied())
        .stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output();
    match o { Ok(o) if o.status.success() => decode(&o.stdout).into_bytes(), _ => vec![] }
}

// Comparisons: core/hist's Hist.mark
const REMOVED: u32 = 26; const CHANGED: u32 = 27; const ADDED: u32 = 28;
fn on(layer: u32, suffix: &str, i: Item) -> Item {
    let prim = match i.prim { Prim::Tint { ps, .. } => Prim::Poly { ps, w: 0.0, fill: true }, p => p };
    Item { key: format!("{}{suffix}", i.key), sig: i.sig, layer, prim, info: i.info }
}
fn dim(mut i: Item) -> Item { if let Prim::Tint { alpha, .. } = &mut i.prim { *alpha /= 3 } i }
fn box_of(i: &Item) -> [f32; 4] {
    let mut b = [1000000.0f32, 1000000.0, -1000000.0, -1000000.0];
    if let Prim::Poly { ps, .. } = &i.prim { for q in ps { b = [fl::min(b[0], q.x), fl::min(b[1], q.y), fl::max(b[2], q.x), fl::max(b[3], q.y)] } }
    b
}
/// Mark.refill: a pour filled again, its extent the same, dims like what did not change
fn refill(o: &Item, n: &Item) -> bool {
    if !n.info.starts_with("zone") { return false }
    let (a, b) = (box_of(o), box_of(n));
    (0..4).all(|i| (a[i] - b[i]).abs() < 0.001)
}
/// Hist.mark: the comparison of an old version's items with a new one's
fn mark(mut os: Vec<Item>, mut ns: Vec<Item>) -> Vec<Item> {
    os.sort_by(|a, b| a.key.cmp(&b.key)); ns.sort_by(|a, b| a.key.cmp(&b.key));
    let (mut o, mut n) = (os.into_iter().peekable(), ns.into_iter().peekable());
    let mut out = Vec::new();
    loop {
        let c = match (o.peek(), n.peek()) { (None, None) => break, (None, _) => std::cmp::Ordering::Greater, (_, None) => std::cmp::Ordering::Less, (Some(a), Some(b)) => a.key.cmp(&b.key) };
        match c {
            std::cmp::Ordering::Greater => out.push(on(ADDED, "", n.next().unwrap())),
            std::cmp::Ordering::Less => out.push(on(REMOVED, "", o.next().unwrap())),
            std::cmp::Ordering::Equal => {
                let (a, b) = (o.next().unwrap(), n.next().unwrap());
                if a.sig == b.sig || refill(&a, &b) { out.push(dim(b)) } else { out.push(on(REMOVED, "~was", a)); out.push(on(CHANGED, "", b)) }
            }
        }
    }
    out
}
fn in_bits(bits: u32, l: u32) -> bool { bits & (1u32 << l.min(31)) != 0 }
/// Hist.mark.on: the comparison of two versions on some layers (0 every layer)
fn mark_on(bits: u32, os: Vec<Item>, ns: Vec<Item>) -> Vec<Item> {
    if bits == 0 { return mark(os, ns) }
    let (ko, _): (Vec<Item>, Vec<Item>) = os.into_iter().partition(|i| in_bits(bits, i.layer));
    let (kn, rest): (Vec<Item>, Vec<Item>) = ns.into_iter().partition(|i| in_bits(bits, i.layer));
    let mut m = mark(ko, kn);
    m.extend(rest.into_iter().map(dim));
    m
}
/// Pv.side: one side of a comparison: a file that is not there has no items; None for one still being written
fn side(kind: u32, path: &str) -> Option<Vec<Item>> {
    let b = version(path);
    if b.is_empty() { return Some(vec![]) }
    let fs = forms(&b)?;
    Some(if kind == 0 { plotgen::board_items(&fs) } else { plotgen::sch_items(&fs) })
}
fn u32_read(s: &str) -> u32 { if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) { 0 } else { s.parse().unwrap_or(0) } }
/// Pv.child.plot: a source path as a plot (kind 0 board, 1 schematic)
pub fn read(kind: u32, path: &str) -> Read {
    if kind_of(path) == 2 {
        let (a, b) = (side(kind, at(path, '\n', 1)), side(kind, at(path, '\n', 2)));
        let (Some(a), Some(b)) = (a, b) else { return Read::Half };
        let (mut p, t) = plotgen::items_plot(mark_on(u32_read(at(path, '\n', 3)), a, b), kind | plotgen::DIFF);
        p.thick = 0;
        return Read::Plot(Box::new(p), t)
    }
    let fixed = kind_of(path) != 0;
    let b = version(path);
    if b.is_empty() { return if fixed { Read::Fail(plotgen::UNREAD.into()) } else { plotgen::empty(path) } }
    plotgen::of_bytes(kind, &b, fixed)
}

// A schematic's sheets: core/hier
pub struct Kids { pub file: String, pub sheets: Vec<(String, String)> }
/// Hier.dir: the folder of a path, with its trailing slash ("" for a bare name)
fn dir(p: &str) -> &str { p.rfind('/').map(|i| &p[..=i]).unwrap_or("") }
fn resolve(parent: &str, f: &str) -> String { if crate::plat::fs::is_abs(f) { f.to_string() } else { format!("{}{f}", dir(parent)) } }
fn sheet_forms<'b>(fs: &'b Option<Vec<&'b [u8]>>) -> impl Iterator<Item = &'b [u8]> + 'b { fs.iter().flatten().copied().filter(|f| head(f) == b"sheet") }
pub fn kids(file: &str, b: &[u8]) -> Kids {
    let fs = forms(b);
    let sheets = sheet_forms(&fs).map(|f| { let x = parse(f); (sheet_prop(&x, "Sheetname", "Sheet name").to_string(), resolve(file, sheet_prop(&x, "Sheetfile", "Sheet file"))) }).collect();
    Kids { file: file.to_string(), sheets }
}
fn boxes_line(key: &str, b: &[u8]) -> String {
    let fs = forms(b);
    let mut v = vec![key.to_string()];
    for f in sheet_forms(&fs) {
        let x = parse(f);
        let (a, s) = (x.get("at"), x.get("size"));
        v.push(format!("{} {} {} {}", fl::show(a.f(0)), fl::show(a.f(1)), fl::show(a.f(0) + s.f(0)), fl::show(a.f(1) + s.f(1))));
    }
    v.join("\t")
}
fn page_of(p: &Sx) -> String { format!("{}={}", p.arg(0), p.attr("page")) }
fn uids_line(key: &str, b: &[u8]) -> String {
    let fs = forms(b);
    let mut v = vec![key.to_string()];
    for f in sheet_forms(&fs) {
        let x = parse(f);
        let mut e = vec![x.attr("uuid").to_string()];
        for pr in x.get("instances").all("project") { e.extend(pr.all("path").map(page_of)) }
        v.push(e.join("|"));
    }
    v.join("\t")
}
/// Hier.top: the root file's own uuid and its sheet_instances pages
fn top(b: &[u8]) -> String {
    let Some(fs) = forms(b) else { return String::new() };
    let first = |n: &[u8]| fs.iter().copied().find(|f| head(f) == n);
    let mut v = vec![first(b"uuid").map(|f| parse(f).arg(0).to_string()).unwrap_or_default()];
    if let Some(f) = first(b"sheet_instances") { let x = parse(f); v.extend(x.all("path").map(page_of)) }
    v.join("|")
}
const MARK: &[u8] = b"(property \"Reference\" \"";
/// Hier.refs: each placed symbol's reference, found by a scan with no parse (a byte is a char, as the scan keeps it)
fn refs(b: &[u8]) -> Vec<String> {
    let (mut m, mut live, mut cur, mut out): (usize, bool, String, Vec<String>) = (0, false, String::new(), vec![]);
    for &c in b {
        if live {
            if c == 34 {
                if !cur.starts_with('#') && !cur.contains('?') && cur.bytes().any(|d| d.is_ascii_digit()) { out.push(std::mem::take(&mut cur)) } else { cur.clear() }
                live = false; m = 0;
            } else { cur.push(c as char) }
        } else if c == MARK[m] {
            if m + 1 == MARK.len() { live = true; m = 0; cur.clear() } else { m += 1 }
        } else { m = (c == 40) as usize }
    }
    out
}
fn refs_line(key: &str, b: &[u8]) -> String { let mut v = vec![key.to_string()]; v.extend(refs(b)); v.join("\t") }
fn trim_end(s: &str) -> &str { s.trim_end_matches([' ', '\t', '\n', '\u{b}', '\u{c}', '\r']) }

#[derive(Debug, PartialEq)]
struct Row { depth: usize, name: String, file: String, lp: bool }
/// Hier.rows: the whole tree from the root file, pre-order, at most 4096 rows; a sheet that includes an ancestor is listed, marked, and never expanded. `tab` holds the newest entry last.
fn rows(tab: &[Kids], root: &str) -> Vec<Row> {
    struct Td { d: usize, n: String, f: String, above: Vec<String> }
    let mut st = vec![Td { d: 0, n: root.rsplit('/').next().unwrap_or(root).to_string(), f: root.to_string(), above: vec![] }];
    let mut out = vec![];
    while let Some(t) = st.pop() {
        if out.len() == 4096 { break }
        let lp = t.above.contains(&t.f);
        if !lp {
            if let Some(k) = tab.iter().rev().find(|k| k.file == t.f) {
                let mut above = t.above.clone(); above.push(t.f.clone());
                for (n, f) in k.sheets.iter().rev() { st.push(Td { d: t.d + 1, n: n.clone(), f: f.clone(), above: above.clone() }) }
            }
        }
        out.push(Row { depth: t.d, name: t.n, file: t.f, lp });
    }
    out
}
/// `--plot-tree`'s answer
#[derive(Debug, PartialEq, Default)]
pub struct Tree { pub rows: String, pub boxes: String, pub ids: String, pub top: String, pub refs: String }
/// Pv.child.tree: the files the root reaches, read one by one (a sheet that includes an ancestor is listed but never read twice, at most 64 files), and the rows from the root, pre-order
pub fn tree(root: &str) -> Tree {
    let mut tab: Vec<Kids> = vec![]; // newest last
    let mut todo: Vec<String> = vec![root.to_string()];
    let mut bytes: Vec<Vec<u8>> = vec![];
    for _ in 0..64 {
        if todo.is_empty() { break }
        let p = todo.remove(0);
        let b = version(&p);
        let k = kids(&p, &b);
        let ss: Vec<String> = k.sheets.iter().map(|s| s.1.clone()).collect();
        tab.push(k); bytes.push(b);
        for f in ss { if !tab.iter().any(|k| k.file == f) && !todo.contains(&f) { todo.push(f) } }
    }
    let lines = |g: &dyn Fn(&str, &[u8]) -> String| -> String {
        let mut o = String::new();
        for (k, b) in tab.iter().zip(&bytes).rev() { o.push_str(&g(&family(&k.file), b)); o.push('\n') } // Tree.boxes walks the table newest first
        trim_end(&o).to_string()
    };
    let rows: Vec<String> = rows(&tab, root).iter().map(|r| format!("{}\t{}\t{}\t{}", r.depth, if r.lp { "1" } else { "0" }, r.name, family(&r.file))).collect();
    Tree { rows: rows.join("\n"), boxes: lines(&boxes_line), ids: lines(&uids_line), top: top(bytes.first().map(|b| &b[..]).unwrap_or(&[])), refs: lines(&refs_line) }
}
fn quote(s: &str, o: &mut String) {
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), '\n' => o.push_str("\\n"), '\r' => o.push_str("\\r"), '\t' => o.push_str("\\t"),
            c if (c as u32) < 32 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
}
/// what `--plot-tree` prints, for comparing with it
pub fn tree_json(t: &Tree) -> String {
    let mut o = String::from("{");
    for (i, (k, v)) in [("rows", &t.rows), ("boxes", &t.boxes), ("ids", &t.ids), ("top", &t.top), ("refs", &t.refs)].iter().enumerate() {
        if i > 0 { o.push(',') }
        quote(k, &mut o); o.push(':'); quote(v, &mut o);
    }
    o.push('}');
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::board::Pt;
    use std::sync::Arc;
    fn it(key: &str, sig: u32, layer: u32, info: &str, x: f32) -> Item { Item { key: key.into(), sig, layer, prim: Prim::Poly { ps: vec![Pt { x, y: 0.0 }, Pt { x: 10.0, y: 5.0 }], w: 0.0, fill: true }, info: Arc::from(info) } }
    #[test] fn kinds_and_family() {
        assert_eq!(kind_of("/a/b"), 0); assert_eq!(kind_of("git\tg\tc\t2\t/t/x"), 1); assert_eq!(kind_of("diff\na\nb"), 2);
        assert_eq!(family("/a/b"), "/a/b"); assert_eq!(family("git\tg\tc\t2\t/t/x"), "/t/x"); assert_eq!(family("diff\ngit\tg\tc\t2\t/t/x\ngit\tg\td\t2\t/t/y\n5"), "/t/y");
        assert_eq!(norm("a/./b/x/../c"), "a/b/c"); assert_eq!(norm("../a"), "../a"); assert_eq!(git_obj("git\tg\tc0\t2\t/t/s/../x.kicad_sch"), "c0:x.kicad_sch");
    }
    #[test] fn marks_by_key_and_sig() {
        let m = mark(vec![it("a", 1, 5, "", 0.0), it("b", 2, 5, "", 0.0), it("c", 3, 5, "", 0.0)], vec![it("b", 2, 5, "", 0.0), it("c", 9, 5, "", 0.0), it("d", 4, 5, "", 0.0)]);
        let got: Vec<(String, u32)> = m.iter().map(|i| (i.key.clone(), i.layer)).collect();
        assert_eq!(got, [("a".to_string(), REMOVED), ("b".to_string(), 5), ("c~was".to_string(), REMOVED), ("c".to_string(), CHANGED), ("d".to_string(), ADDED)]);
    }
    #[test] fn a_refilled_pour_dims() {
        let m = mark(vec![it("z", 1, 12, "zone;net=G", 0.0)], vec![it("z", 2, 12, "zone;net=G", 0.0001)]); assert_eq!(m.len(), 1); assert_eq!(m[0].layer, 12);
        let m = mark(vec![it("z", 1, 12, "zone", 0.0)], vec![it("z", 2, 12, "zone", 1.0)]); assert_eq!(m.len(), 2);
    }
    // law hist_mark_self: a version compared with itself: every item dimmed, nothing marked
    #[test] fn hist_mark_self() {
        let mut t = it("c", 3, 1, "", 2.0); t.prim = Prim::Tint { ps: vec![Pt { x: 0.0, y: 0.0 }], color: 5, alpha: 90 };
        let v = vec![it("a", 1, 5, "", 0.0), it("b", 2, 6, "", 1.0), t];
        assert_eq!(mark(v.clone(), v.clone()), v.into_iter().map(dim).collect::<Vec<_>>());
    }
    // law rv_mark_on_all: a comparison on no layers in particular is the plain comparison
    #[test] fn rv_mark_on_all() {
        let (o, n) = (vec![it("a", 1, 5, "", 0.0), it("b", 2, 6, "", 1.0)], vec![it("b", 3, 6, "", 1.0), it("c", 4, 7, "", 1.0)]);
        assert_eq!(mark_on(0, o.clone(), n.clone()), mark(o, n));
    }
    // law hier_resolve_absolute, hier_resolve_beside
    #[test] fn hier_resolve_absolute() { for p in ["", "/p/a", "git\tg\tc\t2\t/t/a"] { assert_eq!(resolve(p, "/x/y.kicad_sch"), "/x/y.kicad_sch") } }
    #[test] fn hier_resolve_beside() { assert_eq!(resolve("/p/io/io.kicad_sch", "adc.kicad_sch"), "/p/io/adc.kicad_sch") }
    // law hier_loop_listed_once: a sheet that includes an ancestor is listed once, marked, and never expanded
    #[test] fn hier_loop_listed_once() {
        let k = |f: &str, n: &str, g: &str| Kids { file: f.into(), sheets: vec![(n.into(), g.into())] };
        let r = rows(&[k("/p/b", "a", "/p/a"), k("/p/a", "b", "/p/b")], "/p/a");
        let row = |d, n: &str, f: &str, lp| Row { depth: d, name: n.into(), file: f.into(), lp };
        assert_eq!(r, [row(0, "a", "/p/a", false), row(1, "b", "/p/b", false), row(2, "a", "/p/a", true)]);
    }
    // law hist_sheet_same_commit: a sheet of a file in a commit stays in that commit
    #[test] fn hist_sheet_same_commit() { assert_eq!(resolve("git\t/g\tc\t2\t/t/hw/root.kicad_sch", "kid.kicad_sch"), "git\t/g\tc\t2\t/t/hw/kid.kicad_sch") }
    // law hist_tree_wire_file: the rows sent carry the live file of a version
    #[test] fn hist_tree_wire_file() { assert_eq!(family("git\t/r/.git\tc1\t2\t/r/root.kicad_sch"), "/r/root.kicad_sch") }
    #[test] fn only_the_bits_are_compared() {
        let m = mark_on(1 << 5, vec![it("a", 1, 5, "", 0.0), it("b", 1, 6, "", 0.0)], vec![it("a", 2, 5, "", 0.0), it("b", 2, 6, "", 0.0)]);
        assert_eq!(m.iter().map(|i| i.layer).collect::<Vec<_>>(), [REMOVED, CHANGED, 6]);
    }
    #[test] fn references_scan() {
        let b = br##"(symbol (property "Reference" "R12" (at 0 0)) (property "Reference" "#PWR01") (property "Reference" "U?") (property "Reference" "R") (property "Reference" "C3"))"##;
        assert_eq!(refs(b), ["R12", "C3"]);
    }
    #[test] fn paths_beside_the_parent() { assert_eq!(resolve("/a/b.kicad_sch", "c.kicad_sch"), "/a/c.kicad_sch"); assert_eq!(resolve("/a/b", "/x/c"), "/x/c"); assert_eq!(resolve("b", "c"), "c"); assert_eq!(dir("git\tg\tc\t2\t/t/s/x"), "git\tg\tc\t2\t/t/s/") }
}
