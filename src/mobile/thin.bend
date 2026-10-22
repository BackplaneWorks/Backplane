import Base
import ../core/json.bend as J
import ../core/client.bend as U
import ../core/presentation.bend as P
import ./hubs.bend as H
import ./session.bend as S
import ../core/i18n.bend as I
import ../core/msgs.bend as MSG

# These adapters merge server-owned display JSON, never canonical state or
# an outbox. Normal Hubs rendering still decides the paired-hub screen.
def Obj.keep( +drop: Bool, +key: String, +value: J.Json, +tail: J.Json) -> J.Json:
  match drop:
    case True{}: tail
    case False{}: J.Field{key, value, tail}

def Obj.drop( +fs: J.Json, +key: String) -> J.Json:
  match fs:
    case J.Field{+k, v, tail}: Obj.keep(String.eq(k, key), k, v, Obj.drop(tail, key))
    case _: J.End{}

def Obj.remove( +j: J.Json, +key: String) -> J.Json:
  match j:
    case J.Obj{fs}: J.Obj{Obj.drop(fs, key)}
    case _: j

def Obj.put( +j: J.Json, +key: String, +value: J.Json) -> J.Json:
  match j:
    case J.Obj{fs}: J.Obj{J.Field{key, value, Obj.drop(fs, key)}}
    case _: J.Json.obj([J.KV{key, value}])

def Target.json( +kind: String, +id: String) -> J.Json:
  J.Json.obj([J.KV{"kind", J.Str{kind}}, J.KV{"id", J.Str{id}}])

def Target.thread( +empty: Bool, +session: J.Json) -> J.Json:
  match empty:
    case True{}: Target.json("recent", "")
    case False{}: Target.json("thread", J.Json.get_str(session, "sel", ""))

def Target.route( +bot: Bool, +session: J.Json) -> J.Json:
  match bot:
    case True{}: Target.json("bot", J.Json.get_str(J.Json.get_obj(session, "route"), "bot", ""))
    case False{}: Target.thread(String.is_empty(J.Json.get_str(session, "sel", "")), session)

def Target.of(+session: J.Json) -> J.Json:
  Target.route(Bool.and(String.eq(J.Json.get_str(J.Json.get_obj(session, "route"), "kind", ""), "bot-tab"),
    Bool.not(String.is_empty(J.Json.get_str(J.Json.get_obj(session, "route"), "bot", "")))), session)

# Selection/draft are the only choices made without a ready server view.
# Bot, room, tab and policy actions go through the normal server actor.
def Session.route_if( +present: Maybe<&2, J.Json>, +result: J.Json) -> J.Json:
  match present:
    case Some{route}: Obj.put(result, "route", route)
    case None{}: result

def Session.keep_route(+before: J.Json, +result: J.Json) -> J.Json:
  Session.route_if(J.Json.get(before, "route"), result)

def Session.draft(+session: J.Json, +text: String) -> J.Json:
  Session.keep_route(session, P.Session.draft(session, text))

def Session.prepare(+session: J.Json, +input: J.Json, +action: String) -> J.Json:
  Session.keep_route(session, P.Session.prepare(session, input, action))

def Session.reply(+session: J.Json, +o: J.Json) -> J.Json:
  Session.keep_route(session, P.Session.reply(session, o))

def Session.local(+session: J.Json, +key: String, +value: String) -> J.Json:
  Session.keep_route(session, S.Session.local(session, key, value))

def Session.reading(+session: J.Json, +id: String, +seen: String, +count: Nat, +away: Bool) -> J.Json:
  Session.keep_route(session, P.Session.reading(session, id, seen, count, away))

def Choice(+session: J.Json, +action: String, +value: String) -> J.Json:
  match action:
    case "draft": Session.keep_route(session, P.Session.draft(session, value))
    case "select": Obj.remove(Obj.put(session, "sel", J.Str{H.Hubs.rest(value)}), "route")
    case _: session

def Partial.found( +hit: Bool, +row: J.Json, +rest: J.Json) -> J.Json:
  match hit:
    case True{}: row
    case False{}: rest

def Partial.find( +rows: List<&2, J.Json>, +id: String) -> J.Json:
  match rows:
    case Nil{}: J.Null{}
    case +row <> tail: Partial.found(String.eq(J.Json.get_str(row, "id", ""), id), row, Partial.find(tail, id))

def Partial.entry_k( +msg: Bool, +part: J.Json, +entry: J.Json) -> J.Json:
  match msg:
    case True{}: entry
    case False{}: Obj.put(entry, "partial", part)

# a message's text is always whole: only activity rows carry a partial
# footer, whatever the window advertises (no "read rest of message")
def Partial.message( +kind: String) -> Bool:
  Bool.or(String.eq(kind, "user"), String.eq(kind, "assistant"))

def Partial.entry_if( +part: J.Json, +entry: J.Json) -> J.Json:
  match part:
    case J.Null{}: entry
    case +p: Partial.entry_k(Partial.message(J.Json.get_str(entry, "kind", "")), p, entry)

def Partial.entries( +entries: List<&2, J.Json>, +parts: List<&2, J.Json>) -> List<&2, J.Json>:
  match entries:
    case Nil{}: Nil{}
    case +entry <> tail: Partial.entry_if(Partial.find(parts, J.Json.get_str(entry, "id", "")), entry) <> Partial.entries(tail, parts)

def Partial.thread( +th: J.Json, +parts: List<&2, J.Json>) -> J.Json:
  match th:
    case J.Null{}: J.Null{}
    case +t: Obj.put(t, "entries", J.Json.arr(Partial.entries(J.Json.elems(J.Json.get_obj(t, "entries")), parts)))

def Partial.screen(+shown: J.Json, +window: J.Json) -> J.Json:
  Obj.put(shown, "thread", Partial.thread(J.Json.get_obj(shown, "thread"), J.Json.elems(J.Json.get_obj(window, "partial"))))

# Unlike the provisional full-replay presentation path, these are already
# authenticated current views. Do not turn their Mailroom back into loading.
def Overlay.one( +m: Maybe<&2, H.Shown>, +hs: H.Hubs, +key: String, +u: U.Ui, +cur: String) -> List<&2, H.Shown>:
  match m:
    case Some{s}: [s]
    case None{}: H.Hubs.shown(H.Hubs.loc(hs), [H.Hub{key, u}], cur)

def Overlay.go( +list: List<&2, H.Hub>, +hs: H.Hubs, +js: List<&2, J.Json>, +cur: String) -> List<&2, H.Shown>:
  match list:
    case Nil{}: Nil{}
    case H.Hub{+key, u} <> tail:
      List.append(&2, H.Shown, Overlay.one(S.Session.overlay.find(js, key), hs, key, u, cur), Overlay.go(tail, hs, js, cur))

def Overlay.screen(+hs: H.Hubs, +js: J.Json, +cur: String) -> J.Json:
  H.Hubs.screen.k(H.Hubs.loc(hs), H.Hubs.relabel(Overlay.go(H.Hubs.list(hs), hs, J.Json.elems(js), cur)), cur, H.Hubs.list(hs), H.Hubs.pl.want(H.Hubs.list(hs), cur))

# Existing native Find schema, populated from the bounded server search.
# Selecting a row is an explicit normal thread choice, not an eager replay.
def Search.rows( +rows: List<&2, J.Json>, +hub: String) -> List<&2, J.Json>:
  match rows:
    case Nil{}: Nil{}
    case +row <> tail:
      J.Json.obj([J.KV{"label", J.Str{J.Json.get_str(row, "text", "")}}, J.KV{"action", J.Str{"select"}},
        J.KV{"value", J.Str{hub ++ "|" ++ J.Json.get_str(row, "thread", "")}}, J.KV{"kind", J.Str{"thread"}}]) <> Search.rows(tail, hub)

def Search.more( +more: Bool, +search: J.Json, +lc: I.Loc) -> List<&2, J.Json>:
  match more:
    case False{}: Nil{}
    case True{}: [J.Json.obj([J.KV{"label", J.Str{MSG.T.common.more(lc)}}, J.KV{"action", J.Str{"thin-search-more"}},
      J.KV{"value", J.Str{J.Json.show(J.Json.obj([J.KV{"query", J.Str{J.Json.get_str(search, "query", "")}}, J.KV{"cursor", J.Str{J.Json.get_str(search, "cursor", "")}}]))}}, J.KV{"kind", J.Str{"action"}}])]

def Search.screen(+screen: J.Json, +search: J.Json) -> J.Json:
  Obj.put(screen, "find", J.Json.obj([J.KV{"mode", J.Str{"search"}}, J.KV{"query", J.Str{J.Json.get_str(search, "query", "")}},
    J.KV{"rows", J.Json.arr(List.append(&2, J.Json, Search.rows(J.Json.elems(J.Json.get_obj(search, "rows")), J.Json.get_str(screen, "hub", "")), Search.more(J.Json.get_bool(search, "hasMore", False{}), search, I.I18n.of_tag(J.Json.get_str(screen, "locale", "en")))))}]))
