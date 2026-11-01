import Base
import ../src/core/json.bend as J
import ../src/server/thin-delta.bend as D

# Driver: test/tools/view_delta.mjs (the client's applier against these)

def parse(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def diff.k(d: Maybe<&2, List<&2, D.Op>>) -> String:
  match d:
    case Some{+ops}: J.Json.show(J.Json.arr(D.Delta.json(ops, "")))
    case None{}: "null"

# the ops from old to new, as the wire carries them
def diff(old: String, new: String) -> String:
  diff.k(D.Delta.diff(parse(old), parse(new)))

def diffd.k(d: Maybe<&2, List<&2, D.Op>>, +draft: String) -> String:
  match d:
    case Some{+ops}: J.Json.show(J.Json.arr(D.Delta.json(ops, draft)))
    case None{}: "null"

# the same, with the request's draft sent as ["t", path]
def diffd(old: String, new: String, draft: String) -> String:
  diffd.k(D.Delta.diff(parse(old), parse(new)), draft)

# new, rebuilt by the Bend applier from old and the diff
def round(old: String, new: String) -> String:
  J.Json.show(D.Delta.round(parse(old), parse(new)))
