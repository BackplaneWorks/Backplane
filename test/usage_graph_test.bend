import Base
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/usage_graph.bend as G
import ../src/core/usage_page.bend as UP
import ../src/core/i18n.bend as I
import ../src/core/json.bend as J
import ../src/web/usage-page.bend as W
import ../src/web/dom.bend as D

def base() -> M.State:
  M.State.replay([
    M.ProjectCreated{"p", "Board", "/tmp/usage-fixture", 1n},
    M.ThreadCreated{"t", "p", "Work", M.Local{}, "", "", "claude", "m", 1n},
    M.TurnUsage{"t", "claude", "m", "one", 10n, 20n, 30n, 40n, 5n, 100000000n, "irwotc", "", 1792100000n},
    M.TurnUsage{"t", "codex", "m", "two", 100n, 0n, 0n, 10n, 2n, 0n, "iot", "", 1792100001n},
    M.TurnUsage{"t", "codex", "m", "unknown", 0n, 0n, 0n, 0n, 0n, 0n, "", "", 1792100002n}], M.State.empty())

# Explicit validation fixture; these are TurnUsage inputs, not live readings.
def state() -> M.State:
  M.State.replay([
    M.ProjectCreated{"p2", "Board", "/tmp/usage-two", 1n},
    M.ProjectCreated{"gone", "Old project", "/tmp/usage-gone", 1n},
    M.ThreadCreated{"t2", "p2", "Other work", M.Local{}, "", "", "codex", "m", 1n},
    M.ThreadCreated{"tg", "gone", "Old work", M.Local{}, "", "", "claude", "m", 1n},
    M.TurnUsage{"t", "claude", "another-model", "three", 100n, 0n, 0n, 200n, 150n, 999999999n, "ioct", "", 1792186400n},
    M.TurnUsage{"t2", "codex", "m", "four", 250n, 0n, 0n, 100n, 50n, 0n, "iot", "", 1792272800n},
    M.TurnUsage{"tg", "claude", "m", "five", 10n, 0n, 0n, 20n, 0n, 0n, "io", "", 1792272801n},
    M.TurnUsage{"missing-thread", "claude", "m", "six", 4n, 0n, 0n, 6n, 0n, 0n, "io", "", 1792272802n},
    M.TurnUsage{"t2", "codex", "m", "zero", 0n, 0n, 0n, 0n, 0n, 0n, "io", "", 1792359200n},
    M.ProjectRemoved{"gone", 1792359200n}], base())

def ui() -> U.Ui:
  U.Ui.info_set(U.Ui.info_set(U.Ui.adopt(U.Ui.init(1792359200n), state()), "models", "{\"claude\":[\"m\"],\"codex\":[\"m\"],\"grok\":[\"grok\"],\"muse\":[]}"), "machines", "{\"self\":{\"name\":\"Validation fixture\"}}")

def page(u: U.Ui) -> List<&2, D.Node>:
  W.Uv.page(I.En{}, u)

def days(+u: U.Ui) -> List<&2, G.UgDay>:
  G.Ug.days(U.Ui.state.of(u), U.Side.now(u), U.Usage.page.period(u), U.Usage.page.provider(u))

def projects(+u: U.Ui) -> List<&2, G.UgProject>:
  G.Ug.projects(U.Ui.state.of(u), U.Ui.projects(U.Ui.state.of(u)), U.Side.now(u), U.Usage.page.period(u))

def providers(+u: U.Ui) -> List<&2, String>:
  U.Usage.page.providers(u)

def selected(+u: U.Ui) -> String:
  U.Usage.page.provider(u)

def act(u: U.Ui, action: String, value: String) -> U.Ui:
  U.Act.ui(U.Ui.act(u, action, value))

def no_requests(+u: U.Ui) -> Nat:
  List.length(&2, U.Cmd, U.Act.cmds(U.Ui.act(u, "usage-provider", "codex")))

def caps(u: U.Ui, models: String) -> U.Ui:
  U.Ui.info_set(u, "models", models)

def epoch(u: U.Ui) -> U.Ui:
  U.Ui.adopt(u, M.State.setting(state(), "tok.epoch", "reset"))

def start(now: Nat, period: String) -> Nat:
  G.Ug.start(now, period)

def count(now: Nat, period: String) -> Nat:
  G.Ug.count(now, period)

def month_offset(now: Nat) -> Nat:
  G.Ug.month.offset(now)

def days_at(+now: Nat, +period: String) -> List<&2, G.UgDay>:
  G.Ug.days(M.State.empty(), now, period, "claude")

def reading(v: String) -> String:
  G.Ug.reading(v)

def legacy() -> M.State:
  M.State.setting(M.State.empty(), "tok.d.2026-10-15", "1/100/0/0/200/100/0/0/1")

def legacy_days() -> List<&2, G.UgDay>:
  G.Ug.days(legacy(), 1792100000n, "today", "claude")

def many_projects(+n: Nat, st: M.State) -> M.State:
  match n:
    case 0n: st
    case 1n+k: many_projects(k, M.State.setting(st, "tok.p.extra" ++ Nat.show(k) ++ "|2026-10", "1/" ++ Nat.show(k) ++ "/0/0/0/0/0/0/1"))

def top_many() -> List<&2, G.UgProject>:
  G.Ug.projects(many_projects(10n, state()), Nil{}, 1792359200n, "month")
