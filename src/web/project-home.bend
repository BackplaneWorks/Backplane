import Base
import ../core/json.bend as J
import ../core/project-home.bend as PH
import ../core/client.bend as U
import ../core/model.bend as M
import ../core/subthreads.bend as ST
import ../core/subs.bend as SB
import ../core/signal.bend as SG
import ../core/codec.bend as C
import ../core/images.bend as IM
import ./dom.bend as D
import ./composer-bar.bend as CB
import ../core/badge.bend as BG
import ../core/draftbound.bend as DB
import ./icon.bend as WI

# The home renders a bounded, saved contents snapshot. It performs no discovery.
# Only the active label owns the shared rename field. Composer state is separate.
def HomeView.name.edit(on: Bool, +u: U.Ui, +kind: String, +id: String, title: String) -> D.Node:
  match on:
    case True{}:
      D.El{"div", "", [D.At{"class", "home-inline-edit"}], [
        D.El{"input", "thread-title", [D.At{"id", "thread-title"}, D.At{"aria-label", "Display name"}, D.At{"value", U.Ui.scratch(u, "@f.title")}, D.At{"data-bfield", "title"}, D.At{"data-value", id}, D.At{"data-pre", id ++ "\u{1f}"}, D.At{"data-enter", "rename-save"}, D.At{"data-esc", "rename-no"}, D.On{"input", "bfield"}], Nil{}},
        D.El{"button", "", [D.At{"data-value", id}, D.At{"data-space", id}, D.At{"data-field", "thread-title"}, D.On{"click", "rename-save"}], [D.Txt{"Save"}]}, D.Dom.button("", "rename-no", "", "Cancel name edit", "Cancel")]}
    case False{}:
      D.Dom.button("home-edit-name", "home-rename", J.Json.show(J.Json.obj([J.KV{"kind", J.Str{kind}}, J.KV{"id", J.Str{id}}])), "Edit display name", title)

def HomeView.name(+u: U.Ui, +kind: String, +id: String, title: String) -> D.Node:
  HomeView.name.edit(Bool.and(String.eq(U.Ui.scratch(u, "@ren.owner"), U.Ui.sel(u)), Bool.and(String.eq(U.Ui.scratch(u, "@ren.kind"), kind), String.eq(U.Ren.on(u), id))), u, kind, id, title)

def HomeView.named.section(+u: U.Ui, +path: String, title: String, children: List<&2, D.Node>) -> D.Node:
  D.El{"section", "", [D.At{"class", "home-card"}], D.El{"h2", "", Nil{}, [HomeView.name(u, "artifact", path, U.Asset.name(u, path, title))]} <> children}

def HomeView.section(+title: String, children: List<&2, D.Node>) -> D.Node:
  D.El{"section", "", [D.At{"class", "home-card"}], D.El{"h2", "", Nil{}, [D.Txt{title}]} <> children}

def HomeView.view(+kind: String, +label: String) -> D.Node:
  D.Dom.button("", "home-view", kind, label, label)

def HomeView.image(none: Bool, path: String, title: String) -> List<&2, D.Node>:
  match none:
    case True{}: Nil{}
    case False{}: [D.El{"img", "", [D.At{"class", "home-thumbnail"}, D.At{"src", IM.Img.url(path)}, D.At{"alt", title}, D.At{"loading", "lazy"}, D.At{"decoding", "async"}], Nil{}}]

def HomeView.check(+title: String, +path: String, +summary: String) -> D.Node:
  +r = Maybe.default(&2, J.Json, J.Json.parse(summary), J.Null{})
  +counts = Nat.show(J.Json.get_nat(r, "errors", 0n)) ++ " errors · " ++ Nat.show(J.Json.get_nat(r, "warnings", 0n)) ++ " warnings · " ++ Nat.show(J.Json.get_nat(r, "unconnected", 0n)) ++ " unconnected"
  HomeView.section(title, [D.Dom.button("home-status", "home-document", path, "Inspect this saved check report", counts),
    D.Dom.span("home-file", "Saved report · " ++ J.Json.get_str(r, "date", "") ++ Bool.pick(String, J.Json.get_bool(r, "stale", True{}), " · source changed", ""))])

def HomeView.feature(value: Maybe<&2, J.Json>, +label: String, +unit: String) -> List<&2, D.Node>:
  match value:
    case Some{J.Num{n}}: [D.Dom.span("home-feature", label ++ n ++ unit)]
    case _: Nil{}

def HomeView.features(+f: J.Json) -> List<&2, D.Node>:
  List.concat(&2, D.Node, [HomeView.feature(J.Json.get(f, "width_mm"), "Width ", " mm"), HomeView.feature(J.Json.get(f, "height_mm"), "Height ", " mm"), HomeView.feature(J.Json.get(f, "min_drill_mm"), "Minimum drill ", " mm"),
    HomeView.feature(J.Json.get(f, "size_x_mm"), "X ", " mm"), HomeView.feature(J.Json.get(f, "size_y_mm"), "Y ", " mm"), HomeView.feature(J.Json.get(f, "size_z_mm"), "Z ", " mm"),
    HomeView.feature(J.Json.get(f, "volume_cm3"), "Volume ", " cm³"), HomeView.feature(J.Json.get(f, "area_cm2"), "Area ", " cm²"),
    HomeView.feature(J.Json.get(f, "flat_x_mm"), "Flat X ", " mm"), HomeView.feature(J.Json.get(f, "flat_y_mm"), "Flat Y ", " mm"), HomeView.feature(J.Json.get(f, "cut_length_mm"), "Cut length ", " mm")])

def HomeView.fab(+path: String, summary: String) -> D.Node:
  +r = Maybe.default(&2, J.Json, J.Json.parse(summary), J.Null{})
  +f = J.Json.get_obj(r, "features")
  HomeView.section("Fab readiness", [D.Dom.span("home-file", Bool.pick(String, J.Json.get_bool(r, "order", False{}), "Included in order plan; release checks still apply", "Order excluded")),
    D.El{"div", "", [D.At{"class", "home-features"}], HomeView.features(f)},
    D.El{"p", "", Nil{}, [D.Txt{J.Json.get_str(r, "engineering_status", "Saved fabrication options; release status unverified.")}]},
    D.Dom.button("", "home-document", path, "Inspect saved fab metadata", "Details"), D.Dom.button("", "home-price", "", "Estimate using saved measured features and current pricing settings", "Estimate price")])

def HomeView.artifact(+u: U.Ui, kind: String, +path: String, +title: String, +summary: String, +row: J.Json) -> D.Node:
  match kind:
    case "board": HomeView.named.section(u, path, title, [D.Dom.span("home-file", path), D.El{"div", "", [D.At{"class", "home-card-actions"}], List.append(&2, D.Node, [HomeView.view("board", "Board"), HomeView.view("3d", "3D")], D.Dom.when(PH.Home.can_split(U.Home.contents(u)), HomeView.view("split", "Split")))}])
    case "schematic": HomeView.section(title, [HomeView.view("schematic", "Open schematic"), D.Dom.button("", "home-bom", "", "Inspect components in the existing BOM viewer", "Inspect BOM")])
    case "drc": HomeView.check(title, path, summary)
    case "erc": HomeView.check(title, path, summary)
    case "fab": HomeView.fab(path, summary)
    case "mech":
      HomeView.named.section(u, path, title, List.append(&2, D.Node, HomeView.image(String.is_empty(J.Json.get_str(row, "thumbnail", "")), J.Json.get_str(row, "thumbnail", ""), title),
        [D.Dom.button("", "home-model", path, "Open this model in Mech", "Open in Mech")]))
    case "prs": HomeView.section(title, D.Md.render(String.take(summary, 1200n)))
    case "outside": HomeView.section(title, D.Md.render(String.take(summary, 2400n)))
    case "changes": HomeView.section(title, [D.El{"pre", "", Nil{}, [D.Txt{summary}]}])
    case _: HomeView.section(title, [D.El{"div", "", [D.At{"class", "body"}], D.Md.render(String.take(summary, 1200n))}, D.Dom.button("", "home-document", path, "Read the full file", "Read file")])

def HomeView.panel.keep(+u: U.Ui, skip: Bool, panel: PH.Panel, +c: J.Json) -> List<&2, D.Node>:
  match skip panel:
    case True{} _: Nil{}
    case False{} PH.Panel{k, +p, t, s}: [HomeView.artifact(u, k, p, t, s, PH.Home.row(c, p))]

def HomeView.panels(+u: U.Ui, ps: List<&2, PH.Panel>, +c: J.Json, +embedded: Bool) -> List<&2, D.Node>:
  match ps:
    case Nil{}: Nil{}
    case PH.Panel{+k, p, t, s} <> rest:
      List.append(&2, D.Node, HomeView.panel.keep(u, Bool.or(String.eq(k, "intent"), Bool.and(embedded, Bool.or(String.eq(k, "board"), String.eq(k, "schematic")))), PH.Panel{k, p, t, s}, c), HomeView.panels(u, rest, c, embedded))

# One hierarchy for web/desktop and the phones' native home cards.
# The branch marker is text too, so native renderers retain the relationship.
def HomeView.thread.at(+u: U.Ui, +t: M.Thread, +depth: Nat) -> D.Node:
  match t:
    case M.Thread{+id, _, title, _, _, turn, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      +indent = Nat.min(depth, 4n)
      +badge = SB.Subs.badge(SB.Subs.count(U.Ui.state(u), id))
      D.El{"div", id, [D.At{"class", "home-thread"}, D.At{"data-depth", Nat.show(depth)}, D.At{"style", "padding-inline-start:" ++ Nat.show(Nat.mul(indent, 16n)) ++ "px"}],
        [HomeView.name(u, "thread", id, String.repeat("↳ ", indent) ++ title),
         D.Dom.button("home-thread-state", "select", id, "Open thread", SG.Signal.display.name(U.Side.signal(u, t), U.Ui.state(u), id) ++ Bool.pick(String, String.is_empty(badge), "", " · " ++ badge) ++ " →")]}

def HomeView.thread(+u: U.Ui, t: M.Thread) -> D.Node:
  HomeView.thread.at(u, t, 0n)

def HomeView.thread.live(t: M.Thread, +project: String) -> Bool:
  match t:
    case M.Thread{_, p, _, _, _, _, _, archived, _, _, _, _, _, _, _, _, _, _, _, _, deleted}:
      Bool.and(String.eq(p, project), Bool.not(Bool.or(archived, deleted)))

def HomeView.trows(+u: U.Ui, rows: List<&2, ST.TRow>) -> List<&2, D.Node>:
  match rows:
    case Nil{}: Nil{}
    case ST.TRow{t, depth, _, _} <> rest:
      HomeView.thread.at(u, t, depth) <> HomeView.trows(u, rest)

# Rank roots as before, then traverse each subtree once. Home shows every
# child even when its sidebar branch is collapsed. Page the flattened rows.
def HomeView.thread.rows(+s: M.State, ts: List<&2, M.Thread>) -> List<&2, ST.TRow>:
  ST.Tree.rows.k(ST.Tree.roots.go(ts, s, Nil{}), s, "", True{})

def HomeView.thread.project(ts: List<&2, M.Thread>, +project: String) -> List<&2, M.Thread>:
  match ts:
    case Nil{}: Nil{}
    case +t <> rest: ST.Tree.roots.add(Bool.not(HomeView.thread.live(t, project)), t, HomeView.thread.project(rest, project))

def HomeView.threads(+u: U.Ui, ts: List<&2, M.Thread>, +project: String, skip: Nat, left: Nat) -> List<&2, D.Node>:
  HomeView.trows(u, List.take(&2, ST.TRow, List.drop(&2, ST.TRow, HomeView.thread.rows(U.Ui.state(u), HomeView.thread.project(ts, project)), skip), left))

# Rank with the same urgency/pinning/recency decisions as the sidebar.
def HomeView.ordered(rows: List<&2, U.ARow>) -> List<&2, M.Thread>:
  match rows:
    case Nil{}: Nil{}
    case U.ARow{_, _, t, _, _, _} <> rest: t <> HomeView.ordered(rest)

def HomeView.ranked(+u: U.Ui, +project: String) -> List<&2, M.Thread>:
  HomeView.ordered(List.sort(~U.ARow, ~U.ARow.before,
    U.Active.live(M.Thread.filter_project(U.Ui.threads(U.Ui.state(u)), project), U.Ui.state(u), project, "", Nil{})))

def HomeView.intent.edit(edit: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match edit:
    case False{}: [D.Dom.button("", "home-intent", "", "Edit goal, parts and notes", "Edit goal & notes")]
    case True{}:
      [D.El{"textarea", "home-intent", [D.At{"class", "home-intent-field"}, D.At{"aria-label", "Goal, parts and notes"}, D.At{"data-bfield", "home-intent:" ++ U.Ui.sel(u)}, D.At{"value", U.Ui.scratch(u, "@f.home-intent:" ++ U.Ui.sel(u))}, D.On{"input", "bfield"}], Nil{}},
       D.Dom.button("primary", "home-save", "", "Save goal and notes", "Save"), D.Dom.button("", "home-cancel", "", "Cancel editing", "Cancel")]

def HomeView.cursor.button(action: String, n: String, +label: String) -> List<&2, D.Node>:
  match action:
    case "home-more": [D.Dom.button("", "home-more", n, label, label)]
    case "home-scan": [D.Dom.button("", "home-scan", n, label, label)]
    case "home-edit-page": [D.Dom.button("", "home-edit-page", n, label, label)]
    case "home-document-more": [D.Dom.button("", "home-document-more", n, label, label)]
    case _: Nil{}

def HomeView.cursor.value(value: Maybe<&2, J.Json>, action: String, +label: String) -> List<&2, D.Node>:
  match value:
    case Some{J.Num{n}}: HomeView.cursor.button(action, n, label)
    case _: Nil{}

def HomeView.cursor(c: J.Json, key: String, action: String, label: String) -> List<&2, D.Node>:
  HomeView.cursor.value(J.Json.get(c, key), action, label)

def HomeView.more(+u: U.Ui, +c: J.Json) -> List<&2, D.Node>:
  List.append(&2, D.Node, HomeView.cursor(c, "next", "home-more", "More parts"), List.append(&2, D.Node, HomeView.cursor(c, "scanNext", "home-scan", "Continue discovery"), Bool.pick(List<&2, D.Node>, String.eq(U.Ui.scratch(u, "@home.edit:" ++ U.Ui.sel(u)), "1"), Nil{}, HomeView.cursor(c, "intentNext", "home-edit-page", "Next notes section"))))

# Use the existing one-button-high control row. New thread defaults are edited
# by the existing agent Settings page; no model RPC targets a project owner.
def HomeView.composer(+u: U.Ui, +project: String) -> D.Node:
  D.El{"div", "composer", [D.At{"class", "composer home-composer"}, D.At{"data-home-composer", "1"}], [
    D.El{"textarea", "composer-field", [D.At{"id", "composer"}, D.At{"aria-label", "Start a thread about this project"}, D.At{"placeholder", "Start a thread about this project…"}, D.At{"value", U.Ui.draft_text(u)}, D.At{"data-enter", "send"}, D.On{"input", "draft"}], Nil{}},
    CB.ComposerBar.node([D.Dom.button("model-chip", "settings-page", "agents", "Choose defaults for new threads", "New thread defaults"),
      CB.ComposerBar.more([D.Dom.button("", "new-thread", project, "Create an empty thread", "Empty thread")]),
      D.El{"div", "", [D.At{"class", "foot-end"}], [D.Dom.button("primary", "send", "", "Start a new thread with this message", "Start thread")]}])]}

def HomeView.header.name(+u: U.Ui, +project: String) -> D.Node:
  D.El{"div", "", [D.At{"class", "home-project-name"}], [
    D.El{"button", "", [D.At{"class", "home-project-icon"}, D.At{"style", "color:" ++ BG.Badge.css_of(U.Ui.state(u), project)}, D.At{"title", "Change project icon"}, D.At{"data-value", project}, D.On{"click", "look"}], [WI.WIcon.svg(BG.Badge.icon(U.Ui.state(u), project), 24)]},
    D.El{"div", "", Nil{}, [D.Dom.span("home-eyebrow", "PROJECT HOME"), D.El{"h1", "", Nil{}, [HomeView.name(u, "project", project, U.Ui.project_title(U.Ui.state(u), project))]}]}]}

def HomeView.header(+u: U.Ui, +project: String) -> D.Node:
  D.El{"header", "", [D.At{"class", "home-header"}], [
    D.Dom.button("back", "select", "", "Back to projects", "‹ Projects"), HomeView.header.name(u, project),
    D.Dom.button("", "home-refresh", "", "Refresh saved project contents", "Refresh")]}

def HomeView.thread.card(+u: U.Ui, +project: String) -> D.Node:
  +offset = Maybe.default(&2, Nat, Nat.read(U.Ui.scratch(u, "@home.threads")), 0n)
  +rows = HomeView.threads(u, HomeView.ranked(u, project), project, offset, 33n)
  +previous = Bool.pick(List<&2, D.Node>, Nat.is_eq(offset, 0n), Nil{},
    [D.Dom.button("", "home-threads", Nat.show(Nat.sub(offset, 32n)), "Previous threads", "Previous")])
  +next = Bool.pick(List<&2, D.Node>, Nat.is_gt(List.length(&2, D.Node, rows), 32n),
    [D.Dom.button("", "home-threads", Nat.show(Nat.add(offset, 32n)), "More project threads", "More threads")], Nil{})
  HomeView.section("Threads", List.concat(&2, D.Node, [List.take(&2, D.Node, rows, 32n), previous, next]))

# Document continuation never replaces the artifact picture or the composer.
def HomeView.document.shown(show: Bool, +doc: J.Json) -> List<&2, D.Node>:
  match show:
    case False{}: Nil{}
    case True{}:
      [HomeView.section(J.Json.get_str(doc, "path", "Saved document"), List.concat(&2, D.Node, [
        [D.El{"pre", "", [D.At{"class", "home-document-body"}], [D.Txt{J.Json.get_str(doc, "text", J.Json.get_str(doc, "error", ""))}]}],
        HomeView.cursor(doc, "next", "home-document-more", "Next section"),
        [D.Dom.button("", "home-document-close", "", "Close this document", "Close document")]]))]

def HomeView.document(+u: U.Ui) -> List<&2, D.Node>:
  +r = Maybe.default(&2, J.Json, J.Json.parse(U.Ui.info(u, "home.document")), J.Null{})
  HomeView.document.shown(String.eq(J.Json.get_str(r, "project_home", ""), PH.Home.project(U.Ui.sel(u))), J.Json.get_obj(r, "document"))

def HomeView.intent.shown(has: Bool, +u: U.Ui, +c: J.Json) -> List<&2, D.Node>:
  match has:
    case True{}: [HomeView.section("Goal & notes", List.append(&2, D.Node, D.Md.render(J.Json.get_str(c, "intent", "")), HomeView.intent.edit(String.eq(U.Ui.scratch(u, "@home.edit:" ++ U.Ui.sel(u)), "1"), u)))]
    case False{}: HomeView.intent.edit(String.eq(U.Ui.scratch(u, "@home.edit:" ++ U.Ui.sel(u)), "1"), u)

# The home viewer offers only destinations backed by this contents snapshot.
def HomeView.viewer.tab(+u: U.Ui, +kind: String, +label: String) -> D.Node:
  +active = String.eq(U.Ui.view_get(u, "view.kind"), kind)
  D.El{"button", "", [D.At{"class", Bool.pick(String, active, "tab on", "tab")}, D.At{"aria-pressed", Bool.pick(String, active, "true", "false")},
    D.At{"data-value", kind}, D.On{"click", "home-view"}], [D.Txt{label}]}

def HomeView.viewer.tabs(+u: U.Ui) -> D.Node:
  +c = U.Home.contents(u)
  D.El{"nav", "", [D.At{"class", "viewer-tabs"}], List.concat(&2, D.Node, [
    D.Dom.when(Bool.not(String.is_empty(PH.Home.path(c, "board"))), HomeView.viewer.tab(u, "board", "Board")),
    D.Dom.when(Bool.not(String.is_empty(PH.Home.path(c, "schematic"))), HomeView.viewer.tab(u, "schematic", "Schematic")),
    D.Dom.when(Bool.not(String.is_empty(PH.Home.path(c, "board"))), HomeView.viewer.tab(u, "3d", "3D")),
    D.Dom.when(PH.Home.can_split(c), HomeView.viewer.tab(u, "split", "Split")),
    D.Dom.when(Bool.not(String.is_empty(PH.Home.path(c, "schematic"))), D.Dom.button("", "home-bom", "", "Inspect components in the existing BOM viewer", "BOM")),
    D.Dom.when(Bool.not(String.is_empty(PH.Home.path(c, "mech"))), D.Dom.button("", "home-model", PH.Home.path(c, "mech"), "Open saved mechanical part", "Mech")),
    [D.Dom.button("", "home-viewer-size", Bool.pick(String, String.eq(U.Ui.scratch(u, "@home.viewer.full:" ++ U.Ui.sel(u)), "1"), "", "1"),
      "Keep this saved view and camera while changing its size", Bool.pick(String, String.eq(U.Ui.scratch(u, "@home.viewer.full:" ++ U.Ui.sel(u)), "1"), "Back to project", "Open viewer")),
     D.Dom.button("", "panel", "", "Close viewer", "Close")]])}

def HomeView.price(shown: Bool, +price: J.Json) -> List<&2, D.Node>:
  match shown:
    case False{}: Nil{}
    case True{}: [HomeView.section("Price estimate", [D.Dom.span("", Bool.pick(String, String.is_empty(J.Json.get_str(price, "label", "")), "No estimate returned; check included parts and pricing settings", J.Json.get_str(price, "label", "")) ++ " " ++ J.Json.get_str(price, "error", "") ++ " " ++ J.Json.get_str(price, "estimate_problems", ""))])]

def HomeView.viewer.title.kind(editable: Bool, +u: U.Ui, +path: String, title: String) -> D.Node:
  match editable:
    case True{}: HomeView.name(u, "artifact", path, U.Asset.name(u, path, title))
    case False{}: D.Dom.span("home-view-title", title)

def HomeView.viewer.title(+u: U.Ui, +c: J.Json) -> D.Node:
  +path = U.Ui.view_get(u, "view.path")
  +row = PH.Home.row(c, path)
  +kind = J.Json.get_str(row, "kind", "")
  HomeView.viewer.title.kind(Bool.or(String.eq(kind, "board"), String.eq(kind, "mech")), u, path,
    J.Json.get_str(row, "title", Bool.pick(String, String.eq(U.Ui.view_get(u, "view.kind"), "review"), "Design review", "Saved view")))

def HomeView.warning(+u: U.Ui) -> String:
  +limit = DB.Bound.say(U.Lang.loc(u), U.Ui.scratch(u, "@draft.bound"))
  Bool.pick(String, String.is_empty(limit), U.Ui.scratch(u, "@home.error"), limit)

def HomeView.grid(+u: U.Ui, +project: String, +c: J.Json, +viewer: List<&2, D.Node>) -> D.Node:
  +generic = List.append(&2, D.Node, HomeView.intent.shown(Bool.not(String.is_empty(PH.Home.path(c, "intent"))), u, c), [HomeView.thread.card(u, project)])
  +embedded = Bool.not(List.is_empty(&2, D.Node, viewer))
  +details: D.Node = D.El{"div", "home-details", [D.At{"id", "home-details"}, D.At{"class", "home-grid home-details"}],
    List.concat(&2, D.Node, [HomeView.document(u), HomeView.price(Bool.and(Bool.not(String.is_empty(PH.Home.path(c, "fab"))), Maybe.is_some(&2, J.Json, J.Json.get(c, "price"))), J.Json.get_obj(c, "price")), generic, HomeView.panels(u, PH.Home.panels(c), c, embedded)])}
  D.El{"div", "home-layout", [D.At{"class", "home-layout"}, D.At{"data-split-project", project}], List.append(&2, D.Node,
    Bool.pick(List<&2, D.Node>, embedded,
      [D.El{"section", "home-live-viewer", [D.At{"id", "home-live-viewer"}, D.At{"class", "home-live-viewer"}], D.El{"h2", "", Nil{}, [HomeView.viewer.title(u, c)]} <> viewer},
       D.El{"div", "home-resize", [D.At{"class", "home-resize combo-resize"}, D.At{"data-project-split", "home"}, D.At{"role", "separator"}, D.At{"tabindex", "0"}, D.At{"aria-label", "Resize viewer and project details"}, D.At{"aria-controls", "home-live-viewer home-details"}, D.At{"data-first-label", "Viewer"}, D.At{"data-second-label", "Project details"}], Nil{}}], Nil{}), [details])}

def HomeView.scroll(+u: U.Ui, +project: String, +c: J.Json, viewer: List<&2, D.Node>) -> D.Node:
  D.El{"div", "", [D.At{"class", "home-scroll"}], [
    D.Dom.span("home-error", HomeView.warning(u)), HomeView.grid(u, project, c, viewer),
    D.El{"div", "", [D.At{"class", "home-card-actions"}], List.append(&2, D.Node, D.Dom.when(Nat.is_gt(J.Json.get_nat(c, "after", 0n), 0n), D.Dom.button("", "home-more", "0", "Return to the first parts window", "First parts")), HomeView.more(u, c))}]}

def HomeView.node.picker(+u: U.Ui, viewer: List<&2, D.Node>, picker: List<&2, D.Node>) -> D.Node:
  +project = PH.Home.project(U.Ui.sel(u))
  D.El{"main", "project-home", [D.At{"class", "thread-view project-home"}, D.At{"data-project-home", project}, D.At{"data-thread", U.Ui.sel(u)},
    D.At{"data-home-viewer-full", Bool.pick(String, Bool.and(U.Home.viewer.allowed(u), String.eq(U.Ui.scratch(u, "@home.viewer.full:" ++ U.Ui.sel(u)), "1")), "1", "0")}], [
    HomeView.header(u, project), D.El{"div", "", [D.At{"class", "home-icon-picker"}], picker}, HomeView.scroll(u, project, U.Home.contents(u), viewer), HomeView.composer(u, project)]}

def HomeView.node(+u: U.Ui, viewer: List<&2, D.Node>) -> D.Node:
  HomeView.node.picker(u, viewer, Nil{})
