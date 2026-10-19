import Base
import ../src/core/telemetry.bend as T
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/onboard.bend as O

def Cases.id() -> String:
  "7d0c2f6e-3b1a-4f7e-9a51-2c8e0d4b6f10"
def Cases.id2() -> String:
  "00000000-0000-4000-8000-000000000000"
def Cases.ev(+e: String, +ver: String, +share: Bool) -> T.TmEv:
  T.TmEv{e, "2026-10-14", share, Bool.pick(String, share, Cases.id(), ""), ver, "", "release", "linux", "x64", Bool.pick(String, Bool.and(String.eq(e, "install"), share), "1", ""), 1n, False{}}
def Cases.view(share: Bool, asked: Bool, reset: String, ver: String, day: String, now: Nat) -> T.TmView:
  T.TmView{share, asked, reset, ver, "release", "linux", "x64", day, now}
def Cases.st(queue: List<&2, T.TmEv>) -> T.TmState:
  T.TmState{Cases.id(), "0.11.3", "2026-10-14", True{}, "0", queue, 0n, 0n, True{}}
def Cases.plan(p: T.TmPlan) -> T.TmState:
  match p:
    case T.TmPlan{st, _}: st
def Cases.run(st: T.TmState, v: T.TmView) -> T.TmState:
  Cases.plan(T.Tm.plan(st, v, Cases.id2()))
def Cases.off() -> T.TmState:
  Cases.run(Cases.st([Cases.ev("install", "0.11.3", True{}), Cases.ev("update", "0.11.3", True{}), Cases.ev("active", "0.11.3", True{})]), Cases.view(False{}, True{}, "0", "0.11.3", "2026-10-14", 2n))
def Cases.body(+st: T.TmState) -> String:
  T.Tm.body(2n, "0123456789abcdef", T.Tm.ready(T.TmState.queue(st), 2n))
def Cases.unasked() -> T.TmState:
  Cases.run(T.Tm.fresh(), Cases.view(True{}, False{}, "0", "0.11.3", "2026-10-14", 1n))
def Cases.twice() -> T.TmState:
  Cases.run(Cases.run(Cases.st(Nil{}), Cases.view(True{}, True{}, "0", "0.11.3", "2026-10-15", 2n)), Cases.view(True{}, True{}, "0", "0.11.3", "2026-10-15", 3n))
def Cases.repeat(n: Nat) -> List<&2, T.TmEv>:
  match n:
    case 0n: Nil{}
    case 1n+m: Cases.ev("active", "0.11.3", True{}) <> Cases.repeat(m)
def Cases.bad(ver: String) -> String:
  T.Tm.body(1n, "0123456789abcdef", [Cases.ev("install", ver, False{})])
def Cases.ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u
def Cases.old() -> U.Ui:
  U.Ui.scratch_set(U.Ui.info_set(U.Ui.info_set(U.Ui.adopt.raw(U.Ui.init(1n), M.State.replay([M.ProjectCreated{"p", "private project", "/private", 1n}, M.SettingSet{"onboard.done", "1", 1n}], M.State.empty())), "os", "linux"), "telemetry.live", "1"), "@sync", "1")
def Cases.version(u: U.Ui) -> U.Ui:
  match u:
    case U.Ui{st, a, b, c, d, e, f, g, h, _, j, k, l, m, n, o, p}: U.Ui{st, a, b, c, d, e, f, g, h, "0.11.3", j, k, l, m, n, o, p}
def Cases.setting(a: U.Act, key: String) -> String:
  M.Setting.get(U.Ui.state(Cases.ui(a)), key, "")

def Cases.updates(+n: Nat) -> List<&2, T.TmEv>:
  match n:
    case 0n: Nil{}
    case 1n+m: Cases.ev("update", "0.11." ++ Nat.show(m), True{}) <> Cases.updates(m)
