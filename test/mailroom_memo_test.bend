import Base
import ./check.bend as C
import ./mailroom_client_test.bend as MC
import ./mailroom_parity_test.bend as PT
import ../src/app/mm.bend as MM
import ../src/app/nui.bend as N
import ../src/app/layout.bend as L
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J

def key(+u: U.Ui) -> String:
  MM.Mm.mail(N.Nui.init(u, 1400, 900), "same plane data")

def equal(name: String, a: String, b: String, want: Bool) -> IO(Unit):
  C.Check.str(name, Bool.show(String.eq(a, b)), Bool.show(want))

def keep(name: String, now: Nat, m: MM.BotM, want: Bool) -> IO(Unit):
  C.Check.str(name, Bool.show(MM.BotM.same("k", now, m)), Bool.show(want))

def seeded() -> U.Ui:
  U.Mailv.put(PT.ui(), U.Mailv.list.put(U.Mailv.data(PT.ui()), U.Mailv.cache(MC.ui())))

def reset(+u: U.Ui, since: Nat, origin: String) -> U.Ui:
  U.Ui.reset(u, J.Json.obj([J.KV{"since", J.Json.nat(since)}, J.KV{"origin", J.Str{origin}}, J.KV{"items", J.Json.arr(Nil{})}]))

def main() -> IO(Unit):
  do IO<Unit>:
    +u : U.Ui = PT.ui()
    C.Check.str("renderer capability enables Mailroom", Bool.show(U.Mailv.shown(u)), "True")
    C.Check.str("client without renderer does not request mail", Bool.show(U.Mailv.shown(U.Ui.info_set(u, "local.mailroom", ""))), "False")
    equal("same clock minute alone is not a body input", key(u), key(U.Ui.tick(u, Nat.add(U.Side.now(u), 1n))), True{})
    equal("origin invalidates with equal revisions", key(u), key(U.Ui.info_set(u, "origin", "new-hub")), False{})
    equal("generation invalidates with equal origin", key(u), key(U.Ui.info_set(u, "local.generation", "next")), False{})
    equal("tab is explicit", key(u), key(U.Ui.scratch_set(u, "@btab", "status")), False{})
    equal("list scroll invalidates", key(u), key(U.Ui.info_set(u, "pane:mr", "2")), False{})
    equal("body scroll invalidates", key(u), key(U.Ui.info_set(u, "pane:mrbody", "1")), False{})
    equal("thread draft stays outside mail key", key(u), key(U.Ui.scratch_set(u, "another-thread", "draft")), True{})
    +origin : U.Ui = U.Mailv.see(u, M.MessageFrom{"t1", "m1", "thread:x", "thread", "", 200001n})
    equal("MessageFrom advances body revision", key(u), key(origin), False{})
    C.Check.str("origin does not change plane stamp", U.Ui.scratch(origin, "@mr.plane"), U.Ui.scratch(u, "@mr.plane"))
    C.Check.str("age before 60 uses relative timestamp", Nat.show(MM.BotM.age_due(160n, 101n)), "161")
    C.Check.str("age minute boundary advances deadline", Nat.show(MM.BotM.age_due(161n, 101n)), "221")
    C.Check.str("hour boundary", Nat.show(MM.BotM.age_due(3700n, 101n)), "3701")
    C.Check.str("future time stays now", Nat.show(MM.BotM.age_due(10n, 101n)), "161")
    +memo : MM.BotM = MM.BotM{"k", L.Out.none(), 100n, 161n, False{}}
    keep("before drawn age changes is reusable", 160n, memo, True{})
    keep("exact age change invalidates", 161n, memo, False{})
    keep("clock backwards invalidates", 99n, memo, False{})
    keep("static empty body has no clock deadline", 999999n, MM.BotM{"k", L.Out.none(), 1n, 0n, False{}}, True{})
    +cache : U.Ui = U.Ui.info_set(seeded(), "origin", "same")
    C.Check.str("full replay drops stale mail cache", Nat.show(U.Mailv.rev(reset(cache, 0n, "same"))), "0")
    C.Check.str("new origin drops stale mail cache", Nat.show(U.Mailv.rev(reset(cache, 1n, "other"))), "0")
    C.Check.str("real same-origin resume keeps cache", Nat.show(U.Mailv.rev(reset(cache, 1n, "same"))), Nat.show(U.Mailv.rev(cache)))
