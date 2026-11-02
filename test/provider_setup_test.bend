import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/accounts.bend as A
import ../src/core/provider-setup.bend as P
import ../src/mobile/screen.bend as S

# Real UI actions and shared phone projection, without credentials or CLI IO.
def fresh() -> U.Ui:
  +s = M.State.replay([M.ProjectCreated{"p", "Project", "/fixture", 1n}, M.ThreadCreated{"t", "p", "Thread", M.Local{}, "", "", "codex", "", 2n}, M.SessionSet{"t", "keep-session", 3n}], M.State.empty())
  U.Ui.info_set(U.Ui.scratch_set(U.Ui.adopt.raw(U.Ui.init(100n), s), "@sync", "1"), "models", "{\"about\":{\"codex\":{\"cli\":\"codex fixture\"},\"claude\":{\"cli\":\"claude fixture\"}}}")

def act(u: U.Ui, a: String, v: String) -> U.Ui:
  U.Act.ui(U.Ui.act(u, a, v))

def opened() -> U.Ui:
  act(fresh(), "provider-setup-open", "")

def profile(+state: String) -> J.Json:
  J.Json.obj([J.KV{"id", J.Str{"a"}}, J.KV{"provider", J.Str{"codex"}}, J.KV{"label", J.Str{"Personal"}}, J.KV{"state", J.Str{state}}, J.KV{"epoch", J.Json.nat(2n)}])

def account(+state: String) -> U.Ui:
  +u = opened()
  U.Ui.adopt.raw(u, M.State.setting(U.Ui.state(u), "accounts.inventory", J.Json.show(J.Json.obj([J.KV{"profiles", J.Json.arr([profile(state)])}]))))

def sent(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}: ""
    case U.Send{s} <> rest: s ++ "|" ++ sent(rest)
    case _ <> rest: sent(rest)

def count(cs: List<&2, U.Cmd>) -> Nat:
  match cs:
    case Nil{}: 0n
    case U.Send{_} <> rest: 1n+count(rest)
    case _ <> rest: count(rest)

def notes(rs: List<&2, U.AgRow>) -> String:
  match rs:
    case Nil{}: ""
    case U.AgRow{l, n, _, _} <> rest: l ++ " " ++ n ++ "|" ++ notes(rest)

def btns(bs: List<&2, U.AgBtn>) -> String:
  match bs:
    case Nil{}: ""
    case U.AgBtn{l, a, v, _} <> rest: l ++ ":" ++ a ++ ":" ++ v ++ "|" ++ btns(rest)

def buttons(rs: List<&2, U.AgRow>) -> String:
  match rs:
    case Nil{}: ""
    case U.AgRow{_, _, _, bs} <> rest: btns(bs) ++ buttons(rest)

def has(+s: String, +p: String) -> String:
  Bool.show(String.contains(s, p))

def waiting(+state: String) -> U.Ui:
  U.Ui.info_set(account("connecting"), "accounts.live", "{\"attempts\":[{\"id\":\"attempt-a\",\"profile\":\"a\",\"state\":\"" ++ state ++ "\",\"url\":\"https://example.test/login\",\"userCode\":\"DEVICE-ONLY\"}]}")

def main() -> IO(Unit):
  +used = U.Ui.act(account("connected"), "provider-setup-use", "")
  +draft = act(U.Ui.select.k(account("connected"), "t"), "draft", "Keep 日本語 draft")
  do IO<Unit>:
    C.Check.str("installed alone is not authenticated", Bool.show(P.Ps.ready(True{}, profile("unchecked"))), "False")
    C.Check.str("removed CLI is not usable", Bool.show(P.Ps.ready(False{}, profile("connected"))), "False")
    C.Check.str("verified installed account is usable", Bool.show(P.Ps.ready(True{}, profile("connected"))), "True")
    C.Check.str("fresh Next introduces setup", Bool.show(U.Psv.active(act(fresh(), "ob-next", ""))), "True")
    C.Check.str("opening setup pauses tour", Bool.show(U.Tour.on(opened())), "False")
    C.Check.str("opening setup preserves tour position", Nat.show(U.Tour.at(opened())), "0")
    C.Check.str("opening never starts login", Nat.show(count(U.Act.cmds(U.Ui.act(fresh(), "provider-setup-open", "")))), "0")
    C.Check.str("setup opens Agents settings", U.Settings.page(opened()), "agents")
    C.Check.str("return resumes tour", Bool.show(U.Tour.on(act(opened(), "provider-setup-close", ""))), "True")
    C.Check.str("skip does not trap Next", Nat.show(U.Tour.at(act(act(opened(), "provider-setup-close", ""), "ob-next", ""))), "1")
    C.Check.str("program absence has installation next step", has(notes(U.Psv.rows(1n, U.Ui.info_set(opened(), "models", "{}"))), "Program not found yet"), "True")
    C.Check.str("native phone hands account forms to browser", has(notes(U.Psv.rows(2n, opened())), "Connect in your browser"), "True")
    C.Check.str("native never starts unsupported account ceremony", has(buttons(U.Psv.rows(2n, account("disconnected"))), "acct-login"), "False")
    C.Check.str("web has account creation", has(buttons(U.Psv.rows(1n, opened())), "acct-add"), "True")
    C.Check.str("provider choice has no settings mutation", Nat.show(count(U.Act.cmds(U.Ui.act(opened(), "provider-setup-pick", "claude")))), "0")
    C.Check.str("Claude Add uses actual provider", has(sent(U.Act.cmds(U.Ui.act(U.Ui.scratch_set(act(opened(), "provider-setup-pick", "claude"), "@f.accountlabel", "Personal"), "acct-add", ""))), "\"provider\":\"claude\""), "True")
    C.Check.str("CLI provider choice clears old provider model", M.Setting.get(U.Ui.state(act(act(U.Ui.adopt.raw(opened(), M.State.setting(U.Ui.state(opened()), "agent.model", "old-model")), "provider-setup-pick", "grok"), "provider-setup-use-cli", "")), "agent.model", "unchanged"), "")
    C.Check.str("CLI provider choice starts no login or model turn", Nat.show(count(U.Act.cmds(U.Ui.act(act(opened(), "provider-setup-pick", "grok"), "provider-setup-use-cli", "")))), "2")
    C.Check.str("native handoff uses existing hub link", has(buttons(U.Psv.rows(2n, U.Ui.info_set(opened(), "pair", "https://hub.example.test/#token=fixture"))), "open-url:https://hub.example.test/#token=fixture"), "True")
    C.Check.str("waiting exposes provider code", has(notes(U.Psv.rows(1n, waiting("waiting"))), "DEVICE-ONLY"), "True")
    C.Check.str("waiting has cancel", has(buttons(U.Psv.rows(1n, waiting("waiting"))), "acct-cancel:a:attempt-a"), "True")
    C.Check.str("waiting suppresses competing login", has(buttons(U.Psv.rows(1n, waiting("waiting"))), "acct-login"), "False")
    C.Check.str("failed ceremony offers retry", has(buttons(U.Psv.rows(1n, waiting("failed"))), "Sign in / Try again"), "True")
    C.Check.str("expired ceremony offers retry", has(buttons(U.Psv.rows(1n, waiting("expired"))), "Sign in / Try again"), "True")
    C.Check.str("failed ceremony has no stale provider link", has(buttons(U.Psv.rows(1n, waiting("failed"))), "Open provider sign-in"), "False")
    C.Check.str("success updates next action", has(notes(U.Psv.rows(1n, account("connected"))), "Connection verified"), "True")
    C.Check.str("unchecked account cannot change defaults", Nat.show(count(U.Act.cmds(U.Ui.act(account("unchecked"), "provider-setup-use", "")))), "0")
    C.Check.str("explicit use sends three separate requests", Nat.show(count(U.Act.cmds(used))), "3")
    C.Check.str("use chooses new thread default account generation", has(sent(U.Act.cmds(used)), "\"profile\":\"a\",\"epoch\":2"), "True")
    C.Check.str("use preserves existing session", M.Switch.sess(U.Ui.thread(U.Ui.state(U.Act.ui(used)), "t")), "keep-session")
    C.Check.str("use preserves existing account binding", A.Accounts.id(U.Ui.state(U.Act.ui(used)), "t", "codex"), "")
    C.Check.str("setup and return preserve draft", U.Ui.draft_text(act(act(draft, "provider-setup-use", ""), "provider-setup-close", "")), "Keep 日本語 draft")
    C.Check.str("phone projection has setup controls", has(J.Json.show(S.Mob.screen(opened())), "provider-setup-pick"), "True")
    C.Check.str("finished tour never auto-requests setup", Bool.show(U.Psv.due(U.Ui.adopt.raw(fresh(), M.State.setting(U.Ui.state(fresh()), "onboard.done", "1")))), "False")
    C.Check.str("verified account bypasses first-run setup", Bool.show(U.Psv.due(account("connected"))), "False")
    C.Check.str("sealed sign-in response targets existing account flow", U.Sealed.agy(opened(), "acct-response:a:attempt-a"), "account:a:attempt-a")
    C.Check.str("setup starts on the agent this machine has", U.Psv.provider(U.Ui.info_set(opened(), "models", "{\"about\":{\"claude\":{\"cli\":\"2.1.286 (Claude Code)\"}}}")), "claude")
    C.Check.str("or the one new threads use, when installed", U.Psv.provider(U.Ui.adopt.raw(opened(), M.State.replay([M.SettingSet{"agent.provider", "claude", 4n}], U.Ui.state(opened())))), "claude")
    C.Check.str("with none installed, Codex as before", U.Psv.provider(U.Ui.info_set(opened(), "models", "{}")), "codex")
    C.Check.str("a choice made stays", U.Psv.provider(act(U.Ui.info_set(opened(), "models", "{\"about\":{\"claude\":{\"cli\":\"c\"}}}"), "provider-setup-pick", "grok")), "grok")
    return Unit{}
