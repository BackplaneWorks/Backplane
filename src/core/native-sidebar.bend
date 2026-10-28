import Base
import ./json.bend as J
import ./model.bend as M
import ./client.bend as U
import ./presentation.bend as P
import ./thin-wire.bend as W
import ./sha256.bend as S
import ./utf8.bend as U8

# Opt-in metadata navigation. The key and positions are actor-private;
# Session.info does not adopt or serialize local.* fields. No history body,
# canonical setting, command or cached client state is accepted here.
def Obj.keep(drop: Bool, k: String, v: J.Json, rest: J.Json) -> J.Json:
  match drop:
    case True{}: rest
    case False{}: J.Field{k, v, rest}

def Obj.fields(fs: J.Json, +key: String) -> J.Json:
  match fs:
    case J.Field{+k, v, tail}:
      Obj.keep(String.eq(k, key), k, v, Obj.fields(tail, key))
    case _: J.End{}

def Obj.put(j: J.Json, +key: String, value: J.Json) -> J.Json:
  match j:
    case J.Obj{fs}: J.Obj{J.Field{key, value, Obj.fields(fs, key)}}
    case _: J.Json.obj([J.KV{key, value}])

def Ns.parse(text: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})

def Ns.hash(text: String) -> String:
  S.Hex.encode(S.Sha256.hash(U8.Utf8.encode(text)))

def Ns.ctx(u: U.Ui) -> J.Json:
  Ns.parse(U.Ui.info(u, "local.native.sidebar.context"))

def Ns.enabled(u: U.Ui) -> Bool:
  String.eq(U.Ui.info(u, "local.native.sidebar"), "1")

def Ns.offered(web: Bool, p: J.Json) -> Bool:
  Bool.and(Bool.not(web), W.Wire.version(Maybe.default(&2, J.Json, J.Json.get(p, "nativeSidebar"), J.Null{})))

def Ns.key(u: U.Ui) -> String:
  U.Ui.info(u, "local.native.sidebar.key")

def Ns.key_valid.decoded(bytes: Maybe<&2, List<&2, U32>>) -> Bool:
  match bytes:
    case None{}: False{}
    case Some{bs}: Nat.is_eq(List.length(&2, U32, bs), 16n)

def Ns.key_valid(key: String) -> Bool:
  Ns.key_valid.decoded(S.Hex.decode(key))

def Ns.project_limit(u: U.Ui) -> Nat:
  Nat.max(1n, Nat.min(8n, P.Session.number(Ns.parse(U.Ui.info(u, "local.native.sidebar.budget")), "projects", 8n)))

def Ns.row_budget(u: U.Ui) -> Nat:
  Nat.max(1n, Nat.min(48n, P.Session.number(Ns.parse(U.Ui.info(u, "local.native.sidebar.budget")), "rows", 48n)))

# Server-only projection pressure. No input field can select these budgets;
# the accepted actor retains the fitted capacity so paging stays contiguous.
def Ns.reduce(+u: U.Ui) -> U.Ui:
  U.Ui.info_set(u, "local.native.sidebar.budget", J.Json.show(J.Json.obj([
    J.KV{"projects", J.Json.nat(Nat.max(1n, Nat.div(Ns.project_limit(u), 2n)))},
    J.KV{"rows", J.Json.nat(Nat.max(1n, Nat.div(Ns.row_budget(u), 2n)))}])))

def Ns.mode(+u: U.Ui) -> String:
  +local = U.Ui.scratch(u, "@local.view")
  Bool.pick(String, Bool.or(String.eq(local, "projects"), String.eq(local, "active")), local, U.Side.view(U.Ui.state(u)))

def Ns.choices(+u: U.Ui) -> String:
  Ns.hash(J.Json.show(J.Json.obj([J.KV{"mode", J.Str{Ns.mode(u)}}, J.KV{"query", J.Str{U.Ui.scratch(u, "@f.pq")}},
    J.KV{"open", J.Json.obj(P.Session.flags(Map.to_list(&2, Bool, U.Side.open(u))))}])))

def Ns.same(+a: J.Json, +b: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(a, "hub", ""), J.Json.get_str(b, "hub", "")),
  Bool.and(String.eq(J.Json.get_str(a, "generation", ""), J.Json.get_str(b, "generation", "")),
  Bool.and(String.eq(J.Json.get_str(a, "epoch", ""), J.Json.get_str(b, "epoch", "")),
  Bool.and(String.eq(J.Json.get_str(a, "viewId", ""), J.Json.get_str(b, "viewId", "")), W.Target.same(J.Json.get_obj(a, "target"), J.Json.get_obj(b, "target"))))))

def Ns.context(+p: J.Json, epoch: String, view: String, revision: Nat) -> J.Json:
  J.Json.obj([J.KV{"hub", J.Str{J.Json.get_str(p, "hub", "")}}, J.KV{"generation", J.Str{J.Json.get_str(p, "generation", "")}},
    J.KV{"epoch", J.Str{epoch}}, J.KV{"viewId", J.Str{view}}, J.KV{"target", J.Json.get_obj(p, "target")}, J.KV{"metadataRevision", J.Json.nat(revision)}])

def Ns.retained_key(+u: U.Ui, p: J.Json, epoch: String, view: String) -> String:
  Bool.pick(String, Bool.and(Ns.enabled(u), Ns.same(Ns.ctx(u), Ns.context(p, epoch, view, 0n))), Ns.key(u), "")

def Ns.prepare.on(on: Bool, +u: U.Ui, +ctx: J.Json, key: String) -> U.Ui:
  match on:
    case False{}:
      U.Ui.info_drop(U.Ui.info_drop(U.Ui.info_drop(U.Ui.info_drop(U.Ui.info_drop(u, "local.native.sidebar"), "local.native.sidebar.context"), "local.native.sidebar.key"), "local.native.sidebar.pages"), "local.native.sidebar.budget")
    case True{}:
      U.Ui.info_set(U.Ui.info_set(U.Ui.info_set(U.Ui.info_set(u, "local.native.sidebar.pages",
        Bool.pick(String, Bool.and(Ns.same(Ns.ctx(u), ctx),
          Bool.and(String.eq(J.Json.show(J.Json.get_obj(Ns.ctx(u), "metadataRevision")), J.Json.show(J.Json.get_obj(ctx, "metadataRevision"))),
            String.eq(J.Json.get_str(Ns.ctx(u), "choicesDigest", ""), J.Json.get_str(ctx, "choicesDigest", "")))), U.Ui.info(u, "local.native.sidebar.pages"), "{}")),
        "local.native.sidebar.context", J.Json.show(ctx)), "local.native.sidebar.key", key), "local.native.sidebar", "1")

def Ns.prepare.budget(same: Bool, u: U.Ui) -> U.Ui:
  match same:
    case True{}: u
    case False{}: U.Ui.info_drop(u, "local.native.sidebar.budget")

def Ns.prepare(+u: U.Ui, web: Bool, +p: J.Json, +epoch: String, +view: String, revision: Nat, +key: String) -> U.Ui:
  Ns.prepare.on(Bool.and(Ns.offered(web, p), Ns.key_valid(key)),
    Ns.prepare.budget(Ns.same(Ns.ctx(u), Ns.context(p, epoch, view, 0n)), u),
    Obj.put(Ns.context(p, epoch, view, revision), "choicesDigest", J.Str{Ns.choices(u)}), key)

def Ns.ids.json(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case x <> rest: J.Str{x} <> Ns.ids.json(rest)

def Ns.member(ids: List<&2, String>) -> String:
  Ns.hash(J.Json.show(J.Json.arr(Ns.ids.json(ids))))

def Ns.scope(kind: String, owner: String) -> String:
  Ns.hash(J.Json.show(J.Json.obj([J.KV{"kind", J.Str{kind}}, J.KV{"owner", J.Str{owner}}])))

def Ns.payload(+u: U.Ui, kind: String, owner: String, member: String, offset: Nat, limit: Nat) -> J.Json:
  +ctx = Ns.ctx(u)
  J.Json.obj([J.KV{"version", J.Num{"1"}}, J.KV{"hub", J.Str{J.Json.get_str(ctx, "hub", "")}},
    J.KV{"generation", J.Str{J.Json.get_str(ctx, "generation", "")}}, J.KV{"epoch", J.Str{J.Json.get_str(ctx, "epoch", "")}},
    J.KV{"viewId", J.Str{J.Json.get_str(ctx, "viewId", "")}}, J.KV{"target", J.Json.get_obj(ctx, "target")},
    J.KV{"metadataRevision", J.Json.get_obj(ctx, "metadataRevision")}, J.KV{"listKind", J.Str{kind}}, J.KV{"projectOwner", J.Str{owner}},
    J.KV{"choicesDigest", J.Str{Ns.choices(u)}}, J.KV{"membershipDigest", J.Str{member}}, J.KV{"offset", J.Json.nat(offset)}, J.KV{"limit", J.Json.nat(limit)}])

def Ns.mac(key: String, payload: J.Json) -> String:
  S.Hex.encode(S.Hmac.sha256(Maybe.default(&2, List<&2, U32>, S.Hex.decode(key), Nil{}),
    U8.Utf8.encode("backplane.native-sidebar.v1\n" ++ J.Json.show(payload))))

# The opaque token omits duplicate private context. The MAC still covers
# the exact full canonical payload above. Owner hashes resolve only against
# this actor's canonical project metadata, never client-supplied owner text.
def Ns.context_digest(+u: U.Ui) -> String:
  +c = Ns.ctx(u)
  Ns.hash("backplane.native-sidebar.context.v1\n" ++ J.Json.show(J.Json.obj([
    J.KV{"hub", J.Json.get_obj(c, "hub")}, J.KV{"generation", J.Json.get_obj(c, "generation")},
    J.KV{"epoch", J.Json.get_obj(c, "epoch")}, J.KV{"viewId", J.Json.get_obj(c, "viewId")}, J.KV{"target", J.Json.get_obj(c, "target")}])) )

def Ns.owner_digest(owner: String) -> String:
  Ns.hash("backplane.native-sidebar.owner.v1\n" ++ owner)

type Owner is Data:
  Owner{count: Nat, value: String}

def Ns.owner.step(found: Bool, +id: String, rest: Owner) -> Owner:
  match found rest:
    case True{} Owner{n, _}: Owner{Nat.add(n, 1n), id}
    case False{} r: r

def Ns.owner.projects(ps: List<&2, M.Project>, +digest: String) -> Owner:
  match ps:
    case Nil{}: Owner{0n, ""}
    case M.Project{+id, _, _, _, _} <> rest:
      Ns.owner.step(String.eq(Ns.owner_digest(id), digest), id, Ns.owner.projects(rest, digest))

def Ns.owner.global(global: Bool, +u: U.Ui, digest: String) -> Owner:
  match global:
    case True{}: Owner{1n, ""}
    case False{}: Ns.owner.projects(U.Ui.projects(U.Ui.state(u)), digest)

def Ns.owner(+u: U.Ui, +digest: String) -> Owner:
  Ns.owner.global(String.eq(digest, Ns.owner_digest("")), u, digest)

def Ns.expand.owner(owner: Owner, +u: U.Ui, +p: J.Json) -> J.Json:
  match owner:
    case Owner{1n, value}:
      +ctx = Ns.ctx(u)
      J.Json.obj([J.KV{"version", J.Json.get_obj(p, "version")}, J.KV{"hub", J.Json.get_obj(ctx, "hub")},
        J.KV{"generation", J.Json.get_obj(ctx, "generation")}, J.KV{"epoch", J.Json.get_obj(ctx, "epoch")},
        J.KV{"viewId", J.Json.get_obj(ctx, "viewId")}, J.KV{"target", J.Json.get_obj(ctx, "target")},
        J.KV{"metadataRevision", J.Json.get_obj(p, "metadataRevision")}, J.KV{"listKind", J.Json.get_obj(p, "listKind")},
        J.KV{"projectOwner", J.Str{value}}, J.KV{"choicesDigest", J.Json.get_obj(p, "choicesDigest")},
        J.KV{"membershipDigest", J.Json.get_obj(p, "membershipDigest")}, J.KV{"offset", J.Json.get_obj(p, "offset")}, J.KV{"limit", J.Json.get_obj(p, "limit")}])
    case _: J.Null{}

def Ns.expand(+u: U.Ui, +p: J.Json) -> J.Json:
  Ns.expand.owner(Ns.owner(u, J.Json.get_str(p, "ownerDigest", "")), u, p)

def Ns.token(+u: U.Ui, +payload: J.Json) -> String:
  J.Json.show(J.Json.obj([J.KV{"payload", J.Json.obj([
    J.KV{"version", J.Json.get_obj(payload, "version")}, J.KV{"contextDigest", J.Str{Ns.context_digest(u)}},
    J.KV{"metadataRevision", J.Json.get_obj(payload, "metadataRevision")}, J.KV{"listKind", J.Json.get_obj(payload, "listKind")},
    J.KV{"ownerDigest", J.Str{Ns.owner_digest(J.Json.get_str(payload, "projectOwner", ""))}},
    J.KV{"choicesDigest", J.Json.get_obj(payload, "choicesDigest")}, J.KV{"membershipDigest", J.Json.get_obj(payload, "membershipDigest")},
    J.KV{"offset", J.Json.get_obj(payload, "offset")}, J.KV{"limit", J.Json.get_obj(payload, "limit")}])},
    J.KV{"signature", J.Str{Ns.mac(Ns.key(u), payload)}}]))

def Ns.control.valid(show: Bool, +u: U.Ui, kind: String, owner: String, member: String, offset: Nat, limit: Nat, label: String) -> J.Json:
  match show:
    case False{}: J.Null{}
    case True{}: J.Json.obj([J.KV{"label", J.Str{label}}, J.KV{"action", J.Str{"native-sidebar-page"}}, J.KV{"value", J.Str{Ns.token(u, Ns.payload(u, kind, owner, member, offset, limit))}}])

def Ns.control(show: Bool, +u: U.Ui, kind: String, owner: String, member: String, offset: Nat, limit: Nat, label: String) -> J.Json:
  Ns.control.valid(Bool.and(show, Ns.key_valid(Ns.key(u))), u, kind, owner, member, offset, limit, label)

def Ns.has(ids: List<&2, String>, +id: String) -> Bool:
  match ids:
    case Nil{}: False{}
    case x <> rest: Bool.or(String.eq(x, id), Ns.has(rest, id))

def Ns.position(+u: U.Ui, kind: String, owner: String, +member: String, +limit: Nat, +total: Nat) -> Nat:
  +saved = J.Json.get_obj(Ns.parse(U.Ui.info(u, "local.native.sidebar.pages")), Ns.scope(kind, owner))
  +n = P.Session.number(saved, "offset", 0n)
  Bool.pick(Nat, Bool.and(Nat.is_lt(n, total), Bool.and(Nat.is_eq(P.Session.number(saved, "limit", 0n), limit),
    Bool.and(String.eq(J.Json.get_str(saved, "membershipDigest", ""), member),
    Bool.and(String.eq(J.Json.get_str(saved, "choicesDigest", ""), Ns.choices(u)),
      String.eq(J.Json.show(J.Json.get_obj(saved, "metadataRevision")), J.Json.show(J.Json.get_obj(Ns.ctx(u), "metadataRevision"))))))), n, 0n)

type Slice is Data:
  Slice{ids: List<&2, String>, pager: J.Json}

def Slice.ids(s: Slice) -> List<&2, String>:
  match s:
    case Slice{ids, _}: ids

def Slice.pager(s: Slice) -> J.Json:
  match s:
    case Slice{_, pager}: pager

def Ns.slice.empty(empty: Bool, +u: U.Ui, +ids: List<&2, String>, +kind: String, +owner: String, +limit: Nat, +selected: String) -> Slice:
  match empty:
    case True{}: Slice{Nil{}, J.Null{}}
    case False{}:
      +total = List.length(&2, String, ids)
      +member = Ns.member(ids)
      +offset = Ns.position(u, kind, owner, member, limit, total)
      +ordinary = List.take(&2, String, List.drop(&2, String, ids, offset), limit)
      +extra = Bool.and(Bool.not(String.is_empty(selected)), Bool.and(Ns.has(ids, selected), Bool.not(Ns.has(ordinary, selected))))
      +next = Nat.add(offset, List.length(&2, String, ordinary))
      Slice{List.append(&2, String, ordinary, Bool.pick(List<&2, String>, extra, [selected], Nil{})),
        J.Json.obj([J.KV{"version", J.Num{"1"}}, J.KV{"total", J.Json.nat(total)}, J.KV{"offset", J.Json.nat(offset)}, J.KV{"limit", J.Json.nat(limit)},
          J.KV{"selectedOutside", J.Flag{extra}},
          J.KV{"previous", Ns.control(Nat.is_gt(offset, 0n), u, kind, owner, member, Nat.sub(offset, limit), limit, "Previous")},
          J.KV{"next", Ns.control(Nat.is_lt(next, total), u, kind, owner, member, next, limit, "Next · " ++ Nat.show(total) ++ " total")}])}

def Ns.slice(u: U.Ui, +ids: List<&2, String>, kind: String, owner: String, +limit: Nat, selected: String) -> Slice:
  Ns.slice.empty(List.is_empty(&2, String, ids), u, ids, kind, owner, Bool.pick(Nat, Nat.is_eq(limit, 0n), 1n, Nat.min(48n, limit)), selected)

# The owner prefix is introduced once by native Q.all. No UI-provided
# unqualified value or token from another paired hub can route this action.
def Ns.raw(+u: U.Ui, +value: String) -> String:
  +prefix = J.Json.get_str(Ns.ctx(u), "hub", "") ++ "|"
  Bool.pick(String, String.starts_with(value, prefix), String.drop(value, String.length(prefix)), "")

def Ns.valid.payload(+u: U.Ui, +token: J.Json) -> Bool:
  +compact = J.Json.get_obj(token, "payload")
  +p = Ns.expand(u, compact)
  Bool.and(String.eq(J.Json.get_str(compact, "contextDigest", ""), Ns.context_digest(u)), Bool.and(Ns.enabled(u), Bool.and(Ns.key_valid(Ns.key(u)),
  Bool.and(W.Wire.version(Maybe.default(&2, J.Json, J.Json.get(p, "version"), J.Null{})),
  Bool.and(Ns.same(p, Ns.ctx(u)), Bool.and(Maybe.is_some(&2, Nat, W.Wire.nat(p, "offset", P.Session.nat.max())),
  Bool.and(W.Wire.positive(W.Wire.nat(p, "limit", 48n)),
    S.Ct.eq(U8.Utf8.encode(J.Json.get_str(token, "signature", "")), U8.Utf8.encode(Ns.mac(Ns.key(u), p))))))))))

def Ns.valid.short(short: Bool, u: U.Ui, raw: String) -> Bool:
  match short:
    case False{}: False{}
    case True{}: Ns.valid.payload(u, Ns.parse(raw))

def Ns.valid(+u: U.Ui, +value: String) -> Bool:
  +raw = Ns.raw(u, value)
  Ns.valid.short(Bool.and(Bool.not(String.is_empty(raw)), W.Wire.within(raw, 2048n)), u, raw)

def Ns.count(j: J.Json) -> Nat:
  match j:
    case J.Obj{fs}: Ns.count(fs)
    case J.Field{_, _, tail}: Nat.add(1n, Ns.count(tail))
    case _: 0n

def Ns.store(+p: J.Json) -> J.Json:
  J.Json.obj([J.KV{"offset", J.Json.get_obj(p, "offset")}, J.KV{"limit", J.Json.get_obj(p, "limit")},
    J.KV{"metadataRevision", J.Json.get_obj(p, "metadataRevision")}, J.KV{"choicesDigest", J.Json.get_obj(p, "choicesDigest")},
    J.KV{"membershipDigest", J.Json.get_obj(p, "membershipDigest")}])

def Ns.accept.source(fresh: Bool, +u: U.Ui, +p: J.Json) -> U.Ui:
  match fresh:
    case False{}: U.Ui.info_set(u, "local.native.sidebar.pages", "{}")
    case True{}:
      +old = Ns.parse(U.Ui.info(u, "local.native.sidebar.pages"))
      U.Ui.info_set(u, "local.native.sidebar.pages", J.Json.show(Obj.put(
        Bool.pick(J.Json, Nat.is_lt(Ns.count(old), 64n), old, J.Json.obj(Nil{})),
        Ns.scope(J.Json.get_str(p, "listKind", ""), J.Json.get_str(p, "projectOwner", "")), Ns.store(p))))

def Ns.accept.valid(valid: Bool, +u: U.Ui, value: String) -> U.Ui:
  match valid:
    case False{}: u
    case True{}:
      +p = Ns.expand(u, J.Json.get_obj(Ns.parse(Ns.raw(u, value)), "payload"))
      Ns.accept.source(Bool.and(String.eq(J.Json.show(J.Json.get_obj(p, "metadataRevision")), J.Json.show(J.Json.get_obj(Ns.ctx(u), "metadataRevision"))),
        String.eq(J.Json.get_str(p, "choicesDigest", ""), Ns.choices(u))), u, p)

def Ns.accept(+u: U.Ui, +value: String) -> U.Ui:
  Ns.accept.valid(Ns.valid(u, value), u, value)

# Rebuild private positions from the projection that actually survives the
# final byte gate. Hidden/removed lists and changed membership/limits retire
# their saved positions permanently, even if an old list later reappears.
def Ns.control.unqualify(s: String) -> String:
  match s:
    case SNil{}: ""
    case SCon{Chr{124}, rest}: rest
    case SCon{_, rest}: Ns.control.unqualify(rest)

def Ns.control.payload(+u: U.Ui, +value: String) -> J.Json:
  Ns.expand(u, J.Json.get_obj(Ns.parse(Bool.pick(String, String.starts_with(value, "{"), value, Ns.control.unqualify(value))), "payload"))

def Ns.reconcile.payload(+pager: J.Json, pages: J.Json, +payload: J.Json) -> J.Json:
  match payload:
    case J.Obj{_}:
      Obj.put(pages, Ns.scope(J.Json.get_str(payload, "listKind", ""), J.Json.get_str(payload, "projectOwner", "")),
        Ns.store(Obj.put(Obj.put(payload, "offset", J.Json.get_obj(pager, "offset")), "limit", J.Json.get_obj(pager, "limit"))))
    case _: pages

def Ns.reconcile.pager(+u: U.Ui, +pager: J.Json, pages: J.Json) -> J.Json:
  +control = Bool.pick(J.Json, Nat.is_gt(J.Json.get_nat(pager, "offset", 0n), 0n), J.Json.get_obj(pager, "previous"), J.Json.get_obj(pager, "next"))
  Ns.reconcile.payload(pager, pages, Ns.control.payload(u, J.Json.get_str(control, "value", "")))

def Ns.reconcile.projects(+u: U.Ui, xs: List<&2, J.Json>, pages: J.Json) -> J.Json:
  match xs:
    case Nil{}: pages
    case +p <> rest:
      Ns.reconcile.projects(u, rest, Ns.reconcile.pager(u, J.Json.get_obj(p, "threadsPage"),
        Ns.reconcile.pager(u, J.Json.get_obj(p, "snoozedPage"), Ns.reconcile.pager(u, J.Json.get_obj(p, "settledPage"),
          Ns.reconcile.pager(u, J.Json.get_obj(p, "archivedPage"), pages)))))

def Ns.reconcile.descriptors(+u: U.Ui, xs: List<&2, J.Json>, pages: J.Json) -> J.Json:
  match xs:
    case Nil{}: pages
    case +p <> rest:
      Ns.reconcile.descriptors(u, rest, Ns.reconcile.pager(u, J.Json.get_obj(p, "projects"),
        Ns.reconcile.pager(u, J.Json.get_obj(p, "active"), Ns.reconcile.pager(u, J.Json.get_obj(p, "legacy"), pages))))

def Ns.reconcile.on(on: Bool, +u: U.Ui, +screen: J.Json) -> U.Ui:
  match on:
    case False{}: u
    case True{}:
      U.Ui.info_set(u, "local.native.sidebar.pages", J.Json.show(Ns.reconcile.projects(u, J.Json.elems(J.Json.get_obj(screen, "projects")),
        Ns.reconcile.descriptors(u, J.Json.elems(J.Json.get_obj(screen, "sidebarPages")), J.Json.obj(Nil{})))))

def Ns.reconcile(+u: U.Ui, +screen: J.Json) -> U.Ui:
  Ns.reconcile.on(Bool.and(Ns.enabled(u), Nat.is_eq(J.Json.get_nat(screen, "nativeSidebar", 0n), 1n)), u, screen)

def Ns.memo_key(+u: U.Ui) -> String:
  Ns.hash(J.Json.show(J.Json.obj([J.KV{"enabled", J.Flag{Ns.enabled(u)}}, J.KV{"context", Ns.ctx(u)},
    J.KV{"nonceDigest", J.Str{Ns.hash(Ns.key(u))}}, J.KV{"choices", J.Str{Ns.choices(u)}},
    J.KV{"budget", Ns.parse(U.Ui.info(u, "local.native.sidebar.budget"))},
    J.KV{"pages", Ns.parse(U.Ui.info(u, "local.native.sidebar.pages"))}])))
