import Base
import ../core/json.bend as J
import ../core/client.bend as U
import ../core/model.bend as M
import ../core/i18n.bend as I
import ./hubs.bend as H
import ./screen.bend as SC

# A presentation session contains only the person's transient choices. It
# cannot supply a read model, a log origin, or a canonical replay cursor.
# The server renders against its already-folded state; the phone continues
# replaying the real ordered log independently in the background.
def Session.strings(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, J.KV>:
  match xs:
    case Nil{}: Nil{}
    case (k, v) <> t: J.KV{k, J.Json.str(v)} <> Session.strings(t)

def Session.flags(xs: List<&2, Sigma<&2, &2, String, _ => Bool>>) -> List<&2, J.KV>:
  match xs:
    case Nil{}: Nil{}
    case (k, v) <> t: J.KV{k, J.Json.bool(v)} <> Session.flags(t)

def Session.read.strings(j: J.Json, acc: Map<&2, String>) -> Map<&2, String>:
  match j:
    case J.Obj{fs}: Session.read.strings(fs, acc)
    case J.Field{k, J.Str{v}, t}: Session.read.strings(t, Map.set(&2, String, acc, k, v))
    case J.Field{_, _, t}: Session.read.strings(t, acc)
    case _: acc

def Session.read.flags(j: J.Json, acc: Map<&2, Bool>) -> Map<&2, Bool>:
  match j:
    case J.Obj{fs}: Session.read.flags(fs, acc)
    case J.Field{k, J.Flag{v}, t}: Session.read.flags(t, Map.set(&2, Bool, acc, k, v))
    case J.Field{_, _, t}: Session.read.flags(t, acc)
    case _: acc

# Only presentation fields may cross the handoff. In particular the saved
# origin, connection generation, live-buffer revisions and replay flags stay
# with the canonical client, even if a late response carries older values.
def Session.info.key(+k: String) -> Bool:
  Bool.or(String.starts_with(k, "fold:"), Bool.or(String.starts_with(k, "subs:"), String.starts_with(k, "@f.")))

def Session.info.put(keep: Bool, acc: Map<&2, String>, k: String, v: String) -> Map<&2, String>:
  match keep:
    case True{}: Map.set(&2, String, acc, k, v)
    case False{}: acc

def Session.info.read(j: J.Json, acc: Map<&2, String>) -> Map<&2, String>:
  match j:
    case J.Obj{fs}: Session.info.read(fs, acc)
    case J.Field{+k, J.Str{v}, t}: Session.info.read(t, Session.info.put(Session.info.key(k), acc, k, v))
    case J.Field{_, _, t}: Session.info.read(t, acc)
    case _: acc

def Session.info.fields.put(keep: Bool, k: String, v: String, rest: List<&2, J.KV>) -> List<&2, J.KV>:
  match keep:
    case True{}: J.KV{k, J.Json.str(v)} <> rest
    case False{}: rest

def Session.info.fields(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, J.KV>:
  match xs:
    case Nil{}: Nil{}
    case (+k, v) <> t: Session.info.fields.put(Session.info.key(k), k, v, Session.info.fields(t))

# Parsed message-image paths are recipient-local derived data, never a
# presentation preference. Keep them across adoption without sending the
# entire history's cache with each screen/action request.
def Session.cache.put(keep: Bool, acc: Map<&2, String>, k: String, v: String) -> Map<&2, String>:
  match keep:
    case True{}: Map.set(&2, String, acc, k, v)
    case False{}: acc

def Session.cache.go(xs: List<&2, Sigma<&2, &2, String, _ => String>>, acc: Map<&2, String>) -> Map<&2, String>:
  match xs:
    case Nil{}: acc
    case (+k, v) <> t: Session.cache.go(t, Session.cache.put(String.starts_with(k, "@img:"), acc, k, v))

def Session.scratch.put(cache: Bool, k: String, v: String, rest: List<&2, J.KV>) -> List<&2, J.KV>:
  match cache:
    case True{}: rest
    case False{}: J.KV{k, J.Json.str(v)} <> rest

def Session.scratch.fields(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, J.KV>:
  match xs:
    case Nil{}: Nil{}
    case (+k, v) <> t: Session.scratch.put(String.starts_with(k, "@img:"), k, v, Session.scratch.fields(t))

def Session.drafts(+j: J.Json, +old: Map<&2, String>) -> Map<&2, String>:
  Map.set(&2, String, Map.set(&2, String, Session.read.strings(j, Session.cache.go(Map.to_list(&2, String, old), Map.new(&2, String))), "@boot", U.Str.get(old, "@boot")), "@sync", U.Str.get(old, "@sync"))

def Session.pack(u: U.Ui) -> J.Json:
  match u:
    case U.Ui{_, sel, dr, _, op, _, _, ad, pa, _, rid, w, er, note, nf, ob, _}:
      J.Json.obj([J.KV{"sel", J.Json.str(sel)},
        J.KV{"drafts", J.Json.obj(Session.scratch.fields(Map.to_list(&2, String, dr)))},
        J.KV{"open", J.Json.obj(Session.flags(Map.to_list(&2, Bool, op)))},
        J.KV{"adding", J.Json.bool(ad)}, J.KV{"panel", J.Json.bool(pa)},
        J.KV{"rid", J.Json.nat(rid)}, J.KV{"wantNew", J.Json.bool(w)},
        J.KV{"error", J.Json.str(er)}, J.KV{"note", J.Json.str(note)},
        J.KV{"info", J.Json.obj(Session.info.fields(Map.to_list(&2, String, nf)))},
        J.KV{"outbox", J.Json.obj(Session.strings(Map.to_list(&2, String, ob)))}])

# Used both before a server render and at the final client handoff. State,
# live buffers, online status, clock, version and typed local stores always
# come from the recipient, never from a serialized presentation response.
def Session.adopt(u: U.Ui, +o: J.Json) -> U.Ui:
  match u:
    case U.Ui{st, _, +dr, live, _, on, now, _, _, v, +rid, _, _, _, +nf, ob, stores}:
      U.Ui{st, J.Json.get_str(o, "sel", ""), Session.drafts(J.Json.get_obj(o, "drafts"), dr), live,
        Session.read.flags(J.Json.get_obj(o, "open"), Map.new(&2, Bool)), on, now,
        J.Json.get_bool(o, "adding", False{}), J.Json.get_bool(o, "panel", False{}), v,
        M.Time.max(rid, J.Json.get_nat(o, "rid", 0n)), J.Json.get_bool(o, "wantNew", False{}),
        J.Json.get_str(o, "error", ""), J.Json.get_str(o, "note", ""),
        Session.info.read(J.Json.get_obj(o, "info"), nf),
        Session.read.strings(J.Json.get_obj(o, "outbox"), ob), stores}

# Carry the page the person is already reading across cached-screen startup.
# This is presentation scratch only; canonical state and replay never change.
def Session.reading.ui(away: Bool, +u: U.Ui, +id: String, +seen: String, count: Nat) -> U.Ui:
  match away:
    case False{}: U.Hold.end(u, id)
    case True{}:
      U.Ui.scratch_set(U.Ui.scratch_set(u, "@away." ++ id,
        Bool.pick(String, String.is_empty(U.Ui.scratch(u, "@away." ++ id)), seen, U.Ui.scratch(u, "@away." ++ id))),
        "@more." ++ id, Nat.show(M.Time.max(Maybe.default(&2, Nat, Nat.read(U.Ui.scratch(u, "@more." ++ id)), 0n), Nat.add(Nat.div(count, U.Page.size()), 1n))))

def Session.reading(session: J.Json, id: String, seen: String, count: Nat, away: Bool) -> J.Json:
  Session.pack(Session.reading.ui(away, Session.adopt(U.Ui.init(0n), session), id, seen, count))

def Session.draft(+session: J.Json, +text: String) -> J.Json:
  Session.pack(U.Ui.scratch_set(Session.adopt(U.Ui.init(0n), session), J.Json.get_str(session, "sel", ""), text))

def Session.prepare.k(send: Bool, +session: J.Json, +input: J.Json) -> J.Json:
  match send:
    case True{}:
      Session.pack(U.Ui.scratch_set(U.Ui.select.k(Session.adopt(U.Ui.init(0n), session), J.Json.get_str(input, "sel", "")),
        J.Json.get_str(input, "sel", ""), J.Json.get_str(J.Json.get_obj(input, "drafts"), J.Json.get_str(input, "sel", ""), "")))
    case False{}:
      session

# Capture the exact message and recipient at the tap, even when a later
# keystroke or thread switch overtakes its presentation response.
def Session.prepare(session: J.Json, input: J.Json, +action: String) -> J.Json:
  Session.prepare.k(Bool.or(String.eq(action, "send"), String.eq(action, "send-alt")), session, input)

def Session.failed.k(send: Bool, +key: String, +input: J.Json) -> List<&2, J.Json>:
  match send:
    case True{}:
      [J.Json.obj([J.KV{"type", J.Json.str("send-failed")}, J.KV{"thread", J.Json.str(H.Hubs.id(key, J.Json.get_str(input, "sel", "")))},
        J.KV{"text", J.Json.str(J.Json.get_str(J.Json.get_obj(input, "drafts"), J.Json.get_str(input, "sel", ""), ""))}])]
    case False{}: Nil{}

def Session.failed(key: String, input: J.Json, +action: String) -> List<&2, J.Json>:
  Session.failed.k(Bool.or(String.eq(action, "send"), String.eq(action, "send-alt")), key, input)

def Session.reply(session: J.Json, o: J.Json) -> J.Json:
  Session.pack(U.Ui.reply(Session.adopt(U.Ui.init(0n), session), o))

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
def Session.input(+st: M.State, info: J.Json, live: J.Json, +now: Nat, +cid: String, session: J.Json) -> U.Ui:
  U.Ui.scratch_set(U.Ui.recv(U.Ui.online(U.Ui.info_merge(U.Ui.set_cid(U.Ui.adopt(Session.adopt(U.Ui.init(now), session), M.State.private(st)), cid), info), True{}), live), "@sync", "1")

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

def Session.shown.draft(+shown: J.Json, +session: J.Json) -> J.Json:
  Session.object.set(shown, "j", Session.object.set(J.Json.get_obj(shown, "j"), "thread",
    Session.object.set(J.Json.get_obj(J.Json.get_obj(shown, "j"), "thread"), "draft",
      J.Json.str(J.Json.get_str(J.Json.get_obj(session, "drafts"), J.Json.get_str(session, "sel", ""), "")))))

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
    case Some{s}: [s]
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
  Session.target.ref(String.eq(action, "ref-open"), hs, cur, action, value)
