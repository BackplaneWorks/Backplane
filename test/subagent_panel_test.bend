import Base
import ./check.bend as C
import ../src/core/subs.bend as SB
import ../src/core/i18n.bend as I
import ../src/web/view.bend as V
import ../src/web/dom.bend as D

def row(+key: String, agent: Bool, +word: String, live: Bool) -> SB.SubRow:
  SB.SubRow{key, agent, key, "claude", word, "", live, False{}, 0n, Nil{}}

def mixed() -> List<&2, SB.SubRow>:
  [row("agent", True{}, "running", True{}), row("child", False{}, "running", True{}),
    SB.Subs.done.toggle("t1", 1n, True{}), row("finished-agent", True{}, "done", False{})]

def keys(ns: List<&2, D.Node>) -> String:
  match ns:
    case Nil{}: ""
    case D.El{_, key, _, _} <> rest: key ++ ";" ++ keys(rest)
    case _ <> rest: keys(rest)

def many(+n: Nat) -> List<&2, SB.SubRow>:
  match n:
    case 0n: Nil{}
    case 1n+k: row(Nat.show(k), True{}, "done", False{}) <> many(k)

def main() -> IO(Unit):
  do IO<Unit>:
    +info : Map<&2, String> = Map.new(&2, String)
    C.Check.str("web renders live agents, delegated children and finished rows", keys(V.View.subs.signals.go(I.En{}, mixed(), info)), "agent;child;subs.done:t1;finished-agent;")
    +lazy : Map<&2, String> = Map.set(&2, String, info, "local.view.lazy", "1")
    C.Check.str("thin page renders the same row identities", keys(V.View.subs.signals.go(I.En{}, mixed(), lazy)), "agent;child;subs.done:t1;finished-agent;")
    C.Check.str("finished panel paginates all its rows", Nat.show(List.length(&2, D.Node, V.View.subs.signals.go(I.En{}, many(20n), lazy))), "17")
    C.Check.str("closing still records work signatures", String.join(SB.Subs.keys(mixed()), ";"), "live:agent;live:child")
