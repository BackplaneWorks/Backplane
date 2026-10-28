import Base
import ./json.bend as J
import ./client.bend as U
import ./model.bend as M
import ./outbox.bend as OB

# Transient presentation choices shared by mobile and desktop. No state,
# replay cursor, device identity or typed body cache is decoded from JSON.

def Session.nat.max() -> Nat:
  Nat.add(Nat.mul(65536n, 4294967295n), 65535n)

def Session.nat.step(valid: Bool, acc: Nat, digit: Nat) -> Maybe<&2, Nat>:
  match valid:
    case False{}: None{}
    case True{}: Some{Nat.add(Nat.mul(acc, 10n), digit)}

def Session.nat.digit(+c: U32, +acc: Nat, +limit: Nat) -> Maybe<&2, Nat>:
  +digit = U32.to_nat((c - 48 : U32))
  Session.nat.step(Bool.and(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)),
    Bool.or(Nat.is_lt(acc, Nat.div(limit, 10n)), Bool.and(Nat.is_eq(acc, Nat.div(limit, 10n)), Nat.is_le(digit, Nat.mod(limit, 10n))))), acc, digit)

def Session.nat.go(s: String, value: Maybe<&2, Nat>, +limit: Nat) -> Maybe<&2, Nat>:
  match s value:
    case _ None{}: None{}
    case SNil{} Some{n}: Some{n}
    case SCon{Chr{c}, tail} Some{n}: Session.nat.go(tail, Session.nat.digit(c, n, limit), limit)

def Session.nat.canonical(+raw: String, value: Maybe<&2, Nat>) -> Maybe<&2, Nat>:
  match value:
    case None{}: None{}
    case Some{+n}: Bool.pick(Maybe<&2, Nat>, String.eq(raw, Nat.show(n)), Some{n}, None{})

def Session.nat.short(short: Bool, +raw: String, limit: Nat) -> Maybe<&2, Nat>:
  match short:
    case False{}: None{}
    case True{}: Session.nat.canonical(raw, Session.nat.go(raw, Some{0n}, limit))

# Check each digit and pre-multiply domain bound before arithmetic. No
# Nat.read, coercion, oversized accumulator or invalid-digit subtraction
# reaches native Nat construction. Empty/noncanonical numbers are refused.
def Session.nat(+raw: String, limit: Nat) -> Maybe<&2, Nat>:
  Session.nat.short(Nat.is_le(String.length(raw), 15n), raw, limit)

def Session.number.field(value: Maybe<&2, J.Json>, fallback: Nat) -> Nat:
  match value:
    case Some{J.Num{raw}}: Maybe.default(&2, Nat, Session.nat(raw, Session.nat.max()), fallback)
    case _: fallback

def Session.number(+o: J.Json, key: String, fallback: Nat) -> Nat:
  Session.number.field(J.Json.get(o, key), fallback)

def Session.keyed.empty(empty: Bool, key: String, id: String) -> String:
  match empty:
    case True{}: ""
    case False{}: key ++ "|" ++ id

def Session.keyed(key: String, +id: String) -> String:
  Session.keyed.empty(String.is_empty(id), key, id)

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
# what a session keeps of its info between requests: folds, form fields,
# and a thread's ordering card from its fab.look reply (fab.ready:<t>,
# fab.buy:<t>, fab.line:<t>...); without those the Ready to order bar and
# the header's Buy it never showed in a thin view; and each thread's
# viewer, open or shut, its kind and file, which the page keeps across a
# reload (client.bend's Viewer.keep)
def Session.info.key(+k: String) -> Bool:
  Bool.or(Bool.or(String.starts_with(k, "fold:"), Bool.or(String.starts_with(k, "subs:"), Bool.or(String.starts_with(k, "@f."), String.starts_with(k, "fab.")))),
    Bool.or(String.starts_with(k, "panel:"), Bool.or(String.starts_with(k, "view.kind:"), String.starts_with(k, "view.path:"))))

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
# Mailroom cache/request/send state belongs to the canonical recipient.
# A recent presentation cannot issue or adopt this state before replay ends.
def Session.mail.key(+key: String) -> Bool:
  Bool.or(String.starts_with(key, "@wm:"), Bool.or(String.starts_with(key, "@mr."), String.starts_with(key, "@f.mr")))

def Session.mail.remote.put(keep: Bool, acc: Map<&2, String>, key: String, value: String) -> Map<&2, String>:
  match keep:
    case True{}: Map.set(&2, String, acc, key, value)
    case False{}: acc

def Session.mail.remote(j: J.Json, acc: Map<&2, String>) -> Map<&2, String>:
  match j:
    case J.Obj{fs}: Session.mail.remote(fs, acc)
    case J.Field{+key, J.Str{value}, tail}:
      Session.mail.remote(tail, Session.mail.remote.put(Bool.not(Session.mail.key(key)), acc, key, value))
    case J.Field{_, _, tail}: Session.mail.remote(tail, acc)
    case _: acc

def Session.cache.put(keep: Bool, acc: Map<&2, String>, k: String, v: String) -> Map<&2, String>:
  match keep:
    case True{}: Map.set(&2, String, acc, k, v)
    case False{}: acc

def Session.cache.go(xs: List<&2, Sigma<&2, &2, String, _ => String>>, acc: Map<&2, String>) -> Map<&2, String>:
  match xs:
    case Nil{}: acc
    case (+k, v) <> t: Session.cache.go(t, Session.cache.put(Bool.or(String.starts_with(k, "@img:"), Session.mail.key(k)), acc, k, v))

def Session.scratch.put(cache: Bool, k: String, v: String, rest: List<&2, J.KV>) -> List<&2, J.KV>:
  match cache:
    case True{}: rest
    case False{}: J.KV{k, J.Json.str(v)} <> rest

def Session.scratch.fields(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, J.KV>:
  match xs:
    case Nil{}: Nil{}
    case (+k, v) <> t: Session.scratch.put(Bool.or(String.starts_with(k, "@img:"), Session.mail.key(k)), k, v, Session.scratch.fields(t))

def Session.drafts(+j: J.Json, +old: Map<&2, String>) -> Map<&2, String>:
  Map.set(&2, String, Map.set(&2, String, Session.mail.remote(j, Session.cache.go(Map.to_list(&2, String, old), Map.new(&2, String))), "@boot", U.Str.get(old, "@boot")), "@sync", U.Str.get(old, "@sync"))

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
        M.Time.max(rid, Session.number(o, "rid", 0n)), J.Json.get_bool(o, "wantNew", False{}),
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
        "@more." ++ id, Nat.show(M.Time.max(Maybe.default(&2, Nat, Session.nat(U.Ui.scratch(u, "@more." ++ id), Session.nat.max()), 0n), Nat.add(Nat.div(count, U.Page.size()), 1n))))

def Session.reading(session: J.Json, id: String, seen: String, count: Nat, away: Bool) -> J.Json:
  Session.pack(Session.reading.ui(away, Session.adopt(U.Ui.init(0n), session), id, seen, count))

# a thread picked in the hub-drawn view leaves Settings, as Ui.select does,
# and brings its own viewer, open or shut, as it was left (the one left
# keeps its own: Ui.reselect)
def Session.select(+session: J.Json, +id: String) -> J.Json:
  Session.pack(U.Ui.reselect(J.Json.get_str(session, "sel", ""), U.Ui.select.nav(Session.adopt(U.Ui.init(0n), session), id)))

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
      [J.Json.obj([J.KV{"type", J.Json.str("send-failed")}, J.KV{"thread", J.Json.str(Session.keyed(key, J.Json.get_str(input, "sel", "")))},
        J.KV{"text", J.Json.str(J.Json.get_str(J.Json.get_obj(input, "drafts"), J.Json.get_str(input, "sel", ""), ""))}])]
    case False{}: Nil{}

def Session.failed(key: String, input: J.Json, +action: String) -> List<&2, J.Json>:
  Session.failed.k(Bool.or(String.eq(action, "send"), String.eq(action, "send-alt")), key, input)

def Session.reply(session: J.Json, o: J.Json) -> J.Json:
  Session.pack(U.Ui.reply(Session.adopt(U.Ui.init(0n), session), o))

def Session.input(+st: M.State, info: J.Json, live: J.Json, +now: Nat, +cid: String, session: J.Json) -> U.Ui:
  U.Ui.scratch_set(U.Ui.recv(U.Ui.online(U.Ui.info_merge(U.Ui.set_cid(U.Ui.adopt(Session.adopt(U.Ui.init(now), session), M.State.private(st)), cid), info), True{}), live), "@sync", "1")

def Session.mail.action(+action: String, +value: String) -> Bool:
  Bool.or(String.starts_with(action, "mail-"), Bool.or(String.eq(action, "mailroom-action"),
    Bool.or(Bool.and(String.eq(action, "bot-tab"), String.eq(value, "mail")),
      Bool.and(String.eq(action, "bfield"), String.starts_with(value, "mrreply" ++ SCon{Chr{31}, SNil{}})))))

# Preserve the same writer's accounting across the presentation/history fence.
# Only boot progress, remainder and mirror cursor change.
def Session.history_credit(cr: OB.Cr, at: Nat, more: Bool, rest: List<&2, String>) -> OB.Cr:
  OB.Cr.rest.set(OB.Cr.wait.set(OB.Cr.catch(cr, at, 0n), more), rest)

def Session.resume_cursor(+since: Nat, requested: String, actual: String, +count: Nat) -> Nat:
  Bool.pick(Nat, Bool.and(String.eq(requested, actual), Nat.is_le(since, count)), since, 0n)

# Explicit frontend clients may open a presentation before their cached frames
# finish folding. Only history-start receives that fully applied local cursor.
type Resume is Data:
  Captured{}
  Cursor{since: Nat, origin: String}
  Invalid{}

def Session.cursor.number(value: Maybe<&2, Nat>, origin: String) -> Resume:
  match value:
    case None{}: Invalid{}
    case Some{n}: Cursor{n, origin}

def Session.cursor.short(short: Bool, +raw: String, +origin: String) -> Resume:
  match short:
    case False{}: Invalid{}
    case True{}:
      Session.cursor.number(Session.nat(raw, 4294967295n), origin)

def Session.cursor.fields(since: Maybe<&2, J.Json>, origin: Maybe<&2, J.Json>) -> Resume:
  match since origin:
    case None{} None{}: Captured{}
    case Some{J.Num{+raw}} Some{J.Str{+value}}:
      Session.cursor.short(Bool.and(Nat.is_le(String.length(raw), 10n), Nat.is_le(String.length(value), 128n)), raw, value)
    case _ _: Invalid{}

def Session.cursor(+p: J.Json) -> Resume:
  Session.cursor.fields(J.Json.get(p, "since"), J.Json.get(p, "origin"))

def Session.resume_allowed(mode: String, resume: Resume) -> Bool:
  match mode resume:
    case _ Invalid{}: False{}
    case "frontend" Captured{}: False{}
    case _ _: True{}

def Session.mode.mobile(mobile: Bool) -> String:
  match mobile:
    case True{}: "mobile"
    case False{}: ""

def Session.mode.frontend(frontend: Bool, mobile: String) -> String:
  match frontend:
    case True{}: "frontend"
    case False{}: Session.mode.mobile(String.eq(mobile, "1"))

def Session.mode(mobile: String, frontend: String) -> String:
  Session.mode.frontend(String.eq(frontend, "1"), mobile)

# Complete command transport, not execution. A matched presentation response
# supplies an instruction once; normal authenticated RPCs still do all IO.
def Session.command(c: U.Cmd) -> J.Json:
  match c:
    case U.Send{text}: J.Json.obj([J.KV{"type", J.Str{"send"}}, J.KV{"text", J.Str{text}}])
    case U.Copy{text}: J.Json.obj([J.KV{"type", J.Str{"copy"}}, J.KV{"text", J.Str{text}}])
    case U.Focus{id}: J.Json.obj([J.KV{"type", J.Str{"focus"}}, J.KV{"id", J.Str{id}}])
    case U.ScrollEnd{}: J.Json.obj([J.KV{"type", J.Str{"scroll"}}])
    case U.Connect{url}: J.Json.obj([J.KV{"type", J.Str{"connect"}}, J.KV{"url", J.Str{url}}])
    case U.Keep{thread, text}: J.Json.obj([J.KV{"type", J.Str{"keep"}}, J.KV{"thread", J.Str{thread}}, J.KV{"text", J.Str{text}}])
    case U.Notify{thread, title, body, key}: J.Json.obj([J.KV{"type", J.Str{"notify"}}, J.KV{"thread", J.Str{thread}}, J.KV{"title", J.Str{title}}, J.KV{"body", J.Str{body}}, J.KV{"key", J.Str{key}}])
    case U.Later{ms, action, value}: J.Json.obj([J.KV{"type", J.Str{"later"}}, J.KV{"ms", J.Json.nat(ms)}, J.KV{"action", J.Str{action}}, J.KV{"value", J.Str{value}}])
    case U.Jump{id}: J.Json.obj([J.KV{"type", J.Str{"jump"}}, J.KV{"id", J.Str{id}}])

def Session.commands(cs: List<&2, U.Cmd>) -> List<&2, J.Json>:
  match cs:
    case Nil{}: Nil{}
    case c <> tail: Session.command(c) <> Session.commands(tail)
