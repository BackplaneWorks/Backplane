import Base
import ../core/json.bend as J
import ../core/thin-wire.bend as W
import ./thin-actor.bend as A
import ./thin-view.bend as V
import ./thin-sidebar-wire.bend as SW
import ./thin-delta.bend as D

# The hub view protocol's keyed delta on the wire. A client that offers
# {"viewDelta": {"version": 1, "token": T}} holds the body it was sent under
# token T (src/web/thin.js keeps the last three it applied). The reply's
# body ({shown, session, window, overlay}: everything a render makes) is
# kept here under its own token, the request's revision; when T names a
# body kept under the same scope, the reply carries only the ops from that
# body to this one (thin-delta.bend):
#   "viewDelta": {"version": 1, "base": T, "token": <this>, "ops": [...]}
# in place of the body's fields. Otherwise the reply is whole, with
# "viewDelta": {"version": 1, "base": "", "token": <this>}. A client that
# offers nothing gets exactly the old replies (the sidebar region still
# applies to it). The scope is everything that names the picture's world:
# hub, generation, epoch, view, target and renderer, so a base from another
# view, thread or connection is never used.

def Vd.fields() -> List<&2, String>:
  ["shown", "session", "window", "overlay"]

def Vd.offered(+p: J.Json) -> Bool:
  W.Wire.version(Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(p, "viewDelta"), "version"), J.Null{}))

def Vd.body.one(m: Maybe<&2, J.Json>, +n: String, rest: List<&2, J.KV>) -> List<&2, J.KV>:
  match m:
    case Some{+v}: J.KV{n, v} <> rest
    case None{}: rest

def Vd.body.go(names: List<&2, String>, +res: J.Json) -> List<&2, J.KV>:
  match names:
    case Nil{}: Nil{}
    case +n <> tail: Vd.body.one(J.Json.get(res, n), n, Vd.body.go(tail, res))

def Vd.body(+res: J.Json) -> J.Json:
  J.Json.obj(Vd.body.go(Vd.fields(), res))

def Vd.strip.go(names: List<&2, String>, +fs: J.Json) -> J.Json:
  match names:
    case Nil{}: fs
    case +n <> tail: Vd.strip.go(tail, V.Obj.drop(fs, n))

def Vd.strip(+res: J.Json) -> J.Json:
  match res:
    case J.Obj{+fs}: J.Obj{Vd.strip.go(Vd.fields(), fs)}
    case _: res

def Vd.scope(+web: Bool, +a: A.Actor, +p: J.Json) -> String:
  J.Json.show(J.Json.obj([J.KV{"hub", J.Str{J.Json.get_str(p, "hub", "")}},
    J.KV{"generation", J.Str{J.Json.get_str(p, "generation", "")}}, J.KV{"epoch", J.Str{J.Json.get_str(p, "epoch", "")}},
    J.KV{"view", J.Str{A.Sidebar.view(a)}}, J.KV{"target", J.Json.get_obj(p, "target")}, J.KV{"web", J.Flag{web}}]))

# the body kept under this scope and token
def Vd.find(ms: List<&2, A.Memo>, +scope: String, +token: String) -> Maybe<&2, J.Json>:
  match ms:
    case Nil{}: None{}
    case A.ViewSent{+s, +t, +body} <> rest:
      Bool.pick(Maybe<&2, J.Json>, Bool.and(String.eq(s, scope), Bool.and(Bool.not(String.is_empty(t)), String.eq(t, token))), Some{body}, Vd.find(rest, scope, token))
    case _ <> rest: Vd.find(rest, scope, token)

# this body first, then at most two more of the same scope; every other
# memo stays as it was
def Vd.put.one(keep: Bool, +m: A.Memo, rest: List<&2, A.Memo>) -> List<&2, A.Memo>:
  match keep:
    case True{}: m <> rest
    case False{}: rest

def Vd.put.go(ms: List<&2, A.Memo>, +scope: String, +n: Nat) -> List<&2, A.Memo>:
  match ms:
    case Nil{}: Nil{}
    case A.ViewSent{+s, +t, +body} <> rest:
      Vd.put.one(Bool.and(String.eq(s, scope), Nat.is_gt(n, 0n)), A.ViewSent{s, t, body}, Vd.put.go(rest, scope, Nat.sub(n, Bool.pick(Nat, String.eq(s, scope), 1n, 0n))))
    case +m <> rest: m <> Vd.put.go(rest, scope, n)

def Vd.save(a: A.Actor, +scope: String, +token: String, +body: J.Json) -> A.Actor:
  match a:
    case A.Actor{u, view, target, pending, reads, dirty, ms}:
      A.Actor{u, view, target, pending, reads, dirty, A.ViewSent{scope, token, body} <> Vd.put.go(ms, scope, 2n)}

def Vd.meta(+base: String, +token: String) -> List<&2, J.KV>:
  [J.KV{"version", J.Num{"1"}}, J.KV{"base", J.Str{base}}, J.KV{"token", J.Str{token}}]

def Vd.whole.k(long: Bool, +res: J.Json, +token: String, +draft: String) -> J.Json:
  match long:
    case False{}: V.Obj.put(res, "viewDelta", J.Json.obj(Vd.meta("", token)))
    case True{}:
      V.Obj.put(Vd.strip(res), "viewDelta", J.Json.obj(List.append(&2, J.KV, Vd.meta("", token), [J.KV{"ops", J.Json.arr(D.Delta.whole(Vd.body(res), draft))}])))

# a whole body; one that holds a long draft goes as ops from nothing, the
# draft by reference (D.Delta.whole), so it never carries the text back
def Vd.whole(+res: J.Json, +token: String, +draft: String) -> J.Json:
  Vd.whole.k(D.Delta.draft.long(draft), res, token, draft)

def Vd.reply.k(d: Maybe<&2, List<&2, D.Op>>, +res: J.Json, +base: String, +token: String, +draft: String) -> J.Json:
  match d:
    case Some{+ops}:
      V.Obj.put(Vd.strip(res), "viewDelta", J.Json.obj(List.append(&2, J.KV, Vd.meta(base, token), [J.KV{"ops", J.Json.arr(D.Delta.json(ops, draft))}])))
    case None{}: Vd.whole(res, token, draft)

def Vd.reply(m: Maybe<&2, J.Json>, +body: J.Json, +res: J.Json, +base: String, +token: String, +draft: String) -> J.Json:
  match m:
    case Some{+old}: Vd.reply.k(D.Delta.diff(old, body), res, base, token, draft)
    case None{}: Vd.whole(res, token, draft)

# the selected thread's draft this request carried (thin-host.js's
# requestSession sends that one only)
def Vd.draft(+p: J.Json) -> String:
  +session = J.Json.get_obj(p, "session")
  J.Json.get_str(J.Json.get_obj(session, "drafts"), J.Json.get_str(session, "sel", ""), "")

# what the size fit measures: for a client that takes the delta, the
# reply without its own long draft (it never carries it back)
def Vd.measured.k(on: Bool, +p: J.Json, +res: J.Json) -> J.Json:
  match on:
    case True{}: D.Delta.undraft(res, Vd.draft(p))
    case False{}: res

def Vd.measured(+p: J.Json, +res: J.Json) -> J.Json:
  Vd.measured.k(Bool.and(Vd.offered(p), D.Delta.draft.long(Vd.draft(p))), p, res)

def Vd.send(+web: Bool, +a: A.Actor, +res: J.Json, +p: J.Json) -> SW.Output:
  +scope = Vd.scope(web, a, p)
  +token = Nat.show(J.Json.get_nat(p, "requestRevision", 0n))
  +base = J.Json.get_str(J.Json.get_obj(p, "viewDelta"), "token", "")
  +body = Vd.body(res)
  SW.Output{[Vd.save(a, scope, token, body)], Vd.reply(Vd.find(A.Sidebar.memos(a), scope, base), body, res, base, token, Vd.draft(p))}

# a picture to send (shown and ready), to a client that offered the delta
def Vd.applies(+p: J.Json, +res: J.Json) -> Bool:
  Bool.and(Vd.offered(p), Bool.and(J.Json.get_bool(res, "remoteReady", False{}),
    Bool.and(Maybe.is_some(&2, J.Json, J.Json.get(res, "shown")),
    Bool.and(Bool.not(Maybe.is_some(&2, J.Json, J.Json.get(res, "nextTarget"))),
      W.Wire.positive(W.Wire.nat(p, "requestRevision", 4294967295n))))))

def Vd.checked(ok: Bool, +web: Bool, +actors: List<&2, A.Actor>, +res: J.Json, +p: J.Json) -> SW.Output:
  match ok actors:
    case True{} +a <> Nil{}: Vd.send(web, a, res, p)
    case _ _: SW.Region.output(web, actors, res, p)

# the reply as it goes out: the keyed delta where offered, else the
# sidebar region (thin-sidebar-wire.bend), else as rendered
def Vd.output(+web: Bool, +actors: List<&2, A.Actor>, +res: J.Json, +p: J.Json) -> SW.Output:
  Vd.checked(Vd.applies(p, res), web, actors, res, p)
