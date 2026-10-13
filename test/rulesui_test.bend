import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/rules.bend as RL
import ../src/core/ruledata.bend as RD

# Settings › Rules (client.bend's RulesUi): the rows, the form's round trip
# to rules.set, Delete's two clicks, a refusal opening the form again.

def base() -> U.Ui:
  U.Ui.far.st(U.Ui.init(1000n), RL.Rules.demo.scoped())

def enforced() -> U.Ui:
  U.Ui.far.st(U.Ui.init(1000n), RL.Rules.demo.enforced())

def off() -> U.Ui:
  U.Ui.far.st(U.Ui.init(1000n), RL.Rules.demo.off())

def ui(+a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def sends(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{s} <> t:
      s ++ sends(t)
    case _ <> t:
      sends(t)

def sent(+a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      sends(cs)

def toast(u: U.Ui) -> String:
  match u:
    case U.Ui{_, _, _, _, _, _, _, _, _, _, _, _, e, _, _, _, _}:
      e

def go(+u: U.Ui, +a: String, +v: String) -> U.Ui:
  ui(U.Ui.act(u, a, v))

def say(+u: U.Ui, +field: String, +text: String) -> U.Ui:
  go(u, "bfield", field ++ "\u{1f}" ++ text)

def titles(+rs: List<&2, U.RRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.RRow{_, +t, _, _, _} <> r:
      t ++ "," ++ titles(r)

def notes(+rs: List<&2, U.RRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.RRow{_, _, +n, _, _} <> r:
      n ++ "|" ++ notes(r)

def armed(+rs: List<&2, U.RRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.RRow{_, _, _, _, +a} <> r:
      Bool.show(a) ++ "," ++ armed(r)

def chips(+cs: List<&2, U.RChip>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.RChip{+l, _, +on} <> r:
      l ++ Bool.pick(String, on, "*", "") ++ "," ++ chips(r)

def reply(+id: Nat, +ok: Bool, +err: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{Nat.show(id)}}, J.KV{"ok", J.Flag{ok}}, J.KV{"error", J.Str{err}}])

def main() -> IO(Unit):
  do IO<Unit>:
    # the rows
    C.Check.str("rows: one a rule, oldest first", titles(U.RulesUi.rows(base())), "Miso only,Power only,")
    C.Check.str("rows: the scope in words", notes(U.RulesUi.rows(base())), "Bot Miso|Project Power board|")
    C.Check.str("rows: enforced", notes(U.RulesUi.rows(enforced())), "Bot Miso · tool bot_send · enforced unless \"sudo\"|")
    C.Check.str("rows: off", notes(U.RulesUi.rows(off())), "Bot Miso · tool bot_send · enforced unless \"sudo\" · off|")
    C.Check.str("rows: none", titles(U.RulesUi.rows(U.Ui.init(1000n))), "")
    C.Check.str("everywhere", RD.Rules.note("", "", "", False{}, "", True{}), "Everywhere")
    C.Check.str("enforced with no word", RD.Rules.note("", "", "bot_send", True{}, "", True{}), "Everywhere · tool bot_send · enforced")
    # the section's words
    C.Check.str("title", U.RulesUi.title(), "Rules")
    C.Check.str("lead", U.RulesUi.lead(), "Standing rules every agent is given. Enforced rules refuse the named tools unless your latest message has the word.")
    C.Check.str("none", U.RulesUi.none(), "No rules yet.")
    # a new rule: the form opens empty, the chips are the bots here and the projects
    +n : U.Ui = go(base(), "rule-new", "")
    C.Check.str("new: the form is open", Bool.show(U.RulesUi.open(n)), "True")
    C.Check.str("new: bot chips", chips(U.RulesUi.chips.bot(n)), "Any*,Miso,Tofu,")
    C.Check.str("new: project chips", chips(U.RulesUi.chips.proj(n)), "Any*,Power board,")
    +f : U.Ui = go(go(go(go(go(say(say(say(n, "rutext", " Ask before spending "), "rutool", "bot_send"), "ruword", "sudo"), "rule-bot", "b-miso"), "rule-proj", "p1"), "rule-enf", ""), "rule-bot", "b-miso"), "rule-bot", "b-miso")
    C.Check.str("new: the bot chosen", chips(U.RulesUi.chips.bot(f)), "Any,Miso*,Tofu,")
    C.Check.str("new: the project chosen", chips(U.RulesUi.chips.proj(f)), "Any,Power board*,")
    C.Check.str("new: enforced", Bool.show(U.RulesUi.enf(f)), "True")
    +s : U.Act = U.Ui.act(f, "rule-save", "")
    C.Check.str("save: rules.set with the form's fields", sent(s), "{\"id\":0,\"m\":\"rules.set\",\"p\":{\"rule\":{\"text\":\"Ask before spending\",\"bot\":\"b-miso\",\"project\":\"p1\",\"tool\":\"bot_send\",\"enforce\":true,\"word\":\"sudo\",\"on\":true}}}")
    C.Check.str("save: the form closes", Bool.show(U.RulesUi.open(ui(s))), "False")
    # a refusal opens it again with its reason, the fields kept
    +e : U.Ui = U.Ui.reply(ui(s), reply(0n, False{}, "an enforced rule needs a tool"))
    C.Check.str("refused: the form is open again", Bool.show(U.RulesUi.open(e)), "True")
    C.Check.str("refused: the reason", U.RulesUi.err(e), "an enforced rule needs a tool")
    C.Check.str("refused: no toast", toast(e), "")
    C.Check.str("refused: the text kept", U.Ui.scratch(e, "@f.rutext"), " Ask before spending ")
    # another request's answer leaves the form alone
    C.Check.str("another answer", Bool.show(U.RulesUi.open(U.Ui.reply(ui(s), reply(7n, False{}, "x")))), "False")
    # accepted: the form is empty
    +a : U.Ui = U.Ui.reply(ui(s), reply(0n, True{}, ""))
    C.Check.str("accepted: the text cleared", U.Ui.scratch(a, "@f.rutext"), "")
    C.Check.str("accepted: closed", Bool.show(U.RulesUi.open(a)), "False")
    # cancel closes and forgets
    C.Check.str("cancel", U.Ui.scratch(go(f, "rule-cancel", ""), "@f.rutext") ++ Bool.show(U.RulesUi.open(go(f, "rule-cancel", ""))), "False")
    # editing fills the form from the rule, and saves under its id
    +d : U.Ui = go(base(), "rule-edit", "r1")
    C.Check.str("edit: text", U.Ui.scratch(d, "@f.rutext"), "Miso only")
    C.Check.str("edit: bot", U.Ui.scratch(d, "@f.rubot"), "b-miso")
    C.Check.str("edit: id", U.RulesUi.edit(d), "r1")
    C.Check.str("edit: save keeps the id", sent(U.Ui.act(say(d, "rutext", "Miso, only"), "rule-save", "")), "{\"id\":0,\"m\":\"rules.set\",\"p\":{\"rule\":{\"id\":\"r1\",\"text\":\"Miso, only\",\"bot\":\"b-miso\",\"project\":\"\",\"tool\":\"\",\"enforce\":false,\"word\":\"\",\"on\":true}}}")
    # On/Off
    C.Check.str("on: turns it off", sent(U.Ui.act(base(), "rule-on", "r1")), "{\"id\":0,\"m\":\"rules.set\",\"p\":{\"rule\":{\"id\":\"r1\",\"on\":false}}}")
    C.Check.str("on: turns it on", sent(U.Ui.act(off(), "rule-on", "r1")), "{\"id\":0,\"m\":\"rules.set\",\"p\":{\"rule\":{\"id\":\"r1\",\"on\":true}}}")
    # Delete takes two clicks
    +k1 : U.Act = U.Ui.act(base(), "rule-del", "r1")
    C.Check.str("delete: the first click sends nothing", sent(k1), "")
    C.Check.str("delete: and arms it", armed(U.RulesUi.rows(ui(k1))), "True,False,")
    C.Check.str("delete: keep disarms", armed(U.RulesUi.rows(go(ui(k1), "rule-keep", ""))), "False,False,")
    C.Check.str("delete: another rule's click moves the arming", armed(U.RulesUi.rows(go(ui(k1), "rule-del", "r2"))), "False,True,")
    C.Check.str("delete: the second click drops it", sent(U.Ui.act(ui(k1), "rule-del", "r1")), "{\"id\":0,\"m\":\"rules.drop\",\"p\":{\"id\":\"r1\"}}")
    C.Check.str("delete: and disarms", armed(U.RulesUi.rows(go(ui(k1), "rule-del", "r1"))), "False,False,")
