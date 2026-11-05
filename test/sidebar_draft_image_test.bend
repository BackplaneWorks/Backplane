import Base
import ./check.bend as C
import ./chat_sidebar_projection_test.bend as F
import ./chat_sidebar_projection_display_test.bend as PAGE
import ../src/core/sidebar-drafts.bend as D
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/server/thin-actor.bend as A
import ../src/server/thin-sidebar.bend as SC
import ../src/web/sidebar-slot.bend as SLOT

# These laws cover arbitrary ordered tails, not particular fixture keys.
def body_drop(+key: String, value: String, +rest: List<&2, Sigma<&2, &2, String, _ => String>>, h: {String.starts_with(key, "@") == False{} : Bool}) ->
    {D.Input.filter((key, value) <> rest) == D.Input.filter(rest) : List<&2, Sigma<&2, &2, String, _ => String>>}:
  %Equal.sym(Bool, String.starts_with(key, "@"), False{}, h)
    : {D.Input.keep(_, key, value, D.Input.filter(rest)) == D.Input.filter(rest) : List<&2, Sigma<&2, &2, String, _ => String>>}
  {==}

def scratch_keep(+key: String, +value: String, +rest: List<&2, Sigma<&2, &2, String, _ => String>>, h: {String.starts_with(key, "@") == True{} : Bool}) ->
    {D.Input.filter((key, value) <> rest) == (key, value) <> D.Input.filter(rest) : List<&2, Sigma<&2, &2, String, _ => String>>}:
  %Equal.sym(Bool, String.starts_with(key, "@"), True{}, h)
    : {D.Input.keep(_, key, value, D.Input.filter(rest)) == (key, value) <> D.Input.filter(rest) : List<&2, Sigma<&2, &2, String, _ => String>>}
  {==}

def settings_separate(u: U.Ui, epoch: String, view: String, revision: Nat) ->
    {A.SidebarImage.key.input(True{}, u, epoch, view, revision) == "" : String}:
  {==}

def fields(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> String:
  J.Json.show(J.Json.obj(SC.Sidebar.drafts(D.Input.fields(D.Input.of(xs)))))

def ui() -> U.Ui:
  F.ordinary(F.state(), 10n)

def key(u: U.Ui) -> String:
  A.SidebarImage.key.input(False{}, u, "epoch", "view", 1n)

def compare(+u: U.Ui) -> String:
  F.equal(key(ui()), key(u))

def exact(+before: U.Ui, +after: U.Ui) -> String:
  +node = SLOT.Slot.sidebar(J.Json.get_obj(PAGE.frame(before, J.Null{}), "shown"))
  +cached = A.SidebarImage.lookup([A.SidebarImage{key(before), node}], key(after))
  F.equal(J.Json.show(PAGE.frame(after, cached)), J.Json.show(PAGE.frame(after, J.Null{})))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("empty scratch input", fields(Nil{}), "{}")
    C.Check.str("body values do not enter private input", fields([("self", "PRIVATE 日本語"), ("other", "another body")]), "{}")
    C.Check.str("scratch retains order and exact values", fields([("self", "private"), ("@look", "日本語"), ("@future", "unknown value"), ("other", "private")]), "{\"@look\":\"日本語\",\"@future\":\"unknown value\"}")
    C.Check.str("at-prefixed ids stay conservative", fields([("@thread-like-id", "body or scratch")]), "{\"@thread-like-id\":\"body or scratch\"}")
    C.Check.str("ordinary body reuses private image", compare(U.Ui.scratch_set(ui(), "self", "New ordinary draft")), "same")
    C.Check.str("IME body reuses private image", compare(U.Ui.scratch_set(ui(), "self", "未確定 日本語 👩🏽‍💻")), "same")
    C.Check.str("full picture still includes ordinary body", F.equal(SC.Sidebar.key(ui(), "epoch", "view"), SC.Sidebar.key(U.Ui.scratch_set(ui(), "self", "Changed draft"), "epoch", "view")), "different")
    C.Check.str("search scratch invalidates private image", compare(U.Ui.scratch_set(ui(), "@f.pq", "Other")), "different")
    C.Check.str("unknown scratch remains an input", compare(U.Ui.scratch_set(ui(), "@future.scratch", "unknown value")), "different")
    C.Check.str("bot form scratch remains an input", compare(U.Ui.scratch_set(ui(), "@f.persona", "Current persona")), "different")
    C.Check.str("row menu scratch remains an input", compare(U.Ui.scratch_set(ui(), "@menu", "t:other")), "different")
    C.Check.str("exact clock remains an input", compare(F.ordinary(F.state(), 11n)), "different")
    C.Check.str("owner view remains an input", F.equal(key(ui()), A.SidebarImage.key.input(False{}, ui(), "epoch", "other view", 1n)), "different")
    C.Check.str("origin epoch remains an input", F.equal(key(ui()), A.SidebarImage.key.input(False{}, ui(), "other epoch", "view", 1n)), "different")
    C.Check.str("trusted metadata revision remains an input", F.equal(key(ui()), A.SidebarImage.key.input(False{}, ui(), "epoch", "view", 2n)), "different")
    C.Check.str("fresh recipient info remains an input", compare(U.Ui.info_set(ui(), "models", "Fresh labels")), "different")
    C.Check.str("first edit dismissal info still invalidates", compare(U.Ui.info_set(ui(), "sk.off", "")), "different")
    C.Check.str("online state remains an input", compare(U.Ui.online(ui(), False{})), "different")
    C.Check.str("fold state remains an input", compare(U.Ui.flag_on(ui(), "active-settled")), "different")
    C.Check.str("Settings image stays separate", A.SidebarImage.key.input(True{}, ui(), "epoch", "view", 1n), "")
    C.Check.str("fab lookup mark reuses private image", compare(U.Ui.scratch_set(ui(), "@fab.at", "onon#t1#completed")), "same")
    C.Check.str("review reply bookkeeping reuses private image", compare(U.Ui.scratch_set(ui(), "@rp.req:17", "t1:bom")), "same")
    C.Check.str("telemetry reply bookkeeping reuses private image", compare(U.Ui.scratch_set(ui(), "@telemetry.req:17", "1")), "same")
    C.Check.str("desk watch info reuses private image", compare(U.Ui.info_set(ui(), "desk", "t1")), "same")
    C.Check.str("tool-call fold reuses private image", compare(U.Ui.info_set(ui(), "fold:msg_1", "o")), "same")
    C.Check.str("viewer tab and panel reuse private image", compare(U.Ui.info_set(U.Ui.info_set(ui(), "view.kind:t1", "board"), "panel:t1", "1")), "same")
    C.Check.str("subagents fold remains an input", compare(U.Ui.info_set(ui(), "subs:t1", "1")), "different")
    C.Check.str("whole page with a tool-call fold and retained sidebar", exact(ui(), U.Ui.info_set(ui(), "fold:msg_1", "o")), "same")
    C.Check.str("whole page with viewer choices and retained sidebar", exact(ui(), U.Ui.info_set(U.Ui.info_set(U.Ui.info_set(ui(), "view.kind:t1", "board"), "panel:t1", "1"), "hist.at:t1", "3")), "same")
    C.Check.str("fab-like scratch remains an input", compare(U.Ui.scratch_set(ui(), "@fab.atx", "1")), "different")
    C.Check.str("whole page with fab mark and retained sidebar", exact(ui(), U.Ui.scratch_set(ui(), "@fab.at", "onon#t1#completed")), "same")
    C.Check.str("whole page with reply bookkeeping and retained sidebar", exact(ui(), U.Ui.scratch_set(U.Ui.scratch_set(ui(), "@rp.req:17", "t1:bom"), "@telemetry.req:18", "1")), "same")
    C.Check.str("whole page with desk info and retained sidebar", exact(ui(), U.Ui.info_set(ui(), "desk", "t1")), "same")
    C.Check.str("whole page with new draft and retained sidebar", exact(ui(), U.Ui.scratch_set(ui(), "self", "New current body")), "same")
    C.Check.str("whole page with IME and retained sidebar", exact(ui(), U.Ui.scratch_set(ui(), "self", "未確定 日本語 👩🏽‍💻")), "same")
    C.Check.str("whole page with search miss is fresh", exact(ui(), U.Ui.scratch_set(ui(), "@f.pq", "Other")), "same")
    C.Check.str("whole page with current info miss is fresh", exact(ui(), U.Ui.info_set(ui(), "sk.off", "")), "same")
