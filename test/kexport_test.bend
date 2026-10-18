import Base
import ./check.bend as C
import ../src/core/kexport.bend as K
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/core/kihealth.bend as KH
import ../src/mobile/screen.bend as MOB
import ../src/web/view.bend as WEB
import ../src/web/dom.bend as D

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def obj(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def sample() -> J.Json:
  obj("{\"entries\":[{\"category\":\"lower\",\"feature\":\"Text\",\"detail\":\"Converted\",\"count\":2},{\"category\":\"drop\",\"feature\":\"Data\",\"detail\":\"Removed\",\"count\":1},{\"category\":\"keep\",\"feature\":\"Companion\",\"detail\":\"board.backplane.json\",\"count\":1}],\"future\":true}")

def blocked() -> K.KReport:
  K.KX.report(obj("{\"entries\":[{\"category\":\"block\",\"feature\":\"Line endings\",\"detail\":\"Not representable\",\"count\":1}]}"))

def paths() -> K.KPaths:
  K.KX.paths("/tmp/bp", "/design/My Board.kicad_pro")

def note(n: Maybe<&2, KH.KNote>) -> String:
  match n:
    case None{}: "none"
    case Some{KH.KNote{_, _, b, _}}: b

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("project stem", K.KX.name("/design/My Board.kicad_pro"), "My Board")
    C.Check.str("base outside project", yes(String.starts_with(K.KX.base(paths()), "/tmp/bp/exports/My-Board-")), "yes")
    C.Check.str("output", K.KX.output(paths()), K.KX.base(paths()) ++ "/kicad10/")
    C.Check.str("report outside output", K.KX.report_path(paths()), K.KX.base(paths()) ++ "/report.json")
    C.Check.str("path hash disambiguates", yes(String.eq(K.KX.base(K.KX.paths("/tmp/bp", "/a/x.kicad_pro")), K.KX.base(K.KX.paths("/tmp/bp", "/b/x.kicad_pro")))), "no")
    C.Check.str("empty", yes(K.KX.empty(K.KX.report(obj("{\"entries\":[]}")))), "yes")
    C.Check.str("invalid report refused", yes(K.KX.valid(obj("{\"future\":[]}"))), "no")
    C.Check.str("lower drop and companion", String.join(K.KX.lines(K.KX.report(sample())), "|"), "Changed for KiCad 10|Text: Converted (2)|Removed|Data: Removed (1)|Kept beside the copy|Companion: board.backplane.json (1)")
    C.Check.str("unknown metadata does not invalidate consent", K.KX.signature(sample()), K.KX.signature(obj("{\"entries\":[{\"category\":\"lower\",\"feature\":\"Text\",\"detail\":\"Converted\",\"count\":2},{\"category\":\"drop\",\"feature\":\"Data\",\"detail\":\"Removed\",\"count\":1},{\"category\":\"keep\",\"feature\":\"Companion\",\"detail\":\"board.backplane.json\",\"count\":1}],\"timestamp\":99}")))
    C.Check.str("unknown fields tolerated", yes(K.KX.valid(sample())), "yes")
    C.Check.str("unknown category needs consent", yes(K.KX.block(K.KX.report(obj("{\"entries\":[{\"category\":\"future\"}]}")))), "yes")
    C.Check.str("block", yes(K.KX.block(blocked())), "yes")
    C.Check.str("no force without consent", String.join(K.KX.flags(blocked(), False{}), "|"), "")
    C.Check.str("lossy force only", String.join(K.KX.flags(K.KX.report(sample()), True{}), "|"), "--force")
    C.Check.str("block flags", String.join(K.KX.flags(blocked(), True{}), "|"), "--force|--drop-approximations")
    C.Check.str("marker required", yes(K.KX.may_clear(K.KX.base(paths()), False{})), "no")
    C.Check.str("marked managed base", yes(K.KX.may_clear(K.KX.base(paths()), True{})), "yes")
    C.Check.str("outside exports refused", yes(K.KX.may_clear("/tmp/design", True{})), "no")
    C.Check.str("traversal refused", yes(K.KX.may_clear("/tmp/bp/exports/../design", True{})), "no")
    C.Check.str("other home refused", yes(K.KX.may_clear_in("/tmp/home", "/elsewhere/exports/x", True{})), "no")
    C.Check.str("nested export base refused", yes(K.KX.may_clear_in("/tmp/home", "/tmp/home/exports/x/y", True{})), "no")
    C.Check.str("root refused", yes(K.KX.may_clear("/", True{})), "no")
    +r : J.Json = J.Json.obj([J.KV{"export10", J.Str{"confirm"}}, J.KV{"thread", J.Str{"t1"}}, J.KV{"report", sample()}, J.KV{"text", J.Str{"Review"}}])
    +u : U.Ui = U.Ui.recv(U.Ui.init(1n), J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"ok", J.Flag{True{}}}, J.KV{"result", r}]))
    C.Check.str("client report retained", yes(U.KX.pending(U.Ui.infos(u), "t1")), "yes")
    C.Check.str("other thread unaffected", yes(U.KX.pending(U.Ui.infos(u), "t2")), "no")
    C.Check.str("grouped report visible", yes(String.contains(note(U.KX.note(U.Ui.infos(u), "t1")), "Changed for KiCad 10")), "yes")
    C.Check.str("phone report rows include lower bucket", yes(String.contains(J.Json.show(MOB.Mob.kx.menu(U.KX.note(U.Ui.infos(u), "t1"))), "Changed for KiCad 10")), "yes")
    C.Check.str("phone export button", yes(String.contains(J.Json.show(MOB.Mob.kx.menu(U.KX.note(U.Ui.infos(u), "t1"))), "kicad-export10-anyway")), "yes")
    C.Check.str("web panel", Nat.show(List.length(&2, D.Node, WEB.View.kx.note(U.KX.note(U.Ui.infos(u), "t1"), U.Ui.infos(u), "t1"))), "1")
    C.Check.str("cancel clears only report", yes(U.KX.pending(U.Ui.infos(U.Act.ui(U.Ui.act(u, "kicad-export10-cancel", "t1"))), "t1")), "no")
    C.Check.str("success message", K.KX.success("/copy/"), "Exported a KiCad 10 copy to /copy/. Changes made there don't come back into this project.")
