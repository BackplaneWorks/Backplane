import Base
import ../src/core/hub.bend as H
import ../src/core/model.bend as M

law no_implicit_publication:
  for s: M.State
  for now: Nat
  for th: String
  for text: String
  for key: String
  {H.Private.tell(False{}, s, now, th, text, key) == H.Step.none() : H.Step}
def no_implicit_publication(s, now, th, text, key): {==}

law duplicate_publication_none:
  for s: M.State
  for now: Nat
  for th: String
  for text: String
  for key: String
  {H.Private.tell.fresh(False{}, s, now, th, text, key) == H.Step.none() : H.Step}
def duplicate_publication_none(s, now, th, text, key): {==}
