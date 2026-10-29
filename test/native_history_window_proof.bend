import Base
import ../src/core/history-window.bend as H

def Disjoint(b: Bool) -> Type:
  match b:
    case False{}: Unit
    case True{}: Empty

def false_true(h: {False{} == True{} : Bool}) -> Empty:
  %h : Disjoint(_)
  Unit{}

def left(+a: Bool, b: Bool, h: {Bool.and(a, b) == True{} : Bool}) -> {a == True{} : Bool}:
  match a:
    case False{}: Empty.absurd({False{} == True{} : Bool}, false_true(h))
    case True{}: {==}

def right(a: Bool, +b: Bool, h: {Bool.and(a, b) == True{} : Bool}) -> {b == True{} : Bool}:
  match a:
    case False{}: Empty.absurd({b == True{} : Bool}, false_true(h))
    case True{}: h

def adopt_visible_k(ok: Bool, +old: List<&2, String>, +candidate: List<&2, String>, +visible: List<&2, String>,
    before: {H.History.ordered(visible, old) == True{} : Bool},
    h: {H.History.valid(old, candidate, visible) == ok : Bool}) ->
    {H.History.ordered(visible, H.History.choose(ok, old, candidate)) == True{} : Bool}:
  match ok:
    case False{}: before
    case True{}: left(H.History.ordered(visible, candidate), Nat.is_ge(List.length(&2, String, candidate), List.length(&2, String, old)), h)

def adopt_visible(+old: List<&2, String>, +candidate: List<&2, String>, +visible: List<&2, String>,
    before: {H.History.ordered(visible, old) == True{} : Bool}) ->
    {H.History.ordered(visible, H.History.adopt(old, candidate, visible)) == True{} : Bool}:
  adopt_visible_k(H.History.valid(old, candidate, visible), old, candidate, visible, before, {==})

def adopt_size_k(ok: Bool, +old: List<&2, String>, +candidate: List<&2, String>, +visible: List<&2, String>,
    h: {H.History.valid(old, candidate, visible) == ok : Bool}) ->
    {Nat.is_ge(List.length(&2, String, H.History.choose(ok, old, candidate)), List.length(&2, String, old)) == True{} : Bool}:
  match ok:
    case False{}: Nat.ge_refl(List.length(&2, String, old))
    case True{}: right(H.History.ordered(visible, candidate), Nat.is_ge(List.length(&2, String, candidate), List.length(&2, String, old)), h)

def adopt_size(+old: List<&2, String>, +candidate: List<&2, String>, +visible: List<&2, String>) ->
    {Nat.is_ge(List.length(&2, String, H.History.adopt(old, candidate, visible)), List.length(&2, String, old)) == True{} : Bool}:
  adopt_size_k(H.History.valid(old, candidate, visible), old, candidate, visible, {==})

def reject_k(order: Bool, size: Bool, +old: List<&2, String>, +candidate: List<&2, String>, h: {order == False{} : Bool}) ->
    {H.History.choose(Bool.and(order, size), old, candidate) == old : List<&2, String>}:
  match order:
    case False{}: {==}
    case True{}: Empty.absurd({H.History.choose(size, old, candidate) == old : List<&2, String>}, false_true(Equal.sym(Bool, True{}, False{}, h)))

def reject_order(+old: List<&2, String>, +candidate: List<&2, String>, +visible: List<&2, String>,
    h: {H.History.ordered(visible, candidate) == False{} : Bool}) ->
    {H.History.adopt(old, candidate, visible) == old : List<&2, String>}:
  reject_k(H.History.ordered(visible, candidate), Nat.is_ge(List.length(&2, String, candidate), List.length(&2, String, old)), old, candidate, h)

def page_visible_k(ok: Bool, +old: List<&2, String>, +fresh: List<&2, String>, +candidate: List<&2, String>, +visible: List<&2, String>, +older: Bool,
    before: {H.History.ordered(visible, old) == True{} : Bool},
    h: {H.History.valid(old, candidate, visible) == ok : Bool}) ->
    {H.History.ordered(visible, H.History.page.k(ok, old, fresh, candidate, visible, older)) == True{} : Bool}:
  match ok:
    case True{}: left(H.History.ordered(visible, candidate), Nat.is_ge(List.length(&2, String, candidate), List.length(&2, String, old)), h)
    case False{}: adopt_visible(old, H.History.project.planned(old, fresh, old, older), visible, before)

def page_visible(+old: List<&2, String>, +fresh: List<&2, String>, +keep: List<&2, String>, +visible: List<&2, String>, +older: Bool,
    before: {H.History.ordered(visible, old) == True{} : Bool}) ->
    {H.History.ordered(visible, H.History.page(old, fresh, keep, visible, older)) == True{} : Bool}:
  +candidate = H.History.project.planned(old, fresh, keep, older)
  page_visible_k(H.History.valid(old, candidate, visible), old, fresh, candidate, visible, older, before, {==})

def page_size_k(ok: Bool, +old: List<&2, String>, +fresh: List<&2, String>, +candidate: List<&2, String>, +visible: List<&2, String>, +older: Bool,
    h: {H.History.valid(old, candidate, visible) == ok : Bool}) ->
    {Nat.is_ge(List.length(&2, String, H.History.page.k(ok, old, fresh, candidate, visible, older)), List.length(&2, String, old)) == True{} : Bool}:
  match ok:
    case True{}: right(H.History.ordered(visible, candidate), Nat.is_ge(List.length(&2, String, candidate), List.length(&2, String, old)), h)
    case False{}: adopt_size(old, H.History.project.planned(old, fresh, old, older), visible)

def page_size(+old: List<&2, String>, +fresh: List<&2, String>, +keep: List<&2, String>, +visible: List<&2, String>, +older: Bool) ->
    {Nat.is_ge(List.length(&2, String, H.History.page(old, fresh, keep, visible, older)), List.length(&2, String, old)) == True{} : Bool}:
  +candidate = H.History.project.planned(old, fresh, keep, older)
  page_size_k(H.History.valid(old, candidate, visible), old, fresh, candidate, visible, older, {==})
