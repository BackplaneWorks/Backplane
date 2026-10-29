import Base
import ../../src/mobile/screen.bend as MOB
import ../../src/mobile/app.bend as APP
import ../../src/core/model.bend as M
import ../../src/core/fold.bend as F
import ../../src/core/stamp.bend as ST
import ../../src/core/i18n.bend as I
import ../../src/core/json.bend as J
import ../check.bend as C

def entries() -> List<&2, M.Entry>:
  [M.Msg{"u", "t", M.User{}, "begin", 1n},
   M.Act{"a", "t", "tool", "shell", String.repeat("# command\n**hidden** [link](https://example.invalid)\n", 600n), 2n},
   M.Act{"b", "t", "tool", "read", "second", 3n},
   M.Msg{"foreign", "other", M.Assistant{}, "cross-thread", 4n},
   M.Msg{"done", "t", M.Assistant{}, "finished", 5n}]

def legacy(es: List<&2, M.Entry>, rs: List<&2, F.Role>, +st: M.State, +sp: ST.Stamps, +run: String, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match es rs:
    case Nil{} _: acc
    case e <> t +r <> rt:
      +at = MOB.Mob.run.of(r, run)
      legacy(t, rt, st, sp, at, MOB.Mob.role(r, MOB.Mob.run.mark(at, MOB.Mob.entry(I.En{}, e, st, "t", Map.new(&2, String), sp)), acc))
    case e <> t Nil{}:
      legacy(t, Nil{}, st, sp, "", MOB.Mob.onto(MOB.Mob.entry(I.En{}, e, st, "t", Map.new(&2, String), sp), acc))

def output(+es: List<&2, M.Entry>, +rs: List<&2, F.Role>, old: Bool) -> String:
  +st = M.State.empty()
  +sp = ST.Stamps.of.s(es, st, 100n, False{}, False{})
  +rows = Bool.pick(List<&2, J.Json>, old, legacy(es, rs, st, sp, "", Nil{}), MOB.Mob.entries.go(I.En{}, es, rs, st, "t", Map.new(&2, String), sp, "", Nil{}))
  J.Json.show(J.Json.arr(List.reverse(&2, J.Json, rows)))

def verify(+name: String, +es: List<&2, M.Entry>, +rs: List<&2, F.Role>) -> IO(Unit):
  C.Check.str(name, Bool.show(C.Check.same(output(es, rs, False{}), output(es, rs, True{}))), "True")

def partial(+rows: String) -> String:
  +j = Maybe.default(&2, J.Json, J.Json.parse(rows), J.Null{})
  +shown = J.Json.obj([J.KV{"thread", J.Json.obj([J.KV{"entries", j}])}])
  +window = Maybe.default(&2, J.Json, J.Json.parse("{\"partial\":[{\"id\":\"a\",\"version\":\"3\",\"cursor\":\"c\",\"hasMore\":true,\"complete\":false}]}"), J.Null{})
  J.Json.show(APP.thin_partial(shown, window))

def main() -> IO(Unit):
  do IO<Unit>:
    +es : List<&2, M.Entry> = entries()
    +no : Map<&2, String> = Map.new(&2, String)
    verify("closed head and tail / fallback cross-thread", es, [F.RShow{}, F.RHead{"fold", "tools", False{}, False{}}, F.RIn{False{}}])
    verify("open header and body", es, [F.RShow{}, F.RHead{"fold", "tools", True{}, False{}}, F.RIn{True{}}, F.RShow{}, F.RShow{}])
    verify("open work closed nested fold", es, [F.RShow{}, F.RWork{"work", "work", True{}, F.RHead{"fold", "tools", False{}, False{}}}, F.RIn{False{}}, F.RShow{}, F.RShow{}])
    verify("closed work open inner", es, [F.RShow{}, F.RWork{"work", "work", False{}, F.RHead{"fold", "tools", True{}, False{}}}, F.RIn{False{}}, F.RShow{}, F.RShow{}])
    verify("open work open inner", es, [F.RShow{}, F.RWork{"work", "work", True{}, F.RHead{"fold", "tools", True{}, False{}}}, F.RIn{True{}}, F.RShow{}, F.RShow{}])
    verify("real roles stopped", es, F.Fold.roles.loc(I.En{}, es, no, False{}))
    verify("real roles running", es, F.Fold.roles.loc(I.En{}, es, no, True{}))
    verify("explicit open", es, F.Fold.roles.loc(I.En{}, es, Map.set(&2, String, no, "fold:a", "1"), False{}))
    verify("explicit close while running", es, F.Fold.roles.loc(I.En{}, es, Map.set(&2, String, no, "fold:a", "0"), True{}))
    +open : List<&2, F.Role> = [F.RShow{}, F.RHead{"fold", "tools", True{}, False{}}, F.RIn{True{}}, F.RShow{}, F.RShow{}]
    C.Check.str("partial authority preserved", Bool.show(C.Check.same(partial(output(es, open, False{})), partial(output(es, open, True{})))), "True")
    C.Check.str("closed nested body skipped", Bool.show(MOB.Mob.role.body(F.RWork{"w", "work", True{}, F.RHead{"h", "head", False{}, False{}}})), "False")
    C.Check.str("open nested body included", Bool.show(MOB.Mob.role.body(F.RWork{"w", "work", True{}, F.RHead{"h", "head", True{}, False{}}})), "True")
