import Base
import ./json.bend as J

# Design checks (pure): kicad-cli's ERC and DRC JSON reports read into
# findings, a markdown checklist ("- [ ] text") that can be ticked in place,
# the custom checks and gate in .backplane/checks.json, the markdown report
# and the gate. No IO here; the server runs the commands (core/cad.bend) and
# hands the texts in.
#
# Report shapes (kicad-cli 10, --format json, coordinates in the --units, mm):
#   ERC  {"sheets":[{"path":"/Sub/","violations":[V...]}...], ...}
#   DRC  {"violations":[V...], "unconnected_items":[V...], "schematic_parity":[V...], ...}
#   V    {"type":"...", "severity":"error|warning|exclusion", "description":"...",
#         "items":[{"description":"...", "pos":{"x":..,"y":..}, "uuid":"..."}]}
# A DRC item does not name its layer except in its description ("Pad 1 [GND]
# of C21 on B.Cu"); the finding's place is the layer read from there.

# One finding. source: "erc", "drc", "bom", "checklist" or "custom".
# severity: "error", "warning", "info" or "exclusion". kind is the report's
# type. place is the sheet path (ERC) or the layer (DRC), "" when unknown.
# (x, y) is the first item's position in mm when has_pos.
type Finding is Data:
  Finding{source: String, severity: String, kind: String, text: String, place: String, x: F32, y: F32, has_pos: Bool}

# the first item's position
type Spot is Data:
  Spot{x: F32, y: F32, has: Bool}

def Fd.num(+o: J.Json, +key: String) -> F32:
  Maybe.default(&2, F32, F32.read(J.Json.as_raw(J.Json.get(o, key), "")), 0.0)

def Fd.none() -> Spot:
  Spot{0.0, 0.0, False{}}

def Fd.spot.of(m: Maybe<&2, J.Json>) -> Spot:
  match m:
    case Some{+p}:
      Spot{Fd.num(p, "x"), Fd.num(p, "y"), True{}}
    case None{}:
      Fd.none()

def Fd.spot(its: List<&2, J.Json>) -> Spot:
  match its:
    case Nil{}:
      Fd.none()
    case i <> _:
      Fd.spot.of(J.Json.get(i, "pos"))

def Fd.descs(its: List<&2, J.Json>) -> List<&2, String>:
  match its:
    case Nil{}:
      Nil{}
    case i <> t:
      J.Json.get_str(i, "description", "") <> Fd.descs(t)

def Fd.text.k(+desc: String, +ds: List<&2, String>) -> String:
  match ds:
    case Nil{}:
      desc
    case _:
      desc ++ ": " ++ String.join(ds, "; ")

# the description, then the items' descriptions
def Fd.text(+desc: String, its: List<&2, J.Json>) -> String:
  Fd.text.k(desc, Fd.descs(its))

def Fd.head(xs: List<&2, String>) -> String:
  match xs:
    case h <> _:
      h
    case Nil{}:
      ""

def Fd.layer.pick(on: Bool, +w: String, +cur: String) -> String:
  match on:
    case True{}:
      Fd.head(String.split(w, ','))
    case False{}:
      cur

# the word after the last "on" ("Pad 1 [GND] of C21 on B.Cu" -> "B.Cu")
def Fd.layer.w(ws: List<&2, String>, prev_on: Bool, cur: String) -> String:
  match ws:
    case Nil{}:
      cur
    case +w <> t:
      Fd.layer.w(t, String.eq(w, "on"), Fd.layer.pick(prev_on, w, cur))

def Fd.layers.pick.k(none: Bool, +l: String, rest: String) -> String:
  match none:
    case True{}:
      rest
    case False{}:
      l

def Fd.layers.pick(+l: String, rest: String) -> String:
  Fd.layers.pick.k(String.is_empty(l), l, rest)

# the layer the first item that names one is on
def Fd.layers(its: List<&2, J.Json>) -> String:
  match its:
    case Nil{}:
      ""
    case +i <> t:
      Fd.layers.pick(Fd.layer.w(String.split(J.Json.get_str(i, "description", ""), ' '), False{}, ""), Fd.layers(t))

def Fd.fin(+source: String, +sev: String, +kind: String, +text: String, +place: String, s: Spot) -> Finding:
  match s:
    case Spot{x, y, h}:
      Finding{source, sev, kind, text, place, x, y, h}

def Fd.place.k(none: Bool, +place: String, layer: String) -> String:
  match none:
    case True{}:
      layer
    case False{}:
      place

def Fd.place(+place: String, +its: List<&2, J.Json>) -> String:
  Fd.place.k(String.is_empty(place), place, Fd.layers(its))

def Fd.make.k(+source: String, +place: String, sev: String, kind: String, +desc: String, +its: List<&2, J.Json>) -> Finding:
  Fd.fin(source, sev, kind, Fd.text(desc, its), Fd.place(place, its), Fd.spot(its))

# one violation; an empty place is read from the items (a DRC layer)
def Fd.make(+source: String, +place: String, +v: J.Json) -> Finding:
  Fd.make.k(source, place, J.Json.get_str(v, "severity", "warning"), J.Json.get_str(v, "type", ""), J.Json.get_str(v, "description", ""), J.Json.elems(J.Json.get_obj(v, "items")))

# a pass over a report's violations: tail calls, newest first
def Fd.walk(vs: List<&2, J.Json>, +source: String, +place: String, acc: List<&2, Finding>) -> List<&2, Finding>:
  match vs:
    case Nil{}:
      acc
    case +v <> t:
      Fd.walk(t, source, place, Fd.make(source, place, v) <> acc)

def Fd.sheets(ss: List<&2, J.Json>, acc: List<&2, Finding>) -> List<&2, Finding>:
  match ss:
    case Nil{}:
      acc
    case +s <> t:
      Fd.sheets(t, Fd.walk(J.Json.elems(J.Json.get_obj(s, "violations")), "erc", J.Json.get_str(s, "path", "/"), acc))

def Fd.erc.k(m: Maybe<&2, J.Json>) -> List<&2, Finding>:
  match m:
    case Some{+j}:
      List.reverse(&2, Finding, Fd.sheets(J.Json.elems(J.Json.get_obj(j, "sheets")), Nil{}))
    case None{}:
      Nil{}

# the findings of an ERC report (kicad-cli sch erc --format json), in report
# order; none when the text is not JSON
def Checks.erc(+text: String) -> List<&2, Finding>:
  Fd.erc.k(J.Json.parse(text))

def Fd.drc.k(m: Maybe<&2, J.Json>) -> List<&2, Finding>:
  match m:
    case Some{+j}:
      List.reverse(&2, Finding, Fd.walk(J.Json.elems(J.Json.get_obj(j, "schematic_parity")), "drc", "", Fd.walk(J.Json.elems(J.Json.get_obj(j, "unconnected_items")), "drc", "", Fd.walk(J.Json.elems(J.Json.get_obj(j, "violations")), "drc", "", Nil{}))))
    case None{}:
      Nil{}

# the findings of a DRC report: violations, then unconnected items, then
# schematic parity issues
def Checks.drc(+text: String) -> List<&2, Finding>:
  Fd.drc.k(J.Json.parse(text))

def Checks.parses.k(m: Maybe<&2, J.Json>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

# whether a report text is JSON at all
def Checks.parses(+text: String) -> Bool:
  Checks.parses.k(J.Json.parse(text))

# a finding with no position
def Checks.finding(source: String, severity: String, kind: String, text: String, place: String) -> Finding:
  Finding{source, severity, kind, text, place, 0.0, 0.0, False{}}

# Report validation is separate from design findings: a failed tool or a
# malformed report can never be interpreted as a clean design.
def Report.string(v: Maybe<&2, J.Json>) -> Bool:
  match v:
    case Some{J.Str{_}}:
      True{}
    case _:
      False{}

def Report.array(v: Maybe<&2, J.Json>) -> Bool:
  match v:
    case Some{J.Arr{_}}:
      True{}
    case _:
      False{}

def Report.violation(+v: J.Json) -> Bool:
  +sev = J.Json.get_str(v, "severity", "")
  Bool.and(Bool.or(String.eq(sev, "error"), Bool.or(String.eq(sev, "warning"), String.eq(sev, "exclusion"))), Bool.and(Report.string(J.Json.get(v, "type")), Bool.and(Report.string(J.Json.get(v, "description")), Report.array(J.Json.get(v, "items")))))

def Report.violations(vs: List<&2, J.Json>, valid: Bool) -> Bool:
  match vs valid:
    case _ False{}:
      False{}
    case Nil{} True{}:
      True{}
    case +v <> t True{}:
      Report.violations(t, Report.violation(v))

def Report.list(+o: J.Json, +key: String) -> Bool:
  Report.violations(J.Json.elems(J.Json.get_obj(o, key)), Report.array(J.Json.get(o, key)))

def Report.sheets(ss: List<&2, J.Json>, valid: Bool) -> Bool:
  match ss valid:
    case _ False{}:
      False{}
    case Nil{} True{}:
      True{}
    case +s <> t True{}:
      Report.sheets(t, Bool.and(Report.string(J.Json.get(s, "path")), Report.list(s, "violations")))

def Report.shape(+id: String, +o: J.Json) -> Bool:
  match id:
    case "erc":
      Report.sheets(J.Json.elems(J.Json.get_obj(o, "sheets")), Report.array(J.Json.get(o, "sheets")))
    case "drc":
      Bool.and(Report.list(o, "violations"), Bool.and(Report.list(o, "unconnected_items"), Report.list(o, "schematic_parity")))
    case _:
      False{}

def Report.parsed(m: Maybe<&2, J.Json>, +id: String) -> Bool:
  match m:
    case Some{o}:
      Report.shape(id, o)
    case None{}:
      False{}

def Checks.report_valid(+id: String, +text: String) -> Bool:
  Report.parsed(J.Json.parse(text), id)

# Execution failures retain the failing check's identity, distinct from ERC
# and DRC design findings and independent of their configured thresholds.
def Checks.execution(fs: List<&2, Finding>) -> String:
  match fs:
    case Nil{}:
      ""
    case Finding{"execution", _, +kind, +text, _, _, _, _} <> t:
      kind ++ ": " ++ text ++ "\n" ++ Checks.execution(t)
    case _ <> t:
      Checks.execution(t)

# Counting
# --------

def Ct.add(hit: Bool, n: Nat) -> Nat:
  match hit:
    case True{}:
      1n+n
    case False{}:
      n

# how many findings of a source have a severity
def Ct.n(fs: List<&2, Finding>, +source: String, +sev: String, acc: Nat) -> Nat:
  match fs:
    case Nil{}:
      acc
    case Finding{+s, +v, _, _, _, _, _, _} <> t:
      Ct.n(t, source, sev, Ct.add(Bool.and(String.eq(s, source), String.eq(v, sev)), acc))

def Checks.count(+fs: List<&2, Finding>, +source: String, +sev: String) -> Nat:
  Ct.n(fs, source, sev, 0n)

type Tally is Data:
  Tally{source: String, errors: Nat, warnings: Nat, infos: Nat, exclusions: Nat}

def Checks.sources() -> List<&2, String>:
  ["erc", "drc", "bom", "checklist", "custom"]

def Checks.tally(+fs: List<&2, Finding>, +source: String) -> Tally:
  Tally{source, Checks.count(fs, source, "error"), Checks.count(fs, source, "warning"), Checks.count(fs, source, "info"), Checks.count(fs, source, "exclusion")}

def Ct.tallies(ss: List<&2, String>, +fs: List<&2, Finding>) -> List<&2, Tally>:
  match ss:
    case Nil{}:
      Nil{}
    case +s <> t:
      Checks.tally(fs, s) <> Ct.tallies(t, fs)

# counts per source (erc, drc, bom, checklist, custom) and severity
def Checks.summary(+fs: List<&2, Finding>) -> List<&2, Tally>:
  Ct.tallies(Checks.sources(), fs)

def Ct.total(t: Tally) -> Nat:
  match t:
    case Tally{_, e, w, i, x}:
      Nat.add(Nat.add(e, w), Nat.add(i, x))

# "1 error", "2 errors"
def Ct.plural(+n: Nat, +word: String) -> String:
  Nat.show(n) ++ " " ++ word ++ Bool.pick(String, Nat.is_eq(n, 1n), "", "s")

def Ct.part.k(none: Bool, +n: Nat, +word: String, rest: List<&2, String>) -> List<&2, String>:
  match none:
    case True{}:
      rest
    case False{}:
      Ct.plural(n, word) <> rest

def Ct.part(+n: Nat, +word: String, rest: List<&2, String>) -> List<&2, String>:
  Ct.part.k(Nat.is_eq(n, 0n), n, word, rest)

def Ct.line.k(+parts: List<&2, String>) -> String:
  match parts:
    case Nil{}:
      "No findings"
    case _:
      String.join(parts, "; ")

# Checklist
# ---------

# one box of a markdown task list. group is the heading above it.
type Task is Data:
  Task{text: String, done: Bool, group: String}

# 0 not a task line, 1 an open box, 2 a ticked one ("- [ ]", "* [x]")
def Cl.box.k(+t: String) -> Nat:
  Bool.pick(Nat, Bool.or(String.starts_with(t, "- [ ]"), String.starts_with(t, "* [ ]")), 1n, Bool.pick(Nat, Bool.or(Bool.or(String.starts_with(t, "- [x]"), String.starts_with(t, "- [X]")), Bool.or(String.starts_with(t, "* [x]"), String.starts_with(t, "* [X]"))), 2n, 0n))

def Cl.box(+line: String) -> Nat:
  Cl.box.k(String.trim_start(line))

# the box's mark flipped: the text after the first '[' starts with the mark
def Cl.mark(r: String) -> String:
  match r:
    case SCon{Chr{+c}, t}:
      Bool.pick(String, U32.is_eq(c, 32), "x", " ") ++ t
    case SNil{}:
      ""

def Cl.flip.k(+ps: List<&2, String>) -> List<&2, String>:
  match ps:
    case p <> r <> more:
      p <> Cl.mark(r) <> more
    case _:
      ps

# a task line with its box flipped, every other byte as it was
def Cl.flip(+line: String) -> String:
  String.join(Cl.flip.k(String.split(line, '[')), "[")

# walking the lines: tasks still to pass before the target, whether it was
# flipped, the lines so far (newest first)
type Tg is Data:
  Tg{n: Nat, done: Bool, acc: List<&2, String>}

def Cl.step.n(n: Nat, +l: String, acc: List<&2, String>) -> Tg:
  match n:
    case 0n:
      Tg{0n, True{}, Cl.flip(l) <> acc}
    case 1n+p:
      Tg{p, False{}, l <> acc}

def Cl.step.t(d: Bool, n: Nat, +l: String, acc: List<&2, String>) -> Tg:
  match d:
    case True{}:
      Tg{n, True{}, l <> acc}
    case False{}:
      Cl.step.n(n, l, acc)

def Cl.step.k(b: Nat, +l: String, n: Nat, d: Bool, acc: List<&2, String>) -> Tg:
  match b:
    case 0n:
      Tg{n, d, l <> acc}
    case 1n+_:
      Cl.step.t(d, n, l, acc)

def Cl.step(+l: String, st: Tg) -> Tg:
  match st:
    case Tg{n, d, acc}:
      Cl.step.k(Cl.box(l), l, n, d, acc)

def Cl.tog(ls: List<&2, String>, st: Tg) -> Tg:
  match ls:
    case Nil{}:
      st
    case +l <> t:
      Cl.tog(t, Cl.step(l, st))

def Cl.tog.fin(st: Tg) -> List<&2, String>:
  match st:
    case Tg{_, _, acc}:
      List.reverse(&2, String, acc)

# the file text with the index-th box (counting boxes from 0) flipped,
# everything else byte for byte; an index past the last box changes nothing
def Checklist.toggle(+text: String, index: Nat) -> String:
  String.join(Cl.tog.fin(Cl.tog(String.split(text, '\n'), Tg{index, False{}, Nil{}})), "\n")

def Cl.add(hit: Bool, n: Nat) -> Nat:
  Ct.add(hit, n)

# the number of leading '#'
def Cl.hashes(s: String, +run: Bool, acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Cl.hashes(t, Bool.and(run, U32.is_eq(c, 35)), Cl.add(Bool.and(run, U32.is_eq(c, 35)), acc))

type Cl is Data:
  Cl{group: String, acc: List<&2, Task>}

def Cl.task(+l: String, b: Nat, +g: String) -> Task:
  Task{String.trim(String.drop(String.trim_start(l), 5n)), Nat.is_eq(b, 2n), g}

def Cl.head.k(+t: String, st: Cl) -> Cl:
  match st:
    case Cl{_, acc}:
      Cl{String.trim(String.drop(t, Cl.hashes(t, True{}, 0n))), acc}

def Cl.head(isHead: Bool, +l: String, st: Cl) -> Cl:
  match isHead:
    case True{}:
      Cl.head.k(String.trim_start(l), st)
    case False{}:
      st

def Cl.line.t(b: Nat, +l: String, st: Cl) -> Cl:
  match st:
    case Cl{+g, acc}:
      Cl{g, Cl.task(l, b, g) <> acc}

def Cl.line.k(+b: Nat, +l: String, st: Cl) -> Cl:
  match b:
    case 0n:
      Cl.head(String.starts_with(String.trim_start(l), "#"), l, st)
    case 1n+_:
      Cl.line.t(b, l, st)

def Cl.line(+l: String, st: Cl) -> Cl:
  Cl.line.k(Cl.box(l), l, st)

def Cl.parse(ls: List<&2, String>, st: Cl) -> Cl:
  match ls:
    case Nil{}:
      st
    case +l <> t:
      Cl.parse(t, Cl.line(l, st))

def Cl.parse.fin(st: Cl) -> List<&2, Task>:
  match st:
    case Cl{_, acc}:
      List.reverse(&2, Task, acc)

# the boxes of a checklist file, in order, each with its heading
def Checklist.parse(+text: String) -> List<&2, Task>:
  Cl.parse.fin(Cl.parse(String.split(text, '\n'), Cl{"", Nil{}}))

def Cl.open(ts: List<&2, Task>, acc: Nat) -> Nat:
  match ts:
    case Nil{}:
      acc
    case Task{_, d, _} <> t:
      Cl.open(t, Ct.add(Bool.not(d), acc))

# boxes not ticked
def Checklist.open(ts: List<&2, Task>) -> Nat:
  Cl.open(ts, 0n)

# a sensible review list for a hardware design
def Checklist.default() -> String:
  "# Power\n- [ ] Every rail has a source, a load budget and an enable\n- [ ] Decoupling on every IC power pin, placed close\n- [ ] Input protection: reverse polarity, overvoltage, inrush\n# Connectors\n- [ ] Pinouts match the mating part, pin 1 marked\n- [ ] ESD protection on exposed signals\n# Layout\n- [ ] Mounting holes and board outline checked against the enclosure\n- [ ] Clearances, creepage and current capacity of power tracks\n- [ ] Test points on key nets\n# Fabrication\n- [ ] Fab notes: stackup, finish, thickness, impedance\n- [ ] Silkscreen: polarity marks, reference designators legible\n- [ ] Board and schematic revision updated\n# Assembly\n- [ ] BOM complete: every part has an MPN and a footprint\n- [ ] DNP parts marked\n# Checks\n- [ ] ERC clean\n- [ ] DRC clean\n"

# Custom checks and gate
# ----------------------

# a shell command to run, and when: any of "pr", "release", "manual"
type Custom is Data:
  Custom{name: String, run: String, on: List<&2, String>}

def Ct.label.k(erc: Bool, drc: Bool, bom: Bool, cl: Bool, +s: String) -> String:
  Bool.pick(String, erc, "ERC", Bool.pick(String, drc, "DRC", Bool.pick(String, bom, "BOM", Bool.pick(String, cl, "Checklist", Bool.pick(String, String.eq(s, "custom"), "Custom", s)))))

def Ct.label(+s: String) -> String:
  Ct.label.k(String.eq(s, "erc"), String.eq(s, "drc"), String.eq(s, "bom"), String.eq(s, "checklist"), s)

def Ct.words(t: Tally) -> String:
  match t:
    case Tally{+s, e, w, i, x}:
      Ct.label(s) ++ " " ++ String.join(Ct.part(e, "error", Ct.part(w, "warning", Ct.part(i, "note", Ct.part(x, "excluded", Nil{})))), ", ")

def Ct.line.keep(none: Bool, t: Tally, rest: List<&2, String>) -> List<&2, String>:
  match none:
    case True{}:
      rest
    case False{}:
      Ct.words(t) <> rest

def Ct.line.go(ts: List<&2, Tally>) -> List<&2, String>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> r:
      Ct.line.keep(Nat.is_eq(Ct.total(t), 0n), t, Ct.line.go(r))

# one line: "ERC 2 errors, 1 warning; DRC 3 errors", or "No findings"
def Checks.line(+fs: List<&2, Finding>) -> String:
  Ct.line.k(Ct.line.go(Checks.summary(fs)))

# how strict ERC and DRC gate a release: "error" (errors block), "warning"
# (warnings too), "off"; and whether unticked checklist boxes block
type Gate is Data:
  Gate{erc: String, drc: String, checklist: Bool}

type Config is Data:
  Config{checks: List<&2, Custom>, gate: Gate}

def Checks.gate.default() -> Gate:
  Gate{"error", "error", False{}}

def Checks.config.default() -> Config:
  Config{Nil{}, Checks.gate.default()}

def Cu.strs(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case J.Str{s} <> t:
      s <> Cu.strs(t)
    case _ <> t:
      Cu.strs(t)

def Cu.on.k(m: Maybe<&2, J.Json>) -> List<&2, String>:
  match m:
    case Some{a}:
      Cu.strs(J.Json.elems(a))
    case None{}:
      ["pr", "release", "manual"]

def Cu.on(+j: J.Json) -> List<&2, String>:
  Cu.on.k(J.Json.get(j, "on"))

def Cu.make(+run: String, +name: String, on: List<&2, String>) -> Custom:
  Custom{Bool.pick(String, String.is_empty(name), run, name), run, on}

def Cu.keep(none: Bool, c: Custom, rest: List<&2, Custom>) -> List<&2, Custom>:
  match none:
    case True{}:
      rest
    case False{}:
      c <> rest

def Cu.one(+c: Custom, rest: List<&2, Custom>) -> List<&2, Custom>:
  match c:
    case Custom{_, +r, _}:
      Cu.keep(String.is_empty(r), c, rest)

def Cu.list(xs: List<&2, J.Json>) -> List<&2, Custom>:
  match xs:
    case Nil{}:
      Nil{}
    case +j <> t:
      Cu.one(Cu.make(J.Json.get_str(j, "run", ""), J.Json.get_str(j, "name", ""), Cu.on(j)), Cu.list(t))

def Cu.gate(+g: J.Json) -> Gate:
  Gate{J.Json.get_str(g, "erc", "error"), J.Json.get_str(g, "drc", "error"), J.Json.get_bool(g, "checklist", False{})}

def Cu.config(+j: J.Json) -> Config:
  Config{Cu.list(J.Json.elems(J.Json.get_obj(j, "checks"))), Cu.gate(J.Json.get_obj(j, "gate"))}

def Cu.config.k(m: Maybe<&2, J.Json>) -> Config:
  match m:
    case Some{j}:
      Cu.config(j)
    case None{}:
      Checks.config.default()

# .backplane/checks.json; the defaults when missing, empty or malformed.
# A check without a command is dropped; with no name its command is its name
def Checks.config(+text: String) -> Config:
  Cu.config.k(J.Json.parse(text))

def Cu.has(xs: List<&2, String>, +w: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.or(String.eq(h, w), Cu.has(t, w))

# the checks to run for a trigger ("pr", "release", "manual")
def Checks.for(cs: List<&2, Custom>, +trigger: String) -> List<&2, Custom>:
  match cs:
    case Nil{}:
      Nil{}
    case Custom{n, r, +on} <> t:
      Cu.keep(Bool.not(Cu.has(on, trigger)), Custom{n, r, on}, Checks.for(t, trigger))

# What a custom check did: its exit success and its output
type Run is Data:
  Run{name: String, ok: Bool, out: String}

def Gt.bad.k(+e: Nat, w: Nat, +level: String) -> Nat:
  Bool.pick(Nat, String.eq(level, "error"), e, Bool.pick(Nat, String.eq(level, "warning"), Nat.add(e, w), 0n))

# Gate
# ----

def Gt.bad(+fs: List<&2, Finding>, +source: String, +level: String) -> Nat:
  Gt.bad.k(Checks.count(fs, source, "error"), Checks.count(fs, source, "warning"), level)

def Gt.what(+n: Nat, +level: String) -> String:
  Bool.pick(String, String.eq(level, "warning"), Nat.show(n) ++ " errors or warnings", Ct.plural(n, "error"))

def Gt.reason.k(none: Bool, +label: String, +n: Nat, +level: String, rest: List<&2, String>) -> List<&2, String>:
  match none:
    case True{}:
      rest
    case False{}:
      (label ++ ": " ++ Gt.what(n, level)) <> rest

def Gt.reason(+label: String, +n: Nat, +level: String, rest: List<&2, String>) -> List<&2, String>:
  Gt.reason.k(Nat.is_eq(n, 0n), label, n, level, rest)

def Gt.reasons(+fs: List<&2, Finding>, g: Gate) -> List<&2, String>:
  match g:
    case Gate{+e, +d, _}:
      Gt.reason("ERC", Gt.bad(fs, "erc", e), e, Gt.reason("DRC", Gt.bad(fs, "drc", d), d, Nil{}))

def Gt.join(rs: List<&2, String>) -> String:
  String.join(rs, "; ")

# "" when the findings pass the gate, else why not. Exclusions and notes never
# block; "off" turns a source off
def Checks.gate(+fs: List<&2, Finding>, g: Gate) -> String:
  Gt.join(Gt.reasons(fs, g))

def Checks.passes(+fs: List<&2, Finding>, g: Gate) -> Bool:
  String.is_empty(Checks.gate(fs, g))

def Gt.fails(rs: List<&2, Run>, acc: Nat) -> Nat:
  match rs:
    case Nil{}:
      acc
    case Run{_, ok, _} <> t:
      Gt.fails(t, Ct.add(Bool.not(ok), acc))

def Gt.extra.k(none: Bool, n: Nat, +label: String, +what: String, rest: List<&2, String>) -> List<&2, String>:
  match none:
    case True{}:
      rest
    case False{}:
      (label ++ ": " ++ Nat.show(n) ++ " " ++ what) <> rest

def Gt.extra(+n: Nat, +label: String, +what: String, rest: List<&2, String>) -> List<&2, String>:
  Gt.extra.k(Nat.is_eq(n, 0n), n, label, what, rest)

def Gt.open(g: Gate, tasks: List<&2, Task>) -> Nat:
  match g:
    case Gate{_, _, c}:
      Bool.pick(Nat, c, Checklist.open(tasks), 0n)

# the whole gate: findings, unticked boxes when the config says so, failed
# custom checks
def Checks.gate.all(+fs: List<&2, Finding>, tasks: List<&2, Task>, runs: List<&2, Run>, +g: Gate) -> String:
  Gt.join(List.append(&2, String, Gt.reasons(fs, g), Gt.extra(Gt.open(g, tasks), "Checklist", "unticked", Gt.extra(Gt.fails(runs, 0n), "Custom", "failed", Nil{}))))

# Report
# ------

# rows shown per source before "N more"
def Checks.cap() -> Nat:
  200n

# a table cell: no pipes, no line breaks
def Ct.esc(+s: String) -> String:
  String.join(String.split(String.join(String.split(String.join(String.split(s, '\n'), " "), '\r'), ""), '|'), "\\|")

def Ct.row(f: Finding) -> String:
  match f:
    case Finding{_, +sev, +kind, +text, +place, _, _, _}:
      "| " ++ Ct.esc(sev) ++ " | " ++ Ct.esc(kind) ++ " | " ++ Ct.esc(place) ++ " | " ++ Ct.esc(text) ++ " |\n"

def Ct.src(f: Finding) -> String:
  match f:
    case Finding{s, _, _, _, _, _, _, _}:
      s

# rows so far (newest first), how many more may be added, how many did not fit
type Rows is Data:
  Rows{acc: List<&2, String>, left: Nat, more: Nat}

def Ct.take.k(left: Nat, +row: String, acc: List<&2, String>, more: Nat) -> Rows:
  match left:
    case 0n:
      Rows{acc, 0n, 1n+more}
    case 1n+p:
      Rows{row <> acc, p, more}

def Ct.take(+row: String, st: Rows) -> Rows:
  match st:
    case Rows{acc, left, more}:
      Ct.take.k(left, row, acc, more)

def Ct.step(hit: Bool, +f: Finding, st: Rows) -> Rows:
  match hit:
    case True{}:
      Ct.take(Ct.row(f), st)
    case False{}:
      st

def Ct.rows(fs: List<&2, Finding>, +source: String, st: Rows) -> Rows:
  match fs:
    case Nil{}:
      st
    case +f <> t:
      Ct.rows(t, source, Ct.step(String.eq(Ct.src(f), source), f, st))

def Ct.more(+n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "", "\n_" ++ Nat.show(n) ++ " more not shown_\n")

def Ct.table.fin(st: Rows) -> String:
  match st:
    case Rows{acc, _, more}:
      String.concat(List.reverse(&2, String, acc)) ++ Ct.more(more)

def Ct.section.k(none: Bool, +source: String, n: Nat, +fs: List<&2, Finding>) -> String:
  match none:
    case True{}:
      ""
    case False{}:
      "\n## " ++ Ct.label(source) ++ " (" ++ Nat.show(n) ++ ")\n\n| Severity | Type | Where | Description |\n|---|---|---|---|\n" ++ Ct.table.fin(Ct.rows(fs, source, Rows{Nil{}, Checks.cap(), 0n}))

def Ct.count(t: Tally) -> Nat:
  Ct.total(t)

def Ct.section.m(+n: Nat, +s: String, +fs: List<&2, Finding>) -> String:
  Ct.section.k(Nat.is_eq(n, 0n), s, n, fs)

def Ct.section.n(t: Tally, +s: String, +fs: List<&2, Finding>) -> String:
  Ct.section.m(Ct.total(t), s, fs)

def Ct.sections(ss: List<&2, String>, +fs: List<&2, Finding>) -> String:
  match ss:
    case Nil{}:
      ""
    case +s <> t:
      Ct.section.n(Checks.tally(fs, s), s, fs) ++ Ct.sections(t, fs)

def Ct.head(same: Bool, +g: String) -> String:
  match same:
    case True{}:
      ""
    case False{}:
      "\n**" ++ g ++ "**\n\n"

def Ct.box(d: Bool) -> String:
  Bool.pick(String, d, "- [x] ", "- [ ] ")

# the checklist as ticked and open boxes under their headings
def Ct.tasks(ts: List<&2, Task>, +last: String) -> String:
  match ts:
    case Nil{}:
      ""
    case Task{+x, d, +g} <> t:
      Ct.head(String.eq(g, last), g) ++ Ct.box(d) ++ x ++ "\n" ++ Ct.tasks(t, g)

def Ct.done(ts: List<&2, Task>, acc: Nat) -> Nat:
  match ts:
    case Nil{}:
      acc
    case Task{_, d, _} <> t:
      Ct.done(t, Ct.add(d, acc))

def Ct.checklist.k(+ts: List<&2, Task>) -> String:
  match ts:
    case Nil{}:
      ""
    case _:
      "\n## Checklist (" ++ Nat.show(Ct.done(ts, 0n)) ++ "/" ++ Nat.show(List.length(&2, Task, ts)) ++ ")\n" ++ Ct.tasks(ts, "\0")

def Ct.out(+s: String) -> String:
  Ct.esc(String.take(String.trim(Fd.head(String.split(s, '\n'))), 120n))

def Ct.run(r: Run) -> String:
  match r:
    case Run{+n, ok, +o}:
      "| " ++ Ct.esc(n) ++ " | " ++ Bool.pick(String, ok, "pass", "FAIL") ++ " | " ++ Ct.out(o) ++ " |\n"

def Ct.runs(rs: List<&2, Run>) -> String:
  match rs:
    case Nil{}:
      ""
    case r <> t:
      Ct.run(r) ++ Ct.runs(t)

def Ct.runs.k(+rs: List<&2, Run>) -> String:
  match rs:
    case Nil{}:
      ""
    case _:
      "\n## Custom checks\n\n| Check | Result | Output |\n|---|---|---|\n" ++ Ct.runs(rs)

# the report: one summary line, a table per source with findings (200 rows at
# most, then "N more"), the checklist, the custom checks' results
def Checks.md(+fs: List<&2, Finding>, tasks: List<&2, Task>, runs: List<&2, Run>) -> String:
  "**Design checks:** " ++ Checks.line(fs) ++ "\n" ++ Ct.sections(Checks.sources(), fs) ++ Ct.checklist.k(tasks) ++ Ct.runs.k(runs)
