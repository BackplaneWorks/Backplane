import Base
import ./model.bend as M
import ./fold.bend as F

def Context.key(+thread: String) -> String:
  "@message.context:" ++ thread

def Context.runs.key(+thread: String) -> String:
  "@message.runs:" ++ thread

def Context.root.get(r: Map<&2, String> & String) -> String:
  (_, id) = r
  id

def Context.roots(es: List<&2, M.Entry>, acc: Map<&2, String>) -> Map<&2, String>:
  match es:
    case Nil{}: acc
    case M.Act{id, _, _, head, _, _} <> rest: Context.roots(rest, Map.set(&2, String, acc, id, head))
    case _ <> rest: Context.roots(rest, acc)

def Context.role(+e: M.Entry, r: F.Role, roots: Map<&2, String>) -> F.Role:
  match r:
    case F.RHead{+id, label, open, live}:
      F.RHead{Context.root.get(Map.get(String, id, roots, M.Idx.id(e))), label, open, live}
    case other: other

def Context.role.open(+e: M.Entry, r: F.Role, +info: Map<&2, String>, open: Bool) -> F.Role:
  match r:
    case F.RHead{+id, label, _, +live}:
      F.RHead{id, label, F.Fold.run.open(F.Fold.user(info, id), live), live}
    case F.RIn{was}: F.RIn{Bool.pick(Bool, F.Fold.blank(e), was, open)}
    case other: other

def Context.open(r: F.Role, previous: Bool) -> Bool:
  match r:
    case F.RHead{_, _, open, _}: open
    case F.RIn{_}: previous
    case F.RWork{_, _, _, inner}: Context.open(inner, previous)
    case F.RShow{}: False{}

def Context.roles.go(es: List<&2, M.Entry>, rs: List<&2, F.Role>, +roots: Map<&2, String>, +info: Map<&2, String>, +open: Bool) -> List<&2, F.Role>:
  match es rs:
    case +e <> tail r <> rest:
      +role = Context.role.open(e, Context.role(e, r, roots), info, open)
      role <> Context.roles.go(tail, rest, roots, info, Context.open(role, open))
    case _ _: Nil{}

# The same canonical run identity drives rendering, body admission and
# retention. An off-page call cannot manufacture a second drawer.
def Context.roles(+es: List<&2, M.Entry>, rs: List<&2, F.Role>, +st: M.State, +thread: String, info: Map<&2, String>) -> List<&2, F.Role>:
  Context.roles.go(es, rs, Context.roots(M.State.mine(st, Context.runs.key(thread)), Map.new(&2, String)), info, False{})

# Display identities are separate from immutable source ordinals. A closed
# tool run spends one retained row even when it crosses many source pages.
def Context.unit(e: M.Entry, r: F.Role) -> List<&2, String>:
  match r:
    case F.RShow{}: [M.Idx.id(e)]
    case F.RHead{+id, _, open, _}:
      Bool.pick(List<&2, String>, open, ["fold:" ++ id, M.Idx.id(e)], ["fold:" ++ id])
    case F.RIn{open}: Bool.pick(List<&2, String>, open, [M.Idx.id(e)], Nil{})
    case F.RWork{_, _, _, inner}: Context.unit(e, inner)

def Context.units(es: List<&2, M.Entry>, rs: List<&2, F.Role>) -> List<&2, String>:
  match es rs:
    case e <> tail r <> rest: List.append(&2, String, Context.unit(e, r), Context.units(tail, rest))
    case _ _: Nil{}

def Context.units.stored(es: List<&2, M.Entry>, rs: List<&2, F.Role>, +roots: Map<&2, String>) -> List<&2, String>:
  match es rs:
    case +e <> tail r <> rest: List.append(&2, String, Context.unit(e, Context.role(e, r, roots)), Context.units.stored(tail, rest, roots))
    case _ _: Nil{}

def Context.message(e: M.Entry) -> Bool:
  match e:
    case M.Msg{_, _, M.User{}, _, _}: True{}
    case M.Msg{_, _, M.Assistant{}, _, _}: True{}
    case _: False{}

def Context.add(keep: Bool, id: String, rest: List<&2, String>) -> List<&2, String>:
  match keep:
    case True{}: id <> rest
    case False{}: rest

def Context.messages(es: List<&2, M.Entry>) -> List<&2, String>:
  match es:
    case Nil{}: Nil{}
    case +e <> tail: Context.add(Bool.and(Context.message(e), Bool.not(F.Fold.blank(e))), M.Idx.id(e), Context.messages(tail))

# The latest human prompt and assistant text provide context independently
# of how much tool activity followed them. Stop once both have been found.
def Context.latest(es: List<&2, M.Entry>, +human: Bool, +agent: Bool) -> List<&2, String>:
  match es human agent:
    case _ True{} True{}: Nil{}
    case Nil{} _ _: Nil{}
    case M.Msg{+id, _, M.User{}, _, _} <> tail False{} _:
      id <> Context.latest(tail, True{}, agent)
    case M.Msg{+id, _, M.Assistant{}, +text, +at} <> tail _ False{}:
      Context.add(Bool.not(F.Fold.blank(M.Msg{id, "", M.Assistant{}, text, at})), id,
        Context.latest(tail, human, Bool.not(F.Fold.blank(M.Msg{id, "", M.Assistant{}, text, at}))))
    case _ <> tail _ _: Context.latest(tail, human, agent)

def Context.targets.pick(empty: Bool, stored: List<&2, M.Entry>, source: List<&2, M.Entry>) -> List<&2, String>:
  match empty:
    case True{}: Context.latest(source, False{}, False{})
    case False{}: Context.latest(stored, False{}, False{})

def Context.targets(+st: M.State, +thread: String) -> List<&2, String>:
  +stored = M.State.mine(st, Context.key(thread))
  Context.targets.pick(List.is_empty(&2, M.Entry, stored), stored, M.State.mine(st, thread))
