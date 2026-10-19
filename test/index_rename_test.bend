import Base
import ../src/app/mm.bend as MM
import ../src/app/nui.bend as N
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/codec.bend as C
import ../src/core/json.bend as J
import ./sidebar_test.bend as SD

# Independent oracle: every indexed thread field, asks, children and work
# compared to Side.ix built fresh. No cached checksum or title-only test.
def Fp.q(s: String) -> String:
  J.Json.show(J.Str{s})

def Fp.b(b: Bool) -> String:
  Bool.pick(String, b, "1", "0")

def Fp.ov(o: M.Override) -> String:
  match o:
    case M.NoOverride{}:
      "n"
    case M.ForcedSettled{}:
      "s"
    case M.ForcedActive{}:
      "a"

def Fp.thread(t: M.Thread) -> String:
  match t:
    case M.Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de}:
      Fp.q(id) ++ "|" ++ Fp.q(p) ++ "|" ++ Fp.q(ti) ++ "|" ++ Nat.show(c) ++ "|" ++ Nat.show(a) ++ "|" ++ C.Turn.name(tu) ++ "|" ++ Nat.show(pe) ++ "|" ++ Fp.b(ar) ++ "|" ++ Fp.ov(o)
        ++ "|" ++ Nat.show(sa) ++ "|" ++ Nat.show(ua) ++ "|" ++ Fp.b(pi) ++ "|" ++ C.Env.name(en) ++ "|" ++ Fp.q(br) ++ "|" ++ Fp.q(wt) ++ "|" ++ Fp.q(pr) ++ "|" ++ Fp.q(mo)
        ++ "|" ++ C.Runtime.name(rm) ++ "|" ++ C.Interaction.name(im) ++ "|" ++ Fp.q(se) ++ "|" ++ Fp.b(de) ++ ";"

def Fp.threads(ts: List<&2, M.Thread>) -> String:
  match ts:
    case Nil{}:
      ""
    case t <> rest:
      Fp.thread(t) ++ Fp.threads(rest)

def Fp.by(xs: List<&2, Sigma<&2, &2, String, _ => List<&2, M.Thread>>>) -> String:
  match xs:
    case Nil{}:
      ""
    case (+k, v) <> t:
      "[" ++ Fp.q(k) ++ "]" ++ Fp.threads(v) ++ Fp.by(t)

def Fp.askl(as: List<&2, M.Ask>) -> String:
  match as:
    case Nil{}:
      ""
    case M.Ask{i, th, k, tl, d, o, op, an} <> t:
      Fp.q(i) ++ "," ++ Fp.q(th) ++ "," ++ Fp.q(k) ++ "," ++ Fp.q(tl) ++ "," ++ Fp.q(d) ++ "," ++ Fp.q(o) ++ "," ++ Fp.b(op) ++ "," ++ Fp.q(an) ++ ";" ++ Fp.askl(t)

def Fp.asks(xs: List<&2, Sigma<&2, &2, String, _ => List<&2, M.Ask>>>) -> String:
  match xs:
    case Nil{}:
      ""
    case (+k, v) <> t:
      "[" ++ Fp.q(k) ++ "]" ++ Fp.askl(v) ++ Fp.asks(t)

def Fp.nats(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>) -> String:
  match xs:
    case Nil{}:
      ""
    case (+k, v) <> t:
      Fp.q(k) ++ "=" ++ Nat.show(v) ++ ";" ++ Fp.nats(t)

def Fp.ix(x: U.SIx) -> String:
  match x:
    case U.SIx{by, a, k, w}:
      Fp.by(Map.to_list(&2, List<&2, M.Thread>, by)) ++ "#" ++ Fp.asks(Map.to_list(&2, List<&2, M.Ask>, a)) ++ "#" ++ Fp.nats(Map.to_list(&2, Nat, k)) ++ "#" ++ Fp.nats(Map.to_list(&2, Nat, w))


def check(ok: Bool, label: String) -> IO(Unit):
  match ok:
    case True{}: IO.print("ok " ++ label)
    case False{}: IO.die(Unit, 1, "FAIL index rename: " ++ label)

def items(cs: List<&2, M.Change>) -> List<&2, J.Json>:
  match cs:
    case Nil{}: Nil{}
    case c <> rest: C.Change.encode(c) <> items(rest)

def changes(cs: List<&2, M.Change>) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"changes"}}, J.KV{"items", J.Json.arr(items(cs))}])

type Sample is Data:
  Sample{ui: U.Ui, touch: MM.TouchK, index: MM.IxK, mode: Nat}

def Sample.ui(s: Sample) -> U.Ui:
  match s:
    case Sample{u, _, _, _}: u

def Sample.touch(s: Sample) -> MM.TouchK:
  match s:
    case Sample{_, t, _, _}: t

def Sample.index(s: Sample) -> MM.IxK:
  match s:
    case Sample{_, _, k, _}: k

def Sample.mode(s: Sample) -> Nat:
  match s:
    case Sample{_, _, _, m}: m

def Sample.seed() -> Sample:
  +u = U.Ui.fold([M.SettingSet{"bg.a", "job\tworking", 1000000n}, M.MessagePosted{"b", "ref", M.Assistant{}, "[A](backplane://thread/a)", 1000000n}], SD.base())
  Sample{u, MM.TouchK.none(), MM.Ix.full(u, 0n), 0n}

def Sample.step.k(+u: U.Ui, +t: MM.TouchK, +k: MM.IxK) -> Sample:
  Sample{u, t, MM.Ix.up([k], t, u), MM.Ix.mode(MM.IxK.n(k), MM.IxK.lt(k), t, u)}

def Sample.step(+cs: List<&2, M.Change>, +s: Sample) -> Sample:
  Sample.step.k(U.Ui.fold(cs, Sample.ui(s)), MM.TouchK.msg(changes(cs), Sample.touch(s)), Sample.index(s))

def Sample.fresh(+s: Sample, label: String) -> IO(Unit):
  check(String.eq(Fp.ix(MM.IxK.ix(Sample.index(s))), Fp.ix(U.Side.ix(U.Ui.state(Sample.ui(s))))), label)

def rename() -> M.Change:
  M.ThreadRenamed{"a", "Renamed 猫 | \"x\" [;]", 1000001n}

def first() -> Sample:
  Sample.step([rename()], Sample.seed())

def global() -> M.Change:
  M.ProjectCreated{"p5", "New project", "/tmp/new", 1000002n}

def gfirst() -> Sample:
  Sample.step([global()], Sample.seed())

def key(+s: Sample) -> String:
  MM.Mm.tlkey(N.Nui.init(SD.ui(U.Ui.select(Sample.ui(s), "b")), 1400, 900), Sample.touch(s))

def main() -> IO(Unit):
  do IO<Unit>:
    Sample.fresh(first(), "rename exact complete index")
    check(Nat.is_eq(Sample.mode(first()), 2n), "rename takes incremental path rather than full Side.ix")
    check(Nat.is_eq(MM.TouchK.g(Sample.touch(first())), 1n), "rename still globally invalidates linked titles")
    check(Nat.is_eq(MM.TouchK.r(Sample.touch(first())), 0n), "rename does not erase/add relationship stamp")
    check(Nat.is_eq(MM.TouchK.index_g(Sample.touch(first())), 0n), "index-only global stamp excludes pure rename")
    check(String.eq(String.join(MM.Ix.since.lg(MM.TouchK.lg(Sample.touch(first())), 0n), ","), "a"), "renamed ID remains in newest-first change log")
    check(Bool.not(String.eq(key(first()), key(Sample.seed()))), "unrelated open timeline key still changes for linked title")
    check(Nat.is_eq(MM.TouchK.at(Sample.touch(first()), "#side"), 1n), "sidebar presentation invalidated on rename")
    Sample.fresh(Sample.step([M.ThreadRenamed{"a", "Again", 1000002n}], first()), "successive renames exact")
    Sample.fresh(Sample.step([rename(), M.ThreadRenamed{"b", "Second", 1000002n}], Sample.seed()), "two renames in one hub frame exact")
    Sample.fresh(Sample.step([M.AskOpened{"q2", "a", "input", "AskUserQuestion", "Q|;", "", 1000002n}], first()), "asks after rename remain exact")
    Sample.fresh(Sample.step([M.SettingSet{"watch.a", "watch\tmonitoring", 1000003n}], first()), "work settings after rename remain exact")
    Sample.fresh(Sample.step([M.ThreadCreated{"new", "p2", "Added", M.Local{}, "", "", "codex", "", 1000003n}, rename()], Sample.seed()), "membership creation still falls back to exact full construction")
    Sample.fresh(Sample.step([M.ThreadDeleted{"a", 1000004n}, rename()], Sample.seed()), "deleted rename cannot retain obsolete index member")
    Sample.fresh(Sample.step([M.ThreadRenamed{"missing", "No record", 1000003n}], first()), "missing thread rename fallback exact")
    Sample.fresh(Sample.step([M.ThreadCreated{"child", "p1", "Child", M.Local{}, "", "", "codex", "", 1000003n}, M.TaskDelegated{"task", "a", "child", 1n, "codex", "", 1000004n}, rename()], Sample.seed()), "child membership and parent counts remain exact")
    Sample.fresh(Sample.step([global(), rename()], Sample.seed()), "global change before rename in same frame stays exact")
    check(Nat.is_eq(Sample.mode(Sample.step([global(), rename()], Sample.seed())), 1n), "rename cannot hide earlier same-frame global debt")
    check(Nat.is_eq(Sample.mode(Sample.step([rename(), global()], Sample.seed())), 1n), "later same-frame global debt still forces full")
    Sample.fresh(Sample.step([rename(), global()], Sample.seed()), "global change after rename remains exact")
    check(Nat.is_eq(MM.TouchK.index_g(MM.TouchK.msg(changes([rename()]), MM.TouchK.msg(changes([global()]), MM.TouchK.none()))), 1n), "separate pending global message survives later rename")
    check(Nat.is_eq(MM.Ix.mode(0n, "", MM.TouchK.msg(changes([rename()]), MM.TouchK.msg(changes([global()]), MM.TouchK.none())), SD.base()), 1n), "pending global message still forces full index")
    check(Nat.is_eq(MM.Ix.mode(0n, "", MM.TouchK.msg(changes([rename()]), MM.TouchK.ids(["!"], MM.TouchK.tick(MM.TouchK.none()))), SD.base()), 1n), "relationship invalidation survives same-frame rename")
    check(Nat.is_eq(MM.Ix.mode(0n, "", MM.TouchK.msg(J.Json.obj([J.KV{"t", J.Str{"reply"}}]), Sample.touch(first())), SD.base()), 1n), "unknown/reply message conservatively forces full")
    check(Nat.is_eq(MM.Ix.mode(0n, "", MM.TouchK.items([J.Null{}], MM.TouchK.tick(Sample.touch(first()))), SD.base()), 1n), "undecodable change conservatively forces full")
    check(Nat.is_eq(MM.TouchK.index_g(MM.TouchK{5n, 4n, 0n, Map.new(&2, Nat), Nil{}}), 4n), "old/manual touch records fall back to global stamp")
    check(Bool.not(List.is_empty(&2, MM.TkE, MM.TouchK.lg(MM.TouchK.consumed(Sample.touch(first()), [0n, 1n])))), "lagging registered reader retains rename ID")
    check(List.is_empty(&2, MM.TkE, MM.TouchK.lg(MM.TouchK.consumed(Sample.touch(first()), [1n]))), "consumed rename log clears only after reader catches up")
    check(Nat.is_eq(MM.TouchK.index_g(MM.TouchK.consumed(Sample.touch(first()), [1n])), 0n), "consumption does not manufacture a global index touch")
