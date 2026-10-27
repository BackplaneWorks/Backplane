import Base
import ../core/presentation.bend as PRESENT
import ../core/client.bend as U
import ../core/model.bend as M
import ../core/json.bend as J
import ../core/vt.bend as VT
import ../core/far.bend as FR
import ./dom.bend as D

# Web desktop commands reuse the native sidebar keymap and normal actions.
# The actor owns the query and PTY owner; no canonical log or shell bytes
# are serialized into presentation scratch.
def WS.owner(+u: U.Ui) -> String:
  U.Ui.scratch(u, "@terminal.owner")

def WS.pid(+u: U.Ui) -> String:
  U.Ui.thread_project(U.Ui.thread(U.Ui.state(u), U.Ui.sel(u)))

def WS.none(u: U.Ui) -> U.Act:
  U.Act{u, Nil{}}

def WS.jump(+u: U.Ui, +id: String) -> U.Act:
  Bool.pick(U.Act, String.is_empty(id), WS.none(u), U.Ui.act(u, "select", id))

def WS.project.here(empty: Bool, +u: U.Ui, +pid: String, +thread: String) -> U.Act:
  match empty:
    case True{}: U.Ui.act(u, "new-thread", pid)
    case False{}: WS.jump(u, thread)

def WS.project(+u: U.Ui, +pid: String) -> U.Act:
  Bool.pick(U.Act, String.is_empty(pid), WS.none(u), WS.project.here(String.is_empty(U.Side.home(u, pid)), u, pid, U.Side.home(u, pid)))

# Same non-wrapping neighbours as app/nui.bend's Keys.after.
def WS.after(xs: List<&2, String>, +current: String) -> String:
  match xs:
    case +a <> +b <> tail: Bool.pick(String, String.eq(a, current), b, WS.after(b <> tail, current))
    case _: ""

def WS.neighbour(+u: U.Ui, +next: Bool) -> U.Act:
  WS.jump(u, WS.after(Bool.pick(List<&2, String>, next, U.Side.visible(u), List.reverse(&2, String, U.Side.visible(u))), U.Ui.sel(u)))

def WS.palette.open(+u: U.Ui, +mode: String) -> U.Act:
  U.Ui.also(WS.none(U.Palette.open(u, mode)), [U.Focus{"workspace-query"}])

def WS.palette.close(u: U.Ui) -> U.Act:
  U.Act{U.Ui.flag_off(u, "palette"), [U.Focus{"composer"}]}

def WS.term.hide(u: U.Ui) -> U.Act:
  U.Act{U.Ui.flag_off(u, "term"), [U.Focus{"composer"}]}

def WS.term.size(+u: U.Ui, +owner: String, cols: U32, rows: U32, +method: String) -> U.Act:
  +c = U32.max(20, U32.min(cols, 400))
  +r = U32.max(5, U32.min(rows, 150))
  U.Ui.rpc(U.Ui.term_set(u, owner, VT.Vt.resize(U.Ui.term(u, owner), c, r)), method,
    [J.KV{"thread", J.Str{owner}}, J.KV{"cols", J.Json.u32(c)}, J.KV{"rows", J.Json.u32(r)}])

def WS.term.dimensions(sz: U32 & U32, u: U.Ui, owner: String, method: String) -> U.Act:
  (c, r) = sz
  WS.term.size(u, owner, c, r, method)

def WS.term.resize(+u: U.Ui, size: String) -> U.Act:
  Bool.pick(U.Act, String.is_empty(WS.owner(u)), WS.none(u), WS.term.dimensions(U.Term.size.k(String.split(size, 'x')), u, WS.owner(u), "term.resize"))

def WS.term.local(+u: U.Ui) -> Bool:
  Bool.and(Bool.not(FR.Fr.is(U.Ui.sel(u))), Bool.and(Bool.not(String.is_empty(U.Ui.sel(u))), Bool.not(String.is_empty(U.Ui.root(u)))))

def WS.term.show(valid: Bool, +u: U.Ui, size: String) -> U.Act:
  match valid:
    case False{}: WS.none(U.Ui.error_set(u, "Select a local project thread before opening a terminal."))
    case True{}:
      +owner = U.Ui.sel(u)
      +incarnation = M.Switch.num(U.Ui.info(u, "local.terminal.incarnation"), 0n)
      +fresh = U.Ui.info_set(U.Ui.info_set(u, "local.terminal.pending", ""), "local.terminal.incarnation", Nat.show(Bool.pick(Nat, Nat.is_lt(incarnation, PRESENT.Session.nat.max()), Nat.add(incarnation, 1n), incarnation)))
      +ready = U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.flag_on(fresh, "term"), "@terminal.owner", owner),
        "@terminal.cwd", U.Ui.root(u)), "@terminal.title", U.Active.title(U.Ui.thread(U.Ui.state(u), owner)))
      U.Ui.also(WS.term.dimensions(U.Term.size.k(String.split(size, 'x')), ready, owner, "term.open"), [U.Focus{"term"}])

def WS.term.toggle(+u: U.Ui, +size: String) -> U.Act:
  Bool.pick(U.Act, Bool.and(U.Ui.flag(u, "term"), String.eq(WS.owner(u), U.Ui.sel(u))), WS.term.hide(u),
    WS.term.show(WS.term.local(u), u, size))

def WS.term.send(+u: U.Ui, bytes: List<&2, U32>) -> U.Act:
  Bool.pick(U.Act, String.is_empty(WS.owner(u)), WS.none(u), U.Ui.rpc(u, "term.input", [J.KV{"thread", J.Str{WS.owner(u)}}, J.KV{"b", J.Json.arr(U.Term.nums(bytes))}]))

def WS.term.key.sym(ks: U32 & U32, +u: U.Ui, mods: U32) -> U.Act:
  (sym, cp) = ks
  WS.term.send(u, VT.Vt.key(U.Ui.term(u, WS.owner(u)), sym, cp, mods))

def WS.term.key.parts(ps: List<&2, String>, u: U.Ui) -> U.Act:
  match ps:
    case +key <> mods <> _: WS.term.key.sym(U.Keys.dom(key), u, U32.from_nat(Maybe.default(&2, Nat, Nat.read(mods), 0n)))
    case _: WS.none(u)

def WS.term.key(+u: U.Ui, value: String) -> U.Act:
  WS.term.key.parts(String.split(value, '\t'), u)

def WS.commands(+u: U.Ui) -> List<&2, U.PItem>:
  List.append(&2, U.PItem, Bool.pick(List<&2, U.PItem>, U.ThreadTree.can_route(U.Ui.state(u), U.Ui.sel(u)),
    [U.PItem{Bool.pick(String, U.ThreadTree.can_create(U.Ui.state(u), U.Ui.sel(u)), "New subthread of current thread", "Choose project conversation…"), "thread-child", U.Ui.sel(u)}], Nil{}),
  [U.PItem{"Uberbot chat · Ctrl+Shift+U", "uber", ""},
   U.PItem{"Floating terminal here · Backtick (`) · Ctrl+J", "ws-term-here", ""},
   U.PItem{"Hide terminal (keep shell running)", "ws-term-hide", ""},
   U.PItem{"Previous thread · Ctrl+Shift+[", "ws-prev", ""}, U.PItem{"Next thread · Ctrl+Shift+]", "ws-next", ""},
   U.PItem{"New thread · Ctrl+N", "new-thread", WS.pid(u)},
   U.PItem{"Rename thread", "row-rename", U.Ui.sel(u)}, U.PItem{"Fork thread", "fork", ""},
   U.PItem{"Pin thread", "pin", ""}, U.PItem{"Unpin thread", "unpin", ""}, U.PItem{"Settle thread · Ctrl+Shift+S", "settle", ""},
   U.PItem{"Archive thread", "archive", ""}, U.PItem{"Unarchive thread", "unarchive", ""}, U.PItem{"Delete thread…", "delete", ""},
   U.PItem{"Add project…", "proj-open", ""}, U.PItem{"Remove current project…", "proj-remove", WS.pid(u)},
   U.PItem{"Find file · Ctrl+P", "fm", ""},
   U.PItem{"Board viewer", "view-kind", "board"}, U.PItem{"Schematic viewer", "view-kind", "schematic"},
   U.PItem{"3D viewer", "view-kind", "3d"}, U.PItem{"Mechanical parts", "mech", ""}, U.PItem{"Browser", "view-kind", "browser"},
   U.PItem{"Diff · Ctrl+D", "diff", ""}, U.PItem{"Design review", "rv-page", "bom"},
   U.PItem{"Agent, model and effort · Ctrl+M", "flag", "models"},
   U.PItem{"Show / hide viewer", "panel", ""}, U.PItem{"Settings", "flag", "settings"},
   U.PItem{"Snooze thread for 1 hour", "snooze", "1h"}, U.PItem{"Snooze thread until tomorrow", "snooze", "tomorrow"}, U.PItem{"Wake thread", "snooze", "off"}])

def WS.projects(xs: List<&2, M.Project>) -> List<&2, U.PItem>:
  match xs:
    case Nil{}: Nil{}
    case M.Project{id, title, _, _, False{}} <> tail: U.PItem{"Project: " ++ title, "ws-project", id} <> WS.projects(tail)
    case _ <> tail: WS.projects(tail)

def WS.items(+u: U.Ui) -> List<&2, U.PItem>:
  List.take(&2, U.PItem, U.Palette.filter(List.concat(&2, U.PItem, [WS.commands(u), WS.projects(U.Ui.projects(U.Ui.state(u))),
    U.Palette.threads(U.Lang.loc(u), U.Kids.keep(U.Ui.threads(U.Ui.state(u)), U.Kids.set(U.Ui.state(u))))]), U.Palette.query(u)), 24n)

def WS.palette.move(+u: U.Ui, +delta: String) -> U.Act:
  +i = U.Palette.index(u)
  WS.none(U.Palette.set(u, U.Palette.query(u), Bool.pick(Nat, String.eq(delta, "up"), Nat.sub(i, 1n), Nat.min(1n+i, Nat.sub(List.length(&2, U.PItem, WS.items(u)), 1n)))))

def WS.run.kind(+action: String, +u: U.Ui, value: String) -> U.Act:
  match action:
    case "ws-term-here": WS.term.show(WS.term.local(u), u, "")
    case "ws-term-hide": WS.term.hide(u)
    case "ws-project": WS.project(u, value)
    case "ws-prev": WS.neighbour(u, False{})
    case "ws-next": WS.neighbour(u, True{})
    case _: U.Ui.act(u, action, value)

def WS.palette.run.item(item: U.PItem, u: U.Ui) -> U.Act:
  match item:
    case U.PItem{_, action, value}: WS.run.kind(action, U.Ui.flag_off(u, "palette"), value)

def WS.palette.run(+u: U.Ui, +index: Nat) -> U.Act:
  WS.palette.run.item(U.Palette.at(WS.items(u), index), u)

def WS.key.digit.valid(valid: Bool, +combo: String, +u: U.Ui, +digit: Nat) -> U.Act:
  match valid:
    case False{}: WS.none(u)
    case True{}: Bool.pick(U.Act, String.starts_with(combo, "alt+"), WS.project(u, U.Side.nth(U.Side.projects(u), Nat.sub(digit, 1n))), WS.jump(u, U.Side.nth(U.Side.here(u), Nat.sub(digit, 1n))))

def WS.key.digit(+combo: String, +u: U.Ui) -> U.Act:
  +digit = Maybe.default(&2, Nat, Nat.read(String.drop(combo, Bool.pick(Nat, String.starts_with(combo, "alt+"), 4n, 5n))), 0n)
  WS.key.digit.valid(Bool.and(Bool.or(String.starts_with(combo, "alt+"), String.starts_with(combo, "ctrl+")), Bool.and(Nat.is_ge(digit, 1n), Nat.is_le(digit, 9n))), combo, u, digit)

def WS.key(+combo: String, +u: U.Ui) -> U.Act:
  match combo:
    case "palette": WS.palette.open(u, "")
    case "uber": U.Ui.act(u, "uber", "")
    case "term": WS.term.toggle(u, "")
    case "prev": WS.neighbour(u, False{})
    case "next": WS.neighbour(u, True{})
    case "new": U.Ui.act(u, "new-thread", WS.pid(u))
    case "settle": U.Ui.act(u, "settle", "")
    case "diff": U.Ui.act(u, "diff", "")
    case "files": U.Ui.act(u, "fm", "")
    case _: WS.key.digit(combo, u)

def WS.handles(+action: String) -> Bool:
  Bool.or(String.starts_with(action, "ws-"), Bool.or(String.eq(action, "term-toggle"), Bool.or(String.eq(action, "term-key"), String.eq(action, "term-paste"))))

def WS.act(action: String, +u: U.Ui, +value: String) -> U.Act:
  match action:
    case "ws-palette-open": WS.palette.open(u, "")
    case "ws-palette-close": WS.palette.close(u)
    case "ws-palette-query": WS.none(U.Palette.set(u, value, 0n))
    case "bfield": WS.none(U.Palette.set(u, String.drop(value, 10n), 0n))
    case "ws-palette-move": WS.palette.move(u, value)
    case "ws-palette-enter": WS.palette.run(u, U.Palette.index(u))
    case "ws-palette-run": WS.palette.run(u, Maybe.default(&2, Nat, Nat.read(value), 0n))
    case "ws-key": WS.key(value, u)
    case "term-toggle": WS.term.toggle(u, value)
    case "ws-term-hide": WS.term.hide(u)
    case "ws-term-resize": WS.term.resize(u, value)
    case "term-key": WS.term.key(u, value)
    case "term-paste": WS.term.send(u, VT.Vt.paste(U.Ui.term(u, WS.owner(u)), value))
    case _: WS.none(u)

def WS.rows(xs: List<&2, U.PItem>, +chosen: Nat, +i: Nat) -> List<&2, D.Node>:
  match xs:
    case Nil{}: Nil{}
    case U.PItem{label, _, _} <> tail:
      D.El{"button", "command:" ++ Nat.show(i), [D.At{"type", "button"}, D.At{"role", "option"}, D.At{"id", "workspace-command-" ++ Nat.show(i)},
        D.At{"aria-selected", Bool.pick(String, Nat.is_eq(chosen, i), "true", "false")}, D.At{"class", Bool.pick(String, Nat.is_eq(chosen, i), "workspace-command on", "workspace-command")},
        D.At{"data-value", Nat.show(i)}, D.On{"click", "ws-palette-run"}], [D.Txt{label}]} <> WS.rows(tail, chosen, 1n+i)

def WS.palette(on: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match on:
    case False{}: Nil{}
    case True{}:
      [D.El{"div", "workspace-palette", [D.At{"class", "workspace-palette-backdrop"}], [
        D.El{"section", "", [D.At{"class", "workspace-palette"}, D.At{"role", "dialog"}, D.At{"aria-label", "Command palette"}, D.At{"aria-modal", "true"}], [
          D.El{"header", "", Nil{}, [D.Txt{"Commands"}, D.Dom.button("", "ws-palette-close", "", "Close command palette · Esc", "Close")]},
          D.El{"input", "workspace-query", [D.At{"id", "workspace-query"}, D.At{"type", "text"}, D.At{"autocomplete", "off"}, D.At{"placeholder", "Search commands, threads and projects"},
            D.At{"value", U.Palette.query(u)}, D.At{"data-bfield", "wspalette"}, D.At{"role", "combobox"}, D.At{"aria-label", "Search commands"}, D.At{"aria-expanded", "true"}, D.At{"aria-controls", "workspace-commands"},
            D.At{"aria-activedescendant", "workspace-command-" ++ Nat.show(U.Palette.index(u))}, D.On{"input", "bfield"}], Nil{}},
          D.El{"div", "workspace-commands", [D.At{"id", "workspace-commands"}, D.At{"role", "listbox"}],
            Bool.pick(List<&2, D.Node>, List.is_empty(&2, U.PItem, WS.items(u)), [D.Dom.span("workspace-empty", "No matching commands")], WS.rows(WS.items(u), U.Palette.index(u), 0n))},
          D.El{"footer", "", Nil{}, [D.Txt{"↑↓ choose · Enter run · Esc close. Ctrl/Cmd+Shift+P (Ctrl+K also opens)."},
            D.El{"details", "", Nil{}, [D.El{"summary", "", Nil{}, [D.Txt{"Keyboard shortcuts"}]},
              D.El{"p", "", Nil{}, [D.Txt{"Alt+1…9 project · Ctrl+1…9 thread · Ctrl+Shift+[ / ] previous / next thread · Ctrl+Shift+U uberbot chat · Ctrl+N new thread · Ctrl+Shift+S settle · Ctrl+P files · Ctrl+D diff · Ctrl+M agent, model and effort."}]},
              D.El{"p", "", Nil{}, [D.Txt{"Backtick (`) toggles Terminal from the workspace or composer. In the shell and other text fields, backtick types normally; use Ctrl+J to toggle. Super+grave / tilde also works if your desktop delivers it. Navigation shortcuts leave editors, IME and PTYs alone. Pin is in Commands (its former Ctrl+Shift+P opens this palette)."}]}]}]}]}]}]

def WS.terminal(on: Bool, +u: U.Ui, screen: D.Node) -> List<&2, D.Node>:
  match on:
    case False{}: Nil{}
    case True{}:
      [D.El{"section", "floating-terminal", [D.At{"class", "floating-terminal"}, D.At{"role", "region"}, D.At{"aria-label", "Floating interactive terminal"}, D.At{"data-terminal-owner", WS.owner(u)}], [
        D.El{"header", "", [D.At{"class", "floating-terminal-handle"}], [D.El{"span", "", [D.At{"class", "floating-terminal-title"}],
          [D.Txt{"Terminal · " ++ U.Ui.scratch(u, "@terminal.title")}]}, D.Dom.button("", "ws-term-hide", "", "Hide terminal; shell keeps running · Ctrl+J", "Hide")]},
        D.El{"div", "", [D.At{"class", "floating-terminal-path"}, D.At{"title", U.Ui.scratch(u, "@terminal.cwd")}], [D.Txt{U.Ui.scratch(u, "@terminal.cwd")}]}, screen,
        D.El{"footer", "", Nil{}, [D.Txt{"Interactive shell · Hide keeps the session · type exit to end · drag to move, resize from corner"}]}]}]

def WS.view(+u: U.Ui, base: D.Node, screen: D.Node) -> D.Node:
  D.El{"div", "workspace-root", [D.At{"class", "workspace-root"}], List.concat(&2, D.Node,
    [[base], WS.terminal(Bool.and(U.Ui.flag(u, "term"), Bool.not(String.is_empty(WS.owner(u)))), u, screen), WS.palette(U.Ui.flag(u, "palette"), u)])}
