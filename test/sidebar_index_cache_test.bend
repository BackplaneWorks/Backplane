import Base
import ./check.bend as C
import ../src/server/thin-side-index.bend as X
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/signal.bend as SG

def state() -> M.State:
  M.State.replay([M.ThreadCreated{"self", "p", "日本語", M.Local{}, "", "", "codex", "", 1n},
    M.ThreadCreated{"child", "p", "Child", M.Local{}, "", "", "codex", "", 2n}], M.State.empty())

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def saved() -> X.IndexCache:
  X.IndexCache.obtain.revision("epoch", 2n, state(), None{})

def after(+e: M.Change) -> X.IndexCache:
  +st = M.State.apply(state(), e)
  X.IndexCache.obtain.revision("epoch", SG.SignalRevision.advance([e], 2n, M.State.seq(st)), st, Some{saved()})

def revision(c: X.IndexCache) -> String:
  match c:
    case X.IndexCache{_, n, _, _}: Nat.show(n)

def asks(c: X.IndexCache) -> String:
  Nat.show(List.length(&2, M.Ask, U.SIx.open(X.IndexCache.index(c), "self")))

def many(+n: Nat, acc: Map<&2, Nat>) -> Map<&2, Nat>:
  match n:
    case 0n: acc
    case 1n+k: many(k, Map.set(&2, Nat, acc, Nat.show(k), 1n))

def repeated(+n: Nat, acc: List<&2, M.Ask>) -> List<&2, M.Ask>:
  match n:
    case 0n: acc
    case 1n+k: repeated(k, M.Ask{Nat.show(k), "self", "input", "", "ask detail", "", True{}, ""} <> acc)

def large_asks(n: Nat) -> U.SIx:
  U.SIx{Map.new(&2, List<&2, M.Thread>), Map.set(&2, List<&2, M.Ask>, Map.new(&2, List<&2, M.Ask>), "self", repeated(n, Nil{})), Map.new(&2, Nat), Map.new(&2, Nat)}

def large_maps(k: Nat, w: Nat) -> U.SIx:
  U.SIx{Map.new(&2, List<&2, M.Thread>), Map.new(&2, List<&2, M.Ask>), many(k, Map.new(&2, Nat)), many(w, Map.new(&2, Nat))}

def too_big() -> X.IndexCache:
  X.IndexCache.build("epoch", 2n, large_asks(16384n))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("same trusted dependency revision hits", yes(X.IndexCache.hit(saved(), "epoch", 2n)), "yes")
    C.Check.str("epoch replacement misses", yes(X.IndexCache.hit(saved(), "replacement", 2n)), "no")
    C.Check.str("dependent revision change misses", yes(X.IndexCache.hit(saved(), "epoch", 3n)), "no")
    C.Check.str("activity history preserves dependency revision", revision(after(M.ActivityLogged{"self", "act", "ok", "tool", "fresh transcript", 3n})), "2")
    C.Check.str("assistant history preserves dependency revision", revision(after(M.MessagePosted{"self", "msg", M.Assistant{}, "fresh assistant", 3n})), "2")
    C.Check.str("system history preserves dependency revision", revision(after(M.MessagePosted{"self", "msg", M.System{}, "fresh system", 3n})), "2")
    C.Check.str("user activity invalidates metadata", revision(after(M.MessagePosted{"self", "msg", M.User{}, "request", 3n})), "3")
    C.Check.str("new approval is present on miss", asks(after(M.AskOpened{"a", "self", "approval", "tool", "choose", "yes,no", 3n})), "1")
    C.Check.str("watch ownership is present on miss", Nat.show(U.SIx.work(X.IndexCache.index(after(M.WorkSet{"self", "watch", "w\twatching\t\tbuild\twatch", 3n})), "self")), "1")
    C.Check.str("delegated child ownership is present on miss", Nat.show(U.SIx.kids(X.IndexCache.index(after(M.TaskDelegated{"task", "self", "child", 1n, "codex", "", 3n})), "self")), "1")
    C.Check.str("unknown setting conservatively invalidates", revision(after(M.SettingSet{"future.ownership", "new", 3n})), "3")
    C.Check.str("bot ownership conservatively invalidates", revision(after(M.BotSet{"bot", "Bot", "child", 1n, "", 3n})), "3")
    C.Check.str("mail ownership setting conservatively invalidates", revision(after(M.SettingSet{"mail.on", "1", 3n})), "3")
    C.Check.str("empty metadata is bounded", yes(X.IndexCache.fits(U.Side.ix(M.State.empty()))), "yes")
    C.Check.str("10841 catalog entries and grouping metadata fit", yes(X.IndexCache.fits(large_maps(10841n, 1n))), "yes")
    C.Check.str("8192 combined metadata entries fit", yes(X.IndexCache.fits(large_maps(4096n, 4096n))), "yes")
    C.Check.str("16385 combined entries do not fit", yes(X.IndexCache.fits(large_maps(8192n, 8193n))), "no")
    C.Check.str("8191 asks and their grouping key fit", yes(X.IndexCache.fits(large_asks(8191n))), "yes")
    C.Check.str("16385th ask grouping entry counts toward retention cap", yes(X.IndexCache.bounded(too_big())), "no")
    C.Check.str("oversized projection cannot hit", yes(X.IndexCache.hit(too_big(), "epoch", 2n)), "no")
    C.Check.str("oversized projection is returned complete", asks(too_big()), "16384")
    C.Check.str("legacy entry uses full canonical sequence", revision(X.IndexCache.obtain("epoch", M.State.apply(state(), M.MessagePosted{"self", "m", M.Assistant{}, "history", 3n}), Some{saved()})), "3")
