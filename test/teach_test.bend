import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/task.bend as T
import ../src/core/teach.bend as E
import ../src/core/bot.bend as BT

def yes(+b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def js(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Json.obj(Nil{}))

def info(+b: E.TeachBot) -> String:
  E.Teach.mode.text(b) ++ "|" ++ E.Teach.bot.drive(b) ++ "|" ++ Nat.show(List.length(&2, T.TkStep, E.Teach.bot.steps(b)))

def rec1() -> J.Json:
  js("{\"do\":\"click\",\"target\":{\"role\":\"button\",\"name\":\"Search\"},\"url\":\"https://a.test/\"}")

def rec2() -> J.Json:
  js("{\"do\":\"type\",\"target\":{\"role\":\"textbox\",\"name\":\"Order\"},\"value\":\"W2026\",\"url\":\"https://a.test/\"}")

def rec3() -> J.Json:
  js("{\"do\":\"type\",\"target\":{\"role\":\"textbox\",\"name\":\"Password\",\"type\":\"password\"},\"secret\":true,\"label\":\"Password\",\"value\":\"hunter2\",\"url\":\"https://a.test/\"}")

def nav() -> J.Json:
  js("{\"do\":\"navigate\",\"url\":\"https://a.test/\"}")

def drafted() -> E.TeachBot:
  E.Teach.add(E.Teach.add(E.Teach.add(E.Teach.add(E.Teach.start("b1", 100n), nav()), rec2()), rec3()), rec1())

def taskname(m: E.Saved) -> String:
  match m:
    case E.Saved{ok, err, slug, json}:
      Bool.pick(String, ok, "ok " ++ slug, "err " ++ err)

def savedjson(m: E.Saved) -> String:
  match m:
    case E.Saved{_, _, _, json}:
      json

def st.of(+s: E.TeachSt, +bot: String) -> String:
  info(E.Teach.get(s, bot))

def main() -> IO(Unit):
  +s0 = E.Teach.empty()
  +s1 = E.Teach.put(s0, E.Teach.start("b1", 100n))
  +s2 = E.Teach.put(s1, E.Teach.takeover(E.Teach.blank("b2")))
  +task = E.Teach.build(drafted(), "Order status", "looks it up", [js("{\"step\":2,\"name\":\"order number\",\"label\":\"Order\"}"), js("{\"step\":3,\"name\":\"pw\"}")], 500n)
  do IO<Unit>:
    C.Check.str("idle bot: no state", st.of(s0, "b1"), "||0")
    C.Check.str("teach starts recording", st.of(s1, "b1"), "rec 100 0|teach|0")
    C.Check.str("teach: the bot may not drive", yes(E.Teach.bot_may(s1, "b1")), "no")
    C.Check.str("teach: another bot may", yes(E.Teach.bot_may(s1, "b9")), "yes")
    C.Check.str("takeover: the bot may not drive", yes(E.Teach.bot_may(s2, "b2")), "no")
    C.Check.str("idle: the bot may drive", yes(E.Teach.bot_may(s0, "b1")), "yes")
    C.Check.str("an idle bot leaves the list", Nat.show(List.length(&2, E.TeachBot, E.Teach.st.bots(E.Teach.put(s2, E.Teach.blank("b2"))))), "1")
    C.Check.str("refusal text", E.Teach.refusal(), "The person has the browser (teaching or taking over); wait for them to hand it back.")
    C.Check.str("steps join the draft in order", info(drafted()), "rec 100 4|teach|4")
    C.Check.str("steps are numbered from 1", E.Teach.last(drafted()), "{\"n\":4,\"do\":\"click\",\"target\":{\"role\":\"button\",\"name\":\"Search\",\"label\":\"\",\"placeholder\":\"\",\"text\":\"\",\"testid\":\"\",\"css\":\"\",\"nth\":0,\"frame\":\"\",\"near\":\"\"},\"value\":\"\",\"wait\":{\"url\":\"\",\"text\":\"\"},\"url\":\"https://a.test/\",\"risk\":\"none\",\"note\":\"\"}")
    C.Check.str("a secret's value is never in the draft", yes(String.contains(E.Teach.draft.text(drafted()), "hunter2")), "no")
    C.Check.str("a secret keeps its label", yes(String.contains(E.Teach.draft.text(drafted()), "{\"secret\":\"Password\"}")), "yes")
    C.Check.str("a typed value is kept", yes(String.contains(E.Teach.draft.text(drafted()), "W2026")), "yes")
    C.Check.str("nothing joins when not recording", info(E.Teach.add(E.Teach.stop(drafted()), rec1())), "review|teach|4")
    C.Check.str("stop keeps the draft", info(E.Teach.stop(drafted())), "review|teach|4")
    C.Check.str("a note joins", E.Teach.last(E.Teach.add(E.Teach.start("b", 1n), E.Teach.note("check the total"))), "{\"n\":1,\"do\":\"note\",\"target\":{\"role\":\"\",\"name\":\"\",\"label\":\"\",\"placeholder\":\"\",\"text\":\"\",\"testid\":\"\",\"css\":\"\",\"nth\":0,\"frame\":\"\",\"near\":\"\"},\"value\":\"\",\"wait\":{\"url\":\"\",\"text\":\"\"},\"url\":\"\",\"risk\":\"none\",\"note\":\"check the total\"}")
    C.Check.str("takeover then hand back", info(E.Teach.handback(E.Teach.takeover(E.Teach.blank("b")))), "||0")
    C.Check.str("takeover while teaching changes nothing", info(E.Teach.takeover(E.Teach.start("b", 3n))), "rec 3 0|teach|0")
    C.Check.str("hand back leaves a teach", info(E.Teach.handback(E.Teach.start("b", 3n))), "rec 3 0|teach|0")
    C.Check.str("a recording ends after ten minutes", yes(E.Teach.expired(E.Teach.start("b", 100n), 700n)), "yes")
    C.Check.str("not before", yes(E.Teach.expired(E.Teach.start("b", 100n), 699n)), "no")
    C.Check.str("a review does not expire", yes(E.Teach.expired(E.Teach.stop(E.Teach.start("b", 100n)), 9999n)), "no")
    C.Check.str("info keys", E.Teach.msg(E.Teach.start("b1", 5n)), "{\"t\":\"info\",\"info\":{\"drive.b1\":\"teach\",\"teach.b1\":\"rec 5 0\",\"teach.draft.b1\":\"[]\"}}")
    C.Check.str("info keys clear when idle", E.Teach.msg(E.Teach.blank("b1")), "{\"t\":\"info\",\"info\":{\"drive.b1\":\"\",\"teach.b1\":\"\",\"teach.draft.b1\":\"\"}}")
    C.Check.str("a joining client hears every bot", Nat.show(List.length(&2, J.KV, E.Teach.info(s2))), "6")
    # saving
    C.Check.str("task slug", E.Teach.task.slug(task), "order-status")
    C.Check.str("task: a marked value is a parameter", yes(String.contains(E.Teach.task.text(task), "\"value\":\"{{order_number}}\"")), "yes")
    C.Check.str("task: the example is kept in the parameter", yes(String.contains(E.Teach.task.text(task), "\"example\":\"W2026\"")), "yes")
    C.Check.str("task: a secret never becomes a parameter", yes(String.contains(E.Teach.task.text(task), "pw")), "no")
    C.Check.str("task: no secret value", yes(String.contains(E.Teach.task.text(task), "hunter2")), "no")
    C.Check.str("task: origin", yes(String.contains(E.Teach.task.text(task), "\"origin\":\"show\"")), "yes")
    C.Check.str("task: starts at the first navigate", yes(String.contains(E.Teach.task.text(task), "\"start\":\"https://a.test/\"")), "yes")
    C.Check.str("task: urls", yes(String.contains(E.Teach.task.text(task), "\"urls\":[\"https://a.test/\"]")), "yes")
    C.Check.str("param name", E.Teach.pname("Order number-2!"), "Order_number_2")
    C.Check.str("taught text", yes(String.starts_with(E.Teach.taught("Order status", task), "[taught: Order status] The person just showed you this task")), "yes")
    C.Check.str("taught steps", yes(String.contains(E.Teach.taught("Order status", task), "2 type Order {{order_number}}; 3 type Password <secret: Password>; 4 click Search")), "yes")
    C.Check.str("taught learns", yes(String.contains(E.Teach.taught("Order status", task), "save it with task_save")), "yes")
    C.Check.str("run message", E.Teach.run_msg("order-status", "{\"order\":\"W1\"}"), "[task: order-status] Run the task skill order-status with {\"order\":\"W1\"}.")
    # task_save
    C.Check.str("save: new", taskname(E.Teach.save(None{}, js("{\"name\":\"A b\",\"steps\":[{\"do\":\"navigate\",\"url\":\"u\"}]}"), 5n)), "ok a-b")
    C.Check.str("save: needs a name", taskname(E.Teach.save(None{}, js("{\"slug\":\"x\"}"), 5n)), "err name the task")
    C.Check.str("save: origin tell", yes(String.contains(savedjson(E.Teach.save(None{}, js("{\"name\":\"A b\"}"), 5n)), "\"origin\":\"tell\"")), "yes")
    C.Check.str("save: merges", yes(String.contains(savedjson(E.Teach.save(Some{E.Teach.task.text(task)}, js("{\"slug\":\"order-status\",\"description\":\"x\"}"), 6n)), "{{order_number}}")), "yes")
    C.Check.str("save: merged description", yes(String.contains(savedjson(E.Teach.save(Some{E.Teach.task.text(task)}, js("{\"slug\":\"order-status\",\"description\":\"x\"}"), 6n)), "\"description\":\"x\"")), "yes")
    C.Check.str("save: keeps created", yes(String.contains(savedjson(E.Teach.save(Some{E.Teach.task.text(task)}, js("{\"slug\":\"order-status\",\"description\":\"x\"}"), 6n)), "\"created\":500,\"updated\":6")), "yes")
    # run plan
    C.Check.str("plan: locators", yes(String.contains(J.Json.show(E.Teach.plan(task)), "\"locators\":[\"role=button[name=\\\"Search\\\"]\"]")), "yes")
    C.Check.str("run id", E.Teach.run.id("a-b", 77n), "a-b-77")
    C.Check.str("run slug", E.Teach.run.slug("a-b-77"), "a-b")
    C.Check.str("run ok", yes(E.Teach.run.ok("a-b-77")), "yes")
    C.Check.str("run with slash", yes(E.Teach.run.ok("a/b-77")), "no")
    C.Check.str("run with dots", yes(E.Teach.run.ok("..-77")), "no")
    C.Check.str("run empty", yes(E.Teach.run.ok("")), "no")
    C.Check.str("file ok", yes(E.Teach.file.ok("pause-3.jpg")), "yes")
    C.Check.str("file with slash", yes(E.Teach.file.ok("../x.jpg")), "no")
    C.Check.str("file with dotdot", yes(E.Teach.file.ok("a..jpg")), "no")
    C.Check.str("run dir", E.Teach.run.dir("a-b-77"), "tasks/a-b/runs/a-b-77")
    C.Check.str("act text", E.Teach.act.text("a", "a-1", "running", 0n, "", Nil{}), "{\"slug\":\"a\",\"run\":\"a-1\",\"status\":\"running\",\"n\":0,\"summary\":\"\",\"shots\":[]}")
    # pause
    C.Check.str("options risky", E.Pause.options(True{}), "confirm\nskip\nstop")
    C.Check.str("options plain", E.Pause.options(False{}), "continue\nskip\nstop")
    C.Check.str("detail", E.Pause.detail("b1", "a-1", "a", 3n, "login", "log in?", False{}), "{\"bot\":\"b1\",\"run\":\"a-1\",\"slug\":\"a\",\"n\":3,\"kind\":\"login\",\"question\":\"log in?\",\"risky\":false,\"shot\":\"/bots/b1/tasks/shot?run=a-1&n=3\"}")
    C.Check.str("alert once", yes(E.Pause.alerts(s0, "a-1", 3n)), "yes")
    C.Check.str("not again", yes(E.Pause.alerts(E.Teach.alert_mark(s0, T.Run.alert_key("a-1", 3n)), "a-1", 3n)), "no")
    C.Check.str("a new step alerts", yes(E.Pause.alerts(E.Teach.alert_mark(s0, T.Run.alert_key("a-1", 3n)), "a-1", 4n)), "yes")
    C.Check.str("title", E.Pause.title("Miso"), "Miso needs you")
    C.Check.str("reply confirm", yes(String.contains(E.Pause.reply("confirm"), "confirmed")), "yes")
    C.Check.str("reply stop", yes(String.contains(E.Pause.reply("stop"), "task_end")), "yes")
    # the tasks block
    C.Check.str("block: none", E.Teach.block(Nil{}), "")
    C.Check.str("block: one", E.Teach.block([BT.TRow{"a", E.Teach.task.text(task)}]), "<tasks>\n- a: Order status - looks it up (params: order_number)\n</tasks>")
