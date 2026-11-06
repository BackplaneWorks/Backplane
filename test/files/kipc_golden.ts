// protobufjs's own answers for proto.rs to match: bun kipc_golden.ts DESC CASES.mjs-style list below -> OUT.json
// Run from a copy with tools/kipc's node_modules installed (the repository keeps none):
//   bun test/files/kipc_golden.ts fixture test/fixtures/kipc/api.desc test/files/fixtures/kipc_golden_fixture.json
//   bun test/files/kipc_golden.ts runtime <runtime>/share/backplane/api.desc test/files/fixtures/kipc_golden_runtime.json
import { fromBytes } from "../../../tools/kipc/src/schema.ts";
import { readFileSync, writeFileSync } from "node:fs";

const [which, descFile, outFile] = process.argv.slice(2);
const schema = fromBytes(readFileSync(descFile));
const TO = { longs: String, enums: String, bytes: String, defaults: false, json: true } as const;
const bc = "kiapi.board.commands", ct = "kiapi.common.types", bt = "kiapi.board.types", cc = "kiapi.common.commands";

const fixture: [string, unknown][] = [
  [`${cc}.Ping`, {}], [`${cc}.GetVersion`, { nothing: 1 }], [`${cc}.Ping`, 5], [`${cc}.Ping`, "x"], [`${cc}.Ping`, []],
  [`${cc}.GetVersionResponse`, { version: { major: 10, minor: 99, patch: 0, full_version: "10.99.0-bp.4" } }],
  [`${cc}.GetVersionResponse`, { version: 7 }], [`${cc}.GetVersionResponse`, { version: null }], [`${cc}.GetVersionResponse`, { version: [] }],
  [`${cc}.KiCadVersion`, { major: "12", minor: 1.9, patch: -1 }], [`${cc}.KiCadVersion`, { major: true, full_version: 5 }],
  [`${cc}.KiCadVersion`, { major: "abc", full_version: true }], [`${cc}.KiCadVersion`, { full_version: null }], [`${cc}.KiCadVersion`, { full_version: {} }], [`${cc}.KiCadVersion`, { full_version: [1, 2] }],
  [`${cc}.KiCadVersion`, { fullVersion: "camel is not a field name here" }],
  [`${bc}.GetItems`, { types: ["track", "via"] }], [`${bc}.GetItems`, { types: [] }], [`${bc}.GetItems`, { types: "track" }], [`${bc}.GetItems`, { types: [1, true, null] }], [`${bc}.GetItems`, { types: 0 }],
  [`${bc}.Track`, { id: { value: "00000000-0000-0000-0000-000000000001" }, net: "GND", width: "250000" }], [`${bc}.Track`, { width: 300000 }], [`${bc}.Track`, { width: "-5" }], [`${bc}.Track`, { width: { low: 5, high: 1 } }], [`${bc}.Track`, { width: "" }], [`${bc}.Track`, { width: 1.5 }],
  [`${bc}.GetItemsResponse`, { items: [{ "@type": `${bc}.Track`, id: { value: "a" }, net: "N", width: "1" }, { "@type": `type.googleapis.com/${bc}.KIID`, value: "b" }] }],
  [`${bc}.GetItemsResponse`, { items: [{ "@type": "example.com/no.Such", x: 1 }] }], [`${bc}.GetItemsResponse`, { items: [{ type_url: "t/x", value: "AQID" }] }], [`${bc}.GetItemsResponse`, { items: [{ "@type": `.${bc}.KIID`, value: "c" }] }], [`${bc}.GetItemsResponse`, { items: [5] }],
  ["kiapi.backplane.Shutdown", { force: 1 }], ["kiapi.backplane.Shutdown", { force: "" }], ["kiapi.backplane.Shutdown", { force: "no" }],
  ["kiapi.backplane.GetCapabilitiesResponse", { fork_version: "x", api_version: 2, capabilities: ["a", "b"], commands: [] }],
];

const big = { value_nm: "9007199254740993" };
const runtime: [string, unknown][] = [
  ...["Ping", "GetVersion"].map((c) => [`${cc}.${c}`, {}] as [string, unknown]),
  [`${cc}.GetItems`, { types: ["KOT_PCB_TRACE", "KOT_PCB_VIA"], header: { document: { type: "DOCTYPE_PCB", board_filename: "x.kicad_pcb" } } }],
  [`${cc}.GetItems`, { types: ["KOT_PCB_TRACE", "nope", 3, 12345, 1.5, null] }], [`${cc}.GetItems`, { types: "KOT_PCB_TRACE" }], [`${cc}.GetItems`, { header: 1 }], [`${cc}.GetItems`, { header: { document: { type: 1 } } }], [`${cc}.GetItems`, { header: { document: { type: "NOPE" } } }],
  [`${cc}.GetItems`, { header: { document: { board_filename: "a", lib_id: { library_nickname: "l" } } } }],
  [`${cc}.GetItems`, { header: { document: { type: "DOCTYPE_PCB", project: { name: "p", path: "/x" } }, container: { value: "k" }, field_mask: { paths: ["a.b", "c"] } } }],
  [`${cc}.GetItems`, { header: { field_mask: "a,b" } }], [`${cc}.GetItems`, { header: { field_mask: { paths: "a" } } }],
  [`${cc}.UpdateItems`, { items: [{ "@type": `type.googleapis.com/${bt}.Track`, id: { value: "k" }, start: { x_nm: "1", y_nm: -2 }, end: { x_nm: 3000000000, y_nm: "-4000000000" }, width: big, locked: "LS_LOCKED", layer: "BL_F_Cu", net: { name: "GND" } }] }],
  [`${cc}.UpdateItems`, { items: [{ "@type": `${bt}.Via`, id: { value: "v" }, is_free: true, type: "VT_THROUGH", position: { x_nm: 0, y_nm: 0 } }] }],
  [`${cc}.CreateItems`, { items: [{ "@type": `${ct}.Text`, text: "hello", attributes: { line_spacing: 1.25, bold: true, visible: false, angle: { value_degrees: 90.5 }, size: { x_nm: 1000000, y_nm: 1000000 } } }], container: { value: "c" } }],
  [`${ct}.TextAttributes`, { line_spacing: 0 }], [`${ct}.TextAttributes`, { line_spacing: "1.5", bold: 1 }], [`${ct}.TextAttributes`, { line_spacing: "abc" }], [`${ct}.TextAttributes`, { line_spacing: 1e21 }], [`${ct}.TextAttributes`, { line_spacing: 1e-7 }],
  [`${ct}.DocumentSpecifier`, { type: "DOCTYPE_SCHEMATIC", board_filename: "" }], [`${ct}.DocumentSpecifier`, { board_filename: "a", sheet_path: { path: [{ value: "1" }] } }],
  [`${ct}.Vector2`, { x_nm: "9223372036854775807", y_nm: "-9223372036854775808" }], [`${ct}.Vector2`, { x_nm: 0, y_nm: "0" }], [`${ct}.Vector2`, { x_nm: true }], [`${ct}.Vector2`, { x_nm: null }],
  ["kiapi.board.BoardStackupLayer", { thickness: { value_nm: "35000" }, layer: "BL_F_Cu", enabled: true, type: "BSLT_COPPER", material_name: "Cu" }],
  ["kiapi.board.BoardStackupLayer", { dielectric: {}, soldermask: {} }],
  ["kiapi.board.commands.BoardLayers", { layers: ["BL_F_Cu", "BL_B_Cu", 999] }],
];
// a map field and bytes from the real schema, found by walking it
const root = schema.root as any;
function walk(ns: any, out: any[]) { for (const n of ns.nestedArray ?? []) { if (n.fieldsArray) out.push(n); if (n.nestedArray) walk(n, out); } return out; }
for (const t of walk(root, [])) {
  const name = t.fullName.slice(1);
  for (const f of t.fieldsArray) {
    if (f.map && !f.resolvedType && which === "runtime" && f.type === "string") runtime.push([name, { [f.name]: { alpha: "x" } }], [name, { [f.name]: 5 }], [name, { [f.name]: {} }]);
    if (f.type === "bytes" && which === "runtime") runtime.push([name, { [f.name]: "AQID" }], [name, { [f.name]: "A" }], [name, { [f.name]: "-_-_" }], [name, { [f.name]: [1, 2, 300] }], [name, { [f.name]: 5 }]);
    if (f.type === "uint64" && which === "runtime") runtime.push([name, { [f.name]: "18446744073709551615" }], [name, { [f.name]: -1 }]);
    if (f.type === "double" && f.partOf === null && which === "runtime" && runtime.length < 90) runtime.push([name, { [f.name]: 0.1 }], [name, { [f.name]: 3 }]);
    if (f.type === "sint32" && which === "runtime") runtime.push([name, { [f.name]: -3 }]);
  }
}

function hex(b: Uint8Array) { return Buffer.from(b).toString("hex"); }
const out = [];
for (const [type, message] of which === "runtime" ? runtime : fixture) {
  const T = schema.type(type);
  if (!T) throw new Error("no type " + type);
  const c: Record<string, unknown> = { type, in: message };
  try {
    const m = T.fromObject((message ?? {}) as object);
    const err = T.verify(m);
    if (err) throw new Error(err);
    const b = T.encode(m).finish();
    c.hex = hex(b);
    c.out = T.toObject(T.decode(b), TO);
  } catch (e) {
    c.error = (e as Error).message;
  }
  out.push(c);
}
// bytes straight off the wire: fields that JSON above cannot produce
const direct: [string, string][] = which === "runtime"
  ? [[`${cc}.GetItemsResponse`, "1a0a0a06782f6e6f6e65" + "1200"], [`${cc}.GetItemsResponse`, "1a0d0a0b6e6f736c6173682e747970"], [`${cc}.GetItemsResponse`, "1003"], [`${cc}.GetItemsResponse`, "10e807"], [`${ct}.TextAttributes`, "290000000000000000"], [`${ct}.TextAttributes`, "29000000000000f07f"], [`${ct}.TextAttributes`, "29000000000000f0ff"], [`${ct}.TextAttributes`, "29000000000000f87f"], [`${ct}.Vector2`, "08ffffffffffffffffff01"], [`${ct}.Vector2`, "f8ffff0f01"]]
  : [[`${bc}.GetItemsResponse`, "0a0a0a06782f6e6f6e651200"], [`${cc}.GetVersionResponse`, "0a00"]];
for (const [type, h] of direct) {
  const T = schema.type(type)!;
  out.push({ type, hex: h, out: T.toObject(T.decode(Buffer.from(h, "hex")), TO) });
}
writeFileSync(outFile, JSON.stringify(out, null, 0).replace(/\},\{"type"/g, '},\n{"type"') + "\n");
console.log(which, out.length, "cases", out.filter((c: any) => c.error).length, "errors");
