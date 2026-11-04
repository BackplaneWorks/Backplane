import Base
import ./check.bend as C
import ../src/server/server.bend as S
import ../src/server/botnet.bend as BN
import ../src/server/kicad.bend as K
import ../src/core/cad-install.bend as CI
import ../src/core/json.bend as J

# The actual Cfg consumer is folded in the adversarial order: A captured,
# A terminal, B reserved, late A delivered, B terminal, dismissed, late B.
# This is a consumer/queue-handoff regression, not a whole-hub wire test.
def cfg() -> S.Cfg:
  S.Cfg{"/tmp/unused-cad-handoff", "", "127.0.0.1", 0, "", Nil{}, "test",
    S.Net{False{}, False{}, "", "", "", K.Rt.empty(), None{}, None{}, None{}, "[]", None{}, BN.Bn.empty(), True{}, "", "", None{}}, Nil{}, Nil{}, "", ""}

def begin(+c: S.Cfg, +tool: String) -> S.Cfg:
  S.Cfg.cad.store(c, tool, CI.Cad.begin(S.Cfg.cad.at(c, tool)))

def snapshot(attempt: String, phase: String, done: Nat) -> String:
  J.Json.show(J.Json.obj([J.KV{"attempt", J.Str{attempt}}, J.KV{"phase", J.Str{phase}}, J.KV{"done", J.Json.nat(done)}, J.KV{"total", J.Num{"1000"}}, J.KV{"error", J.Str{"fixture failure"}}]))

def main() -> IO(Unit):
  do IO<Unit>:
    +a : S.Cfg = begin(cfg(), "kicad")
    +captured : String = snapshot("1", "download", 400n)
    +terminal : S.Cfg = S.Cfg.cad.apply(a, "kicad", snapshot("1", "failed", 0n))
    +b : S.Cfg = begin(terminal, "kicad")
    C.Check.str("start reservation is visible before first poll", J.Json.get_str(S.Cfg.cad.at(a, "kicad"), "phase", ""), "starting")
    C.Check.str("retry reservation replaces old terminal immediately", J.Json.get_str(S.Cfg.cad.at(b, "kicad"), "phase", ""), "starting")
    C.Check.str("retry has a new consumer-owned token", J.Json.get_str(S.Cfg.cad.at(b, "kicad"), "attempt", ""), "2")
    C.Check.str("captured A cannot overwrite accepted B", S.Cfg.cad(S.Cfg.cad.apply(b, "kicad", captured)), S.Cfg.cad(b))
    +live : S.Cfg = S.Cfg.cad.apply(b, "kicad", snapshot("2", "download", 700n))
    C.Check.str("consumer in-phase progress does not decrease", Nat.show(J.Json.get_nat(S.Cfg.cad.at(S.Cfg.cad.apply(live, "kicad", snapshot("2", "download", 100n)), "kicad"), "done", 0n)), "700")
    C.Check.str("active dismiss preserves reconnect state exactly", S.Cfg.cad(S.Cfg.cad.clear(live, "kicad", "2")), S.Cfg.cad(live))
    C.Check.str("join info contains active snapshot", J.Json.get_str(S.Hub.info(live), "cad.install", ""), S.Cfg.cad(live))
    C.Check.str("unknown tool cannot grow the two-tool snapshot", S.Cfg.cad(S.Cfg.cad.store(live, "other", CI.Cad.begin(J.Null{}))), S.Cfg.cad(live))
    +done : S.Cfg = S.Cfg.cad.apply(live, "kicad", snapshot("2", "failed", 0n))
    +cleared : S.Cfg = S.Cfg.cad.clear(done, "kicad", "2")
    C.Check.str("terminal dismiss leaves only a generation tombstone", J.Json.get_str(S.Cfg.cad.at(cleared, "kicad"), "phase", ""), "dismissed")
    C.Check.str("terminal dismiss clears the error", J.Json.get_str(S.Cfg.cad.at(cleared, "kicad"), "error", ""), "")
    C.Check.str("late same-token result cannot resurrect dismissal", S.Cfg.cad(S.Cfg.cad.apply(cleared, "kicad", snapshot("2", "failed", 0n))), S.Cfg.cad(cleared))
    +again : S.Cfg = begin(cleared, "kicad")
    C.Check.str("new start after dismiss has fresh visible token", J.Json.get_str(S.Cfg.cad.at(again, "kicad"), "attempt", ""), "3")
    C.Check.str("old terminal dismiss cannot touch new active attempt", S.Cfg.cad(S.Cfg.cad.clear(again, "kicad", "2")), S.Cfg.cad(again))
    +two : S.Cfg = begin(again, "freecad")
    C.Check.str("captured old message preserves both tools and order", S.Cfg.cad(S.Cfg.cad.apply(two, "kicad", captured)), S.Cfg.cad(two))
