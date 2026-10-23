import Base
import ../core/client.bend as U
import ../core/mailview.bend as MV
import ../core/plane.bend as P
import ./dom.bend as D
import ./mail-actions.bend as MA
import ../core/model.bend as M
import ../core/bot.bend as BT
import ../core/mail.bend as ML
import ../core/i18n.bend as I

# Draw the existing shared Mailv cache and actions. No separate mailbox store,
# request protocol or frontend interpretation of reply/needs-attention state.
def WM.button(+u: U.Ui, +value: String, +label: String) -> D.Node:
  D.Dom.button(Bool.pick(String, String.eq(U.Ui.scratch(u, "@mr.status"), value), "chip on", "chip"), "mail-filter", value, label, label)

def WM.filters(+u: U.Ui) -> D.Node:
  D.Dom.div("mail-filters", [WM.button(u, "unread", "Unread"), WM.button(u, "act", "Act"), WM.button(u, "fyi", "FYI"), WM.button(u, "snoozed", "Snoozed"), WM.button(u, "done", "Done"), WM.button(u, "due", "Due"), WM.button(u, "today", "Today"), WM.button(u, "overview", "Overview"), WM.button(u, "", "All")])

def WM.field(+id: String, +value: String, +action: String, +placeholder: String) -> D.Node:
  D.El{"input", id, [D.At{"id", id}, D.At{"class", "field"}, D.At{"aria-label", placeholder}, D.At{"placeholder", placeholder}, D.At{"value", value}, D.On{"input", action}], Nil{}}

def WM.row(+r: MV.Row, +u: U.Ui) -> D.Node:
  D.El{"button", "mail:" ++ MV.Row.id(r), [D.At{"class", "mail-row" ++ Bool.pick(String, MV.Row.unread(r), " unread", "") ++ Bool.pick(String, String.eq(MV.Row.id(r), U.Ui.scratch(u, "@mr.conv")), " on", "")}, D.At{"data-value", MV.Row.id(r)}, D.On{"click", "mail-open"}],
    [D.Dom.span("mail-subject", MV.Row.subject(r)), D.Dom.span("mail-meta", MV.Row.who(r) ++ " · " ++ MV.Row.project(r) ++ " · " ++ MV.Row.status(r)), D.Dom.span("mail-preview", MV.Row.head(r))]}

def WM.rows(rs: List<&2, MV.Row>, +u: U.Ui) -> List<&2, D.Node>:
  match rs:
    case Nil{}: [D.Dom.span("mail-empty", "No conversations in this page.")]
    case r <> Nil{}: [WM.row(r, u)]
    case r <> t: WM.row(r, u) <> WM.rows(t, u)

def WM.list.rows(search: Bool, +u: U.Ui) -> List<&2, MV.Row>:
  match search:
    case False{}: MV.Mv.rows(U.Mailv.cache(u), U.Mailv.key(u), U.Mailv.skip(u), 50n)
    case True{}: List.take(&2, MV.Row, List.drop(&2, MV.Row, MV.Find.hits(U.Mailv.find(u)), U.Mailv.skip(u)), 50n)

def WM.list(+u: U.Ui) -> D.Node:
  D.El{"section", "mail-list", [D.At{"class", "mail-list"}, D.At{"aria-label", "Conversations"}], List.append(&2, D.Node,
    WM.rows(WM.list.rows(Bool.not(String.is_empty(U.Ui.scratch(u, "@f.mrsearch"))), u), u),
    [D.Dom.div("mail-pages", [D.Dom.button("", "web-mail-page", Nat.show(Nat.add(Nat.sub(U.Mailv.skip(u), 50n), 2n)), "Previous page", "Previous"), D.Dom.span("", "Page " ++ Nat.show(Nat.add(1n, Nat.div(U.Mailv.skip(u), 50n)))), D.Dom.button("", "web-mail-page", Nat.show(Nat.add(U.Mailv.skip(u), 52n)), "Next page", "Next")])])}

def WM.bubble(b: MV.Bub, +now: Nat) -> D.Node:
  match b:
    case MV.Bub{+id, _, who, kind, want, at, text, _}:
      D.El{"article", "bubble:" ++ id, [D.At{"class", "mail-bubble"}, D.At{"data-mail-id", id}], [D.Dom.span("mail-meta", who ++ " · " ++ kind ++ " · " ++ want ++ " · " ++ MV.Mv.ago(now, at)), D.Dom.div("mail-text", [D.Txt{text}])]}

def WM.bubbles(bs: List<&2, MV.Bub>, +now: Nat) -> List<&2, D.Node>:
  match bs:
    case Nil{}: Nil{}
    case b <> t: WM.bubble(b, now) <> WM.bubbles(t, now)

# A header without a returned body is not evidence of an empty conversation.
# Keep transport errors visible separately and avoid showing a blank reader.
def WM.read_hint(loading: Bool, failed: Bool, has_messages: Bool) -> String:
  match loading failed has_messages:
    case _ _ True{}: ""
    case True{} _ False{}: "Loading message bodies…"
    case False{} True{} False{}: "Messages could not be loaded. See the mail error above."
    case False{} False{} False{}: "No message bodies were returned for this conversation."

def WM.read_status(+u: U.Ui) -> D.Node:
  D.El{"div", "mail-read-status", [D.At{"class", "mail-empty"}, D.At{"role", "status"}, D.At{"aria-live", "polite"}],
    [D.Txt{WM.read_hint(Bool.not(String.is_empty(U.Ui.scratch(u, "@mr.read"))), Bool.not(String.is_empty(MA.MA.get(u, "read-error"))), Bool.not(List.is_empty(&2, MV.Bub, U.Mailv.bubbles(u))))}]}

def WM.targets(os: List<&2, MV.Opt>, +u: U.Ui) -> List<&2, D.Node>:
  match os:
    case Nil{}: Nil{}
    case MV.Opt{+addr, +label} <> t:
      D.Dom.button(Bool.pick(String, String.eq(U.Ui.scratch(u, "@mr.to"), addr), "chip on", "chip"), "mail-to", addr, label, label) <> WM.targets(t, u)

def WM.conversation(+u: U.Ui) -> D.Node:
  +rd = MV.Mv.read_of(U.Mailv.cache(u), U.Ui.scratch(u, "@mr.conv"))
  D.El{"section", "mail-conversation", [D.At{"class", "mail-conversation"}, D.At{"aria-label", "Conversation"}], [
    D.Dom.div("mail-toolbar", [D.Dom.button("", "mail-close", "", "Close conversation", "Back"), D.Dom.span("mail-subject", MV.Read.subject(rd)), D.Dom.button("", "mail-more", "", "Load earlier messages", "Load earlier"), D.Dom.button("", "mail-mark", "done", "Mark conversation done", "Done")]),
    D.El{"div", "mail-messages", [D.At{"class", "mail-messages"}, D.At{"data-conv", U.Ui.scratch(u, "@mr.conv")}, D.At{"role", "log"}, D.At{"aria-live", "polite"}], WM.read_status(u) <> WM.bubbles(U.Mailv.bubbles(u), U.Side.now(u))},
    D.Dom.div("mail-targets", WM.targets(U.Mailv.targets(u), u)),
    D.El{"textarea", "mail-reply", [D.At{"id", "mail-reply"}, D.At{"class", "field"}, D.At{"aria-label", "Reply to a participant"}, D.At{"placeholder", "Reply to a participant"}, D.At{"data-bfield", "mrreply"}, D.At{"value", U.Ui.scratch(u, "@f.mrreply")}, D.On{"input", "bfield"}], Nil{}},
    D.Dom.span("mail-send-status", MA.MA.status(u)),
    D.Dom.button("primary", "mail-send", "", "Send reply", "Send")]}

def WM.reader(open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case False{}: Nil{}
    case True{}: [WM.conversation(u)]

# a pin says what the row asks first (Plane.why: approval, question, failed
# run), then the title, its project and machine; a click opens the thread
def WM.pin.text(+why: String, +title: String, +state: String, +machine: String, +note: String) -> String:
  Bool.pick(String, String.is_empty(why), title ++ " · " ++ state, why ++ " · " ++ title) ++ Bool.pick(String, String.is_empty(machine), "", " · " ++ machine) ++ Bool.pick(String, String.is_empty(note), "", " · " ++ note)

def WM.pins(rs: List<&2, P.PRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}: Nil{}
    case P.PRow{_, +id, action, value, title, note, machine, +state, _, _} <> t:
      D.El{"button", "mail-pin:" ++ id, [D.At{"class", "mail-pin " ++ state}, D.At{"data-value", value}, D.On{"click", action}], [D.Txt{WM.pin.text(P.Plane.why(I.En{}, state), title, state, machine, note)}]} <> WM.pins(t)

def WM.bots(rs: List<&2, P.PRow>, +u: U.Ui) -> List<&2, D.Node>:
  match rs:
    case Nil{}: Nil{}
    case P.PRow{_, +id, _, _, +title, _, _, _, _, _} <> t:
      D.Dom.button(Bool.pick(String, String.eq(U.Mailv.bot(u), id), "chip on", "chip"), "mail-filter", "bot=" ++ id, title, title) <> WM.bots(t, u)

def WM.keep(on: Bool, r: P.PRow, rest: List<&2, P.PRow>) -> List<&2, P.PRow>:
  match on:
    case True{}: r <> rest
    case False{}: rest

def WM.run_owned(r: Maybe<&2, ML.Run>, +bot: String) -> Bool:
  match r:
    case Some{x}: String.eq(ML.Run.bot(x), bot)
    case None{}: False{}

def WM.owned(+r: P.PRow, +u: U.Ui) -> Bool:
  match r:
    case P.PRow{_, +id, _, +val, _, _, +machine, _, _, _}:
      Bool.and(Bool.or(String.is_empty(U.Ui.scratch(u, "@mr.machine")), String.eq(machine, U.Ui.scratch(u, "@mr.machine"))),
        Bool.or(String.eq(U.Mailv.bot(u), "*"), Bool.or(String.eq(id, U.Mailv.bot(u)), Bool.or(String.eq(id, BT.Bot.thread(BT.World.bot(M.State.bots(U.Ui.state(u)), U.Mailv.bot(u)))),
          Bool.or(String.starts_with(val, U.Mailv.bot(u) ++ "|"), WM.run_owned(ML.Box.run_of(M.State.mail(U.Ui.state(u)), id), U.Mailv.bot(u)))))))

def WM.for_bot(rs: List<&2, P.PRow>, +u: U.Ui) -> List<&2, P.PRow>:
  match rs:
    case Nil{}: Nil{}
    case +r <> t: WM.keep(WM.owned(r, u), r, WM.for_bot(t, u))

def WM.routines(rs: List<&2, P.PRow>, +running: Bool) -> List<&2, P.PRow>:
  match rs:
    case Nil{}: Nil{}
    case +r <> t: WM.keep(Bool.not(Bool.xor(String.eq(P.PRow.state(r), "working"), running)), r, WM.routines(t, running))

def WM.running(+p: P.Plane) -> List<&2, P.PRow>:
  List.concat(&2, P.PRow, [P.Plane.run(p), P.Plane.locks(p), WM.routines(P.Plane.routines(p), True{})])

def WM.has_id(rs: List<&2, P.PRow>, +id: String) -> Bool:
  match rs:
    case Nil{}: False{}
    case r <> t: Bool.or(String.eq(P.PRow.id(r), id), WM.has_id(t, id))

def WM.live_runs(rs: List<&2, ML.Run>, +u: U.Ui, +already: List<&2, P.PRow>) -> List<&2, P.PRow>:
  match rs:
    case Nil{}: Nil{}
    case +r <> t:
      WM.keep(Bool.and(ML.Run.live(r), Bool.and(Bool.or(String.eq(U.Mailv.bot(u), "*"), String.eq(U.Mailv.bot(u), ML.Run.bot(r))), Bool.not(WM.has_id(already, ML.Run.thread(r))))),
        P.PRow{"run", ML.Run.thread(r), "select", ML.Run.thread(r), "Run · " ++ ML.Run.bot(r), ML.Run.conv(r), "", "working", ML.Run.at(r), False{}}, WM.live_runs(t, u, already))

def WM.secondary.k(i: Nat, +p: P.Plane) -> List<&2, P.PRow>:
  match i:
    case 0n: P.Plane.idle(p)
    case 1n: WM.routines(P.Plane.routines(p), False{})
    case 2n: P.Plane.rooms(p)
    case 3n: List.concat(&2, P.PRow, [P.Plane.need(p), P.Plane.run(p), P.Plane.locks(p), P.Plane.routines(p), P.Plane.rooms(p), P.Plane.idle(p), P.Plane.bots(p)])
    case _: Nil{}

def WM.secondary(status: String, p: P.Plane) -> List<&2, P.PRow>:
  WM.secondary.k(U.Keys.pos(["today", "due", "unread", "overview"], status), p)

def WM.running_if(open: Bool, rs: List<&2, P.PRow>) -> List<&2, D.Node>:
  match open:
    case True{}: WM.pins(rs)
    case False{}: Nil{}

# every row that needs the person, whatever bot or machine the mail shows:
# the same rows the uberbot's count counts, asks told from failed runs
def WM.need.words(+need: List<&2, P.PRow>) -> String:
  Bool.pick(String, List.is_empty(&2, P.PRow, need), "0", P.Plane.counts.of(I.En{}, List.length(&2, P.PRow, need), P.Plane.failed(need), 0n))

def WM.attention(+p: P.Plane, +u: U.Ui) -> D.Node:
  +need = P.Plane.need(p)
  +running = List.append(&2, P.PRow, WM.for_bot(WM.running(p), u), WM.live_runs(Map.values(&2, ML.Run, ML.Box.runs(M.State.mail(U.Ui.state(u)))), u, WM.running(p)))
  D.Dom.div("mail-pins", List.concat(&2, D.Node, [[D.Dom.span("mail-meta", "Needs you · " ++ WM.need.words(need))], WM.pins(need),
    [D.Dom.button("chip", "mail-run", "", "Show running work", Nat.show(List.length(&2, P.PRow, running)) ++ " running · " ++ Nat.show(List.length(&2, P.PRow, P.Plane.locks(p))) ++ " locks")],
    WM.running_if(U.Mailv.run_open(u), running), WM.pins(WM.for_bot(WM.secondary(U.Ui.scratch(u, "@mr.status"), p), u))]))

def WM.machines(ms: List<&2, P.PMach>) -> List<&2, D.Node>:
  match ms:
    case Nil{}: Nil{}
    case P.PMach{+name, +state} <> t:
      D.Dom.button("chip", "mail-filter", "machine=" ++ Bool.pick(String, String.eq(state, "here"), "", name), state, name ++ " · " ++ state) <> WM.machines(t)

def WM.owner_chip(+u: U.Ui) -> D.Node:
  +id = U.Mailv.owner(u)
  D.Dom.button(Bool.pick(String, String.eq(U.Mailv.bot(u), id), "chip on", "chip"), "mail-filter", "bot=" ++ id, "Owner mail", BT.Bot.name(BT.World.bot(M.State.bots(U.Ui.state(u)), id)))

def WM.page(+u: U.Ui) -> D.Node:
  +p = P.Plane.of(u, U.Side.now(u))
  D.El{"div", "mailroom", [D.At{"id", "mailroom"}, D.At{"class", "mailroom" ++ Bool.pick(String, String.is_empty(U.Ui.scratch(u, "@mr.conv")), "", " reading")}], [
    D.Dom.div("mail-toolbar", [D.Dom.span("mail-heading", "Mailroom"), WM.field("mail-search", U.Ui.scratch(u, "@f.mrsearch"), "mail-search", "Search mail"), WM.field("mail-project", U.Ui.scratch(u, "@mr.project"), "web-mail-project", "Project")]),
    D.Dom.div("mail-machines", WM.machines(P.Plane.machines(p))),
    D.Dom.div("mail-bots", D.Dom.button("chip", "mail-filter", "bot=*", "All mail", "All mail") <> WM.owner_chip(u) <> WM.bots(P.Plane.bots(p), u)),
    D.El{"div", "mail-error", [D.At{"class", "mail-error"}, D.At{"role", "alert"}], [D.Txt{MA.MA.errors(u)} ]},
    WM.filters(u),
    WM.attention(p, u),
    D.Dom.div("mail-columns", WM.list(u) <> WM.reader(Bool.not(String.is_empty(U.Ui.scratch(u, "@mr.conv"))), u))]}
