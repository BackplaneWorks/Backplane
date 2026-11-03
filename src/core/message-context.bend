import Base
import ./model.bend as M
import ./fold.bend as F

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
