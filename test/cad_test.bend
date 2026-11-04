import Base
import ./check.bend as C
import ../src/core/cad.bend as D

def hit.s(h: D.Hit) -> String:
  match h:
    case D.Hit{t, r, d}:
      t ++ ":" ++ r ++ ":" ++ Bool.show(d)

def hit(+p: String) -> String:
  hit.s(D.Cad.of(p))

def argv(xs: List<&2, String>) -> String:
  String.join(xs, "|")

def vars() -> List<&2, D.Var>:
  [D.Var{"cli", "/usr/bin/kicad-cli"}, D.Var{"out", "/tmp/o"}, D.Var{"pcb", "my board.kicad_pcb"}, D.Var{"sch", "my.kicad_sch"}, D.Var{"name", "my"}]

def outs.pick(same: Bool, a: List<&2, String>, on: Bool, rest: String) -> String:
  match same:
    case True{}:
      Bool.show(on) ++ " " ++ argv(D.Cad.fill(a, vars()))
    case False{}:
      rest

def outs(os: List<&2, D.Out>, +id: String) -> String:
  match os:
    case Nil{}:
      "none"
    case D.Out{+i, _, +a, on} <> t:
      outs.pick(String.eq(i, id), a, on, outs(t, id))

def kicad.out.k(m: Maybe<&2, D.CadTool>, +id: String) -> String:
  match m:
    case Some{D.CadTool{_, _, _, _, os}}:
      outs(os, id)
    case None{}:
      "no kicad"

def kicad.out(+id: String) -> String:
  kicad.out.k(D.Cad.find(D.Cad.tools(), "kicad"), id)

def chks.pick(same: Bool, a: List<&2, String>, rp: String, rest: String) -> String:
  match same:
    case True{}:
      rp ++ " " ++ argv(D.Cad.fill(a, vars()))
    case False{}:
      rest

def chks(cs: List<&2, D.Chk>, +id: String) -> String:
  match cs:
    case Nil{}:
      "none"
    case D.Chk{+i, _, +a, +r, +p} <> t:
      chks.pick(String.eq(i, id), a, r ++ " " ++ p, chks(t, id))

def chk.k(m: Maybe<&2, D.CadTool>, +id: String) -> String:
  match m:
    case Some{D.CadTool{_, _, _, cs, _}}:
      chks(cs, id)
    case None{}:
      "no kicad"

def chk(+id: String) -> String:
  chk.k(D.Cad.find(D.Cad.tools(), "kicad"), id)

def nout(m: Maybe<&2, D.CadTool>) -> String:
  match m:
    case Some{D.CadTool{_, _, d, cs, os}}:
      Bool.show(d) ++ Nat.show(List.length(&2, D.Chk, cs)) ++ Nat.show(List.length(&2, D.Out, os))
    case None{}:
      "none"

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("kicad board", hit("a/b/board.kicad_pcb"), "kicad:board:True")
    C.Check.str("kicad schematic", hit("x.kicad_sch"), "kicad:schematic:True")
    C.Check.str("kicad project", hit("x.kicad_pro"), "kicad:project:True")
    C.Check.str("kicad symbols", hit("lib.kicad_sym"), "kicad:symbols:True")
    C.Check.str("kicad footprint", hit("R_0402.kicad_mod"), "kicad:footprint:True")
    C.Check.str("case ignored", hit("X.KiCad_PCB"), "kicad:board:True")
    C.Check.str("altium board", hit("/p/Main.PcbDoc"), "altium:board:False")
    C.Check.str("altium schematic", hit("Main.SchDoc"), "altium:schematic:False")
    C.Check.str("altium project", hit("Main.PrjPcb"), "altium:project:False")
    C.Check.str("orcad", hit("a.dsn") ++ " " ++ hit("a.opj"), "orcad:schematic:False orcad:project:False")
    C.Check.str("allegro board", hit("a.brd"), "allegro:board:False")
    C.Check.str("eagle schematic", hit("a.sch"), "eagle:schematic:False")
    C.Check.str("unknown", hit("readme.md"), "::False")
    C.Check.str("no extension", hit("Makefile"), "::False")
    C.Check.str("a dot in the folder", hit("a.d/board.kicad_pcb"), "kicad:board:True")
    C.Check.str("fill", argv(D.Cad.fill(["{cli}", "pcb", "drc", "-o", "{out}/drc.json", "{pcb}"], vars())), "/usr/bin/kicad-cli|pcb|drc|-o|/tmp/o/drc.json|my board.kicad_pcb")
    C.Check.str("fill leaves other braces", argv(D.Cad.fill(["${QUANTITY}", "{nope}", "{out"], vars())), "${QUANTITY}|{nope}|{out")
    C.Check.str("fill twice in one arg", argv(D.Cad.fill(["{out}:{out}"], vars())), "/tmp/o:/tmp/o")
    C.Check.str("shell", D.Cad.shell(["echo", "it's", "a b", ""]), "'echo' 'it'\\''s' 'a b' ''")
    C.Check.str("shell quote", D.Cad.quote("'"), "''\\'''")
    C.Check.str("erc", chk("erc"), "erc.json kicad-erc /usr/bin/kicad-cli|sch|erc|--format|json|--severity-all|--units|mm|-o|/tmp/o/erc.json|my.kicad_sch")
    C.Check.str("drc", chk("drc"), "drc.json kicad-drc /usr/bin/kicad-cli|pcb|drc|--format|json|--severity-all|--units|mm|--schematic-parity|-o|/tmp/o/drc.json|my board.kicad_pcb")
    C.Check.str("gerbers", kicad.out("gerbers"), "True /usr/bin/kicad-cli|pcb|export|gerbers|-o|/tmp/o/gerbers/|my board.kicad_pcb")
    C.Check.str("drill", kicad.out("drill"), "True /usr/bin/kicad-cli|pcb|export|drill|--format|excellon|--excellon-separate-th|-o|/tmp/o/gerbers/|my board.kicad_pcb")
    C.Check.str("cpl", kicad.out("cpl"), "True /usr/bin/kicad-cli|pcb|export|pos|--format|csv|--units|mm|--side|both|--exclude-dnp|-o|/tmp/o/my-cpl.csv|my board.kicad_pcb")
    C.Check.str("bom", kicad.out("bom"), "True /usr/bin/kicad-cli|sch|export|bom|--fields|Reference,Value,Footprint,${QUANTITY},MPN,Manufacturer,${DNP}|--labels|Refs,Value,Footprint,Qty,MPN,Manufacturer,DNP|--group-by|Value,Footprint,MPN,Manufacturer|--exclude-dnp|-o|/tmp/o/my-bom.csv|my.kicad_sch")
    C.Check.str("sch pdf", kicad.out("sch-pdf"), "True /usr/bin/kicad-cli|sch|export|pdf|-o|/tmp/o/my-schematic.pdf|my.kicad_sch")
    C.Check.str("pcb pdf", kicad.out("pcb-pdf"), "True /usr/bin/kicad-cli|pcb|export|pdf|--mode-multipage|-l|F.Cu,B.Cu,F.Silkscreen,B.Silkscreen,F.Mask,B.Mask,Edge.Cuts|--cl|Edge.Cuts|-o|/tmp/o/my-board.pdf|my board.kicad_pcb")
    C.Check.str("step", kicad.out("step"), "True /usr/bin/kicad-cli|pcb|export|step|--subst-models|-f|-o|/tmp/o/my.step|my board.kicad_pcb")
    C.Check.str("ipc2581 off", kicad.out("ipc2581"), "False /usr/bin/kicad-cli|pcb|export|ipc2581|-o|/tmp/o/my-ipc2581.xml|my board.kicad_pcb")
    C.Check.str("odb off", kicad.out("odb"), "False /usr/bin/kicad-cli|pcb|export|odb|-o|/tmp/o/my-odb.zip|my board.kicad_pcb")
    C.Check.str("unknown output", kicad.out("zzz"), "none")
    C.Check.str("kicad counts", nout(D.Cad.find(D.Cad.tools(), "kicad")), "True29")
    C.Check.str("altium can't run", nout(D.Cad.find(D.Cad.tools(), "altium")), "False00")
    C.Check.str("no such tool", nout(D.Cad.find(D.Cad.tools(), "zuken")), "none")
