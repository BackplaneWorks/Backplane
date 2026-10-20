import Base

# Viewer polls compare metadata exactly. The revision includes subsecond
# times and file identity; it is not a hash (collisions must not lose edits).
def Revision.decide(size_changed: Bool, same_revision: Bool) -> Bool:
  match size_changed same_revision:
    case True{} _: True{}
    case False{} False{}: True{}
    case False{} True{}: False{}

def Revision.changed(sz: U32, rev: String, oldsz: U32, oldrev: String) -> Bool:
  Revision.decide(U32.is_ne(sz, oldsz), String.eq(rev, oldrev))
