import Base
import ./check.bend as C
import ../src/core/accounts.bend as A
import ../src/core/model.bend as M
import ../src/core/json.bend as J

def state() -> M.State:
  M.State.replay([M.ThreadCreated{"t", "p", "Thread", M.Local{}, "", "", "codex", "model", 1n}, M.SessionSet{"t", "legacy-session", 1n}], M.State.empty())

def switched(+s: M.State, id: String, epoch: Nat, session: String, count: Nat) -> M.State:
  M.State.replay(A.Accounts.select(s, 2n, "t", "codex", id, epoch, session, count), s)

def cycle() -> M.State:
  +a = switched(state(), "A", 2n, "legacy-session", 4n)
  +b = switched(a, "B", 2n, "session-A", 5n)
  switched(b, "A", 2n, "session-B", 7n)

def request(+m: String, +thread: String) -> J.Json:
  J.Json.obj([J.KV{"m", J.Str{m}}, J.KV{"p", J.Json.obj([J.KV{"thread", J.Str{thread}}])}])

def deleting_default(+snapshot: String) -> M.State:
  +before = M.State.replay([
    M.SettingSet{"accounts.inventory", "{\"profiles\":[{\"id\":\"A\",\"provider\":\"codex\",\"epoch\":2,\"state\":\"connected\"},{\"id\":\"B\",\"provider\":\"codex\"}]}", 8n},
    M.SettingSet{"account.default.codex", A.Accounts.binding.show("A", 2n), 8n}], cycle())
  M.State.replay(A.Accounts.changes(before, 9n, snapshot), before)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("missing account has a clear selection reason", A.Accounts.unavailable(J.Null{}, 1n), "This saved account is no longer available. Choose another account.")
    C.Check.str("unchecked sign-in asks for refresh rather than authentication", A.Accounts.unavailable(A.Accounts.parse("{\"id\":\"A\",\"state\":\"unchecked\"}"), 1n), "This saved sign-in has not been checked yet. Reopen the account menu to refresh it.")
    C.Check.str("disconnected account has an authentication next step", A.Accounts.unavailable(A.Accounts.parse("{\"id\":\"A\",\"state\":\"disconnected\"}"), 1n), "This account is not connected. Use Status / Refresh or Connect / Login in Settings → Agents.")
    C.Check.str("replaced sign-in asks for a fresh selection", A.Accounts.unavailable(A.Accounts.parse("{\"id\":\"A\",\"state\":\"connected\",\"epoch\":2}"), 1n), "This account's sign-in changed. Reopen the account menu and select it again.")
    C.Check.str("deleting default clears it for new threads", M.Setting.get(deleting_default("{\"profiles\":[{\"id\":\"B\",\"provider\":\"codex\"}]}"), "account.default.codex", ""), A.Accounts.binding.show("", 0n))
    C.Check.str("deleting account keeps old thread explicitly bound", A.Accounts.id(deleting_default("{\"profiles\":[{\"id\":\"B\",\"provider\":\"codex\"}]}"), "t", "codex"), "A")
    C.Check.str("deleted account never falls back to default on an old thread", Bool.show(A.Accounts.allowed(deleting_default("{\"profiles\":[{\"id\":\"B\",\"provider\":\"codex\"}]}"), "t", "codex")), "False")
    C.Check.str("registry read failure does not clear account default", M.Setting.get(deleting_default("{\"profiles\":[],\"error\":\"unavailable\"}"), "account.default.codex", ""), A.Accounts.binding.show("A", 2n))
    C.Check.str("old unset binding is legacy", A.Accounts.id(state(), "t", "codex"), "")
    C.Check.str("legacy session saved on first selection", A.Accounts.session(switched(state(), "A", 2n, "legacy-session", 4n), "t", "codex", "", 0n), "legacy-session")
    C.Check.str("A to B to A resumes A", M.Switch.sess(M.State.thread.get(cycle(), "t")), "session-A")
    C.Check.str("B session remains separately owned", A.Accounts.session(cycle(), "t", "codex", "B", 2n), "session-B")
    C.Check.str("new identity cannot resume old A", M.Switch.sess(M.State.thread.get(switched(cycle(), "A", 3n, "session-A", 8n), "t")), "")
    C.Check.str("another thread cannot resume A", A.Accounts.session(cycle(), "u", "codex", "A", 2n), "")
    C.Check.str("provider tuple cannot alias", A.Accounts.session(cycle(), "t", "claude", "A", 2n), "")
    C.Check.str("metadata drops login ceremonies", A.Accounts.public(A.Accounts.parse("{\"profiles\":[],\"attempts\":[{\"code\":\"private\"}]}")), "{\"version\":1,\"revision\":0,\"profiles\":[],\"error\":\"\"}")
    C.Check.str("invalid binding fails closed", A.Accounts.id(M.State.replay([M.SettingSet{A.Accounts.binding.key("t", "codex"), "broken", 2n}], state()), "t", "codex"), "!invalid")
    C.Check.str("old native cannot send named turn", Bool.show(A.Accounts.client.allowed(False{}, cycle(), request("turn.start", "t"), "")), "False")
    C.Check.str("old native legacy send unchanged", Bool.show(A.Accounts.client.allowed(False{}, state(), request("turn.start", "t"), "")), "True")
    C.Check.str("named account client still enters canonical executor", Bool.show(A.Accounts.client.allowed(True{}, cycle(), request("turn.start", "t"), "")), "True")
    C.Check.str("old native can interrupt named turn", Bool.show(A.Accounts.client.allowed(False{}, cycle(), request("turn.interrupt", "t"), "")), "True")
    C.Check.str("global account status has no selected conversation owner", Bool.show(A.Accounts.request.current(cycle(), A.Accounts.parse("{\"m\":\"accounts\",\"p\":{\"op\":\"status\",\"profile\":\"A\"}}"), "t")), "True")
    C.Check.str("global secret names are not fenced by selected account", Bool.show(A.Accounts.request.current(cycle(), A.Accounts.parse("{\"m\":\"secret.list\",\"p\":{}}"), "t")), "True")
    C.Check.str("global provider status remains available", Bool.show(A.Accounts.request.current(cycle(), A.Accounts.parse("{\"m\":\"bots.voice\",\"p\":{\"op\":\"status\"}}"), "t")), "True")
    C.Check.str("explicit named turn still needs account generation", Bool.show(A.Accounts.request.current(cycle(), request("turn.start", "t"), "")), "False")
    C.Check.str("view action still inherits named target owner", Bool.show(A.Accounts.request.current(cycle(), A.Accounts.parse("{\"m\":\"view.action\",\"p\":{\"action\":\"send\"}}"), "t")), "False")
    C.Check.str("view action cannot override actual selected owner", Bool.show(A.Accounts.request.current(cycle(), request("view.action", ""), "t")), "False")
    C.Check.str("current view action carries selected generation", Bool.show(A.Accounts.request.current(cycle(), A.Accounts.parse("{\"m\":\"view.action\",\"p\":{\"action\":\"send\",\"accountBinding\":\"{\\\"id\\\":\\\"A\\\",\\\"epoch\\\":2}\"}}"), "t")), "True")
    return Unit{}
