import Base
import ../core/i18n.bend as I
import ../core/msgs.bend as MSG
import ../core/json.bend as J
import ../core/far.bend as FR
import ../core/client.bend as U
import ../app/viewer.bend as V
import ../core/look.bend as LO
import ../core/hier.bend as HI
import ../core/board.bend as B
import ../core/schematic.bend as SC
import ../core/mech.bend as MC
import ../core/hist.bend as HS
import ../core/fm.bend as FM
import ../core/images.bend as IM
import ../core/layers.bend as LY
import ../core/review.bend as RVW

# The phones' board viewer, as the Bend client decides it. The viewer
# shows the selected thread's project board, schematic or 3D model
# ("@view:<thread>": "board", "schematic", "3d", "" closed; each thread
# keeps its own). The hub is asked for exactly the source on
# screen (kicad.watch), once per change of source, and again after a
# reconnect (a new connection watches nothing). Plots (core/plot.bend)
# come straight to the phone's renderer, not through here; one whose key
# is not the screen's viewer key is stale and never shown.

def View.known(+kind: String) -> Bool:
  Bool.or(String.eq(kind, "board"), Bool.or(String.eq(kind, "schematic"), String.eq(kind, "3d")))

def View.num(+kind: String) -> String:
  Bool.pick(String, String.eq(kind, "schematic"), "1", Bool.pick(String, String.eq(kind, "3d"), "2", "0"))

# the source key a plot for kind and root carries ("" when none)
def View.key(+kind: String, +root: String) -> String:
  Bool.pick(String, Bool.or(Bool.or(String.is_empty(root), String.is_empty(kind)), Bool.not(View.known(kind))), "",
    View.num(kind) ++ "|" ++ root ++ "|")

# the plot whose chunks the view draws: a 3D view draws the board's
# layers on its faces ("" for none)
def View.layers(+kind: String, +root: String) -> String:
  View.key(Bool.pick(String, String.eq(kind, "3d"), "board", kind), root)

# each thread's viewer is its own: "@view:<thread>"
def View.slot(+u: U.Ui) -> String:
  "@view:" ++ U.Ui.sel(u)

def View.kind(+u: U.Ui) -> String:
  U.Ui.scratch(u, View.slot(u))

# a file opened from the Files tab ("@view.file:<thread>", its absolute
# path): the board, schematic or model the viewer shows in place of the
# project's own until a view is picked by hand
def View.file(+u: U.Ui) -> String:
  U.Ui.scratch(u, "@view.file:" ++ U.Ui.sel(u))

# the Files tab and what it opened that is not a plot (an image, a PDF)
def View.fm(+u: U.Ui) -> Bool:
  Bool.or(String.eq(View.kind(u), "files"), Bool.or(String.eq(View.kind(u), "image"), String.eq(View.kind(u), "pdf")))

# The schematic's sheet ("@view.sheet:<thread>", "" for the root) and the
# 3D model's parts ("@view.bare" set: the board alone, the hub's kind 3)
# are part of the source: the key a plot carries, the hub's Src.key
def View.sheet(+u: U.Ui) -> String:
  U.Ui.scratch(u, "@view.sheet:" ++ U.Ui.sel(u))

def View.bare(+u: U.Ui) -> Bool:
  Bool.not(String.is_empty(U.Ui.scratch(u, "@view.bare")))

# The design history (client.bend's Hs): the client's view kind and path
# are the phone's board or schematic when a history action runs
# (View.hist.act), and while a version or a comparison of that kind shows,
# it is the source (View.src) in place of the project's file or sheet
def View.hist.num(+kind: String) -> Nat:
  Bool.pick(Nat, String.eq(kind, "schematic"), 1n, Bool.pick(Nat, String.eq(kind, "board"), 0n, 2n))

# (a version of the phone's kind of file: a schematic's version never
# goes to the board)
def View.hist(+u: U.Ui) -> Bool:
  Bool.and(Nat.is_lt(View.hist.num(View.kind(u)), 2n),
    Bool.and(Bool.not(Nat.is_eq(HS.Hist.kind(U.Ui.view_get(u, "view.path")), 0n)), Nat.is_eq(HS.Hist.ext(HS.Hist.family(U.Ui.view_get(u, "view.path"))), View.hist.num(View.kind(u)))))

# the client as the phone's viewer is: its kind the phone's
def View.as_client(+u: U.Ui) -> U.Ui:
  Bool.pick(U.Ui, Nat.is_lt(View.hist.num(View.kind(u)), 2n), U.Ui.view_get.set(u, "view.kind", View.kind(u)), u)

# the file the source names: a version while the history shows one, else
# a schematic's sheet ("" the project's own)
def View.src(+u: U.Ui) -> String:
  Bool.pick(String, View.hist(u), U.Ui.view_get(u, "view.path"), Bool.pick(String, Bool.not(String.is_empty(View.file(u))), View.file(u),
    Bool.pick(String, String.eq(View.kind(u), "schematic"), View.sheet(u), "")))

# a board view's own path: a version of the board, or a board file opened
# from the Files tab (never a schematic's sheet)
def View.board.path(+path: String) -> Bool:
  Bool.or(Bool.not(Nat.is_eq(HS.Hist.kind(path), 0n)), String.ends_with(path, ".kicad_pcb"))

def View.key.at(+kind: String, +root: String, +path: String, bare: Bool) -> String:
  Bool.pick(String, Bool.or(String.eq(kind, "schematic"), Bool.and(String.eq(kind, "board"), View.board.path(path))), View.key(kind, root) ++ path,
    Bool.pick(String, Bool.and(bare, Bool.not(String.is_empty(View.key(kind, root)))), "3|" ++ root ++ "|", View.key(kind, root)))

def View.layers.at(+kind: String, +root: String, +path: String) -> String:
  Bool.pick(String, Bool.or(String.eq(kind, "schematic"), Bool.and(String.eq(kind, "board"), View.board.path(path))), View.key.at(kind, root, path, False{}),
    Bool.pick(String, Bool.and(String.eq(kind, "3d"), String.ends_with(path, ".kicad_pcb")), "0|" ++ root ++ "|" ++ path, View.layers(kind, root)))

# Mech ("mech") shows the part on show in 3D, with the parts over it;
# Renders ("renders") its pictures (client.bend's Mpage decides the part)
def View.mech(+u: U.Ui) -> Bool:
  Bool.or(String.eq(View.kind(u), "mech"), String.eq(View.kind(u), "renders"))

def View.part.k(on: Bool, +u: U.Ui) -> String:
  match on:
    case True{}:
      Bool.pick(String, String.is_empty(U.Mpage.cur.path(u)), "", MC.Mech.abs(U.Mpage.root(u), U.Mpage.cur.path(u)))
    case False{}:
      ""

# the part in 3D (its absolute path), "" when Mech is not open or has none
def View.part(+u: U.Ui) -> String:
  View.part.k(String.eq(View.kind(u), "mech"), u)

def View.on_part(+u: U.Ui) -> Bool:
  Bool.not(String.is_empty(View.part(u)))

# the source the screen shows now
def View.want(+u: U.Ui) -> String:
  Bool.pick(String, Bool.and(View.on_part(u), Bool.not(String.is_empty(U.Ui.root(u)))), "2|" ++ U.Ui.root(u) ++ "|" ++ View.part(u),
    Bool.pick(String, Bool.and(String.eq(View.kind(u), "3d"), Bool.not(String.is_empty(View.file(u)))), "2|" ++ U.Ui.root(u) ++ "|" ++ View.file(u),
    View.key.at(View.kind(u), U.Ui.root(u), View.src(u), Bool.and(String.eq(View.kind(u), "3d"), View.bare(u)))))

# the file a source names: a version, a schematic's sheet, or Mech's part
def View.path(+u: U.Ui) -> String:
  Bool.pick(String, String.eq(View.kind(u), "mech"), View.part(u), View.src(u))

# (Mech's part is a 3D model to the hub)
def View.ask(+want: String, +kind: String, +root: String, +path: String) -> List<&2, J.KV>:
  [J.KV{"kind", J.Str{Bool.pick(String, String.is_empty(want), "", Bool.pick(String, String.starts_with(want, "3|"), "3d-bare", Bool.pick(String, String.eq(kind, "mech"), "3d", kind)))}}, J.KV{"root", J.Str{root}},
   J.KV{"path", J.Str{Bool.pick(String, Bool.or(Bool.or(String.eq(kind, "schematic"), String.eq(kind, "board")), Bool.or(String.eq(kind, "mech"), String.eq(kind, "3d"))), path, "")}}]

def View.join(a: U.Act, cmds: List<&2, U.Cmd>) -> U.Act:
  match a:
    case U.Act{u, cs}:
      U.Act{u, List.append(&2, U.Cmd, cmds, cs)}

def View.sync.k(same: Bool, u: U.Ui, +want: String, +kind: String, +root: String, +path: String, cmds: List<&2, U.Cmd>) -> U.Act:
  match same:
    case True{}:
      U.Act{u, cmds}
    case False{}:
      View.join(U.Ui.rpc(U.Ui.scratch_set(u, "@view.at", want), "kicad.watch", View.ask(want, kind, root, path)), cmds)

def View.sync.u(+u: U.Ui, cmds: List<&2, U.Cmd>) -> U.Act:
  View.sync.k(String.eq(View.want(u), U.Ui.scratch(u, "@view.at")), u, View.want(u), View.kind(u), U.Ui.root(u), View.path(u), cmds)

# an action's result with the hub asked for the source now on screen,
# when that changed (law view_asks_once)
# (and the Mechanical page's parts, when that is the phone's view: its
# own slot, kept across launches, which the client's view kind is not)
# A thread elsewhere's viewer needs its hub's cache of the owner's files
# (client.bend's Fv): the file on screen, from the owner's folder.
def View.fv.want(+u: U.Ui) -> String:
  Bool.pick(String, String.eq(View.kind(u), "mech"), U.Fv.rel.of(U.Ui.root(u), View.part(u)),
    Bool.pick(String, Bool.or(View.known(View.kind(u)), Bool.or(String.eq(View.kind(u), "image"), String.eq(View.kind(u), "pdf"))),
      Bool.pick(String, Bool.or(String.eq(View.kind(u), "image"), String.eq(View.kind(u), "pdf")), U.Fv.rel.of(U.Ui.root(u), U.Fv.t(u, "view.path")),
        Bool.pick(String, Nat.is_eq(HS.Hist.kind(View.src(u)), 0n), U.Fv.rel.of(U.Ui.root(u), View.src(u)), "")), ""))

def View.fv(a: U.Act) -> U.Act:
  match a:
    case U.Act{+u, cmds}:
      U.Fv.sync.on(Bool.and(FR.Fr.is(U.Ui.sel(u)), Bool.not(String.is_empty(View.kind(u)))), View.fv.want(u), U.Act{u, cmds})

def View.sync.m(a: U.Act) -> U.Act:
  match a:
    case U.Act{+u, cmds}:
      U.Mpage.sync.if(View.mech(u), View.sync.u(u, cmds))

def View.sync(a: U.Act) -> U.Act:
  View.fv(View.sync.m(a))

# a hub's answer to far.view: the viewer asks for the source again, now
# that the folder is known (and to history.resolve: the version it picked
# is the source now)
def View.far.k(far: Bool, a: U.Act) -> U.Act:
  match far a:
    case True{} +x:
      View.sync(x)
    case False{} +x:
      x

def View.far(+o: J.Json, a: U.Act) -> U.Act:
  View.far.k(Bool.or(U.Fm.has(J.Json.get_obj(o, "result"), "farView"), U.Rv.reply.has(o)), a)

# open a viewer ("board", "schematic", "3d") or close it (""); what was
# picked is let go
# (in a project with several boards, another tab stays on the board picked:
# client.bend's Bd.keep)
def View.open.k(+u: U.Ui, +v: String) -> U.Act:
  View.sync(U.Act{U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.scratch_set(u, "@view.file:" ++ U.Ui.sel(u), U.Bd.keep(u, v, U.Bd.shown(u, View.file(u)))), View.slot(u), v), "@pick", ""), "@card", ""), "@pick.rid", ""), Nil{}})

# The Files tab (client.bend's Fm): its actions are the client's; the
# phone's view follows them: the folder ("files"), or a file opened in the
# view its kind takes (a board, schematic or model is the source; an image
# or a PDF is shown by the app)
def View.fm.to(a: U.Act, +slot: String, +file: String) -> U.Act:
  match a:
    case U.Act{+u, cs}:
      View.sync(U.Act{U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.scratch_set(u, "@view.file:" ++ U.Ui.sel(u), file), View.slot(u), slot), "@pick", ""), "@card", ""), cs})

def View.fm.open(+u: U.Ui, +rel: String) -> U.Act:
  Bool.pick(U.Act, Bool.or(String.is_empty(FM.Fm.kind(rel)), String.is_empty(U.Ui.root(u))), U.Act{u, Nil{}},
    View.fm.to(U.Ui.act(u, "fm-open", rel), FM.Fm.kind(rel), Bool.pick(String, Bool.or(String.eq(FM.Fm.kind(rel), "image"), String.eq(FM.Fm.kind(rel), "pdf")), "", FM.Fm.abs(U.Ui.root(u), rel))))

def View.fm.act(+u: U.Ui, +action: String, +value: String) -> U.Act:
  Bool.pick(U.Act, String.eq(action, "fm-open"), View.fm.open(u, value), View.fm.to(U.Ui.act(u, action, value), "files", ""))

# going back to the thread shuts the phone's viewer over it
def View.hist.jump(jump: Bool, a: U.Act) -> U.Act:
  match jump a:
    case True{} U.Act{u, cs}:
      View.join(View.open.k(u, ""), cs)
    case False{} _:
      View.sync(a)

# a step's row in the thread opens the phone's viewer on it (its board or
# its schematic, as Hs.show picked)
def View.hist.show(show: Bool, a: U.Act) -> U.Act:
  match show a:
    case True{} U.Act{+u, cs}:
      View.join(View.open.k(u, Bool.pick(String, String.eq(U.Ui.view_get(u, "view.kind"), "schematic"), "schematic", "board")), cs)
    case False{} _:
      a

def View.hist.act.k(+u: U.Ui, +action: String, +value: String) -> U.Act:
  View.hist.jump(String.eq(action, "hist-jump"), View.hist.show(String.eq(action, "hist-show"), U.Ui.act(View.as_client(u), action, value)))

# The Review tab (client.bend's Rp): "rv-page" and the compare bar's
# "rv-bom" show a page, and the phone's view follows to "review" (no
# source to watch: View.key is empty for it)
def View.rv.to(a: U.Act) -> U.Act:
  match a:
    case U.Act{+u, cs}:
      View.join(View.open.k(u, "review"), cs)

def View.rv.shows(+action: String) -> Bool:
  Bool.or(String.eq(action, "rv-page"), String.eq(action, "rv-bom"))

def View.hist.act.r(shows: Bool, +u: U.Ui, +action: String, +value: String) -> U.Act:
  match shows:
    case True{}:
      View.rv.to(U.Ui.act(u, action, value))
    case False{}:
      View.hist.act.k(u, action, value)

def View.hist.act(+u: U.Ui, +action: String, +value: String) -> U.Act:
  View.hist.act.r(View.rv.shows(action), u, action, value)

# (the Mechanical page, "mech", is the client's: its view is set there,
# and the parts asked for; a view picked by hand shows the project's own
# board, not a part)
def View.open.mech(mech: Bool, a: U.Act) -> U.Act:
  match mech a:
    case True{} U.Act{u, cs}:
      View.join(U.Ui.act(u, "mech", ""), cs)
    case False{} _:
      a

# (back from the Review tab to the board or schematic, the client's view
# comes back too, with a comparison still open: client.bend's Rv.back)
def View.open.back.k(a: U.Act, +v: String) -> U.Act:
  match a:
    case U.Act{+u, cs}:
      View.join(View.open.k(u, v), cs)

def View.open.back(back: Bool, +u: U.Ui, +v: String) -> U.Act:
  match back:
    case True{}:
      View.open.back.k(U.Ui.act(u, "view-kind", v), v)
    case False{}:
      View.open.k(u, v)

def View.open.go(+u: U.Ui, +v: String) -> U.Act:
  View.open.mech(String.eq(v, "mech"), View.open.back(Bool.and(String.eq(U.Ui.view_get(u, "view.kind"), "review"), Bool.or(String.eq(v, "board"), String.eq(v, "schematic"))), u, v))

# (Files asks the hub for the folder last shown)
def View.open.rv(rv: Bool, +u: U.Ui, +v: String) -> U.Act:
  match rv:
    case True{}:
      View.hist.act(u, "rv-page", "bom")
    case False{}:
      View.open.go(u, v)

def View.open(+u: U.Ui, +v: String) -> U.Act:
  Bool.pick(U.Act, String.eq(v, "files"), View.fm.act(u, "fm", ""), View.open.rv(String.eq(v, "review"), u, v))

# a Mech action the client takes (the part, its view), then the phone's
# view on Mech ("mech") or Renders ("renders")
def View.mech.to(a: U.Act, v: String) -> U.Act:
  match a:
    case U.Act{u, cs}:
      View.join(View.open.k(u, v), cs)

def View.mech.act(+u: U.Ui, +action: String, +value: String, v: String) -> U.Act:
  View.mech.to(U.Ui.act(u, action, value), v)

# a new connection watches nothing: ask again once online
# (a Review tab kept across a launch asks for its page again)
def View.online.rv(rv: Bool, a: U.Act) -> U.Act:
  match rv a:
    case True{} U.Act{+u, cs}:
      View.join(U.Rp.ask(u, "bom"), cs)
    case False{} _:
      a

def View.online.k(on: Bool, +u: U.Ui) -> U.Act:
  match on:
    case True{}:
      U.Rc.sync.any(View.online.rv(String.eq(View.kind(u), "review"), View.sync(U.Act{U.Ui.scratch_set(u, "@view.at", ""), Nil{}})))
    case False{}:
      U.Act{u, Nil{}}

def View.online(u: U.Ui, +on: Bool) -> U.Act:
  View.online.k(on, U.Ui.online(u, on))

# The screen's part
# -----------------

def View.bg(+kind: String, light: Bool) -> U32:
  Bool.pick(U32, light, LO.Look.ground(True{}), 855826)

# the viewer's own light ground ("vw-light" flips it, as the window's sun
# and moon button does): the phone paints each layer in its colour here
# (look.bend's light table, by layer index) over View.bg
def View.light(+u: U.Ui) -> Bool:
  Bool.not(String.is_empty(U.Ui.scratch(u, "@vw.light")))

def View.look.go(n: Nat, +kind: U32, +i: U32) -> List<&2, J.Json>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      J.Num{U32.show(Bool.pick(U32, LO.Look.mark(i), LO.Look.mark.color(True{}, i), LO.Look.light.k(U32.is_eq(kind, 1), i)))} <> View.look.go(k, kind, (i + 1 : U32))

# (up to a comparison's marks, in their light colours)
def View.look(light: Bool, +kind: String) -> List<&2, J.Json>:
  match light:
    case True{}:
      View.look.go(LO.Look.count(LO.Look.diff()), Bool.pick(U32, String.eq(kind, "schematic"), 1, 0), 0)
    case False{}:
      Nil{}

# which board layers a 3D view lays on the board's top and bottom faces,
# in paint order as seen from that side (from below, B.Cu first and
# B.SilkS over it); inner layers are inside the board
def View.top() -> List<&2, U32>:
  [12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24]

def View.bottom() -> List<&2, U32>:
  [5, 13, 4, 2, 3, 1, 0, 22, 23, 24]

def View.nums(xs: List<&2, U32>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Num{U32.show(x)} <> View.nums(t)

def View.choice(+label: String, +value: String, +kind: String) -> J.Json:
  J.Json.obj([J.KV{"label", J.Str{label}}, J.KV{"value", J.Str{value}}, J.KV{"on", J.Flag{String.eq(kind, value)}}])

# what was picked let go
def Pick.clear(u: U.Ui) -> U.Ui:
  U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.scratch_set(u, "@pick", ""), "@card", ""), "@pick.rid", "")

# Layers
# ------
# The layers of the board or the schematic the user turned off, a bit per
# layer ("@view.off.<kind>"): the phone does not draw them and a tap does
# not pick them. Only layers the hub sends (those shown by default) are
# listed.

def View.off.key(+u: U.Ui) -> String:
  "@view.off." ++ View.kind(u)

def View.off(+u: U.Ui) -> U32:
  Maybe.default(&2, U32, U32.read(U.Ui.scratch(u, View.off.key(u))), 0)

def View.layer(+u: U.Ui, +v: String) -> U.Act:
  U.Act{Pick.clear(U.Ui.scratch_set(u, View.off.key(u), U32.show(V.Look.toggle(View.off(u), Maybe.default(&2, U32, U32.read(v), 31))))), Nil{}}

# The focus (core/layers.bend): the active layer and how the others show
# ("@view.focus.<kind>"); "view-layer-pick" makes a layer stand out (again,
# all back), "view-layer-mode" sets All, Dim or Only

def View.focus.key(+u: U.Ui) -> String:
  "@view.focus." ++ View.kind(u)

def View.focus(+u: U.Ui) -> LY.Focus:
  LY.Lyr.read(U.Ui.scratch(u, View.focus.key(u)))

# the look kind (0 a board, 1 a schematic)
def View.lkind(+u: U.Ui) -> U32:
  Bool.pick(U32, String.eq(View.kind(u), "schematic"), 1, 0)

def View.focus.put(+u: U.Ui, +f: LY.Focus) -> U.Act:
  View.hist.act(Pick.clear(U.Ui.scratch_set(u, View.focus.key(u), LY.Lyr.show(f))), "rv-bits", U32.show(U.Rv.bits(View.lkind(u), f)))

def View.layer.pick(+u: U.Ui, +v: String) -> U.Act:
  View.focus.put(u, LY.Lyr.pick(View.focus(u), LY.Lyr.num(v, LY.Lyr.none())))

def View.layer.mode(+u: U.Ui, +v: String) -> U.Act:
  View.focus.put(u, LY.Lyr.set(View.lkind(u), View.focus(u), LY.Lyr.num(v, 0)))

# what the phone draws from: the layers not drawn and those dimmed
def View.masks(+u: U.Ui) -> LY.Masks:
  LY.Lyr.masks(View.lkind(u), View.off(u), View.focus(u))

# the list's mode switch: as it is (0), dimmed (1), hidden (2)
def View.lmode(+u: U.Ui) -> U32:
  Bool.pick(U32, LY.Lyr.on(View.focus(u)), LY.Lyr.mode(View.focus(u)), 0)

# the Layers button names the active layer while one stands out
def View.lhead.loc(+lc: I.Loc, +u: U.Ui) -> String:
  Bool.pick(String, LY.Lyr.on(View.focus(u)), MSG.T.viewer.layers.active(lc, LY.Lyr.name(View.lkind(u), LY.Lyr.active(View.focus(u)))), MSG.T.viewer.layers(lc))

def View.lhead(+u: U.Ui) -> String:
  View.lhead.loc(I.En{}, u)

def View.names(+kind: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.eq(kind, "schematic"), SC.Sch.layer_names(), B.Layer.names())

def View.layer.row(shown: Bool, +i: U32, +name: String, +off: U32, +act: Bool, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match shown:
    case True{}:
      J.Json.obj([J.KV{"layer", J.Num{U32.show(i)}}, J.KV{"name", J.Str{name}}, J.KV{"on", J.Flag{Bool.not(V.Look.flipped(off, i))}}, J.KV{"active", J.Flag{act}}]) <> rest
    case False{}:
      rest

def View.layer.rows(ls: List<&2, U32>, +k: U32, +off: U32, +f: LY.Focus) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      Nil{}
    case +i <> t:
      View.layer.row(U32.is_gt(V.Look.alpha(k, i, 0), 0), i, LY.Lyr.name(k, i), off, Bool.and(LY.Lyr.on(f), U32.is_eq(i, LY.Lyr.active(f))), View.layer.rows(t, k, off, f))

# the list the phone's Layers menu shows (none for 3D), in KiCad's order
# (core/layers.bend; a comparison's marks too: removed, changed, added)
def View.layer.list(+u: U.Ui) -> List<&2, J.Json>:
  +k = {U32.or(Bool.pick(U32, String.eq(View.kind(u), "schematic"), 1, 0), Bool.pick(U32, Bool.and(View.hist(u), Nat.is_eq(HS.Hist.kind(U.Ui.view_get(u, "view.path")), 2n)), LO.Look.diff(), 0)) : U32}
  Bool.pick(List<&2, J.Json>, Bool.or(String.eq(View.kind(u), "board"), String.eq(View.kind(u), "schematic")),
    View.layer.rows(LY.Lyr.order(k), k, View.off(u), View.focus(u)), Nil{})

# Sheets
# ------
# The hub sends a schematic's sheets when a phone watches it ({"t":
# "sheets", root, rows as Hier.show writes them}); "view-sheet" picks one
# by its row, the root being row 0.

def View.sheets.k(hit: Bool, +u: U.Ui, +o: J.Json) -> U.Ui:
  match hit:
    case True{}:
      U.Ui.scratch_set(U.Ui.scratch_set(u, "@view.tree", J.Json.get_str(o, "rows", "")), "@view.tree.root", J.Json.get_str(o, "root", ""))
    case False{}:
      u

# a 3D model's note ({"t":"plotnote", key, text}: parts with no model),
# kept with the source it is about
def View.note.k(hit: Bool, +u: U.Ui, +o: J.Json) -> U.Ui:
  match hit:
    case True{}:
      U.Ui.scratch_set(U.Ui.scratch_set(u, "@view.note", J.Json.get_str(o, "text", "")), "@view.note.key", J.Json.get_str(o, "key", ""))
    case False{}:
      u

def View.sheets(+u: U.Ui, +o: J.Json) -> U.Ui:
  View.note.k(String.eq(J.Json.get_str(o, "t", ""), "plotnote"), View.sheets.k(String.eq(J.Json.get_str(o, "t", ""), "sheets"), u, o), o)

def View.note(+u: U.Ui) -> String:
  Bool.pick(String, String.eq(U.Ui.scratch(u, "@view.note.key"), View.want(u)), U.Ui.scratch(u, "@view.note"), "")

def View.rows(+u: U.Ui) -> List<&2, HI.Row>:
  Bool.pick(List<&2, HI.Row>, String.eq(U.Ui.scratch(u, "@view.tree.root"), U.Ui.root(u)), HI.Hier.read(U.Ui.scratch(u, "@view.tree")), Nil{})

def View.sheet.go(+u: U.Ui, +i: Nat, +file: String) -> U.Act:
  View.sync(U.Act{Pick.clear(U.Ui.scratch_set(u, "@view.sheet:" ++ U.Ui.sel(u), Bool.pick(String, Nat.is_eq(i, 0n), "", file))), Nil{}})

def View.sheet.pick.u(+u: U.Ui, +v: String) -> U.Act:
  View.sheet.go(u, Maybe.default(&2, Nat, Nat.read(v), 0n), HI.Hier.file(View.rows(u), Maybe.default(&2, Nat, Nat.read(v), 0n)))

def View.sheet.pick.k(hist: Bool, +u: U.Ui, +v: String) -> U.Act:
  match hist:
    case True{}:
      View.sheet.pick.u(U.Act.ui(U.Ui.act(View.as_client(u), "hist-live", "")), v)
    case False{}:
      View.sheet.pick.u(u, v)

# a sheet picked while a comparison shows keeps comparing: the same
# versions of that sheet (design review, core's Hist.sheet)
def View.sheet.cmp(+u: U.Ui, +i: Nat, +file: String) -> U.Act:
  View.sheet.go(U.Ui.view_put(u, "schematic", HS.Hist.sheet(U.Ui.view_get(u, "view.path"), file)), i, file)

def View.sheet.cmp.u(+u: U.Ui, +v: String) -> U.Act:
  View.sheet.cmp(u, Maybe.default(&2, Nat, Nat.read(v), 0n), HI.Hier.file(View.rows(u), Maybe.default(&2, Nat, Nat.read(v), 0n)))

def View.sheet.pick.c(cmp: Bool, +u: U.Ui, +v: String) -> U.Act:
  match cmp:
    case True{}:
      View.sheet.cmp.u(u, v)
    case False{}:
      View.sheet.pick.k(View.hist(u), u, v)

# (while a version shows, the live files come back first: the sheets
# listed are the live design's)
def View.sheet.pick.s(+u: U.Ui, +v: String) -> U.Act:
  View.sheet.pick.c(U.Hs.cmp(View.as_client(u)), u, v)

# one of the project's boards ("b:<path from its folder>", client.bend's
# Bd): the phone's viewer shows its board, schematic or 3D, as a file from
# the Files tab would
def View.board.pick(+u: U.Ui, +rel: String) -> U.Act:
  View.fm.to(U.Act{U.Ui.scratch_set(u, "@view.sheet:" ++ U.Ui.sel(u), ""), Nil{}}, U.Bd.kind(View.kind(u)), FM.Fm.abs(U.Ui.root(u), U.Bd.path(U.Bd.kind(View.kind(u)), rel)))

def View.sheet.pick(+u: U.Ui, +v: String) -> U.Act:
  Bool.pick(U.Act, String.starts_with(v, "b:"), View.board.pick(u, String.drop(v, 2n)), View.sheet.pick.s(u, v))

def View.sheet.row(r: HI.Row, +i: Nat, +cur: String) -> J.Json:
  match r:
    case HI.Row{+d, n, +f, l}:
      J.Json.obj([J.KV{"label", J.Str{String.repeat("  ", d) ++ n}}, J.KV{"value", J.Str{Nat.show(i)}}, J.KV{"on", J.Flag{String.eq(f, cur)}}, J.KV{"loop", J.Flag{l}}])

def View.sheet.rows(rs: List<&2, HI.Row>, +i: Nat, +cur: String) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      View.sheet.row(r, i, cur) <> View.sheet.rows(t, 1n+i, cur)

# the project's boards for the same menu, when it has several
def View.board.rows(fs: List<&2, String>, +cur: String) -> List<&2, J.Json>:
  match fs:
    case Nil{}:
      Nil{}
    case +f <> t:
      J.Json.obj([J.KV{"label", J.Str{U.Bd.name(f)}}, J.KV{"value", J.Str{"b:" ++ f}}, J.KV{"on", J.Flag{U.Bd.on(cur, f)}}, J.KV{"loop", J.Flag{False{}}}]) <> View.board.rows(t, cur)

def View.board.list(+u: U.Ui) -> List<&2, J.Json>:
  Bool.pick(List<&2, J.Json>, Bool.and(U.Bd.many(u), Bool.or(String.eq(View.kind(u), "board"), Bool.or(String.eq(View.kind(u), "schematic"), String.eq(View.kind(u), "3d")))),
    View.board.rows(U.Bd.files(u), U.Bd.shown(u, View.file(u))), Nil{})

# the schematic's sheets for the phone's Sheet menu (none with one sheet)
def View.sheet.list(+u: U.Ui) -> List<&2, J.Json>:
  Bool.pick(List<&2, J.Json>, Bool.and(String.eq(View.kind(u), "schematic"), Nat.is_gt(List.length(&2, HI.Row, View.rows(u)), 1n)),
    View.sheet.rows(View.rows(u), 0n, Bool.pick(String, String.is_empty(View.sheet(u)), HI.Hier.file(View.rows(u), 0n), View.sheet(u))), Nil{})

# the 3D model with its parts or the board alone ("view-parts")
def View.parts(+u: U.Ui) -> U.Act:
  View.sync(U.Act{U.Ui.scratch_set(u, "@view.bare", Bool.pick(String, View.bare(u), "", "1")), Nil{}})

# Inspecting a tap
# ----------------
# The phone hit-tests its own geometry and hands over what lies under the
# finger: each piece's place (its chunk's index in the plot, the piece's
# index in the chunk), its size (in 100 square micrometres) and its layer.
# The smallest wins (a pad over the zone it sits in), then the one drawn on
# top. It is highlighted at once, and its info (net, ref, value...) asked
# of the hub, which answers from the very chunks this phone holds (law
# plot_delta_exact); the card shows it as the window's inspector does.

type Cand is Data:
  Cand{id: String, area: U32, layer: U32}

def Pick.num(+o: J.Json, +k: String) -> U32:
  Maybe.default(&2, U32, U32.read(J.Json.as_raw(J.Json.get(o, k), "0")), 0)

def Pick.cand(+o: J.Json) -> Cand:
  Cand{J.Json.as_raw(J.Json.get(o, "c"), "0") ++ "," ++ J.Json.as_raw(J.Json.get(o, "p"), "0"), Pick.num(o, "a"), Pick.num(o, "l")}

def Pick.cands(js: List<&2, J.Json>) -> List<&2, Cand>:
  match js:
    case Nil{}:
      Nil{}
    case j <> t:
      Pick.cand(j) <> Pick.cands(t)

def Pick.over(c: Cand, b: Cand) -> Bool:
  match c b:
    case Cand{_, +a, +l} Cand{_, +ba, +bl}:
      Bool.or(U32.is_lt(a, ba), Bool.and(U32.is_eq(a, ba), U32.is_ge(l, bl)))

def Pick.better.k(take: Bool, c: Cand, b: Cand) -> Cand:
  match take:
    case True{}:
      c
    case False{}:
      b

def Pick.better(+c: Cand, m: Maybe<&2, Cand>) -> Maybe<&2, Cand>:
  match m:
    case None{}:
      Some{c}
    case Some{+b}:
      Some{Pick.better.k(Pick.over(c, b), c, b)}

# the piece a tap picks, of those under the finger
def Pick.best(cs: List<&2, Cand>, m: Maybe<&2, Cand>) -> Maybe<&2, Cand>:
  match cs:
    case Nil{}:
      m
    case c <> t:
      Pick.best(t, Pick.better(c, m))

# a layer the user turned off is not there to tap
def Pick.shown.k(off: Bool, c: Cand, rest: List<&2, Cand>) -> List<&2, Cand>:
  match off:
    case True{}:
      rest
    case False{}:
      c <> rest

def Pick.shown(cs: List<&2, Cand>, +off: U32) -> List<&2, Cand>:
  match cs:
    case Nil{}:
      Nil{}
    case Cand{+id, +a, +l} <> t:
      Pick.shown.k(V.Look.flipped(off, l), Cand{id, a, l}, Pick.shown(t, off))

def Pick.of(+text: String, +off: U32) -> Maybe<&2, Cand>:
  Pick.best(Pick.shown(Pick.cands(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))), off), None{})

def Pick.ask.k(+at: String, a: U.Act) -> U.Act:
  match a:
    case U.Act{u, cs}:
      U.Act{U.Ui.scratch_set(u, "@pick", at), cs}

def Pick.ask(m: Maybe<&2, Cand>, +u: U.Ui) -> U.Act:
  match m:
    case None{}:
      U.Act{Pick.clear(u), Nil{}}
    case Some{Cand{+id, _, _}}:
      Pick.ask.k(id, U.Ui.rpc(U.Ui.scratch_set(Pick.clear(u), "@pick.rid", Nat.show(U.Ui.rid(u))), "kicad.info",
        [J.KV{"key", J.Str{View.layers.at(View.kind(u), U.Ui.root(u), View.sheet(u))}},
         J.KV{"at", J.Num{Maybe.default(&2, String, List.head(&2, String, String.split(id, ',')), "0")}},
         J.KV{"idx", J.Num{Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, String.split(id, ','), 1n)), "0")}}]))

# a tap in the viewer, given what lies under the finger: the piece it
# picks is highlighted and its info asked of the hub; a tap on nothing
# clears the card (law pick_nothing_there)
def View.pick(+u: U.Ui, +text: String) -> U.Act:
  Pick.ask(Pick.of(text, LY.Masks.off(View.masks(u))), u)

def View.picked.k(mine: Bool, +u: U.Ui, +o: J.Json) -> U.Ui:
  match mine:
    case True{}:
      U.Ui.scratch_set(U.Ui.scratch_set(u, "@pick.rid", ""), "@card", J.Json.get_str(J.Json.get_obj(o, "result"), "info", ""))
    case False{}:
      u

# the hub's answer to the newest pick fills the card (a late one for an
# earlier tap changes nothing)
def View.picked(+u: U.Ui, +o: J.Json) -> U.Ui:
  View.picked.k(Bool.and(Bool.and(Bool.not(String.is_empty(U.Ui.scratch(u, "@pick.rid"))), String.eq(J.Json.as_raw(J.Json.get(o, "id"), ""), U.Ui.scratch(u, "@pick.rid"))),
    String.eq(J.Json.get_str(o, "t", ""), "reply")), u, o)

def Insp.row(kv: List<&2, String>) -> J.Json:
  match kv:
    case k <> v:
      J.Json.obj([J.KV{"k", J.Str{k}}, J.KV{"v", J.Str{String.join(v, "=")}}])
    case Nil{}:
      J.Json.obj([J.KV{"k", J.Str{""}}, J.KV{"v", J.Str{""}}])

def Insp.rows(ps: List<&2, String>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      Insp.row(String.split(p, '=')) <> Insp.rows(t)

# an item's card: its kind, then key = value rows (net, ref, value,
# footprint, pin, label...), as the window's inspector
def Insp.card(+info: String) -> J.Json:
  J.Json.obj([J.KV{"info", J.Str{info}}, J.KV{"title", J.Str{Maybe.default(&2, String, List.head(&2, String, String.split(info, ';')), "")}},
    J.KV{"rows", J.Json.arr(Insp.rows(List.drop(&2, String, String.split(info, ';'), 1n)))}])

def Insp.json(+info: String) -> J.Json:
  Bool.pick(J.Json, String.is_empty(info), J.Null{}, Insp.card(info))

# The Mechanical page, as the phones draw it: what to say while there are
# no parts, the parts as chips (action "mech-part"), the part on show
# (Open in 3D: "mech-3d" with its path) and its renders (hub URLs), and
# where to get FreeCAD when the hub has none (client.bend's Mpage)
def Mech.chips(ps: List<&2, MC.MPart>, +cur: String) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case MC.MPart{+p, n, _} <> t:
      J.Json.obj([J.KV{"label", J.Str{n}}, J.KV{"value", J.Str{p}}, J.KV{"on", J.Flag{String.eq(p, cur)}}]) <> Mech.chips(t, cur)

def Mech.shots(fs: List<&2, MC.MFile>, +root: String) -> List<&2, J.Json>:
  match fs:
    case Nil{}:
      Nil{}
    case +f <> t:
      J.Json.obj([J.KV{"view", J.Str{MC.Mech.view.of(f)}}, J.KV{"url", J.Str{MC.Mech.url(root, f)}}]) <> Mech.shots(t, root)

def Mech.part(m: Maybe<&2, MC.MPart>, +root: String) -> List<&2, J.KV>:
  match m:
    case Some{MC.MPart{+p, +n, +fs}}:
      [J.KV{"name", J.Str{n}}, J.KV{"path", J.Str{MC.Mech.abs(root, p)}}, J.KV{"shots", J.Json.arr(Mech.shots(fs, root))},
       J.KV{"empty", J.Str{Bool.pick(String, List.is_empty(&2, MC.MFile, fs), MC.Mech.unrendered(n), "")}}]
    case None{}:
      [J.KV{"name", J.Str{""}}, J.KV{"path", J.Str{""}}, J.KV{"shots", J.Json.arr(Nil{})}, J.KV{"empty", J.Str{""}}]

def Mech.say.loc(+lc: I.Loc, +state: Nat) -> String:
  Bool.pick(String, Nat.is_eq(state, 0n), MSG.T.viewer.mech.loading(lc), Bool.pick(String, Nat.is_eq(state, 1n), MC.Mech.none(), ""))

# (busy: a render job runs; render: its button's label; err: why the last failed)

def Mech.say(+state: Nat) -> String:
  Mech.say.loc(I.En{}, state)

def Mech.json(+lc: I.Loc, on: Bool, +u: U.Ui) -> J.Json:
  match on:
    case True{}:
      J.Json.obj(List.append(&2, J.KV, [J.KV{"say", J.Str{Mech.say.loc(lc, U.Mpage.state(u))}}, J.KV{"note", J.Str{U.Mpage.note.loc(lc, u)}},
        J.KV{"parts", J.Json.arr(Mech.chips(U.Mpage.parts(u), U.Mpage.cur.path(u)))}, J.KV{"rel", J.Str{U.Mpage.cur.path(u)}},
        J.KV{"busy", J.Flag{Bool.not(String.is_empty(U.Mpage.busy(u)))}}, J.KV{"render", J.Str{U.Mpage.render.label.loc(lc, u)}}, J.KV{"err", J.Str{U.Mpage.err(u)}}],
        Mech.part(U.Mpage.cur(u), U.Mpage.root(u))))
    case False{}:
      J.Null{}

# The history bar as the phones draw it: null when there is none, else
# what Hs.bar says, the scrubber's ticks (id "" the live file, on the
# one shown), the menu of versions while one is open, and the legend's
# colours (0xRRGGBB)
def Hist.ticks(ss: List<&2, HS.Step>, +i: Nat, +at: Nat) -> List<&2, J.Json>:
  match ss:
    case Nil{}:
      [J.Json.obj([J.KV{"id", J.Str{""}}, J.KV{"on", J.Json.bool(Nat.is_eq(at, 0n))}])]
    case +s <> t:
      J.Json.obj([J.KV{"id", J.Str{HS.Step.id(s)}}, J.KV{"on", J.Json.bool(Nat.is_eq(at, 1n+i))}]) <> Hist.ticks(t, 1n+i, at)

def Hist.rows(rs: List<&2, U.HRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case U.HRow{+l, +sub, +v} <> t:
      J.Json.obj([J.KV{"label", J.Str{l}}, J.KV{"sub", J.Str{sub}}, J.KV{"value", J.Str{v}}]) <> Hist.rows(t)

# (made only while the compare menu is open: Bool.pick made it, every
# design step's row, on every screen of a thread with history)
def Hist.menu.k(shut: Bool, +lc: I.Loc, +side: String, +u: U.Ui) -> J.Json:
  match shut:
    case True{}:
      J.Null{}
    case False{}:
      J.Json.arr(Hist.rows(U.Hs.menu.loc(lc, u, side)))

def Hist.menu(+lc: I.Loc, +side: String, +u: U.Ui) -> J.Json:
  Hist.menu.k(String.is_empty(side), lc, side, u)

def Hist.json.k(+lc: I.Loc, b: U.HBar, +u: U.Ui) -> J.Json:
  match b:
    case U.HBar{_, +n, +at, +title, +entry, +label, +said, +pl, +df, +cmp, +a, +bb, +menu}:
      J.Json.obj([J.KV{"n", J.Num{Nat.show(n)}}, J.KV{"at", J.Num{Nat.show(at)}}, J.KV{"title", J.Str{title}}, J.KV{"entry", J.Str{entry}},
        J.KV{"said", J.Str{Bool.pick(String, Nat.is_eq(at, 0n), U.Hs.live.said.loc(lc, n), Bool.pick(String, String.is_empty(said), label, said))}},
        J.KV{"playing", J.Json.bool(pl)}, J.KV{"diff", J.Json.bool(df)}, J.KV{"cmp", J.Json.bool(cmp)}, J.KV{"a", J.Str{a}}, J.KV{"b", J.Str{bb}}, J.KV{"side", J.Str{menu}}, J.KV{"as", J.Str{U.Rv.showing(u)}},
        J.KV{"ticks", J.Json.arr(Hist.ticks(U.Hs.track(u), 0n, at))}, J.KV{"menu", Hist.menu(lc, menu, u)},
        J.KV{"keys", J.Json.arr([J.Num{U32.show(LO.Look.mark.color(False{}, 26))}, J.Num{U32.show(LO.Look.mark.color(False{}, 27))}, J.Num{U32.show(LO.Look.mark.color(False{}, 28))}])}])

# (made only when the bar shows: Bool.pick made it, every step parsed for
# its ticks, on every screen)
def Hist.json.on(on: Bool, +lc: I.Loc, +b: U.HBar, +u: U.Ui) -> J.Json:
  match on:
    case True{}:
      Hist.json.k(lc, b, u)
    case False{}:
      J.Null{}

def Hist.json(+lc: I.Loc, +b: U.HBar, +u: U.Ui) -> J.Json:
  Hist.json.on(U.HBar.on(b), lc, b, u)

# what the phone's viewer needs: the source open ("" closed), its key and
# the key of the plot it draws layers from, the choices, and how to draw:
# background, the fade of new chunks (ms, as the window's), the first
# view's margin, the zoom range as a share of that view and in pixels per
# micrometre, how far from a tap (points) an item still counts as tapped,
# and for 3D the faces' layers, the board's colour before its model
# arrives, and the field of view (degrees)
# the Files tab as the apps draw it: the trail, the rows (each with what a
# tap sends), whether the listing is in; null when not shown
def Files.trail(ps: List<&2, U.PItem>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case U.PItem{+l, +a, +v} <> t:
      J.Json.obj([J.KV{"label", J.Str{l}}, J.KV{"action", J.Str{a}}, J.KV{"value", J.Str{v}}]) <> Files.trail(t)

def Files.rows(rs: List<&2, U.FmRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case U.FmRow{+l, +a, +v, +d, +k} <> t:
      J.Json.obj([J.KV{"label", J.Str{l}}, J.KV{"action", J.Str{a}}, J.KV{"value", J.Str{v}}, J.KV{"detail", J.Str{d}}, J.KV{"kind", J.Str{k}}, J.KV{"icon", J.Str{U.Fm.icon(k)}}]) <> Files.rows(t)

def Files.json(+lc: I.Loc, on: Bool, +u: U.Ui) -> J.Json:
  match on:
    case True{}:
      J.Json.obj([J.KV{"trail", J.Json.arr(Files.trail(U.Fm.trail.loc(lc, u)))}, J.KV{"rows", J.Json.arr(Files.rows(U.Fm.rows(u)))}, J.KV{"ready", J.Json.bool(U.Fm.ready(u))}])
    case False{}:
      J.Null{}

# an image or a PDF opened from the Files tab: its name, its kind, where
# the hub serves it (the app adds the hub and its token)
def File.json(on: Bool, +u: U.Ui) -> J.Json:
  match on:
    case True{}:
      J.Json.obj([J.KV{"name", J.Str{U.Fm.name(u)}}, J.KV{"kind", J.Str{View.kind(u)}}, J.KV{"url", J.Str{IM.Img.url(U.Ui.view_get(u, "view.path"))}}])
    case False{}:
      J.Null{}

# the Review tab as the apps draw it: the page chips, the title, the
# report's Markdown as the timeline's nodes (or what to say without one)
def Rv.tabs(ts: List<&2, U.RpTab>) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case U.RpTab{+p, +l, +on} <> t:
      J.Json.obj([J.KV{"page", J.Str{p}}, J.KV{"label", J.Str{l}}, J.KV{"on", J.Flag{on}}]) <> Rv.tabs(t)

# the Checks page's boxes (client.bend's Rp.tasks): each with the index
# "rv-tick" takes, its words, whether it is ticked and its heading
def Rv.tasks(ts: List<&2, U.RpTask>) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case U.RpTask{+i, +text, +done, +group} <> t:
      J.Json.obj([J.KV{"index", J.Num{Nat.show(i)}}, J.KV{"text", J.Str{text}}, J.KV{"done", J.Flag{done}}, J.KV{"group", J.Str{group}}]) <> Rv.tasks(t)

def Rv.checks(on: Bool, +u: U.Ui) -> J.Json:
  match on:
    case True{}:
      J.Json.obj([J.KV{"gate", J.Str{U.Rp.gate(u)}}, J.KV{"line", J.Str{U.Rp.line(u)}}, J.KV{"tasks", J.Json.arr(Rv.tasks(U.Rp.tasks(u)))}])
    case False{}:
      J.Null{}

# the Release page's form (client.bend's Rl.form): the tag that will be
# made (the field "rltag", else the proposed one), the proposed one, the two
# toggles ("rv-release-pub", "rv-release-force"); "rv-release" makes it
def Rv.release.form(f: U.RlForm, +err: String, +text: String) -> J.Json:
  match f:
    case U.RlForm{+tag, +next, publish, force}:
      J.Json.obj([J.KV{"tag", J.Str{tag}}, J.KV{"next", J.Str{next}}, J.KV{"publish", J.Flag{publish}}, J.KV{"force", J.Flag{force}}, J.KV{"err", J.Str{err}}, J.KV{"text", J.Str{text}}])

def Rv.release.loc(+lc: I.Loc, on: Bool, +u: U.Ui) -> J.Json:
  match on:
    case True{}:
      Rv.release.form(U.Rl.form(u), U.RpView.err(U.Rp.view.loc(lc, u)), U.Ui.scratch(u, "@f.rltag"))
    case False{}:
      J.Null{}

def Rv.release(on: Bool, +u: U.Ui) -> J.Json:
  Rv.release.loc(I.En{}, on, u)

def Rv.json.k(+lc: I.Loc, +v: U.RpView, +rb: J.Json, +checks: J.Json, +release: J.Json) -> J.Json:
  J.Json.obj([J.KV{"tabs", J.Json.arr(Rv.tabs(U.RpView.tabs(v)))}, J.KV{"title", J.Str{U.RpView.title(v)}}, J.KV{"busy", J.Flag{U.RpView.busy(v)}},
    J.KV{"note", J.Str{U.RpView.note.loc(lc, v)}}, J.KV{"blocks", rb}, J.KV{"checks", checks}, J.KV{"release", release}])

def Rv.json(+lc: I.Loc, on: Bool, +u: U.Ui, +rb: J.Json) -> J.Json:
  match on:
    case True{}:
      Rv.json.k(lc, U.Rp.view.loc(lc, u), rb, Rv.checks(String.eq(U.Rp.page(u), "checks"), u), Rv.release.loc(lc, String.eq(U.Rp.page(u), "release"), u))
    case False{}:
      J.Null{}

# Review comments in the viewer (client.bend's Rc): the pins of the file on
# show (places in mm of the file's own coordinates), the one open with its
# replies, and the place waiting for its note ("" none)
def Rc.pins(ps: List<&2, RVW.Pin>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case RVW.Pin{+id, +x, +y, _, _, +r, +n} <> t:
      J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"x", J.Num{F32.show(x)}}, J.KV{"y", J.Num{F32.show(y)}}, J.KV{"resolved", J.Flag{r}}, J.KV{"count", J.Num{Nat.show(n)}}]) <> Rc.pins(t)

def Rc.replies(rs: List<&2, String>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      +ps = String.split(r, Chr{31})
      J.Json.obj([J.KV{"text", J.Str{Maybe.default(&2, String, List.get(&2, String, ps, 0n), "")}}, J.KV{"by", J.Str{Maybe.default(&2, String, List.get(&2, String, ps, 1n), "")}}]) <> Rc.replies(t)

def Rc.open.json(m: Maybe<&2, U.RcOpen>) -> J.Json:
  match m:
    case Some{U.RcOpen{+id, +text, +by, +place, +r, +rs}}:
      J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"text", J.Str{text}}, J.KV{"by", J.Str{by}}, J.KV{"place", J.Str{place}}, J.KV{"resolved", J.Flag{r}}, J.KV{"replies", J.Json.arr(Rc.replies(rs))}])
    case None{}:
      J.Null{}

def Rc.json(+u: U.Ui) -> J.Json:
  J.Json.obj([J.KV{"pins", J.Json.arr(Rc.pins(U.Rc.pins(u)))}, J.KV{"open", Rc.open.json(U.Rc.open(u))}, J.KV{"at", J.Str{U.Rc.at(u)}}])

def View.json(+lc: I.Loc, +u: U.Ui, +rb: J.Json) -> J.Json:
  J.Json.obj([J.KV{"open", J.Str{Bool.pick(String, Bool.or(Bool.or(Bool.or(Bool.not(String.is_empty(View.want(u))), View.mech(u)), View.fm(u)), String.eq(View.kind(u), "review")), View.kind(u), "")}}, J.KV{"key", J.Str{View.want(u)}},
    J.KV{"comments", Rc.json(u)},
    J.KV{"review", Rv.json(lc, String.eq(View.kind(u), "review"), u, rb)},
    J.KV{"files", Files.json(lc, String.eq(View.kind(u), "files"), u)}, J.KV{"file", File.json(Bool.or(String.eq(View.kind(u), "image"), String.eq(View.kind(u), "pdf")), u)},
    J.KV{"layers", J.Str{Bool.pick(String, View.mech(u), "", View.layers.at(View.kind(u), U.Ui.root(u), View.src(u)))}},
    J.KV{"hist", Hist.json(lc, U.Hs.bar.for.loc(lc, View.as_client(u), Bool.and(Bool.not(String.is_empty(View.want(u))), Nat.is_lt(View.hist.num(View.kind(u)), 2n))), View.as_client(u))},
    J.KV{"mech", Mech.json(lc, View.mech(u), u)},
    J.KV{"sheets", J.Json.arr(List.append(&2, J.Json, View.board.list(u), View.sheet.list(u)))}, J.KV{"layerList", J.Json.arr(View.layer.list(u))},
    J.KV{"off", J.Num{U32.show(LY.Masks.off(View.masks(u)))}}, J.KV{"dim", J.Num{U32.show(LY.Masks.dim(View.masks(u)))}}, J.KV{"lift", J.Num{U32.show(LY.Lyr.top(View.focus(u)))}},
    J.KV{"layerMode", J.Num{U32.show(View.lmode(u))}}, J.KV{"layerModes", J.Flag{String.eq(View.kind(u), "board")}}, J.KV{"layerHead", J.Str{View.lhead.loc(lc, u)}},
    J.KV{"parts", J.Flag{Bool.not(View.bare(u))}}, J.KV{"note", J.Str{View.note(u)}},
    J.KV{"choices", J.Json.arr(Bool.pick(List<&2, J.Json>, String.is_empty(U.Ui.root(u)), Nil{},
      [View.choice(MSG.T.viewer.board(lc), "board", View.kind(u)), View.choice(MSG.T.viewer.schematic(lc), "schematic", View.kind(u)), View.choice("3D", "3d", View.kind(u)), View.choice(MSG.T.viewer.mech(lc), "mech", Bool.pick(String, View.mech(u), "mech", "")),
       View.choice(MSG.T.viewer.files(lc), "files", Bool.pick(String, View.fm(u), "files", "")), View.choice(MSG.T.viewer.review(lc), "review", View.kind(u))]))},
    J.KV{"bg", J.Num{U32.show(View.bg(View.kind(u), View.light(u)))}}, J.KV{"light", J.Json.bool(View.light(u))}, J.KV{"look", J.Json.arr(View.look(View.light(u), View.kind(u)))}, J.KV{"fade", J.Num{Nat.show(V.Fade.ms())}}, J.KV{"margin", J.Num{"0.9"}},
    J.KV{"zmin", J.Num{"0.5"}}, J.KV{"zmax", J.Num{"0.5"}}, J.KV{"tap", J.Num{"14"}},
    J.KV{"top", J.Json.arr(View.nums(View.top()))}, J.KV{"bottom", J.Json.arr(View.nums(View.bottom()))},
    J.KV{"slab", J.Num{"2375219"}}, J.KV{"fov", J.Num{"35"}},
    J.KV{"picked", J.Str{U.Ui.scratch(u, "@pick")}}, J.KV{"card", Insp.json(U.Ui.scratch(u, "@card"))}])

def View.mention.text(+draft: String, +info: String) -> String:
  draft ++ Bool.pick(String, String.is_empty(draft), "", " ") ++ "[" ++ info ++ "] "

# "Mention in chat": the item goes into the thread's draft (as the
# window's inspector puts it) and the viewer closes on it
def View.mention.k(a: U.Act) -> U.Act:
  match a:
    case U.Act{u, cs}:
      View.join(View.open(u, ""), cs)

def View.mention(+u: U.Ui, +info: String) -> U.Act:
  View.mention.k(U.Ui.act(u, "draft", View.mention.text(U.Ui.draft_text(u), info)))
