import Base
import ../src/core/i18n.bend as I
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/botui.bend as BU
import ../src/core/task.bend as TK

# Teaching a bot browser tasks, as a client shows it (client.bend's Teach.*):
# the Browser strip for each of the hub's drive/teach infos, input only while
# the person drives, the save sheet's chips (a secret never one), the Tasks
# tab and the stuck card.

def now() -> Nat:
  1000n

def task() -> String:
  "{\"v\":1,\"slug\":\"jlcpcb\",\"name\":\"Check order\",\"description\":\"Look up an order\",\"origin\":\"show\",\"start\":\"https://jlcpcb.com\",\"params\":[{\"name\":\"order\",\"label\":\"Order number\",\"kind\":\"text\",\"example\":\"\"}],\"steps\":[],\"urls\":[],\"created\":1,\"updated\":2}"

def run(+status: String) -> String:
  "{\"slug\":\"jlcpcb\",\"run\":\"r1\",\"status\":\"" ++ status ++ "\",\"n\":3,\"summary\":\"done\",\"shots\":[\"/bots/b1/tasks/shot?run=r1&n=2\"]}"

def draft() -> String:
  "[{\"n\":1,\"do\":\"type\",\"target\":{\"role\":\"textbox\",\"label\":\"Order number\"},\"value\":\"A123\"},{\"n\":2,\"do\":\"type\",\"target\":{\"label\":\"Password\"},\"value\":{\"secret\":\"Password\"}},{\"n\":3,\"do\":\"click\",\"target\":{\"role\":\"button\",\"name\":\"Search\"},\"value\":\"\"}]"

def detail(+risky: String) -> String:
  "{\"bot\":\"b1\",\"run\":\"r1\",\"slug\":\"jlcpcb\",\"n\":3,\"kind\":\"login\",\"question\":\"Sign in?\",\"risky\":" ++ risky ++ ",\"shot\":\"/bots/b1/tasks/shot?run=r1&n=3\"}"

def log(+opts: String, +risky: String) -> List<&2, M.Change>:
  [M.ThreadCreated{"bt1", "", "Kit", M.Local{}, "", "", "claude", "", 100n}, M.BotSet{"b1", "Kit", "bt1", 7n, "helps", 100n},
   M.TaskSet{"b1", "jlcpcb", task(), 200n},
   M.ActivityLogged{"bt1", "r1:a", "task", "run", run("running"), 300n},
   M.ActivityLogged{"bt1", "r1:b", "task", "run", run("ok"), 310n},
   M.AskOpened{"k1", "bt1", "browser", "task_pause", detail(risky), opts, 400n}]

def base(+opts: String, +risky: String) -> U.Ui:
  U.Ui.scratch_set(U.Ui.fold(log(opts, risky), U.Ui.init(now())), "@bot", "b1")

def plain() -> U.Ui:
  base("", "false")

def with(+u: U.Ui, +drive: String, +teach: String) -> U.Ui:
  U.Ui.info_set(U.Ui.info_set(u, "drive.b1", drive), "teach.b1", teach)

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def sent.go(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{t} <> _:
      t
    case _ <> r:
      sent.go(r)

def sent(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      sent.go(cs)

def act(+u: U.Ui, +a: String, +v: String) -> U.Ui:
  ui(U.Ui.act(u, a, v))

def labels(bs: List<&2, U.TBtn>) -> String:
  match bs:
    case Nil{}:
      ""
    case U.TBtn{+l, _, _, _} <> t:
      l ++ "," ++ labels(t)

def strip.k(s: U.TStrip) -> String:
  match s:
    case U.TStrip{+m, +t, +c, +n, bs}:
      Nat.show(m) ++ "|" ++ t ++ "|" ++ c ++ "|" ++ Nat.show(n) ++ "|" ++ labels(bs)

def strip(+u: U.Ui) -> String:
  strip.k(U.Teach.strip(u, "b1"))

def chips.go(rs: List<&2, U.TStepRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.TStepRow{+n, _, _, +c, _, _, _} <> t:
      Nat.show(n) ++ "=" ++ c ++ ";" ++ chips.go(t)

def chips(+u: U.Ui) -> String:
  chips.go(U.Teach.chips.go(U.Teach.rows(u, "b1")))

def rows.go(rs: List<&2, U.TStepRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.TStepRow{_, +t, _, _, +s, _, _} <> r:
      t ++ "[" ++ Bool.show(s) ++ "];" ++ rows.go(r)

def tasks.go(ts: List<&2, U.TTask>) -> String:
  match ts:
    case Nil{}:
      ""
    case U.TTask{+s, +n, +d, ps, +o, +a, +l} <> t:
      s ++ "|" ++ n ++ "|" ++ d ++ "|" ++ Nat.show(List.length(&2, TK.TkParam, ps)) ++ "|" ++ Bool.show(o) ++ "|" ++ Bool.show(a) ++ "|" ++ l ++ ";" ++ tasks.go(t)

def runs.go(rs: List<&2, U.TRun>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.TRun{+r, +s, +st, +n, +sm, sh} <> t:
      r ++ "|" ++ s ++ "|" ++ st ++ "|" ++ Nat.show(n) ++ "|" ++ sm ++ "|" ++ String.join(sh, ",") ++ ";" ++ runs.go(t)

def ask.k(as: List<&2, M.Ask>, +u: U.Ui) -> U.TAsk:
  match as:
    case a <> _:
      U.Teach.ask.of(a, u, "b1")
    case Nil{}:
      U.Teach.ask(I.En{}, M.Ask{"", "", "", "", "", "", False{}, ""}, "")

def ask(+u: U.Ui) -> U.TAsk:
  ask.k(U.Asks.open(U.Ui.state(u), "bt1"), u)

def asked.k(a: U.TAsk) -> String:
  match a:
    case U.TAsk{+i, +r, +s, +n, +k, +q, +risky, +shot, bs}:
      i ++ "|" ++ r ++ "|" ++ s ++ "|" ++ Nat.show(n) ++ "|" ++ k ++ "|" ++ q ++ "|" ++ Bool.show(risky) ++ "|" ++ shot ++ "|" ++ labels(bs)

def asked(+u: U.Ui) -> String:
  asked.k(ask(u))

def asklabels(a: U.TAsk) -> String:
  match a:
    case U.TAsk{_, _, _, _, _, _, _, _, bs}:
      labels(bs)

def values(bs: List<&2, U.TBtn>) -> String:
  match bs:
    case Nil{}:
      ""
    case U.TBtn{_, +a, +v, _} <> t:
      a ++ ":" ++ v ++ ";" ++ values(t)

def askvalues(a: U.TAsk) -> String:
  match a:
    case U.TAsk{_, _, _, _, _, _, _, _, bs}:
      values(bs)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("tab after browser", String.join(BU.BU.tabs(), ","), "chat,space,browser,tasks,memory,routines,hooks,settings")
    C.Check.str("tab label", BU.BU.tab.label(I.En{}, "tasks"), "Tasks")
    C.Check.str("far bot has no tasks", String.join(BU.BU.tabs.of("p1~b1"), ","), "chat,space,memory,routines")
    C.Check.str("idle strip", strip(plain()), "0|||0|Teach a task,Take over,")
    C.Check.str("idle frame takes no input", Bool.show(U.Teach.input(plain(), "b1")), "False")
    C.Check.str("recording strip", strip(with(plain(), "teach", "rec 940 2")), "1|Kit is watching and learning|1:00|2|Stop & save,Discard,")
    C.Check.str("recording frame takes input", Bool.show(U.Teach.input(with(plain(), "teach", "rec 940 2"), "b1")), "True")
    C.Check.str("steps count the draft", Nat.show(U.Teach.count(U.Ui.info_set(with(plain(), "teach", "rec 940 1"), "teach.draft.b1", draft()), "b1")), "3")
    C.Check.str("review strip", strip(with(plain(), "teach", "review")), "2|Save this task||0|Save,Discard,")
    C.Check.str("takeover strip", strip(with(plain(), "takeover", "")), "3|You have the browser||0|Hand back,")
    C.Check.str("takeover frame takes input", Bool.show(U.Teach.input(with(plain(), "takeover", ""), "b1")), "True")
    C.Check.str("two minutes forty", U.Teach.clock(160n), "2:40")
    C.Check.str("sheet rows", rows.go(U.Teach.rows(U.Ui.info_set(plain(), "teach.draft.b1", draft()), "b1")), "Type in Order number[False];Type in Password[True];Click Search[False];")
    C.Check.str("only the typed non-secret value is a chip", chips(U.Ui.info_set(plain(), "teach.draft.b1", draft())), "1=A123;")
    C.Check.str("tasks", tasks.go(U.Teach.tasks(plain(), "b1")), "jlcpcb|Check order|Look up an order|1|False|False|ok;")
    C.Check.str("runs, newest log of each", runs.go(U.Teach.runs(plain(), "b1")), "r1|jlcpcb|ok|3|done|/bots/b1/tasks/shot?run=r1&n=2;")
    C.Check.str("runs shown by the task's name", runs.go(U.Teach.runs.shown(plain(), "b1")), "r1|Check order|ok|3|done|/bots/b1/tasks/shot?run=r1&n=2;")
    C.Check.str("open a task", tasks.go(U.Teach.tasks(act(plain(), "tk-open", "jlcpcb"), "b1")), "jlcpcb|Check order|Look up an order|1|True|False|ok;")
    C.Check.str("delete asks first", tasks.go(U.Teach.tasks(act(plain(), "tk-del", "jlcpcb"), "b1")), "jlcpcb|Check order|Look up an order|1|False|True|ok;")
    C.Check.str("delete twice deletes", sent(U.Ui.act(act(plain(), "tk-del", "jlcpcb"), "tk-del", "jlcpcb")), "{\"id\":0,\"m\":\"bots.task\",\"p\":{\"bot\":\"b1\",\"op\":\"delete\",\"slug\":\"jlcpcb\"}}")
    C.Check.str("run sends its parameters", sent(U.Ui.act(act(plain(), "bfield", "tkrun.order\u{1f}A123"), "tk-run", "jlcpcb")), "{\"id\":0,\"m\":\"bots.task\",\"p\":{\"bot\":\"b1\",\"op\":\"run\",\"slug\":\"jlcpcb\",\"params\":{\"order\":\"A123\"}}}")
    C.Check.str("teach starts", sent(U.Ui.act(plain(), "tk-teach", "")), "{\"id\":0,\"m\":\"bots.teach\",\"p\":{\"bot\":\"b1\",\"op\":\"start\"}}")
    C.Check.str("stop", sent(U.Ui.act(plain(), "tk-stop", "")), "{\"id\":0,\"m\":\"bots.teach\",\"p\":{\"bot\":\"b1\",\"op\":\"stop\"}}")
    C.Check.str("discard", sent(U.Ui.act(plain(), "tk-discard", "")), "{\"id\":0,\"m\":\"bots.teach\",\"p\":{\"bot\":\"b1\",\"op\":\"discard\"}}")
    C.Check.str("a note", sent(U.Ui.act(act(plain(), "bfield", "tknote\u{1f}  use the second tab "), "tk-note", "")), "{\"id\":0,\"m\":\"bots.teach\",\"p\":{\"bot\":\"b1\",\"op\":\"note\",\"text\":\"use the second tab\"}}")
    C.Check.str("a blank note is not sent", sent(U.Ui.act(plain(), "tk-note", "")), "")
    C.Check.str("a nameless task is not saved", sent(U.Ui.act(U.Ui.info_set(plain(), "teach.draft.b1", draft()), "tk-save", "")), "")
    C.Check.str("save names the marked chip", sent(U.Ui.act(act(act(act(U.Ui.info_set(plain(), "teach.draft.b1", draft()), "bfield", "tkname\u{1f}Check order"), "tk-mark", "1"), "bfield", "tkp.1\u{1f}order"), "tk-save", "")),
      "{\"id\":0,\"m\":\"bots.teach\",\"p\":{\"bot\":\"b1\",\"op\":\"save\",\"name\":\"Check order\",\"description\":\"\",\"params\":[{\"step\":1,\"name\":\"order\",\"label\":\"Order number\"}]}}")
    C.Check.str("a secret cannot be marked", sent(U.Ui.act(act(act(U.Ui.info_set(plain(), "teach.draft.b1", draft()), "bfield", "tkname\u{1f}x"), "tk-mark", "2"), "tk-save", "")),
      "{\"id\":0,\"m\":\"bots.teach\",\"p\":{\"bot\":\"b1\",\"op\":\"save\",\"name\":\"x\",\"description\":\"\",\"params\":[]}}")
    C.Check.str("mark again unmarks", sent(U.Ui.act(act(act(act(U.Ui.info_set(plain(), "teach.draft.b1", draft()), "bfield", "tkname\u{1f}x"), "tk-mark", "1"), "tk-mark", "1"), "tk-save", "")),
      "{\"id\":0,\"m\":\"bots.teach\",\"p\":{\"bot\":\"b1\",\"op\":\"save\",\"name\":\"x\",\"description\":\"\",\"params\":[]}}")
    C.Check.str("takeover", sent(U.Ui.act(plain(), "tk-takeover", "")), "{\"id\":0,\"m\":\"bots.browser\",\"p\":{\"bot\":\"b1\",\"op\":\"takeover\"}}")
    C.Check.str("hand back", sent(U.Ui.act(plain(), "tk-handback", "")), "{\"id\":0,\"m\":\"bots.browser\",\"p\":{\"bot\":\"b1\",\"op\":\"handback\"}}")
    C.Check.str("a click in thousandths", sent(U.Ui.act(plain(), "bb-click", "250 500")), "{\"id\":0,\"m\":\"bots.browser\",\"p\":{\"bot\":\"b1\",\"op\":\"click\",\"x\":250,\"y\":500}}")
    C.Check.str("typing", sent(U.Ui.act(plain(), "bb-type", "hi")), "{\"id\":0,\"m\":\"bots.browser\",\"p\":{\"bot\":\"b1\",\"op\":\"type\",\"text\":\"hi\"}}")
    C.Check.str("a key", sent(U.Ui.act(plain(), "bb-key", "Enter")), "{\"id\":0,\"m\":\"bots.browser\",\"p\":{\"bot\":\"b1\",\"op\":\"press\",\"key\":\"Enter\"}}")
    C.Check.str("scroll up", sent(U.Ui.act(plain(), "bb-scroll", "-120")), "{\"id\":0,\"m\":\"bots.browser\",\"p\":{\"bot\":\"b1\",\"op\":\"scroll\",\"dy\":-120}}")
    C.Check.str("address bar", sent(U.Ui.act(act(plain(), "bfield", "bburl\u{1f}example.com"), "bb-nav", "")), "{\"id\":0,\"m\":\"bots.browser\",\"p\":{\"bot\":\"b1\",\"op\":\"navigate\",\"url\":\"https://example.com\"}}")
    C.Check.str("back", sent(U.Ui.act(plain(), "bb-back", "")), "{\"id\":0,\"m\":\"bots.browser\",\"p\":{\"bot\":\"b1\",\"op\":\"back\"}}")
    C.Check.str("reload", sent(U.Ui.act(plain(), "bb-reload", "")), "{\"id\":0,\"m\":\"bots.browser\",\"p\":{\"bot\":\"b1\",\"op\":\"reload\"}}")
    C.Check.str("stuck card", asked(base("continue\nskip\nstop", "false")), "k1|r1|jlcpcb|3|login|Sign in?|False|/bots/b1/tasks/shot?run=r1&n=3|Continue,Skip this step,Stop task,Take over,")
    C.Check.str("a risky ask offers Confirm", asked(base("confirm\nskip\nstop", "true")), "k1|r1|jlcpcb|3|login|Sign in?|True|/bots/b1/tasks/shot?run=r1&n=3|Confirm,Skip this step,Stop task,Take over,")
    C.Check.str("a risky ask never offers Continue", asklabels(ask(base("continue\nskip\nstop", "true"))), "Confirm,Skip this step,Stop task,Take over,")
    C.Check.str("no options: risky", asklabels(ask(base("", "true"))), "Confirm,Skip this step,Stop task,Take over,")
    C.Check.str("no options: plain", asklabels(ask(base("", "false"))), "Continue,Skip this step,Stop task,Take over,")
    C.Check.str("the person has the browser: no take over", asklabels(ask(with(base("", "false"), "takeover", ""))), "Continue,Skip this step,Stop task,")
    C.Check.str("answers go through the ask", askvalues(ask(base("", "false"))), "answer:k1|continue;answer:k1|skip;answer:k1|stop;tk-takeover:;")
    C.Check.str("a browser ask is a question", Bool.show(U.Asks.question(U.Asks.open(U.Ui.state(plain()), "bt1"))), "True")
