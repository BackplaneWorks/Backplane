import Base
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/thin.bend as T
import ../core/thinq.bend as Q
import ../core/client.bend as U
import ../web/dom.bend as D
import ../web/presentation.bend as W
import ./thin-view.bend as V
import ../web/workspace.bend as WS

# A compact frame is constructed from issued SOURCE rows, never recovered
# from an arbitrarily large/deep renderer tree. It cannot lose its timeline
# to viewer chrome. Identities and cursors still come from the normal index.
# Text is plain at this tier: Markdown cannot expand into an unbounded DOM.
def Frame.text(s: String, left: Nat) -> String:
  String.take(s, Nat.div(left, 6n))

def Frame.row(+row: T.Row) -> D.Node:
  +entry = T.Row.entry(row)
  D.El{"div", "@compact.entry", [D.At{"class", "msg assistant"}, D.At{"data-id", "@compact.entry"}],
    [D.El{"pre", "", [D.At{"class", "body"}, D.At{"style", "white-space:pre-wrap;overflow-wrap:anywhere"}], [D.Txt{"Message\n" ++ Frame.text(T.Entry.text(entry), 3072n)}]}]}

def Frame.rows(ids: List<&2, String>, +index: T.Index) -> List<&2, D.Node>:
  match ids:
    case Nil{}: Nil{}
    case id <> rest: Frame.row(T.Index.row(index, Q.Q.num(id))) <> Frame.rows(rest, index)

def Frame.palette.rows(xs: List<&2, U.PItem>, +i: Nat) -> List<&2, D.Node>:
  match xs:
    case Nil{}: Nil{}
    case U.PItem{label, _, _} <> rest:
      D.El{"button", "command:" ++ Nat.show(i), [D.At{"type", "button"}, D.At{"class", "workspace-command"}, D.At{"data-value", Nat.show(i)}, D.On{"click", "ws-palette-run"}],
        [D.Txt{Frame.text(label, 960n)}]} <> Frame.palette.rows(rest, 1n+i)

def Frame.palette(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case False{}: Nil{}
    case True{}: [D.El{"div", "workspace-palette", [D.At{"class", "workspace-palette-backdrop"}], [
      D.El{"section", "", [D.At{"class", "workspace-palette"}, D.At{"role", "dialog"}, D.At{"aria-modal", "true"}], [
        D.Dom.button("", "ws-palette-close", "", "Close", "Close"),
        D.El{"input", "workspace-query", [D.At{"id", "workspace-query"}, D.At{"type", "text"}, D.At{"value", Frame.text(U.Palette.query(u), 1536n)}, D.At{"data-bfield", "wspalette"}, D.On{"input", "bfield"}], Nil{}},
        D.El{"div", "workspace-commands", [D.At{"id", "workspace-commands"}], Frame.palette.rows(List.take(&2, U.PItem, WS.WS.items(u), 8n), 0n)}]}]}]

def Frame.more(on: Bool) -> List<&2, D.Node>:
  match on:
    case False{}: Nil{}
    case True{}: [D.Dom.button("thin-message-more", "thin-message-more", "@compact.body", "More", "More")]

def Frame.shown.core(+u: U.Ui, +index: T.Index, +ids: List<&2, String>, +window: J.Json, paletteOn: Bool) -> J.Json:
  W.Node.write(D.El{"div", "workspace-root", [D.At{"class", "workspace-root"}], List.append(&2, D.Node, [D.El{"div", "app", [D.At{"class", "app picked"}], [
    D.El{"aside", "sidebar", [D.At{"class", "sidebar"}], [
      D.Dom.button("cmd", "ws-palette-open", "", "Jump or run a command", "Jump or run a command"),
      D.Dom.button("", "select", "", "Projects", "Projects")]},
    D.El{"main", "thread", [D.At{"class", "thread-view"}, D.At{"data-thread", "@compact.thread"}], [
      D.El{"div", "timeline", [D.At{"id", "timeline"}, D.At{"class", "timeline"}], List.append(&2, D.Node,
        V.Render.older(J.Json.get_bool(window, "hasMore", False{}), u),
        List.append(&2, D.Node, List.reverse(&2, D.Node, Frame.rows(ids, index)), Frame.more(Bool.not(List.is_empty(&2, J.Json, J.Json.elems(J.Json.get_obj(window, "partial")))))))},
      D.El{"div", "composer", [D.At{"class", "composer"}], [
        D.El{"textarea", "composer-input", [D.At{"id", "composer"}, D.At{"value", ""}, D.At{"placeholder", "Message"}, D.On{"input", "draft"}], Nil{}},
        D.Dom.button("send", "send", "", "Send", "Send")]}]}]}], Frame.palette(paletteOn, u))})

def Frame.shown(+u: U.Ui, index: T.Index, ids: List<&2, String>, window: J.Json) -> J.Json:
  Frame.shown.core(u, index, ids, window, U.Ui.flag(u, "palette"))

def Frame.floor(u: U.Ui, index: T.Index, ids: List<&2, String>, window: J.Json) -> J.Json:
  Frame.shown.core(u, index, ids, window, False{})

# Session/commands stay EXACT: a separately framed control lane at the final
# handoff carries them if they do not fit alongside the bounded picture.
def Frame.aliases(ids: List<&2, String>, +index: T.Index, +u: U.Ui) -> J.Json:
  match ids:
    case Nil{}: J.Json.obj([J.KV{"@compact.thread", J.Str{U.Ui.sel(u)}}])
    case id <> _: J.Json.obj([J.KV{"@compact.thread", J.Str{U.Ui.sel(u)}},
      J.KV{"@compact.entry", J.Str{M.Idx.id(T.Row.entry(T.Index.row(index, Q.Q.num(id))))}}])


# Split by encoded scalar cost. Every control packet is <= 32 KiB of text
# plus fixed metadata. Values are reconstructed before adoption/dispatch;
# they are never interpreted or partially adopted while travelling.
type Part is Data:
  Part{text: String, rest: String}

def Part.take(+s: String, left: Nat) -> Part:
  +n = Nat.max(1n, Nat.div(left, 6n))
  Part{String.take(s, n), String.drop(s, n)}

def Part.text(p: Part) -> String:
  match p:
    case Part{text, _}: text

def Part.rest(p: Part) -> String:
  match p:
    case Part{_, rest}: rest

def Part.packet(+id: J.Json, +n: Nat, +text: String, +last: Bool) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"view.control"}}, J.KV{"id", id}, J.KV{"part", J.Json.nat(n)},
    J.KV{"last", J.Flag{last}}, J.KV{"text", J.Str{text}}])

# Only shown/window plus the small protocol identity remain in the frame.
# All other values (including future result fields) stay exact in control.
def Control.display(+key: String) -> Bool:
  String.eq(key, "shown")

def Control.field(keep: Bool, k: String, v: J.Json, rest: J.Json) -> J.Json:
  match keep:
    case True{}: J.Field{k, v, rest}
    case False{}: rest

def Control.fields(fs: J.Json, +display: Bool) -> J.Json:
  match fs:
    case J.Field{+k, v, rest}: Control.field(Bool.not(Bool.xor(Control.display(k), display)), k, v, Control.fields(rest, display))
    case _: J.End{}

def Control.of(j: J.Json, display: Bool) -> J.Json:
  match j:
    case J.Obj{fs}: J.Obj{Control.fields(fs, display)}
    case _: J.Json.obj(Nil{})

# This is the FINAL socket-envelope schema, after all normal metadata,
# delta, region and account composition. No arbitrary outer fields survive.
def Control.picture(m: Maybe<&2, J.Json>, rid: J.Json) -> J.Json:
  match m:
    case Some{shown}: J.Json.obj([J.KV{"shown", shown}, J.KV{"controlId", rid}])
    case None{}: J.Json.obj([J.KV{"controlId", rid}])

def Control.envelope(+rid: J.Json, result: J.Json) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", rid}, J.KV{"ok", J.Flag{True{}}},
    J.KV{"result", Control.picture(J.Json.get(result, "shown"), rid)}])

def Frame.choose(fits: Bool, candidate: J.Json, minimum: J.Json) -> J.Json:
  match fits:
    case True{}: candidate
    case False{}: minimum

# Charge the complete final schema with a conservative admitted-ID size.
# Source IDs/window/session/commands are exact controls, not optional chrome.
def Frame.make(+u: U.Ui, +index: T.Index, +ids: List<&2, String>, +window: J.Json, result: J.Json) -> J.Json:
  +candidate = Frame.shown(u, index, ids, window)
  +minimum = Frame.floor(u, index, ids, window)
  +fits = Nat.is_le(J.Json.bytes(Control.envelope(J.Num{"14073748835532914073748835532900"}, J.Json.obj([J.KV{"shown", candidate}]))), 229376n)
  V.Obj.put(V.Obj.put(V.Obj.put(result, "shown", Frame.choose(fits, candidate, minimum)), "entryAliases", Frame.aliases(ids, index, u)), "boundedFrame", J.Num{"1"})
