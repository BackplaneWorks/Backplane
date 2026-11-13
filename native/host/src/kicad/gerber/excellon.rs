//! Excellon drill files to holes: the header's tools (T1C0.300), units (METRIC / INCH, with LZ / TZ and a FORMAT), then hits and slots
//! (G85 between two points, or a routed G00..M15 G01.. M16). Decimal coordinates as KiCad writes them, and the integer forms of older
//! tools. Whether a file is plated is told by its X2 attribute (TF.FileFunction,Plated / NonPlated) or its name (-NPTH). A line it
//! cannot read is reported and the rest is read.
use std::collections::HashMap;

pub type P = (f64, f64);

#[derive(Clone, Debug, PartialEq)]
pub enum Hole { Drill { c: P, d: f64 }, Slot { a: P, b: P, d: f64 } }

#[derive(Debug, Default)]
pub struct Drills { pub holes: Vec<(Hole, usize)>, pub plated: Option<bool>, pub errors: Vec<(usize, String)> }

pub fn parse(text: &str) -> Drills {
    let mut out = Drills::default();
    let (mut metric, mut lz, mut fmt) = (true, true, (3u32, 3u32));
    let mut tools: HashMap<u32, f64> = HashMap::new();
    let (mut tool, mut x, mut y) = (0u32, 0.0f64, 0.0f64);
    let mut route: Option<P> = None;
    let mut header = false;
    for (n, raw) in text.lines().enumerate() {
        let line = n + 1;
        let l = raw.trim();
        if l.is_empty() { continue }
        if let Some(c) = l.strip_prefix(';') {
            // KiCad writes X2 attributes as comments: ";#@! TF.FileFunction,Plated,1,4,PTH"
            let c = c.trim().trim_start_matches("#@!").trim();
            if let Some(f) = c.strip_prefix("TF.FileFunction,") { out.plated = Some(!f.contains("NonPlated")) }
            if let Some(f) = c.strip_prefix("FORMAT") { let d: Vec<u32> = f.chars().filter_map(|c| c.to_digit(10)).collect(); if d.len() >= 3 { fmt = (d[d.len() - 2], d[d.len() - 1]) } }
            continue
        }
        match l {
            "M48" => { header = true; continue }
            "%" | "M95" => { header = false; continue }
            "M30" | "M00" => break,
            "G90" | "G05" | "M71" | "M72" | "FMAT,2" | "G93X0Y0" | "ICI,OFF" | "M17" | "G40" => { if l == "M72" { metric = false } if l == "M71" { metric = true } continue }
            _ => {}
        }
        if l.starts_with("METRIC") || l.starts_with("INCH") {
            metric = l.starts_with("METRIC");
            lz = !l.contains("TZ");
            if !l.contains(',') || l.split(',').nth(1).map_or(false, |p| p == "LZ" || p == "TZ") { fmt = if metric { (3, 3) } else { (2, 4) } }
            if let Some(f) = l.split(',').find(|p| p.contains('.') && p.chars().all(|c| c == '0' || c == '.')) { let (a, b) = f.split_once('.').unwrap(); fmt = (a.len() as u32, b.len() as u32) }
            continue
        }
        let k = if metric { 1.0 } else { 25.4 };
        let num = |s: &str| -> Option<f64> {
            if s.contains('.') { return s.parse::<f64>().ok().map(|v| v * k) }
            let (neg, d) = match s.strip_prefix('-') { Some(d) => (true, d), None => (false, s.strip_prefix('+').unwrap_or(s)) };
            if d.is_empty() || !d.bytes().all(|c| c.is_ascii_digit()) { return None }
            let d = if lz { format!("{d:0<w$}", w = (fmt.0 + fmt.1) as usize) } else { d.to_string() };
            let v = d.parse::<f64>().ok()? / 10f64.powi(fmt.1 as i32) * k;
            Some(if neg { -v } else { v })
        };
        if let Some(t) = l.strip_prefix('T') {
            // T1C0.300 (a definition, in the header or not), T1 (a choice), T0 (none)
            let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
            let Ok(id) = digits.parse::<u32>() else { out.errors.push((line, format!("tool {l}"))); continue };
            let rest = &t[digits.len()..];
            if let Some(cp) = rest.find('C') {
                let d: String = rest[cp + 1..].chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                match d.parse::<f64>() { Ok(v) => { tools.insert(id, v * k); } Err(_) => out.errors.push((line, format!("tool size in {l}"))) }
            }
            if !header { tool = id }
            continue
        }
        if header { continue }
        // a hit, a slot, or a route
        let word = |s: &str, k: char| -> Option<String> { let p = s.find(k)?; Some(s[p + 1..].chars().take_while(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.')).collect()) };
        let at = |s: &str, x0: f64, y0: f64| -> Option<P> { Some((match word(s, 'X') { Some(v) => num(&v)?, None => x0 }, match word(s, 'Y') { Some(v) => num(&v)?, None => y0 })) };
        let d = tools.get(&tool).copied();
        if let Some((a, b)) = l.split_once("G85") {
            let (Some(p), Some(dd)) = (at(a, x, y), d) else { out.errors.push((line, format!("slot {l}"))); continue };
            let Some(q) = at(b, p.0, p.1) else { out.errors.push((line, format!("slot {l}"))); continue };
            out.holes.push((Hole::Slot { a: p, b: q, d: dd }, line)); x = q.0; y = q.1;
            continue
        }
        if l.starts_with("G00") { if let Some(p) = at(&l[3..], x, y) { x = p.0; y = p.1; route = Some(p) } continue }
        if l == "M15" { continue }
        if l == "M16" { route = None; continue }
        if l.starts_with("G01") {
            let (Some(p), Some(dd)) = (at(&l[3..], x, y), d) else { out.errors.push((line, format!("route {l}"))); continue };
            if let Some(s) = route { out.holes.push((Hole::Slot { a: s, b: p, d: dd }, line)) }
            route = Some(p); x = p.0; y = p.1;
            continue
        }
        if l.starts_with('X') || l.starts_with('Y') {
            let Some(p) = at(l, x, y) else { out.errors.push((line, format!("hit {l}"))); continue };
            x = p.0; y = p.1;
            match d { Some(dd) => out.holes.push((Hole::Drill { c: p, d: dd }, line)), None => out.errors.push((line, format!("a hit with tool T{tool}, which is not defined"))) }
            continue
        }
        if l.starts_with('G') || l.starts_with('M') { continue }
        out.errors.push((line, format!("unknown line {l}")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    const KICAD: &str = "M48\n; DRILL file {KiCad 10.99} date 2026-11-12\n; FORMAT={-:-/ absolute / metric / decimal}\n; #@! TF.FileFunction,Plated,1,4,PTH\nFMAT,2\nMETRIC\n; #@! TA.AperFunction,Plated,PTH,ViaDrill\nT1C0.300\nT2C1.000\n%\nG90\nG05\nT1\nX101.5Y-52.25\nX102.0Y-52.25\nT2\nX110.0Y-60.0\nX110.0Y-58.0G85X110.0Y-56.0\nM30\n";
    #[test] fn reads_kicad_drills() {
        let d = parse(KICAD);
        assert!(d.errors.is_empty(), "{:?}", d.errors);
        assert_eq!(d.plated, Some(true), "KiCad's X2 attribute in a #@! comment");
        assert_eq!(d.holes.iter().map(|h| h.0.clone()).collect::<Vec<_>>(), vec![
            Hole::Drill { c: (101.5, -52.25), d: 0.3 }, Hole::Drill { c: (102.0, -52.25), d: 0.3 }, Hole::Drill { c: (110.0, -60.0), d: 1.0 }, Hole::Slot { a: (110.0, -58.0), b: (110.0, -56.0), d: 1.0 }]);
    }
    #[test] fn integer_coordinates_and_routes() {
        // inches, leading zeros kept (trailing omitted), 2.4
        let d = parse("M48\nINCH,LZ\nT01C0.0400\n%\nT01\nX01Y005\nG00X0Y0\nM15\nG01X02\nM16\nM30\n");
        assert!(d.errors.is_empty(), "{:?}", d.errors);
        assert_eq!(d.holes[0].0, Hole::Drill { c: (25.4, 12.7), d: 1.016 });
        assert!(matches!(d.holes[1].0, Hole::Slot { a: (x0, _), b: (x1, _), .. } if x0 == 0.0 && (x1 - 50.8).abs() < 1e-9));
        let bad = parse("M48\nMETRIC\nT1C0.3\n%\nT1\nX1.0Y1.0\nXabc\nT9\nX2.0Y2.0\nM30\n");
        assert_eq!(bad.holes.len(), 1); assert_eq!(bad.errors.iter().map(|e| e.0).collect::<Vec<_>>(), [7, 9]);
    }
}
