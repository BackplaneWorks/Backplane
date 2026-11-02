import Base
import ../src/core/json.bend as J
import ../src/core/eq.bend as Q
import ../src/server/thin-content-eq.bend as E

# Existing equality lemmas copied unchanged from PROOF, minimal dependency closure.
def Bool.disc(b: Bool) -> Type:
  match b:
    case False{}:
      Unit
    case True{}:
      Empty

law false_ne_true:
  for e: {False{} == True{} : Bool}
  Empty

def false_ne_true(e):
  %e : Bool.disc(_)
  Unit{}

# anything is at least zero

def Cmp.eqd(c: Cmp) -> Type:
  match c:
    case LT{}:
      Unit
    case EQ{}:
      Empty
    case GT{}:
      Unit

law lt_ne_eq:
  for e: {LT{} == EQ{} : Cmp}
  Empty

def lt_ne_eq(e):
  %e : Cmp.eqd(_)
  Unit{}

law gt_ne_eq:
  for e: {GT{} == EQ{} : Cmp}
  Empty

def gt_ne_eq(e):
  %e : Cmp.eqd(_)
  Unit{}

law and_l:
  for a: Bool
  for -b: Bool
  for h: {Bool.and(a, b) == True{} : Bool}
  {a == True{} : Bool}

def and_l(a, b, h):
  match a:
    case False{}:
      Empty.absurd({False{} == True{} : Bool}, false_ne_true(h))
    case True{}:
      {==}

law and_r:
  for a: Bool
  for -b: Bool
  for h: {Bool.and(a, b) == True{} : Bool}
  {b == True{} : Bool}

def and_r(a, b, h):
  match a:
    case False{}:
      Empty.absurd({b == True{} : Bool}, false_ne_true(h))
    case True{}:
      h

law cmp_is_eq_sound:
  for c: Cmp
  for h: {Cmp.is_eq(c) == True{} : Bool}
  {c == EQ{} : Cmp}

def cmp_is_eq_sound(c, h):
  match c:
    case LT{}:
      Empty.absurd({LT{} == EQ{} : Cmp}, false_ne_true(h))
    case EQ{}:
      {==}
    case GT{}:
      Empty.absurd({GT{} == EQ{} : Cmp}, false_ne_true(h))

law bool_cmp_sound:
  for a: Bool
  for b: Bool
  for h: {Bool.cmp(a, b) == EQ{} : Cmp}
  {a == b : Bool}

def bool_cmp_sound(a, b, h):
  match a b:
    case False{} False{}:
      {==}
    case False{} True{}:
      Empty.absurd({False{} == True{} : Bool}, lt_ne_eq(h))
    case True{} False{}:
      Empty.absurd({True{} == False{} : Bool}, gt_ne_eq(h))
    case True{} True{}:
      {==}

law word_fin_t:
  for -ab: Bool
  for -bb: Bool
  for t: Cmp
  for h: {Word.cmp.fin(ab, bb, t) == EQ{} : Cmp}
  {t == EQ{} : Cmp}

def word_fin_t(ab, bb, t, h):
  match t:
    case LT{}:
      Empty.absurd({LT{} == EQ{} : Cmp}, lt_ne_eq(h))
    case EQ{}:
      {==}
    case GT{}:
      Empty.absurd({GT{} == EQ{} : Cmp}, gt_ne_eq(h))

law word_fin_b:
  for -ab: Bool
  for -bb: Bool
  for t: Cmp
  for h: {Word.cmp.fin(ab, bb, t) == EQ{} : Cmp}
  {Bool.cmp(ab, bb) == EQ{} : Cmp}

def word_fin_b(ab, bb, t, h):
  match t:
    case LT{}:
      Empty.absurd({Bool.cmp(ab, bb) == EQ{} : Cmp}, lt_ne_eq(h))
    case EQ{}:
      h
    case GT{}:
      Empty.absurd({Bool.cmp(ab, bb) == EQ{} : Cmp}, gt_ne_eq(h))

law word_cmp_sound:
  for n: Nat
  for a: Word(n)
  for b: Word(n)
  for +h: {Word.cmp(n, a, b) == EQ{} : Cmp}
  {a == b : Word(n)}

def word_cmp_sound(n, a, b, h):
  match n:
    case 0n:
      match a b:
        case WNil{} WNil{}:
          {==}
    case 1n+p0:
      match a b:
        case WCon{+ab, +at} WCon{+bb, +bt}:
          +p = p0
          e0 = bool_cmp_sound(ab, bb, word_fin_b(ab, bb, Word.cmp(p, at, bt), h))
          e1 = word_cmp_sound(p, at, bt, word_fin_t(ab, bb, Word.cmp(p, at, bt), h))
          %e0 : {WCon{ab, at} == WCon{_, bt} : Word(1n+p)}
          %e1 : {WCon{ab, at} == WCon{ab, _} : Word(1n+p)}
          {==}

law u32_eq_sound:
  for a: U32
  for b: U32
  for h: {U32.is_eq(a, b) == True{} : Bool}
  {a == b : U32}

def u32_eq_sound(a, b, h):
  match a b:
    case U32{+x} U32{+y}:
      e = word_cmp_sound(32n, x, y, cmp_is_eq_sound(Word.cmp(32n, x, y), h))
      %e : {U32{x} == U32{_} : U32}
      {==}

law bool_eq_sound:
  for a: Bool
  for b: Bool
  for h: {Q.Eq.bool(a, b) == True{} : Bool}
  {a == b : Bool}

def bool_eq_sound(a, b, h):
  match a b:
    case False{} False{}:
      {==}
    case False{} True{}:
      Empty.absurd({False{} == True{} : Bool}, false_ne_true(h))
    case True{} False{}:
      Empty.absurd({True{} == False{} : Bool}, false_ne_true(h))
    case True{} True{}:
      {==}

law eq_chr_sound:
  for a: Char
  for b: Char
  for +h: {Q.Eq.chr(a, b) == True{} : Bool}
  {a == b : Char}

def eq_chr_sound(a, b, h):
  match a b:
    case Chr{+a0} Chr{+b0}:
      e0 = u32_eq_sound(a0, b0, h)
      %e0 : {Chr{a0} == Chr{_} : Char}
      {==}

# Eq.str walks with its verdict carried (Eq.str.go): a False verdict
# answers False at once, whatever is left
law eq_str_go_false:
  for a: String
  for b: String
  {Q.Eq.str.go(a, b, False{}) == False{} : Bool}

def eq_str_go_false(a, b):
  match a b:
    case SNil{} SNil{}:
      {==}
    case SNil{} SCon{+y, +yt}:
      {==}
    case SCon{+x, +xt} SNil{}:
      {==}
    case SCon{+x, +xt} SCon{+y, +yt}:
      {==}

# so a walk that ends True was carrying True
law eq_str_go_ok:
  for a: String
  for b: String
  for ok: Bool
  for +h: {Q.Eq.str.go(a, b, ok) == True{} : Bool}
  {ok == True{} : Bool}

def eq_str_go_ok(a, b, ok, h):
  match ok:
    case True{}:
      {==}
    case False{}:
      Empty.absurd({False{} == True{} : Bool}, false_ne_true(Equal.trans(Bool, False{}, Q.Eq.str.go(a, b, False{}), True{},
        Equal.sym(Bool, Q.Eq.str.go(a, b, False{}), False{}, eq_str_go_false(a, b)), h)))

law eq_str_go_sound:
  for a: String
  for b: String
  for ok: Bool
  for +h: {Q.Eq.str.go(a, b, ok) == True{} : Bool}
  {a == b : String}

def eq_str_go_sound(a, b, ok, h):
  match a b ok:
    case SNil{} SNil{} True{}:
      {==}
    case SNil{} SNil{} False{}:
      {==}
    case SNil{} SCon{+y, +yt} True{}:
      Empty.absurd({SNil{} == SCon{y, yt} : String}, false_ne_true(h))
    case SNil{} SCon{+y, +yt} False{}:
      Empty.absurd({SNil{} == SCon{y, yt} : String}, false_ne_true(h))
    case SCon{+x, +xt} SNil{} True{}:
      Empty.absurd({SCon{x, xt} == SNil{} : String}, false_ne_true(h))
    case SCon{+x, +xt} SNil{} False{}:
      Empty.absurd({SCon{x, xt} == SNil{} : String}, false_ne_true(h))
    case SCon{+x, +xt} SCon{+y, +yt} True{}:
      e0 = eq_chr_sound(x, y, eq_str_go_ok(xt, yt, Q.Eq.chr(x, y), h))
      e1 = eq_str_go_sound(xt, yt, Q.Eq.chr(x, y), h)
      %e0 : {SCon{x, xt} == SCon{_, yt} : String}
      %e1 : {SCon{x, xt} == SCon{x, _} : String}
      {==}
    case SCon{+x, +xt} SCon{+y, +yt} False{}:
      Empty.absurd({SCon{x, xt} == SCon{y, yt} : String}, false_ne_true(h))

law eq_str_sound:
  for a: String
  for b: String
  for +h: {Q.Eq.str(a, b) == True{} : Bool}
  {a == b : String}

def eq_str_sound(a, b, h):
  eq_str_go_sound(a, b, True{}, h)

law content_equal_sound:
  for a: J.Json
  for b: J.Json
  for +h: {E.ContentEq.go(a, b) == True{} : Bool}
  {a == b : J.Json}

def content_equal_sound(a, b, h):
  match a b:
    case J.Null{} J.Null{}:
      {==}
    case J.Null{} J.Flag{+bx}:
      Empty.absurd({J.Null{} == J.Flag{bx} : J.Json}, false_ne_true(h))
    case J.Null{} J.Num{+bx}:
      Empty.absurd({J.Null{} == J.Num{bx} : J.Json}, false_ne_true(h))
    case J.Null{} J.Str{+bx}:
      Empty.absurd({J.Null{} == J.Str{bx} : J.Json}, false_ne_true(h))
    case J.Null{} J.Arr{+bx}:
      Empty.absurd({J.Null{} == J.Arr{bx} : J.Json}, false_ne_true(h))
    case J.Null{} J.Obj{+bx}:
      Empty.absurd({J.Null{} == J.Obj{bx} : J.Json}, false_ne_true(h))
    case J.Null{} J.Item{+bx, +bt}:
      Empty.absurd({J.Null{} == J.Item{bx, bt} : J.Json}, false_ne_true(h))
    case J.Null{} J.Field{+bk, +bx, +bt}:
      Empty.absurd({J.Null{} == J.Field{bk, bx, bt} : J.Json}, false_ne_true(h))
    case J.Null{} J.End{}:
      Empty.absurd({J.Null{} == J.End{} : J.Json}, false_ne_true(h))
    case J.Flag{+ax} J.Null{}:
      Empty.absurd({J.Flag{ax} == J.Null{} : J.Json}, false_ne_true(h))
    case J.Flag{+ax} J.Flag{+bx}:
      e = bool_eq_sound(ax, bx, h)
      %e : {J.Flag{ax} == J.Flag{_} : J.Json}
      {==}
    case J.Flag{+ax} J.Num{+bx}:
      Empty.absurd({J.Flag{ax} == J.Num{bx} : J.Json}, false_ne_true(h))
    case J.Flag{+ax} J.Str{+bx}:
      Empty.absurd({J.Flag{ax} == J.Str{bx} : J.Json}, false_ne_true(h))
    case J.Flag{+ax} J.Arr{+bx}:
      Empty.absurd({J.Flag{ax} == J.Arr{bx} : J.Json}, false_ne_true(h))
    case J.Flag{+ax} J.Obj{+bx}:
      Empty.absurd({J.Flag{ax} == J.Obj{bx} : J.Json}, false_ne_true(h))
    case J.Flag{+ax} J.Item{+bx, +bt}:
      Empty.absurd({J.Flag{ax} == J.Item{bx, bt} : J.Json}, false_ne_true(h))
    case J.Flag{+ax} J.Field{+bk, +bx, +bt}:
      Empty.absurd({J.Flag{ax} == J.Field{bk, bx, bt} : J.Json}, false_ne_true(h))
    case J.Flag{+ax} J.End{}:
      Empty.absurd({J.Flag{ax} == J.End{} : J.Json}, false_ne_true(h))
    case J.Num{+ax} J.Null{}:
      Empty.absurd({J.Num{ax} == J.Null{} : J.Json}, false_ne_true(h))
    case J.Num{+ax} J.Flag{+bx}:
      Empty.absurd({J.Num{ax} == J.Flag{bx} : J.Json}, false_ne_true(h))
    case J.Num{+ax} J.Num{+bx}:
      e = eq_str_sound(ax, bx, h)
      %e : {J.Num{ax} == J.Num{_} : J.Json}
      {==}
    case J.Num{+ax} J.Str{+bx}:
      Empty.absurd({J.Num{ax} == J.Str{bx} : J.Json}, false_ne_true(h))
    case J.Num{+ax} J.Arr{+bx}:
      Empty.absurd({J.Num{ax} == J.Arr{bx} : J.Json}, false_ne_true(h))
    case J.Num{+ax} J.Obj{+bx}:
      Empty.absurd({J.Num{ax} == J.Obj{bx} : J.Json}, false_ne_true(h))
    case J.Num{+ax} J.Item{+bx, +bt}:
      Empty.absurd({J.Num{ax} == J.Item{bx, bt} : J.Json}, false_ne_true(h))
    case J.Num{+ax} J.Field{+bk, +bx, +bt}:
      Empty.absurd({J.Num{ax} == J.Field{bk, bx, bt} : J.Json}, false_ne_true(h))
    case J.Num{+ax} J.End{}:
      Empty.absurd({J.Num{ax} == J.End{} : J.Json}, false_ne_true(h))
    case J.Str{+ax} J.Null{}:
      Empty.absurd({J.Str{ax} == J.Null{} : J.Json}, false_ne_true(h))
    case J.Str{+ax} J.Flag{+bx}:
      Empty.absurd({J.Str{ax} == J.Flag{bx} : J.Json}, false_ne_true(h))
    case J.Str{+ax} J.Num{+bx}:
      Empty.absurd({J.Str{ax} == J.Num{bx} : J.Json}, false_ne_true(h))
    case J.Str{+ax} J.Str{+bx}:
      e = eq_str_sound(ax, bx, h)
      %e : {J.Str{ax} == J.Str{_} : J.Json}
      {==}
    case J.Str{+ax} J.Arr{+bx}:
      Empty.absurd({J.Str{ax} == J.Arr{bx} : J.Json}, false_ne_true(h))
    case J.Str{+ax} J.Obj{+bx}:
      Empty.absurd({J.Str{ax} == J.Obj{bx} : J.Json}, false_ne_true(h))
    case J.Str{+ax} J.Item{+bx, +bt}:
      Empty.absurd({J.Str{ax} == J.Item{bx, bt} : J.Json}, false_ne_true(h))
    case J.Str{+ax} J.Field{+bk, +bx, +bt}:
      Empty.absurd({J.Str{ax} == J.Field{bk, bx, bt} : J.Json}, false_ne_true(h))
    case J.Str{+ax} J.End{}:
      Empty.absurd({J.Str{ax} == J.End{} : J.Json}, false_ne_true(h))
    case J.Arr{+ax} J.Null{}:
      Empty.absurd({J.Arr{ax} == J.Null{} : J.Json}, false_ne_true(h))
    case J.Arr{+ax} J.Flag{+bx}:
      Empty.absurd({J.Arr{ax} == J.Flag{bx} : J.Json}, false_ne_true(h))
    case J.Arr{+ax} J.Num{+bx}:
      Empty.absurd({J.Arr{ax} == J.Num{bx} : J.Json}, false_ne_true(h))
    case J.Arr{+ax} J.Str{+bx}:
      Empty.absurd({J.Arr{ax} == J.Str{bx} : J.Json}, false_ne_true(h))
    case J.Arr{+ax} J.Arr{+bx}:
      e = content_equal_sound(ax, bx, h)
      %e : {J.Arr{ax} == J.Arr{_} : J.Json}
      {==}
    case J.Arr{+ax} J.Obj{+bx}:
      Empty.absurd({J.Arr{ax} == J.Obj{bx} : J.Json}, false_ne_true(h))
    case J.Arr{+ax} J.Item{+bx, +bt}:
      Empty.absurd({J.Arr{ax} == J.Item{bx, bt} : J.Json}, false_ne_true(h))
    case J.Arr{+ax} J.Field{+bk, +bx, +bt}:
      Empty.absurd({J.Arr{ax} == J.Field{bk, bx, bt} : J.Json}, false_ne_true(h))
    case J.Arr{+ax} J.End{}:
      Empty.absurd({J.Arr{ax} == J.End{} : J.Json}, false_ne_true(h))
    case J.Obj{+ax} J.Null{}:
      Empty.absurd({J.Obj{ax} == J.Null{} : J.Json}, false_ne_true(h))
    case J.Obj{+ax} J.Flag{+bx}:
      Empty.absurd({J.Obj{ax} == J.Flag{bx} : J.Json}, false_ne_true(h))
    case J.Obj{+ax} J.Num{+bx}:
      Empty.absurd({J.Obj{ax} == J.Num{bx} : J.Json}, false_ne_true(h))
    case J.Obj{+ax} J.Str{+bx}:
      Empty.absurd({J.Obj{ax} == J.Str{bx} : J.Json}, false_ne_true(h))
    case J.Obj{+ax} J.Arr{+bx}:
      Empty.absurd({J.Obj{ax} == J.Arr{bx} : J.Json}, false_ne_true(h))
    case J.Obj{+ax} J.Obj{+bx}:
      e = content_equal_sound(ax, bx, h)
      %e : {J.Obj{ax} == J.Obj{_} : J.Json}
      {==}
    case J.Obj{+ax} J.Item{+bx, +bt}:
      Empty.absurd({J.Obj{ax} == J.Item{bx, bt} : J.Json}, false_ne_true(h))
    case J.Obj{+ax} J.Field{+bk, +bx, +bt}:
      Empty.absurd({J.Obj{ax} == J.Field{bk, bx, bt} : J.Json}, false_ne_true(h))
    case J.Obj{+ax} J.End{}:
      Empty.absurd({J.Obj{ax} == J.End{} : J.Json}, false_ne_true(h))
    case J.Item{+ax, +at} J.Null{}:
      Empty.absurd({J.Item{ax, at} == J.Null{} : J.Json}, false_ne_true(h))
    case J.Item{+ax, +at} J.Flag{+bx}:
      Empty.absurd({J.Item{ax, at} == J.Flag{bx} : J.Json}, false_ne_true(h))
    case J.Item{+ax, +at} J.Num{+bx}:
      Empty.absurd({J.Item{ax, at} == J.Num{bx} : J.Json}, false_ne_true(h))
    case J.Item{+ax, +at} J.Str{+bx}:
      Empty.absurd({J.Item{ax, at} == J.Str{bx} : J.Json}, false_ne_true(h))
    case J.Item{+ax, +at} J.Arr{+bx}:
      Empty.absurd({J.Item{ax, at} == J.Arr{bx} : J.Json}, false_ne_true(h))
    case J.Item{+ax, +at} J.Obj{+bx}:
      Empty.absurd({J.Item{ax, at} == J.Obj{bx} : J.Json}, false_ne_true(h))
    case J.Item{+ax, +at} J.Item{+bx, +bt}:
      e0 = content_equal_sound(ax, bx, and_l(E.ContentEq.go(ax, bx), E.ContentEq.go(at, bt), h))
      e1 = content_equal_sound(at, bt, and_r(E.ContentEq.go(ax, bx), E.ContentEq.go(at, bt), h))
      %e0 : {J.Item{ax, at} == J.Item{_, bt} : J.Json}
      %e1 : {J.Item{ax, at} == J.Item{ax, _} : J.Json}
      {==}
    case J.Item{+ax, +at} J.Field{+bk, +bx, +bt}:
      Empty.absurd({J.Item{ax, at} == J.Field{bk, bx, bt} : J.Json}, false_ne_true(h))
    case J.Item{+ax, +at} J.End{}:
      Empty.absurd({J.Item{ax, at} == J.End{} : J.Json}, false_ne_true(h))
    case J.Field{+ak, +ax, +at} J.Null{}:
      Empty.absurd({J.Field{ak, ax, at} == J.Null{} : J.Json}, false_ne_true(h))
    case J.Field{+ak, +ax, +at} J.Flag{+bx}:
      Empty.absurd({J.Field{ak, ax, at} == J.Flag{bx} : J.Json}, false_ne_true(h))
    case J.Field{+ak, +ax, +at} J.Num{+bx}:
      Empty.absurd({J.Field{ak, ax, at} == J.Num{bx} : J.Json}, false_ne_true(h))
    case J.Field{+ak, +ax, +at} J.Str{+bx}:
      Empty.absurd({J.Field{ak, ax, at} == J.Str{bx} : J.Json}, false_ne_true(h))
    case J.Field{+ak, +ax, +at} J.Arr{+bx}:
      Empty.absurd({J.Field{ak, ax, at} == J.Arr{bx} : J.Json}, false_ne_true(h))
    case J.Field{+ak, +ax, +at} J.Obj{+bx}:
      Empty.absurd({J.Field{ak, ax, at} == J.Obj{bx} : J.Json}, false_ne_true(h))
    case J.Field{+ak, +ax, +at} J.Item{+bx, +bt}:
      Empty.absurd({J.Field{ak, ax, at} == J.Item{bx, bt} : J.Json}, false_ne_true(h))
    case J.Field{+ak, +ax, +at} J.Field{+bk, +bx, +bt}:
      +hr = and_r(Q.Eq.str(ak, bk), Bool.and(E.ContentEq.go(ax, bx), E.ContentEq.go(at, bt)), h)
      e0 = eq_str_sound(ak, bk, and_l(Q.Eq.str(ak, bk), Bool.and(E.ContentEq.go(ax, bx), E.ContentEq.go(at, bt)), h))
      e1 = content_equal_sound(ax, bx, and_l(E.ContentEq.go(ax, bx), E.ContentEq.go(at, bt), hr))
      e2 = content_equal_sound(at, bt, and_r(E.ContentEq.go(ax, bx), E.ContentEq.go(at, bt), hr))
      %e0 : {J.Field{ak, ax, at} == J.Field{_, bx, bt} : J.Json}
      %e1 : {J.Field{ak, ax, at} == J.Field{ak, _, bt} : J.Json}
      %e2 : {J.Field{ak, ax, at} == J.Field{ak, ax, _} : J.Json}
      {==}
    case J.Field{+ak, +ax, +at} J.End{}:
      Empty.absurd({J.Field{ak, ax, at} == J.End{} : J.Json}, false_ne_true(h))
    case J.End{} J.Null{}:
      Empty.absurd({J.End{} == J.Null{} : J.Json}, false_ne_true(h))
    case J.End{} J.Flag{+bx}:
      Empty.absurd({J.End{} == J.Flag{bx} : J.Json}, false_ne_true(h))
    case J.End{} J.Num{+bx}:
      Empty.absurd({J.End{} == J.Num{bx} : J.Json}, false_ne_true(h))
    case J.End{} J.Str{+bx}:
      Empty.absurd({J.End{} == J.Str{bx} : J.Json}, false_ne_true(h))
    case J.End{} J.Arr{+bx}:
      Empty.absurd({J.End{} == J.Arr{bx} : J.Json}, false_ne_true(h))
    case J.End{} J.Obj{+bx}:
      Empty.absurd({J.End{} == J.Obj{bx} : J.Json}, false_ne_true(h))
    case J.End{} J.Item{+bx, +bt}:
      Empty.absurd({J.End{} == J.Item{bx, bt} : J.Json}, false_ne_true(h))
    case J.End{} J.Field{+bk, +bx, +bt}:
      Empty.absurd({J.End{} == J.Field{bk, bx, bt} : J.Json}, false_ne_true(h))
    case J.End{} J.End{}:
      {==}

law exact_sound:
  for a: J.Json
  for b: J.Json
  for +h: {E.ContentEq.same(a, b) == True{} : Bool}
  {a == b : J.Json}

def exact_sound(a, b, h):
  content_equal_sound(a, b, h)
