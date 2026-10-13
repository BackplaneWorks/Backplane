import Base
import ../core/model.bend as M
import ./dom.bend as D
import ./icon.bend as WI
import ./cat.bend as WC
import ./space.bend as WS
import ../core/client.bend as U
import ../core/bot.bend as BT
import ../core/botui.bend as BI
import ../core/cron.bend as CR
import ../core/plane.bend as PL
import ../core/uber.bend as UB

# The bots on the web (docs/bots.md): the sidebar's cats and rooms, a bot's
# header, tabs and tab bodies, a room, a bot on another machine. What is
# shown comes from core/botui.bend; this file only lays it out. A bot's Chat
# tab is the thread view (view.bend).
#
# Form fields send action "bfield" with "<field>\x1f<text>": an input with
# data-bfield="<field>" gets that prefix from host.js.

def WB.sep() -> String:
  "\u{1f}"

# a "bfield" value that sets a field
def WB.bf(+field: String, +v: String) -> String:
  field ++ WB.sep() ++ v

def WB.enter(+act: String) -> List<&2, D.Attr>:
  Bool.pick(List<&2, D.Attr>, String.is_empty(act), Nil{}, [D.At{"data-enter", act}])

# a one-line form field; Enter sends `act` (none when "")
def WB.input(+id: String, +field: String, +value: String, +ph: String, +act: String) -> D.Node:
  D.El{"input", id, List.append(&2, D.Attr, [D.At{"id", id}, D.At{"class", "field"}, D.At{"placeholder", ph}, D.At{"autocomplete", "off"}, D.At{"spellcheck", "false"},
    D.At{"value", value}, D.At{"data-bfield", field}, D.On{"input", "bfield"}], WB.enter(act)), Nil{}}

def WB.secret(+id: String, +field: String, +value: String, +ph: String) -> D.Node:
  D.El{"input", id, [D.At{"id", id}, D.At{"class", "field"}, D.At{"type", "password"}, D.At{"placeholder", ph}, D.At{"autocomplete", "off"},
    D.At{"value", value}, D.At{"data-bfield", field}, D.On{"input", "bfield"}], Nil{}}

def WB.area(+id: String, +field: String, +value: String, +ph: String) -> D.Node:
  D.El{"textarea", id, [D.At{"id", id}, D.At{"class", "field"}, D.At{"rows", "3"}, D.At{"placeholder", ph}, D.At{"value", value},
    D.At{"data-bfield", field}, D.On{"input", "bfield"}], Nil{}}

def WB.div(+cls: String, kids: List<&2, D.Node>) -> D.Node:
  D.El{"div", "", [D.At{"class", cls}], kids}

def WB.note(+s: String) -> D.Node:
  WB.div("bot-note", [D.Txt{s}])

def WB.btns(kids: List<&2, D.Node>) -> D.Node:
  WB.div("bform-btns", kids)

def WB.ago(+now: Nat, +at: Nat) -> String:
  Bool.pick(String, Nat.is_eq(at, 0n), "", U.Time.ago(now, at))

def WB.mood.cls(+m: String) -> String:
  "mood " ++ m

def WB.running(t: M.TurnState) -> Bool:
  match t:
    case M.Running{}:
      True{}
    case _:
      False{}

def WB.keep(-A: Data, hit: Bool, x: A, rest: List<&2, A>) -> List<&2, A>:
  match hit:
    case True{}:
      x <> rest
    case False{}:
      rest

# Sidebar
# -------

# a bot: its cat, its name and its mood; a bot elsewhere shows its
# machine (BU.row.title); a click does what BU.act says
def WB.row(+r: BI.BRow) -> D.Node:
  match r:
    case BI.BRow{+id, _, _, +look, +mood, note, sel, _, _, _}:
      D.El{"div", "bot:" ++ id, [D.At{"class", Bool.pick(String, sel, "bot-row on", "bot-row")}, D.At{"title", id}, D.At{"data-value", id}, D.On{"click", BI.BU.act(r)}],
        [WC.WCat.img(look, mood, 28), WB.div("bot-text", [D.Dom.span("bot-name", BI.BU.row.title(r)), D.Dom.span(WB.mood.cls(mood), note)])]}

def WB.rows(rs: List<&2, BI.BRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      WB.row(r) <> WB.rows(t)

def WB.rrow(r: BI.RRow) -> D.Node:
  match r:
    case BI.RRow{+id, name, +n, sel}:
      D.El{"div", "room:" ++ id, [D.At{"class", Bool.pick(String, sel, "room-row on", "room-row")}, D.At{"data-value", id}, D.On{"click", "room"}],
        [D.Dom.span("hash", "#"), D.Dom.span("title", name), D.El{"span", "", [D.At{"class", "ago"}, D.At{"title", Nat.show(n) ++ " members"}], [D.Txt{Nat.show(n)}]}]}

def WB.rrows(rs: List<&2, BI.RRow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      WB.rrow(r) <> WB.rrows(t)

def WB.pchips(xs: List<&2, String>, +cur: String) -> List<&2, D.Node>:
  match xs:
    case Nil{}:
      Nil{}
    case +p <> t:
      D.Dom.button(Bool.pick(String, String.eq(p, cur), "on", ""), "bfield", WB.bf("provider", p), p, p) <> WB.pchips(t, cur)

def WB.prov(+u: U.Ui) -> String:
  Bool.pick(String, String.is_empty(BI.BU.field(u, "provider")), M.Setting.get(U.Ui.state(u), "agent.provider", "claude"), BI.BU.field(u, "provider"))

# the new bot: a name, a persona, a provider
def WB.bot.form(open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      [WB.div("bform", [
        WB.input("bot-name", "name", BI.BU.field(u, "name"), U.Bots.name.hint(u), "bot-create"),
        WB.area("bot-persona", "persona", BI.BU.field(u, "persona"), "Persona: who it is, what it does"),
        WB.div("chips-row", WB.pchips(["claude", "codex", "grok", "muse", "antigravity"], WB.prov(u))),
        WB.btns([D.Dom.button("primary", "bot-create", "", "Make the bot", "Create"), D.Dom.button("", "bot-new", "", "Cancel", "Cancel")])])]
    case False{}:
      Nil{}

def WB.picks(ps: List<&2, BI.Pick>) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case BI.Pick{id, name, on} <> t:
      D.Dom.button(Bool.pick(String, on, "pick on", "pick"), "room-pick", id, "Member", name) <> WB.picks(t)

# a new room: a name and the bots in it
def WB.room.form(open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      [WB.div("bform", [
        WB.input("room-name", "rname", BI.BU.field(u, "rname"), "Name", "room-save"),
        WB.div("chips-row", WB.picks(BI.BU.picks(u))),
        WB.btns([D.Dom.button("primary", "room-save", "", "Make the room", "Save"), D.Dom.button("", "room-new", "", "Cancel", "Cancel")])])]
    case False{}:
      Nil{}

def WB.shead(+label: String, +act: String, +title: String) -> D.Node:
  D.El{"div", "", [D.At{"class", "project-head bots-head"}], [D.Dom.span("name", label), WI.WIcon.button("icon", act, "", title, "plus", 16, "")]}

# the sidebar's bots and rooms (below the projects)
def WB.side(+u: U.Ui) -> List<&2, D.Node>:
  [D.El{"section", "bots", [D.At{"class", "bots"}], List.concat(&2, D.Node, [
    [WB.shead("Bots", "bot-new", "New bot")],
    WB.bot.form(BI.BU.open(u, "@bnew"), u),
    WB.rows(BI.BU.rows(u)),
    [D.Dom.button("project-head board-entry", "tb-open", "", "Open Board", U.BoardUi.label(u)), WB.shead("Rooms", "room-new", "New room")],
    WB.room.form(BI.BU.open(u, "@rnew"), u),
    WB.rrows(BI.BU.rooms(u))])}]

# whether the main area shows a bot, a room or a bot elsewhere
def WB.shown(v: BI.View) -> Bool:
  match v:
    case BI.VThread{}:
      False{}
    case _:
      True{}

# A bot
# -----

# a bot's header (BU.head, the same for a bot here, one mirrored from a
# linked machine and one only reported by one): its cat, its name with
# where it lives, its mood, then sub (its model) and tools (Stop)
def WB.header(hd: BI.Head, sub: List<&2, D.Node>, tools: List<&2, D.Node>) -> D.Node:
  match hd:
    case BI.Head{+title, +look, +mood, +note, _, _, _, _}:
      D.El{"header", "", [D.At{"class", "thread-head bot-head"}], [
        D.Dom.button("back", "select", "", "Back", "‹"),
        WC.WCat.img(look, mood, 72),
        WB.div("bot-id", D.Dom.span("bot-title", title) <> D.Dom.span(WB.mood.cls(mood), note) <> sub),
        D.El{"div", "", [D.At{"class", "tools"}], tools}]}

def WB.head.t(t: M.Thread, hd: BI.Head) -> D.Node:
  match t:
    case M.Thread{_, _, _, _, _, turn, _, _, _, _, _, _, _, _, _, +pr, +mo, _, _, _, _}:
      WB.header(hd, [D.Dom.span("bot-model", U.Models.label(pr, mo))], D.Dom.when(WB.running(turn), D.Dom.button("", "interrupt", "", "Stop the running turn", "Stop")))

# the bot's header: its cat, name, mood and model
def WB.head(+u: U.Ui, +b: BT.Bot, hd: BI.Head) -> D.Node:
  WB.head.t(U.Ui.thread(U.Ui.state(u), BT.Bot.thread(b)), hd)

def WB.tabs.go(ts: List<&2, String>, +cur: String) -> List<&2, D.Node>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> r:
      D.Dom.button(Bool.pick(String, String.eq(t, cur), "tab on", "tab"), "bot-tab", t, BI.BU.tab.label(t), BI.BU.tab.label(t)) <> WB.tabs.go(r, cur)

# the tabs a bot has (BU.head), the one shown on
def WB.tabs(+hd: BI.Head) -> D.Node:
  D.El{"nav", "", [D.At{"class", "bot-tabs"}], WB.tabs.go(BI.Head.tabs(hd), BI.Head.tab(hd))}

# Browser: the bot's page, a frame the hub serves (the pairing cookie
# carries the token, as for /img); n changes with each "shot" message
def WB.browser.k(none: Bool, +n: String, +id: String, +name: String) -> D.Node:
  match none:
    case True{}:
      WB.note(name ++ " hasn't opened the browser yet.")
    case False{}:
      D.El{"a", "", [D.At{"class", "bot-screen"}, D.At{"href", "/bots/" ++ id ++ "/screen.jpg?n=" ++ n}, D.At{"target", "_blank"}, D.At{"data-lightbox", "/bots/" ++ id ++ "/screen.jpg?n=" ++ n}],
        [D.El{"img", "", [D.At{"src", "/bots/" ++ id ++ "/screen.jpg?n=" ++ n}, D.At{"alt", name ++ "'s browser"}], Nil{}}]}

def WB.browser(+u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.browser.k(String.is_empty(U.Ui.info(u, "shot." ++ BT.Bot.id(b))), U.Ui.info(u, "shot." ++ BT.Bot.id(b)), BT.Bot.id(b), BT.Bot.name(b))

# Memory: grouped by kind
def WB.kind.of(ms: List<&2, BT.Mem>, +k: String) -> List<&2, BT.Mem>:
  match ms:
    case Nil{}:
      Nil{}
    case BT.Mem{b, ke, +kd, x, tg, c, up} <> t:
      WB.keep(BT.Mem, String.eq(kd, k), BT.Mem{b, ke, kd, x, tg, c, up}, WB.kind.of(t, k))

def WB.tags(+tags: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(tags), Nil{}, [D.Dom.span("item-tags", tags)])

# a memory; Forget only for a bot here (BU.owns)
def WB.mem(m: BT.Mem, +own: Bool, +now: Nat) -> D.Node:
  match m:
    case BT.Mem{_, +key, _, text, +tags, _, +up}:
      D.El{"div", "mem:" ++ key, [D.At{"class", "item"}], [
        WB.div("item-main", D.Dom.cat([D.Dom.span("item-key", key), D.Dom.span("item-text", text)], WB.tags(tags))),
        D.Dom.span("ago", WB.ago(now, up)),
        WB.div("item-btns", D.Dom.when(own, D.Dom.button("", "mem-forget", key, "Forget this", "Forget")))]}

def WB.mems(ms: List<&2, BT.Mem>, +own: Bool, +now: Nat) -> List<&2, D.Node>:
  match ms:
    case Nil{}:
      Nil{}
    case m <> t:
      WB.mem(m, own, now) <> WB.mems(t, own, now)

def WB.mgroup(ms: List<&2, BT.Mem>, +k: String, +own: Bool, +now: Nat) -> List<&2, D.Node>:
  match ms:
    case Nil{}:
      Nil{}
    case m <> t:
      [WB.div("group", D.Dom.span("group-label", k) <> WB.mems(m <> t, own, now))]

def WB.mgroups(ks: List<&2, String>, +ms: List<&2, BT.Mem>, +own: Bool, +now: Nat) -> List<&2, D.Node>:
  match ks:
    case Nil{}:
      Nil{}
    case +k <> t:
      List.append(&2, D.Node, WB.mgroup(WB.kind.of(ms, k), k, own, now), WB.mgroups(t, ms, own, now))

def WB.memory.k(+ms: List<&2, BT.Mem>, +own: Bool, +now: Nat) -> D.Node:
  WB.div("bot-list", Bool.pick(List<&2, D.Node>, List.is_empty(&2, BT.Mem, ms), [WB.note("Nothing remembered yet.")], WB.mgroups(BT.Mem.kinds(), ms, own, now)))

def WB.memory(+u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.memory.k(BI.BU.mems(u, BT.Bot.id(b)), BI.BU.owns(BT.Bot.id(b)), U.Side.now(u))

# Routines
def WB.when(+cron: String) -> String:
  Bool.pick(String, String.is_empty(CR.Cron.describe(cron)), cron, CR.Cron.describe(cron))

# a routine's buttons: Run now for any bot, the rest for a bot here
def WB.routine.btns(own: Bool, +id: String, +on: Bool) -> List<&2, D.Node>:
  match own:
    case True{}:
      [D.Dom.button(Bool.pick(String, on, "on", ""), "routine-toggle", id, Bool.pick(String, on, "Turn off", "Turn on"), Bool.pick(String, on, "On", "Off")),
       D.Dom.button("", "routine-run", id, "Run it now", "Run now"),
       D.Dom.button("", "routine-edit", id, "Edit", "Edit"),
       D.Dom.button("", "routine-delete", id, "Delete", "Delete")]
    case False{}:
      [D.Dom.button("", "routine-run", id, "Run it now", "Run now")]

def WB.routine(r: BT.Routine, +own: Bool, +now: Nat) -> D.Node:
  match r:
    case BT.Routine{+id, _, name, +cron, prompt, +on, +last, _, _}:
      D.El{"div", "ro:" ++ id, [D.At{"class", Bool.pick(String, on, "item", "item off")}], [
        WB.div("item-main", [D.Dom.span("item-key", name), D.Dom.span("item-when", WB.when(cron)), D.Dom.span("item-text", prompt)]),
        D.El{"span", "", [D.At{"class", "ago"}, D.At{"title", "Last run"}], [D.Txt{WB.ago(now, last)}]},
        WB.div("item-btns", WB.routine.btns(own, id, on))]}

def WB.routines.go(rs: List<&2, BT.Routine>, +own: Bool, +now: Nat) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      WB.routine(r, own, now) <> WB.routines.go(t, own, now)

def WB.cron.hint.k(empty: Bool, bad: Bool, +d: String) -> List<&2, D.Node>:
  match empty bad:
    case True{} _:
      Nil{}
    case False{} True{}:
      [D.Dom.span("hint bad", "Not a schedule")]
    case False{} False{}:
      [D.Dom.span("hint", d)]

# what the schedule being typed means
def WB.cron.hint(+s: String) -> List<&2, D.Node>:
  WB.cron.hint.k(String.is_empty(String.trim(s)), String.is_empty(CR.Cron.describe(s)), CR.Cron.describe(s))

def WB.rform(+u: U.Ui) -> D.Node:
  WB.div("bform wide", D.Dom.cat([
    WB.input("routine-name", "oname", BI.BU.field(u, "oname"), "Name", ""),
    WB.input("routine-cron", "ocron", BI.BU.field(u, "ocron"), "every 15m · daily 09:30 · weekdays 08:00 · cron", "")],
    D.Dom.cat(WB.cron.hint(BI.BU.field(u, "ocron")), [
    WB.area("routine-prompt", "oprompt", BI.BU.field(u, "oprompt"), "What to do each time"),
    WB.btns([D.Dom.button("primary", "routine-save", "", "Save the routine", Bool.pick(String, String.is_empty(BI.BU.field(u, "oid")), "Add routine", "Save routine")),
      D.Dom.button("", "routine-edit", "", "Clear the form", "Clear")])])))

def WB.routines(+u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.div("bot-list", List.append(&2, D.Node, WB.routines.go(BI.BU.routines(u, BT.Bot.id(b)), BI.BU.owns(BT.Bot.id(b)), U.Side.now(u)), D.Dom.when(BI.BU.owns(BT.Bot.id(b)), WB.rform(u))))

# Webhooks
def WB.page(+u: U.Ui) -> String:
  U.Ui.scratch(u, "@page")

def WB.calls(+n: Nat, +last: String) -> String:
  Nat.show(n) ++ Bool.pick(String, Nat.is_eq(n, 1n), " call", " calls") ++ Bool.pick(String, String.is_empty(last), "", " · " ++ last)

def WB.hook(h: BT.Hook, +now: Nat, +origin: String) -> D.Node:
  match h:
    case BT.Hook{+id, _, name, _, _, +last, +count}:
      D.El{"div", "hk:" ++ id, [D.At{"class", "item"}], [
        WB.div("item-main", [D.Dom.span("item-key", name), D.Dom.span("item-url", origin ++ "/hook/" ++ id)]),
        D.Dom.span("ago", WB.calls(count, WB.ago(now, last))),
        WB.div("item-btns", [D.Dom.copy(origin ++ "/hook/" ++ id), D.Dom.button("", "hook-token", id, "A bearer token, for senders that cannot sign (a Pebble Index ring)", "Token"),
          D.Dom.button("", "hook-revoke", id, "Revoke this webhook", "Revoke")])]}

def WB.hooks.go(hs: List<&2, BT.Hook>, +now: Nat, +origin: String) -> List<&2, D.Node>:
  match hs:
    case Nil{}:
      Nil{}
    case h <> t:
      WB.hook(h, now, origin) <> WB.hooks.go(t, now, origin)

# how to call a webhook: the key is the secret's 32 bytes (hexkey), as
# core/sign.bend's Sign.header keys it
def WB.curl(+url: String, +secret: String) -> String:
  "ts=$(date +%s); body='{\"text\":\"hello\"}'\n"
    ++ "sig=$(printf '%s.%s' \"$ts\" \"$body\" | openssl dgst -sha256 -mac HMAC -macopt hexkey:" ++ secret ++ " | awk '{print $NF}')\n"
    ++ "curl -X POST " ++ url ++ " -H \"X-Backplane-Timestamp: $ts\" -H \"X-Backplane-Signature: sha256=$sig\" -d \"$body\""

# a new webhook's secret, shown once (only this page has it)
def WB.secret.k(show: Bool, +url: String, +secret: String) -> List<&2, D.Node>:
  match show:
    case True{}:
      [WB.div("secret", [
        D.Dom.span("secret-label", "Secret, shown once"),
        WB.div("secret-line", [D.El{"code", "", Nil{}, [D.Txt{secret}]}, D.Dom.copy(secret)]),
        WB.div("codeblock", [D.Dom.copy(WB.curl(url, secret)), D.El{"pre", "", Nil{}, [D.El{"code", "", Nil{}, [D.Txt{WB.curl(url, secret)}]}]}]),
        D.Dom.span("secret-label", "GitHub: payload URL " ++ url ++ ", content type application/json, this secret."),
        WB.btns([D.Dom.button("", "hook-done", "", "Hide the secret: it is not shown again", "Done")])])]
    case False{}:
      Nil{}

def WB.line(+s: String) -> D.Node:
  WB.div("secret-line", [D.El{"code", "", Nil{}, [D.Txt{s}]}, D.Dom.copy(s)])

def WB.token.curl(+url: String, +tok: String) -> String:
  "curl -X POST " ++ url ++ " -H \"Authorization: Bearer " ++ tok ++ "\" -F transcription=hello -F recordedAt=$(date +%s000)"

# a phone wants HTTPS: say how to get it when the hub has none
def WB.token.warn(+url: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.starts_with(url, "http://"),
    [D.Dom.span("secret-label", "Plain HTTP: the ring's app wants HTTPS. Give the hub an HTTPS port on the tailnet (tailscale serve --bg --https=<free port> http://127.0.0.1:<the hub's port>) and open this again.")],
    Nil{})

# a webhook's bearer token and where to post, shown until hidden
def WB.token.k(show: Bool, +url: String, +tok: String, +hid: String) -> List<&2, D.Node>:
  match show:
    case True{}:
      [WB.div("secret", List.concat(&2, D.Node, [
        [D.Dom.span("secret-label", "URL to post to"), WB.line(url),
         D.Dom.span("secret-label", "Header"), WB.line("Authorization: Bearer " ++ tok),
         D.Dom.span("secret-label", "Token"), WB.line(tok)],
        WB.token.warn(url),
        [WB.div("codeblock", [D.Dom.copy(WB.token.curl(url, tok)), D.El{"pre", "", Nil{}, [D.El{"code", "", Nil{}, [D.Txt{WB.token.curl(url, tok)}]}]}]),
         D.Dom.span("secret-label", "Pebble app: Index 01 settings, Webhook: this URL, a header Authorization with the value Bearer and the token, send Transcription. The bot reads what arrives as untrusted data."),
         WB.btns([D.Dom.button("", "hook-retoken", hid, "Make a new token: the old one stops working", "New token"),
           D.Dom.button("", "hook-untoken", hid, "Turn bearer tokens off for this webhook", "Turn off"),
           D.Dom.button("", "hook-done", "", "Hide the token", "Done")])]]))]
    case False{}:
      Nil{}

def WB.hooks(+u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.div("bot-list", List.concat(&2, D.Node, [
    WB.token.k(Bool.and(Bool.not(String.is_empty(U.Ui.scratch(u, "@hook.token"))),
        String.eq(BT.Hook.bot(BT.World.hook(M.State.bots(U.Ui.state(u)), U.Ui.scratch(u, "@hook.tokfor"))), BT.Bot.id(b))),
      U.Ui.scratch(u, "@hook.tokurl"), U.Ui.scratch(u, "@hook.token"), U.Ui.scratch(u, "@hook.tokfor")),
    WB.secret.k(Bool.and(Bool.not(String.is_empty(U.Ui.scratch(u, "@hook.secret"))),
        String.eq(BT.Hook.bot(BT.World.hook(M.State.bots(U.Ui.state(u)), U.Ui.scratch(u, "@hook.id"))), BT.Bot.id(b))),
      WB.page(u) ++ "/hook/" ++ U.Ui.scratch(u, "@hook.id"), U.Ui.scratch(u, "@hook.secret")),
    WB.hooks.go(BI.BU.hooks(u, BT.Bot.id(b)), U.Side.now(u), WB.page(u)),
    [WB.div("bform wide row", [WB.input("hook-name", "hname", BI.BU.field(u, "hname"), "Name", "hook-new"),
      D.Dom.button("primary", "hook-new", "", "Make a webhook", "New webhook")])]]))

# Settings
def WB.looks(n: Nat, +i: Nat, +cur: Nat) -> List<&2, D.Node>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      D.El{"button", "", [D.At{"class", Bool.pick(String, Nat.is_eq(i, cur), "look-pick on", "look-pick")}, D.At{"title", "Look " ++ Nat.show(i)}, D.At{"data-value", Nat.show(i)}, D.On{"click", "bot-look"}],
        [WC.WCat.img(i, "idle", 40)]} <> WB.looks(k, 1n+i, cur)

def WB.set(+label: String, kids: List<&2, D.Node>) -> D.Node:
  WB.div("bset", D.Dom.span("set-label", label) <> kids)

def WB.persona(+u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.set("Persona", [
    WB.area("bot-edit-persona", "persona", Bool.pick(String, String.is_empty(BI.BU.field(u, "persona")), BT.Bot.persona(b), BI.BU.field(u, "persona")), "Who it is, what it does"),
    WB.btns(Bool.pick(List<&2, D.Node>, String.is_empty(BI.BU.field(u, "persona")), Nil{},
      [D.Dom.button("primary", "bot-edit", "", "Save the persona", "Save"), D.Dom.button("", "bfield", WB.bf("persona", ""), "Put it back", "Revert")]))])

def WB.gurl(+url: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(url), Nil{}, [
    WB.div("bform-btns", [D.El{"a", "", [D.At{"class", "link"}, D.At{"href", url}, D.At{"target", "_blank"}, D.At{"rel", "noopener noreferrer"}], [D.Txt{"Open Google sign-in"}]}])])

# the user's own OAuth client, shown on asking
def WB.gown.k(open: Bool, +u: U.Ui) -> List<&2, D.Node>:
  match open:
    case True{}:
      [WB.input("google-id", "gid", BI.BU.field(u, "gid"), "OAuth client id (Desktop app)", ""),
       WB.secret("google-secret", "gsecret", BI.BU.field(u, "gsecret"), "Client secret"),
       WB.btns([D.Dom.button("", "google", "config", "Save the client id and secret", "Save client"),
         D.Dom.button("", "bfield", WB.bf("gown", ""), "Hide the client fields", "Hide")])]
    case False{}:
      Nil{}

# one signed-in account and its own sign-out
def WB.gaccts(xs: List<&2, String>) -> List<&2, D.Node>:
  match xs:
    case Nil{}:
      Nil{}
    case +e <> t:
      D.El{"div", "gacct:" ++ e, [D.At{"class", "item"}], [WB.div("item-main", [D.Dom.span("item-key", e)]),
        WB.div("item-btns", [D.Dom.button("", "google", "signout:" ++ e, "Sign out " ++ e, "Sign out")])]} <> WB.gaccts(t)

# Google: connect (sign in in a new tab; again for another account; paste
# the page it lands on when that tab is on another device), status, and
# every account signed in with its own sign-out
def WB.google(+u: U.Ui) -> D.Node:
  WB.set("Google", List.concat(&2, D.Node, [
    Bool.pick(List<&2, D.Node>, String.is_empty(U.Ui.scratch(u, "@google")), Nil{}, [D.Dom.span("set-note", U.Ui.scratch(u, "@google"))]),
    [WB.btns([D.Dom.button("primary", "google", "connect", "Sign in with Google", Bool.pick(String, List.is_empty(&2, String, U.Bots.google.accts(u)), "Connect Google", "Add account")),
       D.Dom.button("", "google", "status", "Check the connection", "Status"),
       D.Dom.button("", "bfield", WB.bf("gown", "1"), "Use an OAuth client of your own", "Own client")])],
    WB.gaccts(U.Bots.google.accts(u)),
    WB.gown.k(Bool.not(String.is_empty(BI.BU.field(u, "gown"))), u),
    WB.gurl(U.Ui.scratch(u, "@google.url")),
    Bool.pick(List<&2, D.Node>, String.is_empty(U.Ui.scratch(u, "@google.url")), Nil{}, [
      WB.div("bform wide row", [WB.input("google-paste", "gpaste", BI.BU.field(u, "gpaste"), "Or paste the address the sign-in ended on", "google"),
        D.Dom.button("", "google", "finish", "Finish signing in", "Finish")])])]))

def WB.peer(p: BT.Peer) -> D.Node:
  match p:
    case BT.Peer{+id, name, url, _, _}:
      D.El{"div", "peer:" ++ id, [D.At{"class", "item"}], [WB.div("item-main", [D.Dom.span("item-key", name), D.Dom.span("item-url", url)]),
        WB.div("item-btns", [D.Dom.button("", "peer-revoke", id, "Unlink this machine", "Unlink")])]}

def WB.peers.go(ps: List<&2, BT.Peer>) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      WB.peer(p) <> WB.peers.go(t)

def WB.invite(+s: String) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, String.is_empty(s), Nil{}, [WB.div("secret-line", [D.El{"code", "", Nil{}, [D.Txt{s}]}, D.Dom.copy(s)])])

# linked machines: make an invite, join with one, the links
def WB.machines(+u: U.Ui) -> D.Node:
  WB.set("Linked machines", List.concat(&2, D.Node, [
    WB.peers.go(BI.BU.peers(u)),
    [WB.div("bform wide row", [WB.input("peer-url", "purl", BI.BU.field(u, "purl"), "This hub's URL, as the other machine reaches it", "peer-invite"),
      D.Dom.button("", "peer-invite", "", "Make an invite to paste on the other machine", "Invite")])],
    WB.invite(U.Ui.scratch(u, "@peer.invite")),
    [WB.div("bform wide row", [WB.input("peer-join", "invite", BI.BU.field(u, "invite"), "Paste an invite (bp1:…)", "peer-join"),
      D.Dom.button("", "peer-join", "", "Link to the machine that made the invite", "Join")])]]))

def WB.del.k(armed: Bool, +id: String, +name: String) -> List<&2, D.Node>:
  match armed:
    case True{}:
      [D.Dom.button("danger", "bot-delete", id, "Delete this bot for good", "Delete " ++ name), D.Dom.button("", "bfield", WB.bf("delbot", ""), "Keep it", "Cancel")]
    case False{}:
      [D.Dom.button("", "bfield", WB.bf("delbot", id), "Delete this bot", "Delete bot")]

def WB.settings(+u: U.Ui, +b: BT.Bot) -> D.Node:
  WB.div("bot-list", [
    WB.persona(u, b),
    WB.set("Look", [WB.div("looks", WB.looks(WC.WCat.looks(), 0n, Nat.mod(BT.Bot.look(b), WC.WCat.looks())))]),
    WB.google(u),
    WB.machines(u),
    WB.set("", [WB.btns(WB.del.k(String.eq(BI.BU.field(u, "delbot"), BT.Bot.id(b)), BT.Bot.id(b), BT.Bot.name(b)))])])

def WB.pos(xs: List<&2, String>, +s: String, +i: Nat) -> Nat:
  match xs:
    case Nil{}:
      i
    case +x <> t:
      Bool.pick(Nat, String.eq(x, s), i, WB.pos(t, s, 1n+i))

def WB.body.k(i: Nat, +u: U.Ui, +b: BT.Bot) -> D.Node:
  match i:
    case 0n:
      WS.WSpace.view(BI.BU.space(u, b), BT.Bot.id(b))
    case 1n:
      WB.browser(u, b)
    case 2n:
      WB.memory(u, b)
    case 3n:
      WB.routines(u, b)
    case 4n:
      WB.hooks(u, b)
    case _:
      WB.settings(u, b)

# a tab other than Chat
def WB.body(+u: U.Ui, +b: BT.Bot, +tab: String) -> D.Node:
  D.El{"div", "bot-body:" ++ tab, [D.At{"class", "bot-body"}], [WB.body.k(WB.pos(["space", "browser", "memory", "routines", "hooks"], tab, 0n), u, b)]}

# The uberbot (docs/uber.md)
# --------------------------

# its sidebar row, pinned above the search: its cat, its name and what
# waits on the person (Plane.counts); a click does action "uber"
def WB.uber.k(r: BI.BRow, +counts: String) -> D.Node:
  match r:
    case BI.BRow{+id, +name, _, +look, +mood, _, sel, _, _, _}:
      D.El{"div", "uber:" ++ id, [D.At{"class", Bool.pick(String, sel, "bot-row uber-row on", "bot-row uber-row")}, D.At{"title", id}, D.At{"data-value", id}, D.On{"click", "uber"}],
        [WC.WCat.img(look, mood, 28), WB.div("bot-text", [D.Dom.span("bot-name", name), D.Dom.span("uber-counts", Bool.pick(String, String.is_empty(counts), BI.Mood.note(mood), counts))])]}

def WB.uber.m(m: Maybe<&2, BI.BRow>, +u: U.Ui) -> List<&2, D.Node>:
  match m:
    case Some{r}:
      [WB.uber.k(r, PL.Plane.tally.words(u, U.Side.now(u)))]
    case None{}:
      Nil{}

# no uberbot, no row
def WB.uber(+u: U.Ui) -> List<&2, D.Node>:
  WB.uber.m(BI.BU.uber(u), u)

# the dot of a row: thread states and bot moods alike
def WB.pdot(+s: String) -> String:
  Bool.pick(String, Bool.or(String.eq(s, "approval"), String.eq(s, "wait")), "dot approval",
    Bool.pick(String, String.eq(s, "question"), "dot input",
      Bool.pick(String, Bool.or(String.eq(s, "failed"), String.eq(s, "error")), "dot failed",
        Bool.pick(String, Bool.or(String.eq(s, "working"), Bool.or(String.eq(s, "work"), String.eq(s, "talk"))), "dot working",
          Bool.pick(String, Bool.or(String.eq(s, "queued"), String.eq(s, "think")), "dot queued",
            Bool.pick(String, String.eq(s, "unread"), "dot waiting", "dot"))))))

def WB.pago(+kind: String, +now: Nat, +at: Nat) -> String:
  Bool.pick(String, Bool.or(String.eq(kind, "routine"), Bool.or(Nat.is_eq(at, 0n), Nat.is_gt(at, now))), "", U.Time.ago(now, at))

# one row: a button for what a click sends (act, val), a fixed height
def WB.prow(r: PL.PRow, +now: Nat) -> D.Node:
  match r:
    case PL.PRow{+kind, +id, +act, +val, +title, +sub, +machine, +state, +at, away}:
      D.El{"button", kind ++ ":" ++ id, [D.At{"class", Bool.pick(String, away, "plane-row away", "plane-row")}, D.At{"title", title}, D.At{"data-value", val}, D.On{"click", act}],
        [D.Dom.span(WB.pdot(state), ""), D.Dom.span("title", title), D.Dom.span("sub", sub), D.Dom.span("mach", machine), D.Dom.span("ago", WB.pago(kind, now, at))]}

def WB.prows(rs: List<&2, PL.PRow>, +now: Nat) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      WB.prow(r, now) <> WB.prows(t, now)

# a labelled section; none of it when it is empty
def WB.psec(+label: String, rs: List<&2, PL.PRow>, +more: Nat, +now: Nat) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case _ <> _:
      [WB.div("plane-sec", D.Dom.span("plane-label", label) <> List.append(&2, D.Node, WB.prows(rs, now), D.Dom.when(Nat.is_gt(more, 0n), D.Dom.span("plane-more", "+" ++ Nat.show(more) ++ " more"))))]

def WB.pcount(+label: String, +n: Nat) -> D.Node:
  WB.div(Bool.pick(String, Nat.is_eq(n, 0n), "plane-count", "plane-count on"), [D.Dom.span("n", Nat.show(n)), D.Dom.span("l", label)])

def WB.pmach(m: PL.PMach) -> D.Node:
  match m:
    case PL.PMach{+name, +state}:
      WB.div("plane-mach " ++ state, [D.Dom.span(Bool.pick(String, String.eq(state, "here"), "dot working", Bool.pick(String, String.eq(state, "syncing"), "dot queued", "dot")), ""), D.Dom.span("", name)])

def WB.pmachs(ms: List<&2, PL.PMach>) -> List<&2, D.Node>:
  match ms:
    case Nil{}:
      Nil{}
    case m <> t:
      WB.pmach(m) <> WB.pmachs(t)

# the strip: the counters and the machines
# where the conversation is held, and the offer to make this machine the
# home (core's Plane.home decides)
def WB.phome(+h: PL.PHome) -> List<&2, D.Node>:
  Bool.pick(List<&2, D.Node>, PL.PHome.on(h),
    [WB.div("plane-home", List.concat(&2, D.Node, [[D.Dom.span("", PL.Plane.home.line(h))],
      Bool.pick(List<&2, D.Node>, PL.Plane.home.button(h), [D.Dom.button("plane-home-btn", "uber-home", "", PL.Plane.home.label(), PL.Plane.home.label())], Nil{})]))],
    Nil{})

def WB.pstrip(+p: PL.Plane, +h: PL.PHome) -> D.Node:
  WB.div("plane-strip", List.concat(&2, D.Node, [[
    WB.pcount("Need you", List.length(&2, PL.PRow, PL.Plane.need(p))),
    WB.pcount("Running", List.length(&2, PL.PRow, PL.Plane.run(p))),
    WB.pcount("Bots working", PL.Plane.working(p)),
    WB.pcount("Unread rooms", List.length(&2, PL.PRow, PL.Plane.rooms(p))),
    WB.div("plane-machs", WB.pmachs(PL.Plane.machines(p)))], WB.phome(h)]))

def WB.pidle.more(+shown: Nat, +n: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_gt(n, shown), Nat.sub(n, shown), 0n)

def WB.pempty(+p: PL.Plane) -> Bool:
  Bool.and(List.is_empty(&2, PL.PRow, PL.Plane.need(p)), Bool.and(List.is_empty(&2, PL.PRow, PL.Plane.run(p)), List.is_empty(&2, PL.PRow, PL.Plane.idle(p))))

def WB.plane.k(+p: PL.Plane, +h: PL.PHome, +now: Nat) -> D.Node:
  WB.div("plane", [
    WB.pstrip(p, h),
    WB.div("plane-grid", [
      WB.div("plane-main", List.concat(&2, D.Node, [
        WB.psec("Needs you", PL.Plane.need(p), 0n, now),
        WB.psec("Running", PL.Plane.run(p), 0n, now),
        WB.psec("Idle", PL.Plane.idle(p), WB.pidle.more(List.length(&2, PL.PRow, PL.Plane.idle(p)), PL.Plane.idle_n(p)), now),
        D.Dom.when(WB.pempty(p), WB.note("Nothing active"))])),
      WB.div("plane-side", List.concat(&2, D.Node, [
        WB.psec("Bots", PL.Plane.bots(p), 0n, now),
        WB.psec("Rooms", PL.Plane.rooms(p), 0n, now),
        WB.psec("Board · " ++ Nat.show(PL.Plane.board_n(p)) ++ " open", PL.Plane.board(p), Nat.sub(PL.Plane.board_n(p), List.length(&2, PL.PRow, PL.Plane.board(p))), now),
        WB.psec("Routines", PL.Plane.routines(p), 0n, now)]))])])

# the Status tab's body
def WB.plane(+u: U.Ui) -> D.Node:
  WB.plane.k(PL.Plane.of(u, U.Side.now(u)), PL.Plane.home(u), U.Side.now(u))

# a tab's body: Status here, the others in WB.body; only the one shown is built
def WB.main.k(status: Bool, +u: U.Ui, +b: BT.Bot, +tab: String) -> D.Node:
  match status:
    case True{}:
      D.El{"div", "bot-body:status", [D.At{"class", "bot-body"}], [WB.plane(u)]}
    case False{}:
      WB.body(u, b, tab)

def WB.main(+u: U.Ui, +b: BT.Bot, +tab: String) -> D.Node:
  WB.main.k(String.eq(tab, "status"), u, b, tab)

# Rooms and bots elsewhere
# ------------------------

# a composer for a room or a bot elsewhere
def WB.compose(+id: String, +field: String, +value: String, +ph: String, +act: String) -> D.Node:
  WB.div("composer", [
    D.El{"textarea", id, [D.At{"id", id}, D.At{"rows", "2"}, D.At{"placeholder", ph}, D.At{"value", value}, D.At{"data-bfield", field}, D.On{"input", "bfield"}, D.At{"data-enter", act}], Nil{}},
    WB.div("composer-foot", [D.Dom.span("", ""), D.Dom.button("primary", act, "", "Enter", "Send")])])

def WB.post.bot(m: Maybe<&2, BT.Bot>, +from: String, text: String, +at: String, +st: M.State, +now: Nat, +fl: Nat, +fm: String) -> List<&2, D.Node>:
  match m:
    case Some{+b}:
      [WC.WCat.img(BT.Bot.look(b), BI.Mood.bot(st, now, b), 28),
       WB.div("post-main", [WB.div("post-from", [D.Dom.span("", BT.Bot.name(b)), D.Dom.span("ago", at)]), WB.div("body", D.Md.render(text))])]
    case None{}:
      [WC.WCat.img(fl, fm, 28),
       WB.div("post-main", [WB.div("post-from", [D.Dom.span("", from), D.Dom.span("ago", at)]), WB.div("body", D.Md.render(text))])]

def WB.post.k(you: Bool, +id: String, +from: String, text: String, +at: String, +u: U.Ui, +fl: Nat, +fm: String) -> D.Node:
  match you:
    case True{}:
      D.El{"div", id, [D.At{"class", "msg user"}], [WB.div("body plain", D.Dom.linked(text))]}
    case False{}:
      D.El{"div", id, [D.At{"class", "post"}], WB.post.bot(BT.World.find.id(BT.World.bot(M.State.bots(U.Ui.state(u)), from)), from, text, at, U.Ui.state(u), U.Side.now(u), fl, fm)}

# a post; one from elsewhere shows the cat fl/fm (a bot elsewhere's own)
def WB.post(p: BT.Post, +u: U.Ui, +fl: Nat, +fm: String) -> D.Node:
  match p:
    case BT.Post{+id, _, +from, text, _, +at}:
      WB.post.k(String.eq(from, "you"), id, from, text, U.Time.ago(U.Side.now(u), at), u, fl, fm)

def WB.posts(ps: List<&2, BT.Post>, +u: U.Ui, +fl: Nat, +fm: String) -> List<&2, D.Node>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      WB.post(p, u, fl, fm) <> WB.posts(t, u, fl, fm)

def WB.timeline(ns: List<&2, D.Node>) -> D.Node:
  D.El{"div", "timeline", [D.At{"class", "timeline"}, D.At{"id", "timeline"}], ns}

def WB.room.del(armed: Bool, +id: String) -> List<&2, D.Node>:
  match armed:
    case True{}:
      [D.Dom.button("danger", "room-delete", id, "Delete this room for good", "Delete room"), D.Dom.button("", "bfield", WB.bf("delroom", ""), "Keep it", "Cancel")]
    case False{}:
      [D.Dom.button("", "bfield", WB.bf("delroom", id), "Delete this room", "Delete")]

# a room: its members, its posts, and a line to write to it
def WB.room(+u: U.Ui, r: BT.Room) -> D.Node:
  match r:
    case BT.Room{+id, +name, ms, c, d}:
      D.El{"main", "room", [D.At{"class", "thread-view room-view"}], [
        D.El{"header", "", [D.At{"class", "thread-head"}], [
          D.Dom.button("back", "select", "", "Back", "‹"),
          D.Dom.span("title", "# " ++ name),
          D.Dom.span("meta", BI.BU.members(u, BT.Room{id, name, ms, c, d})),
          D.El{"div", "", [D.At{"class", "tools"}], WB.room.del(String.eq(BI.BU.field(u, "delroom"), id), id)}]},
        WB.timeline(Bool.pick(List<&2, D.Node>, List.is_empty(&2, BT.Post, BI.BU.posts(u, id)), [WB.note("No posts yet.")], WB.posts(BI.BU.posts(u, id), u, 0n, "away"))),
        WB.compose("room-post", "rpost", BI.BU.field(u, "rpost"), "Write to the room", "room-post")]}

# a bot only reported by a linked machine: the bot's view (BU.head: its
# cat, "name · machine", its mood, Chat alone) over the person's
# conversation with it, and a line to it
def WB.remote.h(+u: U.Ui, +id: String, +hd: BI.Head) -> D.Node:
  D.El{"main", "remote", [D.At{"class", "thread-view bot-view room-view"}], [
    WB.header(hd, Nil{}, Nil{}),
    WB.tabs(hd),
    WB.timeline(Bool.pick(List<&2, D.Node>, List.is_empty(&2, BT.Post, BI.BU.remote.posts(u, id)), [WB.note("Nothing said yet.")], WB.posts(BI.BU.remote.posts(u, id), u, BI.Head.look(hd), BI.Head.mood(hd)))),
    WB.compose("remote-send", "rtext", BI.BU.field(u, "rtext"), "Write to " ++ id, "remote-send")]}

def WB.remote(+u: U.Ui, +id: String) -> D.Node:
  WB.remote.h(u, id, BI.BU.head(u, BI.VRemote{id}))
