import Base
import ./dom.bend as D
import ../core/client.bend as U
import ../core/model.bend as M
import ../core/usage_page.bend as UP
import ../core/usage_graph.bend as G

# A CSS column graph uses ordinary DOM nodes, including an exact data table.
# Its fixed plot height and bounded UTC day count reserve space on first paint.
def Graph.tabs(ps: List<&2, String>, +cur: String) -> List<&2, D.Node>:
  match ps:
    case Nil{}: Nil{}
    case +p <> rest:
      D.El{"button", "usage-tab-" ++ p, [D.At{"id", "usage-tab-" ++ p}, D.At{"role", "tab"}, D.At{"aria-selected", Bool.pick(String, String.eq(p, cur), "true", "false")},
        D.At{"aria-controls", "usage-daily"}, D.At{"tabindex", Bool.pick(String, String.eq(p, cur), "0", "-1")}, D.At{"data-value", p}, D.On{"click", "usage-provider"}], [D.Txt{p}]} <> Graph.tabs(rest, cur)

def Graph.bar(+date: String, +v: String, +max: Nat) -> D.Node:
  +n = UP.Up.tokens(v)
  +height = Nat.div(Nat.mul(n, 10000n), Bool.pick(Nat, Nat.is_eq(max, 0n), 1n, max))
  D.El{"div", date, [D.At{"class", "usage-column"}, D.At{"title", date ++ " · " ++ G.Ug.reading(v)}],
    [D.El{"div", "", [D.At{"class", "usage-bar" ++ Bool.pick(String, Bool.not(G.Ug.known(v)), " missing", Bool.pick(String, Nat.is_eq(n, 0n), " zero", ""))},
      D.At{"style", "height:calc(" ++ Nat.show(height) ++ " * 1% / 100)"}], Nil{}}]}

def Graph.bars(ds: List<&2, G.UgDay>, +max: Nat) -> List<&2, D.Node>:
  match ds:
    case Nil{}: Nil{}
    case G.UgDay{date, v} <> rest: Graph.bar(date, v, max) <> Graph.bars(rest, max)

def Graph.table.rows(ds: List<&2, G.UgDay>) -> List<&2, D.Node>:
  match ds:
    case Nil{}: Nil{}
    case G.UgDay{+date, v} <> rest:
      D.El{"tr", date, Nil{}, [D.Dom.el("th", [D.At{"scope", "row"}], [D.Txt{date}]), D.Dom.el("td", Nil{}, [D.Txt{G.Ug.reading(v)}])]} <> Graph.table.rows(rest)

def Graph.table(ds: List<&2, G.UgDay>) -> D.Node:
  D.Dom.el("details", [D.At{"class", "usage-data"}], [D.Dom.el("summary", Nil{}, [D.Txt{"Daily readings"}]),
    D.Dom.el("table", Nil{}, [D.Dom.el("caption", Nil{}, [D.Txt{"Recorded tokens by UTC day"}]),
      D.Dom.el("thead", Nil{}, [D.Dom.el("tr", Nil{}, [D.Dom.el("th", [D.At{"scope", "col"}], [D.Txt{"Day (UTC)"}]), D.Dom.el("th", [D.At{"scope", "col"}], [D.Txt{"Reading"}])])]),
      D.Dom.el("tbody", Nil{}, Graph.table.rows(ds))])])

def Graph.plot(+ds: List<&2, G.UgDay>, +u: U.Ui, +provider: String) -> D.Node:
  +now = U.Side.now(u)
  +period = U.Usage.page.period(u)
  D.El{"section", "usage-daily", [D.At{"id", "usage-daily"}, D.At{"role", "tabpanel"}, D.At{"aria-labelledby", "usage-tab-" ++ provider}, D.At{"tabindex", "0"}],
    [D.Dom.el("h3", Nil{}, [D.Txt{"Daily recorded tokens"}]),
     D.Dom.div("usage-caption", [D.Txt{M.Tok.day(G.Ug.start(now, period)) ++ " – " ++ M.Tok.day(now) ++ " · UTC · " ++ Nat.show(G.Ug.sum(ds, 0n)) ++ " recorded tokens"}]),
     D.Dom.div("usage-scale", [D.Txt{Nat.show(G.Ug.max(ds, 0n)) ++ " tokens"}]),
     D.Dom.el("div", [D.At{"class", "usage-plot"}, D.At{"role", "img"}, D.At{"aria-label", provider ++ " daily recorded tokens. Exact values and coverage in Daily readings."}],
       [D.Dom.el("div", [D.At{"class", "usage-bars"}, D.At{"aria-hidden", "true"}], Graph.bars(ds, G.Ug.max(ds, 0n)))]),
     D.Dom.div("usage-axis", [D.Dom.span("", M.Tok.day(G.Ug.start(now, period))), D.Dom.span("", M.Tok.day(now))]),
     D.Dom.div("usage-caption", [D.Txt{"Solid: recorded tokens · hatched: no readings · baseline: recorded zero"}]),
     Graph.table(ds), D.Dom.el("p", [D.At{"class", "usage-caption"}], [D.Txt{G.Ug.coverage(U.Ui.state.of(u))}])]}

def Graph.provider(have: Bool, +u: U.Ui, +provider: String) -> List<&2, D.Node>:
  match have:
    case True{}: [Graph.plot(G.Ug.days(U.Ui.state.of(u), U.Side.now(u), U.Usage.page.period(u), provider), u, provider)]
    case False{}: [D.Dom.div("usage-empty", [D.Txt{Bool.pick(String, String.is_empty(U.Ui.info(u, "models")), "Provider availability is loading. Cached usage remains below.", "No enabled providers on this machine.")}])]

def Graph.ranked(ps: List<&2, G.UgProject>, +rank: Nat, +max: Nat) -> List<&2, D.Node>:
  match ps:
    case Nil{}: Nil{}
    case G.UgProject{+id, +title, +n} <> rest:
      D.El{"li", "usage-project-" ++ id, Nil{}, [D.Dom.div("usage-project-line", [D.Dom.span("", Nat.show(rank) ++ ". " ++ title), D.Dom.span("usage-project-tokens", Nat.show(n) ++ " tokens")]),
        D.Dom.div("usage-project-id", [D.Txt{Bool.pick(String, String.is_empty(id), "No project identity", id)}]),
        D.Dom.div("usage-project-track", [D.Dom.el("div", [D.At{"style", "width:calc(" ++ Nat.show(Nat.div(Nat.mul(n, 10000n), Bool.pick(Nat, Nat.is_eq(max, 0n), 1n, max))) ++ " * 1% / 100)"}], Nil{})])]} <> Graph.ranked(rest, Nat.add(rank, 1n), max)

def Graph.top.k(ps: List<&2, G.UgProject>) -> List<&2, D.Node>:
  match ps:
    case Nil{}: [D.Dom.div("usage-empty", [D.Txt{"No project token readings in this period."}])]
    case +h <> +rest: [D.Dom.el("ol", [D.At{"class", "usage-projects"}], Graph.ranked(h <> rest, 1n, G.UgProject.tokens(h)))]

def Graph.top(+u: U.Ui) -> D.Node:
  +st = U.Ui.state.of(u)
  D.Dom.el("section", [D.At{"class", "usage-top"}, D.At{"aria-label", "Top 5 projects by token consumption"}], [D.Dom.el("h3", Nil{}, [D.Txt{"Top 5 projects"}]),
    D.Dom.el("p", [D.At{"class", "usage-caption"}], [D.Txt{UP.Up.range(U.Side.now(u), U.Usage.page.period(u)) ++ " · recorded tokens across all models and providers. Unreported usage is unknown." ++
      Bool.pick(String, Bool.not(String.eq(U.Usage.page.period(u), "month")), " Daily project readings share the daily coverage shown above.", "")}]),
    D.Dom.div("", Graph.top.k(G.Ug.projects(st, U.Ui.projects(st), U.Side.now(u), U.Usage.page.period(u))))])

def Graph.visible(show: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match show:
    case False{}: Nil{}
    case True{}:
      +provider = U.Usage.page.provider(u)
      [D.Dom.div("usage-dashboard", List.concat(&2, D.Node, [[D.Dom.el("div", [D.At{"class", "usage-tabs"}, D.At{"role", "tablist"}, D.At{"aria-label", "Usage provider"}], Graph.tabs(U.Usage.page.providers(u), provider))],
        Graph.provider(Bool.not(String.is_empty(provider)), u, provider), [Graph.top(u)]]))]

def Graph.page(+u: U.Ui) -> List<&2, D.Node>:
  Graph.visible(String.eq(U.Usage.page.metric(u), "tokens"), u)
