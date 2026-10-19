import Base
import ../fixtures/i18n_webphone.bend as F
import ../../src/core/json.bend as J
import ../../src/core/client.bend as U
import ../../src/web/dom.bend as D
import ../../src/web/view.bend as W
import ../../src/mobile/screen.bend as S

# Compare actual English UI text and phone JSON with the pre-extraction tree.
# Ignore DOM identity/style attributes; retain labels, hints and accessibility.
def attrs(xs: List<&2, D.Attr>, acc: String) -> String:
  match xs:
    case Nil{}: acc
    case D.At{+k, v} <> t:
      attrs(t, acc ++ Bool.pick(String, Bool.or(String.eq(k, "title"), Bool.or(String.eq(k, "placeholder"), String.eq(k, "aria-label"))), k ++ ":" ++ v ++ "|", ""))
    case _ <> t: attrs(t, acc)

def words(fuel: Nat, ns: List<&2, D.Node>, acc: String) -> String:
  match fuel ns:
    case 0n _: acc
    case 1n+_ Nil{}: acc
    case 1n+k D.Txt{t} <> rest: words(k, rest, acc ++ t ++ "|")
    case 1n+k D.El{_, _, as, ks} <> rest:
      words(k, List.append(&2, D.Node, ks, rest), attrs(as, acc))

def dump(name: String, +u: U.Ui) -> IO(Unit):
  IO.print(J.Json.show(J.Json.obj([J.KV{"case", J.Str{name}},
    J.KV{"web", J.Str{words(10000n, W.View.expand([W.View.page(u)]), "")}},
    J.KV{"phone", S.Mob.screen(u)}])))

def main() -> IO(Unit):
  do IO<Unit>:
    dump("home", F.Fixture.ui("en"))
    dump("thread", F.Fixture.thread("en"))
    dump("settings", F.Fixture.settings("en"))
    dump("menu", F.Fixture.menu("en"))
    dump("bot", F.Fixture.bot("en"))
