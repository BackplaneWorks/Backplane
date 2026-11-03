import Base
import ./check.bend as C
import ./allowance_test.bend as F
import ./allowance_ui_test.bend as Q
import ../src/core/allowance.bend as A
import ../src/core/allowance_stream.bend as AS
import ../src/core/accounts.bend as AC
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J

def row(scope: String, used: J.Json) -> J.Json:
  J.Json.obj([J.KV{"name", J.Str{"Reported pool"}}, J.KV{"scope", J.Str{scope}}, J.KV{"used_percent", used}, J.KV{"resets_at", J.Json.nat(2000n)}, J.KV{"window_minutes", J.Json.nat(300n)}])

def quota(provider: String, ws: List<&2, J.Json>, state: String) -> J.Json:
  J.Json.obj([J.KV{"provider", J.Str{provider}}, J.KV{"state", J.Str{state}}, J.KV{"at", J.Json.nat(1000n)},
    J.KV{"windows", J.Json.arr(ws)}, J.KV{"source", J.Str{"codex-app-server"}}])

def profile(+id: String, provider: String, epoch: Nat, state: String, allowance: J.Json) -> J.Json:
  J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"label", J.Str{"Fixture " ++ id}}, J.KV{"provider", J.Str{provider}}, J.KV{"epoch", J.Json.nat(epoch)}, J.KV{"state", J.Str{state}}, J.KV{"allowance", allowance}])

def bound(+provider: String, +ps: List<&2, J.Json>, binding: String) -> M.State:
  M.State.replay([M.ThreadCreated{"t", "p", "Allowance scope fixture", M.Local{}, "", "", provider, "opus", 1n},
    M.SettingSet{"allowance." ++ provider, F.good(provider, [F.win("shared", "0")]), 1000n},
    M.SettingSet{"accounts.inventory", J.Json.show(J.Json.obj([J.KV{"profiles", J.Json.arr(ps)}])), 1000n},
    M.SettingSet{AC.Accounts.binding.key("t", provider), binding, 1000n}], M.State.empty())

def state(ps: List<&2, J.Json>) -> M.State:
  bound("codex", ps, AC.Accounts.binding.show("A", 2n))

def named(+q: J.Json) -> M.State:
  state([profile("A", "codex", 2n, "connected", q)])

def chip(+st: M.State, +provider: String, now: Nat) -> String:
  A.Allowance.chip(A.Allowance.selected(st, "t", provider), provider, "opus", now)

def ui() -> U.Ui:
  U.Ui.info_set(U.Ui.select.k(U.Ui.adopt(U.Ui.init(1000n), named(quota("codex", [row("codex", J.Num{"28"}), row("codex", J.Num{"61"})], "ok"))), "t"), "clock", "1000")

def main() -> IO(Unit):
  do IO<Unit>:
    +q : J.Json = quota("codex", [row("codex", J.Num{"28"}), row("codex", J.Num{"61"})], "ok")
    C.Check.str("selected named account limiting window", chip(named(q), "codex", 1000n), "39% left · shared")
    C.Check.str("another profile is never combined", chip(state([profile("A", "codex", 2n, "connected", q), profile("B", "codex", 2n, "connected", quota("codex", [row("codex", J.Num{"99"})], "ok"))]), "codex", 1000n), "39% left · shared")
    C.Check.str("distinct bucket identities remain separate", chip(named(quota("codex", [row("general", J.Num{"28"}), row("special", J.Num{"99"})], "ok")), "codex", 1000n), "Shared limits")
    C.Check.str("missing profile never borrows default", chip(state(Nil{}), "codex", 1000n), "")
    C.Check.str("profile provider mismatch fails closed", chip(state([profile("A", "claude", 2n, "connected", q)]), "codex", 1000n), "")
    C.Check.str("snapshot provider mismatch fails closed", chip(named(quota("claude", [row("codex", J.Num{"0"})], "ok")), "codex", 1000n), "")
    C.Check.str("replaced identity generation fails closed", chip(state([profile("A", "codex", 3n, "connected", q)]), "codex", 1000n), "")
    C.Check.str("unchecked profile fails closed", chip(state([profile("A", "codex", 2n, "unchecked", q)]), "codex", 1000n), "")
    C.Check.str("disconnected profile fails closed", chip(state([profile("A", "codex", 2n, "disconnected", q)]), "codex", 1000n), "")
    C.Check.str("malformed binding never borrows default", chip(bound("codex", Nil{}, "broken"), "codex", 1000n), "")
    C.Check.str("missing named allowance remains unavailable", chip(named(J.Null{}), "codex", 1000n), "")
    C.Check.str("string percentage is unknown", chip(named(quota("codex", [row("codex", J.Str{"28"})], "ok")), "codex", 1000n), "")
    C.Check.str("invalid window blocks partial percentage", chip(named(quota("codex", [row("codex", J.Num{"28"}), row("codex", J.Null{})], "ok")), "codex", 1000n), "")
    C.Check.str("missing pool scope stays unknown", chip(named(quota("codex", [row("", J.Num{"28"})], "ok")), "codex", 1000n), "")
    C.Check.str("fractional used rounds remaining conservatively", chip(named(quota("codex", [row("codex", J.Num{"28.01"})], "ok")), "codex", 1000n), "71% left · shared")
    C.Check.str("named retained snapshot displays last known measurement", chip(named(quota("codex", [row("codex", J.Num{"28"})], "stale")), "codex", 1000n), "72% left · shared")
    C.Check.str("quiet named reading retains its limiting window", chip(named(q), "codex", 1301n), "39% left · shared")
    C.Check.str("named deadline uses metadata lifetime", Nat.show(A.Allowance.until(A.Allowance.selected(named(q), "t", "codex"), "codex", "")), "1301")
    C.Check.str("deadline retires at reported reset", Nat.show(A.Allowance.until(F.good("codex", [F.window("short", "shared", J.Num{"280"}, J.Num{"1792100010"})]), "codex", "")), "1792100010")
    C.Check.str("named explicit check never invokes default collector", Nat.show(Q.commands(ui(), "allowance-read", "")), "0")
    C.Check.str("opening details is display only", Nat.show(Q.commands(ui(), "allowance-open", "")), "1")
    C.Check.str("named Claude cannot overwrite default snapshot", Nat.show(List.length(&2, M.Change, AS.AllowanceStream.default(False{}, M.State.empty(), 1000n, Q.stream()))), "0")
    C.Check.str("canonical named request rejects default collector", Bool.show(A.Allowance.read.allowed(named(q), "t")), "False")
    C.Check.str("canonical missing thread rejects default collector", Bool.show(A.Allowance.read.allowed(named(q), "missing")), "False")
    C.Check.str("unknown provider keeps no percentage", chip(bound("pi", Nil{}, ""), "pi", 1000n), "")
    C.Check.str("named selected account label is shown", Bool.show(String.contains(F.text(A.Allowance.rows(A.Allowance.selected(named(q), "t", "codex"), "codex", "", 1000n)), "Fixture A")), "True")
