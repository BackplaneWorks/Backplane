import Base
import ../core/client.bend as U
import ../core/model.bend as M
import ../core/json.bend as J
import ../core/presentation.bend as P
import ../core/thin-wire.bend as W
import ../core/sidebar-drafts.bend as SD

# Private, per-connection sidebar identity. All recipient info/live data,
# exact clock, scope and every transient UI input are retained.
# Only request/model-rehydration counters and five main-panel-only fields are omitted.
# No key or draft value is included in a response or diagnostic.
# "@keys.rm" (the API key a Settings page asks to delete) is Settings-only
# too: Settings.go clears it on every page change.
def Sidebar.omit(+key: String) -> Bool:
  Bool.or(String.eq(key, "@settings.page"), Bool.or(String.eq(key, "@set.sc"), Bool.or(String.eq(key, "@set.focus"), String.eq(key, "@keys.rm"))))

def Sidebar.drafts(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, J.KV>:
  match xs:
    case Nil{}: Nil{}
    case (+k, v) <> rest:
      List.append(&2, J.KV, Bool.pick(List<&2, J.KV>, Sidebar.omit(k), Nil{}, [J.KV{k, J.Str{v}}]), Sidebar.drafts(rest))

def Sidebar.flags(xs: List<&2, Sigma<&2, &2, String, _ => Bool>>) -> List<&2, J.KV>:
  match xs:
    case Nil{}: Nil{}
    case (+k, v) <> rest:
      List.append(&2, J.KV, Bool.pick(List<&2, J.KV>, Bool.or(String.eq(k, "settings"), String.eq(k, "pairqr")), Nil{}, [J.KV{k, J.Flag{v}}]), Sidebar.flags(rest))

# Info.put.models increments this native-memo counter even for an identical
# catalog on rehydration. Web sidebar reads neither counter; actual model
# content and every other info field remain part of the exact private key.
# (and never the viewer reads kept hub-wide, server.bend's Kept.info: whole
# folders' listings, a diff; the sidebar reads none of them, and with them
# the key outgrew Sidebar.bounded's 32 KB, so the sidebar was drawn afresh
# on every render)
def Sidebar.info.kept(+k: String) -> Bool:
  Bool.or(Bool.or(String.starts_with(k, "fm.kept:"), String.starts_with(k, "refs.kept:")), Bool.or(String.starts_with(k, "diff.kept:"), String.starts_with(k, "kept.seq:")))

def Sidebar.info(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, J.KV>:
  match xs:
    case Nil{}: Nil{}
    case (+k, v) <> rest:
      List.append(&2, J.KV, Bool.pick(List<&2, J.KV>, Bool.or(String.eq(k, "models.v"), Sidebar.info.kept(k)), Nil{}, [J.KV{k, J.Str{v}}]), Sidebar.info(rest))

# The private ordinary image also leaves out what moves on every request but
# no sidebar reads: the fab lookup's own mark ("@fab.at", client.bend's
# Fab.sync), the per-request reply bookkeeping ("@rp.req:<id>",
# "@telemetry.req:<id>", read only by their reply handlers) and the desks'
# watched threads (info "desk", read only by the phones' alerts). With them
# in, the image missed on the action and the update after every view.open
# (2026-11-05: three full sidebar renders a click, 8-12 ms each).
def Sidebar.image.omit(+key: String) -> Bool:
  Bool.or(String.eq(key, "@fab.at"), Bool.or(String.starts_with(key, "@rp.req:"), String.starts_with(key, "@telemetry.req:")))

def Sidebar.image.drafts(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, Sigma<&2, &2, String, _ => String>>:
  match xs:
    case Nil{}: Nil{}
    case (+k, v) <> rest: SD.Input.keep(Bool.not(Sidebar.image.omit(k)), k, v, Sidebar.image.drafts(rest))

# Each thread's viewer and fold choices live in the info map too (client.bend's
# Reset.prefixes): a tool-call drawer opened ("fold:<id>"), a viewer tab, path
# or panel ("view.", "panel:", "panel.hand:"), a diff, a history step or
# comparison, a Files folder. The sidebar reads none of them (its one walk over
# the info, Side.signals.ix, reads "@signal:" keys only), so a fold or a viewer
# tab no longer draws the sidebar again (2026-11-05: every tool-call fold missed).
def Sidebar.image.viewer.go(ps: List<&2, String>, +k: String, hit: Bool) -> Bool:
  match ps hit:
    case _ True{}: True{}
    case Nil{} False{}: False{}
    case +p <> rest False{}: Sidebar.image.viewer.go(rest, k, String.starts_with(k, p))

def Sidebar.image.viewer(+k: String) -> Bool:
  Sidebar.image.viewer.go(["fold:", "view.", "panel:", "panel.hand:", "diff:", "hist.at:", "hist.back:", "hist.diff:", "cmp.a:", "cmp.b:", "cmp.al", "cmp.bl", "fm.want:"], k, String.eq(k, "desk"))

def Sidebar.image.info(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, Sigma<&2, &2, String, _ => String>>:
  match xs:
    case Nil{}: Nil{}
    case (+k, v) <> rest: SD.Input.keep(Bool.not(Sidebar.image.viewer(k)), k, v, Sidebar.image.info(rest))

def Sidebar.input_info(image: Bool, xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, J.KV>:
  match image:
    case False{}: Sidebar.info(xs)
    case True{}: Sidebar.info(Sidebar.image.info(xs))

# Keep the original full draft map for picture/transport keys. The private
# ordinary image uses the reviewed scratch input; the three pre-existing
# Settings-only omissions above remain identical in both modes.
def Sidebar.input_drafts(image: Bool, xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, J.KV>:
  match image:
    case False{}: Sidebar.drafts(xs)
    case True{}: Sidebar.drafts(Sidebar.image.drafts(SD.Input.fields(SD.Input.of(xs))))

# The trusted hub revision can replace the full sequence only for the
# metadata-only server sidebar. It excludes exactly the three history-only
# events proved for sidebar-read.Input. Clock/UI/info/scope fences stay exact.
# The legacy entry below keeps its full-sequence independent oracle.
def Sidebar.key.input(+image: Bool, u: U.Ui, epoch: String, view: String, revision: Nat) -> String:
  match u:
    case U.Ui{_, sel, dr, live, op, on, now, ad, pa, version, _, want, error, note, nf, outbox, _}:
      J.Json.show(J.Json.obj([J.KV{"epoch", J.Str{epoch}}, J.KV{"view", J.Str{view}}, J.KV{"seq", J.Json.nat(revision)},
        J.KV{"sel", J.Str{sel}}, J.KV{"drafts", J.Json.obj(Sidebar.input_drafts(image, Map.to_list(&2, String, dr)))},
        J.KV{"live", J.Json.obj(P.Session.strings(Map.to_list(&2, String, live)))}, J.KV{"open", J.Json.obj(Sidebar.flags(Map.to_list(&2, Bool, op)))},
        J.KV{"online", J.Flag{on}}, J.KV{"clock", J.Json.nat(now)}, J.KV{"adding", J.Flag{ad}}, J.KV{"panel", J.Flag{pa}}, J.KV{"version", J.Str{version}},
        J.KV{"want", J.Flag{want}}, J.KV{"error", J.Str{error}}, J.KV{"note", J.Str{note}},
        J.KV{"info", J.Json.obj(Sidebar.input_info(image, Map.to_list(&2, String, nf)))}, J.KV{"outbox", J.Json.obj(P.Session.strings(Map.to_list(&2, String, outbox)))}]))

def Sidebar.key.revision(u: U.Ui, epoch: String, view: String, revision: Nat) -> String:
  Sidebar.key.input(False{}, u, epoch, view, revision)

# No public picture or transport receipt consumes this image-only key.
# Exact metadata revision, recipient info, clock and all other fields stay.
def Sidebar.key.image(u: U.Ui, epoch: String, view: String, revision: Nat) -> String:
  Sidebar.key.input(True{}, u, epoch, view, revision)

def Sidebar.key(+u: U.Ui, epoch: String, view: String) -> String:
  Sidebar.key.revision(u, epoch, view, M.State.seq(U.Ui.state(u)))

def Sidebar.bounded(+key: String) -> Bool:
  W.Wire.within(key, 32768n)

# Limit reuse to Settings actions and refreshes while Settings is visible.
# Normal actions/commands still execute once before any region is reused.
def Sidebar.request(+p: J.Json, +u: U.Ui) -> Bool:
  +action = J.Json.get_str(p, "action", "")
  Bool.or(Bool.and(String.eq(action, "flag"), String.eq(J.Json.get_str(p, "value", ""), "settings")),
    Bool.or(String.eq(action, "settings-page"), Bool.and(String.is_empty(action), U.Ui.flag(u, "settings"))))

def Sidebar.lookup(+p: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(p, "action", ""), "fab-look"), String.is_empty(J.Json.get_str(p, "value", "")))

# Canonical-derived data is independent of the selected main panel. Ordinary
# reads may use it without acquiring Settings' rendered-sidebar reuse.
# The ordering card's own lookup (client.bend's Fab.sync, "fab-look" with
# no value, sent after each message) is a read too: it asks the hub for
# the card and changes nothing the sidebar draws that its image key misses.
def Sidebar.derived(+p: J.Json, u: U.Ui) -> Bool:
  Bool.or(String.is_empty(J.Json.get_str(p, "action", "")), Bool.or(Sidebar.lookup(p), Sidebar.request(p, u)))

# Ordinary chat keeps the original full canonical sequence in its picture
# identity. Only the already-qualified Settings path uses the metadata key.
def Sidebar.key.for(settings: Bool, +u: U.Ui, +epoch: String, +view: String, revision: Nat) -> String:
  match settings:
    case True{}: Sidebar.key.revision(u, epoch, view, revision)
    case False{}: Sidebar.key(u, epoch, view)
