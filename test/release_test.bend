import Base
import ./check.bend as C
import ../src/core/cad.bend as D
import ../src/core/release.bend as R

def b(+x: Bool) -> String:
  Bool.show(x)

def valids(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case +h <> t:
      b(R.Release.valid(h)) ++ " " ++ valids(t)

def argv(xs: List<&2, String>) -> String:
  String.join(xs, "|")

def steps(ss: List<&2, R.Step>) -> String:
  match ss:
    case Nil{}:
      ""
    case R.Step{i, _, a} <> t:
      i ++ ":" ++ argv(a) ++ "\n" ++ steps(t)

def plan.s(p: R.Plan) -> String:
  match p:
    case R.Plan{t, d, ss}:
      t ++ " " ++ d ++ "\n" ++ steps(ss)

def plan.k(m: Maybe<&2, D.Tool>, ids: List<&2, String>, +pcb: String, +sch: String) -> String:
  match m:
    case Some{+t}:
      plan.s(R.Release.plan(t, ids, "/r", "v1", "kicad-cli", pcb, sch))
    case None{}:
      "no tool"

def plan(ids: List<&2, String>, +pcb: String, +sch: String) -> String:
  plan.k(D.Cad.find(D.Cad.tools(), "kicad"), ids, pcb, sch)

def defaults.k(m: Maybe<&2, D.Tool>) -> String:
  match m:
    case Some{t}:
      argv(R.Release.defaults(t))
    case None{}:
      "no tool"

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("tags valid", valids(["v1.2", "rev-A", "release/2026", "r7", "1", "v1.2-rc.1", "a.b"]), "True True True True True True True ")
    C.Check.str("tags invalid", valids(["", "a b", "a..b", "-x", "x.lock", "a~b", "a^b", "a:b", "a?b", "a*b", "a[b", "a\\b", "@", "a@{b", "/x", "x/", "x.", "a//b", ".x", "a/.b", "a\tb", "a\nb"]), "False False False False False False False False False False False False False False False False False False False False False False ")
    C.Check.str("next: v1.2", R.Release.next(["v1.1", "v1.2"]), "v1.3")
    C.Check.str("next: order given doesn't matter", R.Release.next(["v1.2", "v1.1"]), "v1.3")
    C.Check.str("next: numbers by value", R.Release.next(["v1.9", "v1.10", "v1.2"]), "v1.11")
    C.Check.str("next: carry", R.Release.next(["v1.9"]), "v1.10")
    C.Check.str("next: rev-A", R.Release.next(["rev-A"]), "rev-B")
    C.Check.str("next: rev-Z", R.Release.next(["rev-Z"]), "rev-AA")
    C.Check.str("next: r7", R.Release.next(["r7"]), "r8")
    C.Check.str("next: padding kept", R.Release.next(["r009"]), "r010")
    C.Check.str("next: 99", R.Release.next(["99"]), "100")
    C.Check.str("next: none", R.Release.next(Nil{}), "v1")
    C.Check.str("next: not bumpable ones ignored", R.Release.next(["latest", "v2-rc", "v2"]), "v3")
    C.Check.str("next: only unbumpable", R.Release.next(["latest"]), "v1")
    C.Check.str("bump", R.Release.bump("v1.2.9"), "v1.2.10")
    C.Check.str("stem", R.Release.stem("/r/a/board.kicad_pcb") ++ " " ++ R.Release.stem("x") ++ " " ++ R.Release.stem("a.b.kicad_sch") ++ " " ++ R.Release.stem(""), "board x a.b ")
    C.Check.str("dir", R.Release.dir("/r", "v1"), "/r/.backplane/releases/v1")
    C.Check.str("defaults", defaults.k(D.Cad.find(D.Cad.tools(), "kicad")), "gerbers|drill|cpl|bom|sch-pdf|pcb-pdf|step")
    C.Check.str("plan", plan(["gerbers", "bom", "cpl"], "/r/a.kicad_pcb", "/r/a.kicad_sch"), "v1 /r/.backplane/releases/v1\ngerbers:kicad-cli|pcb|export|gerbers|-o|/r/.backplane/releases/v1/gerbers/|/r/a.kicad_pcb\ncpl:kicad-cli|pcb|export|pos|--format|csv|--units|mm|--side|both|--exclude-dnp|-o|/r/.backplane/releases/v1/a-cpl.csv|/r/a.kicad_pcb\nbom:kicad-cli|sch|export|bom|--fields|Reference,Value,Footprint,${QUANTITY},MPN,Manufacturer,${DNP}|--labels|Refs,Value,Footprint,Qty,MPN,Manufacturer,DNP|--group-by|Value,Footprint,MPN,Manufacturer|--exclude-dnp|-o|/r/.backplane/releases/v1/a-bom.csv|/r/a.kicad_sch\n")
    C.Check.str("plan without a schematic", plan(["gerbers", "bom", "sch-pdf", "step"], "/r/a.kicad_pcb", ""), "v1 /r/.backplane/releases/v1\ngerbers:kicad-cli|pcb|export|gerbers|-o|/r/.backplane/releases/v1/gerbers/|/r/a.kicad_pcb\nstep:kicad-cli|pcb|export|step|--subst-models|-f|-o|/r/.backplane/releases/v1/a.step|/r/a.kicad_pcb\n")
    C.Check.str("plan without a board", plan(["cpl", "sch-pdf"], "", "/r/x.kicad_sch"), "v1 /r/.backplane/releases/v1\nsch-pdf:kicad-cli|sch|export|pdf|-o|/r/.backplane/releases/v1/x-schematic.pdf|/r/x.kicad_sch\n")
    C.Check.str("plan, nothing chosen", plan(Nil{}, "/r/a.kicad_pcb", ""), "v1 /r/.backplane/releases/v1\n")
    C.Check.str("manifest", R.Release.manifest("v1", "abc123", "2026-10-12T00:00:00Z", ["gerbers/a.gbr", "a-bom.csv"]), "{\"tag\":\"v1\",\"commit\":\"abc123\",\"date\":\"2026-10-12T00:00:00Z\",\"files\":[\"gerbers/a.gbr\",\"a-bom.csv\"]}")
    C.Check.str("notes", R.Release.notes("v1.3", "v1.2", "abc123", "| Ref | Change |\n|---|---|\n| R1 | 10k -> 22k |", "ERC 0; DRC 0"), "# v1.3\n\nCommit `abc123`. Changes since v1.2.\n\n## Checks\n\nERC 0; DRC 0\n\n## BOM changes\n\n| Ref | Change |\n|---|---|\n| R1 | 10k -> 22k |\n")
    C.Check.str("notes, first, no sections", R.Release.notes("v1", "", "abc123", "", "  "), "# v1\n\nCommit `abc123`. First release.\n")
    C.Check.str("gh", argv(R.Release.gh("v1.3", "/r/.backplane/releases/v1.3/NOTES.md", ["/r/x/a.zip", "/r/x/b.csv"])), "gh|release|create|v1.3|--title|v1.3|--notes-file|/r/.backplane/releases/v1.3/NOTES.md|--verify-tag|/r/x/a.zip|/r/x/b.csv")
    C.Check.str("export command success", R.Release.command_error("Gerbers (gerbers)", 0, "tool output"), "")
    C.Check.str("export command failure", R.Release.command_error("Gerbers (gerbers)", 23, "  export failed\n"), "Gerbers (gerbers) failed (exit 23): export failed")
    C.Check.str("export command failure without output", R.Release.command_error("STEP (step)", 1, " \n"), "STEP (step) failed (exit 1)")
    C.Check.str("export diagnostic bounded", R.Release.command_error("PDF", 2, String.repeat("x", 400n)), "PDF failed (exit 2): " ++ String.repeat("x", 300n))
