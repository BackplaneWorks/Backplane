import Base
import ./json.bend as J
import ./model.bend as M

# Accounts are settings, not a change to Thread/State or the phone kept-state
# types. Empty bindings preserve every legacy session key and launch behavior.
def Accounts.parse(text: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})

def Accounts.supported(+provider: String) -> Bool:
  String.eq(provider, "claude") || String.eq(provider, "codex")

def Accounts.inventory(s: M.State) -> J.Json:
  Accounts.parse(M.Setting.get(s, "accounts.inventory", ""))

def Accounts.profiles(s: M.State) -> List<&2, J.Json>:
  J.Json.elems(J.Json.get_obj(Accounts.inventory(s), "profiles"))

def Accounts.threads(s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}: Map.values(&2, M.Thread, ts)

def Accounts.find(xs: List<&2, J.Json>, +id: String) -> J.Json:
  match xs:
    case Nil{}: J.Null{}
    case +h <> t:
      Bool.pick(J.Json, String.eq(J.Json.get_str(h, "id", ""), id), h, Accounts.find(t, id))

def Accounts.profile(+s: M.State, id: String) -> J.Json:
  Accounts.find(Accounts.profiles(s), id)

def Accounts.binding.key(+th: String, +provider: String) -> String:
  "account.thread." ++ th ++ "." ++ provider

def Accounts.binding.raw(+raw: String) -> J.Json:
  Bool.pick(J.Json, String.is_empty(raw), J.Json.obj([J.KV{"id", J.Str{""}}, J.KV{"epoch", J.Json.nat(0n)}]), Accounts.parse(raw))

def Accounts.binding(s: M.State, +th: String, +provider: String) -> J.Json:
  Accounts.binding.raw(M.Setting.get(s, Accounts.binding.key(th, provider), ""))

def Accounts.id(+s: M.State, +th: String, +provider: String) -> String:
  J.Json.get_str(Accounts.binding(s, th, provider), "id", "!invalid")

def Accounts.epoch(+s: M.State, +th: String, +provider: String) -> Nat:
  J.Json.get_nat(Accounts.binding(s, th, provider), "epoch", 0n)

def Accounts.binding.show(id: String, epoch: Nat) -> String:
  J.Json.show(J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"epoch", J.Json.nat(epoch)}]))

def Accounts.label(+s: M.State, +th: String, +provider: String) -> String:
  Bool.pick(String, String.is_empty(Accounts.id(s, th, provider)), "Default account", J.Json.get_str(Accounts.profile(s, Accounts.id(s, th, provider)), "label", "Unavailable account"))

def Accounts.ready(+profile: J.Json, +provider: String, +epoch: Nat) -> Bool:
  Bool.and(String.eq(J.Json.get_str(profile, "provider", ""), provider),
    Bool.and(String.eq(J.Json.get_str(profile, "state", ""), "connected"), Nat.is_eq(J.Json.get_nat(profile, "epoch", 0n), epoch)))

def Accounts.allowed.k(legacy: Bool, +s: M.State, +th: String, +provider: String) -> Bool:
  match legacy:
    case True{}: True{}
    case False{}: Accounts.ready(Accounts.profile(s, Accounts.id(s, th, provider)), provider, Accounts.epoch(s, th, provider))

def Accounts.allowed(+s: M.State, +th: String, +provider: String) -> Bool:
  Accounts.allowed.k(String.is_empty(Accounts.id(s, th, provider)), s, th, provider)

# Tuple components have separate length prefixes; labels never enter a key.
def Accounts.part(+s: String) -> String:
  Nat.show(String.length(s)) ++ ":" ++ s

def Accounts.session.key(+th: String, +provider: String, +id: String, epoch: Nat) -> String:
  "account.session." ++ Accounts.part(th) ++ Accounts.part(provider) ++ Accounts.part(id) ++ Nat.show(epoch)

def Accounts.seen.key(+th: String, +provider: String, +id: String, epoch: Nat) -> String:
  "account.seen." ++ Accounts.part(th) ++ Accounts.part(provider) ++ Accounts.part(id) ++ Nat.show(epoch)

def Accounts.session(s: M.State, th: String, provider: String, id: String, epoch: Nat) -> String:
  M.Setting.get(s, Accounts.session.key(th, provider, id, epoch), "")

def Accounts.seen(s: M.State, th: String, provider: String, id: String, epoch: Nat) -> Nat:
  M.Switch.num(M.Setting.get(s, Accounts.seen.key(th, provider, id, epoch), "0"), 0n)

def Accounts.select(+s: M.State, +now: Nat, +th: String, +provider: String, +id: String, +epoch: Nat, +session: String, +count: Nat) -> List<&2, M.Change>:
  [M.SettingSet{Accounts.session.key(th, provider, Accounts.id(s, th, provider), Accounts.epoch(s, th, provider)), session, now},
   M.SettingSet{Accounts.seen.key(th, provider, Accounts.id(s, th, provider), Accounts.epoch(s, th, provider)), Nat.show(count), now},
   M.SettingSet{Accounts.binding.key(th, provider), Accounts.binding.show(id, epoch), now},
   M.SettingSet{M.Switch.session.key(th, provider), Accounts.session(s, th, provider, id, epoch), now},
   M.SettingSet{M.Switch.seen.key(th, provider), Nat.show(Accounts.seen(s, th, provider, id, epoch)), now},
   M.SessionSet{th, Accounts.session(s, th, provider, id, epoch), now}]

# The IO snapshot's attempts (URLs / device codes) are transient info only.
def Accounts.public(+snapshot: J.Json) -> String:
  J.Json.show(J.Json.obj([J.KV{"version", J.Json.nat(1n)}, J.KV{"revision", J.Json.nat(J.Json.get_nat(snapshot, "revision", 0n))}, J.KV{"profiles", J.Json.get_obj(snapshot, "profiles")}, J.KV{"error", J.Str{J.Json.get_str(snapshot, "error", "")}}]))

def Accounts.changes(+s: M.State, +now: Nat, snapshot: String) -> List<&2, M.Change>:
  +value = Accounts.public(Accounts.parse(snapshot))
  Bool.pick(List<&2, M.Change>, String.eq(M.Setting.get(s, "accounts.inventory", ""), value), Nil{}, [M.SettingSet{"accounts.inventory", value, now}])

# Freeze a default at creation. Old threads with no binding remain legacy.
def Accounts.created.one(change: M.Change, +s: M.State) -> List<&2, M.Change>:
  match change:
    case M.ThreadCreated{+id, _, _, _, _, _, +provider, _, +at}:
      Bool.pick(List<&2, M.Change>, Accounts.supported(provider),
        [M.SettingSet{Accounts.binding.key(id, provider), M.Setting.get(s, "account.default." ++ provider, Accounts.binding.show("", 0n)), at}], Nil{})
    case M.ThreadForked{+id, +source, +at}:
      [M.SettingSet{Accounts.binding.key(id, "claude"), M.Setting.get(s, Accounts.binding.key(source, "claude"), Accounts.binding.show("", 0n)), at},
       M.SettingSet{Accounts.binding.key(id, "codex"), M.Setting.get(s, Accounts.binding.key(source, "codex"), Accounts.binding.show("", 0n)), at}]
    case _: Nil{}

def Accounts.created(cs: List<&2, M.Change>, +s: M.State) -> List<&2, M.Change>:
  match cs:
    case Nil{}: Nil{}
    case +c <> t: c <> List.append(&2, M.Change, Accounts.created.one(c, s), Accounts.created(t, s))

def Accounts.catalog(+s: M.State, +th: String, +provider: String) -> List<&2, J.Json>:
  Bool.pick(List<&2, J.Json>, Accounts.allowed(s, th, provider), J.Json.elems(J.Json.get_obj(Accounts.profile(s, Accounts.id(s, th, provider)), "catalog")), Nil{})

def Accounts.models(xs: List<&2, J.Json>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case +h <> t: J.Str{J.Json.get_str(h, "id", "")} <> Accounts.models(t)

# Replace one JSON field without duplicate keys (both parsers see the same value).
def Accounts.without.fields(fs: J.Json, +key: String) -> List<&2, J.KV>:
  match fs:
    case J.Field{+k, v, +t}:
      Bool.pick(List<&2, J.KV>, String.eq(k, key), Accounts.without.fields(t, key), J.KV{k, v} <> Accounts.without.fields(t, key))
    case _: Nil{}

def Accounts.without(o: J.Json, key: String) -> List<&2, J.KV>:
  match o:
    case J.Obj{fs}: Accounts.without.fields(fs, key)
    case _: Nil{}

def Accounts.put(+o: J.Json, +key: String, value: J.Json) -> J.Json:
  J.Json.obj(J.KV{key, value} <> Accounts.without(o, key))

def Accounts.efforts(xs: List<&2, J.Json>) -> List<&2, J.KV>:
  match xs:
    case Nil{}: Nil{}
    case +p <> t: J.KV{J.Json.get_str(p, "id", ""), J.Json.get_obj(p, "efforts")} <> Accounts.efforts(t)

def Accounts.labels(xs: List<&2, J.Json>) -> List<&2, J.KV>:
  match xs:
    case Nil{}: Nil{}
    case +p <> t: J.KV{J.Json.get_str(p, "id", ""), J.Str{J.Json.get_str(p, "label", "")}} <> Accounts.labels(t)

def Accounts.overlay.k(legacy: Bool, +original: J.Json, +s: M.State, +th: String, +provider: String) -> J.Json:
  match legacy:
    case True{}: original
    case False{}:
      +catalog = Accounts.catalog(s, th, provider)
      +about = J.Json.get_obj(original, "about")
      Accounts.put(Accounts.put(original, provider, J.Json.arr(Accounts.models(catalog))), "about",
        Accounts.put(about, provider, Accounts.put(Accounts.put(J.Json.get_obj(about, provider), "efforts", J.Json.obj(Accounts.efforts(catalog))), "labels", J.Json.obj(Accounts.labels(catalog)))))

def Accounts.overlay(+original: String, +s: M.State, +th: String) -> String:
  J.Json.show(Accounts.overlay.k(String.is_empty(Accounts.id(s, th, "claude")),
    Accounts.overlay.k(String.is_empty(Accounts.id(s, th, "codex")), Accounts.parse(original), s, th, "codex"), s, th, "claude"))

def Accounts.str.result(r: Map<&2, String> & String) -> String:
  (_, s) = r
  s

def Accounts.nf(+nf: Map<&2, String>, +s: M.State, +th: String) -> Map<&2, String>:
  Map.set(&2, String, nf, "models", Accounts.overlay(Accounts.str.result(Map.get(String, "", nf, "models")), s, th))

# Compatibility is not authority: named bindings still resolve from state.
def Accounts.named(+s: M.State, +th: String) -> Bool:
  Bool.not(String.is_empty(Accounts.id(s, th, M.Thread.provider(M.State.thread.get(s, th)))))

def Accounts.read.action(+a: String) -> Bool:
  Bool.or(String.eq(a, "draft"), Bool.or(String.eq(a, "bfield"), Bool.or(String.eq(a, "fold"),
    Bool.or(String.eq(a, "thread"), Bool.or(String.eq(a, "sidebar"), Bool.or(String.starts_with(a, "view-"), String.starts_with(a, "sch-")))))))

def Accounts.client.allowed(supported: Bool, +s: M.State, +o: J.Json, +target: String) -> Bool:
  +m = J.Json.get_str(o, "m", "")
  +p = J.Json.get_obj(o, "p")
  Bool.or(supported, Bool.and(Bool.not(String.starts_with(m, "accounts")),
    Bool.and(Bool.not(Bool.and(String.eq(m, "setting.set"), String.starts_with(J.Json.get_str(p, "key", ""), "account."))),
      Bool.or(Bool.not(Accounts.named(s, J.Json.get_str(p, "thread", target))),
        Bool.or(Bool.and(String.starts_with(m, "view."), Bool.or(Bool.not(String.eq(m, "view.action")), Accounts.read.action(J.Json.get_str(p, "action", "")))),
          Bool.or(String.eq(m, "turn.interrupt"), String.starts_with(m, "file.")))))))


# A presented model/action is valid only for the account generation it showed.
def Accounts.owner(+s: M.State, +th: String) -> String:
  M.Setting.get(s, Accounts.binding.key(th, M.Thread.provider(M.State.thread.get(s, th))), "")

def Accounts.request.current(+s: M.State, +o: J.Json, +target: String) -> Bool:
  +p = J.Json.get_obj(o, "p")
  +m = J.Json.get_str(o, "m", "")
  # Only a view action inherits the selected thread. Global Settings/account
  # requests have no conversation owner; explicit thread requests stay fenced.
  +th = Bool.pick(String, String.eq(m, "view.action"), target, J.Json.get_str(p, "thread", ""))
  Bool.or(Bool.not(Accounts.named(s, th)), Bool.or(String.eq(m, "accounts.select"),
    Bool.or(Bool.and(String.starts_with(m, "view."), Bool.not(String.eq(m, "view.action"))),
      Bool.or(String.eq(m, "turn.interrupt"), String.eq(Accounts.owner(s, th), J.Json.get_str(p, "accountBinding", ""))))))

def Accounts.rpc.params(+s: M.State, +ps: List<&2, J.KV>) -> List<&2, J.KV>:
  +th = J.Json.get_str(J.Json.obj(ps), "thread", "")
  Bool.pick(List<&2, J.KV>, Accounts.named(s, th),
    J.KV{"accountBinding", J.Str{Accounts.owner(s, th)}} <> Accounts.without(J.Json.obj(ps), "accountBinding"), ps)
