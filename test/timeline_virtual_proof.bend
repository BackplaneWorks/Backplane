import Base
import ../src/core/timeline-virtual.bend as V

# These prove the actual model's carried offset and branch policies. Decimal
# arithmetic and DOM application are covered by the focused fractional tests
# and integrated browser smoke, not assumed to be floating-point field laws.
def prepend_offset(+a: V.Anchor, before: V.Px, after: V.Px) ->
    {V.Position.offset(V.Anchor.move(a, after)) == V.Position.offset(V.Anchor.move(a, before)) : V.Px}:
  {==}

def anchor_rendered(inRange: Bool) -> {V.Window.keep(True{}, inRange) == True{} : Bool}:
  {==}

def follow_only_at_bottom(follow: Bool, +held: V.Px, end: V.Px) ->
    {V.Follow.choose(V.Follow.allowed(follow, False{}), held, end) == held : V.Px}:
  match follow:
    case False{}: {==}
    case True{}: {==}
