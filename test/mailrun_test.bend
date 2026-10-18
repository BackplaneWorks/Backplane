import Base
import ./check.bend as C
import ../src/core/hub.bend as H
import ../src/core/model.bend as M
import ../src/core/mail.bend as ML
import ../src/core/mailbody.bend as MB
import ../src/core/mailrun.bend as R
import ../src/core/charter.bend as CH
import ../src/core/mcp.bend as MC
import ../src/core/bothub.bend as BH
import ../src/core/client.bend as UI
import ../src/core/json.bend as J
import ../src/core/codec.bend as CD
import ../src/core/notice.bend as NO

def base() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Project", "/tmp/mailrun", 1n},
    M.ThreadCreated{"worker", "p", "Worker", M.Local{}, "", "/tmp/mailrun", "claude", "", 1n},
    M.ThreadCreated{"foh", "", "Beta", M.Local{}, "", "/tmp/mailrun/beta", "claude", "opus", 1n},
    M.BotSet{"beta", "Beta", "foh", 0n, "", 1n}, M.SettingSet{"mail.on", "on", 1n},
    M.MemorySaved{"beta", "always", "pref", "Keep the blue board", "", 2n},
    M.DecisionLogged{"d", "beta", "conv", "old", "Use copper", "conducts", 2n}], M.State.empty())

def body(+want: String, +id: String) -> MB.Body:
  MB.Body{ML.Conv{"conv", "Board question", "pair", ["bot:beta", "thread:worker"], "p", Nil{}, 1n, 2n, 1n},
    ML.Mail{id, "conv", "thread:worker", "bot:beta", "thread", "", want, "hello", 5n, 10n}, "hello"}

def route(+s: M.State, +want: String, +id: String) -> H.Step:
  H.Mail.route_bot(s, 10n, body(want, id), "beta")

def runstate() -> M.State:
  M.State.replay(H.Queue.changes(route(base(), "act", "m1")), base())

def runid(+s: M.State) -> String:
  ML.Box.live(M.State.mail(s), "beta", "conv")

def posts(+cs: List<&2, M.Change>, +th: String) -> Nat:
  match cs:
    case Nil{}: 0n
    case M.MessagePosted{+t, _, _, _, _} <> +r: Nat.add(Bool.pick(Nat, String.eq(th, t), 1n, 0n), posts(r, th))
    case _ <> +r: posts(r, th)

def starts(+os: List<&2, H.Out>) -> String:
  match os:
    case Nil{}: "none"
    case H.AgentStart{_, _, _, _, +session, _, _} <> _: "start:" ++ session
    case _ <> +t: starts(t)

def head(+os: List<&2, H.Out>) -> String:
  match os:
    case Nil{}: ""
    case H.RunStart{_, _, h} <> _: h
    case _ <> +t: head(t)

def alarmcount(+cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}: 0n
    case M.ActivityLogged{_, _, _, "escalation", _, _} <> +t: 1n+alarmcount(t)
    case _ <> +t: alarmcount(t)

def seven() -> M.State:
  M.State.replay([M.RunSet{"r1", "beta", "c1", "m1", 1n}, M.RunSet{"r2", "beta", "c2", "m2", 1n}, M.RunSet{"r3", "beta", "c3", "m3", 1n}, M.RunSet{"r4", "beta", "c4", "m4", 1n}, M.RunSet{"r5", "beta", "c5", "m5", 1n}, M.RunSet{"r6", "beta", "c6", "m6", 1n}, M.RunSet{"r7", "beta", "c7", "m7", 1n}], base())

def command(+cmd: String) -> J.Json:
  J.Json.obj([J.KV{"command", J.Str{cmd}}])

def risky(+cmd: String) -> IO(Unit):
  do IO<Unit>:
    C.Check.str("risky " ++ cmd, Bool.show(CH.Charter.risky("Bash", command(cmd))), "True")
    C.Check.str("denied " ++ cmd, Bool.show(CH.Charter.gate(CH.Charter.default(), "Bash", command(cmd))), "False")
    C.Check.str("allowed listed " ++ cmd, Bool.show(CH.Charter.gate("may: tool:Bash", "Bash", command(cmd))), "True")

def change_lines(+cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case +h <> +t: J.Json.show(CD.Change.encode(h)) ++ "\n" ++ change_lines(t)

def main() -> IO(Unit):
  do IO<Unit>:
    +s : M.State = runstate()
    +th : String = runid(s)
    C.Check.str("FYI filed only", Nat.show(List.length(&2, M.Change, H.Queue.changes(route(base(), "fyi", "m1")))), "0")
    C.Check.str("FYI no output", Nat.show(List.length(&2, H.Out, H.Step.outs_of(route(base(), "fyi", "m1")))), "0")
    C.Check.str("act spawns one", Nat.show(ML.Box.live_n(M.State.mail(s), "beta")), "1")
    C.Check.str("FOH clean", Nat.show(posts(H.Queue.changes(route(base(), "act", "m1")), "foh")), "0")
    C.Check.str("spawn uses home", H.Hub.cwd(s, H.Hub.thread(s, th)), "/tmp/mailrun/beta")
    C.Check.str("default model", H.Run.model(s, "beta"), "sonnet")
    C.Check.str("uber model", H.Run.model(M.State.apply(s, M.SettingSet{"uber.bot", "beta", 2n}), "beta"), "opus")
    +start : H.Step = H.Run.start(s, 11n, th, head(H.Step.outs_of(route(base(), "act", "m1"))), [body("act", "m1")])
    C.Check.str("fresh first session", starts(H.Step.outs_of(start)), "start:")
    +live : M.State = M.State.replay(H.Queue.changes(start), s)
    +again : H.Step = route(live, "act", "m2")
    C.Check.str("second act delivered", Nat.show(posts(H.Queue.changes(again), th)), "1")
    C.Check.str("one live run", Nat.show(ML.Box.live_n(M.State.mail(M.State.replay(H.Queue.changes(again), live)), "beta")), "1")
    +ended : M.State = M.State.apply(live, M.TurnChanged{th, M.Completed{}, 12n})
    +fresh : M.State = M.State.replay(H.Queue.changes(route(ended, "act", "m3")), ended)
    C.Check.str("after end fresh run", Bool.show(Bool.not(String.eq(th, runid(fresh)))), "True")
    +brief : String = head(H.Step.outs_of(route(base(), "act", "m1")))
    C.Check.str("brief charter", Bool.show(String.contains(brief, "<charter>")), "True")
    C.Check.str("brief memory", Bool.show(String.contains(brief, "Keep the blue board")), "True")
    C.Check.str("brief decisions", Bool.show(String.contains(brief, "Use copper")), "True")
    C.Check.str("brief conversation", Bool.show(String.contains(brief, "conv")), "True")
    C.Check.str("run resolves bot", MC.Mcp.mail.scope(live, th), "beta")
    C.Check.str("run hidden", Nat.show(List.length(&2, M.Thread, UI.Ui.threads(live))), "2")
    +esc : H.Step = H.Run.escalate(live, 20n, "beta", "conv", "bot", "Choose colour")
    C.Check.str("escalation bot one row", Nat.show(alarmcount(H.Queue.changes(esc))), "1")
    C.Check.str("escalation bot one line", Nat.show(posts(H.Queue.changes(esc), "foh")), "1")
    +person : H.Step = H.Run.escalate(live, 20n, "beta", "conv", "person", "Approve order")
    +notified : M.State = M.State.replay(H.Queue.changes(person), live)
    C.Check.str("person has needs alert", NO.Alert.kind(NO.Alert.get(notified, "foh")), "ask")
    C.Check.str("person no delivered line", Nat.show(posts(H.Queue.changes(person), "foh")), "0")
    +twice : M.State = M.State.replay(H.Queue.changes(H.Run.escalate(notified, 21n, "beta", "conv", "person", "Approve again")), notified)
    C.Check.str("hour dedup", NO.Alert.rstamp(NO.Alert.get(twice, "foh")), NO.Alert.rstamp(NO.Alert.get(notified, "foh")))
    +alarms : List<&2, M.Change> = H.Run.alarm(seven(), 100n, "beta", "conv")
    C.Check.str("soft alarm seven", Nat.show(alarmcount(alarms)), "1")
    C.Check.str("soft alarm dedup", Nat.show(alarmcount(H.Run.alarm(M.State.replay(alarms, seven()), 101n, "beta", "conv"))), "0")
    C.Check.str("soft alarm next hour", Nat.show(alarmcount(H.Run.alarm(M.State.replay(alarms, seven()), 3700n, "beta", "conv"))), "1")
    C.Check.str("restart never carries run", Bool.show(H.Carry.will(live, th)), "False")
    C.Check.str("restart ends old run", Nat.show(ML.Box.live_n(M.State.mail(M.State.replay(H.Queue.changes(H.Run.recover(live, 20n, Nil{}, H.Step.none())), live)), "beta")), "0")
    C.Check.str("restart writes system mail", Nat.show(List.length(&2, H.Out, H.Step.outs_of(H.Run.recover(live, 20n, Nil{}, H.Step.none())))), "1")
    risky("cd /tmp; git push origin main && echo done")
    risky("cd /tmp & git push")
    risky("gh pr merge 1")
    risky("git push origin main")
    risky("cd /tmp/project && scripts/dev.sh install")
    risky("boat new")
    risky("rm -rf /tmp/foo")
    risky("git branch -D old")
    risky("gh issue delete 1")
    risky("thread delete x")
    risky("gh pr comment 1")
    risky("gh issue create --title hello")
    risky("curl https://example.com/hook")
    C.Check.str("email risky", Bool.show(CH.Charter.risky("gmail_send", J.Null{})), "True")
    C.Check.str("deploy risky", Bool.show(CH.Charter.risky("deploy_restart", J.Null{})), "True")
    C.Check.str("purchase risky", Bool.show(CH.Charter.risky("store_order", J.Null{})), "True")
    C.Check.str("prefix allowed", Bool.show(CH.Charter.allows("may: bash:git push", "Bash", command("cd /tmp && git push origin main"))), "True")
    C.Check.str("prefix boundary", Bool.show(CH.Charter.allows("may: bash:git push", "Bash", command("git pushy"))), "False")
    C.Check.str("prefix cannot grant tail", Bool.show(CH.Charter.allows("may: bash:git push", "Bash", command("git push; rm -rf /tmp/x"))), "False")
    C.Check.str("ordinary tools allowed", Bool.show(CH.Charter.gate(CH.Charter.default(), "Read", J.Null{})), "True")
    +denied : MC.McpOut = MC.Mcp.permit(live, 30n, th, J.Num{"1"}, J.Json.obj([J.KV{"tool_name", J.Str{"Bash"}}, J.KV{"input", command("git push origin main")}]))
    C.Check.str("permission denied and escalated", Bool.show(String.contains(MC.McpOut.reply(denied), "Outside the charter; escalated")), "True")
    +allowed : M.State = M.State.apply(live, M.CharterSet{"beta", "may: bash:git push", "bot", 31n})
    +permitted : MC.McpOut = MC.Mcp.permit(allowed, 32n, th, J.Num{"2"}, J.Json.obj([J.KV{"tool_name", J.Str{"Bash"}}, J.KV{"input", command("git push origin main")}]))
    C.Check.str("permission allowed by charter", Bool.show(String.contains(MC.McpOut.reply(permitted), "allow")), "True")
    C.Check.str("decide rejects worker", Bool.show(MC.Res.good(MC.Mcp.run.call(True{}, 41n, live, 30n, "worker", J.Null{}))), "False")
    C.Check.str("charter rejects run", Bool.show(MC.Res.good(MC.Mcp.run.call(True{}, 43n, live, 30n, th, J.Null{}))), "False")
    C.Check.str("charter accepts FOH", Bool.show(MC.Res.good(MC.Mcp.run.call(True{}, 43n, live, 30n, "foh", J.Json.obj([J.KV{"text", J.Str{"may: tool:Read"}}, J.KV{"reason", J.Str{"test"}}])))), "True")
    +reply : J.Json = MC.Mcp.run.mail.got(ML.Box.run_of(M.State.mail(live), th), live, J.Json.obj([J.KV{"text", J.Str{"Done"}}]))
    C.Check.str("run defaults conversation", J.Json.get_str(reply, "conv", ""), "conv")
    +off : M.State = M.State.apply(base(), M.SettingSet{"mail.on", "off", 30n})
    +old : H.Step = H.Origin.step(H.Hub.inbox.id(off, 31n, "worker", "foh", M.User{}, "old", "t:worker:m"), "foh", "t:worker:m", ML.Origin{"thread:worker", "thread", ""}, 31n)
    +offstep : H.Step = H.Hub.inbox.from(off, 31n, "worker", "foh", M.User{}, "old", "t:worker:m")
    +offrun : M.State = M.State.apply(live, M.SettingSet{"mail.on", "off", 30n})
    C.Check.str("off restores ordinary run-thread identity", Bool.show(R.Run.is(offrun, th)), "False")
    C.Check.str("off restores unfiltered lists", Nat.show(List.length(&2, M.Thread, UI.Ui.threads(offrun))), "3")
    C.Check.str("stopped run cannot start delayed brief", Bool.show(H.Run.ready(M.State.apply(s, M.TurnChanged{th, M.Interrupted{}, 31n}), th)), "False")
    C.Check.str("listed gmail skips duplicate approval", Bool.show(MC.Perm.run.google(M.State.apply(live, M.CharterSet{"beta", "may: tool:gmail_send", "bot", 30n}), th, "gmail_send")), "True")
    C.Check.str("unlisted gmail still blocked", Bool.show(MC.Perm.run.google(live, th, "gmail_send")), "False")
    C.Check.str("mail off changes identical", change_lines(H.Queue.changes(offstep)), change_lines(H.Queue.changes(old)))
    C.Check.str("mail off agent output identical", starts(H.Step.outs_of(offstep)), starts(H.Step.outs_of(old)))
