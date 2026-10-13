import Base
import ./check.bend as C
import ./trb_ui.bend as X
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/rules.bend as RS
import ../src/core/ruledata.bend as RD
import ../src/core/model.bend as M

def row(rs: List<&2, U.RRow>) -> String:
  match rs:
    case U.RRow{_, title, note, _} <> _:
      title ++ "|" ++ note
    case Nil{}:
      ""

def bool(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def saved(r: RS.RR) -> String:
  match r:
    case RS.RRok{+r}:
      RS.Rule.text(r) ++ "|" ++ RS.Rule.bot(r) ++ "|" ++ RS.Rule.project(r) ++ "|" ++ RS.Rule.word(r) ++ "|" ++ bool(RS.Rule.on(r))
    case RS.RRerr{e}:
      e

def main() -> IO(Unit):
  do IO<Unit>:
    +u : U.Ui = X.ui()
    +edit : U.Ui = X.act(u, "rule-edit", "r1")
    +new : U.Ui = X.act(u, "rule-new", "")
    +armed : U.Ui = X.act(u, "rule-del", "r1")
    +o : J.Json = J.Json.get_obj(X.params(edit, "rule-save", ""), "rule")
    C.Check.str("row includes scope, tool, enforcement and off", row(U.RulesUi.rows(u)), "Ask first|Bot Miso · Project Power · tool spend* · enforced unless \"sudo\" · off")
    C.Check.str("edit opens form", bool(U.Ui.flag(edit, "ruleform")), "yes")
    C.Check.str("form round trip to hub", saved(RS.Rules.of.json(o, U.Ui.state(u), 3n, "person")), "Ask first|b1|p1|sudo|no")
    C.Check.str("save method", X.method(edit, "rule-save", ""), "rules.set")
    C.Check.str("save closes form", bool(U.Ui.flag(X.act(edit, "rule-save", ""), "ruleform")), "no")
    C.Check.str("new form clears scope", U.Bots.field(new, "rbot") ++ U.Bots.field(new, "rproj"), "")
    C.Check.str("new omits id", J.Json.show(J.Json.get_obj(U.RulesUi.params(new), "id")), "null")
    C.Check.str("enforce toggle", U.Ui.scratch(X.act(new, "rule-enf", ""), "@rule.enf"), "1")
    C.Check.str("first delete arms without send", X.method(u, "rule-del", "r1"), "")
    C.Check.str("second delete sends", X.method(armed, "rule-del", "r1"), "rules.drop")
    C.Check.str("keep disarms", U.Ui.scratch(X.act(armed, "rule-keep", ""), "@rule.del"), "")
    +waiting : U.Ui = X.act(edit, "rule-save", "")
    +failed : U.Ui = U.Ui.reply(waiting, J.Json.obj([J.KV{"id", J.Num{U.Ui.scratch(waiting, "@rule.rid")}}, J.KV{"ok", J.Flag{False{}}}, J.KV{"error", J.Str{"invalid tool"}}]))
    C.Check.str("hub error reopens form with note", bool(U.Ui.flag(failed, "ruleform")) ++ "|" ++ U.Ui.scratch(failed, "@rule.err"), "yes|invalid tool")
