import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/bot.bend as BT
import ../src/core/task.bend as T

def tgt(+role: String, +name: String) -> T.TkTarget:
  T.TkTarget{role, name, "", "", "", "", "", 0n, "", ""}

def sample() -> T.TkTask:
  T.TkTask{"jlcpcb-order-status", "Check JLCPCB order status", "Looks up an order", "show", "https://jlcpcb.com/",
    [T.TkParam{"order", "Order number", "text", "W2026"}],
    [T.TkStep{1n, "navigate", tgt("", ""), T.VNone{}, "", "", "https://jlcpcb.com/", "none", ""},
     T.TkStep{2n, "type", T.TkTarget{"textbox", "Order", "Order", "Order no", "", "order", "#order", 1n, "", "Orders"}, T.VParam{"order"}, "", "", "https://jlcpcb.com/", "none", ""},
     T.TkStep{3n, "type", tgt("textbox", "Password"), T.VSecret{"Password"}, "", "", "", "none", ""},
     T.TkStep{4n, "click", tgt("button", "Search"), T.VLit{"x"}, "/orders", "Status", "", "none", "note \"q\""}],
    ["https://jlcpcb.com/"], 5n, 6n}

def text() -> String:
  J.Json.show(T.Task.json(sample()))

def again() -> String:
  J.Json.show(T.Task.json(Maybe.default(&2, T.TkTask, T.Task.read(text()), T.TkTask{"", "", "", "", "", Nil{}, Nil{}, Nil{}, 0n, 0n})))

def rec(+s: String) -> T.TkStep:
  T.TkStep.of_record(Maybe.default(&2, J.Json, J.Json.parse(s), J.Json.obj(Nil{})), 7n)

def val(v: T.TkValue) -> String:
  match v:
    case T.VLit{t}:
      "lit:" ++ t
    case T.VParam{n}:
      "param:" ++ n
    case T.VSecret{l}:
      "secret:" ++ l
    case T.VNone{}:
      "none"

def stval(s: T.TkStep) -> String:
  match s:
    case T.TkStep{_, _, _, v, _, _, _, _, _}:
      val(v)

def stact(s: T.TkStep) -> String:
  match s:
    case T.TkStep{n, a, _, _, _, _, _, r, _}:
      Nat.show(n) ++ ":" ++ a ++ ":" ++ r

def stnote(s: T.TkStep) -> String:
  match s:
    case T.TkStep{_, _, _, _, _, _, _, _, n}:
      n

def sttarget(s: T.TkStep) -> String:
  match s:
    case T.TkStep{_, _, T.TkTarget{r, n, l, p, t, i, c, nth, f, _}, _, _, _, _, _, _}:
      r ++ "|" ++ n ++ "|" ++ l ++ "|" ++ p ++ "|" ++ t ++ "|" ++ i ++ "|" ++ c ++ "|" ++ Nat.show(nth) ++ "|" ++ f

def strs(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case +x <> t:
      x ++ ";" ++ strs(t)

def kv(+k: String, +v: String) -> J.KV:
  J.KV{k, J.Str{v}}

def got(r: Result<&2, &2, String, T.TkTask>) -> String:
  match r:
    case Fail{e}:
      "fail " ++ e
    case Done{t}:
      J.Json.show(T.Task.json(t))

def pt(p: T.TkPt) -> String:
  match p:
    case T.TkPt{x, y}:
      Nat.show(x) ++ "," ++ Nat.show(y)

def st(r: T.TkRun) -> String:
  match r:
    case T.Running{n}:
      "run " ++ Nat.show(n)
    case T.Waiting{n, k, b}:
      "wait " ++ Nat.show(n) ++ " " ++ k ++ Bool.pick(String, b, " risky", "")
    case T.Person{n}:
      "person " ++ Nat.show(n)
    case T.Ended{s}:
      "ended " ++ s

def yes(+b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def risk(+act: String, +name: String) -> String:
  T.Task.risk(T.TkStep{1n, act, tgt("button", name), T.VNone{}, "", "", "", "none", ""})

def bw(s: M.State) -> BT.World:
  M.State.bots(s)

def rows(xs: List<&2, BT.TRow>) -> String:
  match xs:
    case Nil{}:
      ""
    case BT.TRow{+s, +j} <> t:
      s ++ "=" ++ j ++ ";" ++ rows(t)

def main() -> IO(Unit):
  +w0 = M.State.empty()
  +w1 = M.State.replay([M.TaskSet{"b1", "a", "{\"slug\":\"a\"}", 1n}, M.TaskSet{"b1", "b", "{\"slug\":\"b\"}", 2n}, M.TaskSet{"b2", "a", "{\"slug\":\"other\"}", 3n}], w0)
  +w2 = M.State.replay([M.TaskSet{"b1", "a", "{\"slug\":\"a2\"}", 4n}, M.TaskDeleted{"b1", "b", 5n}, M.TaskDeleted{"b1", "none", 6n}], w1)
  do IO<Unit>:
    C.Check.str("json: stable round trip", again(), text())
    C.Check.str("json: slug in text", yes(String.contains(text(), "\"slug\":\"jlcpcb-order-status\"")), "yes")
    C.Check.str("json: param is {{name}}", yes(String.contains(text(), "\"value\":\"{{order}}\"")), "yes")
    C.Check.str("json: secret is an object", yes(String.contains(text(), "\"value\":{\"secret\":\"Password\"}")), "yes")
    C.Check.str("parse: not an object", yes(Maybe.is_some(&2, T.TkTask, T.Task.parse(J.Str{"x"}))), "no")
    C.Check.str("parse: no slug", yes(Maybe.is_some(&2, T.TkTask, T.Task.read("{\"name\":\"x\"}"))), "no")
    C.Check.str("parse: bad text", yes(Maybe.is_some(&2, T.TkTask, T.Task.read("{nope"))), "no")
    C.Check.str("parse: keeps nth", sttarget(Maybe.default(&2, T.TkStep, List.head(&2, T.TkStep, T.Tk.steps_of(Maybe.default(&2, T.TkTask, T.Task.read(text()), sample()))), rec("{}"))), "|||||||0|")
    C.Check.str("record: click", stact(rec("{\"do\":\"click\",\"target\":{\"role\":\"button\",\"name\":\"Go\",\"nth\":2},\"url\":\"u\",\"frame\":\"f\"}")), "7:click:none")
    C.Check.str("record: click target", sttarget(rec("{\"do\":\"click\",\"target\":{\"role\":\"button\",\"name\":\"Go\",\"nth\":2,\"near\":\"H\"},\"url\":\"u\",\"frame\":\"f\"}")), "button|Go||||||2|f")
    C.Check.str("record: submit is a risky click", stact(rec("{\"do\":\"submit\",\"target\":{}}")), "7:click:submit")
    C.Check.str("record: type", stval(rec("{\"do\":\"type\",\"target\":{\"role\":\"textbox\"},\"value\":\"hello\"}")), "lit:hello")
    C.Check.str("record: type, no value", stval(rec("{\"do\":\"type\",\"target\":{}}")), "none")
    C.Check.str("record: secret", stval(rec("{\"do\":\"type\",\"secret\":true,\"label\":\"Card\",\"target\":{}}")), "secret:Card")
    C.Check.str("record: secret label from target", stval(rec("{\"do\":\"type\",\"secret\":true,\"target\":{\"label\":\"Password\"}}")), "secret:Password")
    C.Check.str("record: secret ignores a stray value", stval(rec("{\"do\":\"type\",\"secret\":true,\"label\":\"Pin\",\"value\":\"hunter2\",\"target\":{}}")), "secret:Pin")
    C.Check.str("record: no stray value in the JSON", yes(String.contains(J.Json.show(T.Tk.step.json(rec("{\"do\":\"type\",\"secret\":true,\"label\":\"Pin\",\"value\":\"hunter2\",\"target\":{}}"))), "hunter2")), "no")
    C.Check.str("record: note", stnote(rec("{\"do\":\"note\",\"value\":\"check the total\"}")), "check the total")
    C.Check.str("record: note has no value", stval(rec("{\"do\":\"note\",\"value\":\"check the total\"}")), "none")
    C.Check.str("fill: missing", got(T.Task.fill(sample(), Nil{})), "fail missing parameter: order")
    C.Check.str("missing list", strs(T.Task.missing(sample(), [kv("zzz", "1")])), "order;")
    C.Check.str("missing none", strs(T.Task.missing(sample(), [kv("order", "42")])), "")
    C.Check.str("fill: given", yes(String.contains(got(T.Task.fill(sample(), [kv("order", "42")])), "\"value\":\"42\"")), "yes")
    C.Check.str("fill: no braces left", yes(String.contains(got(T.Task.fill(sample(), [kv("order", "42")])), "{{")), "no")
    C.Check.str("fill: number value", yes(String.contains(got(T.Task.fill(sample(), [J.KV{"order", J.Num{"42"}}])), "\"value\":\"42\"")), "yes")
    C.Check.str("fill: keeps the secret", yes(String.contains(got(T.Task.fill(sample(), [kv("order", "42")])), "{\"secret\":\"Password\"}")), "yes")
    C.Check.str("locators: all", strs(T.Task.locators(T.TkTarget{"textbox", "Order \"no\"", "Lbl", "Ph", "Tx", "tid", "#c", 1n, "", ""})),
      "role=textbox[name=\"Order \\\"no\\\"\"] >> nth=1;[aria-label=\"Lbl\"];[placeholder=\"Ph\"];[data-testid=\"tid\"];text=\"Tx\";#c;")
    C.Check.str("locators: role needs a name", strs(T.Task.locators(T.TkTarget{"button", "", "", "", "", "", "a > b", 0n, "", ""})), "a > b;")
    C.Check.str("locators: nth 0 adds nothing", strs(T.Task.locators(tgt("button", "Go"))), "role=button[name=\"Go\"];")
    C.Check.str("locators: nothing", strs(T.Task.locators(tgt("", ""))), "")
    C.Check.str("risk: search", risk("click", "Search"), "none")
    C.Check.str("risk: pay", risk("click", "Pay now"), "purchase")
    C.Check.str("risk: place order", risk("click", "Place Order"), "purchase")
    C.Check.str("risk: order status is fine", risk("click", "Order status"), "none")
    C.Check.str("risk: send", risk("press", "Send message"), "send")
    C.Check.str("risk: delete", risk("click", "Delete account"), "delete")
    C.Check.str("risk: remove", risk("click", "Remove item"), "delete")
    C.Check.str("risk: confirm", risk("click", "Confirm"), "confirm")
    C.Check.str("risk: typing is not risky", risk("type", "Pay"), "none")
    C.Check.str("risk: own mark wins", T.Task.risk(T.TkStep{1n, "click", tgt("button", "Go"), T.VNone{}, "", "", "", "submit", ""}), "submit")
    C.Check.str("risk: explicit none looks at words", T.Task.risk(T.TkStep{1n, "click", tgt("button", "Buy"), T.VNone{}, "", "", "", "none", ""}), "purchase")
    C.Check.str("scale", pt(T.Task.scale(400n, 300n, 800n, 600n, 1280n, 720n)), "640,360")
    C.Check.str("scale: clamped", pt(T.Task.scale(900n, 700n, 800n, 600n, 1280n, 720n)), "1279,719")
    C.Check.str("scale: no frame", pt(T.Task.scale(10n, 20n, 0n, 0n, 1280n, 720n)), "10,20")
    C.Check.str("slug", T.Task.slug("Check JLCPCB order status!"), "check-jlcpcb-order-status")
    C.Check.str("slug: collapses", T.Task.slug("  --A__b  c--"), "a-b-c")
    C.Check.str("slug: empty", T.Task.slug(""), "task")
    C.Check.str("slug: only symbols", T.Task.slug("!!!"), "task")
    C.Check.str("slug: cut", T.Task.slug("abcdefghij abcdefghij abcdefghij abcdefghij abcdefghij abcdefghij"), "abcdefghij-abcdefghij-abcdefghij-abcdefghij-abcd")
    C.Check.str("slug: cut never ends in a dash", T.Task.slug("abcdefghij abcdefghij abcdefghij abcdefghij abc defgh"), "abcdefghij-abcdefghij-abcdefghij-abcdefghij-abc")
    C.Check.str("drive: bot", yes(T.Drive.can_bot(T.DBot{})), "yes")
    C.Check.str("drive: teach", yes(T.Drive.can_bot(T.DPerson{"teach"})), "no")
    C.Check.str("drive: takeover", yes(T.Drive.can_bot(T.DPerson{"takeover"})), "no")
    C.Check.str("run: advance", st(T.Run.step(T.Running{1n}, T.Advance{})), "run 2")
    C.Check.str("run: finish", st(T.Run.step(T.Running{3n}, T.Finish{"ok"})), "ended ok")
    C.Check.str("run: pause", st(T.Run.step(T.Running{3n}, T.Pause{"login", False{}})), "wait 3 login")
    C.Check.str("run: continue", st(T.Run.step(T.Waiting{3n, "login", False{}}, T.Answer{"continue"})), "run 3")
    C.Check.str("run: risky continue stays", st(T.Run.step(T.Waiting{3n, "pay", True{}}, T.Answer{"continue"})), "wait 3 pay risky")
    C.Check.str("run: risky confirm", st(T.Run.step(T.Waiting{3n, "pay", True{}}, T.Answer{"confirm"})), "run 3")
    C.Check.str("run: skip", st(T.Run.step(T.Waiting{3n, "login", False{}}, T.Answer{"skip"})), "run 4")
    C.Check.str("run: takeover", st(T.Run.step(T.Waiting{3n, "captcha", False{}}, T.Answer{"takeover"})), "person 3")
    C.Check.str("run: hand back", st(T.Run.step(T.Person{3n}, T.HandBack{})), "run 3")
    C.Check.str("run: stop", st(T.Run.step(T.Waiting{3n, "x", False{}}, T.Answer{"stop"})), "ended stopped")
    C.Check.str("run: stop while the person drives", st(T.Run.step(T.Person{3n}, T.Answer{"stop"})), "ended stopped")
    C.Check.str("run: unknown answer", st(T.Run.step(T.Waiting{3n, "x", False{}}, T.Answer{"maybe"})), "wait 3 x")
    C.Check.str("run: advance while waiting", st(T.Run.step(T.Waiting{3n, "x", False{}}, T.Advance{})), "wait 3 x")
    C.Check.str("run: ended stays", st(T.Run.step(T.Ended{"ok"}, T.Advance{})), "ended ok")
    C.Check.str("run: person ignores advance", st(T.Run.step(T.Person{3n}, T.Advance{})), "person 3")
    C.Check.str("run: alert key", T.Run.alert_key("r1", 4n), "task-r1-4")
    C.Check.str("alerts: on a pause", yes(T.Run.alerts(T.Running{1n}, T.Waiting{1n, "x", False{}})), "yes")
    C.Check.str("alerts: not while waiting", yes(T.Run.alerts(T.Waiting{1n, "x", False{}}, T.Waiting{1n, "x", False{}})), "no")
    C.Check.str("alerts: not on leaving", yes(T.Run.alerts(T.Waiting{1n, "x", False{}}, T.Running{1n})), "no")
    C.Check.str("alerts: a new pause after a takeover", yes(T.Run.alerts(T.Person{1n}, T.Waiting{2n, "x", False{}})), "yes")
    C.Check.str("world: tasks of b1", rows(BT.World.tasks_of(bw(w1), "b1")), "a={\"slug\":\"a\"};b={\"slug\":\"b\"};")
    C.Check.str("world: tasks of b2", rows(BT.World.tasks_of(bw(w1), "b2")), "a={\"slug\":\"other\"};")
    C.Check.str("world: a task", Maybe.default(&2, String, BT.World.task(bw(w1), "b1", "b"), "none"), "{\"slug\":\"b\"}")
    C.Check.str("world: no such task", Maybe.default(&2, String, BT.World.task(bw(w1), "b1", "zz"), "none"), "none")
    C.Check.str("world: set replaces, delete drops", rows(BT.World.tasks_of(bw(w2), "b1")), "a={\"slug\":\"a2\"};")
    C.Check.str("world: another bot's untouched", rows(BT.World.tasks_of(bw(w2), "b2")), "a={\"slug\":\"other\"};")
    C.Check.str("world: empty", rows(BT.World.tasks_of(bw(w0), "b1")), "")
