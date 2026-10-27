import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/i18n.bend as I
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/mobile/screen.bend as MB
import ../src/core/json.bend as J

# Settings' search (client.bend's Settings.q/found/hint/nav, the field
# "setq"): pages found by their title, a row on them or a word about them,
# whatever the case; Enter ("settings-page" "?") opens the first found.

def state(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def act(+u: U.Ui, +a: String, +v: String) -> U.Ui:
  state(U.Ui.act(u, a, v))

def base() -> U.Ui:
  act(U.Ui.init(0n), "flag", "settings")

def typed(+q: String) -> U.Ui:
  act(base(), "bfield", "setq\u{1f}" ++ q)

def found(+q: String) -> String:
  String.join(U.Settings.found(U.Settings.menu.pages(), I.En{}, U.Settings.q(typed(q))), ",")

# the text and attribute values of nodes (a placeholder is an attribute)
def attrs(as: List<&2, D.Attr>) -> String:
  match as:
    case Nil{}: ""
    case D.At{_, +v} <> t: v ++ "\n" ++ attrs(t)
    case _ <> t: attrs(t)

def texts(ns: List<&2, D.Node>) -> String:
  match ns:
    case Nil{}: ""
    case D.Txt{s} <> t: s ++ "\n" ++ texts(t)
    case D.El{_, _, as, kids} <> t: attrs(as) ++ texts(kids) ++ texts(t)
    case _ <> t: texts(t)

def web(+q: String) -> String:
  texts([V.View.settings.loc(I.En{}, typed(q), U.Ui.state.of(typed(q)), Map.new(&2, String), "test", "")])

def phone(+q: String) -> String:
  J.Json.show(MB.Mob.settings.of.loc(I.En{}, typed(q), U.Ui.state.of(typed(q)), Map.new(&2, String), "test", "", Nil{}))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("an empty search finds every page, in the menu's order", found(""), String.join(U.Settings.menu.pages(), ","))
    C.Check.str("a word about a page finds it", found("stripe"), "ordering")
    C.Check.str("whatever the case", found("TAILSCALE"), "network")
    C.Check.str("a row's name finds its page", found("text size"), "appearance")
    C.Check.str("a page's title finds it", found("privacy"), "privacy")
    C.Check.str("nothing found", found("zzzz"), "")
    C.Check.str("the hint names the rows that matched", U.Settings.hint(I.En{}, "ordering", "price"), "Price estimates")
    C.Check.str("no hint without a query", U.Settings.hint(I.En{}, "ordering", ""), "")
    C.Check.str("settle when done is on Threads", found("when done") ++ " " ++ U.Settings.hint(I.En{}, "threads", "when done"), "threads Settle when done")
    C.Check.str("Enter opens the first page found", U.Settings.page(act(typed("dictation"), "settings-page", "?")), "voice")
    C.Check.str("Enter with nothing found stays on the menu", U.Settings.page(act(typed("zzzz"), "settings-page", "?")), "")
    C.Check.str("a page is still opened by name", U.Settings.page(act(base(), "settings-page", "ordering")), "ordering")
    C.Check.str("the web menu has the search field", Bool.show(String.contains(web(""), "Search settings")), "True")
    C.Check.str("and lists only what it finds", Bool.show(Bool.and(String.contains(web("stripe"), "Ordering ›"), Bool.not(String.contains(web("stripe"), "Appearance ›")))), "True")
    C.Check.str("and says when nothing matches", Bool.show(String.contains(web("zzzz"), "No setting matches.")), "True")
    C.Check.str("phones have the search field", Bool.show(String.contains(phone(""), "\"name\":\"setq\"")), "True")
    C.Check.str("and list only what it finds", Bool.show(Bool.and(String.contains(phone("stripe"), "Ordering ›"), Bool.not(String.contains(phone("stripe"), "Appearance ›")))), "True")
