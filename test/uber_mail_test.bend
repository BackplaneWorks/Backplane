import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/mcp.bend as MC
import ../src/core/maildisposition.bend as D
import ../src/core/mail.bend as ML
import ../src/core/mailbody.bend as MB
import ../src/core/mailrun.bend as R
import ../src/core/json.bend as J

# Mail to the promoted uberbot with automation (mail.on) off: act mail
# wakes it in a private run, carrying the text; fyi is only filed; nothing
# is posted in its person-facing chat (t0); no other bot is woken.

# t0 is Bub's chat, t1 a worker thread; uber: Bub promoted; on: mail.on on
def base(+uber: Bool, +on: Bool) -> M.State:
  +s = {M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t0", "p1", "Bub", M.Local{}, "", "/p", "claude", "", 1n},
    M.ThreadCreated{"t1", "p1", "worker", M.Local{}, "", "/p", "claude", "", 1n}, M.ThreadCreated{"t2", "p1", "Other", M.Local{}, "", "/p", "claude", "", 1n},
    M.BotSet{"bu", "Bub", "t0", 0n, "", 1n}, M.BotSet{"ot", "Other", "t2", 0n, "", 1n}], M.State.empty()) : M.State}
  +s1 = {Bool.pick(M.State, uber, M.State.apply(s, M.SettingSet{"uber.bot", "bu", 1n}), s) : M.State}
  Bool.pick(M.State, on, M.State.apply(s1, M.SettingSet{"mail.on", "on", 1n}), s1)

def text() -> String:
  "gate 4 decision needed: ship the JS hub today?"

def appended(os: List<&2, H.Out>) -> List<&2, M.Change>:
  match os:
    case Nil{}: Nil{}
    case H.MailAppend{_, _, cs} <> _: cs
    case _ <> rest: appended(rest)

def body(os: List<&2, H.Out>) -> Maybe<&2, MB.Body>:
  match os:
    case Nil{}: None{}
    case H.MailAppend{line, _, _} <> _: MB.Body.parse(line)
    case _ <> rest: body(rest)

def bodies(b: Maybe<&2, MB.Body>) -> List<&2, MB.Body>:
  match b:
    case None{}: Nil{}
    case Some{+b}: [b]

def outs(st: H.Step) -> List<&2, H.Out>:
  match st:
    case H.Step{_, os}: os

def changes(st: H.Step) -> List<&2, M.Change>:
  match st:
    case H.Step{cs, _}: cs

# mail from t1 to a thread (a bot's chat), want "" (a thread's own: act) or "fyi"
def filed(+s: M.State, +to: String, +want: String) -> H.Step:
  H.Operational.mail(s, 50n, ML.Origin{H.Mail.addr(s, "t1"), "thread", ""}, to, text(), "t:t1:m50", want, Nil{})

# what routing the filed mail does, after its header commits
def routed.k(b: Maybe<&2, MB.Body>, +s: M.State) -> H.Step:
  match b:
    case None{}: H.Step{[M.SettingSet{"no-body", "x", 0n}], Nil{}}
    case Some{+b}: H.Mail.route(s, 51n, b)

def routed(+s: M.State, +to: String, +want: String) -> H.Step:
  +f = filed(s, to, want)
  routed.k(body(outs(f)), M.State.replay(appended(outs(f)), s))

def runs(os: List<&2, H.Out>) -> Nat:
  match os:
    case Nil{}: 0n
    case H.RunStart{_, _, _} <> +t: 1n+runs(t)
    case _ <> +t: runs(t)

def posts(cs: List<&2, M.Change>, +th: String) -> Nat:
  match cs:
    case Nil{}: 0n
    case M.MessagePosted{+t, _, _, _, _} <> +rest: Nat.add(Bool.pick(Nat, String.eq(t, th), 1n, 0n), posts(rest, th))
    case _ <> +rest: posts(rest, th)

def run.bot(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.RunSet{_, +bot, _, _, _} <> _: bot
    case _ <> +rest: run.bot(rest)

def run.thread(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.RunSet{+th, _, _, _, _} <> _: th
    case _ <> +rest: run.thread(rest)

def res.ok(r: MC.Res) -> Bool:
  match r:
    case MC.Res{+ok, _, _}: ok

def changes.text.go(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.MessagePosted{_, _, _, +t, _} <> +rest: t ++ changes.text.go(rest)
    case _ <> +rest: changes.text.go(rest)

def changes.text(st: H.Step) -> String:
  changes.text.go(changes(st))

def acts(cs: List<&2, M.Change>, +th: String) -> Nat:
  match cs:
    case Nil{}: 0n
    case M.ActivityLogged{+t, _, _, _, _, _} <> +rest: Nat.add(Bool.pick(Nat, String.eq(t, th), 1n, 0n), acts(rest, th))
    case _ <> +rest: acts(rest, th)

# the run's turn ends with no reply, deferral or escalation
def ended(+s: M.State, +th: String) -> H.Step:
  H.Run.after(s, 60n, H.Step{[M.TurnChanged{th, M.Completed{}, 60n}], Nil{}})

def tool.names(ts: List<&2, J.Json>) -> String:
  match ts:
    case Nil{}: ""
    case +t <> +rest: J.Json.get_str(t, "name", "") ++ " " ++ tool.names(rest)

def tools(+s: M.State, +th: String) -> String:
  tool.names(J.Json.elems(J.Json.get_obj(MC.Mcp.tools.for(s, th), "tools")))

def has(+names: String, +name: String) -> String:
  Bool.show(String.contains(names, " " ++ name ++ " "))

def main() -> IO(Unit):
  do IO<Unit>:
    +s : M.State = base(True{}, False{})
    C.Check.str("the uberbot's runs are on with mail.on off", Bool.show(D.Runs.on(s, "bu")), "True")
    C.Check.str("another bot's are not", Bool.show(D.Runs.on(s, "ot")), "False")
    C.Check.str("with mail.on every bot's are", Bool.show(D.Runs.on(base(True{}, True{}), "ot")), "True")
    C.Check.str("no uberbot: none", Bool.show(D.Runs.on(base(False{}, False{}), "bu")), "False")
    # act: a private run carrying the text, nothing in the chat
    +act : H.Step = routed(s, "t0", "")
    C.Check.str("act mail to the uberbot starts one private run", Nat.show(runs(outs(act))), "1")
    C.Check.str("the run is the uberbot's", run.bot(changes(act)), "bu")
    C.Check.str("filing and routing post nothing in its chat", Nat.show(Nat.add(posts(appended(outs(filed(s, "t0", ""))), "t0"), posts(changes(act), "t0"))), "0")
    +s2 : M.State = M.State.replay(changes(act), M.State.replay(appended(outs(filed(s, "t0", ""))), s))
    C.Check.str("the run thread is a run here", Bool.show(R.Run.is(s2, run.thread(changes(act)))), "True")
    C.Check.str("its brief carries the full text", Bool.show(String.contains(changes.text(H.Run.start(s2, 52n, run.thread(changes(act)), "head\n", bodies(body(outs(filed(s, "t0", "")))))), text())), "True")
    # an unresolved run with mail.on off: an act in the run's thread, nothing in the chat
    +run : String = run.thread(changes(act))
    +quiet : H.Step = ended(s2, run)
    C.Check.str("an unresolved uberbot run posts nothing in its chat", Nat.show(posts(changes(quiet), "t0")), "0")
    C.Check.str("it is logged as an act in the run", Nat.show(acts(changes(quiet), run)), "1")
    C.Check.str("and as no act in the chat", Nat.show(acts(changes(quiet), "t0")), "0")
    +loud : H.Step = ended(M.State.apply(s2, M.SettingSet{"mail.on", "on", 59n}), run)
    C.Check.str("with mail.on on, today's escalation into the chat stays", Nat.show(posts(changes(loud), "t0")), "1")
    # fyi: filed only
    +fyi : H.Step = routed(s, "t0", "fyi")
    C.Check.str("fyi mail starts no run", Nat.show(runs(outs(fyi))), "0")
    C.Check.str("fyi mail changes nothing", Nat.show(List.length(&2, M.Change, changes(fyi))), "0")
    # no one else is woken
    C.Check.str("act mail to another bot with mail.on off: no run", Nat.show(runs(outs(routed(s, "t2", "")))), "0")
    C.Check.str("no uberbot promoted: no run", Nat.show(runs(outs(routed(base(False{}, False{}), "t0", "")))), "0")
    # tools: mail_list and mail_read for the uberbot only
    C.Check.str("the uberbot's chat lists mail_list", has(" " ++ tools(s, "t0"), "mail_list"), "True")
    C.Check.str("the uberbot's chat lists mail_read", has(" " ++ tools(s, "t0"), "mail_read"), "True")
    C.Check.str("but not mail (send) with mail.on off", has(" " ++ tools(s, "t0"), "mail"), "False")
    C.Check.str("its run lists escalate", has(" " ++ tools(s2, run.thread(changes(act))), "escalate"), "True")
    C.Check.str("another thread lists no mail tools", has(" " ++ tools(s, "t1"), "mail_list"), "False")
    C.Check.str("another bot lists no mail tools", has(" " ++ tools(s, "t2"), "mail_read"), "False")
    C.Check.str("mail_list answers the uberbot", Bool.show(res.ok(MC.Mcp.call.k(35n, s2, 53n, "t0", J.Null{}, J.Json.obj(Nil{})))), "True")
    C.Check.str("mail_list refuses another bot", Bool.show(res.ok(MC.Mcp.call.k(35n, s2, 53n, "t2", J.Null{}, J.Json.obj(Nil{})))), "False")
