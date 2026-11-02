import Base
import ./dom.bend as D
import ./usage-graph.bend as G
import ../core/client.bend as U
import ../core/usage_page.bend as UP
import ../core/usage.bend as UZ
import ../core/i18n.bend as I
import ../core/msgs.bend as MSG

def Uv.setrow(+label: String, +note: String, ctl: List<&2, D.Node>) -> D.Node:
  D.El{"div", "", [D.At{"class", "set-row"}], [D.El{"div", "", [D.At{"class", "set-text"}], [D.Dom.span("set-label", label), D.Dom.span("set-note", note)]},
    D.El{"div", "", [D.At{"class", "set-ctl"}], ctl}]}

def Uv.btn.loc(+lc: I.Loc, none: Bool, +act: String) -> List<&2, D.Node>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      [D.Dom.button("", act, "", MSG.T.settings.boat.refresh(lc), MSG.T.common.refresh(lc))]

# Settings > Usage (src/core/usage.bend), the first row titled "Usage: ...";
# a row with an action has its Refresh button

def Uv.btn(none: Bool, +act: String) -> List<&2, D.Node>:
  Uv.btn.loc(I.En{}, none, act)

def Uv.row.loc(+lc: I.Loc, +label: String, +r: UZ.URow) -> D.Node:
  Uv.setrow(label, UZ.URow.note(r), Uv.btn.loc(lc, String.is_empty(UZ.URow.act(r)), UZ.URow.act(r)))

def Uv.row(+label: String, +r: UZ.URow) -> D.Node:
  Uv.row.loc(I.En{}, label, r)

def Uv.rows.go.loc(+lc: I.Loc, rs: List<&2, UZ.URow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Uv.row.loc(lc, UZ.URow.label(r), r) <> Uv.rows.go.loc(lc, t)

def Uv.rows.go(rs: List<&2, UZ.URow>) -> List<&2, D.Node>:
  Uv.rows.go.loc(I.En{}, rs)

def Uv.rows.loc(+lc: I.Loc, rs: List<&2, UZ.URow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Uv.row.loc(lc, MSG.T.settings.usage.label(lc, UZ.URow.label(r)), r) <> Uv.rows.go.loc(lc, t)

def Uv.rows(rs: List<&2, UZ.URow>) -> List<&2, D.Node>:
  Uv.rows.loc(I.En{}, rs)

def Uv.controls(cs: List<&2, UP.UpControl>) -> List<&2, D.Node>:
  match cs:
    case Nil{}: Nil{}
    case UP.UpControl{+label, action, value, on} <> rest:
      D.Dom.button(Bool.pick(String, on, "on", ""), action, value, label, label) <> Uv.controls(rest)

def Uv.pagers(cs: List<&2, UP.UpControl>) -> List<&2, D.Node>:
  match cs:
    case Nil{}: Nil{}
    case +h <> +rest: [Uv.setrow("Breakdown pages", "", Uv.controls(h <> rest))]

def Uv.page(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  +cs = U.Usage.page.controls(u)
  List.concat(&2, D.Node, [[Uv.setrow("Period", "Rolling ranges include today (UTC); month = 30 days", Uv.controls(List.take(&2, UP.UpControl, cs, UP.Up.period.size()))),
    Uv.setrow("View", "", Uv.controls(List.take(&2, UP.UpControl, List.drop(&2, UP.UpControl, cs, UP.Up.period.size()), 3n)))],
    G.Graph.page(u), Uv.rows.loc(lc, U.Usage.page.rows(u)), Uv.pagers(List.drop(&2, UP.UpControl, cs, Nat.add(UP.Up.period.size(), 3n)))])
