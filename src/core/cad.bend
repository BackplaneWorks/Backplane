import Base

# The CAD integration layer (pure data, no IO). A Tool says which files are
# its own and what they are (board, schematic, project, symbols, footprint),
# whether Backplane can draw them, and the commands that check and export
# them as argv templates. The server runs the templates, so it holds no
# per-tool logic; a new tool is one more record (see docs/design-review.md).
#
# Templates hold placeholders in braces: {cli} (the tool's command line
# program), {pcb}, {sch}, {out} (an output folder), {name} (the file stem).
# Cad.fill replaces only the names it is given, so other braces (KiCad's
# "${QUANTITY}" field) pass through. A tool Backplane cannot run has no
# checks and no outputs and draw False: the UI says "not supported yet".

# a file extension (lower case, no dot) and what such a file is
type Ext is Data:
  Ext{ext: String, role: String}

# a check: an id, a label, its argv template, the report file it writes
# under {out}, and the parser that reads it
type Chk is Data:
  Chk{id: String, label: String, argv: List<&2, String>, report: String, parser: String}

# a release output: an id, a label, its argv template, on by default
type Out is Data:
  Out{id: String, label: String, argv: List<&2, String>, on: Bool}

type CadTool is Data:
  CadTool{name: String, exts: List<&2, Ext>, draw: Bool, checks: List<&2, Chk>, outs: List<&2, Out>}

# what a file is: its tool's name and its role, "" for both when no tool
# knows it
type Hit is Data:
  Hit{tool: String, role: String, draw: Bool}

# a placeholder name (no braces) and its value
type Var is Data:
  Var{key: String, val: String}

# KiCad
# -----

def Cad.kicad.exts() -> List<&2, Ext>:
  [Ext{"kicad_pcb", "board"}, Ext{"kicad_sch", "schematic"}, Ext{"kicad_pro", "project"}, Ext{"kicad_sym", "symbols"}, Ext{"kicad_mod", "footprint"}]

# kicad-cli 10: "sch erc" has no parity flag; "pcb drc" has --schematic-parity.
# Both write JSON (--format json); units are millimetres in the report
def Cad.kicad.checks() -> List<&2, Chk>:
  [
    Chk{"erc", "ERC", ["{cli}", "sch", "erc", "--format", "json", "--severity-all", "--units", "mm", "-o", "{out}/erc.json", "{sch}"], "erc.json", "kicad-erc"},
    Chk{"drc", "DRC", ["{cli}", "pcb", "drc", "--format", "json", "--severity-all", "--units", "mm", "--schematic-parity", "-o", "{out}/drc.json", "{pcb}"], "drc.json", "kicad-drc"}
  ]

# Gerbers plot every enabled layer (a board's own plot settings,
# --board-plot-params, often leave out Edge.Cuts); drill files go in the same
# folder, PTH and NPTH apart, as fabs expect
def Cad.kicad.outs() -> List<&2, Out>:
  [
    Out{"gerbers", "Gerbers", ["{cli}", "pcb", "export", "gerbers", "-o", "{out}/gerbers/", "{pcb}"], True{}},
    Out{"drill", "Drill files", ["{cli}", "pcb", "export", "drill", "--format", "excellon", "--excellon-separate-th", "-o", "{out}/gerbers/", "{pcb}"], True{}},
    Out{"cpl", "Placement (CPL)", ["{cli}", "pcb", "export", "pos", "--format", "csv", "--units", "mm", "--side", "both", "--exclude-dnp", "-o", "{out}/{name}-cpl.csv", "{pcb}"], True{}},
    Out{"bom", "BOM", ["{cli}", "sch", "export", "bom", "--fields", "Reference,Value,Footprint,${QUANTITY},MPN,Manufacturer,${DNP}", "--labels", "Refs,Value,Footprint,Qty,MPN,Manufacturer,DNP", "--group-by", "Value,Footprint,MPN,Manufacturer", "--exclude-dnp", "-o", "{out}/{name}-bom.csv", "{sch}"], True{}},
    Out{"sch-pdf", "Schematic PDF", ["{cli}", "sch", "export", "pdf", "-o", "{out}/{name}-schematic.pdf", "{sch}"], True{}},
    Out{"pcb-pdf", "Board PDF", ["{cli}", "pcb", "export", "pdf", "--mode-multipage", "-l", "F.Cu,B.Cu,F.Silkscreen,B.Silkscreen,F.Mask,B.Mask,Edge.Cuts", "--cl", "Edge.Cuts", "-o", "{out}/{name}-board.pdf", "{pcb}"], True{}},
    Out{"step", "STEP model", ["{cli}", "pcb", "export", "step", "--subst-models", "-f", "-o", "{out}/{name}.step", "{pcb}"], True{}},
    Out{"ipc2581", "IPC-2581", ["{cli}", "pcb", "export", "ipc2581", "-o", "{out}/{name}-ipc2581.xml", "{pcb}"], False{}},
    Out{"odb", "ODB++", ["{cli}", "pcb", "export", "odb", "-o", "{out}/{name}-odb.zip", "{pcb}"], False{}}
  ]

def Cad.kicad() -> CadTool:
  CadTool{"kicad", Cad.kicad.exts(), True{}, Cad.kicad.checks(), Cad.kicad.outs()}

# Others (listed so the UI can name them; nothing runs)
# -----------------------------------------------------

def Cad.listed(name: String, exts: List<&2, Ext>) -> CadTool:
  CadTool{name, exts, False{}, Nil{}, Nil{}}

def Cad.altium() -> CadTool:
  Cad.listed("altium", [Ext{"pcbdoc", "board"}, Ext{"schdoc", "schematic"}, Ext{"prjpcb", "project"}])

def Cad.orcad() -> CadTool:
  Cad.listed("orcad", [Ext{"dsn", "schematic"}, Ext{"opj", "project"}])

def Cad.allegro() -> CadTool:
  Cad.listed("allegro", [Ext{"brd", "board"}])

# Eagle's .brd and .sch are ambiguous with Allegro's .brd: a file by its
# extension alone is Allegro's .brd (listed first); Eagle is told apart by
# content later
def Cad.eagle() -> CadTool:
  Cad.listed("eagle", [Ext{"brd", "board"}, Ext{"sch", "schematic"}])

# every tool, KiCad first
def Cad.tools() -> List<&2, CadTool>:
  [Cad.kicad(), Cad.altium(), Cad.orcad(), Cad.allegro(), Cad.eagle()]

# The tool and role of a file
# ---------------------------

# the text after the last '.' of the file name, lower case ("" when none)
def Cad.ext.go(ps: List<&2, String>, last: String) -> String:
  match ps:
    case Nil{}:
      last
    case +p <> t:
      Cad.ext.go(t, p)

def Cad.ext.k(dotted: Bool, +s: String) -> String:
  match dotted:
    case True{}:
      String.to_lower(Cad.ext.go(String.split(s, '.'), ""))
    case False{}:
      ""

def Cad.ext(+path: String) -> String:
  Cad.ext.k(String.contains(path, "."), path)

def Cad.role.pick(hit: Bool, r: String, rest: String) -> String:
  match hit:
    case True{}:
      r
    case False{}:
      rest

def Cad.role.go(xs: List<&2, Ext>, +e: String) -> String:
  match xs:
    case Nil{}:
      ""
    case Ext{+x, +r} <> t:
      Cad.role.pick(String.eq(x, e), r, Cad.role.go(t, e))

def Cad.none() -> Hit:
  Hit{"", "", False{}}

def Cad.of.pick.k(none: Bool, +r: String, +n: String, d: Bool, rest: Hit) -> Hit:
  match none:
    case True{}:
      rest
    case False{}:
      Hit{n, r, d}

def Cad.of.pick(+r: String, +n: String, d: Bool, rest: Hit) -> Hit:
  Cad.of.pick.k(String.is_empty(r), r, n, d, rest)

def Cad.of.tool(ts: List<&2, CadTool>, +e: String) -> Hit:
  match ts:
    case Nil{}:
      Cad.none()
    case CadTool{+n, +xs, +d, _, _} <> t:
      Cad.of.pick(Cad.role.go(xs, e), n, d, Cad.of.tool(t, e))

# which tool and role a file is, by its extension (case ignored)
def Cad.of(+path: String) -> Hit:
  Cad.of.tool(Cad.tools(), Cad.ext(path))

def Cad.tool.name(t: CadTool) -> String:
  match t:
    case CadTool{n, _, _, _, _}:
      n

def Cad.find.pick(hit: Bool, +t: CadTool, rest: Maybe<&2, CadTool>) -> Maybe<&2, CadTool>:
  match hit:
    case True{}:
      Some{t}
    case False{}:
      rest

# the tool by name
def Cad.find(ts: List<&2, CadTool>, +name: String) -> Maybe<&2, CadTool>:
  match ts:
    case Nil{}:
      None{}
    case +t <> rest:
      Cad.find.pick(String.eq(Cad.tool.name(t), name), t, Cad.find(rest, name))

# Templates
# ---------

# one piece of a string split at '{': when it starts with "key}" the value
# and the rest, else the brace back
def Cad.seg.k(hit: Bool, seg: String, +key: String, +val: String) -> String:
  match hit:
    case True{}:
      val ++ String.drop(seg, Nat.add(String.length(key), 1n))
    case False{}:
      "{" ++ seg

def Cad.seg(+seg: String, +key: String, +val: String) -> String:
  Cad.seg.k(String.starts_with(seg, key ++ "}"), seg, key, val)

def Cad.segs(xs: List<&2, String>, +key: String, +val: String) -> String:
  match xs:
    case Nil{}:
      ""
    case +h <> t:
      Cad.seg(h, key, val) ++ Cad.segs(t, key, val)

def Cad.sub.k(ps: List<&2, String>, +key: String, +val: String) -> String:
  match ps:
    case Nil{}:
      ""
    case +h <> t:
      h ++ Cad.segs(t, key, val)

# every "{key}" in s replaced by val
def Cad.sub(+s: String, +key: String, +val: String) -> String:
  Cad.sub.k(String.split(s, '{'), key, val)

def Cad.arg(vars: List<&2, Var>, +s: String) -> String:
  match vars:
    case Nil{}:
      s
    case Var{+k, +v} <> t:
      Cad.arg(t, Cad.sub(s, k, v))

# an argv template with the variables put in
def Cad.fill(argv: List<&2, String>, +vars: List<&2, Var>) -> List<&2, String>:
  match argv:
    case Nil{}:
      Nil{}
    case +a <> t:
      Cad.arg(vars, a) <> Cad.fill(t, vars)

# Shell quoting
# -------------

# one word in single quotes, each ' written '\''
def Cad.quote(+s: String) -> String:
  "'" ++ String.join(String.split(s, '\''), "'\\''") ++ "'"

def Cad.shell.go(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +a <> t:
      Cad.quote(a) <> Cad.shell.go(t)

# an argv as one line for sh
def Cad.shell(xs: List<&2, String>) -> String:
  String.join(Cad.shell.go(xs), " ")
