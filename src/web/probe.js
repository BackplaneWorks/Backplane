// Cross-probing: what a click on the board, the schematic or the 3D model
// picks, what a probed part or net is made of in each, and so what each
// viewer lights up. Pure: no DOM, no canvas (plot2d.js and solid.js draw,
// host.js dispatches). Test: node --test desktop/test/probe.test.cjs.
//
// Every piece a viewer draws names its item's info (core/board.bend's and
// core/schematic.bend's "kind;key=value;..."): a pad says its net and its
// footprint's ref, a track or zone its net, a footprint's lines and texts
// their ref; on a schematic a symbol says its ref, a pin its number and
// symbol, and wires, junctions, labels, pins and sheet pins the group of
// the sheet they are connected in (g, schematic.bend's Conn).
//
// A probe is text, as the hub keeps it per project and board:
//   "c\tR12"                     a part (its ref, without a unit's letter)
//   "n\tGND\tR1.2 U3.7"          a net: the board's name for it when known,
//                                and a few of its pins (ref.pin), which
//                                find it again where the name does not
//   ""                           nothing

// "kind;k=v;k=v" (a value may hold "=")
export function parseInfo(s) {
  const parts = String(s || "").split(";");
  const kv = new Map();
  for (let i = 1; i < parts.length; i += 1) {
    const p = parts[i], e = p.indexOf("=");
    if (e > 0) kv.set(p.slice(0, e), p.slice(e + 1));
  }
  const head = parts[0] || "";
  const sp = head.indexOf(" ");
  return { kind: sp < 0 ? head : head.slice(0, sp), arg: sp < 0 ? "" : head.slice(sp + 1), kv };
}

// a ref without the letter a multi-unit symbol's unit adds ("U1A" is U1's)
export function baseRef(ref) {
  const m = /^(.*\d)[A-Z]+$/.exec(String(ref || ""));
  return m ? m[1] : String(ref || "");
}

// a power symbol (its value names a net everywhere); KiCad's PWR_FLAG is
// no net's name
export function isPower(info) {
  const ref = info.kv.get("ref") || "";
  return info.kind === "symbol" && ref.startsWith("#") && !ref.startsWith("#FLG");
}

// what an item is, as far as probing goes: the part it belongs to, the
// board net it is on, the schematic group it is in, its pin ("R1.2")
export function what(s) {
  const info = typeof s === "string" ? parseInfo(s) : s;
  const k = info.kind;
  const r = { kind: k, ref: "", net: "", g: info.kv.get("g") || "", pin: "", name: "" };
  if (k === "pad") {
    r.net = info.kv.get("net") || "";
    r.ref = info.kv.get("ref") || "";
    if (r.ref && info.arg) r.pin = `${r.ref}.${info.arg}`;
  } else if (k === "track" || k === "via" || k === "zone") {
    r.net = info.kv.get("net") || "";
  } else if (k === "pin") {
    r.ref = baseRef(info.kv.get("symbol") || "");
    // (a power symbol's or flag's pin is on no board)
    if (r.ref && info.arg && !r.ref.startsWith("#")) r.pin = `${r.ref}.${info.arg}`;
  } else if (k === "symbol") {
    if (isPower(info)) r.name = info.kv.get("value") || "";
    else r.ref = baseRef(info.kv.get("ref") || "");
  } else if (k === "label" || k === "global" || k === "hier") {
    r.name = info.kv.get("text") || "";
  } else if (k === "sheet" && info.arg === "pin") {
    r.name = info.kv.get("name") || "";
  } else {
    // a footprint's lines, arcs, polygons and texts carry its ref
    r.ref = info.kv.get("ref") || "";
  }
  return r;
}

// the probe text a part or a net makes
export function partProbe(ref) {
  return ref ? `c\t${ref}` : "";
}

export function netProbe(net, pins = []) {
  if (!net && !pins.length) return "";
  return `n\t${net || ""}\t${pins.slice(0, 8).join(" ")}`;
}

export function parseProbe(s) {
  const f = String(s || "").split("\t");
  if (f[0] === "c" && f[1]) return { kind: "c", ref: f[1], net: "", pins: [] };
  if (f[0] === "n" && (f[1] || f[2])) return { kind: "n", ref: "", net: f[1] || "", pins: (f[2] || "").split(" ").filter(Boolean) };
  return { kind: "", ref: "", net: "", pins: [] };
}

// Picking
// -------
// What lies under the pointer, each a candidate {info, area, layer}: as on
// the phones (mobile/view.bend's Pick.best), the smallest wins (a pad over
// the zone it sits in), then the one drawn on top. A board's copper (pads,
// tracks, vias) comes first, then a footprint's own drawing, then a
// footprint whose box holds the point, then a zone; on a schematic a pin,
// wire, label or junction before the symbol body around it.

function rank(c) {
  const i = c.info, w = c.what || (c.what = what(i));
  if (i.kind === "zone") return 4;
  if (w.net || w.g || w.name) return 0;
  if (i.kind === "symbol") return 2;
  if (w.ref) return 1;
  return 5;
}

function over(a, b) {
  const ra = rank(a), rb = rank(b);
  if (ra !== rb) return ra < rb;
  if (a.area !== b.area) return a.area < b.area;
  return a.layer >= b.layer;
}

// the candidate a click picks, or null (only the page, a board's outline
// or a note under it: nothing to probe)
export function best(cands) {
  let b = null;
  for (const c of cands) {
    if (!c.info) c.info = parseInfo(c.text);
    if (!b || over(c, b)) b = c;
  }
  return b && rank(b) < 5 ? b : null;
}

// a footprint whose box holds the point wins over a zone and over nothing
// (rank 3: a click on a part's body between its lines)
export function partOrBest(b, part) {
  if (part && (!b || rank(b) >= 3)) return { info: parseInfo(`footprint;ref=${part}`), area: 0, layer: 0 };
  return b;
}

// What a click on the board picks, from what lies right under the pointer
// (exact) and what lies within a few pixels of it (near), and the smallest
// part whose box holds the point: copper or a part's drawing right under
// it; else that part (a click on its body: a small part at a far zoom has
// copper all round it); else copper near it; else the zone under it
export function pickBoard(exact, near, part) {
  const e = best(exact);
  if (e && rank(e) < 3) return e;
  if (part) return partOrBest(null, part);
  return best(near) || e;
}

// what a click on a schematic picks: what lies right under the pointer (a
// pin before the symbol it is on), else what lies near it
export function pickSheet(exact, near) {
  return best(exact) || best(near);
}

// Info files
// ----------
// The hub's info file for a plot (core/plot.bend's Infos.file): a line per
// chunk, "<sig> <chars> <table>", the table its infos each ended by a NUL,
// <chars> code points long. Read into sig -> infos.
export function parseInfos(text) {
  const out = new Map();
  const t = String(text || "");
  let i = 0;
  const num = () => {
    let n = 0, any = false;
    while (i < t.length) {
      const c = t.charCodeAt(i);
      if (c < 48 || c > 57) break;
      n = n * 10 + (c - 48); i += 1; any = true;
    }
    return any ? n : -1;
  };
  while (i < t.length) {
    const sig = num();
    if (sig < 0 || t[i] !== " ") break;
    i += 1;
    const chars = num();
    if (chars < 0 || t[i] !== " ") break;
    i += 1;
    // chars code points, which may be two UTF-16 units each
    let j = i, n = 0;
    while (n < chars && j < t.length) { const c = t.codePointAt(j); j += c > 0xffff ? 2 : 1; n += 1; }
    const tab = t.slice(i, j);
    i = j;
    if (t[i] === "\n") i += 1;
    const items = tab.split("\u0000");
    if (items.length && items[items.length - 1] === "") items.pop();
    out.set(sig >>> 0, items);
  }
  return out;
}

// The board's nets and parts
// --------------------------

// every pad's net by its pin ("R1.2" -> "GND"), and each net's pins, from
// a board's info tables (sig -> infos)
export function boardNets(tables) {
  const pin = new Map(), pins = new Map();
  for (const items of tables.values()) {
    for (const s of items) {
      if (!s.startsWith("pad ")) continue;
      const w = what(s);
      if (!w.pin || !w.net) continue;
      pin.set(w.pin, w.net);
      if (!pins.has(w.net)) pins.set(w.net, new Set());
      pins.get(w.net).add(w.pin);
    }
  }
  return { pin, pins };
}

// the board net a probe means: its name when the board has it, else the net
// of one of its pins ("" for none)
export function boardNet(p, nets) {
  if (p.kind !== "n") return "";
  if (p.net && (!nets || nets.pins.has(p.net))) return p.net;
  if (nets) for (const q of p.pins) { const n = nets.pin.get(q); if (n) return n; }
  return p.net;
}

// The schematic's groups
// ----------------------
// A sheet's connected groups (g), each with its pins and the names its
// labels and power symbols give it, from the sheet's info tables.
export function sheetGroups(tables) {
  const gs = new Map();
  const get = (g) => {
    if (!gs.has(g)) gs.set(g, { pins: new Set(), global: new Set(), local: new Set() });
    return gs.get(g);
  };
  for (const items of tables.values()) {
    for (const s of items) {
      if (s.indexOf(";g=") < 0) continue;
      const i = parseInfo(s), w = what(i);
      if (!w.g) continue;
      const e = get(w.g);
      if (w.pin) e.pins.add(w.pin);
      if (i.kind === "global" || (i.kind === "symbol" && w.name)) { if (w.name) e.global.add(w.name); }
      else if (w.name) e.local.add(w.name);
    }
  }
  return gs;
}

// the groups of a sheet a net probe lights: those holding one of the net's
// pins (the board's, else the probe's own), or named by it: a power symbol
// or global label of its name, a local or hierarchical label whose name
// ends it (a net "/Power/VBUS" is the label VBUS on the sheet at /Power/)
export function groupsOf(p, gs, nets, place = "/") {
  const out = new Set();
  if (p.kind !== "n") return out;
  const net = boardNet(p, nets);
  const pins = new Set(p.pins);
  if (nets && net && nets.pins.has(net)) for (const q of nets.pins.get(net)) pins.add(q);
  const tail = net.lastIndexOf("/") >= 0 ? net.slice(net.lastIndexOf("/") + 1) : "";
  for (const [g, e] of gs) {
    let hit = false;
    for (const q of e.pins) if (pins.has(q)) { hit = true; break; }
    if (!hit && net && e.global.has(net)) hit = true;
    if (!hit && net) for (const l of e.local) if (net === `${place}${l}` || (tail && l === tail && net.startsWith("/"))) { hit = true; break; }
    if (hit) out.add(g);
  }
  return out;
}

// the probe a click on a schematic net makes: the board's net of its pins
// when the board is known, else the name its power symbol or global label
// gives, else its local label's under the sheet's place; with its pins
export function groupProbe(e, nets, place = "/") {
  const pins = [...e.pins].sort();
  let net = "";
  if (nets) for (const q of pins) { const n = nets.pin.get(q); if (n) { net = n; break; } }
  if (!net && e.global.size) net = [...e.global].sort()[0];
  if (!net && e.local.size) {
    const l = [...e.local].sort()[0], full = `${place}${l}`;
    net = nets && !nets.pins.has(full) ? ([...nets.pins.keys()].find((n) => n === l || n.endsWith(`/${l}`)) || full) : full;
  }
  return netProbe(net, pins);
}

// What a click probes
// -------------------
// side "board": copper probes its net (a pad without one, its part); a
// footprint's drawing its part. side "schematic": a pin, wire, label,
// junction or power symbol probes its group's net; a symbol its part.
// Nothing (or the page, a board's outline) clears.
export function clickProbe(c, side, ctx = {}) {
  if (!c) return "";
  const w = what(c.info || c.text);
  if (side === "board") {
    if (w.net) return netProbe(w.net, w.pin ? [w.pin] : []);
    return partProbe(w.ref);
  }
  if (w.g && ctx.groups && ctx.groups.has(w.g)) return groupProbe(ctx.groups.get(w.g), ctx.nets, ctx.place);
  if (w.g) return netProbe(w.name, w.pin ? [w.pin] : []);
  return partProbe(w.ref);
}

// What lights up
// --------------
// A test for each piece's info: a part lights its pads, lines and texts on
// the board, its symbol, fields and pins on a schematic; a net lights its
// pads, tracks, vias and zones on the board, and on a schematic the groups
// groupsOf finds.
export function lit(p, side, ctx = {}) {
  if (p.kind === "c") {
    return (s) => {
      const w = what(s);
      return w.ref !== "" && w.ref === p.ref && (side === "board" || w.kind === "symbol" || w.kind === "pin");
    };
  }
  if (p.kind === "n") {
    if (side === "board") {
      const net = boardNet(p, ctx.nets);
      return (s) => net !== "" && what(s).net === net;
    }
    const gs = groupsOf(p, ctx.groups || new Map(), ctx.nets, ctx.place);
    return (s) => { const g = what(s).g; return g !== "" && gs.has(g); };
  }
  return () => false;
}

// the chip's words for a probe
export function label(p) {
  if (p.kind === "c") return p.ref;
  if (p.kind === "n") return p.net.startsWith("Net-") ? p.net : `Net ${p.net || p.pins[0] || "?"}`;
  return "";
}
