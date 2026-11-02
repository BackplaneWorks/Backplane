import Base
import ./check.bend as C
import ../src/core/allowance_stream.bend as AS
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/web/view.bend as W
import ../src/web/presentation.bend as P

def snapshot() -> String:
  "{\"provider\":\"codex\",\"at\":1793166000,\"state\":\"ok\",\"auth\":\"chatgpt\",\"source\":\"codex-app-server\",\"windows\":[{\"name\":\"codex/primary\",\"scope\":\"codex\",\"used_permille\":280,\"duration_minutes\":300,\"resets_at\":1793176000,\"shared\":true},{\"name\":\"codex/secondary\",\"scope\":\"codex\",\"used_permille\":610,\"duration_minutes\":10080,\"resets_at\":1793600000,\"shared\":true}]}"

def ui() -> U.Ui:
  U.Ui.info_set(U.Ui.select.k(U.Ui.adopt(U.Ui.init(1793166000n), M.State.replay([
    M.ProjectCreated{"p", "Allowance validation fixture", "/tmp/allowance-fixture", 1n},
    M.ThreadCreated{"t", "p", "Allowance bar", M.Local{}, "", "", "codex", "gpt-6.1-sol", 1n},
    M.SettingSet{"allowance.codex", snapshot(), 1793166000n}], M.State.empty())), "t"), "clock", "1793166000")

def act(u: U.Ui, action: String, value: String) -> U.Ui:
  U.Act.ui(U.Ui.act(u, action, value))

def commands(u: U.Ui, action: String, value: String) -> Nat:
  List.length(&2, U.Cmd, U.Act.cmds(U.Ui.act(u, action, value)))

def page(+u: U.Ui) -> String:
  J.Json.show(P.Node.write(W.View.composer.u(u)))

def model(+u: U.Ui, provider: String) -> U.Ui:
  U.Ui.adopt(u, M.State.replay([M.ProviderSet{"t", provider, "gpt-6.1-sol", 1793166000n}], U.Ui.state(u)))

def set(+u: U.Ui, text: String) -> U.Ui:
  U.Ui.adopt(u, M.State.replay([M.SettingSet{"allowance.codex", text, 1793166000n}], U.Ui.state(u)))

def tick(u: U.Ui, +now: Nat) -> U.Ui:
  U.Ui.info_set(U.Ui.tick(u, now), "clock", Nat.show(now))

def stream() -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse("{\"type\":\"rate_limit_event\",\"rate_limit_info\":{\"status\":\"allowed\",\"unifiedWindows\":{\"five_hour\":{\"utilization\":0.28,\"resetsAt\":1793176000},\"seven_day\":{\"utilization\":0.61,\"resetsAt\":1793600000}}}}"), J.Null{})

def main() -> IO(Unit):
  do IO<Unit>:
    +u : U.Ui = U.Ui.restore(ui(), "t", "draft 日本語 remains")
    +opened : U.Ui = act(u, "allowance-open", "")
    C.Check.str("opening preserves draft", U.Ui.draft_text(opened), "draft 日本語 remains")
    C.Check.str("opening retains selected thread", U.Ui.sel(opened), "t")
    C.Check.str("close restores closed state", Bool.show(U.Ui.flag(act(opened, "allowance-close", ""), "allowance")), "False")
    C.Check.str("Claude details issue no provider request", Nat.show(commands(model(u, "claude"), "allowance-read", "")), "0")
    C.Check.str("unknown provider details issue no request", Nat.show(commands(model(u, "grok"), "allowance-read", "")), "0")
    C.Check.str("Codex explicit refresh is one account read command", Nat.show(commands(u, "allowance-read", "")), "1")
    +s : M.State = M.State.replay(AS.AllowanceStream.changes(M.State.empty(), 1793166000n, stream()), M.State.empty())
    C.Check.str("passive stream persists allowance independently", Bool.show(String.contains(M.Setting.get(s, "allowance.claude", ""), "used_permille")), "True")
    C.Check.str("passive stream leaves token ledger alone", M.Setting.get(s, "tok.epoch", ""), "")
    C.Check.str("repeated stream is bounded for a minute", Nat.show(List.length(&2, M.Change, AS.AllowanceStream.changes(s, 1793166020n, stream()))), "0")
    C.Check.str("unchanged percentages renew timestamp", Nat.show(List.length(&2, M.Change, AS.AllowanceStream.changes(s, 1793166060n, stream()))), "1")
    C.Check.str("invalid negative utilization stays unknown", Nat.show(List.length(&2, J.Json, AS.AllowanceStream.window(J.Json.obj([J.KV{"utilization", J.Num{"-0.4"}}]), "5 h", "shared", 300n))), "1")
    C.Check.str("missing utilization stays unknown", Nat.show(List.length(&2, J.Json, AS.AllowanceStream.window(J.Null{}, "5 h", "shared", 300n))), "1")
    C.Check.str("production composer contains real allowance control", Bool.show(String.contains(page(u), "allowance-chip")), "True")
    C.Check.str("expanded production composer contains details", Bool.show(String.contains(page(opened), "Remaining allowance")), "True")
