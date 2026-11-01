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
def Sidebar.omit(+key: String) -> Bool:
  Bool.or(String.eq(key, "@settings.page"), Bool.or(String.eq(key, "@set.sc"), String.eq(key, "@set.focus")))

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
def Sidebar.info(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, J.KV>:
  match xs:
    case Nil{}: Nil{}
    case (+k, v) <> rest:
      List.append(&2, J.KV, Bool.pick(List<&2, J.KV>, String.eq(k, "models.v"), Nil{}, [J.KV{k, J.Str{v}}]), Sidebar.info(rest))

# Keep the original full draft map for picture/transport keys. The private
# ordinary image uses the reviewed scratch input; the three pre-existing
# Settings-only omissions above remain identical in both modes.
def Sidebar.input_drafts(image: Bool, xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, J.KV>:
  match image:
    case False{}: Sidebar.drafts(xs)
    case True{}: Sidebar.drafts(SD.Input.fields(SD.Input.of(xs)))

# The trusted hub revision can replace the full sequence only for the
# metadata-only server sidebar. It excludes exactly the three history-only
# events proved for sidebar-read.Input. Clock/UI/info/scope fences stay exact.
# The legacy entry below keeps its full-sequence independent oracle.
def Sidebar.key.input(image: Bool, u: U.Ui, epoch: String, view: String, revision: Nat) -> String:
  match u:
    case U.Ui{_, sel, dr, live, op, on, now, ad, pa, version, _, want, error, note, nf, outbox, _}:
      J.Json.show(J.Json.obj([J.KV{"epoch", J.Str{epoch}}, J.KV{"view", J.Str{view}}, J.KV{"seq", J.Json.nat(revision)},
        J.KV{"sel", J.Str{sel}}, J.KV{"drafts", J.Json.obj(Sidebar.input_drafts(image, Map.to_list(&2, String, dr)))},
        J.KV{"live", J.Json.obj(P.Session.strings(Map.to_list(&2, String, live)))}, J.KV{"open", J.Json.obj(Sidebar.flags(Map.to_list(&2, Bool, op)))},
        J.KV{"online", J.Flag{on}}, J.KV{"clock", J.Json.nat(now)}, J.KV{"adding", J.Flag{ad}}, J.KV{"panel", J.Flag{pa}}, J.KV{"version", J.Str{version}},
        J.KV{"want", J.Flag{want}}, J.KV{"error", J.Str{error}}, J.KV{"note", J.Str{note}},
        J.KV{"info", J.Json.obj(Sidebar.info(Map.to_list(&2, String, nf)))}, J.KV{"outbox", J.Json.obj(P.Session.strings(Map.to_list(&2, String, outbox)))}]))

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
