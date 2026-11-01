import Base
import ../core/json.bend as J
import ../core/client.bend as U
import ../core/model.bend as M
import ../core/i18n.bend as I
import ./hubs.bend as H
import ./screen.bend as SC
import ../core/presentation.bend as P

# A presentation session contains only the person's transient choices. It
# cannot supply a read model, a log origin, or a canonical replay cursor.
# The server renders against its already-folded state; the phone continues
# replaying the real ordered log independently in the background.

# Compatibility exports keep the existing phone wire/API stable.
def Session.strings(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, J.KV>:
  P.Session.strings(xs)

def Session.flags(xs: List<&2, Sigma<&2, &2, String, _ => Bool>>) -> List<&2, J.KV>:
  P.Session.flags(xs)

def Session.read.strings(j: J.Json, acc: Map<&2, String>) -> Map<&2, String>:
  P.Session.read.strings(j, acc)

def Session.read.flags(j: J.Json, acc: Map<&2, Bool>) -> Map<&2, Bool>:
  P.Session.read.flags(j, acc)

def Session.info.key(+k: String) -> Bool:
  P.Session.info.key(k)

def Session.info.put(keep: Bool, acc: Map<&2, String>, k: String, v: String) -> Map<&2, String>:
  P.Session.info.put(keep, acc, k, v)

def Session.info.read(j: J.Json, acc: Map<&2, String>) -> Map<&2, String>:
  P.Session.info.read(j, acc)

def Session.info.fields.put(keep: Bool, k: String, v: String, rest: List<&2, J.KV>) -> List<&2, J.KV>:
  P.Session.info.fields.put(keep, k, v, rest)

def Session.info.fields(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, J.KV>:
  P.Session.info.fields(xs)

def Session.mail.key(+key: String) -> Bool:
  P.Session.mail.key(key)

def Session.mail.remote.put(keep: Bool, acc: Map<&2, String>, key: String, value: String) -> Map<&2, String>:
  P.Session.mail.remote.put(keep, acc, key, value)

def Session.mail.remote(j: J.Json, acc: Map<&2, String>) -> Map<&2, String>:
  P.Session.mail.remote(j, acc)

def Session.cache.put(keep: Bool, acc: Map<&2, String>, k: String, v: String) -> Map<&2, String>:
  P.Session.cache.put(keep, acc, k, v)

def Session.cache.go(xs: List<&2, Sigma<&2, &2, String, _ => String>>, acc: Map<&2, String>) -> Map<&2, String>:
  P.Session.cache.go(xs, acc)

def Session.scratch.put(cache: Bool, k: String, v: String, rest: List<&2, J.KV>) -> List<&2, J.KV>:
  P.Session.scratch.put(cache, k, v, rest)

def Session.scratch.fields(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, J.KV>:
  P.Session.scratch.fields(xs)

def Session.drafts(+j: J.Json, +old: Map<&2, String>) -> Map<&2, String>:
  P.Session.drafts(j, old)

def Session.pack(u: U.Ui) -> J.Json:
  P.Session.pack(u)

def Session.adopt(u: U.Ui, +o: J.Json) -> U.Ui:
  P.Session.adopt(u, o)

def Session.reading.ui(away: Bool, +u: U.Ui, +id: String, +seen: String, count: Nat) -> U.Ui:
  P.Session.reading.ui(away, u, id, seen, count)

def Session.reading(session: J.Json, id: String, seen: String, count: Nat, away: Bool) -> J.Json:
  P.Session.reading(session, id, seen, count, away)

def Session.draft(+session: J.Json, +text: String) -> J.Json:
  P.Session.draft(session, text)

def Session.prepare.k(send: Bool, +session: J.Json, +input: J.Json) -> J.Json:
  P.Session.prepare.k(send, session, input)

def Session.prepare(session: J.Json, input: J.Json, +action: String) -> J.Json:
  P.Session.prepare(session, input, action)

def Session.failed.k(send: Bool, +key: String, +input: J.Json) -> List<&2, J.Json>:
  P.Session.failed.k(send, key, input)

def Session.failed(key: String, input: J.Json, +action: String) -> List<&2, J.Json>:
  P.Session.failed(key, input, action)

def Session.reply(session: J.Json, o: J.Json) -> J.Json:
  P.Session.reply(session, o)

def Session.input(+st: M.State, info: J.Json, live: J.Json, +now: Nat, +cid: String, session: J.Json) -> U.Ui:
  P.Session.input(st, info, live, now, cid, session)

def Session.mail.action(+action: String, +value: String) -> Bool:
  P.Session.mail.action(action, value)


def Session.handoff.one(hit: Bool, u: U.Ui, session: J.Json) -> U.Ui:
  match hit:
    case True{}: Session.adopt(u, session)
    case False{}: u

def Session.handoff.go(hs: List<&2, H.Hub>, +key: String, +session: J.Json) -> List<&2, H.Hub>:
  match hs:
    case Nil{}: Nil{}
    case H.Hub{+k, u} <> t:
      H.Hub{k, Session.handoff.one(String.eq(k, key), u, session)} <> Session.handoff.go(t, key, session)

def Session.handoff(hs: H.Hubs, +key: String, session: J.Json, cur: String) -> H.Hubs:
  match hs:
    case H.Hubs{cid, now, _, list}:
      H.Hubs{cid, now, cur, Session.handoff.go(list, key, session)}

def Session.local(session: J.Json, key: String, value: String) -> J.Json:
  Session.pack(H.Hubs.ui(H.Hubs.local(H.Hubs{"", 0n, "local", [H.Hub{"local", Session.adopt(U.Ui.init(0n), session)}]}, key, value), "local"))

# The authenticated connection still executes every Send through its normal
# RPC path. This pure renderer never executes commands or writes events.
def Session.act.k(empty: Bool, +hs: H.Hubs, action: String, value: String) -> H.HAct:
  match empty:
    case True{}: H.HAct{hs, Nil{}, Nil{}}
    case False{}: H.Hubs.act(hs, action, value)

def Session.act(+u: U.Ui, +key: String, +cid: String, +now: Nat, +action: String, value: String) -> H.HAct:
  Session.act.k(String.is_empty(action), H.Hubs{cid, now, key, [H.Hub{key, u}]}, action, value)

def Session.commands(cs: List<&2, H.HCmd>) -> List<&2, J.Json>:
  match cs:
    case Nil{}: Nil{}
    case H.HCmd{k, U.Send{text}} <> t:
      J.Json.obj([J.KV{"type", J.Json.str("send")}, J.KV{"hub", J.Json.str(k)}, J.KV{"text", J.Json.str(text)}]) <> Session.commands(t)
    case H.HCmd{k, U.Keep{thread, text}} <> t:
      J.Json.obj([J.KV{"type", J.Json.str("keep")}, J.KV{"thread", J.Json.str(k ++ "|" ++ thread)}, J.KV{"text", J.Json.str(text)}]) <> Session.commands(t)
    case H.HCmd{_, U.Focus{id}} <> t:
      J.Json.obj([J.KV{"type", J.Json.str("focus")}, J.KV{"id", J.Json.str(id)}]) <> Session.commands(t)
    case H.HCmd{_, U.ScrollEnd{}} <> t:
      J.Json.obj([J.KV{"type", J.Json.str("scroll")}]) <> Session.commands(t)
    case H.HCmd{k, U.Later{ms, action, value}} <> t:
      J.Json.obj([J.KV{"type", J.Json.str("later")}, J.KV{"ms", J.Json.nat(ms)}, J.KV{"action", J.Json.str(action)}, J.KV{"value", J.Json.str(k ++ "|" ++ value)}]) <> Session.commands(t)
    case H.HCmd{_, U.Jump{id}} <> t:
      J.Json.obj([J.KV{"type", J.Json.str("jump")}, J.KV{"id", J.Json.str(id)}]) <> Session.commands(t)
    case _ <> t: Session.commands(t)

def Session.shown.json(xs: List<&2, H.Shown>) -> J.Json:
  match xs:
    case H.Shown{k, name, j} <> _:
      J.Json.obj([J.KV{"key", J.Json.str(k)}, J.KV{"name", J.Json.str(name)}, J.KV{"j", j}])
    case Nil{}: J.Null{}

def Session.fields.hit(fs: J.Json, key: String) -> Bool:
  match fs:
    case J.Field{k, _, _}: String.eq(k, key)
    case _: False{}

def Session.fields.set(fs: J.Json, +key: String, +value: J.Json, hit: Bool) -> J.Json:
  match fs hit:
    case J.Field{k, _, tail} True{}: J.Field{k, value, tail}
    case J.Field{k, v, +tail} False{}: J.Field{k, v, Session.fields.set(tail, key, value, Session.fields.hit(tail, key))}
    case _ _: fs

def Session.object.set(j: J.Json, +key: String, +value: J.Json) -> J.Json:
  match j:
    case J.Obj{+fs}: J.Obj{Session.fields.set(fs, key, value, Session.fields.hit(fs, key))}
    case _: j

# The composer for the draft typed now, from the thread the hub drew (the
# phones' twin of the web's View.local): the hub's rows stay while its
# render is for this draft, or while neither draft is a slash command; else
# the draft's slash commands (Claude's own only in a Claude thread, the
# thread's "claude"). The send button follows the draft as the hub's would
# (U.Follow: Stop while a turn runs and nothing is typed or attached).
# A key on a phone changes only these fields (Composer.fields), so the
# native side redraws the composer and its popup, never the screen.
def Composer.fields() -> List<&2, String>:
  ["draft", "skills", "send", "sendAct"]

def Composer.slash(typing: Bool, +claude: Bool, +draft: String) -> J.Json:
  match typing:
    case True{}: J.Json.arr(SC.Mob.skills(U.Slash.local(claude, draft)))
    case False{}: J.Arr{J.End{}}

def Composer.rows.k(mine: Bool, +thread: J.Json, +claude: Bool, +draft: String) -> J.Json:
  match mine:
    case True{}: J.Json.get_obj(thread, "skills")
    case False{}: Composer.slash(U.Slash.typing.d(draft), claude, draft)

def Composer.rows(+thread: J.Json, +draft: String) -> J.Json:
  +drawn = J.Json.get_str(thread, "draft", "")
  Composer.rows.k(Bool.or(String.eq(drawn, draft), Bool.not(Bool.or(U.Slash.typing.d(draft), U.Slash.typing.d(drawn)))),
    thread, J.Json.get_bool(thread, "claude", False{}), draft)

def Composer.stop(+thread: J.Json, +draft: String) -> Bool:
  U.Follow.stop.k(String.eq(J.Json.get_str(thread, "state", ""), "run"),
    Bool.and(String.is_empty(String.trim(draft)), List.is_empty(&2, J.Json, J.Json.elems(J.Json.get_obj(thread, "attaching")))))

def Composer.of(+thread: J.Json, +draft: String, +lc: I.Loc) -> J.Json:
  +stop = Composer.stop(thread, draft)
  J.Json.obj([J.KV{"draft", J.Json.str(draft)}, J.KV{"skills", Composer.rows(thread, draft)},
    J.KV{"send", J.Json.str(U.Follow.button(lc, M.State.empty(), "", String.eq(J.Json.get_str(thread, "state", ""), "run"), stop, draft))},
    J.KV{"sendAct", J.Json.str(U.Follow.act(stop))}])

def Composer.put(fs: List<&2, String>, +thread: J.Json, +c: J.Json) -> J.Json:
  match fs:
    case Nil{}: thread
    case +k <> t: Composer.put(t, Session.object.set(thread, k, J.Json.get_obj(c, k)), c)

# the names of a composer's rows (for laws and tests)
def Composer.names(rows: List<&2, J.Json>) -> List<&2, String>:
  match rows:
    case Nil{}: Nil{}
    case +r <> t: J.Json.get_str(r, "name", "") <> Composer.names(t)

# the thread with its composer drawn for the draft: the hub's own fields
# replaced, none added (an absent thread stays absent)
def Composer.thread(+thread: J.Json, +draft: String, +lc: I.Loc) -> J.Json:
  Composer.put(Composer.fields(), thread, Composer.of(thread, draft, lc))

def Session.shown.draft(+shown: J.Json, +session: J.Json, +lc: I.Loc) -> J.Json:
  Session.object.set(shown, "j", Session.object.set(J.Json.get_obj(shown, "j"), "thread",
    Composer.thread(J.Json.get_obj(J.Json.get_obj(shown, "j"), "thread"),
      J.Json.get_str(J.Json.get_obj(session, "drafts"), J.Json.get_str(session, "sel", ""), ""), lc)))

def Session.issued.go(cs: List<&2, J.Json>) -> List<&2, J.Json>:
  match cs:
    case Nil{}: Nil{}
    case c <> t: Session.object.set(c, "type", J.Json.str("send-issued")) <> Session.issued.go(t)

def Session.issued(key: String, input: J.Json, action: String) -> J.Json:
  J.Json.arr(Session.issued.go(Session.failed(key, input, action)))

def Session.result.shown(+hs: H.Hubs, cs: List<&2, H.HCmd>, +key: String, +shown: List<&2, H.Shown>) -> J.Json:
  J.Json.obj([J.KV{"mobile", J.Json.nat(1n)}, J.KV{"session", Session.pack(H.Hubs.ui(hs, key))},
    J.KV{"shown", Session.shown.json(shown)},
    J.KV{"screen", H.Hubs.screen.k(H.Hubs.loc(hs), H.Hubs.relabel(shown), key, H.Hubs.list(hs), H.Hubs.pl.want(H.Hubs.list(hs), key))},
    J.KV{"cmds", J.Json.arr(Session.commands(cs))}])

def Session.image.missing(missing: Bool, u: U.Ui, id: String, text: String) -> U.Ui:
  match missing:
    case True{}: SC.Mob.imgs.put(id, text, u)
    case False{}: u

def Session.image.step(+u: U.Ui, e: M.Entry) -> U.Ui:
  match e:
    case M.Msg{+id, _, M.Assistant{}, text, _}:
      Session.image.missing(String.is_empty(U.Ui.scratch(u, SC.Mob.imgs.key(id))), u, id, text)
    case _: u

def Session.images.go(es: List<&2, M.Entry>, u: U.Ui) -> U.Ui:
  match es:
    case Nil{}: u
    case e <> t: Session.images.go(t, Session.image.step(u, e))

def Session.images.page(u: U.Ui) -> U.Win:
  match u:
    case U.Ui{st, sel, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      SC.Mob.page(dr, sel, st)

def Session.images(+u: U.Ui) -> U.Ui:
  Session.images.go(U.Win.shown(Session.images.page(u)), u)

def Session.images.hubs(hs: List<&2, H.Hub>) -> List<&2, H.Hub>:
  match hs:
    case Nil{}: Nil{}
    case H.Hub{k, u} <> t: H.Hub{k, Session.images(u)} <> Session.images.hubs(t)

def Session.result.warm(+hs: H.Hubs, cs: List<&2, H.HCmd>, +key: String) -> J.Json:
  Session.result.shown(hs, cs, key, H.Hubs.shown(H.Hubs.loc(hs), H.Hubs.list(hs), key))

def Session.result.hubs(hs: H.Hubs, cs: List<&2, H.HCmd>, key: String) -> J.Json:
  match hs:
    case H.Hubs{cid, now, cur, list}:
      Session.result.warm(H.Hubs{cid, now, cur, Session.images.hubs(list)}, cs, key)

def Session.result(a: H.HAct, +key: String) -> J.Json:
  match a:
    case H.HAct{+hs, cs, _}:
      Session.result.hubs(hs, cs, key)

def Session.render(st: M.State, info: J.Json, live: J.Json, +now: Nat, +p: J.Json) -> J.Json:
  Session.result(Session.act(Session.input(st, info, live, now, J.Json.get_str(p, "cid", ""), J.Json.get_obj(p, "session")),
    J.Json.get_str(p, "key", ""), J.Json.get_str(p, "cid", ""), now, J.Json.get_str(p, "action", ""), J.Json.get_str(p, "value", "")), J.Json.get_str(p, "key", ""))

def Session.overlay.hit(js: List<&2, J.Json>, key: String) -> Bool:
  match js:
    case j <> _: String.eq(J.Json.get_str(j, "key", ""), key)
    case Nil{}: False{}

def Session.overlay.find.go(js: List<&2, J.Json>, +key: String, hit: Bool) -> Maybe<&2, H.Shown>:
  match js hit:
    case +j <> _ True{}: Some{H.Shown{key, J.Json.get_str(j, "name", ""), J.Json.get_obj(j, "j")}}
    case _ <> +t False{}: Session.overlay.find.go(t, key, Session.overlay.hit(t, key))
    case Nil{} _: None{}

def Session.overlay.find(+js: List<&2, J.Json>, +key: String) -> Maybe<&2, H.Shown>:
  Session.overlay.find.go(js, key, Session.overlay.hit(js, key))

def Session.overlay.one(m: Maybe<&2, H.Shown>, +lc: I.Loc, +key: String, u: U.Ui, cur: String) -> List<&2, H.Shown>:
  match m:
    case Some{H.Shown{k, name, j}}: [H.Shown{k, name, H.Hubs.mail.loading(u, j)}]
    case None{}: H.Hubs.shown(lc, [H.Hub{key, u}], cur)

def Session.overlay.go(hs: List<&2, H.Hub>, +lc: I.Loc, +js: List<&2, J.Json>, +cur: String) -> List<&2, H.Shown>:
  match hs:
    case Nil{}: Nil{}
    case H.Hub{+key, u} <> t:
      List.append(&2, H.Shown, Session.overlay.one(Session.overlay.find(js, key), lc, key, u, cur), Session.overlay.go(t, lc, js, cur))

def Session.overlay.k(+hs: H.Hubs, +cur: String, +shown: List<&2, H.Shown>) -> J.Json:
  H.Hubs.screen.k(H.Hubs.loc(hs), H.Hubs.relabel(H.Hubs.catalog.shown(shown, H.Hubs.list(hs))), cur, H.Hubs.list(hs), H.Hubs.pl.want(H.Hubs.list(hs), cur))

def Session.overlay(+hs: H.Hubs, js: J.Json, +cur: String) -> J.Json:
  Session.overlay.k(hs, cur, Session.overlay.go(H.Hubs.list(hs), H.Hubs.loc(hs), J.Json.elems(js), cur))

def Session.target.k(qualified: Bool, cur: String, +value: String) -> String:
  match qualified:
    case True{}: H.Hubs.key_of(value)
    case False{}: cur

def Session.target.ref.known(known: Bool, cur: String, key: String) -> String:
  match known:
    case True{}: key
    case False{}: cur

# Recent-first sessions have no canonical history yet. Send an explicit
# source/origin to that paired hub, where the same resolver checks existence.
# Unknown explicit hubs remain in the payload and fail resolution there.
def Session.target.ref.json(+hs: H.Hubs, +cur: String, +j: J.Json) -> String:
  Session.target.ref.known(H.Hubs.has(H.Hubs.list(hs), J.Json.get_str(j, "hub", "")), cur, J.Json.get_str(j, "hub", ""))

def Session.target.ref(internal: Bool, +hs: H.Hubs, cur: String, +action: String, +value: String) -> String:
  match internal:
    case True{}: Session.target.ref.json(hs, cur, Maybe.default(&2, J.Json, J.Json.parse(value), J.Null{}))
    case False{}:
      Session.target.k(Bool.and(Bool.or(String.eq(action, "usage-refresh"), Nat.is_lt(H.Hubs.routed(action), H.Hubs.routes())), H.Hubs.has(H.Hubs.list(hs), H.Hubs.key_of(value))), cur, value)

def Session.target(+hs: H.Hubs, cur: String, +action: String, +value: String) -> String:
  Session.target.ref(Bool.or(String.eq(action, "ref-open"), String.eq(action, "mailroom-action")), hs, cur, action, value)

# No Mailroom mutation may enter Recent.waiting and execute after handoff.
