import Base
import ./check.bend as C
import ../src/core/accounts.bend as A
import ../src/core/json.bend as J
import ../src/web/dom.bend as D
import ../src/web/view.bend as V

def profile(state: String, provider: String) -> J.Json:
  J.Json.obj([J.KV{"id", J.Str{"saved"}}, J.KV{"provider", J.Str{provider}}, J.KV{"label", J.Str{"Other account"}}, J.KV{"state", J.Str{state}}])

def attrs(xs: List<&2, D.Attr>, +name: String) -> String:
  match xs:
    case Nil{}: ""
    case D.At{+n, +v} <> t: Bool.pick(String, String.eq(n, name), v, attrs(t, name))
    case D.On{+e, +a} <> t: Bool.pick(String, String.eq(e, name), a, attrs(t, name))

def attr(node: D.Node, name: String) -> String:
  match node:
    case D.El{_, _, xs, _}: attrs(xs, name)
    case _: ""

def label(node: D.Node) -> String:
  match node:
    case D.El{_, _, _, [D.Txt{text}]}: text
    case _: ""

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("saved other-provider account remains visible", Nat.show(List.length(&2, D.Node, V.View.accounts.options([profile("connected", "claude"), profile("connected", "codex")], "codex"))), "2")
    C.Check.str("connected account can be selected", attr(V.View.accounts.option(profile("connected", "claude")), "click"), "acct-select")
    C.Check.str("account label identifies its provider", label(V.View.accounts.option(profile("connected", "claude"))), "claude · Other account · connected")
    C.Check.str("unchecked saved account waits for verification", attr(V.View.accounts.option(profile("unchecked", "claude")), "disabled"), "disabled")
    C.Check.str("unchecked account offers checking feedback", label(V.View.accounts.option(profile("unchecked", "claude"))), "claude · Other account · Checking saved sign-in…")
    C.Check.str("disconnected account cannot be selected", attr(V.View.accounts.option(profile("disconnected", "codex")), "click"), "")
    C.Check.str("disconnected account shows authentication next step", label(V.View.accounts.option(profile("disconnected", "codex"))), "codex · Other account · Connect in Settings → Agents")
    C.Check.str("pending sign-in stays unselectable", attr(V.View.accounts.option(profile("connecting", "claude")), "disabled"), "disabled")
    return Unit{}
