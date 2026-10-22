import Base
import ./test/mobile_loading_proof.bend as PRESENTPROOF
import ./src/core/mailcontinuity.bend as MCN
import ./test/mem_score_lazy_proof.bend as MSCORE
import ./src/core/refresh.bend as RFSH
import ./src/core/auth.bend as AUTH
import ./src/core/http.bend as HTTPAUTH
import ./src/core/pcb2d.bend as PCB2 # PCB viewer text
import ./src/app/side_rows.bend as SR
import ./src/app/side_row_types.bend as SRT
import ./src/gfx/draw.bend as D
import ./src/server/botnet.bend as MPLOG # mail perf runtime count
import ./src/server/server.bend as MPCREDIT # mail perf
import ./src/core/schematic.bend as SC # kicad fork viewers
import ./src/core/charter.bend as CH # mail 2c
import ./src/core/mailrun.bend as MRUN # mail 2c
import ./src/app/mailroom.bend as MRUI # mail ui
import ./src/server/mailstore.bend as MIX # mail index
import ./src/app/mm.bend as MM # window memos
import ./src/core/kexport.bend as KEX
import ./src/core/i18n.bend as I18
import ./test/fixtures/i18n_fallback.bend as I18F
import ./src/core/mail.bend as ML # mail 1
import ./src/core/mailbody.bend as MB # mail 2a
import ./src/core/mailview.bend as MVW # mail view
import ./src/core/ref.bend as MREF # mail 2b
import ./src/core/stamp.bend as STP
import ./src/core/kihealth.bend as KH
import ./src/core/tz.bend as TZ
import ./src/core/when.bend as WH
import ./src/core/sched.bend as SCH
import ./src/core/locks.bend as LCK
import ./src/gfx/text.bend as X
import ./src/core/bothub.bend as BHS
import ./src/core/bot.bend as BOT
import ./src/core/botui.bend as BU
import ./LAWS.bend as Laws
import ./src/core/model.bend as M
import ./src/app/viewer.bend as V
import ./src/core/board.bend as B
import ./src/core/semver.bend as SV
import ./src/app/nui.bend as N
import ./src/core/hub.bend as H
import ./src/core/ship.bend as SHP # deploy main
import ./src/core/refs.bend as RF
import ./src/core/todo.bend as TD
import ./src/core/acp.bend as AC
import ./src/core/mcp.bend as MC
import ./src/core/ride.bend as RD
import ./src/core/handoff.bend as HD
import ./src/core/agent.bend as AG
import ./src/core/fuzzy.bend as FZ
import ./src/mobile/notify.bend as Nt
import ./src/core/notice.bend as NO
import ./src/core/codec.bend as C
import ./src/core/json.bend as J
import ./src/core/attach.bend as AT
import ./src/core/svg.bend as SVG
import ./src/core/lightbox.bend as LBX
import ./src/core/links.bend as LK
import ./src/web/dom.bend as DM
import ./src/core/tailnet.bend as TN
import ./src/gfx/tiles.bend as T
import ./src/gfx/raster.bend as R
import ./src/gfx/font.bend as F
import ./src/app/layout.bend as L
import ./src/core/eq.bend as Q
import ./src/core/gh.bend as GH
import ./src/core/fold.bend as FO
import ./src/core/subs.bend as SB
import ./src/core/plot.bend as PL
import ./src/core/look.bend as LO
import ./src/core/hist.bend as HS
import ./src/mobile/view.bend as MV
import ./src/core/pcb3d.bend as P3
import ./src/gfx/mesh.bend as MS
import ./src/gfx/face.bend as FA
import ./src/core/glb.bend as G
import ./src/mobile/picker.bend as MP
import ./src/mobile/hubs.bend as Hb
import ./src/mobile/bots.bend as MBN
import ./src/core/space.bend as SPC
import ./src/core/client.bend as U
import ./src/core/menu.bend as MN
import ./src/core/hier.bend as HI

import ./src/core/boot.bend as BT
import ./src/core/store.bend as ST
import ./src/core/sign.bend as SG
import ./src/core/cron.bend as CR
import ./src/core/google.bend as GO
import ./src/core/cat.bend as CAT
import ./src/core/voice.bend as VO
import ./src/core/once.bend as ON
import ./src/core/bom.bend as BOM
import ./src/core/cad.bend as CAD
import ./src/core/checks.bend as CHK
import ./src/core/release.bend as REL
import ./src/core/plane.bend as PLN
import ./src/core/uber.bend as UBR
import ./src/core/ubersync.bend as UBS
import ./src/core/ubop.bend as UBO

# Lemmas
# ------

law bool_cmp_refl:
  for +b: Bool
  {Bool.cmp(b, b) == EQ{} : Cmp}

def bool_cmp_refl(b):
  match b:
    case False{}:
      {==}
    case True{}:
      {==}

law word_cmp_refl:
  for n: Nat
  for +w: Word(n)
  {Word.cmp(n, w, w) == EQ{} : Cmp}

def word_cmp_refl(n, w):
  match n:
    case 0n:
      {==}
    case 1n+p:
      match w:
        case WCon{+b, +t}:
          %Equal.sym(Cmp, Word.cmp(p, t, t), EQ{}, word_cmp_refl(p, t)) : {Word.cmp.fin(b, b, _) == EQ{} : Cmp}
          bool_cmp_refl(b)

law u32_cmp_refl:
  for +x: U32
  {U32.cmp(x, x) == EQ{} : Cmp}

def u32_cmp_refl(x):
  match x:
    case U32{+w}:
      word_cmp_refl(32n, w)

# a string compares equal to itself, handing both copies back
law string_cmp_refl:
  for +s: String
  {String.cmp(s, s) == ((s, s), EQ{}) : (String & String) & Cmp}

def string_cmp_refl(s):
  match s:
    case SNil{}:
      {==}
    case SCon{+h, +t}:
      match h:
        case Chr{+x}:
          %Equal.sym(Cmp, U32.cmp(x, x), EQ{}, u32_cmp_refl(x)) : {String.cmp.fin(t, t, ((Chr{x}, Chr{x}), _)) == ((SCon{Chr{x}, t}, SCon{Chr{x}, t}), EQ{}) : (String & String) & Cmp}
          %Equal.sym((String & String) & Cmp, String.cmp(t, t), ((t, t), EQ{}), string_cmp_refl(t)) : {String.cmp.rec(Chr{x}, Chr{x}, _) == ((SCon{Chr{x}, t}, SCon{Chr{x}, t}), EQ{}) : (String & String) & Cmp}
          {==}

# False is not True
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
law ge_zero:
  for +a: Nat
  {Nat.is_ge(a, 0n) == True{} : Bool}

def ge_zero(a):
  match a:
    case 0n:
      {==}
    case 1n+p:
      {==}

# n is never at least n + 1 + k
law ge_past:
  for n: Nat
  for +k: Nat
  {Nat.is_ge(n, Nat.add(n, 1n+k)) == False{} : Bool}

def ge_past(n, k):
  match n:
    case 0n:
      {==}
    case 1n+p:
      ge_past(p, k)

# moving the left side forward keeps a >= b
law ge_add:
  for a: Nat
  for b: Nat
  for +d: Nat
  for h: {Nat.is_ge(a, b) == True{} : Bool}
  {Nat.is_ge(Nat.add(a, d), b) == True{} : Bool}

def ge_add(a, b, d, h):
  match a b:
    case 0n 0n:
      ge_zero(d)
    case 0n 1n+q:
      Empty.absurd({Nat.is_ge(Nat.add(0n, d), 1n+q) == True{} : Bool}, false_ne_true(h))
    case 1n+p 0n:
      ge_zero(1n+Nat.add(p, d))
    case 1n+p 1n+q:
      ge_add(p, q, d, h)

law add_succ_r:
  for a: Nat
  for -b: Nat
  {Nat.add(a, 1n+b) == 1n+Nat.add(a, b) : Nat}

def add_succ_r(a, b):
  match a:
    case 0n:
      {==}
    case 1n+p:
      %add_succ_r(p, b) : {1n+Nat.add(p, 1n+b) == 1n+_ : Nat}
      {==}

# x + y is one more than x + z when y is one more than z
law add_lift:
  for x: Nat
  for y: Nat
  for z: Nat
  for h: {y == 1n+z : Nat}
  {Nat.add(x, y) == 1n+Nat.add(x, z) : Nat}

def add_lift(x, y, z, h):
  %Equal.sym(Nat, y, 1n+z, h) : {Nat.add(x, _) == 1n+Nat.add(x, z) : Nat}
  add_succ_r(x, z)

# one put adds exactly one thread to the shelves
law put_count:
  for sh: M.Shelf
  for -t: M.Thread
  for s: M.Shelves
  {M.Shelves.count(M.Shelves.put(sh, t, s)) == 1n+M.Shelves.count(s) : Nat}

def put_count(sh, t, s):
  match sh s:
    case M.Pinned{} M.Shelves{+p, +a, +st, +h, +sn}:
      {==}
    case M.Active{} M.Shelves{+p, +a, +st, +h, +sn}:
      add_lift(List.length(&2, M.Thread, p), Nat.add(1n+List.length(&2, M.Thread, a), Nat.add(List.length(&2, M.Thread, st), Nat.add(List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn)))), Nat.add(List.length(&2, M.Thread, a), Nat.add(List.length(&2, M.Thread, st), Nat.add(List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn)))), {==})
    case M.Settled{} M.Shelves{+p, +a, +st, +h, +sn}:
      add_lift(List.length(&2, M.Thread, p), Nat.add(List.length(&2, M.Thread, a), Nat.add(1n+List.length(&2, M.Thread, st), Nat.add(List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn)))), Nat.add(List.length(&2, M.Thread, a), Nat.add(List.length(&2, M.Thread, st), Nat.add(List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn)))), add_lift(List.length(&2, M.Thread, a), Nat.add(1n+List.length(&2, M.Thread, st), Nat.add(List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn))), Nat.add(List.length(&2, M.Thread, st), Nat.add(List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn))), {==}))
    case M.Hidden{} M.Shelves{+p, +a, +st, +h, +sn}:
      add_lift(List.length(&2, M.Thread, p), Nat.add(List.length(&2, M.Thread, a), Nat.add(List.length(&2, M.Thread, st), Nat.add(1n+List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn)))), Nat.add(List.length(&2, M.Thread, a), Nat.add(List.length(&2, M.Thread, st), Nat.add(List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn)))), add_lift(List.length(&2, M.Thread, a), Nat.add(List.length(&2, M.Thread, st), Nat.add(1n+List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn))), Nat.add(List.length(&2, M.Thread, st), Nat.add(List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn))), add_lift(List.length(&2, M.Thread, st), Nat.add(1n+List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn)), Nat.add(List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn)), {==})))
    case M.Snoozed{} M.Shelves{+p, +a, +st, +h, +sn}:
      add_lift(List.length(&2, M.Thread, p), Nat.add(List.length(&2, M.Thread, a), Nat.add(List.length(&2, M.Thread, st), Nat.add(List.length(&2, M.Thread, h), 1n+List.length(&2, M.Thread, sn)))), Nat.add(List.length(&2, M.Thread, a), Nat.add(List.length(&2, M.Thread, st), Nat.add(List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn)))), add_lift(List.length(&2, M.Thread, a), Nat.add(List.length(&2, M.Thread, st), Nat.add(List.length(&2, M.Thread, h), 1n+List.length(&2, M.Thread, sn))), Nat.add(List.length(&2, M.Thread, st), Nat.add(List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn))), add_lift(List.length(&2, M.Thread, st), Nat.add(List.length(&2, M.Thread, h), 1n+List.length(&2, M.Thread, sn)), Nat.add(List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn)), add_succ_r(List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn)))))

# Laws
# ----

def Laws.string_eq_refl(s):
  %Equal.sym((String & String) & Cmp, String.cmp(s, s), ((s, s), EQ{}), string_cmp_refl(s)) : {Cmp.is_eq(Pair.snd(String & String, Cmp, _)) == True{} : Bool}
  {==}

def Laws.settle_at_window(last, w):
  Nat.ge_refl(Nat.add(last, w))

def Laws.settle_fresh(now, w):
  ge_past(now, w)

def Laws.settle_monotone(now, last, w, d, h):
  ge_add(now, Nat.add(last, w), d, h)

def Laws.settle_never_running(now, w, pending, archived, o):
  {==}

def Laws.settle_never_pending(t, p, archived, o, stale):
  match t:
    case M.Idle{}:
      {==}
    case M.Running{}:
      {==}
    case M.Completed{}:
      {==}
    case M.Interrupted{}:
      {==}
    case M.Failed{}:
      {==}

def Laws.settle_respects_unsettle(busy, archived, stale):
  match busy archived:
    case False{} False{}:
      {==}
    case False{} True{}:
      {==}
    case True{} False{}:
      {==}
    case True{} True{}:
      {==}

def Laws.sidebar_split_at_count(ts, navs, now):
  match ts:
    case Nil{}:
      {==}
    case +t <> +rest:
      Equal.trans(Nat, M.Shelves.count(M.Shelves.put(M.Shelf.at(t, M.Nav.until(M.Nav.get(navs, M.Thread.id(t))), now), t, M.Sidebar.split.at(rest, navs, now))),
        1n+M.Shelves.count(M.Sidebar.split.at(rest, navs, now)), 1n+List.length(&2, M.Thread, rest),
        put_count(M.Shelf.at(t, M.Nav.until(M.Nav.get(navs, M.Thread.id(t))), now), t, M.Sidebar.split.at(rest, navs, now)),
        Equal.cong(Nat, Nat, x => 1n+x, M.Shelves.count(M.Sidebar.split.at(rest, navs, now)), List.length(&2, M.Thread, rest),
          Laws.sidebar_split_at_count(rest, navs, now)))

def Laws.settle_never_snoozed(now, w, t, until, h):
  %Equal.sym(Bool, Nat.is_ge(now, until), False{}, h) : {M.Settle.auto.s(Bool.not(_), now, w, t, until) == False{} : Bool}
  {==}

def Laws.snooze_wakes(now, until, h):
  %Equal.sym(Bool, Nat.is_ge(now, until), True{}, h) : {Bool.not(_) == False{} : Bool}
  {==}

def Laws.replay_snoc(es, s, e):
  match es:
    case Nil{}:
      {==}
    case h <> t:
      Laws.replay_snoc(t, M.State.apply(s, h), e)

def Laws.sidebar_split_count(ts):
  match ts:
    case Nil{}:
      {==}
    case +t <> +rest:
      Equal.trans(Nat, M.Shelves.count(M.Shelves.put(M.Shelf.of(t), t, M.Sidebar.split(rest))),
        1n+M.Shelves.count(M.Sidebar.split(rest)), 1n+List.length(&2, M.Thread, rest),
        put_count(M.Shelf.of(t), t, M.Sidebar.split(rest)),
        Equal.cong(Nat, Nat, x => 1n+x, M.Shelves.count(M.Sidebar.split(rest)), List.length(&2, M.Thread, rest),
          Laws.sidebar_split_count(rest)))

# Viewer lemmas
# -------------

law sub_self:
  for a: Nat
  {Nat.sub(a, a) == 0n : Nat}

def sub_self(a):
  match a:
    case 0n:
      {==}
    case 1n+p:
      sub_self(p)

law key_fin_rec:
  for -h1: Char
  for -h2: Char
  for r: (String & String) & Cmp
  {V.Key.cmp.fin(String.cmp.rec(h1, h2, r)) == V.Key.cmp.fin(r) : Cmp}

def key_fin_rec(h1, h2, r):
  match r:
    case (ab, c):
      match ab:
        case (a, b):
          {==}

# a key is in order with itself
law key_cmp_refl:
  for +s: String
  {V.Key.cmp(s, s) == EQ{} : Cmp}

def key_cmp_refl(s):
  match s:
    case SNil{}:
      {==}
    case SCon{+h, +t}:
      match h:
        case Chr{+x}:
          %Equal.sym(Cmp, U32.cmp(x, x), EQ{}, u32_cmp_refl(x)) : {V.Key.cmp.fin(String.cmp.fin(t, t, ((Chr{x}, Chr{x}), _))) == EQ{} : Cmp}
          Equal.trans(Cmp, V.Key.cmp.fin(String.cmp.rec(Chr{x}, Chr{x}, String.cmp(t, t))),
            V.Key.cmp.fin(String.cmp(t, t)), EQ{},
            key_fin_rec(Chr{x}, Chr{x}, String.cmp(t, t)), key_cmp_refl(t))

law u32_eq_refl:
  for +x: U32
  {U32.is_eq(x, x) == True{} : Bool}

def u32_eq_refl(x):
  %Equal.sym(Cmp, U32.cmp(x, x), EQ{}, u32_cmp_refl(x)) : {Cmp.is_eq(_) == True{} : Bool}
  {==}

# the scene order puts an item level with itself
law scene_cmp_refl:
  for +k: String
  for +l: U32
  {V.Scene.cmp.k(U32.cmp(l, l), k, k) == EQ{} : Cmp}

def scene_cmp_refl(k, l):
  %Equal.sym(Cmp, U32.cmp(l, l), EQ{}, u32_cmp_refl(l)) : {V.Scene.cmp.k(_, k, k) == EQ{} : Cmp}
  key_cmp_refl(k)

# Viewer laws
# -----------

def Laws.fade_done(born, k):
  %Equal.sym(Bool, Nat.is_ge(Nat.add(Nat.add(born, V.Fade.ms()), k), Nat.add(born, V.Fade.ms())), True{},
    ge_add(Nat.add(born, V.Fade.ms()), Nat.add(born, V.Fade.ms()), k, Nat.ge_refl(Nat.add(born, V.Fade.ms()))))
    : {V.Fade.alpha.k(_, Nat.sub(Nat.add(Nat.add(born, V.Fade.ms()), k), born)) == 255 : U32}
  {==}

# on the dark ground the look is the kind's own, comparison or not
law look_dark_own:
  for d: Bool
  for +kind: U32
  for +layer: U32
  {LO.Look.of.t.d(d, False{}, kind, layer) == LO.Look.of.d(d, kind, layer) : U32 & U32}

def look_dark_own(d, kind, layer):
  match d:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.look_flip_none(kind, layer):
  look_dark_own(LO.Look.is_diff(kind), kind, layer)

def Laws.look_flip_hides():
  {==}

def Laws.look_flip_shows():
  {==}

def Laws.look_light_same_alpha():
  {==}

def Laws.look_grounds_not_layers():
  {==}

def Laws.theme_dark_as_is(c):
  {==}

def Laws.look_flip_twice():
  {==}

def Laws.ops_hidden_draws_nothing(p, l, v, c, now, born, kind, flip, rest, h):
  %Equal.sym(Bool, V.Look.shows(kind, l, flip), False{}, h)
    : {V.Ops.cull(Bool.and(_, Nat.is_lt(U32.to_nat(l), V.Look.count(kind))), p, l, v, c, V.Fade.alpha(now, born), kind, flip, rest) == rest : List<&2, R.Op>}
  {==}

def Laws.pick_hidden_skipped(hit, area, key, bk, ba):
  match hit:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.fade_start(born):
  %Equal.sym(Bool, Nat.is_ge(born, Nat.add(born, V.Fade.ms())), False{}, ge_past(born, 219n))
    : {V.Fade.alpha.k(_, Nat.sub(born, born)) == 0 : U32}
  %Equal.sym(Nat, Nat.sub(born, born), 0n, sub_self(born)) : {V.Fade.alpha.k(False{}, _) == 0 : U32}
  {==}

def Laws.scene_merge_self(ss, now):
  match ss:
    case Nil{}:
      {==}
    case V.Seen{B.Item{+k, +s, +l, +p, +i}, +b} <> +t:
      %Equal.sym(Cmp, V.Scene.cmp.k(U32.cmp(l, l), k, k), EQ{}, scene_cmp_refl(k, l))
        : {V.Scene.merge(V.Seen{B.Item{k, s, l, p, i}, b} <> t, B.Item{k, s, l, p, i} <> V.Scene.items(t), _, now) == V.Seen{B.Item{k, s, l, p, i}, b} <> t : List<&2, V.Seen>}
      %Equal.sym(Bool, U32.is_eq(s, s), True{}, u32_eq_refl(s))
        : {V.Scene.keep(_, B.Item{k, s, l, p, i}, b, now) <> V.Scene.merge(t, V.Scene.items(t), V.Scene.heads(t, V.Scene.items(t)), now) == V.Seen{B.Item{k, s, l, p, i}, b} <> t : List<&2, V.Seen>}
      Equal.cong(List<&2, V.Seen>, List<&2, V.Seen>, xs => V.Seen{B.Item{k, s, l, p, i}, b} <> xs,
        V.Scene.merge(t, V.Scene.items(t), V.Scene.heads(t, V.Scene.items(t)), now), t, Laws.scene_merge_self(t, now))

# Update lemmas and laws
# ----------------------

law nat_cmp_refl:
  for a: Nat
  {Nat.cmp(a, a) == EQ{} : Cmp}

def nat_cmp_refl(a):
  match a:
    case 0n:
      {==}
    case 1n+p:
      nat_cmp_refl(p)

law pre_cmp_refl:
  for b: Bool
  {SV.Ver.pre_cmp(b, b) == EQ{} : Cmp}

def pre_cmp_refl(b):
  match b:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.semver_cmp_refl(v):
  match v:
    case SV.Ver{+a, +b, +c, +p, +d, +n}:
      %Equal.sym(Cmp, Nat.cmp(a, a), EQ{}, nat_cmp_refl(a)) : {SV.Ver.then(_, SV.Ver.then(Nat.cmp(b, b), SV.Ver.then(Nat.cmp(c, c), SV.Ver.then(SV.Ver.pre_cmp(p, p), SV.Ver.then(Nat.cmp(d, d), Nat.cmp(n, n)))))) == EQ{} : Cmp}
      %Equal.sym(Cmp, Nat.cmp(b, b), EQ{}, nat_cmp_refl(b)) : {SV.Ver.then(_, SV.Ver.then(Nat.cmp(c, c), SV.Ver.then(SV.Ver.pre_cmp(p, p), SV.Ver.then(Nat.cmp(d, d), Nat.cmp(n, n))))) == EQ{} : Cmp}
      %Equal.sym(Cmp, Nat.cmp(c, c), EQ{}, nat_cmp_refl(c)) : {SV.Ver.then(_, SV.Ver.then(SV.Ver.pre_cmp(p, p), SV.Ver.then(Nat.cmp(d, d), Nat.cmp(n, n)))) == EQ{} : Cmp}
      %Equal.sym(Cmp, SV.Ver.pre_cmp(p, p), EQ{}, pre_cmp_refl(p)) : {SV.Ver.then(_, SV.Ver.then(Nat.cmp(d, d), Nat.cmp(n, n))) == EQ{} : Cmp}
      %Equal.sym(Cmp, Nat.cmp(d, d), EQ{}, nat_cmp_refl(d)) : {SV.Ver.then(_, Nat.cmp(n, n)) == EQ{} : Cmp}
      nat_cmp_refl(n)

def Laws.update_never_same(v):
  %Equal.sym(Cmp, SV.Ver.cmp(v, v), EQ{}, Laws.semver_cmp_refl(v)) : {Cmp.is_gt(_) == False{} : Bool}
  {==}

def Laws.update_off_managed(c, t, a):
  {==}

def Laws.update_appimage_managed(o, c, t):
  match o:
    case SNil{}:
      {==}
    case SCon{h, r}:
      {==}

law nat_cmp_succ:
  for n: Nat
  {Nat.cmp(1n+n, n) == GT{} : Cmp}

def nat_cmp_succ(n):
  match n:
    case 0n:
      {==}
    case 1n+p:
      nat_cmp_succ(p)

def Laws.semver_pre_before_release(a, b, c, d, n):
  %Equal.sym(Cmp, Nat.cmp(a, a), EQ{}, nat_cmp_refl(a)) : {Cmp.is_gt(SV.Ver.then(_, SV.Ver.then(Nat.cmp(b, b), SV.Ver.then(Nat.cmp(c, c), SV.Ver.then(GT{}, SV.Ver.then(Nat.cmp(0n, d), Nat.cmp(0n, n))))))) == True{} : Bool}
  %Equal.sym(Cmp, Nat.cmp(b, b), EQ{}, nat_cmp_refl(b)) : {Cmp.is_gt(SV.Ver.then(_, SV.Ver.then(Nat.cmp(c, c), SV.Ver.then(GT{}, SV.Ver.then(Nat.cmp(0n, d), Nat.cmp(0n, n)))))) == True{} : Bool}
  %Equal.sym(Cmp, Nat.cmp(c, c), EQ{}, nat_cmp_refl(c)) : {Cmp.is_gt(SV.Ver.then(_, SV.Ver.then(GT{}, SV.Ver.then(Nat.cmp(0n, d), Nat.cmp(0n, n))))) == True{} : Bool}
  {==}

def Laws.semver_nightly_later(a, b, c, d, n):
  %Equal.sym(Cmp, Nat.cmp(a, a), EQ{}, nat_cmp_refl(a)) : {Cmp.is_gt(SV.Ver.then(_, SV.Ver.then(Nat.cmp(b, b), SV.Ver.then(Nat.cmp(c, c), SV.Ver.then(EQ{}, SV.Ver.then(Nat.cmp(d, d), Nat.cmp(1n+n, n))))))) == True{} : Bool}
  %Equal.sym(Cmp, Nat.cmp(b, b), EQ{}, nat_cmp_refl(b)) : {Cmp.is_gt(SV.Ver.then(_, SV.Ver.then(Nat.cmp(c, c), SV.Ver.then(EQ{}, SV.Ver.then(Nat.cmp(d, d), Nat.cmp(1n+n, n)))))) == True{} : Bool}
  %Equal.sym(Cmp, Nat.cmp(c, c), EQ{}, nat_cmp_refl(c)) : {Cmp.is_gt(SV.Ver.then(_, SV.Ver.then(EQ{}, SV.Ver.then(Nat.cmp(d, d), Nat.cmp(1n+n, n))))) == True{} : Bool}
  %Equal.sym(Cmp, Nat.cmp(d, d), EQ{}, nat_cmp_refl(d)) : {Cmp.is_gt(SV.Ver.then(EQ{}, SV.Ver.then(_, Nat.cmp(1n+n, n)))) == True{} : Bool}
  %Equal.sym(Cmp, Nat.cmp(1n+n, n), GT{}, nat_cmp_succ(n)) : {Cmp.is_gt(SV.Ver.then(EQ{}, SV.Ver.then(EQ{}, _))) == True{} : Bool}
  {==}

def Laws.semver_nightly_next_day(a, b, c, d, n, m):
  %Equal.sym(Cmp, Nat.cmp(a, a), EQ{}, nat_cmp_refl(a)) : {Cmp.is_gt(SV.Ver.then(_, SV.Ver.then(Nat.cmp(b, b), SV.Ver.then(Nat.cmp(c, c), SV.Ver.then(EQ{}, SV.Ver.then(Nat.cmp(1n+d, d), Nat.cmp(m, n))))))) == True{} : Bool}
  %Equal.sym(Cmp, Nat.cmp(b, b), EQ{}, nat_cmp_refl(b)) : {Cmp.is_gt(SV.Ver.then(_, SV.Ver.then(Nat.cmp(c, c), SV.Ver.then(EQ{}, SV.Ver.then(Nat.cmp(1n+d, d), Nat.cmp(m, n)))))) == True{} : Bool}
  %Equal.sym(Cmp, Nat.cmp(c, c), EQ{}, nat_cmp_refl(c)) : {Cmp.is_gt(SV.Ver.then(_, SV.Ver.then(EQ{}, SV.Ver.then(Nat.cmp(1n+d, d), Nat.cmp(m, n))))) == True{} : Bool}
  %Equal.sym(Cmp, Nat.cmp(1n+d, d), GT{}, nat_cmp_succ(d)) : {Cmp.is_gt(SV.Ver.then(EQ{}, SV.Ver.then(EQ{}, SV.Ver.then(_, Nat.cmp(m, n))))) == True{} : Bool}
  {==}

# Attachment lemmas and laws
# --------------------------

def Laws.file_bytes_success(bs):
  {==}

def Laws.file_bytes_failure(code, why):
  {==}

law attach_chr_ok:
  for b: Bool
  for +c: U32
  for e: {AT.Att.ok(c) == b : Bool}
  {AT.Att.ok(AT.Att.chr(b, c)) == True{} : Bool}

def attach_chr_ok(b, c, e):
  match b:
    case True{}:
      e
    case False{}:
      {==}

def Laws.attach_clean_ok(s):
  match s:
    case SNil{}:
      {==}
    case SCon{Chr{+c}, t}:
      %Equal.sym(Bool, AT.Att.ok(AT.Att.chr(AT.Att.ok(c), c)), True{}, attach_chr_ok(AT.Att.ok(c), c, {==})) : {Bool.and(_, AT.Att.all_ok(AT.Att.clean(t))) == True{} : Bool}
      Laws.attach_clean_ok(t)

def Laws.drop_non_file_ignored(c, s, rest, h):
  %Equal.sym(Bool, Char.is_eq(c, 'f'), False{}, h) : {AT.Att.uri.keep(String.starts_with.if(s, "ile:", _), SCon{c, s}, rest) == rest : List<&2, String>}
  {==}

def Laws.drop_uri_list_example():
  {==}

# Orchestration laws
# ------------------

def Laws.task_terminal_absorbing(cur, want, h):
  match cur:
    case M.TaskQueued{}:
      Empty.absurd({M.TaskState.next(M.TaskQueued{}, want) == M.TaskQueued{} : M.TaskState}, false_ne_true(h))
    case M.TaskRunning{}:
      Empty.absurd({M.TaskState.next(M.TaskRunning{}, want) == M.TaskRunning{} : M.TaskState}, false_ne_true(h))
    case M.TaskDone{}:
      {==}
    case M.TaskFailed{}:
      {==}
    case M.TaskCancelled{}:
      {==}

def Laws.task_never_requeued():
  {==}

def Laws.task_delivered_once(t):
  match t:
    case M.Task{_, _, _, _, st, _, _, _, _}:
      match st:
        case M.TaskQueued{}:
          {==}
        case M.TaskRunning{}:
          {==}
        case M.TaskDone{}:
          {==}
        case M.TaskFailed{}:
          {==}
        case M.TaskCancelled{}:
          {==}

# n + k is never less than n
law lt_add_self:
  for n: Nat
  for +k: Nat
  {Nat.is_lt(Nat.add(n, k), n) == False{} : Bool}

def lt_add_self(n, k):
  match n:
    case 0n:
      match k:
        case 0n:
          {==}
        case 1n+p:
          {==}
    case 1n+p:
      lt_add_self(p, k)

def Laws.delegation_bounded(k):
  lt_add_self(M.Task.max_depth(), k)

def Laws.setting_default(k, d):
  {==}

def Laws.nui_with_own_vw(n):
  match n:
    case N.Nui{u, w, hh, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}:
      {==}

def Laws.stale_board_ignored(n, key, items, now, h):
  %Equal.sym(Bool, N.Vw.fresh_for(N.Nui.vw(n), key), False{}, h) : {N.Nui.with_vw(n, N.Vw.if(_, N.Nui.vw(n), v => N.Vw.board.same(v, key, items, now))) == n : N.Nui}
  Laws.nui_with_own_vw(n)

def Laws.resume_is_replay(a, b, s):
  match a:
    case Nil{}:
      {==}
    case h <> t:
      Laws.resume_is_replay(t, b, M.State.apply(s, h))

def Laws.post_running_delivers(s, now, th, msg, role, text, h1, h2):
  %Equal.sym(Bool, H.Hub.running(H.Hub.thread(s, th)), True{}, h1) : {H.Hub.post.w(_, M.Worktree.gone(H.Hub.thread(s, th)), M.Worktree.pending(H.Hub.thread(s, th), now), s, now, th, msg, role, text) == H.Hub.deliver.r(True{}, s, now, th, msg, role, text) : H.Step}
  %Equal.sym(Bool, M.Worktree.pending(H.Hub.thread(s, th), now), False{}, h2) : {H.Hub.deliver(True{}, _, s, now, th, msg, role, text) == H.Hub.deliver.r(True{}, s, now, th, msg, role, text) : H.Step}
  {==}

def Laws.deliver_held_go(b, th, now):
  match b:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.deliver_held_steer(b1, b2, th, id, now):
  match b1 b2:
    case True{} True{}:
      {==}
    case True{} False{}:
      {==}
    case False{} True{}:
      {==}
    case False{} False{}:
      {==}

def Laws.deliver_held_restart(b1, b2, b3, th, id, now, v, r):
  match b1 b2 b3:
    case True{} True{} True{}:
      {==}
    case True{} True{} False{}:
      {==}
    case True{} False{} True{}:
      {==}
    case True{} False{} False{}:
      {==}
    case False{} True{} True{}:
      {==}
    case False{} True{} False{}:
      {==}
    case False{} False{} True{}:
      {==}
    case False{} False{} False{}:
      {==}

# a running non-Claude turn: the message rides (nothing held), or, with an
# older log's queue, the turn restarts (Laws.deliver_held_restart)
law ride_held_k:
  for q: Bool
  for +s: M.State
  for +now: Nat
  for +th: String
  for +msg: String
  for role: M.Role
  for +text: String
  {H.Deliver.held(H.Queue.changes(H.Hub.deliver.ride(q, s, now, th, msg, role, text))) == False{} : Bool}

def ride_held_k(q, s, now, th, msg, role, text):
  match q:
    case True{}:
      {==}
    case False{}:
      Laws.deliver_held_restart(String.is_empty(M.Queue.of(s, th)), String.eq(H.Hub.provider(H.Hub.thread(s, th)), "claude"), String.is_empty(H.Ride.of(s, th)), th, H.Hub.id("s", now, H.Hub.seq(s)), now, H.Ride.of(s, th), "with the steer")

# an idle thread: the turn starts, with whatever rode (nothing held)
law ride_post_held:
  for e: Bool
  for +s: M.State
  for +now: Nat
  for +th: String
  for +msg: String
  for role: M.Role
  for +text: String
  {H.Deliver.held(H.Queue.changes(H.Ride.post(e, s, now, th, msg, role, text))) == False{} : Bool}

def ride_post_held(e, s, now, th, msg, role, text):
  match e:
    case True{}:
      Laws.deliver_held_go(String.is_empty(M.Queue.of(s, th)), th, now)
    case False{}:
      Laws.deliver_held_go(String.is_empty(M.Queue.join(M.Queue.of(s, th), RD.Ride.input(H.Ride.of(s, th)))), th, now)

def Laws.deliver_held_steer_k(c, s, now, th, msg, role, text):
  match c:
    case True{}:
      Laws.deliver_held_go(String.is_empty(M.Queue.of(s, th)), th, now)
    case False{}:
      ride_held_k(String.is_empty(M.Queue.of(s, th)), s, now, th, msg, role, text)

def Laws.deliver_never_waits(run, s, now, th, msg, role, text):
  match run:
    case True{}:
      Laws.deliver_held_steer_k(String.eq(H.Hub.provider(H.Hub.thread(s, th)), "claude"), s, now, th, msg, role, text)
    case False{}:
      ride_post_held(String.is_empty(H.Ride.of(s, th)), s, now, th, msg, role, text)

def Laws.deliver_claude_running():
  {==}

def Laws.resend_stored_once(s, now, client, id, th, text, msg, h1, h2):
  %Equal.sym(Bool, String.is_empty(msg), False{}, h1) : {H.Queue.changes(H.Hub.turn.m(s, now, client, id, th, text, Bool.pick(String, _, H.Hub.id("m", now, H.Hub.seq(s)), msg), Bool.not(_))) == [] : List<&2, M.Change>}
  %Equal.sym(Bool, H.Hub.has_msg(H.Hub.entries(s), msg), True{}, h2) : {H.Queue.changes(H.Hub.turn.k(_, s, now, client, id, th, msg, text)) == [] : List<&2, M.Change>}
  {==}

def Laws.has_msg_unnamed_skips(s, msg):
  {==}

def Laws.mcp_last_newest():
  {==}

def Laws.task_finish_not_ended_same(s, now, thread, cs, os, h):
  %Equal.sym(H.Ended, H.Task.ended(cs), H.NotEnded{}, h) : {H.Task.finish.e(_, s, now, thread, cs, os) == H.Step{cs, os} : H.Step}
  {==}

def Laws.inbox_running_delivers(s, now, from, to, role, text, msg, h1, h2):
  %Equal.sym(Bool, H.Hub.running(H.Hub.thread(s, to)), True{}, h1) : {H.Hub.inbox.w(_, M.Worktree.gone(H.Hub.thread(s, to)), M.Worktree.pending(H.Hub.thread(s, to), now), s, now, from, to, role, text, msg) == H.Hub.deliver.r(True{}, s, now, to, msg, role, text) : H.Step}
  %Equal.sym(Bool, M.Worktree.pending(H.Hub.thread(s, to), now), False{}, h2) : {H.Hub.deliver(True{}, _, s, now, to, msg, role, text) == H.Hub.deliver.r(True{}, s, now, to, msg, role, text) : H.Step}
  {==}

law or_true_r:
  for b: Bool
  {Bool.or(b, True{}) == True{} : Bool}

def or_true_r(b):
  match b:
    case False{}:
      {==}
    case True{}:
      {==}

law and_false_r:
  for b: Bool
  {Bool.and(b, False{}) == False{} : Bool}

def and_false_r(b):
  match b:
    case False{}:
      {==}
    case True{}:
      {==}

def Laws.worktree_pending_local(wt, c, now):
  {==}

def Laws.worktree_pending_ends(e, wt, c, now, h):
  match e:
    case M.Worktree{}:
      %Equal.sym(Bool, Nat.is_lt(now, Nat.add(c, M.Worktree.wait())), False{}, h) : {Bool.and(_, String.is_empty(wt)) == False{} : Bool}
      {==}
    case M.Local{}:
      {==}

def Laws.worktree_waits(s, now, th, msg, role, text, h):
  %Equal.sym(Bool, M.Worktree.pending(H.Hub.thread(s, th), now), True{}, h) : {H.Step.outs_of(H.Hub.post.w(H.Hub.running(H.Hub.thread(s, th)), M.Worktree.gone(H.Hub.thread(s, th)), _, s, now, th, msg, role, text)) == [] : List<&2, H.Out>}
  %Equal.sym(Bool, Bool.and(M.Worktree.gone(H.Hub.thread(s, th)), False{}), False{}, and_false_r(M.Worktree.gone(H.Hub.thread(s, th)))) : {H.Step.outs_of(H.Hub.post.v(Bool.and(Bool.not(H.Hub.running(H.Hub.thread(s, th))), _), H.Hub.running(H.Hub.thread(s, th)), True{}, s, now, th, msg, role, text)) == [] : List<&2, H.Out>}
  %Equal.sym(Bool, Bool.and(Bool.not(H.Hub.running(H.Hub.thread(s, th))), False{}), False{}, and_false_r(Bool.not(H.Hub.running(H.Hub.thread(s, th))))) : {H.Step.outs_of(H.Hub.post.v(_, H.Hub.running(H.Hub.thread(s, th)), True{}, s, now, th, msg, role, text)) == [] : List<&2, H.Out>}
  {==}

def Laws.worktree_by_default():
  {==}

def Laws.worktree_revives(s, now, th, msg, role, text, h1, h2, h3):
  %Equal.sym(Bool, H.Hub.running(H.Hub.thread(s, th)), False{}, h1) : {H.Step.outs_of(H.Hub.post.w(_, M.Worktree.gone(H.Hub.thread(s, th)), M.Worktree.pending(H.Hub.thread(s, th), now), s, now, th, msg, role, text)) == [H.Wt.revive(s, th)] : List<&2, H.Out>}
  %Equal.sym(Bool, M.Worktree.gone(H.Hub.thread(s, th)), True{}, h2) : {H.Step.outs_of(H.Hub.post.w(False{}, _, M.Worktree.pending(H.Hub.thread(s, th), now), s, now, th, msg, role, text)) == [H.Wt.revive(s, th)] : List<&2, H.Out>}
  %Equal.sym(Bool, M.Worktree.pending(H.Hub.thread(s, th), now), False{}, h3) : {H.Step.outs_of(H.Hub.post.w(False{}, True{}, _, s, now, th, msg, role, text)) == [H.Wt.revive(s, th)] : List<&2, H.Out>}
  {==}

def Laws.worktree_kept_while_busy(sh, ours, merged, aged):
  {==}

def Laws.worktree_kept_out(busy, sh, ours, merged, aged, h):
  match busy sh:
    case True{} _:
      {==}
    case False{} M.Settled{}:
      Empty.absurd({M.Worktree.drop.k(False{}, M.Settled{}, ours, merged, aged) == False{} : Bool}, false_ne_true(Equal.sym(Bool, True{}, False{}, h)))
    case False{} M.Hidden{}:
      Empty.absurd({M.Worktree.drop.k(False{}, M.Hidden{}, ours, merged, aged) == False{} : Bool}, false_ne_true(Equal.sym(Bool, True{}, False{}, h)))
    case False{} M.Pinned{}:
      {==}
    case False{} M.Active{}:
      {==}
    case False{} M.Snoozed{}:
      {==}

law worktree_drop_not_ours:
  for busy: Bool
  for sh: M.Shelf
  for merged: Bool
  for aged: Bool
  {M.Worktree.drop.k(busy, sh, False{}, merged, aged) == False{} : Bool}

def worktree_drop_not_ours(busy, sh, merged, aged):
  match busy sh:
    case True{} _:
      {==}
    case False{} M.Settled{}:
      {==}
    case False{} M.Hidden{}:
      {==}
    case False{} M.Pinned{}:
      {==}
    case False{} M.Active{}:
      {==}
    case False{} M.Snoozed{}:
      {==}

def Laws.worktree_kept_unless_ours(busy, sh, wt, merged, aged, h):
  %Equal.sym(Bool, M.Worktree.ours(wt), False{}, h) : {M.Worktree.drop.k(busy, sh, _, merged, aged) == False{} : Bool}
  worktree_drop_not_ours(busy, sh, merged, aged)

def Laws.worktree_kept_fresh(ours):
  and_false_r(ours)

def Laws.worktree_dropped_on_merge(aged):
  {==}

def Laws.worktree_dropped_when_aged(merged):
  or_true_r(merged)

def Laws.worktree_dropped_when_hidden(merged):
  {==}

def Laws.merge_settles(t, at, h):
  match t:
    case M.Thread{id, p, ti, c, a, tu, pe, False{}, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, False{}}:
      {==}
    case M.Thread{id, p, ti, c, a, tu, pe, False{}, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, True{}}:
      Empty.absurd({M.Shelf.of(M.Thread.settled(True{}, at, M.Thread{id, p, ti, c, a, tu, pe, False{}, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, True{}})) == M.Settled{} : M.Shelf},
        false_ne_true(Equal.sym(Bool, True{}, False{}, h)))
    case M.Thread{id, p, ti, c, a, tu, pe, True{}, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de}:
      Empty.absurd({M.Shelf.of(M.Thread.settled(True{}, at, M.Thread{id, p, ti, c, a, tu, pe, True{}, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de})) == M.Settled{} : M.Shelf},
        false_ne_true(Equal.sym(Bool, True{}, False{}, h)))

def Laws.merge_once(sh, branch, url, h):
  match sh:
    case M.Pinned{}:
      %Equal.sym(Bool, String.is_empty(url), False{}, h) : {Bool.and(Bool.not(String.is_empty(branch)), _) == False{} : Bool}
      and_false_r(Bool.not(String.is_empty(branch)))
    case M.Active{}:
      %Equal.sym(Bool, String.is_empty(url), False{}, h) : {Bool.and(Bool.not(String.is_empty(branch)), _) == False{} : Bool}
      and_false_r(Bool.not(String.is_empty(branch)))
    case M.Settled{}:
      {==}
    case M.Hidden{}:
      {==}
    case M.Snoozed{}:
      {==}

def Laws.merge_needs_branch(sh, merged):
  match sh:
    case M.Pinned{}:
      {==}
    case M.Active{}:
      {==}
    case M.Settled{}:
      {==}
    case M.Hidden{}:
      {==}
    case M.Snoozed{}:
      {==}

def Laws.gh_pick_saved(owner_in, saved, owner, active):
  {==}

def Laws.gh_pick_owner(saved, owner, active):
  {==}

def Laws.gh_pick_active(saved, owner, active):
  {==}

def Laws.gh_order_first(first, us):
  {==}

law gh_without_step:
  for b: Bool
  for +h: String
  for +t: List<&2, String>
  for +u: String
  for +e: {String.eq(h, u) == b : Bool}
  for ih: {GH.Gh.has(GH.Gh.without(t, u), u) == False{} : Bool}
  {GH.Gh.has(Bool.pick(List<&2, String>, String.eq(h, u), GH.Gh.without(t, u), h <> GH.Gh.without(t, u)), u) == False{} : Bool}

def gh_without_step(b, h, t, u, e, ih):
  match b:
    case True{}:
      %Equal.sym(Bool, String.eq(h, u), True{}, e) : {GH.Gh.has(Bool.pick(List<&2, String>, _, GH.Gh.without(t, u), h <> GH.Gh.without(t, u)), u) == False{} : Bool}
      ih
    case False{}:
      %Equal.sym(Bool, String.eq(h, u), False{}, e) : {GH.Gh.has(Bool.pick(List<&2, String>, _, GH.Gh.without(t, u), h <> GH.Gh.without(t, u)), u) == False{} : Bool}
      %Equal.sym(Bool, String.eq(h, u), False{}, e) : {Bool.or(_, GH.Gh.has(GH.Gh.without(t, u), u)) == False{} : Bool}
      ih

def Laws.gh_without_drops(us, u):
  match us:
    case Nil{}:
      {==}
    case +h <> +t:
      gh_without_step(String.eq(h, u), h, t, u, {==}, Laws.gh_without_drops(t, u))

def Laws.alert_needs_running(t):
  {==}

def Laws.alert_never_running(b):
  match b:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.alert_not_on_stop(b):
  match b:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.alert_none_on_connect(ts, st, es, desk):
  match ts:
    case Nil{}:
      {==}
    case M.Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de} <> t:
      Laws.alert_none_on_connect(t, st, es, desk)

def Laws.notice_fail_always(bot, asking, posted, first, msg):
  {==}

def Laws.notice_thread_kept(asking, posted, first, msg):
  {==}

def Laws.notice_ask_kept(posted, first, msg):
  {==}

def Laws.notice_room_once(msg):
  {==}

def Laws.notice_chatter_quiet(first):
  {==}

def Laws.notice_earlier_bot_quiet():
  {==}

def Laws.notice_person_opens(own, bot):
  {==}

def Laws.notice_window_opens(room, name, start, local, p):
  {==}

def Laws.hubs_no_id(k):
  {==}

def Laws.q_kind_names():
  {==}

def Laws.hubs_unpaired_quiet(cid, now, cur, t):
  {==}

def Laws.island_idle_empty(ts, es, h):
  match ts:
    case Nil{}:
      {==}
    case M.Thread{id, p, ti, c, a, M.Running{}, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de} <> t:
      Empty.absurd({Nt.Note.lines(M.Thread{id, p, ti, c, a, M.Running{}, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de} <> t, es) == [] : List<&2, J.Json>},
        false_ne_true(Equal.sym(Bool, True{}, False{}, h)))
    case M.Thread{id, p, ti, c, a, M.Idle{}, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de} <> t:
      Laws.island_idle_empty(t, es, h)
    case M.Thread{id, p, ti, c, a, M.Completed{}, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de} <> t:
      Laws.island_idle_empty(t, es, h)
    case M.Thread{id, p, ti, c, a, M.Interrupted{}, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de} <> t:
      Laws.island_idle_empty(t, es, h)
    case M.Thread{id, p, ti, c, a, M.Failed{}, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de} <> t:
      Laws.island_idle_empty(t, es, h)

def Laws.device_token_never_sent(tok, p, k, th, e, b, at):
  {==}

def Laws.device_drop_never_sent(tok, at):
  {==}

def Laws.device_blank_ignored(s, p, k, th, e, b, at):
  match s:
    case M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw}:
      {==}

def Laws.device_register_once(tok, h, p, k, th, e, b, at, p2, k2, th2, e2, b2, at2):
  match tok:
    case SNil{}:
      Empty.absurd({Map.size(&2, M.Device, M.State.devices(M.State.apply(M.State.apply(M.State.empty(), M.DeviceRegistered{SNil{}, p, k, th, e, b, at}), M.DeviceRegistered{SNil{}, p2, k2, th2, e2, b2, at2}))) == 1n : Nat},
        false_ne_true(Equal.sym(Bool, True{}, False{}, h)))
    case SCon{+c, +t}:
      %Equal.sym((String & String) & Cmp, String.cmp(SCon{c, t}, SCon{c, t}), ((SCon{c, t}, SCon{c, t}), EQ{}), string_cmp_refl(SCon{c, t})) : {Map.size(&2, M.Device, Map.set.fin.go(&2, M.Device, MLeaf{SCon{c, t}, M.Device{SCon{c, t}, p, k, th, e, b, at}}, SCon{c, t}, M.Device{SCon{c, t}, p2, k2, th2, e2, b2, at2}, _)) == 1n : Nat}
      {==}

# Order keys
# ----------

law cmpz_r_not_lt:
  for s: List<&2, Bool>
  {Cmp.is_lt(M.Key.cmpz_r(s)) == False{} : Bool}

def cmpz_r_not_lt(s):
  match s:
    case Nil{}:
      {==}
    case x <> r:
      match x:
        case False{}:
          cmpz_r_not_lt(r)
        case True{}:
          {==}

def Laws.key_after(s):
  match s:
    case Nil{}:
      {==}
    case x <> r:
      match x:
        case False{}:
          {==}
        case True{}:
          Laws.key_after(r)

law midz_above:
  for t: List<&2, Bool>
  for h: {Cmp.is_lt(M.Key.cmpz(t)) == True{} : Bool}
  {Cmp.is_lt(M.Key.cmpz(M.Key.midz(t))) == True{} : Bool}

def midz_above(t, h):
  match t:
    case Nil{}:
      Empty.absurd({Cmp.is_lt(M.Key.cmpz(M.Key.midz(Nil{}))) == True{} : Bool}, false_ne_true(h))
    case x <> r:
      match x:
        case False{}:
          midz_above(r, h)
        case True{}:
          {==}

law midz_below:
  for t: List<&2, Bool>
  for h: {Cmp.is_lt(M.Key.cmpz(t)) == True{} : Bool}
  {M.Key.lt(M.Key.midz(t), t) == True{} : Bool}

def midz_below(t, h):
  match t:
    case Nil{}:
      Empty.absurd({M.Key.lt(M.Key.midz(Nil{}), Nil{}) == True{} : Bool}, false_ne_true(h))
    case x <> r:
      match x:
        case False{}:
          midz_below(r, h)
        case True{}:
          {==}

def Laws.key_mid_above(a, b, h):
  match a:
    case Nil{}:
      midz_above(b, h)
    case x <> s:
      match x b:
        case False{} Nil{}:
          Empty.absurd({M.Key.lt(False{} <> s, M.Key.mid(False{} <> s, Nil{})) == True{} : Bool},
            false_ne_true(Equal.trans(Bool, False{}, Cmp.is_lt(M.Key.cmpz_r(s)), True{}, Equal.sym(Bool, Cmp.is_lt(M.Key.cmpz_r(s)), False{}, cmpz_r_not_lt(s)), h)))
        case True{} Nil{}:
          Empty.absurd({M.Key.lt(True{} <> s, M.Key.mid(True{} <> s, Nil{})) == True{} : Bool}, false_ne_true(h))
        case False{} y <> t:
          match y:
            case False{}:
              Laws.key_mid_above(s, t, h)
            case True{}:
              Laws.key_after(s)
        case True{} y <> t:
          match y:
            case False{}:
              Empty.absurd({M.Key.lt(True{} <> s, M.Key.mid(True{} <> s, False{} <> t)) == True{} : Bool}, false_ne_true(h))
            case True{}:
              Laws.key_mid_above(s, t, h)

def Laws.key_mid_below(a, b, h):
  match a:
    case Nil{}:
      midz_below(b, h)
    case x <> s:
      match x b:
        case False{} Nil{}:
          Empty.absurd({M.Key.lt(M.Key.mid(False{} <> s, Nil{}), Nil{}) == True{} : Bool},
            false_ne_true(Equal.trans(Bool, False{}, Cmp.is_lt(M.Key.cmpz_r(s)), True{}, Equal.sym(Bool, Cmp.is_lt(M.Key.cmpz_r(s)), False{}, cmpz_r_not_lt(s)), h)))
        case True{} Nil{}:
          Empty.absurd({M.Key.lt(M.Key.mid(True{} <> s, Nil{}), Nil{}) == True{} : Bool}, false_ne_true(h))
        case False{} y <> t:
          match y:
            case False{}:
              Laws.key_mid_below(s, t, h)
            case True{}:
              {==}
        case True{} y <> t:
          match y:
            case False{}:
              Empty.absurd({M.Key.lt(M.Key.mid(True{} <> s, False{} <> t), False{} <> t) == True{} : Bool}, false_ne_true(h))
            case True{}:
              Laws.key_mid_below(s, t, h)

# Sidebar status
# --------------

def Laws.project_keyed_first(x, xs, a, b):
  {==}

def Laws.project_unkeyed_below(a, y, ys, b):
  {==}

def Laws.status_approval_first(i, t, q):
  {==}

def Laws.status_input_before_work(t, q):
  {==}

def Laws.status_working_before_queued(q):
  {==}

def Laws.status_failed_before_queued(q):
  {==}

def Laws.status_approval_top(a, i, t, q):
  match a:
    case True{}:
      {==}
    case False{}:
      match i:
        case True{}:
          {==}
        case False{}:
          match t q:
            case M.Running{} _:
              {==}
            case M.Failed{} _:
              {==}
            case M.Idle{} True{}:
              {==}
            case M.Idle{} False{}:
              {==}
            case M.Completed{} True{}:
              {==}
            case M.Completed{} False{}:
              {==}
            case M.Interrupted{} True{}:
              {==}
            case M.Interrupted{} False{}:
              {==}

def Laws.phase_approval_first(i, n, t, q):
  {==}

def Laws.phase_input_before_work(n, t, q):
  {==}

def Laws.phase_working_before_tasks(n, q):
  {==}

def Laws.phase_monitoring_before_complete(n, q):
  {==}

def Laws.phase_complete():
  {==}

def Laws.phase_failed_before_queued():
  {==}

def Laws.phase_silent_only_idle(a, i, n, t, q):
  match a:
    case True{}:
      {==}
    case False{}:
      match i:
        case True{}:
          {==}
        case False{}:
          match n:
            case 0n:
              match t q:
                case M.Idle{} True{}:
                  {==}
                case M.Idle{} False{}:
                  {==}
                case M.Running{} True{}:
                  {==}
                case M.Running{} False{}:
                  {==}
                case M.Completed{} True{}:
                  {==}
                case M.Completed{} False{}:
                  {==}
                case M.Interrupted{} True{}:
                  {==}
                case M.Interrupted{} False{}:
                  {==}
                case M.Failed{} True{}:
                  {==}
                case M.Failed{} False{}:
                  {==}
            case 1n+k:
              match t:
                case M.Idle{}:
                  {==}
                case M.Running{}:
                  {==}
                case M.Completed{}:
                  {==}
                case M.Interrupted{}:
                  {==}
                case M.Failed{}:
                  {==}

def Laws.fuzzy_empty_matches(s):
  match s:
    case SNil{}:
      {==}
    case SCon{Chr{c}, t}:
      {==}

def Laws.find_empty_keeps(rs):
  match rs:
    case Nil{}:
      {==}
    case U.SRow{+r, M.Project{+id, +ti, +ro, +c, +rm}, +sh} <> +t:
      %Equal.sym(Bool, Nat.is_gt(FZ.Fuzzy.score(ti, ""), 0n), True{}, Laws.fuzzy_empty_matches(ti)) : {U.Find.keep(_, U.SRow{r, M.Project{id, ti, ro, c, rm}, sh}, U.Find.rows(t, "")) == U.SRow{r, M.Project{id, ti, ro, c, rm}, sh} <> t : List<&2, U.SRow>}
      %Equal.sym(List<&2, U.SRow>, U.Find.rows(t, ""), t, Laws.find_empty_keeps(t)) : {U.SRow{r, M.Project{id, ti, ro, c, rm}, sh} <> _ == U.SRow{r, M.Project{id, ti, ro, c, rm}, sh} <> t : List<&2, U.SRow>}
      {==}

def Laws.find_shut_shows_all(st, now, dr):
  Laws.find_empty_keeps(U.Side.rows(st, now))

def Laws.find_always_on(op, dr):
  {==}

def Laws.urg_approval_top(p):
  match p:
    case M.PhApproval{}:
      {==}
    case M.PhInput{}:
      {==}
    case M.PhWorking{}:
      {==}
    case M.PhMonitoring{n}:
      {==}
    case M.PhFailed{}:
      {==}
    case M.PhComplete{}:
      {==}
    case M.PhStopped{}:
      {==}
    case M.PhIdle{}:
      {==}

def Laws.urg_input_over_working(a, b):
  match a b:
    case True{} True{}:
      {==}
    case True{} False{}:
      {==}
    case False{} True{}:
      {==}
    case False{} False{}:
      {==}

def Laws.urg_done_over_working(a, b):
  match a b:
    case True{} True{}:
      {==}
    case True{} False{}:
      {==}
    case False{} True{}:
      {==}
    case False{} False{}:
      {==}

def Laws.active_fold_shut(l, f):
  {==}

def Laws.urg_pin_within(p):
  match p:
    case M.PhApproval{}:
      {==}
    case M.PhInput{}:
      {==}
    case M.PhWorking{}:
      {==}
    case M.PhMonitoring{n}:
      {==}
    case M.PhFailed{}:
      {==}
    case M.PhComplete{}:
      {==}
    case M.PhStopped{}:
      {==}
    case M.PhIdle{}:
      {==}

def Laws.menu_renames_settled(p, id, armed):
  {==}

def Laws.menu_renames_open(id, armed):
  {==}

def Laws.project_back_no_thread(s, now, client, id, pid, made):
  {==}

def Laws.rename_blank_nothing(th, now):
  {==}

def Laws.menu_settled_brings_back(p, id, armed):
  {==}

def Laws.caret_shows_when_moved():
  {==}

def Laws.tailnet_no_owner_no_trust(login):
  {==}

# Drawing lemmas
# --------------

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

law f32_eq_sound:
  for a: F32
  for b: F32
  for h: {T.Eq.f32(a, b) == True{} : Bool}
  {a == b : F32}

def f32_eq_sound(a, b, h):
  match a b:
    case F32{+x} F32{+y}:
      e = word_cmp_sound(32n, x, y, cmp_is_eq_sound(Word.cmp(32n, x, y), h))
      %e : {F32{x} == F32{_} : F32}
      {==}

law f32_eq_refl:
  for +a: F32
  {T.Eq.f32(a, a) == True{} : Bool}

def f32_eq_refl(a):
  match a:
    case F32{+x}:
      u32_eq_refl(U32{x})

law nat_eq_sound:
  for a: Nat
  for b: Nat
  for h: {Nat.is_eq(a, b) == True{} : Bool}
  {a == b : Nat}

def nat_eq_sound(a, b, h):
  match a b:
    case 0n 0n:
      {==}
    case 0n 1n+q:
      Empty.absurd({0n == 1n+q : Nat}, false_ne_true(h))
    case 1n+p 0n:
      Empty.absurd({1n+p == 0n : Nat}, false_ne_true(h))
    case 1n+p 1n+q:
      e = nat_eq_sound(p, q, h)
      %e : {1n+p == 1n+_ : Nat}
      {==}

law nat_eq_refl:
  for +a: Nat
  {Nat.is_eq(a, a) == True{} : Bool}

def nat_eq_refl(a):
  %Equal.sym(Cmp, Nat.cmp(a, a), EQ{}, nat_cmp_refl(a)) : {Cmp.is_eq(_) == True{} : Bool}
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

law bool_eq_refl:
  for +a: Bool
  {Q.Eq.bool(a, a) == True{} : Bool}

def bool_eq_refl(a):
  match a:
    case False{}:
      {==}
    case True{}:
      {==}

law eq_edge_sound:
  for a: R.Edge
  for b: R.Edge
  for +h: {T.Eq.edge(a, b) == True{} : Bool}
  {a == b : R.Edge}

def eq_edge_sound(a, b, h):
  match a b:
    case R.Edge{+a0, +a1, +a2, +a3} R.Edge{+b0, +b1, +b2, +b3}:
      +hl0 = and_l(T.Eq.f32(a0, b0), Bool.and(T.Eq.f32(a1, b1), Bool.and(T.Eq.f32(a2, b2), T.Eq.f32(a3, b3))), h)
      +hr0 = and_r(T.Eq.f32(a0, b0), Bool.and(T.Eq.f32(a1, b1), Bool.and(T.Eq.f32(a2, b2), T.Eq.f32(a3, b3))), h)
      e0 = f32_eq_sound(a0, b0, hl0)
      +hl1 = and_l(T.Eq.f32(a1, b1), Bool.and(T.Eq.f32(a2, b2), T.Eq.f32(a3, b3)), hr0)
      +hr1 = and_r(T.Eq.f32(a1, b1), Bool.and(T.Eq.f32(a2, b2), T.Eq.f32(a3, b3)), hr0)
      e1 = f32_eq_sound(a1, b1, hl1)
      +hl2 = and_l(T.Eq.f32(a2, b2), T.Eq.f32(a3, b3), hr1)
      +hr2 = and_r(T.Eq.f32(a2, b2), T.Eq.f32(a3, b3), hr1)
      e2 = f32_eq_sound(a2, b2, hl2)
      e3 = f32_eq_sound(a3, b3, hr2)
      %e0 : {R.Edge{a0, a1, a2, a3} == R.Edge{_, b1, b2, b3} : R.Edge}
      %e1 : {R.Edge{a0, a1, a2, a3} == R.Edge{a0, _, b2, b3} : R.Edge}
      %e2 : {R.Edge{a0, a1, a2, a3} == R.Edge{a0, a1, _, b3} : R.Edge}
      %e3 : {R.Edge{a0, a1, a2, a3} == R.Edge{a0, a1, a2, _} : R.Edge}
      {==}

law eq_edge_refl:
  for +a: R.Edge
  {T.Eq.edge(a, a) == True{} : Bool}

def eq_edge_refl(a):
  match a:
    case R.Edge{+a0, +a1, +a2, +a3}:
      %Equal.sym(Bool, T.Eq.f32(a0, a0), True{}, f32_eq_refl(a0)) : {Bool.and(_, Bool.and(T.Eq.f32(a1, a1), Bool.and(T.Eq.f32(a2, a2), T.Eq.f32(a3, a3)))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.f32(a1, a1), True{}, f32_eq_refl(a1)) : {Bool.and(True{}, Bool.and(_, Bool.and(T.Eq.f32(a2, a2), T.Eq.f32(a3, a3)))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.f32(a2, a2), True{}, f32_eq_refl(a2)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(_, T.Eq.f32(a3, a3)))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.f32(a3, a3), True{}, f32_eq_refl(a3)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, _))) == True{} : Bool}
      {==}

law eq_edges_sound:
  for a: List<&2, R.Edge>
  for b: List<&2, R.Edge>
  for +h: {T.Eq.edges(a, b) == True{} : Bool}
  {a == b : List<&2, R.Edge>}

def eq_edges_sound(a, b, h):
  match a b:
    case Nil{} Nil{}:
      {==}
    case Nil{} +y <> +yt:
      Empty.absurd({Nil{} == y <> yt : List<&2, R.Edge>}, false_ne_true(h))
    case +x <> +xt Nil{}:
      Empty.absurd({x <> xt == Nil{} : List<&2, R.Edge>}, false_ne_true(h))
    case +x <> +xt +y <> +yt:
      e0 = eq_edge_sound(x, y, and_l(T.Eq.edge(x, y), T.Eq.edges(xt, yt), h))
      e1 = eq_edges_sound(xt, yt, and_r(T.Eq.edge(x, y), T.Eq.edges(xt, yt), h))
      %e0 : {x <> xt == _ <> yt : List<&2, R.Edge>}
      %e1 : {x <> xt == x <> _ : List<&2, R.Edge>}
      {==}

law eq_edges_refl:
  for +a: List<&2, R.Edge>
  {T.Eq.edges(a, a) == True{} : Bool}

def eq_edges_refl(a):
  match a:
    case Nil{}:
      {==}
    case +x <> +xt:
      %Equal.sym(Bool, T.Eq.edge(x, x), True{}, eq_edge_refl(x)) : {Bool.and(_, T.Eq.edges(xt, xt)) == True{} : Bool}
      eq_edges_refl(xt)

law eq_mask_sound:
  for a: R.Mask
  for b: R.Mask
  for +h: {T.Eq.mask(a, b) == True{} : Bool}
  {a == b : R.Mask}

def eq_mask_sound(a, b, h):
  match a b:
    case R.MWord{+a0} R.MWord{+b0}:
      e0 = u32_eq_sound(a0, b0, h)
      %e0 : {R.MWord{a0} == R.MWord{_} : R.Mask}
      {==}
    case R.MWord{+a0} R.MPair{+b0, +b1}:
      Empty.absurd({R.MWord{a0} == R.MPair{b0, b1} : R.Mask}, false_ne_true(h))
    case R.MWord{+a0} R.M16{+b0, +b1, +b2, +b3, +b4, +b5, +b6, +b7, +b8, +b9, +b10, +b11, +b12, +b13, +b14, +b15}:
      Empty.absurd({R.MWord{a0} == R.M16{b0, b1, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15} : R.Mask}, false_ne_true(h))
    case R.MPair{+a0, +a1} R.MWord{+b0}:
      Empty.absurd({R.MPair{a0, a1} == R.MWord{b0} : R.Mask}, false_ne_true(h))
    case R.MPair{+a0, +a1} R.MPair{+b0, +b1}:
      +hl0 = and_l(T.Eq.mask(a0, b0), T.Eq.mask(a1, b1), h)
      +hr0 = and_r(T.Eq.mask(a0, b0), T.Eq.mask(a1, b1), h)
      e0 = eq_mask_sound(a0, b0, hl0)
      e1 = eq_mask_sound(a1, b1, hr0)
      %e0 : {R.MPair{a0, a1} == R.MPair{_, b1} : R.Mask}
      %e1 : {R.MPair{a0, a1} == R.MPair{a0, _} : R.Mask}
      {==}
    case R.MPair{+a0, +a1} R.M16{+b0, +b1, +b2, +b3, +b4, +b5, +b6, +b7, +b8, +b9, +b10, +b11, +b12, +b13, +b14, +b15}:
      Empty.absurd({R.MPair{a0, a1} == R.M16{b0, b1, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15} : R.Mask}, false_ne_true(h))
    case R.M16{+a0, +a1, +a2, +a3, +a4, +a5, +a6, +a7, +a8, +a9, +a10, +a11, +a12, +a13, +a14, +a15} R.MWord{+b0}:
      Empty.absurd({R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.MWord{b0} : R.Mask}, false_ne_true(h))
    case R.M16{+a0, +a1, +a2, +a3, +a4, +a5, +a6, +a7, +a8, +a9, +a10, +a11, +a12, +a13, +a14, +a15} R.MPair{+b0, +b1}:
      Empty.absurd({R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.MPair{b0, b1} : R.Mask}, false_ne_true(h))
    case R.M16{+a0, +a1, +a2, +a3, +a4, +a5, +a6, +a7, +a8, +a9, +a10, +a11, +a12, +a13, +a14, +a15} R.M16{+b0, +b1, +b2, +b3, +b4, +b5, +b6, +b7, +b8, +b9, +b10, +b11, +b12, +b13, +b14, +b15}:
      +hl0 = and_l(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), Bool.and(U32.is_eq(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15))))))))))))))), h)
      +hr0 = and_r(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), Bool.and(U32.is_eq(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15))))))))))))))), h)
      e0 = u32_eq_sound(a0, b0, hl0)
      +hl1 = and_l(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), Bool.and(U32.is_eq(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15)))))))))))))), hr0)
      +hr1 = and_r(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), Bool.and(U32.is_eq(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15)))))))))))))), hr0)
      e1 = u32_eq_sound(a1, b1, hl1)
      +hl2 = and_l(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), Bool.and(U32.is_eq(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15))))))))))))), hr1)
      +hr2 = and_r(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), Bool.and(U32.is_eq(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15))))))))))))), hr1)
      e2 = u32_eq_sound(a2, b2, hl2)
      +hl3 = and_l(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), Bool.and(U32.is_eq(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15)))))))))))), hr2)
      +hr3 = and_r(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), Bool.and(U32.is_eq(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15)))))))))))), hr2)
      e3 = u32_eq_sound(a3, b3, hl3)
      +hl4 = and_l(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), Bool.and(U32.is_eq(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15))))))))))), hr3)
      +hr4 = and_r(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), Bool.and(U32.is_eq(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15))))))))))), hr3)
      e4 = u32_eq_sound(a4, b4, hl4)
      +hl5 = and_l(U32.is_eq(a5, b5), Bool.and(U32.is_eq(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15)))))))))), hr4)
      +hr5 = and_r(U32.is_eq(a5, b5), Bool.and(U32.is_eq(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15)))))))))), hr4)
      e5 = u32_eq_sound(a5, b5, hl5)
      +hl6 = and_l(U32.is_eq(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15))))))))), hr5)
      +hr6 = and_r(U32.is_eq(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15))))))))), hr5)
      e6 = u32_eq_sound(a6, b6, hl6)
      +hl7 = and_l(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15)))))))), hr6)
      +hr7 = and_r(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15)))))))), hr6)
      e7 = u32_eq_sound(a7, b7, hl7)
      +hl8 = and_l(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15))))))), hr7)
      +hr8 = and_r(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15))))))), hr7)
      e8 = u32_eq_sound(a8, b8, hl8)
      +hl9 = and_l(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15)))))), hr8)
      +hr9 = and_r(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15)))))), hr8)
      e9 = u32_eq_sound(a9, b9, hl9)
      +hl10 = and_l(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15))))), hr9)
      +hr10 = and_r(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15))))), hr9)
      e10 = u32_eq_sound(a10, b10, hl10)
      +hl11 = and_l(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15)))), hr10)
      +hr11 = and_r(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15)))), hr10)
      e11 = u32_eq_sound(a11, b11, hl11)
      +hl12 = and_l(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15))), hr11)
      +hr12 = and_r(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15))), hr11)
      e12 = u32_eq_sound(a12, b12, hl12)
      +hl13 = and_l(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15)), hr12)
      +hr13 = and_r(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15)), hr12)
      e13 = u32_eq_sound(a13, b13, hl13)
      +hl14 = and_l(U32.is_eq(a14, b14), U32.is_eq(a15, b15), hr13)
      +hr14 = and_r(U32.is_eq(a14, b14), U32.is_eq(a15, b15), hr13)
      e14 = u32_eq_sound(a14, b14, hl14)
      e15 = u32_eq_sound(a15, b15, hr14)
      %e0 : {R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.M16{_, b1, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15} : R.Mask}
      %e1 : {R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.M16{a0, _, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15} : R.Mask}
      %e2 : {R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.M16{a0, a1, _, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15} : R.Mask}
      %e3 : {R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.M16{a0, a1, a2, _, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15} : R.Mask}
      %e4 : {R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.M16{a0, a1, a2, a3, _, b5, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15} : R.Mask}
      %e5 : {R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.M16{a0, a1, a2, a3, a4, _, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15} : R.Mask}
      %e6 : {R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.M16{a0, a1, a2, a3, a4, a5, _, b7, b8, b9, b10, b11, b12, b13, b14, b15} : R.Mask}
      %e7 : {R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.M16{a0, a1, a2, a3, a4, a5, a6, _, b8, b9, b10, b11, b12, b13, b14, b15} : R.Mask}
      %e8 : {R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.M16{a0, a1, a2, a3, a4, a5, a6, a7, _, b9, b10, b11, b12, b13, b14, b15} : R.Mask}
      %e9 : {R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, _, b10, b11, b12, b13, b14, b15} : R.Mask}
      %e10 : {R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, _, b11, b12, b13, b14, b15} : R.Mask}
      %e11 : {R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, _, b12, b13, b14, b15} : R.Mask}
      %e12 : {R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, _, b13, b14, b15} : R.Mask}
      %e13 : {R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, _, b14, b15} : R.Mask}
      %e14 : {R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, _, b15} : R.Mask}
      %e15 : {R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} == R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, _} : R.Mask}
      {==}

law eq_mask_refl:
  for +a: R.Mask
  {T.Eq.mask(a, a) == True{} : Bool}

def eq_mask_refl(a):
  match a:
    case R.MWord{+a0}:
      %Equal.sym(Bool, U32.is_eq(a0, a0), True{}, u32_eq_refl(a0)) : {_ == True{} : Bool}
      {==}
    case R.MPair{+a0, +a1}:
      %Equal.sym(Bool, T.Eq.mask(a0, a0), True{}, eq_mask_refl(a0)) : {Bool.and(_, T.Eq.mask(a1, a1)) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.mask(a1, a1), True{}, eq_mask_refl(a1)) : {Bool.and(True{}, _) == True{} : Bool}
      {==}
    case R.M16{+a0, +a1, +a2, +a3, +a4, +a5, +a6, +a7, +a8, +a9, +a10, +a11, +a12, +a13, +a14, +a15}:
      %Equal.sym(Bool, U32.is_eq(a0, a0), True{}, u32_eq_refl(a0)) : {Bool.and(_, Bool.and(U32.is_eq(a1, a1), Bool.and(U32.is_eq(a2, a2), Bool.and(U32.is_eq(a3, a3), Bool.and(U32.is_eq(a4, a4), Bool.and(U32.is_eq(a5, a5), Bool.and(U32.is_eq(a6, a6), Bool.and(U32.is_eq(a7, a7), Bool.and(U32.is_eq(a8, a8), Bool.and(U32.is_eq(a9, a9), Bool.and(U32.is_eq(a10, a10), Bool.and(U32.is_eq(a11, a11), Bool.and(U32.is_eq(a12, a12), Bool.and(U32.is_eq(a13, a13), Bool.and(U32.is_eq(a14, a14), U32.is_eq(a15, a15)))))))))))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a1, a1), True{}, u32_eq_refl(a1)) : {Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a2, a2), Bool.and(U32.is_eq(a3, a3), Bool.and(U32.is_eq(a4, a4), Bool.and(U32.is_eq(a5, a5), Bool.and(U32.is_eq(a6, a6), Bool.and(U32.is_eq(a7, a7), Bool.and(U32.is_eq(a8, a8), Bool.and(U32.is_eq(a9, a9), Bool.and(U32.is_eq(a10, a10), Bool.and(U32.is_eq(a11, a11), Bool.and(U32.is_eq(a12, a12), Bool.and(U32.is_eq(a13, a13), Bool.and(U32.is_eq(a14, a14), U32.is_eq(a15, a15)))))))))))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a2, a2), True{}, u32_eq_refl(a2)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a3, a3), Bool.and(U32.is_eq(a4, a4), Bool.and(U32.is_eq(a5, a5), Bool.and(U32.is_eq(a6, a6), Bool.and(U32.is_eq(a7, a7), Bool.and(U32.is_eq(a8, a8), Bool.and(U32.is_eq(a9, a9), Bool.and(U32.is_eq(a10, a10), Bool.and(U32.is_eq(a11, a11), Bool.and(U32.is_eq(a12, a12), Bool.and(U32.is_eq(a13, a13), Bool.and(U32.is_eq(a14, a14), U32.is_eq(a15, a15)))))))))))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a3, a3), True{}, u32_eq_refl(a3)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a4, a4), Bool.and(U32.is_eq(a5, a5), Bool.and(U32.is_eq(a6, a6), Bool.and(U32.is_eq(a7, a7), Bool.and(U32.is_eq(a8, a8), Bool.and(U32.is_eq(a9, a9), Bool.and(U32.is_eq(a10, a10), Bool.and(U32.is_eq(a11, a11), Bool.and(U32.is_eq(a12, a12), Bool.and(U32.is_eq(a13, a13), Bool.and(U32.is_eq(a14, a14), U32.is_eq(a15, a15)))))))))))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a4, a4), True{}, u32_eq_refl(a4)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a5, a5), Bool.and(U32.is_eq(a6, a6), Bool.and(U32.is_eq(a7, a7), Bool.and(U32.is_eq(a8, a8), Bool.and(U32.is_eq(a9, a9), Bool.and(U32.is_eq(a10, a10), Bool.and(U32.is_eq(a11, a11), Bool.and(U32.is_eq(a12, a12), Bool.and(U32.is_eq(a13, a13), Bool.and(U32.is_eq(a14, a14), U32.is_eq(a15, a15)))))))))))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a5, a5), True{}, u32_eq_refl(a5)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a6, a6), Bool.and(U32.is_eq(a7, a7), Bool.and(U32.is_eq(a8, a8), Bool.and(U32.is_eq(a9, a9), Bool.and(U32.is_eq(a10, a10), Bool.and(U32.is_eq(a11, a11), Bool.and(U32.is_eq(a12, a12), Bool.and(U32.is_eq(a13, a13), Bool.and(U32.is_eq(a14, a14), U32.is_eq(a15, a15)))))))))))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a6, a6), True{}, u32_eq_refl(a6)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a7, a7), Bool.and(U32.is_eq(a8, a8), Bool.and(U32.is_eq(a9, a9), Bool.and(U32.is_eq(a10, a10), Bool.and(U32.is_eq(a11, a11), Bool.and(U32.is_eq(a12, a12), Bool.and(U32.is_eq(a13, a13), Bool.and(U32.is_eq(a14, a14), U32.is_eq(a15, a15)))))))))))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a7, a7), True{}, u32_eq_refl(a7)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a8, a8), Bool.and(U32.is_eq(a9, a9), Bool.and(U32.is_eq(a10, a10), Bool.and(U32.is_eq(a11, a11), Bool.and(U32.is_eq(a12, a12), Bool.and(U32.is_eq(a13, a13), Bool.and(U32.is_eq(a14, a14), U32.is_eq(a15, a15)))))))))))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a8, a8), True{}, u32_eq_refl(a8)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a9, a9), Bool.and(U32.is_eq(a10, a10), Bool.and(U32.is_eq(a11, a11), Bool.and(U32.is_eq(a12, a12), Bool.and(U32.is_eq(a13, a13), Bool.and(U32.is_eq(a14, a14), U32.is_eq(a15, a15)))))))))))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a9, a9), True{}, u32_eq_refl(a9)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a10, a10), Bool.and(U32.is_eq(a11, a11), Bool.and(U32.is_eq(a12, a12), Bool.and(U32.is_eq(a13, a13), Bool.and(U32.is_eq(a14, a14), U32.is_eq(a15, a15)))))))))))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a10, a10), True{}, u32_eq_refl(a10)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a11, a11), Bool.and(U32.is_eq(a12, a12), Bool.and(U32.is_eq(a13, a13), Bool.and(U32.is_eq(a14, a14), U32.is_eq(a15, a15)))))))))))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a11, a11), True{}, u32_eq_refl(a11)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a12, a12), Bool.and(U32.is_eq(a13, a13), Bool.and(U32.is_eq(a14, a14), U32.is_eq(a15, a15)))))))))))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a12, a12), True{}, u32_eq_refl(a12)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a13, a13), Bool.and(U32.is_eq(a14, a14), U32.is_eq(a15, a15)))))))))))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a13, a13), True{}, u32_eq_refl(a13)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a14, a14), U32.is_eq(a15, a15)))))))))))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a14, a14), True{}, u32_eq_refl(a14)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, U32.is_eq(a15, a15)))))))))))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a15, a15), True{}, u32_eq_refl(a15)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, _))))))))))))))) == True{} : Bool}
      {==}

law eq_glyph_sound:
  for a: F.Glyph
  for b: F.Glyph
  for +h: {T.Eq.glyph(a, b) == True{} : Bool}
  {a == b : F.Glyph}

def eq_glyph_sound(a, b, h):
  match a b:
    case F.Glyph{+a0, +a1, +a2, +a3} F.Glyph{+b0, +b1, +b2, +b3}:
      +hl0 = and_l(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), U32.is_eq(a3, b3))), h)
      +hr0 = and_r(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), U32.is_eq(a3, b3))), h)
      e0 = u32_eq_sound(a0, b0, hl0)
      +hl1 = and_l(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), U32.is_eq(a3, b3)), hr0)
      +hr1 = and_r(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), U32.is_eq(a3, b3)), hr0)
      e1 = u32_eq_sound(a1, b1, hl1)
      +hl2 = and_l(U32.is_eq(a2, b2), U32.is_eq(a3, b3), hr1)
      +hr2 = and_r(U32.is_eq(a2, b2), U32.is_eq(a3, b3), hr1)
      e2 = u32_eq_sound(a2, b2, hl2)
      e3 = u32_eq_sound(a3, b3, hr2)
      %e0 : {F.Glyph{a0, a1, a2, a3} == F.Glyph{_, b1, b2, b3} : F.Glyph}
      %e1 : {F.Glyph{a0, a1, a2, a3} == F.Glyph{a0, _, b2, b3} : F.Glyph}
      %e2 : {F.Glyph{a0, a1, a2, a3} == F.Glyph{a0, a1, _, b3} : F.Glyph}
      %e3 : {F.Glyph{a0, a1, a2, a3} == F.Glyph{a0, a1, a2, _} : F.Glyph}
      {==}

law eq_glyph_refl:
  for +a: F.Glyph
  {T.Eq.glyph(a, a) == True{} : Bool}

def eq_glyph_refl(a):
  match a:
    case F.Glyph{+a0, +a1, +a2, +a3}:
      %Equal.sym(Bool, U32.is_eq(a0, a0), True{}, u32_eq_refl(a0)) : {Bool.and(_, Bool.and(U32.is_eq(a1, a1), Bool.and(U32.is_eq(a2, a2), U32.is_eq(a3, a3)))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a1, a1), True{}, u32_eq_refl(a1)) : {Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a2, a2), U32.is_eq(a3, a3)))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a2, a2), True{}, u32_eq_refl(a2)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(_, U32.is_eq(a3, a3)))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a3, a3), True{}, u32_eq_refl(a3)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, _))) == True{} : Bool}
      {==}

law eq_run_sound:
  for a: R.Run
  for b: R.Run
  for +h: {T.Eq.run(a, b) == True{} : Bool}
  {a == b : R.Run}

def eq_run_sound(a, b, h):
  match a b:
    case R.RNone{} R.RNone{}:
      {==}
    case R.RNone{} R.RGlyph{+b0, +b1, +b2, +b3, +b4, +b5}:
      Empty.absurd({R.RNone{} == R.RGlyph{b0, b1, b2, b3, b4, b5} : R.Run}, false_ne_true(h))
    case R.RNone{} R.RPair{+b0, +b1, +b2, +b3, +b4, +b5}:
      Empty.absurd({R.RNone{} == R.RPair{b0, b1, b2, b3, b4, b5} : R.Run}, false_ne_true(h))
    case R.RGlyph{+a0, +a1, +a2, +a3, +a4, +a5} R.RNone{}:
      Empty.absurd({R.RGlyph{a0, a1, a2, a3, a4, a5} == R.RNone{} : R.Run}, false_ne_true(h))
    case R.RGlyph{+a0, +a1, +a2, +a3, +a4, +a5} R.RGlyph{+b0, +b1, +b2, +b3, +b4, +b5}:
      +hl0 = and_l(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(Nat.is_eq(a4, b4), T.Eq.mask(a5, b5))))), h)
      +hr0 = and_r(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(Nat.is_eq(a4, b4), T.Eq.mask(a5, b5))))), h)
      e0 = u32_eq_sound(a0, b0, hl0)
      +hl1 = and_l(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(Nat.is_eq(a4, b4), T.Eq.mask(a5, b5)))), hr0)
      +hr1 = and_r(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(Nat.is_eq(a4, b4), T.Eq.mask(a5, b5)))), hr0)
      e1 = u32_eq_sound(a1, b1, hl1)
      +hl2 = and_l(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(Nat.is_eq(a4, b4), T.Eq.mask(a5, b5))), hr1)
      +hr2 = and_r(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(Nat.is_eq(a4, b4), T.Eq.mask(a5, b5))), hr1)
      e2 = u32_eq_sound(a2, b2, hl2)
      +hl3 = and_l(U32.is_eq(a3, b3), Bool.and(Nat.is_eq(a4, b4), T.Eq.mask(a5, b5)), hr2)
      +hr3 = and_r(U32.is_eq(a3, b3), Bool.and(Nat.is_eq(a4, b4), T.Eq.mask(a5, b5)), hr2)
      e3 = u32_eq_sound(a3, b3, hl3)
      +hl4 = and_l(Nat.is_eq(a4, b4), T.Eq.mask(a5, b5), hr3)
      +hr4 = and_r(Nat.is_eq(a4, b4), T.Eq.mask(a5, b5), hr3)
      e4 = nat_eq_sound(a4, b4, hl4)
      e5 = eq_mask_sound(a5, b5, hr4)
      %e0 : {R.RGlyph{a0, a1, a2, a3, a4, a5} == R.RGlyph{_, b1, b2, b3, b4, b5} : R.Run}
      %e1 : {R.RGlyph{a0, a1, a2, a3, a4, a5} == R.RGlyph{a0, _, b2, b3, b4, b5} : R.Run}
      %e2 : {R.RGlyph{a0, a1, a2, a3, a4, a5} == R.RGlyph{a0, a1, _, b3, b4, b5} : R.Run}
      %e3 : {R.RGlyph{a0, a1, a2, a3, a4, a5} == R.RGlyph{a0, a1, a2, _, b4, b5} : R.Run}
      %e4 : {R.RGlyph{a0, a1, a2, a3, a4, a5} == R.RGlyph{a0, a1, a2, a3, _, b5} : R.Run}
      %e5 : {R.RGlyph{a0, a1, a2, a3, a4, a5} == R.RGlyph{a0, a1, a2, a3, a4, _} : R.Run}
      {==}
    case R.RGlyph{+a0, +a1, +a2, +a3, +a4, +a5} R.RPair{+b0, +b1, +b2, +b3, +b4, +b5}:
      Empty.absurd({R.RGlyph{a0, a1, a2, a3, a4, a5} == R.RPair{b0, b1, b2, b3, b4, b5} : R.Run}, false_ne_true(h))
    case R.RPair{+a0, +a1, +a2, +a3, +a4, +a5} R.RNone{}:
      Empty.absurd({R.RPair{a0, a1, a2, a3, a4, a5} == R.RNone{} : R.Run}, false_ne_true(h))
    case R.RPair{+a0, +a1, +a2, +a3, +a4, +a5} R.RGlyph{+b0, +b1, +b2, +b3, +b4, +b5}:
      Empty.absurd({R.RPair{a0, a1, a2, a3, a4, a5} == R.RGlyph{b0, b1, b2, b3, b4, b5} : R.Run}, false_ne_true(h))
    case R.RPair{+a0, +a1, +a2, +a3, +a4, +a5} R.RPair{+b0, +b1, +b2, +b3, +b4, +b5}:
      +hl0 = and_l(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(T.Eq.run(a4, b4), T.Eq.run(a5, b5))))), h)
      +hr0 = and_r(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(T.Eq.run(a4, b4), T.Eq.run(a5, b5))))), h)
      e0 = u32_eq_sound(a0, b0, hl0)
      +hl1 = and_l(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(T.Eq.run(a4, b4), T.Eq.run(a5, b5)))), hr0)
      +hr1 = and_r(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(T.Eq.run(a4, b4), T.Eq.run(a5, b5)))), hr0)
      e1 = u32_eq_sound(a1, b1, hl1)
      +hl2 = and_l(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(T.Eq.run(a4, b4), T.Eq.run(a5, b5))), hr1)
      +hr2 = and_r(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(T.Eq.run(a4, b4), T.Eq.run(a5, b5))), hr1)
      e2 = u32_eq_sound(a2, b2, hl2)
      +hl3 = and_l(U32.is_eq(a3, b3), Bool.and(T.Eq.run(a4, b4), T.Eq.run(a5, b5)), hr2)
      +hr3 = and_r(U32.is_eq(a3, b3), Bool.and(T.Eq.run(a4, b4), T.Eq.run(a5, b5)), hr2)
      e3 = u32_eq_sound(a3, b3, hl3)
      +hl4 = and_l(T.Eq.run(a4, b4), T.Eq.run(a5, b5), hr3)
      +hr4 = and_r(T.Eq.run(a4, b4), T.Eq.run(a5, b5), hr3)
      e4 = eq_run_sound(a4, b4, hl4)
      e5 = eq_run_sound(a5, b5, hr4)
      %e0 : {R.RPair{a0, a1, a2, a3, a4, a5} == R.RPair{_, b1, b2, b3, b4, b5} : R.Run}
      %e1 : {R.RPair{a0, a1, a2, a3, a4, a5} == R.RPair{a0, _, b2, b3, b4, b5} : R.Run}
      %e2 : {R.RPair{a0, a1, a2, a3, a4, a5} == R.RPair{a0, a1, _, b3, b4, b5} : R.Run}
      %e3 : {R.RPair{a0, a1, a2, a3, a4, a5} == R.RPair{a0, a1, a2, _, b4, b5} : R.Run}
      %e4 : {R.RPair{a0, a1, a2, a3, a4, a5} == R.RPair{a0, a1, a2, a3, _, b5} : R.Run}
      %e5 : {R.RPair{a0, a1, a2, a3, a4, a5} == R.RPair{a0, a1, a2, a3, a4, _} : R.Run}
      {==}

law eq_run_refl:
  for +a: R.Run
  {T.Eq.run(a, a) == True{} : Bool}

def eq_run_refl(a):
  match a:
    case R.RNone{}:
      {==}
    case R.RGlyph{+a0, +a1, +a2, +a3, +a4, +a5}:
      %Equal.sym(Bool, U32.is_eq(a0, a0), True{}, u32_eq_refl(a0)) : {Bool.and(_, Bool.and(U32.is_eq(a1, a1), Bool.and(U32.is_eq(a2, a2), Bool.and(U32.is_eq(a3, a3), Bool.and(Nat.is_eq(a4, a4), T.Eq.mask(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a1, a1), True{}, u32_eq_refl(a1)) : {Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a2, a2), Bool.and(U32.is_eq(a3, a3), Bool.and(Nat.is_eq(a4, a4), T.Eq.mask(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a2, a2), True{}, u32_eq_refl(a2)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a3, a3), Bool.and(Nat.is_eq(a4, a4), T.Eq.mask(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a3, a3), True{}, u32_eq_refl(a3)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(Nat.is_eq(a4, a4), T.Eq.mask(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, Nat.is_eq(a4, a4), True{}, nat_eq_refl(a4)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, T.Eq.mask(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.mask(a5, a5), True{}, eq_mask_refl(a5)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, _))))) == True{} : Bool}
      {==}
    case R.RPair{+a0, +a1, +a2, +a3, +a4, +a5}:
      %Equal.sym(Bool, U32.is_eq(a0, a0), True{}, u32_eq_refl(a0)) : {Bool.and(_, Bool.and(U32.is_eq(a1, a1), Bool.and(U32.is_eq(a2, a2), Bool.and(U32.is_eq(a3, a3), Bool.and(T.Eq.run(a4, a4), T.Eq.run(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a1, a1), True{}, u32_eq_refl(a1)) : {Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a2, a2), Bool.and(U32.is_eq(a3, a3), Bool.and(T.Eq.run(a4, a4), T.Eq.run(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a2, a2), True{}, u32_eq_refl(a2)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a3, a3), Bool.and(T.Eq.run(a4, a4), T.Eq.run(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a3, a3), True{}, u32_eq_refl(a3)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(T.Eq.run(a4, a4), T.Eq.run(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.run(a4, a4), True{}, eq_run_refl(a4)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, T.Eq.run(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.run(a5, a5), True{}, eq_run_refl(a5)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, _))))) == True{} : Bool}
      {==}

law eq_shape_sound:
  for a: R.Shape
  for b: R.Shape
  for +h: {T.Eq.shape(a, b) == True{} : Bool}
  {a == b : R.Shape}

def eq_shape_sound(a, b, h):
  match a b:
    case R.SRect{} R.SRect{}:
      {==}
    case R.SRect{} R.SPoly{+b0, +b1}:
      Empty.absurd({R.SRect{} == R.SPoly{b0, b1} : R.Shape}, false_ne_true(h))
    case R.SRect{} R.SGlyph{+b0, +b1, +b2}:
      Empty.absurd({R.SRect{} == R.SGlyph{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SRect{} R.SSeg{+b0, +b1, +b2, +b3, +b4}:
      Empty.absurd({R.SRect{} == R.SSeg{b0, b1, b2, b3, b4} : R.Shape}, false_ne_true(h))
    case R.SRect{} R.SDisc{+b0, +b1, +b2}:
      Empty.absurd({R.SRect{} == R.SDisc{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SRect{} R.STri{+b0, +b1, +b2, +b3, +b4, +b5}:
      Empty.absurd({R.SRect{} == R.STri{b0, b1, b2, b3, b4, b5} : R.Shape}, false_ne_true(h))
    case R.SRect{} R.SMask{+b0, +b1, +b2, +b3, +b4}:
      Empty.absurd({R.SRect{} == R.SMask{b0, b1, b2, b3, b4} : R.Shape}, false_ne_true(h))
    case R.SPoly{+a0, +a1} R.SRect{}:
      Empty.absurd({R.SPoly{a0, a1} == R.SRect{} : R.Shape}, false_ne_true(h))
    case R.SPoly{+a0, +a1} R.SPoly{+b0, +b1}:
      +hl0 = and_l(Q.Eq.bool(a0, b0), T.Eq.edges(a1, b1), h)
      +hr0 = and_r(Q.Eq.bool(a0, b0), T.Eq.edges(a1, b1), h)
      e0 = bool_eq_sound(a0, b0, hl0)
      e1 = eq_edges_sound(a1, b1, hr0)
      %e0 : {R.SPoly{a0, a1} == R.SPoly{_, b1} : R.Shape}
      %e1 : {R.SPoly{a0, a1} == R.SPoly{a0, _} : R.Shape}
      {==}
    case R.SPoly{+a0, +a1} R.SGlyph{+b0, +b1, +b2}:
      Empty.absurd({R.SPoly{a0, a1} == R.SGlyph{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SPoly{+a0, +a1} R.SSeg{+b0, +b1, +b2, +b3, +b4}:
      Empty.absurd({R.SPoly{a0, a1} == R.SSeg{b0, b1, b2, b3, b4} : R.Shape}, false_ne_true(h))
    case R.SPoly{+a0, +a1} R.SDisc{+b0, +b1, +b2}:
      Empty.absurd({R.SPoly{a0, a1} == R.SDisc{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SPoly{+a0, +a1} R.STri{+b0, +b1, +b2, +b3, +b4, +b5}:
      Empty.absurd({R.SPoly{a0, a1} == R.STri{b0, b1, b2, b3, b4, b5} : R.Shape}, false_ne_true(h))
    case R.SPoly{+a0, +a1} R.SMask{+b0, +b1, +b2, +b3, +b4}:
      Empty.absurd({R.SPoly{a0, a1} == R.SMask{b0, b1, b2, b3, b4} : R.Shape}, false_ne_true(h))
    case R.SGlyph{+a0, +a1, +a2} R.SRect{}:
      Empty.absurd({R.SGlyph{a0, a1, a2} == R.SRect{} : R.Shape}, false_ne_true(h))
    case R.SGlyph{+a0, +a1, +a2} R.SPoly{+b0, +b1}:
      Empty.absurd({R.SGlyph{a0, a1, a2} == R.SPoly{b0, b1} : R.Shape}, false_ne_true(h))
    case R.SGlyph{+a0, +a1, +a2} R.SGlyph{+b0, +b1, +b2}:
      +hl0 = and_l(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), T.Eq.glyph(a2, b2)), h)
      +hr0 = and_r(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), T.Eq.glyph(a2, b2)), h)
      e0 = u32_eq_sound(a0, b0, hl0)
      +hl1 = and_l(U32.is_eq(a1, b1), T.Eq.glyph(a2, b2), hr0)
      +hr1 = and_r(U32.is_eq(a1, b1), T.Eq.glyph(a2, b2), hr0)
      e1 = u32_eq_sound(a1, b1, hl1)
      e2 = eq_glyph_sound(a2, b2, hr1)
      %e0 : {R.SGlyph{a0, a1, a2} == R.SGlyph{_, b1, b2} : R.Shape}
      %e1 : {R.SGlyph{a0, a1, a2} == R.SGlyph{a0, _, b2} : R.Shape}
      %e2 : {R.SGlyph{a0, a1, a2} == R.SGlyph{a0, a1, _} : R.Shape}
      {==}
    case R.SGlyph{+a0, +a1, +a2} R.SSeg{+b0, +b1, +b2, +b3, +b4}:
      Empty.absurd({R.SGlyph{a0, a1, a2} == R.SSeg{b0, b1, b2, b3, b4} : R.Shape}, false_ne_true(h))
    case R.SGlyph{+a0, +a1, +a2} R.SDisc{+b0, +b1, +b2}:
      Empty.absurd({R.SGlyph{a0, a1, a2} == R.SDisc{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SGlyph{+a0, +a1, +a2} R.STri{+b0, +b1, +b2, +b3, +b4, +b5}:
      Empty.absurd({R.SGlyph{a0, a1, a2} == R.STri{b0, b1, b2, b3, b4, b5} : R.Shape}, false_ne_true(h))
    case R.SGlyph{+a0, +a1, +a2} R.SMask{+b0, +b1, +b2, +b3, +b4}:
      Empty.absurd({R.SGlyph{a0, a1, a2} == R.SMask{b0, b1, b2, b3, b4} : R.Shape}, false_ne_true(h))
    case R.SSeg{+a0, +a1, +a2, +a3, +a4} R.SRect{}:
      Empty.absurd({R.SSeg{a0, a1, a2, a3, a4} == R.SRect{} : R.Shape}, false_ne_true(h))
    case R.SSeg{+a0, +a1, +a2, +a3, +a4} R.SPoly{+b0, +b1}:
      Empty.absurd({R.SSeg{a0, a1, a2, a3, a4} == R.SPoly{b0, b1} : R.Shape}, false_ne_true(h))
    case R.SSeg{+a0, +a1, +a2, +a3, +a4} R.SGlyph{+b0, +b1, +b2}:
      Empty.absurd({R.SSeg{a0, a1, a2, a3, a4} == R.SGlyph{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SSeg{+a0, +a1, +a2, +a3, +a4} R.SSeg{+b0, +b1, +b2, +b3, +b4}:
      +hl0 = and_l(T.Eq.f32(a0, b0), Bool.and(T.Eq.f32(a1, b1), Bool.and(T.Eq.f32(a2, b2), Bool.and(T.Eq.f32(a3, b3), T.Eq.f32(a4, b4)))), h)
      +hr0 = and_r(T.Eq.f32(a0, b0), Bool.and(T.Eq.f32(a1, b1), Bool.and(T.Eq.f32(a2, b2), Bool.and(T.Eq.f32(a3, b3), T.Eq.f32(a4, b4)))), h)
      e0 = f32_eq_sound(a0, b0, hl0)
      +hl1 = and_l(T.Eq.f32(a1, b1), Bool.and(T.Eq.f32(a2, b2), Bool.and(T.Eq.f32(a3, b3), T.Eq.f32(a4, b4))), hr0)
      +hr1 = and_r(T.Eq.f32(a1, b1), Bool.and(T.Eq.f32(a2, b2), Bool.and(T.Eq.f32(a3, b3), T.Eq.f32(a4, b4))), hr0)
      e1 = f32_eq_sound(a1, b1, hl1)
      +hl2 = and_l(T.Eq.f32(a2, b2), Bool.and(T.Eq.f32(a3, b3), T.Eq.f32(a4, b4)), hr1)
      +hr2 = and_r(T.Eq.f32(a2, b2), Bool.and(T.Eq.f32(a3, b3), T.Eq.f32(a4, b4)), hr1)
      e2 = f32_eq_sound(a2, b2, hl2)
      +hl3 = and_l(T.Eq.f32(a3, b3), T.Eq.f32(a4, b4), hr2)
      +hr3 = and_r(T.Eq.f32(a3, b3), T.Eq.f32(a4, b4), hr2)
      e3 = f32_eq_sound(a3, b3, hl3)
      e4 = f32_eq_sound(a4, b4, hr3)
      %e0 : {R.SSeg{a0, a1, a2, a3, a4} == R.SSeg{_, b1, b2, b3, b4} : R.Shape}
      %e1 : {R.SSeg{a0, a1, a2, a3, a4} == R.SSeg{a0, _, b2, b3, b4} : R.Shape}
      %e2 : {R.SSeg{a0, a1, a2, a3, a4} == R.SSeg{a0, a1, _, b3, b4} : R.Shape}
      %e3 : {R.SSeg{a0, a1, a2, a3, a4} == R.SSeg{a0, a1, a2, _, b4} : R.Shape}
      %e4 : {R.SSeg{a0, a1, a2, a3, a4} == R.SSeg{a0, a1, a2, a3, _} : R.Shape}
      {==}
    case R.SSeg{+a0, +a1, +a2, +a3, +a4} R.SDisc{+b0, +b1, +b2}:
      Empty.absurd({R.SSeg{a0, a1, a2, a3, a4} == R.SDisc{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SSeg{+a0, +a1, +a2, +a3, +a4} R.STri{+b0, +b1, +b2, +b3, +b4, +b5}:
      Empty.absurd({R.SSeg{a0, a1, a2, a3, a4} == R.STri{b0, b1, b2, b3, b4, b5} : R.Shape}, false_ne_true(h))
    case R.SSeg{+a0, +a1, +a2, +a3, +a4} R.SMask{+b0, +b1, +b2, +b3, +b4}:
      Empty.absurd({R.SSeg{a0, a1, a2, a3, a4} == R.SMask{b0, b1, b2, b3, b4} : R.Shape}, false_ne_true(h))
    case R.SDisc{+a0, +a1, +a2} R.SRect{}:
      Empty.absurd({R.SDisc{a0, a1, a2} == R.SRect{} : R.Shape}, false_ne_true(h))
    case R.SDisc{+a0, +a1, +a2} R.SPoly{+b0, +b1}:
      Empty.absurd({R.SDisc{a0, a1, a2} == R.SPoly{b0, b1} : R.Shape}, false_ne_true(h))
    case R.SDisc{+a0, +a1, +a2} R.SGlyph{+b0, +b1, +b2}:
      Empty.absurd({R.SDisc{a0, a1, a2} == R.SGlyph{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SDisc{+a0, +a1, +a2} R.SSeg{+b0, +b1, +b2, +b3, +b4}:
      Empty.absurd({R.SDisc{a0, a1, a2} == R.SSeg{b0, b1, b2, b3, b4} : R.Shape}, false_ne_true(h))
    case R.SDisc{+a0, +a1, +a2} R.SDisc{+b0, +b1, +b2}:
      +hl0 = and_l(T.Eq.f32(a0, b0), Bool.and(T.Eq.f32(a1, b1), T.Eq.f32(a2, b2)), h)
      +hr0 = and_r(T.Eq.f32(a0, b0), Bool.and(T.Eq.f32(a1, b1), T.Eq.f32(a2, b2)), h)
      e0 = f32_eq_sound(a0, b0, hl0)
      +hl1 = and_l(T.Eq.f32(a1, b1), T.Eq.f32(a2, b2), hr0)
      +hr1 = and_r(T.Eq.f32(a1, b1), T.Eq.f32(a2, b2), hr0)
      e1 = f32_eq_sound(a1, b1, hl1)
      e2 = f32_eq_sound(a2, b2, hr1)
      %e0 : {R.SDisc{a0, a1, a2} == R.SDisc{_, b1, b2} : R.Shape}
      %e1 : {R.SDisc{a0, a1, a2} == R.SDisc{a0, _, b2} : R.Shape}
      %e2 : {R.SDisc{a0, a1, a2} == R.SDisc{a0, a1, _} : R.Shape}
      {==}
    case R.SDisc{+a0, +a1, +a2} R.STri{+b0, +b1, +b2, +b3, +b4, +b5}:
      Empty.absurd({R.SDisc{a0, a1, a2} == R.STri{b0, b1, b2, b3, b4, b5} : R.Shape}, false_ne_true(h))
    case R.SDisc{+a0, +a1, +a2} R.SMask{+b0, +b1, +b2, +b3, +b4}:
      Empty.absurd({R.SDisc{a0, a1, a2} == R.SMask{b0, b1, b2, b3, b4} : R.Shape}, false_ne_true(h))
    case R.STri{+a0, +a1, +a2, +a3, +a4, +a5} R.SRect{}:
      Empty.absurd({R.STri{a0, a1, a2, a3, a4, a5} == R.SRect{} : R.Shape}, false_ne_true(h))
    case R.STri{+a0, +a1, +a2, +a3, +a4, +a5} R.SPoly{+b0, +b1}:
      Empty.absurd({R.STri{a0, a1, a2, a3, a4, a5} == R.SPoly{b0, b1} : R.Shape}, false_ne_true(h))
    case R.STri{+a0, +a1, +a2, +a3, +a4, +a5} R.SGlyph{+b0, +b1, +b2}:
      Empty.absurd({R.STri{a0, a1, a2, a3, a4, a5} == R.SGlyph{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.STri{+a0, +a1, +a2, +a3, +a4, +a5} R.SSeg{+b0, +b1, +b2, +b3, +b4}:
      Empty.absurd({R.STri{a0, a1, a2, a3, a4, a5} == R.SSeg{b0, b1, b2, b3, b4} : R.Shape}, false_ne_true(h))
    case R.STri{+a0, +a1, +a2, +a3, +a4, +a5} R.SDisc{+b0, +b1, +b2}:
      Empty.absurd({R.STri{a0, a1, a2, a3, a4, a5} == R.SDisc{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.STri{+a0, +a1, +a2, +a3, +a4, +a5} R.STri{+b0, +b1, +b2, +b3, +b4, +b5}:
      +hl0 = and_l(T.Eq.f32(a0, b0), Bool.and(T.Eq.f32(a1, b1), Bool.and(T.Eq.f32(a2, b2), Bool.and(T.Eq.f32(a3, b3), Bool.and(T.Eq.f32(a4, b4), T.Eq.f32(a5, b5))))), h)
      +hr0 = and_r(T.Eq.f32(a0, b0), Bool.and(T.Eq.f32(a1, b1), Bool.and(T.Eq.f32(a2, b2), Bool.and(T.Eq.f32(a3, b3), Bool.and(T.Eq.f32(a4, b4), T.Eq.f32(a5, b5))))), h)
      e0 = f32_eq_sound(a0, b0, hl0)
      +hl1 = and_l(T.Eq.f32(a1, b1), Bool.and(T.Eq.f32(a2, b2), Bool.and(T.Eq.f32(a3, b3), Bool.and(T.Eq.f32(a4, b4), T.Eq.f32(a5, b5)))), hr0)
      +hr1 = and_r(T.Eq.f32(a1, b1), Bool.and(T.Eq.f32(a2, b2), Bool.and(T.Eq.f32(a3, b3), Bool.and(T.Eq.f32(a4, b4), T.Eq.f32(a5, b5)))), hr0)
      e1 = f32_eq_sound(a1, b1, hl1)
      +hl2 = and_l(T.Eq.f32(a2, b2), Bool.and(T.Eq.f32(a3, b3), Bool.and(T.Eq.f32(a4, b4), T.Eq.f32(a5, b5))), hr1)
      +hr2 = and_r(T.Eq.f32(a2, b2), Bool.and(T.Eq.f32(a3, b3), Bool.and(T.Eq.f32(a4, b4), T.Eq.f32(a5, b5))), hr1)
      e2 = f32_eq_sound(a2, b2, hl2)
      +hl3 = and_l(T.Eq.f32(a3, b3), Bool.and(T.Eq.f32(a4, b4), T.Eq.f32(a5, b5)), hr2)
      +hr3 = and_r(T.Eq.f32(a3, b3), Bool.and(T.Eq.f32(a4, b4), T.Eq.f32(a5, b5)), hr2)
      e3 = f32_eq_sound(a3, b3, hl3)
      +hl4 = and_l(T.Eq.f32(a4, b4), T.Eq.f32(a5, b5), hr3)
      +hr4 = and_r(T.Eq.f32(a4, b4), T.Eq.f32(a5, b5), hr3)
      e4 = f32_eq_sound(a4, b4, hl4)
      e5 = f32_eq_sound(a5, b5, hr4)
      %e0 : {R.STri{a0, a1, a2, a3, a4, a5} == R.STri{_, b1, b2, b3, b4, b5} : R.Shape}
      %e1 : {R.STri{a0, a1, a2, a3, a4, a5} == R.STri{a0, _, b2, b3, b4, b5} : R.Shape}
      %e2 : {R.STri{a0, a1, a2, a3, a4, a5} == R.STri{a0, a1, _, b3, b4, b5} : R.Shape}
      %e3 : {R.STri{a0, a1, a2, a3, a4, a5} == R.STri{a0, a1, a2, _, b4, b5} : R.Shape}
      %e4 : {R.STri{a0, a1, a2, a3, a4, a5} == R.STri{a0, a1, a2, a3, _, b5} : R.Shape}
      %e5 : {R.STri{a0, a1, a2, a3, a4, a5} == R.STri{a0, a1, a2, a3, a4, _} : R.Shape}
      {==}
    case R.STri{+a0, +a1, +a2, +a3, +a4, +a5} R.SMask{+b0, +b1, +b2, +b3, +b4}:
      Empty.absurd({R.STri{a0, a1, a2, a3, a4, a5} == R.SMask{b0, b1, b2, b3, b4} : R.Shape}, false_ne_true(h))
    case R.SMask{+a0, +a1, +a2, +a3, +a4} R.SRect{}:
      Empty.absurd({R.SMask{a0, a1, a2, a3, a4} == R.SRect{} : R.Shape}, false_ne_true(h))
    case R.SMask{+a0, +a1, +a2, +a3, +a4} R.SPoly{+b0, +b1}:
      Empty.absurd({R.SMask{a0, a1, a2, a3, a4} == R.SPoly{b0, b1} : R.Shape}, false_ne_true(h))
    case R.SMask{+a0, +a1, +a2, +a3, +a4} R.SGlyph{+b0, +b1, +b2}:
      Empty.absurd({R.SMask{a0, a1, a2, a3, a4} == R.SGlyph{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SMask{+a0, +a1, +a2, +a3, +a4} R.SSeg{+b0, +b1, +b2, +b3, +b4}:
      Empty.absurd({R.SMask{a0, a1, a2, a3, a4} == R.SSeg{b0, b1, b2, b3, b4} : R.Shape}, false_ne_true(h))
    case R.SMask{+a0, +a1, +a2, +a3, +a4} R.SDisc{+b0, +b1, +b2}:
      Empty.absurd({R.SMask{a0, a1, a2, a3, a4} == R.SDisc{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SMask{+a0, +a1, +a2, +a3, +a4} R.STri{+b0, +b1, +b2, +b3, +b4, +b5}:
      Empty.absurd({R.SMask{a0, a1, a2, a3, a4} == R.STri{b0, b1, b2, b3, b4, b5} : R.Shape}, false_ne_true(h))
    case R.SMask{+a0, +a1, +a2, +a3, +a4} R.SMask{+b0, +b1, +b2, +b3, +b4}:
      +hl0 = and_l(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(Nat.is_eq(a3, b3), T.Eq.mask(a4, b4)))), h)
      +hr0 = and_r(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(Nat.is_eq(a3, b3), T.Eq.mask(a4, b4)))), h)
      e0 = u32_eq_sound(a0, b0, hl0)
      +hl1 = and_l(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(Nat.is_eq(a3, b3), T.Eq.mask(a4, b4))), hr0)
      +hr1 = and_r(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(Nat.is_eq(a3, b3), T.Eq.mask(a4, b4))), hr0)
      e1 = u32_eq_sound(a1, b1, hl1)
      +hl2 = and_l(U32.is_eq(a2, b2), Bool.and(Nat.is_eq(a3, b3), T.Eq.mask(a4, b4)), hr1)
      +hr2 = and_r(U32.is_eq(a2, b2), Bool.and(Nat.is_eq(a3, b3), T.Eq.mask(a4, b4)), hr1)
      e2 = u32_eq_sound(a2, b2, hl2)
      +hl3 = and_l(Nat.is_eq(a3, b3), T.Eq.mask(a4, b4), hr2)
      +hr3 = and_r(Nat.is_eq(a3, b3), T.Eq.mask(a4, b4), hr2)
      e3 = nat_eq_sound(a3, b3, hl3)
      e4 = eq_mask_sound(a4, b4, hr3)
      %e0 : {R.SMask{a0, a1, a2, a3, a4} == R.SMask{_, b1, b2, b3, b4} : R.Shape}
      %e1 : {R.SMask{a0, a1, a2, a3, a4} == R.SMask{a0, _, b2, b3, b4} : R.Shape}
      %e2 : {R.SMask{a0, a1, a2, a3, a4} == R.SMask{a0, a1, _, b3, b4} : R.Shape}
      %e3 : {R.SMask{a0, a1, a2, a3, a4} == R.SMask{a0, a1, a2, _, b4} : R.Shape}
      %e4 : {R.SMask{a0, a1, a2, a3, a4} == R.SMask{a0, a1, a2, a3, _} : R.Shape}
      {==}
    case R.SRect{} R.SRun{+b0, +b1, +b2}:
      Empty.absurd({R.SRect{} == R.SRun{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SRun{+a0, +a1, +a2} R.SRect{}:
      Empty.absurd({R.SRun{a0, a1, a2} == R.SRect{} : R.Shape}, false_ne_true(h))
    case R.SPoly{+a0, +a1} R.SRun{+b0, +b1, +b2}:
      Empty.absurd({R.SPoly{a0, a1} == R.SRun{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SRun{+a0, +a1, +a2} R.SPoly{+b0, +b1}:
      Empty.absurd({R.SRun{a0, a1, a2} == R.SPoly{b0, b1} : R.Shape}, false_ne_true(h))
    case R.SGlyph{+a0, +a1, +a2} R.SRun{+b0, +b1, +b2}:
      Empty.absurd({R.SGlyph{a0, a1, a2} == R.SRun{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SRun{+a0, +a1, +a2} R.SGlyph{+b0, +b1, +b2}:
      Empty.absurd({R.SRun{a0, a1, a2} == R.SGlyph{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SSeg{+a0, +a1, +a2, +a3, +a4} R.SRun{+b0, +b1, +b2}:
      Empty.absurd({R.SSeg{a0, a1, a2, a3, a4} == R.SRun{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SRun{+a0, +a1, +a2} R.SSeg{+b0, +b1, +b2, +b3, +b4}:
      Empty.absurd({R.SRun{a0, a1, a2} == R.SSeg{b0, b1, b2, b3, b4} : R.Shape}, false_ne_true(h))
    case R.SDisc{+a0, +a1, +a2} R.SRun{+b0, +b1, +b2}:
      Empty.absurd({R.SDisc{a0, a1, a2} == R.SRun{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SRun{+a0, +a1, +a2} R.SDisc{+b0, +b1, +b2}:
      Empty.absurd({R.SRun{a0, a1, a2} == R.SDisc{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.STri{+a0, +a1, +a2, +a3, +a4, +a5} R.SRun{+b0, +b1, +b2}:
      Empty.absurd({R.STri{a0, a1, a2, a3, a4, a5} == R.SRun{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SRun{+a0, +a1, +a2} R.STri{+b0, +b1, +b2, +b3, +b4, +b5}:
      Empty.absurd({R.SRun{a0, a1, a2} == R.STri{b0, b1, b2, b3, b4, b5} : R.Shape}, false_ne_true(h))
    case R.SMask{+a0, +a1, +a2, +a3, +a4} R.SRun{+b0, +b1, +b2}:
      Empty.absurd({R.SMask{a0, a1, a2, a3, a4} == R.SRun{b0, b1, b2} : R.Shape}, false_ne_true(h))
    case R.SRun{+a0, +a1, +a2} R.SMask{+b0, +b1, +b2, +b3, +b4}:
      Empty.absurd({R.SRun{a0, a1, a2} == R.SMask{b0, b1, b2, b3, b4} : R.Shape}, false_ne_true(h))
    case R.SRun{+a0, +a1, +a2} R.SRun{+b0, +b1, +b2}:
      +hl0 = and_l(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), T.Eq.run(a2, b2)), h)
      +hr0 = and_r(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), T.Eq.run(a2, b2)), h)
      e0 = u32_eq_sound(a0, b0, hl0)
      +hl1 = and_l(U32.is_eq(a1, b1), T.Eq.run(a2, b2), hr0)
      +hr1 = and_r(U32.is_eq(a1, b1), T.Eq.run(a2, b2), hr0)
      e1 = u32_eq_sound(a1, b1, hl1)
      e2 = eq_run_sound(a2, b2, hr1)
      %e0 : {R.SRun{a0, a1, a2} == R.SRun{_, b1, b2} : R.Shape}
      %e1 : {R.SRun{a0, a1, a2} == R.SRun{a0, _, b2} : R.Shape}
      %e2 : {R.SRun{a0, a1, a2} == R.SRun{a0, a1, _} : R.Shape}
      {==}


law eq_shape_refl:
  for +a: R.Shape
  {T.Eq.shape(a, a) == True{} : Bool}

def eq_shape_refl(a):
  match a:
    case R.SRect{}:
      {==}
    case R.SPoly{+a0, +a1}:
      %Equal.sym(Bool, Q.Eq.bool(a0, a0), True{}, bool_eq_refl(a0)) : {Bool.and(_, T.Eq.edges(a1, a1)) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.edges(a1, a1), True{}, eq_edges_refl(a1)) : {Bool.and(True{}, _) == True{} : Bool}
      {==}
    case R.SGlyph{+a0, +a1, +a2}:
      %Equal.sym(Bool, U32.is_eq(a0, a0), True{}, u32_eq_refl(a0)) : {Bool.and(_, Bool.and(U32.is_eq(a1, a1), T.Eq.glyph(a2, a2))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a1, a1), True{}, u32_eq_refl(a1)) : {Bool.and(True{}, Bool.and(_, T.Eq.glyph(a2, a2))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.glyph(a2, a2), True{}, eq_glyph_refl(a2)) : {Bool.and(True{}, Bool.and(True{}, _)) == True{} : Bool}
      {==}
    case R.SSeg{+a0, +a1, +a2, +a3, +a4}:
      %Equal.sym(Bool, T.Eq.f32(a0, a0), True{}, f32_eq_refl(a0)) : {Bool.and(_, Bool.and(T.Eq.f32(a1, a1), Bool.and(T.Eq.f32(a2, a2), Bool.and(T.Eq.f32(a3, a3), T.Eq.f32(a4, a4))))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.f32(a1, a1), True{}, f32_eq_refl(a1)) : {Bool.and(True{}, Bool.and(_, Bool.and(T.Eq.f32(a2, a2), Bool.and(T.Eq.f32(a3, a3), T.Eq.f32(a4, a4))))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.f32(a2, a2), True{}, f32_eq_refl(a2)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(T.Eq.f32(a3, a3), T.Eq.f32(a4, a4))))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.f32(a3, a3), True{}, f32_eq_refl(a3)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, T.Eq.f32(a4, a4))))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.f32(a4, a4), True{}, f32_eq_refl(a4)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, _)))) == True{} : Bool}
      {==}
    case R.SDisc{+a0, +a1, +a2}:
      %Equal.sym(Bool, T.Eq.f32(a0, a0), True{}, f32_eq_refl(a0)) : {Bool.and(_, Bool.and(T.Eq.f32(a1, a1), T.Eq.f32(a2, a2))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.f32(a1, a1), True{}, f32_eq_refl(a1)) : {Bool.and(True{}, Bool.and(_, T.Eq.f32(a2, a2))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.f32(a2, a2), True{}, f32_eq_refl(a2)) : {Bool.and(True{}, Bool.and(True{}, _)) == True{} : Bool}
      {==}
    case R.STri{+a0, +a1, +a2, +a3, +a4, +a5}:
      %Equal.sym(Bool, T.Eq.f32(a0, a0), True{}, f32_eq_refl(a0)) : {Bool.and(_, Bool.and(T.Eq.f32(a1, a1), Bool.and(T.Eq.f32(a2, a2), Bool.and(T.Eq.f32(a3, a3), Bool.and(T.Eq.f32(a4, a4), T.Eq.f32(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.f32(a1, a1), True{}, f32_eq_refl(a1)) : {Bool.and(True{}, Bool.and(_, Bool.and(T.Eq.f32(a2, a2), Bool.and(T.Eq.f32(a3, a3), Bool.and(T.Eq.f32(a4, a4), T.Eq.f32(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.f32(a2, a2), True{}, f32_eq_refl(a2)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(T.Eq.f32(a3, a3), Bool.and(T.Eq.f32(a4, a4), T.Eq.f32(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.f32(a3, a3), True{}, f32_eq_refl(a3)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(T.Eq.f32(a4, a4), T.Eq.f32(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.f32(a4, a4), True{}, f32_eq_refl(a4)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, T.Eq.f32(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.f32(a5, a5), True{}, f32_eq_refl(a5)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, _))))) == True{} : Bool}
      {==}
    case R.SMask{+a0, +a1, +a2, +a3, +a4}:
      %Equal.sym(Bool, U32.is_eq(a0, a0), True{}, u32_eq_refl(a0)) : {Bool.and(_, Bool.and(U32.is_eq(a1, a1), Bool.and(U32.is_eq(a2, a2), Bool.and(Nat.is_eq(a3, a3), T.Eq.mask(a4, a4))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a1, a1), True{}, u32_eq_refl(a1)) : {Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a2, a2), Bool.and(Nat.is_eq(a3, a3), T.Eq.mask(a4, a4))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a2, a2), True{}, u32_eq_refl(a2)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(Nat.is_eq(a3, a3), T.Eq.mask(a4, a4))))) == True{} : Bool}
      %Equal.sym(Bool, Nat.is_eq(a3, a3), True{}, nat_eq_refl(a3)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, T.Eq.mask(a4, a4))))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.mask(a4, a4), True{}, eq_mask_refl(a4)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, _)))) == True{} : Bool}
      {==}
    case R.SRun{+a0, +a1, +a2}:
      %Equal.sym(Bool, U32.is_eq(a0, a0), True{}, u32_eq_refl(a0)) : {Bool.and(_, Bool.and(U32.is_eq(a1, a1), T.Eq.run(a2, a2))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a1, a1), True{}, u32_eq_refl(a1)) : {Bool.and(True{}, Bool.and(_, T.Eq.run(a2, a2))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.run(a2, a2), True{}, eq_run_refl(a2)) : {Bool.and(True{}, Bool.and(True{}, _)) == True{} : Bool}
      {==}


law eq_op_sound:
  for a: R.Op
  for b: R.Op
  for +h: {T.Eq.op(a, b) == True{} : Bool}
  {a == b : R.Op}

def eq_op_sound(a, b, h):
  match a b:
    case R.Op{+a0, +a1, +a2, +a3, +a4, +a5, +a6} R.Op{+b0, +b1, +b2, +b3, +b4, +b5, +b6}:
      +hl0 = and_l(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), T.Eq.shape(a6, b6)))))), h)
      +hr0 = and_r(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), T.Eq.shape(a6, b6)))))), h)
      e0 = u32_eq_sound(a0, b0, hl0)
      +hl1 = and_l(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), T.Eq.shape(a6, b6))))), hr0)
      +hr1 = and_r(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), T.Eq.shape(a6, b6))))), hr0)
      e1 = u32_eq_sound(a1, b1, hl1)
      +hl2 = and_l(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), T.Eq.shape(a6, b6)))), hr1)
      +hr2 = and_r(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), T.Eq.shape(a6, b6)))), hr1)
      e2 = u32_eq_sound(a2, b2, hl2)
      +hl3 = and_l(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), T.Eq.shape(a6, b6))), hr2)
      +hr3 = and_r(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), T.Eq.shape(a6, b6))), hr2)
      e3 = u32_eq_sound(a3, b3, hl3)
      +hl4 = and_l(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), T.Eq.shape(a6, b6)), hr3)
      +hr4 = and_r(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), T.Eq.shape(a6, b6)), hr3)
      e4 = u32_eq_sound(a4, b4, hl4)
      +hl5 = and_l(U32.is_eq(a5, b5), T.Eq.shape(a6, b6), hr4)
      +hr5 = and_r(U32.is_eq(a5, b5), T.Eq.shape(a6, b6), hr4)
      e5 = u32_eq_sound(a5, b5, hl5)
      e6 = eq_shape_sound(a6, b6, hr5)
      %e0 : {R.Op{a0, a1, a2, a3, a4, a5, a6} == R.Op{_, b1, b2, b3, b4, b5, b6} : R.Op}
      %e1 : {R.Op{a0, a1, a2, a3, a4, a5, a6} == R.Op{a0, _, b2, b3, b4, b5, b6} : R.Op}
      %e2 : {R.Op{a0, a1, a2, a3, a4, a5, a6} == R.Op{a0, a1, _, b3, b4, b5, b6} : R.Op}
      %e3 : {R.Op{a0, a1, a2, a3, a4, a5, a6} == R.Op{a0, a1, a2, _, b4, b5, b6} : R.Op}
      %e4 : {R.Op{a0, a1, a2, a3, a4, a5, a6} == R.Op{a0, a1, a2, a3, _, b5, b6} : R.Op}
      %e5 : {R.Op{a0, a1, a2, a3, a4, a5, a6} == R.Op{a0, a1, a2, a3, a4, _, b6} : R.Op}
      %e6 : {R.Op{a0, a1, a2, a3, a4, a5, a6} == R.Op{a0, a1, a2, a3, a4, a5, _} : R.Op}
      {==}

law eq_op_refl:
  for +a: R.Op
  {T.Eq.op(a, a) == True{} : Bool}

def eq_op_refl(a):
  match a:
    case R.Op{+a0, +a1, +a2, +a3, +a4, +a5, +a6}:
      %Equal.sym(Bool, U32.is_eq(a0, a0), True{}, u32_eq_refl(a0)) : {Bool.and(_, Bool.and(U32.is_eq(a1, a1), Bool.and(U32.is_eq(a2, a2), Bool.and(U32.is_eq(a3, a3), Bool.and(U32.is_eq(a4, a4), Bool.and(U32.is_eq(a5, a5), T.Eq.shape(a6, a6))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a1, a1), True{}, u32_eq_refl(a1)) : {Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a2, a2), Bool.and(U32.is_eq(a3, a3), Bool.and(U32.is_eq(a4, a4), Bool.and(U32.is_eq(a5, a5), T.Eq.shape(a6, a6))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a2, a2), True{}, u32_eq_refl(a2)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a3, a3), Bool.and(U32.is_eq(a4, a4), Bool.and(U32.is_eq(a5, a5), T.Eq.shape(a6, a6))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a3, a3), True{}, u32_eq_refl(a3)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a4, a4), Bool.and(U32.is_eq(a5, a5), T.Eq.shape(a6, a6))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a4, a4), True{}, u32_eq_refl(a4)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(U32.is_eq(a5, a5), T.Eq.shape(a6, a6))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a5, a5), True{}, u32_eq_refl(a5)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, T.Eq.shape(a6, a6))))))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.shape(a6, a6), True{}, eq_shape_refl(a6)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, _)))))) == True{} : Bool}
      {==}

# the walk's flag is true when it answers true
law ops_go_ok:
  for xs: List<&2, R.Op>
  for ys: List<&2, R.Op>
  for ok: Bool
  for h: {T.Eq.ops.go(xs, ys, ok) == True{} : Bool}
  {ok == True{} : Bool}

def ops_go_ok(xs, ys, ok, h):
  match xs ys ok:
    case _ _ True{}:
      {==}
    case Nil{} Nil{} False{}:
      Empty.absurd({False{} == True{} : Bool}, false_ne_true(h))
    case Nil{} _ <> _ False{}:
      Empty.absurd({False{} == True{} : Bool}, false_ne_true(h))
    case _ <> _ Nil{} False{}:
      Empty.absurd({False{} == True{} : Bool}, false_ne_true(h))
    case _ <> _ _ <> _ False{}:
      Empty.absurd({False{} == True{} : Bool}, false_ne_true(h))

law ops_go_sound:
  for xs: List<&2, R.Op>
  for ys: List<&2, R.Op>
  for ok: Bool
  for +h: {T.Eq.ops.go(xs, ys, ok) == True{} : Bool}
  {xs == ys : List<&2, R.Op>}

def ops_go_sound(xs, ys, ok, h):
  match xs ys ok:
    case Nil{} Nil{} _:
      {==}
    case Nil{} +y <> +yt True{}:
      Empty.absurd({Nil{} == y <> yt : List<&2, R.Op>}, false_ne_true(h))
    case +x <> +xt Nil{} True{}:
      Empty.absurd({x <> xt == Nil{} : List<&2, R.Op>}, false_ne_true(h))
    case Nil{} +y <> +yt False{}:
      Empty.absurd({Nil{} == y <> yt : List<&2, R.Op>}, false_ne_true(h))
    case +x <> +xt Nil{} False{}:
      Empty.absurd({x <> xt == Nil{} : List<&2, R.Op>}, false_ne_true(h))
    case +x <> +xt +y <> +yt False{}:
      Empty.absurd({x <> xt == y <> yt : List<&2, R.Op>}, false_ne_true(h))
    case +x <> +xt +y <> +yt True{}:
      e0 = eq_op_sound(x, y, ops_go_ok(xt, yt, T.Eq.op(x, y), h))
      e1 = ops_go_sound(xt, yt, T.Eq.op(x, y), h)
      %e0 : {x <> xt == _ <> yt : List<&2, R.Op>}
      %e1 : {x <> xt == x <> _ : List<&2, R.Op>}
      {==}

law eq_ops_sound:
  for a: List<&2, R.Op>
  for b: List<&2, R.Op>
  for +h: {T.Eq.ops(a, b) == True{} : Bool}
  {a == b : List<&2, R.Op>}

def eq_ops_sound(a, b, h):
  ops_go_sound(a, b, True{}, h)

law eq_ops_refl:
  for +a: List<&2, R.Op>
  {T.Eq.ops(a, a) == True{} : Bool}

def eq_ops_refl(a):
  match a:
    case Nil{}:
      {==}
    case +x <> +xt:
      %Equal.sym(Bool, T.Eq.op(x, x), True{}, eq_op_refl(x)) : {T.Eq.ops.go(xt, xt, _) == True{} : Bool}
      eq_ops_refl(xt)

law image_go_ok:
  for a: Image
  for b: Image
  for ok: Bool
  for h: {T.Eq.image.go(a, b, ok) == True{} : Bool}
  {ok == True{} : Bool}

def image_go_ok(a, b, ok, h):
  match a b ok:
    case _ _ True{}:
      {==}
    case Pix{_} Pix{_} False{}:
      Empty.absurd({False{} == True{} : Bool}, false_ne_true(h))
    case Pix{_} Qua{_, _, _, _} False{}:
      Empty.absurd({False{} == True{} : Bool}, false_ne_true(h))
    case Qua{_, _, _, _} Pix{_} False{}:
      Empty.absurd({False{} == True{} : Bool}, false_ne_true(h))
    case Qua{_, _, _, _} Qua{_, _, _, _} False{}:
      Empty.absurd({False{} == True{} : Bool}, false_ne_true(h))

law image_go_sound:
  for a: Image
  for b: Image
  for ok: Bool
  for +h: {T.Eq.image.go(a, b, ok) == True{} : Bool}
  {a == b : Image}

def image_go_sound(a, b, ok, h):
  match a b ok:
    case Pix{+a0} Pix{+b0} True{}:
      e0 = u32_eq_sound(a0, b0, h)
      %e0 : {Pix{a0} == Pix{_} : Image}
      {==}
    case Pix{+a0} Qua{+b0, +b1, +b2, +b3} True{}:
      Empty.absurd({Pix{a0} == Qua{b0, b1, b2, b3} : Image}, false_ne_true(h))
    case Qua{+a0, +a1, +a2, +a3} Pix{+b0} True{}:
      Empty.absurd({Qua{a0, a1, a2, a3} == Pix{b0} : Image}, false_ne_true(h))
    case Pix{+a0} Pix{+b0} False{}:
      Empty.absurd({Pix{a0} == Pix{b0} : Image}, false_ne_true(h))
    case Pix{+a0} Qua{+b0, +b1, +b2, +b3} False{}:
      Empty.absurd({Pix{a0} == Qua{b0, b1, b2, b3} : Image}, false_ne_true(h))
    case Qua{+a0, +a1, +a2, +a3} Pix{+b0} False{}:
      Empty.absurd({Qua{a0, a1, a2, a3} == Pix{b0} : Image}, false_ne_true(h))
    case Qua{+a0, +a1, +a2, +a3} Qua{+b0, +b1, +b2, +b3} False{}:
      Empty.absurd({Qua{a0, a1, a2, a3} == Qua{b0, b1, b2, b3} : Image}, false_ne_true(h))
    case Qua{+a0, +a1, +a2, +a3} Qua{+b0, +b1, +b2, +b3} True{}:
      +g0 = T.Eq.image.go(a0, b0, True{})
      +g1 = T.Eq.image.go(a1, b1, g0)
      +g2 = T.Eq.image.go(a2, b2, g1)
      e3 = image_go_sound(a3, b3, g2, h)
      +h2 = image_go_ok(a3, b3, g2, h)
      e2 = image_go_sound(a2, b2, g1, h2)
      +h1 = image_go_ok(a2, b2, g1, h2)
      e1 = image_go_sound(a1, b1, g0, h1)
      +h0 = image_go_ok(a1, b1, g0, h1)
      e0 = image_go_sound(a0, b0, True{}, h0)
      %e0 : {Qua{a0, a1, a2, a3} == Qua{_, b1, b2, b3} : Image}
      %e1 : {Qua{a0, a1, a2, a3} == Qua{a0, _, b2, b3} : Image}
      %e2 : {Qua{a0, a1, a2, a3} == Qua{a0, a1, _, b3} : Image}
      %e3 : {Qua{a0, a1, a2, a3} == Qua{a0, a1, a2, _} : Image}
      {==}

law eq_image_sound:
  for a: Image
  for b: Image
  for +h: {T.Eq.image(a, b) == True{} : Bool}
  {a == b : Image}

def eq_image_sound(a, b, h):
  image_go_sound(a, b, True{}, h)

law image_go_refl:
  for +a: Image
  {T.Eq.image.go(a, a, True{}) == True{} : Bool}

def image_go_refl(a):
  match a:
    case Pix{+a0}:
      %Equal.sym(Bool, U32.is_eq(a0, a0), True{}, u32_eq_refl(a0)) : {_ == True{} : Bool}
      {==}
    case Qua{+a0, +a1, +a2, +a3}:
      %Equal.sym(Bool, T.Eq.image.go(a0, a0, True{}), True{}, image_go_refl(a0)) : {T.Eq.image.go(a3, a3, T.Eq.image.go(a2, a2, T.Eq.image.go(a1, a1, _))) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.image.go(a1, a1, True{}), True{}, image_go_refl(a1)) : {T.Eq.image.go(a3, a3, T.Eq.image.go(a2, a2, _)) == True{} : Bool}
      %Equal.sym(Bool, T.Eq.image.go(a2, a2, True{}), True{}, image_go_refl(a2)) : {T.Eq.image.go(a3, a3, _) == True{} : Bool}
      image_go_refl(a3)

law eq_image_refl:
  for +a: Image
  {T.Eq.image(a, a) == True{} : Bool}

def eq_image_refl(a):
  image_go_refl(a)

law tile_k_l:
  for s: Bool
  for -a: Image
  for -b: Image
  for h: {T.Eq.tile.k(s, a, b) == True{} : Bool}
  {s == True{} : Bool}

def tile_k_l(s, a, b, h):
  match s:
    case False{}:
      Empty.absurd({False{} == True{} : Bool}, false_ne_true(h))
    case True{}:
      {==}

law tile_k_r:
  for s: Bool
  for a: Image
  for b: Image
  for h: {T.Eq.tile.k(s, a, b) == True{} : Bool}
  {T.Eq.image(a, b) == True{} : Bool}

def tile_k_r(s, a, b, h):
  match s:
    case False{}:
      Empty.absurd({T.Eq.image(a, b) == True{} : Bool}, false_ne_true(h))
    case True{}:
      h

# the ops are the same when a tile's comparison holds
law tile_l:
  for xs: List<&2, R.Op>
  for ys: List<&2, R.Op>
  for -a: Image
  for -b: Image
  for h: {T.Eq.tile(xs, ys, a, b) == True{} : Bool}
  {T.Eq.ops(xs, ys) == True{} : Bool}

def tile_l(xs, ys, a, b, h):
  tile_k_l(T.Eq.ops(xs, ys), a, b, h)

# and so are the bases
law tile_r:
  for xs: List<&2, R.Op>
  for ys: List<&2, R.Op>
  for a: Image
  for b: Image
  for h: {T.Eq.tile(xs, ys, a, b) == True{} : Bool}
  {T.Eq.image(a, b) == True{} : Bool}

def tile_r(xs, ys, a, b, h):
  tile_k_r(T.Eq.ops(xs, ys), a, b, h)

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

law eq_chr_refl:
  for +a: Char
  {Q.Eq.chr(a, a) == True{} : Bool}

def eq_chr_refl(a):
  match a:
    case Chr{+a0}:
      %Equal.sym(Bool, U32.is_eq(a0, a0), True{}, u32_eq_refl(a0)) : {_ == True{} : Bool}
      {==}

law eq_str_sound:
  for a: String
  for b: String
  for +h: {Q.Eq.str(a, b) == True{} : Bool}
  {a == b : String}

def eq_str_sound(a, b, h):
  match a b:
    case SNil{} SNil{}:
      {==}
    case SNil{} SCon{+y, +yt}:
      Empty.absurd({SNil{} == SCon{y, yt} : String}, false_ne_true(h))
    case SCon{+x, +xt} SNil{}:
      Empty.absurd({SCon{x, xt} == SNil{} : String}, false_ne_true(h))
    case SCon{+x, +xt} SCon{+y, +yt}:
      e0 = eq_chr_sound(x, y, and_l(Q.Eq.chr(x, y), Q.Eq.str(xt, yt), h))
      e1 = eq_str_sound(xt, yt, and_r(Q.Eq.chr(x, y), Q.Eq.str(xt, yt), h))
      %e0 : {SCon{x, xt} == SCon{_, yt} : String}
      %e1 : {SCon{x, xt} == SCon{x, _} : String}
      {==}

law eq_str_refl:
  for +a: String
  {Q.Eq.str(a, a) == True{} : Bool}

def eq_str_refl(a):
  match a:
    case SNil{}:
      {==}
    case SCon{+x, +xt}:
      %Equal.sym(Bool, Q.Eq.chr(x, x), True{}, eq_chr_refl(x)) : {Bool.and(_, Q.Eq.str(xt, xt)) == True{} : Bool}
      eq_str_refl(xt)

law eq_role_sound:
  for a: M.Role
  for b: M.Role
  for +h: {Q.Eq.role(a, b) == True{} : Bool}
  {a == b : M.Role}

def eq_role_sound(a, b, h):
  match a b:
    case M.User{} M.User{}:
      {==}
    case M.User{} M.Assistant{}:
      Empty.absurd({M.User{} == M.Assistant{} : M.Role}, false_ne_true(h))
    case M.User{} M.System{}:
      Empty.absurd({M.User{} == M.System{} : M.Role}, false_ne_true(h))
    case M.Assistant{} M.User{}:
      Empty.absurd({M.Assistant{} == M.User{} : M.Role}, false_ne_true(h))
    case M.Assistant{} M.Assistant{}:
      {==}
    case M.Assistant{} M.System{}:
      Empty.absurd({M.Assistant{} == M.System{} : M.Role}, false_ne_true(h))
    case M.System{} M.User{}:
      Empty.absurd({M.System{} == M.User{} : M.Role}, false_ne_true(h))
    case M.System{} M.Assistant{}:
      Empty.absurd({M.System{} == M.Assistant{} : M.Role}, false_ne_true(h))
    case M.System{} M.System{}:
      {==}

law eq_role_refl:
  for +a: M.Role
  {Q.Eq.role(a, a) == True{} : Bool}

def eq_role_refl(a):
  match a:
    case M.User{}:
      {==}
    case M.Assistant{}:
      {==}
    case M.System{}:
      {==}

law eq_entry_sound:
  for a: M.Entry
  for b: M.Entry
  for +h: {Q.Eq.entry(a, b) == True{} : Bool}
  {a == b : M.Entry}

def eq_entry_sound(a, b, h):
  match a b:
    case M.Msg{+a0, +a1, +a2, +a3, +a4} M.Msg{+b0, +b1, +b2, +b3, +b4}:
      +hl0 = and_l(Q.Eq.str(a0, b0), Bool.and(Q.Eq.str(a1, b1), Bool.and(Q.Eq.role(a2, b2), Bool.and(Q.Eq.str(a3, b3), Nat.is_eq(a4, b4)))), h)
      +hr0 = and_r(Q.Eq.str(a0, b0), Bool.and(Q.Eq.str(a1, b1), Bool.and(Q.Eq.role(a2, b2), Bool.and(Q.Eq.str(a3, b3), Nat.is_eq(a4, b4)))), h)
      e0 = eq_str_sound(a0, b0, hl0)
      +hl1 = and_l(Q.Eq.str(a1, b1), Bool.and(Q.Eq.role(a2, b2), Bool.and(Q.Eq.str(a3, b3), Nat.is_eq(a4, b4))), hr0)
      +hr1 = and_r(Q.Eq.str(a1, b1), Bool.and(Q.Eq.role(a2, b2), Bool.and(Q.Eq.str(a3, b3), Nat.is_eq(a4, b4))), hr0)
      e1 = eq_str_sound(a1, b1, hl1)
      +hl2 = and_l(Q.Eq.role(a2, b2), Bool.and(Q.Eq.str(a3, b3), Nat.is_eq(a4, b4)), hr1)
      +hr2 = and_r(Q.Eq.role(a2, b2), Bool.and(Q.Eq.str(a3, b3), Nat.is_eq(a4, b4)), hr1)
      e2 = eq_role_sound(a2, b2, hl2)
      +hl3 = and_l(Q.Eq.str(a3, b3), Nat.is_eq(a4, b4), hr2)
      +hr3 = and_r(Q.Eq.str(a3, b3), Nat.is_eq(a4, b4), hr2)
      e3 = eq_str_sound(a3, b3, hl3)
      e4 = nat_eq_sound(a4, b4, hr3)
      %e0 : {M.Msg{a0, a1, a2, a3, a4} == M.Msg{_, b1, b2, b3, b4} : M.Entry}
      %e1 : {M.Msg{a0, a1, a2, a3, a4} == M.Msg{a0, _, b2, b3, b4} : M.Entry}
      %e2 : {M.Msg{a0, a1, a2, a3, a4} == M.Msg{a0, a1, _, b3, b4} : M.Entry}
      %e3 : {M.Msg{a0, a1, a2, a3, a4} == M.Msg{a0, a1, a2, _, b4} : M.Entry}
      %e4 : {M.Msg{a0, a1, a2, a3, a4} == M.Msg{a0, a1, a2, a3, _} : M.Entry}
      {==}
    case M.Msg{+a0, +a1, +a2, +a3, +a4} M.Act{+b0, +b1, +b2, +b3, +b4, +b5}:
      Empty.absurd({M.Msg{a0, a1, a2, a3, a4} == M.Act{b0, b1, b2, b3, b4, b5} : M.Entry}, false_ne_true(h))
    case M.Act{+a0, +a1, +a2, +a3, +a4, +a5} M.Msg{+b0, +b1, +b2, +b3, +b4}:
      Empty.absurd({M.Act{a0, a1, a2, a3, a4, a5} == M.Msg{b0, b1, b2, b3, b4} : M.Entry}, false_ne_true(h))
    case M.Act{+a0, +a1, +a2, +a3, +a4, +a5} M.Act{+b0, +b1, +b2, +b3, +b4, +b5}:
      +hl0 = and_l(Q.Eq.str(a0, b0), Bool.and(Q.Eq.str(a1, b1), Bool.and(Q.Eq.str(a2, b2), Bool.and(Q.Eq.str(a3, b3), Bool.and(Q.Eq.str(a4, b4), Nat.is_eq(a5, b5))))), h)
      +hr0 = and_r(Q.Eq.str(a0, b0), Bool.and(Q.Eq.str(a1, b1), Bool.and(Q.Eq.str(a2, b2), Bool.and(Q.Eq.str(a3, b3), Bool.and(Q.Eq.str(a4, b4), Nat.is_eq(a5, b5))))), h)
      e0 = eq_str_sound(a0, b0, hl0)
      +hl1 = and_l(Q.Eq.str(a1, b1), Bool.and(Q.Eq.str(a2, b2), Bool.and(Q.Eq.str(a3, b3), Bool.and(Q.Eq.str(a4, b4), Nat.is_eq(a5, b5)))), hr0)
      +hr1 = and_r(Q.Eq.str(a1, b1), Bool.and(Q.Eq.str(a2, b2), Bool.and(Q.Eq.str(a3, b3), Bool.and(Q.Eq.str(a4, b4), Nat.is_eq(a5, b5)))), hr0)
      e1 = eq_str_sound(a1, b1, hl1)
      +hl2 = and_l(Q.Eq.str(a2, b2), Bool.and(Q.Eq.str(a3, b3), Bool.and(Q.Eq.str(a4, b4), Nat.is_eq(a5, b5))), hr1)
      +hr2 = and_r(Q.Eq.str(a2, b2), Bool.and(Q.Eq.str(a3, b3), Bool.and(Q.Eq.str(a4, b4), Nat.is_eq(a5, b5))), hr1)
      e2 = eq_str_sound(a2, b2, hl2)
      +hl3 = and_l(Q.Eq.str(a3, b3), Bool.and(Q.Eq.str(a4, b4), Nat.is_eq(a5, b5)), hr2)
      +hr3 = and_r(Q.Eq.str(a3, b3), Bool.and(Q.Eq.str(a4, b4), Nat.is_eq(a5, b5)), hr2)
      e3 = eq_str_sound(a3, b3, hl3)
      +hl4 = and_l(Q.Eq.str(a4, b4), Nat.is_eq(a5, b5), hr3)
      +hr4 = and_r(Q.Eq.str(a4, b4), Nat.is_eq(a5, b5), hr3)
      e4 = eq_str_sound(a4, b4, hl4)
      e5 = nat_eq_sound(a5, b5, hr4)
      %e0 : {M.Act{a0, a1, a2, a3, a4, a5} == M.Act{_, b1, b2, b3, b4, b5} : M.Entry}
      %e1 : {M.Act{a0, a1, a2, a3, a4, a5} == M.Act{a0, _, b2, b3, b4, b5} : M.Entry}
      %e2 : {M.Act{a0, a1, a2, a3, a4, a5} == M.Act{a0, a1, _, b3, b4, b5} : M.Entry}
      %e3 : {M.Act{a0, a1, a2, a3, a4, a5} == M.Act{a0, a1, a2, _, b4, b5} : M.Entry}
      %e4 : {M.Act{a0, a1, a2, a3, a4, a5} == M.Act{a0, a1, a2, a3, _, b5} : M.Entry}
      %e5 : {M.Act{a0, a1, a2, a3, a4, a5} == M.Act{a0, a1, a2, a3, a4, _} : M.Entry}
      {==}

law eq_entry_refl:
  for +a: M.Entry
  {Q.Eq.entry(a, a) == True{} : Bool}

def eq_entry_refl(a):
  match a:
    case M.Msg{+a0, +a1, +a2, +a3, +a4}:
      %Equal.sym(Bool, Q.Eq.str(a0, a0), True{}, eq_str_refl(a0)) : {Bool.and(_, Bool.and(Q.Eq.str(a1, a1), Bool.and(Q.Eq.role(a2, a2), Bool.and(Q.Eq.str(a3, a3), Nat.is_eq(a4, a4))))) == True{} : Bool}
      %Equal.sym(Bool, Q.Eq.str(a1, a1), True{}, eq_str_refl(a1)) : {Bool.and(True{}, Bool.and(_, Bool.and(Q.Eq.role(a2, a2), Bool.and(Q.Eq.str(a3, a3), Nat.is_eq(a4, a4))))) == True{} : Bool}
      %Equal.sym(Bool, Q.Eq.role(a2, a2), True{}, eq_role_refl(a2)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(Q.Eq.str(a3, a3), Nat.is_eq(a4, a4))))) == True{} : Bool}
      %Equal.sym(Bool, Q.Eq.str(a3, a3), True{}, eq_str_refl(a3)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Nat.is_eq(a4, a4))))) == True{} : Bool}
      %Equal.sym(Bool, Nat.is_eq(a4, a4), True{}, nat_eq_refl(a4)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, _)))) == True{} : Bool}
      {==}
    case M.Act{+a0, +a1, +a2, +a3, +a4, +a5}:
      %Equal.sym(Bool, Q.Eq.str(a0, a0), True{}, eq_str_refl(a0)) : {Bool.and(_, Bool.and(Q.Eq.str(a1, a1), Bool.and(Q.Eq.str(a2, a2), Bool.and(Q.Eq.str(a3, a3), Bool.and(Q.Eq.str(a4, a4), Nat.is_eq(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, Q.Eq.str(a1, a1), True{}, eq_str_refl(a1)) : {Bool.and(True{}, Bool.and(_, Bool.and(Q.Eq.str(a2, a2), Bool.and(Q.Eq.str(a3, a3), Bool.and(Q.Eq.str(a4, a4), Nat.is_eq(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, Q.Eq.str(a2, a2), True{}, eq_str_refl(a2)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(Q.Eq.str(a3, a3), Bool.and(Q.Eq.str(a4, a4), Nat.is_eq(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, Q.Eq.str(a3, a3), True{}, eq_str_refl(a3)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Bool.and(Q.Eq.str(a4, a4), Nat.is_eq(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, Q.Eq.str(a4, a4), True{}, eq_str_refl(a4)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(_, Nat.is_eq(a5, a5)))))) == True{} : Bool}
      %Equal.sym(Bool, Nat.is_eq(a5, a5), True{}, nat_eq_refl(a5)) : {Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, Bool.and(True{}, _))))) == True{} : Bool}
      {==}

# Drawing laws
# ------------

law seek_k:
  for s: Bool
  for +e: Nat
  for +lo: List<&2, R.Op>
  for +b: Image
  for +l: List<&2, R.Op>
  for +base: Image
  for +h: {T.Eq.tile(lo, l, b, base) == s : Bool}
  {T.Tiles.seek.k(s, T.Tile.draw(e, lo, b), e, l, base) == T.Tile.draw(e, l, base) : Image}

def seek_k(s, e, lo, b, l, base, h):
  match s:
    case False{}:
      {==}
    case True{}:
      eo = eq_ops_sound(lo, l, tile_l(lo, l, b, base, h))
      eb = eq_image_sound(b, base, tile_r(lo, l, b, base, h))
      %eo : {T.Tile.draw(e, lo, b) == T.Tile.draw(e, _, base) : Image}
      %eb : {T.Tile.draw(e, lo, b) == T.Tile.draw(e, lo, _) : Image}
      {==}

law seek_pick:
  for rt: Bool
  for dn: Bool
  for +j: Nat
  for +a: T.Tiles
  for +b: T.Tiles
  for +c: T.Tiles
  for +d: T.Tiles
  for +x: U32
  for +y: U32
  for +cx: U32
  for +cy: U32
  for +h: U32
  for +e: Nat
  for +l: List<&2, R.Op>
  for +base: Image
  for ha: {T.Tiles.seek(j, a, x, y, cx, cy, h, e, l, base) == T.Tile.draw(e, l, base) : Image}
  for hb: {T.Tiles.seek(j, b, x, y, cx, cy, h, e, l, base) == T.Tile.draw(e, l, base) : Image}
  for hc: {T.Tiles.seek(j, c, x, y, cx, cy, h, e, l, base) == T.Tile.draw(e, l, base) : Image}
  for hd: {T.Tiles.seek(j, d, x, y, cx, cy, h, e, l, base) == T.Tile.draw(e, l, base) : Image}
  {T.Tiles.seek(j, T.Tiles.pick(rt, dn, a, b, c, d), x, y, cx, cy, h, e, l, base) == T.Tile.draw(e, l, base) : Image}

def seek_pick(rt, dn, j, a, b, c, d, x, y, cx, cy, h, e, l, base, ha, hb, hc, hd):
  match rt dn:
    case False{} False{}:
      ha
    case True{} False{}:
      hb
    case False{} True{}:
      hc
    case True{} True{}:
      hd

def Laws.seek_made(k, e, old, b0, mx, my, mn, x, y, rx, ry, n, l, base):
  match k:
    case 0n:
      seek_k(T.Eq.tile(T.Loc.ops(old, mx, my, T.Tile.size(e)), l, b0, base), e, T.Loc.ops(old, mx, my, T.Tile.size(e)), b0, l, base, {==})
    case 1n+j0:
      +j = j0
      seek_pick(U32.is_ge(x, (rx + U32.shr(n) : U32)), U32.is_ge(y, (ry + U32.shr(n) : U32)), j,
        T.Tiles.make(j, e, R.Region.keep(old, mx, my, mx, my, U32.shr(mn)), T.Img.q(b0, 0n), mx, my, U32.shr(mn)),
        T.Tiles.make(j, e, R.Region.keep(old, mx, my, (mx + U32.shr(mn) : U32), my, U32.shr(mn)), T.Img.q(b0, 1n), (mx + U32.shr(mn) : U32), my, U32.shr(mn)),
        T.Tiles.make(j, e, R.Region.keep(old, mx, my, mx, (my + U32.shr(mn) : U32), U32.shr(mn)), T.Img.q(b0, 2n), mx, (my + U32.shr(mn) : U32), U32.shr(mn)),
        T.Tiles.make(j, e, R.Region.keep(old, mx, my, (mx + U32.shr(mn) : U32), (my + U32.shr(mn) : U32), U32.shr(mn)), T.Img.q(b0, 3n), (mx + U32.shr(mn) : U32), (my + U32.shr(mn) : U32), U32.shr(mn)),
        x, y, Bool.pick(U32, U32.is_ge(x, (rx + U32.shr(n) : U32)), (rx + U32.shr(n) : U32), rx), Bool.pick(U32, U32.is_ge(y, (ry + U32.shr(n) : U32)), (ry + U32.shr(n) : U32), ry), U32.shr(n), e, l, base,
        Laws.seek_made(j, e, R.Region.keep(old, mx, my, mx, my, U32.shr(mn)), T.Img.q(b0, 0n), mx, my, U32.shr(mn), x, y, Bool.pick(U32, U32.is_ge(x, (rx + U32.shr(n) : U32)), (rx + U32.shr(n) : U32), rx), Bool.pick(U32, U32.is_ge(y, (ry + U32.shr(n) : U32)), (ry + U32.shr(n) : U32), ry), U32.shr(n), l, base),
        Laws.seek_made(j, e, R.Region.keep(old, mx, my, (mx + U32.shr(mn) : U32), my, U32.shr(mn)), T.Img.q(b0, 1n), (mx + U32.shr(mn) : U32), my, U32.shr(mn), x, y, Bool.pick(U32, U32.is_ge(x, (rx + U32.shr(n) : U32)), (rx + U32.shr(n) : U32), rx), Bool.pick(U32, U32.is_ge(y, (ry + U32.shr(n) : U32)), (ry + U32.shr(n) : U32), ry), U32.shr(n), l, base),
        Laws.seek_made(j, e, R.Region.keep(old, mx, my, mx, (my + U32.shr(mn) : U32), U32.shr(mn)), T.Img.q(b0, 2n), mx, (my + U32.shr(mn) : U32), U32.shr(mn), x, y, Bool.pick(U32, U32.is_ge(x, (rx + U32.shr(n) : U32)), (rx + U32.shr(n) : U32), rx), Bool.pick(U32, U32.is_ge(y, (ry + U32.shr(n) : U32)), (ry + U32.shr(n) : U32), ry), U32.shr(n), l, base),
        Laws.seek_made(j, e, R.Region.keep(old, mx, my, (mx + U32.shr(mn) : U32), (my + U32.shr(mn) : U32), U32.shr(mn)), T.Img.q(b0, 3n), (mx + U32.shr(mn) : U32), (my + U32.shr(mn) : U32), U32.shr(mn), x, y, Bool.pick(U32, U32.is_ge(x, (rx + U32.shr(n) : U32)), (rx + U32.shr(n) : U32), rx), Bool.pick(U32, U32.is_ge(y, (ry + U32.shr(n) : U32)), (ry + U32.shr(n) : U32), ry), U32.shr(n), l, base))

law fresh_k:
  for look: Bool
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +l: List<&2, R.Op>
  for +base: Image
  for +pk: Nat
  for +old2: List<&2, R.Op>
  for +base2: Image
  for +prx: U32
  for +pry: U32
  for +pn: U32
  for +qn: U32
  for +dy: U32
  for +rx: U32
  for +ry: U32
  {T.Tile.fresh.k(look, e, ops, l, base, T.Past{pk, T.Tiles.make(pk, e, old2, base2, prx, pry, pn), qn, dy}, rx, ry) == T.Tile.of(e, ops, l, base) : T.Tile}

def fresh_k(look, e, ops, l, base, pk, old2, base2, prx, pry, pn, qn, dy, rx, ry):
  match look:
    case False{}:
      {==}
    case True{}:
      Equal.cong(Image, T.Tile, i => T.Tile{ops, l, base, i}, T.Tiles.seek(pk, T.Tiles.make(pk, e, old2, base2, prx, pry, pn), rx, (ry - dy : U32), 0, 0, qn, e, l, base), T.Tile.draw(e, l, base),
        Laws.seek_made(pk, e, old2, base2, prx, pry, pn, rx, (ry - dy : U32), 0, 0, qn, l, base))

law tile_again_k:
  for s: Bool
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +old: List<&2, R.Op>
  for +base0: Image
  for +base1: Image
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +pk: Nat
  for +old2: List<&2, R.Op>
  for +base2: Image
  for +prx: U32
  for +pry: U32
  for +pn: U32
  for +qn: U32
  for +dy: U32
  for +h: {T.Eq.tile(ops, old, base1, base0) == s : Bool}
  {T.Tile.again.k(s, e, ops, base1, T.Tile.make(e, old, base0, rx, ry, n), T.Past{pk, T.Tiles.make(pk, e, old2, base2, prx, pry, pn), qn, dy}, rx, ry, n) == T.Tile.make(e, ops, base1, rx, ry, n) : T.Tile}

def tile_again_k(s, e, ops, old, base0, base1, rx, ry, n, pk, old2, base2, prx, pry, pn, qn, dy, h):
  match s:
    case False{}:
      fresh_k(U32.is_ne(dy, 0), e, ops, T.Loc.ops(ops, rx, ry, T.Tile.size(e)), base1, pk, old2, base2, prx, pry, pn, qn, dy, rx, ry)
    case True{}:
      eo = eq_ops_sound(ops, old, tile_l(ops, old, base1, base0, h))
      eb = eq_image_sound(base1, base0, tile_r(ops, old, base1, base0, h))
      %Equal.sym(List<&2, R.Op>, ops, old, eo) : {T.Tile.make(e, old, base0, rx, ry, n) == T.Tile.make(e, _, base1, rx, ry, n) : T.Tile}
      %Equal.sym(Image, base1, base0, eb) : {T.Tile.make(e, old, base0, rx, ry, n) == T.Tile.make(e, old, _, rx, ry, n) : T.Tile}
      {==}

law tnode_cong:
  for -o: List<&2, R.Op>
  for -bs: Image
  for -a1: T.Tiles
  for -a2: T.Tiles
  for -b1: T.Tiles
  for -b2: T.Tiles
  for -c1: T.Tiles
  for -c2: T.Tiles
  for -d1: T.Tiles
  for -d2: T.Tiles
  for ea: {a1 == a2 : T.Tiles}
  for eb: {b1 == b2 : T.Tiles}
  for ec: {c1 == c2 : T.Tiles}
  for ed: {d1 == d2 : T.Tiles}
  {T.TNode{o, bs, a1, b1, c1, d1} == T.TNode{o, bs, a2, b2, c2, d2} : T.Tiles}

def tnode_cong(o, bs, a1, a2, b1, b2, c1, c2, d1, d2, ea, eb, ec, ed):
  %ea : {T.TNode{o, bs, a1, b1, c1, d1} == T.TNode{o, bs, _, b2, c2, d2} : T.Tiles}
  %eb : {T.TNode{o, bs, a1, b1, c1, d1} == T.TNode{o, bs, a1, _, c2, d2} : T.Tiles}
  %ec : {T.TNode{o, bs, a1, b1, c1, d1} == T.TNode{o, bs, a1, b1, _, d2} : T.Tiles}
  %ed : {T.TNode{o, bs, a1, b1, c1, d1} == T.TNode{o, bs, a1, b1, c1, _} : T.Tiles}
  {==}

# a node of the last frame's tiles: kept whole when its ops and base are
# unchanged (the same tiles, as made from equal ops over an equal base),
# else each quadrant again (ha..hd, the law one level down)
law again_node_k:
  for s: Bool
  for +j: Nat
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +old: List<&2, R.Op>
  for +base0: Image
  for +base1: Image
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +pk: Nat
  for +old2: List<&2, R.Op>
  for +base2: Image
  for +prx: U32
  for +pry: U32
  for +pn: U32
  for +qn: U32
  for +dy: U32
  for +h: {T.Eq.tile(ops, old, base1, base0) == s : Bool}
  for ha: {T.Tiles.again(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base1, 0n), T.Tiles.make(j, e, R.Region.keep(old, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base0, 0n), rx, ry, U32.shr(n)), T.Past{pk, T.Tiles.make(pk, e, old2, base2, prx, pry, pn), qn, dy}, rx, ry, U32.shr(n)) == T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base1, 0n), rx, ry, U32.shr(n)) : T.Tiles}
  for hb: {T.Tiles.again(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base1, 1n), T.Tiles.make(j, e, R.Region.keep(old, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base0, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Past{pk, T.Tiles.make(pk, e, old2, base2, prx, pry, pn), qn, dy}, (rx + U32.shr(n) : U32), ry, U32.shr(n)) == T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base1, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)) : T.Tiles}
  for hc: {T.Tiles.again(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base1, 2n), T.Tiles.make(j, e, R.Region.keep(old, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base0, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Past{pk, T.Tiles.make(pk, e, old2, base2, prx, pry, pn), qn, dy}, rx, (ry + U32.shr(n) : U32), U32.shr(n)) == T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base1, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)) : T.Tiles}
  for hd: {T.Tiles.again(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base1, 3n), T.Tiles.make(j, e, R.Region.keep(old, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base0, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Past{pk, T.Tiles.make(pk, e, old2, base2, prx, pry, pn), qn, dy}, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)) == T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base1, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)) : T.Tiles}
  {T.Tiles.again.go(1n+j, e, ops, base1, T.Tiles.make(1n+j, e, old, base0, rx, ry, n), T.Past{pk, T.Tiles.make(pk, e, old2, base2, prx, pry, pn), qn, dy}, rx, ry, n, s) == T.Tiles.make(1n+j, e, ops, base1, rx, ry, n) : T.Tiles}

def again_node_k(s, j, e, ops, old, base0, base1, rx, ry, n, pk, old2, base2, prx, pry, pn, qn, dy, h, ha, hb, hc, hd):
  match s:
    case True{}:
      eo = eq_ops_sound(ops, old, tile_l(ops, old, base1, base0, h))
      eb = eq_image_sound(base1, base0, tile_r(ops, old, base1, base0, h))
      %Equal.sym(List<&2, R.Op>, ops, old, eo) : {T.Tiles.make(1n+j, e, old, base0, rx, ry, n) == T.Tiles.make(1n+j, e, _, base1, rx, ry, n) : T.Tiles}
      %Equal.sym(Image, base1, base0, eb) : {T.Tiles.make(1n+j, e, old, base0, rx, ry, n) == T.Tiles.make(1n+j, e, old, _, rx, ry, n) : T.Tiles}
      {==}
    case False{}:
      tnode_cong(ops, base1, T.Tiles.again(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base1, 0n), T.Tiles.make(j, e, R.Region.keep(old, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base0, 0n), rx, ry, U32.shr(n)), T.Past{pk, T.Tiles.make(pk, e, old2, base2, prx, pry, pn), qn, dy}, rx, ry, U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base1, 0n), rx, ry, U32.shr(n)), T.Tiles.again(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base1, 1n), T.Tiles.make(j, e, R.Region.keep(old, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base0, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Past{pk, T.Tiles.make(pk, e, old2, base2, prx, pry, pn), qn, dy}, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base1, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Tiles.again(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base1, 2n), T.Tiles.make(j, e, R.Region.keep(old, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base0, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Past{pk, T.Tiles.make(pk, e, old2, base2, prx, pry, pn), qn, dy}, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base1, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Tiles.again(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base1, 3n), T.Tiles.make(j, e, R.Region.keep(old, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base0, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Past{pk, T.Tiles.make(pk, e, old2, base2, prx, pry, pn), qn, dy}, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base1, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), ha, hb, hc, hd)

def Laws.tiles_again_exact(k, e, ops, old, base0, base1, rx, ry, n, pk, old2, base2, prx, pry, pn, qn, dy):
  match k:
    case 0n:
      Equal.cong(T.Tile, T.Tiles, t => T.TLeaf{t}, T.Tile.again(e, ops, base1, T.Tile.make(e, old, base0, rx, ry, n), T.Past{pk, T.Tiles.make(pk, e, old2, base2, prx, pry, pn), qn, dy}, rx, ry, n), T.Tile.make(e, ops, base1, rx, ry, n),
        tile_again_k(T.Eq.tile(ops, old, base1, base0), e, ops, old, base0, base1, rx, ry, n, pk, old2, base2, prx, pry, pn, qn, dy, {==}))
    case 1n+j0:
      +j = j0
      again_node_k(T.Eq.tile(ops, old, base1, base0), j, e, ops, old, base0, base1, rx, ry, n, pk, old2, base2, prx, pry, pn, qn, dy, {==},
        Laws.tiles_again_exact(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), R.Region.keep(old, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base0, 0n), T.Img.q(base1, 0n), rx, ry, U32.shr(n), pk, old2, base2, prx, pry, pn, qn, dy),
        Laws.tiles_again_exact(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), R.Region.keep(old, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base0, 1n), T.Img.q(base1, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n), pk, old2, base2, prx, pry, pn, qn, dy),
        Laws.tiles_again_exact(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), R.Region.keep(old, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base0, 2n), T.Img.q(base1, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n), pk, old2, base2, prx, pry, pn, qn, dy),
        Laws.tiles_again_exact(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), R.Region.keep(old, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base0, 3n), T.Img.q(base1, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n), pk, old2, base2, prx, pry, pn, qn, dy))

def Laws.tile_keep(e, ops, l, base, img, p, rx, ry, n):
  %Equal.sym(Bool, T.Eq.ops(ops, ops), True{}, eq_ops_refl(ops)) : {T.Tile.again.k(T.Eq.tile.k(_, base, base), e, ops, base, T.Tile{ops, l, base, img}, p, rx, ry, n) == T.Tile{ops, l, base, img} : T.Tile}
  %Equal.sym(Bool, T.Eq.image(base, base), True{}, eq_image_refl(base)) : {T.Tile.again.k(_, e, ops, base, T.Tile{ops, l, base, img}, p, rx, ry, n) == T.Tile{ops, l, base, img} : T.Tile}
  {==}

law frame_again_k:
  for s: Bool
  for +w0: U32
  for +h0: U32
  for +w: U32
  for +h: U32
  for +ops: List<&2, R.Op>
  for +old: List<&2, R.Op>
  for +base0: Image
  for +base: Image
  for +dy: U32
  for +hs: {Bool.and(U32.is_eq(w0, w), U32.is_eq(h0, h)) == s : Bool}
  {T.Frame.again.k(s, w, h, ops, base, T.Frame.make(w0, h0, old, base0), dy) == T.Frame.make(w, h, ops, base) : T.Tiles}

def frame_again_k(s, w0, h0, w, h, ops, old, base0, base, dy, hs):
  match s:
    case False{}:
      {==}
    case True{}:
      ew = u32_eq_sound(w0, w, and_l(U32.is_eq(w0, w), U32.is_eq(h0, h), hs))
      eh = u32_eq_sound(h0, h, and_r(U32.is_eq(w0, w), U32.is_eq(h0, h), hs))
      %ew : {T.Tiles.again(T.Frame.levels(_, h), T.Frame.edge(_, h), ops, base, T.Frame.make(w0, h0, old, base0), T.Past{T.Frame.levels(_, h), T.Frame.make(w0, h0, old, base0), U32.shln(1, R.Frame.depth(_, h)), dy}, 0, 0, U32.shln(1, R.Frame.depth(_, h))) == T.Frame.make(_, h, ops, base) : T.Tiles}
      %eh : {T.Tiles.again(T.Frame.levels(w0, _), T.Frame.edge(w0, _), ops, base, T.Frame.make(w0, h0, old, base0), T.Past{T.Frame.levels(w0, _), T.Frame.make(w0, h0, old, base0), U32.shln(1, R.Frame.depth(w0, _)), dy}, 0, 0, U32.shln(1, R.Frame.depth(w0, _))) == T.Frame.make(w0, _, ops, base) : T.Tiles}
      Laws.tiles_again_exact(T.Frame.levels(w0, h0), T.Frame.edge(w0, h0), ops, old, base0, base, 0, 0, U32.shln(1, R.Frame.depth(w0, h0)),
        T.Frame.levels(w0, h0), old, base0, 0, 0, U32.shln(1, R.Frame.depth(w0, h0)), U32.shln(1, R.Frame.depth(w0, h0)), dy)

def Laws.frame_again_exact(w0, h0, w, h, ops, old, base0, base, dy):
  frame_again_k(Bool.and(U32.is_eq(w0, w), U32.is_eq(h0, h)), w0, h0, w, h, ops, old, base0, base, dy, {==})

# Drawing in parallel
# -------------------

law take_k:
  for s: Bool
  for +e: Nat
  for +l: List<&2, R.Op>
  for +l2: List<&2, R.Op>
  for +base: Image
  for +b2: Image
  for +h: {T.Eq.tile(l, l2, base, b2) == s : Bool}
  {T.Drawn.take(s, T.Tile.draw(e, l2, b2), e, l, base) == T.Tile.draw(e, l, base) : Image}

def take_k(s, e, l, l2, base, b2, h):
  match s:
    case False{}:
      {==}
    case True{}:
      eo = eq_ops_sound(l, l2, tile_l(l, l2, base, b2, h))
      eb = eq_image_sound(base, b2, tile_r(l, l2, base, b2, h))
      %eo : {T.Tile.draw(e, _, b2) == T.Tile.draw(e, l, base) : Image}
      %eb : {T.Tile.draw(e, l, _) == T.Tile.draw(e, l, base) : Image}
      {==}

def Laws.drawn_get_run(t, path, e, l, base):
  match t path:
    case T.JNone{} _:
      {==}
    case T.JOne{T.Job{+l2, +b2}} _:
      take_k(T.Eq.tile(l, l2, base, b2), e, l, l2, base, b2, {==})
    case T.JTwo{_, _} Nil{}:
      {==}
    case T.JTwo{a, _} False{} <> r:
      Laws.drawn_get_run(a, r, e, l, base)
    case T.JTwo{_, b} True{} <> r:
      Laws.drawn_get_run(b, r, e, l, base)

law plan_make_fill:
  for k: Nat
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +base: Image
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +off: Nat
  for +t: T.Jobs
  {T.Plan.fill(T.Tiles.plan.make(k, e, ops, base, rx, ry, n), e, off, T.Jobs.run(t, e)) == T.Tiles.make(k, e, ops, base, rx, ry, n) : T.Tiles}

def plan_make_fill(k, e, ops, base, rx, ry, n, off, t):
  match k:
    case 0n:
      Equal.cong(Image, T.Tiles, i => T.TLeaf{T.Tile{ops, T.Loc.ops(ops, rx, ry, T.Tile.size(e)), base, i}},
        T.Drawn.get(T.Jobs.run(t, e), T.Jobs.path(24n, off), e, T.Loc.ops(ops, rx, ry, T.Tile.size(e)), base), T.Tile.draw(e, T.Loc.ops(ops, rx, ry, T.Tile.size(e)), base),
        Laws.drawn_get_run(t, T.Jobs.path(24n, off), e, T.Loc.ops(ops, rx, ry, T.Tile.size(e)), base))
    case 1n+j0:
      +j = j0
      tnode_cong(ops, base, T.Plan.fill(T.Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)), e, off, T.Jobs.run(t, e)), T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)), T.Plan.fill(T.Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)), e, Nat.add(off, T.Plan.count(T.Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)))), T.Jobs.run(t, e)), T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Plan.fill(T.Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)), e, Nat.add(Nat.add(off, T.Plan.count(T.Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)))), T.Plan.count(T.Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)))), T.Jobs.run(t, e)), T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Plan.fill(T.Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), e, Nat.add(Nat.add(Nat.add(off, T.Plan.count(T.Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)))), T.Plan.count(T.Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)))), T.Plan.count(T.Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)))), T.Jobs.run(t, e)), T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)),
        plan_make_fill(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n), off, t),
        plan_make_fill(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n), Nat.add(off, T.Plan.count(T.Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)))), t),
        plan_make_fill(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n), Nat.add(Nat.add(off, T.Plan.count(T.Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)))), T.Plan.count(T.Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)))), t),
        plan_make_fill(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n), Nat.add(Nat.add(Nat.add(off, T.Plan.count(T.Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)))), T.Plan.count(T.Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)))), T.Plan.count(T.Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)))), t))


# Tiles.seek is Tiles.find with the drawing put back
def Seek.from(m: Maybe<&2, Image>, +e: Nat, +l: List<&2, R.Op>, +base: Image) -> Image:
  match m:
    case Some{i}:
      i
    case None{}:
      T.Tile.draw(e, l, base)

law find_k:
  for s: Bool
  for +img: Image
  for +e: Nat
  for +l: List<&2, R.Op>
  for +base: Image
  {T.Tiles.seek.k(s, img, e, l, base) == Seek.from(T.Tiles.find.k(s, img), e, l, base) : Image}

def find_k(s, img, e, l, base):
  match s:
    case True{}:
      {==}
    case False{}:
      {==}

law Laws_find:
  for k: Nat
  for t: T.Tiles
  for +x: U32
  for +y: U32
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +e: Nat
  for +l: List<&2, R.Op>
  for +base: Image
  {T.Tiles.seek(k, t, x, y, rx, ry, n, e, l, base) == Seek.from(T.Tiles.find(k, t, x, y, rx, ry, n, l, base), e, l, base) : Image}

def Laws_find(k, t, x, y, rx, ry, n, e, l, base):
  match k t:
    case 0n T.TLeaf{T.Tile{_, +o, +b, +i}}:
      find_k(T.Eq.tile(o, l, b, base), i, e, l, base)
    case 1n+j0 T.TNode{_, _, a, b, c, d}:
      +j = j0
      Laws_find(j, T.Tiles.pick(U32.is_ge(x, (rx + U32.shr(n) : U32)), U32.is_ge(y, (ry + U32.shr(n) : U32)), a, b, c, d), x, y,
        Bool.pick(U32, U32.is_ge(x, (rx + U32.shr(n) : U32)), (rx + U32.shr(n) : U32), rx), Bool.pick(U32, U32.is_ge(y, (ry + U32.shr(n) : U32)), (ry + U32.shr(n) : U32), ry), U32.shr(n), e, l, base)
    case 0n T.TNode{_, _, _, _, _, _}:
      {==}
    case 1n+j T.TLeaf{_}:
      {==}

# a found tile kept, a missing one drawn in parallel: the tile seek gives
law plan_found:
  for m: Maybe<&2, Image>
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +l: List<&2, R.Op>
  for +base: Image
  for +off: Nat
  for +t: T.Jobs
  for -s: Image
  for h: {s == Seek.from(m, e, l, base) : Image}
  {T.Plan.fill(T.Tile.plan.found(m, ops, l, base), e, off, T.Jobs.run(t, e)) == T.TLeaf{T.Tile{ops, l, base, s}} : T.Tiles}

def plan_found(m, e, ops, l, base, off, t, s, h):
  match m:
    case Some{i}:
      %Equal.sym(Image, s, Seek.from(Some{i}, e, l, base), h) : {T.Plan.fill(T.Tile.plan.found(Some{i}, ops, l, base), e, off, T.Jobs.run(t, e)) == T.TLeaf{T.Tile{ops, l, base, _}} : T.Tiles}
      {==}
    case None{}:
      %Equal.sym(Image, s, Seek.from(None{}, e, l, base), h) : {T.Plan.fill(T.Tile.plan.found(None{}, ops, l, base), e, off, T.Jobs.run(t, e)) == T.TLeaf{T.Tile{ops, l, base, _}} : T.Tiles}
      Equal.cong(Image, T.Tiles, i => T.TLeaf{T.Tile{ops, l, base, i}},
        T.Drawn.get(T.Jobs.run(t, e), T.Jobs.path(24n, off), e, l, base), T.Tile.draw(e, l, base),
        Laws.drawn_get_run(t, T.Jobs.path(24n, off), e, l, base))

law plan_fresh_k:
  for look: Bool
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +l: List<&2, R.Op>
  for +base: Image
  for +pk: Nat
  for +pt: T.Tiles
  for +pn: U32
  for +dy: U32
  for +rx: U32
  for +ry: U32
  for +off: Nat
  for +t: T.Jobs
  {T.Plan.fill(T.Tile.plan.fresh.k(look, e, ops, l, base, T.Past{pk, pt, pn, dy}, rx, ry), e, off, T.Jobs.run(t, e)) == T.TLeaf{T.Tile.fresh.k(look, e, ops, l, base, T.Past{pk, pt, pn, dy}, rx, ry)} : T.Tiles}

def plan_fresh_k(look, e, ops, l, base, pk, pt, pn, dy, rx, ry, off, t):
  match look:
    case True{}:
      plan_found(T.Tiles.find(pk, pt, rx, (ry - dy : U32), 0, 0, pn, l, base), e, ops, l, base, off, t,
        T.Tiles.seek(pk, pt, rx, (ry - dy : U32), 0, 0, pn, e, l, base), Laws_find(pk, pt, rx, (ry - dy : U32), 0, 0, pn, e, l, base))
    case False{}:
      Equal.cong(Image, T.Tiles, i => T.TLeaf{T.Tile{ops, l, base, i}},
        T.Drawn.get(T.Jobs.run(t, e), T.Jobs.path(24n, off), e, l, base), T.Tile.draw(e, l, base),
        Laws.drawn_get_run(t, T.Jobs.path(24n, off), e, l, base))

law tile_plan_k:
  for s: Bool
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +base: Image
  for +o: List<&2, R.Op>
  for +l: List<&2, R.Op>
  for +b: Image
  for +i: Image
  for +p: T.Past
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +off: Nat
  for +t: T.Jobs
  {T.Plan.fill(T.Tile.plan.k(s, e, ops, base, T.Tile{o, l, b, i}, p, rx, ry, n), e, off, T.Jobs.run(t, e)) == T.TLeaf{T.Tile.again.k(s, e, ops, base, T.Tile{o, l, b, i}, p, rx, ry, n)} : T.Tiles}

def tile_plan_k(s, e, ops, base, o, l, b, i, p, rx, ry, n, off, t):
  match s p:
    case True{} _:
      {==}
    case False{} T.Past{pk, pt, pn, dy}:
      plan_fresh_k(U32.is_ne(dy, 0), e, ops, T.Loc.ops(ops, rx, ry, T.Tile.size(e)), base, pk, pt, pn, dy, rx, ry, off, t)

# a node of any last tiles: both keep it whole, or both go down a level
law plan_node_k:
  for s: Bool
  for +j: Nat
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +base: Image
  for +o: List<&2, R.Op>
  for +bs: Image
  for +pa: T.Tiles
  for +pb: T.Tiles
  for +pc: T.Tiles
  for +pd: T.Tiles
  for +p: T.Past
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +off: Nat
  for +t: T.Jobs
  for ha: {T.Plan.fill(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n)), e, off, T.Jobs.run(t, e)) == T.Tiles.again(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n)) : T.Tiles}
  for hb: {T.Plan.fill(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), pb, p, (rx + U32.shr(n) : U32), ry, U32.shr(n)), e, Nat.add(off, T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n)))), T.Jobs.run(t, e)) == T.Tiles.again(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), pb, p, (rx + U32.shr(n) : U32), ry, U32.shr(n)) : T.Tiles}
  for hc: {T.Plan.fill(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), pc, p, rx, (ry + U32.shr(n) : U32), U32.shr(n)), e, Nat.add(Nat.add(off, T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n)))), T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), pb, p, (rx + U32.shr(n) : U32), ry, U32.shr(n)))), T.Jobs.run(t, e)) == T.Tiles.again(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), pc, p, rx, (ry + U32.shr(n) : U32), U32.shr(n)) : T.Tiles}
  for hd: {T.Plan.fill(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), pd, p, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), e, Nat.add(Nat.add(Nat.add(off, T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n)))), T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), pb, p, (rx + U32.shr(n) : U32), ry, U32.shr(n)))), T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), pc, p, rx, (ry + U32.shr(n) : U32), U32.shr(n)))), T.Jobs.run(t, e)) == T.Tiles.again(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), pd, p, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)) : T.Tiles}
  {T.Plan.fill(T.Tiles.plan.go(1n+j, e, ops, base, T.TNode{o, bs, pa, pb, pc, pd}, p, rx, ry, n, s), e, off, T.Jobs.run(t, e)) == T.Tiles.again.go(1n+j, e, ops, base, T.TNode{o, bs, pa, pb, pc, pd}, p, rx, ry, n, s) : T.Tiles}

def plan_node_k(s, j, e, ops, base, o, bs, pa, pb, pc, pd, p, rx, ry, n, off, t, ha, hb, hc, hd):
  match s:
    case True{}:
      {==}
    case False{}:
      tnode_cong(ops, base, T.Plan.fill(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n)), e, off, T.Jobs.run(t, e)), T.Tiles.again(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n)), T.Plan.fill(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), pb, p, (rx + U32.shr(n) : U32), ry, U32.shr(n)), e, Nat.add(off, T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n)))), T.Jobs.run(t, e)), T.Tiles.again(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), pb, p, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Plan.fill(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), pc, p, rx, (ry + U32.shr(n) : U32), U32.shr(n)), e, Nat.add(Nat.add(off, T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n)))), T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), pb, p, (rx + U32.shr(n) : U32), ry, U32.shr(n)))), T.Jobs.run(t, e)), T.Tiles.again(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), pc, p, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Plan.fill(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), pd, p, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), e, Nat.add(Nat.add(Nat.add(off, T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n)))), T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), pb, p, (rx + U32.shr(n) : U32), ry, U32.shr(n)))), T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), pc, p, rx, (ry + U32.shr(n) : U32), U32.shr(n)))), T.Jobs.run(t, e)), T.Tiles.again(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), pd, p, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), ha, hb, hc, hd)

def Laws.plan_fill_again(k, e, ops, base, prev, p, rx, ry, n, off, t):
  match k prev:
    case 0n T.TLeaf{T.Tile{+o, +l, +b, +i}}:
      tile_plan_k(T.Eq.tile(ops, o, base, b), e, ops, base, o, l, b, i, p, rx, ry, n, off, t)
    case 1n+j0 T.TNode{+o, +bs, +pa, +pb, +pc, +pd}:
      +j = j0
      plan_node_k(T.Eq.tile(ops, o, base, bs), j, e, ops, base, o, bs, pa, pb, pc, pd, p, rx, ry, n, off, t,
        Laws.plan_fill_again(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n), off, t),
        Laws.plan_fill_again(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), pb, p, (rx + U32.shr(n) : U32), ry, U32.shr(n), Nat.add(off, T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n)))), t),
        Laws.plan_fill_again(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), pc, p, rx, (ry + U32.shr(n) : U32), U32.shr(n), Nat.add(Nat.add(off, T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n)))), T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), pb, p, (rx + U32.shr(n) : U32), ry, U32.shr(n)))), t),
        Laws.plan_fill_again(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), pd, p, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n), Nat.add(Nat.add(Nat.add(off, T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n)))), T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), pb, p, (rx + U32.shr(n) : U32), ry, U32.shr(n)))), T.Plan.count(T.Tiles.plan(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), pc, p, rx, (ry + U32.shr(n) : U32), U32.shr(n)))), t))
    case 0n T.TNode{_, _, _, _, _, _}:
      plan_make_fill(0n, e, ops, base, rx, ry, n, off, t)
    case 1n+j T.TLeaf{_}:
      plan_make_fill(1n+j, e, ops, base, rx, ry, n, off, t)

law tl_one_k:
  for s: Bool
  for +ty: L.Ty
  for +e: M.Entry
  for +o: M.Entry
  for +px: U32
  for +sel: String
  for +h: {Q.Eq.entry(e, o) == s : Bool}
  {L.TL.memo.one.k(s, ty, e, px, sel, L.TL.memo.item(ty, o, px, sel)) == L.TL.memo.item(ty, e, px, sel) : L.TLE}

def tl_one_k(s, ty, e, o, px, sel, h):
  match s:
    case False{}:
      {==}
    case True{}:
      eo = eq_entry_sound(e, o, h)
      %eo : {L.TL.memo.item(ty, _, px, sel) == L.TL.memo.item(ty, e, px, sel) : L.TLE}
      {==}

def Laws.tl_again_exact(es, ty, old, px, sel):
  match es:
    case Nil{}:
      {==}
    case +e <> +t:
      match old:
        case Nil{}:
          {==}
        case +o <> +ot:
          e1 = Equal.sym(L.TLE, L.TL.memo.one.k(Q.Eq.entry(e, o), ty, e, px, sel, L.TL.memo.item(ty, o, px, sel)), L.TL.memo.item(ty, e, px, sel), tl_one_k(Q.Eq.entry(e, o), ty, e, o, px, sel, {==}))
          e2 = Equal.sym(List<&2, L.TLE>, L.TL.memo.again(t, ty, px, sel, L.TL.memo.make(ty, ot, px, sel)), L.TL.memo.make(ty, t, px, sel), Laws.tl_again_exact(t, ty, ot, px, sel))
          %e1 : {_ <> L.TL.memo.again(t, ty, px, sel, L.TL.memo.make(ty, ot, px, sel)) == L.TL.memo.item(ty, e, px, sel) <> L.TL.memo.make(ty, t, px, sel) : List<&2, L.TLE>}
          %e2 : {L.TL.memo.item(ty, e, px, sel) <> _ == L.TL.memo.item(ty, e, px, sel) <> L.TL.memo.make(ty, t, px, sel) : List<&2, L.TLE>}
          {==}

def Laws.tl_keep(ty, e, px, sel, p):
  %Equal.sym(Bool, Q.Eq.entry(e, e), True{}, eq_entry_refl(e)) : {L.TL.memo.one.k(_, ty, e, px, sel, L.TLE{e, p}) == L.TLE{e, p} : L.TLE}
  {==}

law tl_of_k:
  for s: Bool
  for +ty: L.Ty
  for +es: List<&2, M.Entry>
  for +old: List<&2, M.Entry>
  for +px0: U32
  for +sel0: String
  for +px: U32
  for +sel: String
  for +h: {Bool.and(U32.is_eq(px0, px), Q.Eq.str(sel0, sel)) == s : Bool}
  {L.TL.memo.of.k(s, ty, es, px, sel, L.TL.memo.make(ty, old, px0, sel0)) == L.TLMemo{px, sel, L.TL.memo.make(ty, es, px, sel), L.TL.live.none()} : L.TLMemo}

law tl_skip_make:
  for d: Nat
  for +ty: L.Ty
  for old: List<&2, M.Entry>
  for +px: U32
  for +sel: String
  {L.TL.skip(d, L.TL.memo.make(ty, old, px, sel)) == L.TL.memo.make(ty, L.TL.eskip(d, old), px, sel) : List<&2, L.TLE>}

def tl_skip_make(d, ty, old, px, sel):
  match d:
    case 0n:
      {==}
    case 1n+k:
      match old:
        case Nil{}:
          {==}
        case +e <> +t:
          tl_skip_make(k, ty, t, px, sel)

law tl_cat_make:
  for +ty: L.Ty
  for a: List<&2, M.Entry>
  for +b: List<&2, M.Entry>
  for +px: U32
  for +sel: String
  {L.TL.cat(L.TL.memo.make(ty, a, px, sel), L.TL.memo.make(ty, b, px, sel)) == L.TL.memo.make(ty, L.TL.ecat(a, b), px, sel) : List<&2, L.TLE>}

def tl_cat_make(ty, a, b, px, sel):
  match a:
    case Nil{}:
      {==}
    case +e <> +t:
      Equal.cong(List<&2, L.TLE>, List<&2, L.TLE>, x => L.TL.memo.item(ty, e, px, sel) <> x, L.TL.cat(L.TL.memo.make(ty, t, px, sel), L.TL.memo.make(ty, b, px, sel)), L.TL.memo.make(ty, L.TL.ecat(t, b), px, sel), tl_cat_make(ty, t, b, px, sel))

def Laws.tl_align(c, es, ty, old, px, sel):
  match c:
    case L.TLSame{}:
      {==}
    case L.TLDrop{+d}:
      tl_skip_make(d, ty, old, px, sel)
    case L.TLPad{+k}:
      tl_cat_make(ty, L.TL.etake(k, es), old, px, sel)

# the aligned memo brought up to date is the memo made from scratch
law tl_again_aligned:
  for +es: List<&2, M.Entry>
  for +ty: L.Ty
  for +old: List<&2, M.Entry>
  for +px: U32
  for +sel: String
  {L.TL.memo.again(es, ty, px, sel, L.TL.memo.aligned(es, ty, px, sel, L.TL.memo.make(ty, old, px, sel))) == L.TL.memo.make(ty, es, px, sel) : List<&2, L.TLE>}

def tl_again_aligned(es, ty, old, px, sel):
  +c = L.TL.align.of(es, L.TL.memo.make(ty, old, px, sel))
  +ea = Equal.sym(List<&2, L.TLE>, L.TL.memo.apply(c, es, L.TL.memo.make(ty, old, px, sel), ty, px, sel), L.TL.memo.make(ty, L.TL.memo.apply.es(c, es, old), px, sel), Laws.tl_align(c, es, ty, old, px, sel))
  %ea : {L.TL.memo.again(es, ty, px, sel, _) == L.TL.memo.make(ty, es, px, sel) : List<&2, L.TLE>}
  Laws.tl_again_exact(es, ty, L.TL.memo.apply.es(c, es, old), px, sel)

def tl_of_k(s, ty, es, old, px0, sel0, px, sel, h):
  match s:
    case False{}:
      {==}
    case True{}:
      ep = u32_eq_sound(px0, px, and_l(U32.is_eq(px0, px), Q.Eq.str(sel0, sel), h))
      el = eq_str_sound(sel0, sel, and_r(U32.is_eq(px0, px), Q.Eq.str(sel0, sel), h))
      %Equal.sym(U32, px0, px, ep) : {L.TLMemo{px, sel, L.TL.memo.again(es, ty, px, sel, L.TL.memo.aligned(es, ty, px, sel, L.TL.memo.make(ty, old, _, sel0))), L.TL.live.none()} == L.TLMemo{px, sel, L.TL.memo.make(ty, es, px, sel), L.TL.live.none()} : L.TLMemo}
      %Equal.sym(String, sel0, sel, el) : {L.TLMemo{px, sel, L.TL.memo.again(es, ty, px, sel, L.TL.memo.aligned(es, ty, px, sel, L.TL.memo.make(ty, old, px, _))), L.TL.live.none()} == L.TLMemo{px, sel, L.TL.memo.make(ty, es, px, sel), L.TL.live.none()} : L.TLMemo}
      Equal.cong(List<&2, L.TLE>, L.TLMemo, x => L.TLMemo{px, sel, x, L.TL.live.none()}, L.TL.memo.again(es, ty, px, sel, L.TL.memo.aligned(es, ty, px, sel, L.TL.memo.make(ty, old, px, sel))), L.TL.memo.make(ty, es, px, sel), tl_again_aligned(es, ty, old, px, sel))

def Laws.tl_memo_exact(ty, es, old, px0, sel0, px, sel):
  tl_of_k(Bool.and(U32.is_eq(px0, px), Q.Eq.str(sel0, sel)), ty, es, old, px0, sel0, px, sel, {==})

law tl_entries_memo:
  for +ty: L.Ty
  for +es: List<&2, M.Entry>
  for +st: M.State
  for +sel: String
  for +px: U32
  for +pics: Map<&2, String>
  {L.TL.entries.m(ty, L.TL.memo.make(ty, es, px, sel), st, sel, px, pics) == L.TL.entries(ty, es, st, sel, px, pics) : List<&2, L.TLine>}

# the memo's items were made for exactly the entries
law tl_memo_es:
  for +ty: L.Ty
  for es: List<&2, M.Entry>
  for +px: U32
  for +sel: String
  {L.TL.memo.es(L.TL.memo.make(ty, es, px, sel)) == es : List<&2, M.Entry>}

def tl_memo_es(ty, es, px, sel):
  match es:
    case Nil{}:
      {==}
    case +e <> +t:
      Equal.cong(List<&2, M.Entry>, List<&2, M.Entry>, x => e <> x, L.TL.memo.es(L.TL.memo.make(ty, t, px, sel)), t, tl_memo_es(ty, t, px, sel))

# under the same roles, the memo's items show as the entries do
law tl_entries_roles:
  for +ty: L.Ty
  for es: List<&2, M.Entry>
  for rs: List<&2, FO.Role>
  for +st: M.State
  for +sel: String
  for +px: U32
  for +pics: Map<&2, String>
  for +sp: STP.Stamps
  {L.TL.entries.mr(ty, L.TL.memo.make(ty, es, px, sel), rs, st, sel, px, pics, sp) == L.TL.entries.r(ty, es, rs, st, sel, px, pics, sp) : List<&2, L.TLine>}

def tl_entries_roles(ty, es, rs, st, sel, px, pics, sp):
  match es:
    case Nil{}:
      {==}
    case +e <> +t:
      match rs:
        case Nil{}:
          Equal.cong(List<&2, L.TLine>, List<&2, L.TLine>, x => List.append(&2, L.TLine, L.TL.entry.bound(e, L.TL.entry(ty, e, st, sel, px, pics, sp)), x),
            L.TL.entries.mr(ty, L.TL.memo.make(ty, t, px, sel), Nil{}, st, sel, px, pics, sp), L.TL.entries.r(ty, t, Nil{}, st, sel, px, pics, sp), tl_entries_roles(ty, t, Nil{}, st, sel, px, pics, sp))
        case +r <> +rt:
          Equal.cong(List<&2, L.TLine>, List<&2, L.TLine>, x => List.append(&2, L.TLine, L.TL.entry.bound(e, L.TL.mark(String.eq(M.Idx.id(e), U.Str.get(pics, "hist.jump")), L.TL.role(r, ty, L.TL.entry(ty, e, st, sel, px, pics, sp), px))), x),
            L.TL.entries.mr(ty, L.TL.memo.make(ty, t, px, sel), rt, st, sel, px, pics, sp), L.TL.entries.r(ty, t, rt, st, sel, px, pics, sp), tl_entries_roles(ty, t, rt, st, sel, px, pics, sp))

def tl_entries_memo(ty, es, st, sel, px, pics):
  %Equal.sym(List<&2, M.Entry>, L.TL.memo.es(L.TL.memo.make(ty, es, px, sel)), es, tl_memo_es(ty, es, px, sel))
    : {L.TL.entries.mr(ty, L.TL.memo.make(ty, es, px, sel), L.TL.roles(_, st, sel, pics), st, sel, px, pics, L.TL.stamps(_, st, sel, pics)) == L.TL.entries(ty, es, st, sel, px, pics) : List<&2, L.TLine>}
  tl_entries_roles(ty, es, L.TL.roles(es, st, sel, pics), st, sel, px, pics, L.TL.stamps(es, st, sel, pics))

law tl_lines_k:
  for s: Bool
  for +ty: L.Ty
  for +es: List<&2, M.Entry>
  for +old: List<&2, M.Entry>
  for +px0: U32
  for +sel0: String
  for +st: M.State
  for +sel: String
  for +px: U32
  for +pics: Map<&2, String>
  for +h: {Bool.and(U32.is_eq(px0, px), Q.Eq.str(sel0, sel)) == s : Bool}
  {L.TL.memo.lines.k(s, ty, L.TLMemo{px0, sel0, L.TL.memo.make(ty, old, px0, sel0), L.TL.live.none()}, es, st, sel, px, pics) == L.TL.entries(ty, es, st, sel, px, pics) : List<&2, L.TLine>}

def tl_lines_k(s, ty, es, old, px0, sel0, st, sel, px, pics, h):
  match s:
    case False{}:
      {==}
    case True{}:
      ep = u32_eq_sound(px0, px, and_l(U32.is_eq(px0, px), Q.Eq.str(sel0, sel), h))
      el = eq_str_sound(sel0, sel, and_r(U32.is_eq(px0, px), Q.Eq.str(sel0, sel), h))
      %Equal.sym(U32, px0, px, ep) : {L.TL.entries.m(ty, L.TL.memo.again(es, ty, px, sel, L.TL.memo.make(ty, old, _, sel0)), st, sel, px, pics) == L.TL.entries(ty, es, st, sel, px, pics) : List<&2, L.TLine>}
      %Equal.sym(String, sel0, sel, el) : {L.TL.entries.m(ty, L.TL.memo.again(es, ty, px, sel, L.TL.memo.make(ty, old, px, _)), st, sel, px, pics) == L.TL.entries(ty, es, st, sel, px, pics) : List<&2, L.TLine>}
      %Equal.sym(List<&2, L.TLE>, L.TL.memo.again(es, ty, px, sel, L.TL.memo.make(ty, old, px, sel)), L.TL.memo.make(ty, es, px, sel), Laws.tl_again_exact(es, ty, old, px, sel)) : {L.TL.entries.m(ty, _, st, sel, px, pics) == L.TL.entries(ty, es, st, sel, px, pics) : List<&2, L.TLine>}
      tl_entries_memo(ty, es, st, sel, px, pics)

def Laws.tl_lines_exact(ty, es, old, px0, sel0, st, sel, px, pics):
  tl_lines_k(Bool.and(U32.is_eq(px0, px), Q.Eq.str(sel0, sel)), ty, es, old, px0, sel0, st, sel, px, pics, {==})
def Laws.link_no_script(s):
  {==}

def Laws.link_no_file(s):
  {==}

def Laws.link_path_not_web(s):
  {==}

def Laws.link_file_abs_kept(cwd, s):
  {==}

def Laws.menu_file_open_first(p):
  {==}

def Laws.link_plain_line(s):
  {==}

# Image lightbox
# --------------

# a is at most a
law le_refl:
  for a: Nat
  {Nat.is_le(a, a) == True{} : Bool}

def le_refl(a):
  match a:
    case 0n:
      {==}
    case 1n+p:
      le_refl(p)

# not a >= b means b >= a
law ge_flip:
  for a: Nat
  for b: Nat
  for h: {Nat.is_ge(a, b) == False{} : Bool}
  {Nat.is_ge(b, a) == True{} : Bool}

def ge_flip(a, b, h):
  match a b:
    case 0n 0n:
      Empty.absurd({Nat.is_ge(0n, 0n) == True{} : Bool}, false_ne_true(Equal.sym(Bool, True{}, False{}, h)))
    case 0n 1n+q:
      {==}
    case 1n+p 0n:
      Empty.absurd({Nat.is_ge(0n, 1n+p) == True{} : Bool}, false_ne_true(Equal.sym(Bool, True{}, False{}, h)))
    case 1n+p 1n+q:
      ge_flip(p, q, h)

# either of two numbers at most hi is at most hi
law pick_le:
  for c: Bool
  for a: Nat
  for b: Nat
  for -hi: Nat
  for ha: {Nat.is_le(a, hi) == True{} : Bool}
  for hb: {Nat.is_le(b, hi) == True{} : Bool}
  {Nat.is_le(Bool.pick(Nat, c, a, b), hi) == True{} : Bool}

def pick_le(c, a, b, hi, ha, hb):
  match c:
    case True{}:
      ha
    case False{}:
      hb

law min_le_k:
  for c: Bool
  for +a: Nat
  for +b: Nat
  for h: {Nat.is_le(a, b) == c : Bool}
  {Nat.is_le(Bool.pick(Nat, c, a, b), b) == True{} : Bool}

def min_le_k(c, a, b, h):
  match c:
    case True{}:
      h
    case False{}:
      le_refl(b)

# the lesser of a and b is at most b
law lb_min_le:
  for +a: Nat
  for +b: Nat
  {Nat.is_le(LBX.Lb.min(a, b), b) == True{} : Bool}

def lb_min_le(a, b):
  min_le_k(Nat.is_le(a, b), a, b, {==})

law max_ge_k:
  for c: Bool
  for +a: Nat
  for +b: Nat
  for h: {Nat.is_ge(a, b) == c : Bool}
  {Nat.is_ge(Bool.pick(Nat, c, a, b), a) == True{} : Bool}

def max_ge_k(c, a, b, h):
  match c:
    case True{}:
      Nat.ge_refl(a)
    case False{}:
      ge_flip(a, b, h)

# the greater of a and b is at least a
law lb_max_ge:
  for +a: Nat
  for +b: Nat
  {Nat.is_ge(LBX.Lb.max(a, b), a) == True{} : Bool}

def lb_max_ge(a, b):
  max_ge_k(Nat.is_ge(a, b), a, b, {==})

def Laws.lightbox_zoom_min(z):
  lb_max_ge(LBX.Lb.zmin(), LBX.Lb.min(z, LBX.Lb.zmax()))

def Laws.lightbox_zoom_max(z):
  pick_le(Nat.is_ge(LBX.Lb.zmin(), LBX.Lb.min(z, LBX.Lb.zmax())), LBX.Lb.zmin(), LBX.Lb.min(z, LBX.Lb.zmax()), LBX.Lb.zmax(), {==}, lb_min_le(z, LBX.Lb.zmax()))

def Laws.lightbox_scroll_max(a, m, d, v):
  lb_min_le(Nat.sub(Nat.add(Nat.div(Nat.mul(Nat.mul(a, d), LBX.Lb.sub()), LBX.Lb.one()), Nat.mul(Nat.add(LBX.Lb.pad(d, v), LBX.Lb.bias()), LBX.Lb.sub())),
    Nat.mul(Nat.add(m, Nat.div(v, 2n)), LBX.Lb.sub())), Nat.mul(Nat.sub(d, v), LBX.Lb.sub()))

def Laws.lightbox_ease_start(z0, z1, t0):
  %Equal.sym(Bool, Nat.is_ge(t0, Nat.add(t0, LBX.Lb.ms())), False{}, ge_past(t0, 179n))
    : {LBX.Lb.ease(_, Nat.sub(t0, t0), z0, z1) == z0 : Nat}
  %Equal.sym(Nat, Nat.sub(t0, t0), 0n, sub_self(t0)) : {LBX.Lb.ease(False{}, _, z0, z1) == z0 : Nat}
  {==}

def Laws.lightbox_ease_done(z0, z1, t0, k):
  %Equal.sym(Bool, Nat.is_ge(Nat.add(Nat.add(t0, LBX.Lb.ms()), k), Nat.add(t0, LBX.Lb.ms())), True{},
    ge_add(Nat.add(t0, LBX.Lb.ms()), Nat.add(t0, LBX.Lb.ms()), k, Nat.ge_refl(Nat.add(t0, LBX.Lb.ms()))))
    : {LBX.Lb.ease(_, Nat.sub(Nat.add(Nat.add(t0, LBX.Lb.ms()), k), t0), z0, z1) == z1 : Nat}
  {==}

def Laws.lightbox_idle(t0, k):
  %Equal.sym(Bool, Nat.is_ge(Nat.add(Nat.add(t0, LBX.Lb.ms()), k), Nat.add(t0, LBX.Lb.ms())), True{},
    ge_add(Nat.add(t0, LBX.Lb.ms()), Nat.add(t0, LBX.Lb.ms()), k, Nat.ge_refl(Nat.add(t0, LBX.Lb.ms()))))
    : {Bool.not(_) == False{} : Bool}
  {==}

def Laws.lightbox_opens_still(now):
  {==}

def Laws.svg_close_returns(p):
  match p:
    case SVG.Pen{x, y, sx, sy, cx, cy, ck}:
      {==}

def Laws.svg_move_starts(p, x, y):
  match p:
    case SVG.Pen{a, b, c, d, e, f, g}:
      {==}

def Laws.svg_absolute_ignores_pen(p, v):
  {==}

def Laws.svg_implicit_lineto():
  {==}

def Laws.svg_numbers_split():
  {==}

def Laws.icons_used_exist():
  {==}

def Laws.icons_all_draw():
  {==}

def Laws.tools_icons_exist():
  {==}

def Laws.tools_labelled():
  {==}

def Laws.tools_stop_first():
  {==}

def Laws.viewer_auto_opens():
  {==}

def Laws.viewer_auto_hand_shut():
  {==}

def Laws.auto_view_none():
  {==}

def Laws.hotkey_alt_digit():
  {==}

def Laws.hotkey_ctrl_digit():
  {==}

def Laws.hint_alt_starts(h, mods):
  {==}

def Laws.hint_missed_release_ends():
  {==}

def Laws.wrap_keeps_last_space():
  {==}

def Laws.edit_select_all():
  {==}

def Laws.edit_type_replaces():
  {==}

def Laws.delete_asks_first(u):
  {==}

def Laws.row_delete_asks_first(u, id):
  {==}

def Laws.row_toggle_unsettles(t, at, h):
  %Equal.sym(M.Shelf, M.Shelf.of(M.Thread.settled(True{}, at, t)), M.Settled{}, Laws.merge_settles(t, at, h)) : {U.Ui.row_toggle.m(_) == "thread.unsettle" : String}
  {==}

def Laws.row_toggle_settles(at, t):
  match t:
    case M.Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, True{}}:
      {==}
    case M.Thread{id, p, ti, c, a, tu, pe, True{}, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, False{}}:
      {==}
    case M.Thread{id, p, ti, c, a, tu, pe, False{}, o, sa, ua, True{}, en, br, wt, pr, mo, rm, im, se, False{}}:
      {==}
    case M.Thread{id, p, ti, c, a, tu, pe, False{}, o, sa, ua, False{}, en, br, wt, pr, mo, rm, im, se, False{}}:
      {==}

def Laws.picker_drops_stale(u, o):
  {==}

def Laws.tl_frame_exact(ty, es, old, px0, sel0, st, sel, px, pics):
  %Equal.sym(L.TLMemo, L.TL.memo.of(ty, es, px, sel, L.TLMemo{px0, sel0, L.TL.memo.make(ty, old, px0, sel0), L.TL.live.none()}), L.TLMemo{px, sel, L.TL.memo.make(ty, es, px, sel), L.TL.live.none()},
    Laws.tl_memo_exact(ty, es, old, px0, sel0, px, sel))
    : {L.TL.memo.shown(ty, _, st, sel, px, pics) == L.TL.entries(ty, es, st, sel, px, pics) : List<&2, L.TLine>}
  tl_entries_memo(ty, es, st, sel, px, pics)

# Tool calls folded
# -----------------

law fold_flat_push:
  for b: Bool
  for +e: M.Entry
  for gs: List<&2, FO.Grp>
  {FO.Fold.flat(FO.Fold.push.k(b, e, gs)) == e <> FO.Fold.flat(gs) : List<&2, M.Entry>}

def fold_flat_push(b, e, gs):
  match b:
    case False{}:
      {==}
    case True{}:
      match gs:
        case Nil{}:
          {==}
        case +g <> +t:
          match g:
            case FO.GOne{+x}:
              {==}
            case FO.GRun{+xs}:
              {==}

def Laws.fold_keeps_entries(es):
  match es:
    case Nil{}:
      {==}
    case +e <> +t:
      Equal.trans(List<&2, M.Entry>, FO.Fold.flat(FO.Fold.push.k(FO.Fold.tool(e), e, FO.Fold.groups(t))), e <> FO.Fold.flat(FO.Fold.groups(t)), e <> t,
        fold_flat_push(FO.Fold.tool(e), e, FO.Fold.groups(t)),
        Equal.cong(List<&2, M.Entry>, List<&2, M.Entry>, x => e <> x, FO.Fold.flat(FO.Fold.groups(t)), t, Laws.fold_keeps_entries(t)))

def Laws.fold_count(es):
  %Equal.sym(List<&2, M.Entry>, FO.Fold.flat(FO.Fold.groups(es)), es, Laws.fold_keeps_entries(es))
    : {List.length(&2, M.Entry, _) == List.length(&2, M.Entry, es) : Nat}
  {==}

law fold_runs_push:
  for b: Bool
  for +e: M.Entry
  for gs: List<&2, FO.Grp>
  for h: {FO.Fold.tool(e) == b : Bool}
  for ih: {FO.Fold.all(FO.Fold.runs(gs)) == True{} : Bool}
  {FO.Fold.all(FO.Fold.runs(FO.Fold.push.k(b, e, gs))) == True{} : Bool}

def fold_runs_push(b, e, gs, h, ih):
  match b:
    case False{}:
      ih
    case True{}:
      match gs:
        case Nil{}:
          %Equal.sym(Bool, FO.Fold.tool(e), True{}, h) : {FO.Fold.all.and(_, True{}) == True{} : Bool}
          {==}
        case +g <> +t:
          match g:
            case FO.GOne{+x}:
              %Equal.sym(Bool, FO.Fold.tool(e), True{}, h) : {FO.Fold.all.and(_, FO.Fold.all(FO.Fold.runs(t))) == True{} : Bool}
              ih
            case FO.GRun{+xs}:
              %Equal.sym(Bool, FO.Fold.tool(e), True{}, h) : {FO.Fold.all.and(_, FO.Fold.all(List.append(&2, M.Entry, xs, FO.Fold.runs(t)))) == True{} : Bool}
              ih

def Laws.fold_runs_only_tools(es):
  match es:
    case Nil{}:
      {==}
    case +e <> +t:
      fold_runs_push(FO.Fold.tool(e), e, FO.Fold.groups(t), {==}, Laws.fold_runs_only_tools(t))

def Laws.fold_never_error(i, t, k, s, a):
  {==}

def Laws.fold_never_message(i, t, r, x, a):
  {==}

def Laws.fold_user_opened(live):
  {==}

def Laws.fold_user_shut(live):
  {==}

def Laws.fold_default_live(live):
  {==}

def Laws.hub_frames_cbor(t):
  {==}

def Laws.hub_plot_frames_binary(s):
  {==}

# The phones' board viewer
# ------------------------

law eq_chunk_k_sound:
  for s: Bool
  for a: PL.Chunk
  for b: PL.Chunk
  for hs: {PL.Eq.sig(a, b) == s : Bool}
  for +h: {PL.Eq.chunk.k(s, a, b) == True{} : Bool}
  {a == b : PL.Chunk}

def eq_chunk_k_sound(s, a, b, hs, h):
  match s:
    case False{}:
      Empty.absurd({a == b : PL.Chunk}, false_ne_true(h))
    case True{}:
      match a b:
        case PL.Chunk{+s1, +l1, +c1, +a1, +m1, +w1, +d1, +i1} PL.Chunk{+s2, +l2, +c2, +a2, +m2, +w2, +d2, +i2}:
          +r6 = Bool.and(Q.Eq.str(d1, d2), Q.Eq.str(i1, i2))
          +r5 = Bool.and(U32.is_eq(w1, w2), r6)
          +r4 = Bool.and(U32.is_eq(m1, m2), r5)
          +r3 = Bool.and(U32.is_eq(a1, a2), r4)
          +r2 = Bool.and(U32.is_eq(c1, c2), r3)
          +h1 = and_r(U32.is_eq(l1, l2), r2, h)
          +h2 = and_r(U32.is_eq(c1, c2), r3, h1)
          +h3 = and_r(U32.is_eq(a1, a2), r4, h2)
          +h4 = and_r(U32.is_eq(m1, m2), r5, h3)
          +h5 = and_r(U32.is_eq(w1, w2), r6, h4)
          e0 = u32_eq_sound(s1, s2, hs)
          el = u32_eq_sound(l1, l2, and_l(U32.is_eq(l1, l2), r2, h))
          ec = u32_eq_sound(c1, c2, and_l(U32.is_eq(c1, c2), r3, h1))
          ea = u32_eq_sound(a1, a2, and_l(U32.is_eq(a1, a2), r4, h2))
          em = u32_eq_sound(m1, m2, and_l(U32.is_eq(m1, m2), r5, h3))
          ew = u32_eq_sound(w1, w2, and_l(U32.is_eq(w1, w2), r6, h4))
          ed = eq_str_sound(d1, d2, and_l(Q.Eq.str(d1, d2), Q.Eq.str(i1, i2), h5))
          ei = eq_str_sound(i1, i2, and_r(Q.Eq.str(d1, d2), Q.Eq.str(i1, i2), h5))
          %e0 : {PL.Chunk{s1, l1, c1, a1, m1, w1, d1, i1} == PL.Chunk{_, l2, c2, a2, m2, w2, d2, i2} : PL.Chunk}
          %el : {PL.Chunk{s1, l1, c1, a1, m1, w1, d1, i1} == PL.Chunk{s1, _, c2, a2, m2, w2, d2, i2} : PL.Chunk}
          %ec : {PL.Chunk{s1, l1, c1, a1, m1, w1, d1, i1} == PL.Chunk{s1, l1, _, a2, m2, w2, d2, i2} : PL.Chunk}
          %ea : {PL.Chunk{s1, l1, c1, a1, m1, w1, d1, i1} == PL.Chunk{s1, l1, c1, _, m2, w2, d2, i2} : PL.Chunk}
          %em : {PL.Chunk{s1, l1, c1, a1, m1, w1, d1, i1} == PL.Chunk{s1, l1, c1, a1, _, w2, d2, i2} : PL.Chunk}
          %ew : {PL.Chunk{s1, l1, c1, a1, m1, w1, d1, i1} == PL.Chunk{s1, l1, c1, a1, m1, _, d2, i2} : PL.Chunk}
          %ed : {PL.Chunk{s1, l1, c1, a1, m1, w1, d1, i1} == PL.Chunk{s1, l1, c1, a1, m1, w1, _, i2} : PL.Chunk}
          %ei : {PL.Chunk{s1, l1, c1, a1, m1, w1, d1, i1} == PL.Chunk{s1, l1, c1, a1, m1, w1, d1, _} : PL.Chunk}
          {==}

# chunks compared equal are equal
law eq_chunk_sound:
  for a: PL.Chunk
  for b: PL.Chunk
  for h: {PL.Eq.chunk(a, b) == True{} : Bool}
  {a == b : PL.Chunk}

def eq_chunk_sound(a, b, h):
  match a b:
    case PL.Chunk{+s1, l1, c1, a1, m1, w1, d1, i1} PL.Chunk{+s2, l2, c2, a2, m2, w2, d2, i2}:
      eq_chunk_k_sound(U32.is_eq(s1, s2), PL.Chunk{s1, l1, c1, a1, m1, w1, d1, i1}, PL.Chunk{s2, l2, c2, a2, m2, w2, d2, i2}, {==}, h)

law eq_chunk_refl:
  for +x: PL.Chunk
  {PL.Eq.chunk(x, x) == True{} : Bool}

def eq_chunk_refl(x):
  match x:
    case PL.Chunk{+s, +l, +c, +a, +m, +w, +d, +i}:
      %Equal.sym(Bool, U32.is_eq(s, s), True{}, u32_eq_refl(s)) : {PL.Eq.chunk.k(_, PL.Chunk{s, l, c, a, m, w, d, i}, PL.Chunk{s, l, c, a, m, w, d, i}) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(l, l), True{}, u32_eq_refl(l)) : {Bool.and(_, Bool.and(U32.is_eq(c, c), Bool.and(U32.is_eq(a, a), Bool.and(U32.is_eq(m, m), Bool.and(U32.is_eq(w, w), Bool.and(Q.Eq.str(d, d), Q.Eq.str(i, i))))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(c, c), True{}, u32_eq_refl(c)) : {Bool.and(_, Bool.and(U32.is_eq(a, a), Bool.and(U32.is_eq(m, m), Bool.and(U32.is_eq(w, w), Bool.and(Q.Eq.str(d, d), Q.Eq.str(i, i)))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(a, a), True{}, u32_eq_refl(a)) : {Bool.and(_, Bool.and(U32.is_eq(m, m), Bool.and(U32.is_eq(w, w), Bool.and(Q.Eq.str(d, d), Q.Eq.str(i, i))))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(m, m), True{}, u32_eq_refl(m)) : {Bool.and(_, Bool.and(U32.is_eq(w, w), Bool.and(Q.Eq.str(d, d), Q.Eq.str(i, i)))) == True{} : Bool}
      %Equal.sym(Bool, U32.is_eq(w, w), True{}, u32_eq_refl(w)) : {Bool.and(_, Bool.and(Q.Eq.str(d, d), Q.Eq.str(i, i))) == True{} : Bool}
      %Equal.sym(Bool, Q.Eq.str(d, d), True{}, eq_str_refl(d)) : {Bool.and(_, Q.Eq.str(i, i)) == True{} : Bool}
      eq_str_refl(i)

law ref_next_get:
  for r: PL.Ref
  for +o: PL.Chunk
  for +ot: List<&2, PL.Chunk>
  {PL.Ref.get(PL.Ref.next(r), o <> ot) == PL.Ref.get(r, ot) : PL.Chunk}

def ref_next_get(r, o, ot):
  match r:
    case PL.Keep{i}:
      {==}
    case PL.Fresh{c}:
      {==}

law ref_next_fresh:
  for r: PL.Ref
  {PL.Ref.fresh(PL.Ref.next(r)) == PL.Ref.fresh(r) : Nat}

def ref_next_fresh(r):
  match r:
    case PL.Keep{i}:
      {==}
    case PL.Fresh{c}:
      {==}

# the chunk a phone looks up for c is c
law plot_find_get_k:
  for os: List<&2, PL.Chunk>
  for s: Bool
  for +c: PL.Chunk
  for h: {PL.Plot.hit(os, c) == s : Bool}
  {PL.Ref.get(PL.Plot.find(os, s, c), os) == c : PL.Chunk}

def plot_find_get_k(os, s, c, h):
  match os:
    case Nil{}:
      {==}
    case +o <> +ot:
      match s:
        case True{}:
          Equal.sym(PL.Chunk, c, o, eq_chunk_sound(c, o, h))
        case False{}:
          Equal.trans(PL.Chunk, PL.Ref.get(PL.Ref.next(PL.Plot.find(ot, PL.Plot.hit(ot, c), c)), o <> ot), PL.Ref.get(PL.Plot.find(ot, PL.Plot.hit(ot, c), c), ot), c,
            ref_next_get(PL.Plot.find(ot, PL.Plot.hit(ot, c), c), o, ot), plot_find_get_k(ot, PL.Plot.hit(ot, c), c, {==}))

def Laws.plot_delta_exact(ns, old):
  match ns:
    case Nil{}:
      {==}
    case +n <> +t:
      e1 = Equal.sym(PL.Chunk, PL.Ref.get(PL.Plot.find(old, PL.Plot.hit(old, n), n), old), n, plot_find_get_k(old, PL.Plot.hit(old, n), n, {==}))
      e2 = Equal.sym(List<&2, PL.Chunk>, PL.Plot.apply(PL.Plot.delta(t, old), old), t, Laws.plot_delta_exact(t, old))
      %e1 : {_ <> PL.Plot.apply(PL.Plot.delta(t, old), old) == n <> t : List<&2, PL.Chunk>}
      %e2 : {n <> _ == n <> t : List<&2, PL.Chunk>}
      {==}

law append_snoc:
  for pre: List<&2, PL.Chunk>
  for +x: PL.Chunk
  for +t: List<&2, PL.Chunk>
  {List.append(&2, PL.Chunk, pre, x <> t) == List.append(&2, PL.Chunk, List.append(&2, PL.Chunk, pre, [x]), t) : List<&2, PL.Chunk>}

def append_snoc(pre, x, t):
  match pre:
    case Nil{}:
      {==}
    case +p <> +pt:
      Equal.cong(List<&2, PL.Chunk>, List<&2, PL.Chunk>, l => p <> l, List.append(&2, PL.Chunk, pt, x <> t),
        List.append(&2, PL.Chunk, List.append(&2, PL.Chunk, pt, [x]), t), append_snoc(pt, x, t))

# a chunk the phone holds (somewhere after pre) is found, so kept
law plot_find_keeps_k:
  for pre: List<&2, PL.Chunk>
  for s: Bool
  for +x: PL.Chunk
  for +t: List<&2, PL.Chunk>
  for h: {PL.Plot.hit(List.append(&2, PL.Chunk, pre, x <> t), x) == s : Bool}
  {PL.Ref.fresh(PL.Plot.find(List.append(&2, PL.Chunk, pre, x <> t), s, x)) == 0n : Nat}

def plot_find_keeps_k(pre, s, x, t, h):
  match pre:
    case Nil{}:
      match s:
        case True{}:
          {==}
        case False{}:
          Empty.absurd({PL.Ref.fresh(PL.Ref.next(PL.Plot.find(t, PL.Plot.hit(t, x), x))) == 0n : Nat}, false_ne_true(Equal.trans(Bool, False{}, PL.Eq.chunk(x, x), True{}, Equal.sym(Bool, PL.Eq.chunk(x, x), False{}, h), eq_chunk_refl(x))))
    case +p <> +pt:
      match s:
        case True{}:
          {==}
        case False{}:
          Equal.trans(Nat, PL.Ref.fresh(PL.Ref.next(PL.Plot.find(List.append(&2, PL.Chunk, pt, x <> t), PL.Plot.hit(List.append(&2, PL.Chunk, pt, x <> t), x), x))),
            PL.Ref.fresh(PL.Plot.find(List.append(&2, PL.Chunk, pt, x <> t), PL.Plot.hit(List.append(&2, PL.Chunk, pt, x <> t), x), x)), 0n,
            ref_next_fresh(PL.Plot.find(List.append(&2, PL.Chunk, pt, x <> t), PL.Plot.hit(List.append(&2, PL.Chunk, pt, x <> t), x), x)),
            plot_find_keeps_k(pt, PL.Plot.hit(List.append(&2, PL.Chunk, pt, x <> t), x), x, t, {==}))

# every chunk of xs is kept by a phone holding pre ++ xs
law plot_kept_all:
  for xs: List<&2, PL.Chunk>
  for +pre: List<&2, PL.Chunk>
  {PL.Plot.fresh(PL.Plot.delta(xs, List.append(&2, PL.Chunk, pre, xs))) == 0n : Nat}

def plot_kept_all(xs, pre):
  match xs:
    case Nil{}:
      {==}
    case +x <> +t:
      ef = Equal.sym(Nat, PL.Ref.fresh(PL.Plot.find(List.append(&2, PL.Chunk, pre, x <> t), PL.Plot.hit(List.append(&2, PL.Chunk, pre, x <> t), x), x)), 0n,
        plot_find_keeps_k(pre, PL.Plot.hit(List.append(&2, PL.Chunk, pre, x <> t), x), x, t, {==}))
      ea = Equal.sym(List<&2, PL.Chunk>, List.append(&2, PL.Chunk, pre, x <> t), List.append(&2, PL.Chunk, List.append(&2, PL.Chunk, pre, [x]), t), append_snoc(pre, x, t))
      %ef : {Nat.add(_, PL.Plot.fresh(PL.Plot.delta(t, List.append(&2, PL.Chunk, pre, x <> t)))) == 0n : Nat}
      %ea : {PL.Plot.fresh(PL.Plot.delta(t, _)) == 0n : Nat}
      plot_kept_all(t, List.append(&2, PL.Chunk, pre, [x]))

def Laws.plot_unchanged_sends_nothing(cs):
  plot_kept_all(cs, Nil{})

def Laws.plot_hidden_not_sent(key, sig, layer, p, info, kind, acc, h):
  %Equal.sym(U32, PL.Piece.alpha(kind, layer), 0, h) : {PL.Piece.put.k(PL.Piece.shown(_), B.Item{key, sig, layer, p, info}, acc) == acc : List<&2, PL.Piece>}
  {==}

def Laws.plot_courtyard_hidden():
  {==}

def Laws.plot_numbers_exact():
  {==}

def Laws.view_closed_wants_nothing(root):
  match root:
    case SNil{}:
      {==}
    case SCon{h, t}:
      {==}

def Laws.view_asks_once(u, cmds, h):
  %Equal.sym(Bool, String.eq(MV.View.want(u), U.Ui.scratch(u, "@view.at")), True{}, h) : {MV.View.sync.k(_, u, MV.View.want(u), MV.View.kind(u), U.Ui.root(u), MV.View.path(u), cmds) == U.Act{u, cmds} : U.Act}
  {==}

def Laws.plot_frames_binary(bs):
  {==}

def Laws.pick_nothing_there(u):
  {==}

def Laws.pick_late_ignored(u, o, h):
  %Equal.sym(Bool, String.is_empty(U.Ui.scratch(u, "@pick.rid")), True{}, h) : {MV.View.picked.k(Bool.and(Bool.and(Bool.not(_), String.eq(J.Json.as_raw(J.Json.get(o, "id"), ""), U.Ui.scratch(u, "@pick.rid"))), String.eq(J.Json.get_str(o, "t", ""), "reply")), u, o) == u : U.Ui}
  {==}

def Laws.pick_pad_over_zone():
  {==}

def Laws.pick_pad_over_zone_after():
  {==}

def Laws.pick_top_on_tie():
  {==}

def Laws.view_3d_draws_board(root):
  {==}

def Laws.view_3d_key():
  {==}

def Laws.pcb3d_path_vars():
  {==}

def Laws.pcb3d_path_unknown_kept():
  {==}

def Laws.mesh_first_never_fades(now):
  {==}

def Laws.scene3_prompt_parts_no_fade():
  {==}

def Laws.scene3_late_parts_fade():
  {==}

def Laws.scene3_fade_ends():
  {==}

def Laws.scene3_stale_parts_dropped():
  {==}

def Laws.cam_fit_turns_about(b, px, py, pz, c):
  match b c:
    case MS.Box3{x0, y0, z0, x1, y1, z1} MS.Cam{rx, ry, rz, ux, uy, uz, qx, qy, qz, d, sx, sy, fov}:
      {==}

def Laws.scene3_board_pivot(x0, y0, x1, y1, thick, m, c):
  match m c:
    case MS.Model{n, MS.Box3{a0, b0, c0, a1, b1, c1}, cs, t, l} MS.Cam{rx, ry, rz, ux, uy, uz, qx, qy, qz, d, sx, sy, fov}:
      {==}
# The model picker
# ----------------

# a + (b + c) is at least a + b
law ge_add_mid:
  for a: Nat
  for +b: Nat
  for +c: Nat
  {Nat.is_ge(Nat.add(a, Nat.add(b, c)), Nat.add(a, b)) == True{} : Bool}

def ge_add_mid(a, b, c):
  match a:
    case 0n:
      ge_add(b, b, c, Nat.ge_refl(b))
    case 1n+p:
      ge_add_mid(p, b, c)

law and_both:
  for a: Bool
  for b: Bool
  for ha: {a == True{} : Bool}
  for hb: {b == True{} : Bool}
  {Bool.and(a, b) == True{} : Bool}

def and_both(a, b, ha, hb):
  %Equal.sym(Bool, a, True{}, ha) : {Bool.and(_, b) == True{} : Bool}
  hb

# 1 + y is at least y
law ge_succ_self:
  for y: Nat
  {Nat.is_ge(1n+y, y) == True{} : Bool}

def ge_succ_self(y):
  match y:
    case 0n:
      {==}
    case 1n+p:
      ge_succ_self(p)

law ge_trans:
  for a: Nat
  for b: Nat
  for c: Nat
  for h1: {Nat.is_ge(a, b) == True{} : Bool}
  for h2: {Nat.is_ge(b, c) == True{} : Bool}
  {Nat.is_ge(a, c) == True{} : Bool}

def ge_trans(a, b, c, h1, h2):
  match a b c:
    case 0n 0n 0n:
      {==}
    case 0n 0n 1n+r:
      Empty.absurd({Nat.is_ge(0n, 1n+r) == True{} : Bool}, false_ne_true(h2))
    case 0n 1n+q 0n:
      {==}
    case 0n 1n+q 1n+r:
      Empty.absurd({Nat.is_ge(0n, 1n+r) == True{} : Bool}, false_ne_true(h1))
    case 1n+p 0n 0n:
      {==}
    case 1n+p 0n 1n+r:
      Empty.absurd({Nat.is_ge(1n+p, 1n+r) == True{} : Bool}, false_ne_true(h2))
    case 1n+p 1n+q 0n:
      {==}
    case 1n+p 1n+q 1n+r:
      ge_trans(p, q, r, h1, h2)

# p + x is at least x
law ge_add_l:
  for +p: Nat
  for +x: Nat
  {Nat.is_ge(Nat.add(p, x), x) == True{} : Bool}

def ge_add_l(p, x):
  match p:
    case 0n:
      Nat.ge_refl(x)
    case 1n+q:
      ge_trans(1n+Nat.add(q, x), Nat.add(q, x), x, ge_succ_self(Nat.add(q, x)), ge_add_l(q, x))

# a >= b gives a + (h + g) >= b + h
law ge_add_both:
  for +a: Nat
  for +b: Nat
  for +h: Nat
  for +g: Nat
  for hab: {Nat.is_ge(a, b) == True{} : Bool}
  {Nat.is_ge(Nat.add(a, Nat.add(h, g)), Nat.add(b, h)) == True{} : Bool}

def ge_add_both(a, b, h, g, hab):
  match a b:
    case 0n 0n:
      ge_add(h, h, g, Nat.ge_refl(h))
    case 0n 1n+q:
      Empty.absurd({Nat.is_ge(Nat.add(0n, Nat.add(h, g)), Nat.add(1n+q, h)) == True{} : Bool}, false_ne_true(hab))
    case 1n+p 0n:
      ge_trans(Nat.add(1n+p, Nat.add(h, g)), Nat.add(h, g), h, ge_add_l(1n+p, Nat.add(h, g)), ge_add(h, h, g, Nat.ge_refl(h)))
    case 1n+p 1n+q:
      ge_add_both(p, q, h, g, hab)

# a >= r and a - r >= s give a >= r + s
law sub_add_ge:
  for a: Nat
  for r: Nat
  for +s: Nat
  for h1: {Nat.is_ge(a, r) == True{} : Bool}
  for h2: {Nat.is_ge(Nat.sub(a, r), s) == True{} : Bool}
  {Nat.is_ge(a, Nat.add(r, s)) == True{} : Bool}

def sub_add_ge(a, r, s, h1, h2):
  match a r:
    case 0n 0n:
      h2
    case 0n 1n+q:
      Empty.absurd({Nat.is_ge(0n, Nat.add(1n+q, s)) == True{} : Bool}, false_ne_true(h1))
    case 1n+p 0n:
      h2
    case 1n+p 1n+q:
      sub_add_ge(p, q, s, h1, h2)

# a <= b gives b >= a
law le_ge:
  for a: Nat
  for b: Nat
  for h: {Nat.is_le(a, b) == True{} : Bool}
  {Nat.is_ge(b, a) == True{} : Bool}

def le_ge(a, b, h):
  match a b:
    case 0n 0n:
      {==}
    case 0n 1n+q:
      {==}
    case 1n+p 0n:
      Empty.absurd({Nat.is_ge(0n, 1n+p) == True{} : Bool}, false_ne_true(h))
    case 1n+p 1n+q:
      le_ge(p, q, h)

law rows_apart_k:
  for +k: Nat
  for +x: Nat
  for +y: Nat
  for +w: Nat
  {MN.Boxes.apart.k(MN.Menu.rows(k, x, Nat.add(y, MN.Menu.rh()), w), MN.Box{x, y, w, MN.Menu.rh()}) == True{} : Bool}

def rows_apart_k(k, x, y, w):
  match k:
    case 0n:
      {==}
    case 1n+j:
      and_both(MN.Box.apart(MN.Box{x, y, w, MN.Menu.rh()}, MN.Box{x, Nat.add(y, MN.Menu.rh()), w, MN.Menu.rh()}),
        MN.Boxes.apart.k(MN.Menu.rows(j, x, Nat.add(Nat.add(y, MN.Menu.rh()), MN.Menu.rh()), w), MN.Box{x, Nat.add(y, MN.Menu.rh()), w, MN.Menu.rh()}),
        %Equal.sym(Bool, Nat.is_ge(Nat.add(y, MN.Menu.rh()), Nat.add(y, MN.Menu.rh())), True{}, Nat.ge_refl(Nat.add(y, MN.Menu.rh())))
          : {Bool.or(_, Nat.is_ge(x, Nat.add(x, w))) == True{} : Bool}
        {==},
        rows_apart_k(j, x, Nat.add(y, MN.Menu.rh()), w))

def Laws.menu_rows_apart(n, x, y, w):
  match n:
    case 0n:
      {==}
    case 1n+k:
      rows_apart_k(k, x, y, w)

# the next chip, wrapped (c) or not, is apart from this one
law flow_step_apart:
  for c: Bool
  for +left: Nat
  for +cx: Nat
  for +cy: Nat
  for +w: Nat
  for +w2: Nat
  for +h: Nat
  for +gap: Nat
  {MN.Box.apart(MN.Box{cx, cy, w, h}, MN.Box{Bool.pick(Nat, c, left, Nat.add(cx, Nat.add(w, gap))), Bool.pick(Nat, c, Nat.add(cy, Nat.add(h, gap)), cy), w2, h}) == True{} : Bool}

def flow_step_apart(c, left, cx, cy, w, w2, h, gap):
  match c:
    case True{}:
      %Equal.sym(Bool, Nat.is_ge(Nat.add(cy, Nat.add(h, gap)), Nat.add(cy, h)), True{}, ge_add_mid(cy, h, gap)) : {Bool.or(_, Nat.is_ge(left, Nat.add(cx, w))) == True{} : Bool}
      {==}
    case False{}:
      %Equal.sym(Bool, Nat.is_ge(Nat.add(cx, Nat.add(w, gap)), Nat.add(cx, w)), True{}, ge_add_mid(cx, w, gap)) : {Bool.or(Nat.is_ge(cy, Nat.add(cy, h)), _) == True{} : Bool}
      or_true_r(Nat.is_ge(cy, Nat.add(cy, h)))

law flow_apart_k:
  for t: List<&2, Nat>
  for +left: Nat
  for +right: Nat
  for +cx: Nat
  for +cy: Nat
  for +w: Nat
  for +h: Nat
  for +gap: Nat
  {MN.Boxes.apart.k(MN.Flow.place(t, left, right, Nat.add(cx, Nat.add(w, gap)), cy, h, gap), MN.Box{cx, cy, w, h}) == True{} : Bool}

def flow_apart_k(t, left, right, cx, cy, w, h, gap):
  match t:
    case Nil{}:
      {==}
    case +w2 <> +t2:
      +c = MN.Flow.wraps(left, right, Nat.add(cx, Nat.add(w, gap)), w2)
      +cx2 = Bool.pick(Nat, c, left, Nat.add(cx, Nat.add(w, gap)))
      +cy2 = Bool.pick(Nat, c, Nat.add(cy, Nat.add(h, gap)), cy)
      and_both(MN.Box.apart(MN.Box{cx, cy, w, h}, MN.Box{cx2, cy2, w2, h}),
        MN.Boxes.apart.k(MN.Flow.place(t2, left, right, Nat.add(cx2, Nat.add(w2, gap)), cy2, h, gap), MN.Box{cx2, cy2, w2, h}),
        flow_step_apart(c, left, cx, cy, w, w2, h, gap),
        flow_apart_k(t2, left, right, cx2, cy2, w2, h, gap))

def Laws.menu_chips_apart(ws, left, right, x, y, h, gap):
  match ws:
    case Nil{}:
      {==}
    case +w <> +t:
      +c = MN.Flow.wraps(left, right, x, w)
      flow_apart_k(t, left, right, Bool.pick(Nat, c, left, x), Bool.pick(Nat, c, Nat.add(y, Nat.add(h, gap)), y), w, h, gap)

# a line's top, wrapped (c) or not, is at least the line before
law flow_pick_ge:
  for c: Bool
  for +y: Nat
  for +d: Nat
  {Nat.is_ge(Bool.pick(Nat, c, Nat.add(y, d), y), y) == True{} : Bool}

def flow_pick_ge(c, y, d):
  match c:
    case True{}:
      ge_add(y, y, d, Nat.ge_refl(y))
    case False{}:
      Nat.ge_refl(y)

# the flow's last line is at or below its first
law flow_end_ge:
  for ws: List<&2, Nat>
  for +left: Nat
  for +right: Nat
  for +x: Nat
  for +y: Nat
  for +h: Nat
  for +gap: Nat
  {Nat.is_ge(MN.Flow.end(ws, left, right, x, y, h, gap), y) == True{} : Bool}

def flow_end_ge(ws, left, right, x, y, h, gap):
  match ws:
    case Nil{}:
      Nat.ge_refl(y)
    case +w <> +t:
      +c = MN.Flow.wraps(left, right, x, w)
      +cx = Bool.pick(Nat, c, left, x)
      +cy = Bool.pick(Nat, c, Nat.add(y, Nat.add(h, gap)), y)
      ge_trans(MN.Flow.end(t, left, right, Nat.add(cx, Nat.add(w, gap)), cy, h, gap), cy, y,
        flow_end_ge(t, left, right, Nat.add(cx, Nat.add(w, gap)), cy, h, gap),
        flow_pick_ge(c, y, Nat.add(h, gap)))

# every chip ends above any line at least a line and a gap past the last
law flow_above:
  for ws: List<&2, Nat>
  for +left: Nat
  for +right: Nat
  for +x: Nat
  for +y: Nat
  for +h: Nat
  for +gap: Nat
  for +top: Nat
  for +ht: {Nat.is_ge(top, Nat.add(MN.Flow.end(ws, left, right, x, y, h, gap), Nat.add(h, gap))) == True{} : Bool}
  {MN.Boxes.above(MN.Flow.place(ws, left, right, x, y, h, gap), top) == True{} : Bool}

def flow_above(ws, left, right, x, y, h, gap, top, ht):
  match ws:
    case Nil{}:
      {==}
    case +w <> +t:
      +c = MN.Flow.wraps(left, right, x, w)
      +cx = Bool.pick(Nat, c, left, x)
      +cy = Bool.pick(Nat, c, Nat.add(y, Nat.add(h, gap)), y)
      +e = MN.Flow.end(t, left, right, Nat.add(cx, Nat.add(w, gap)), cy, h, gap)
      and_both(Nat.is_ge(top, Nat.add(cy, h)), MN.Boxes.above(MN.Flow.place(t, left, right, Nat.add(cx, Nat.add(w, gap)), cy, h, gap), top),
        ge_trans(top, Nat.add(e, Nat.add(h, gap)), Nat.add(cy, h), ht,
          ge_add_both(e, cy, h, gap, flow_end_ge(t, left, right, Nat.add(cx, Nat.add(w, gap)), cy, h, gap))),
        flow_above(t, left, right, Nat.add(cx, Nat.add(w, gap)), cy, h, gap, top, ht))

def Laws.menu_chips_above_rows(ws, left, right, x, y, h, gap):
  match ws:
    case Nil{}:
      {==}
    case +w <> +t:
      flow_above(w <> t, left, right, x, y, h, gap, Nat.add(MN.Flow.end(w <> t, left, right, x, y, h, gap), Nat.add(h, gap)), Nat.ge_refl(Nat.add(MN.Flow.end(w <> t, left, right, x, y, h, gap), Nat.add(h, gap))))

law span_fit_k:
  for c: Bool
  for +k: Nat
  for +room: Nat
  for hc: {Nat.is_ge(room, MN.Menu.rh()) == c : Bool}
  for ih: {Nat.is_ge(Nat.sub(room, MN.Menu.rh()), MN.Menu.span(k, Nat.sub(room, MN.Menu.rh()))) == True{} : Bool}
  {Nat.is_ge(room, Bool.pick(Nat, c, Nat.add(MN.Menu.rh(), MN.Menu.span(k, Nat.sub(room, MN.Menu.rh()))), 0n)) == True{} : Bool}

def span_fit_k(c, k, room, hc, ih):
  match c:
    case True{}:
      sub_add_ge(room, MN.Menu.rh(), MN.Menu.span(k, Nat.sub(room, MN.Menu.rh())), hc, ih)
    case False{}:
      ge_zero(room)

def Laws.menu_rows_fit(n, room):
  match n:
    case 0n:
      ge_zero(room)
    case 1n+k:
      span_fit_k(Nat.is_ge(room, MN.Menu.rh()), k, room, {==}, Laws.menu_rows_fit(k, Nat.sub(room, MN.Menu.rh())))

law scroll_k:
  for c: Bool
  for +off: Nat
  for +n: Nat
  for +shown: Nat
  for hc: {Nat.is_le(off, Nat.sub(n, shown)) == c : Bool}
  for h: {Nat.is_ge(n, shown) == True{} : Bool}
  {Nat.is_ge(n, Nat.add(shown, Bool.pick(Nat, c, off, Nat.sub(n, shown)))) == True{} : Bool}

def scroll_k(c, off, n, shown, hc, h):
  match c:
    case True{}:
      sub_add_ge(n, shown, off, h, le_ge(off, Nat.sub(n, shown), hc))
    case False{}:
      sub_add_ge(n, shown, Nat.sub(n, shown), h, Nat.ge_refl(Nat.sub(n, shown)))

def Laws.menu_scroll_in_list(off, n, shown, h):
  scroll_k(Nat.is_le(off, Nat.sub(n, shown)), off, n, shown, {==}, h)

law width_k:
  for c: Bool
  for +want: Nat
  for +room: Nat
  for hc: {Nat.is_le(want, room) == c : Bool}
  {Nat.is_ge(room, Bool.pick(Nat, c, want, room)) == True{} : Bool}

def width_k(c, want, room, hc):
  match c:
    case True{}:
      le_ge(want, room, hc)
    case False{}:
      Nat.ge_refl(room)

def Laws.menu_width_fits(want, room):
  width_k(Nat.is_le(want, room), want, room, {==})

def Laws.menu_label_clear(name_x, widest, w):
  ge_add_mid(name_x, MN.Menu.col(widest, w), 16n)

def Laws.menu_press_first(n, x, y):
  {==}

def Laws.menu_right_press_first(n, x, y):
  {==}

def Laws.menu_press_nothing():
  {==}

def Laws.menu_press_elsewhere(x0, y0, x1, y1, a, v, h):
  h

def Laws.menu_press_rows_inside(x0, y0, x1, y1, v):
  {==}

def Laws.menu_press_box_inside(x0, y0, x1, y1, v):
  {==}

def Laws.menu_press_chips_inside(x0, y0, x1, y1, v):
  {==}

def Laws.badge_icons_exist():
  {==}

def Laws.badge_auto_tint(k):
  match k:
    case 0n:
      {==}
    case 1n+a:
      match a:
        case 0n:
          {==}
        case 1n+b:
          match b:
            case 0n:
              {==}
            case 1n+c:
              match c:
                case 0n:
                  {==}
                case 1n+d:
                  match d:
                    case 0n:
                      {==}
                    case 1n+e:
                      match e:
                        case 0n:
                          {==}
                        case 1n+f:
                          match f:
                            case 0n:
                              {==}
                            case 1n+g:
                              match g:
                                case 0n:
                                  {==}
                                case 1n+h:
                                  match h:
                                    case 0n:
                                      {==}
                                    case 1n+i:
                                      {==}

def Laws.badge_unknown_color(id):
  {==}

def Laws.badge_set_color(id):
  {==}

def Laws.badge_default_icon():
  {==}

def Laws.badge_unknown_icon():
  {==}

def Laws.mpick_closed_draws_nothing(u, h):
  %Equal.sym(Bool, U.Ui.flag(u, "proj"), False{}, h) : {MP.MPick.json.k(_, u) == J.Null{} : J.Json}
  {==}

def Laws.mpick_filter_asks_nothing(u, text, h):
  %Equal.sym(Bool, String.eq(MP.MPick.dir(text), U.Ui.scratch(u, "@mpick.asked")), True{}, h) : {MP.MPick.list.k(_, U.Ui.scratch_set(u, "@mpick", text), MP.MPick.dir(text)) == U.Act{U.Ui.scratch_set(u, "@mpick", text), Nil{}} : U.Act}
  {==}

def Laws.mpick_opens_home():
  {==}

def Laws.mpick_tilde():
  {==}

law title_k_nil:
  for same: Bool
  for +thread: String
  for +title: String
  for +now: Nat
  {H.Title.changes.k(same, False{}, thread, title, now) == [] : List<&2, M.Change>}

def title_k_nil(same, thread, title, now):
  match same:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.title_user_rename_wins(current, placeholder, answer, thread, now, h):
  %Equal.sym(Bool, String.eq(current, placeholder), False{}, h) : {H.Title.changes.k(Bool.and(_, Bool.not(String.is_empty(placeholder))), Bool.not(String.is_empty(AG.Title.clean(answer))), thread, AG.Title.clean(answer), now) == [] : List<&2, M.Change>}
  {==}

def Laws.title_applies_when_placeholder(current, placeholder, answer, thread, now, same, set, some):
  %Equal.sym(Bool, String.eq(current, placeholder), True{}, same) : {H.Title.changes.k(Bool.and(_, Bool.not(String.is_empty(placeholder))), Bool.not(String.is_empty(AG.Title.clean(answer))), thread, AG.Title.clean(answer), now) == [M.ThreadRenamed{thread, AG.Title.clean(answer), now}] : List<&2, M.Change>}
  %Equal.sym(Bool, String.is_empty(placeholder), False{}, set) : {H.Title.changes.k(Bool.and(True{}, Bool.not(_)), Bool.not(String.is_empty(AG.Title.clean(answer))), thread, AG.Title.clean(answer), now) == [M.ThreadRenamed{thread, AG.Title.clean(answer), now}] : List<&2, M.Change>}
  %Equal.sym(Bool, String.is_empty(AG.Title.clean(answer)), False{}, some) : {H.Title.changes.k(True{}, Bool.not(_), thread, AG.Title.clean(answer), now) == [M.ThreadRenamed{thread, AG.Title.clean(answer), now}] : List<&2, M.Change>}
  {==}

def Laws.title_empty_keeps(current, placeholder, thread, now):
  title_k_nil(Bool.and(String.eq(current, placeholder), Bool.not(String.is_empty(placeholder))), thread, AG.Title.clean(""), now)

def Laws.text_model_default():
  {==}

def Laws.text_model_unknown_provider():
  {==}

def Laws.text_model_light():
  {==}

def Laws.text_model_mismatch(provider, model, h):
  %Equal.sym(Bool, AG.Text.fits(AG.Text.norm(String.trim(provider)), String.trim(model)), False{}, h) : {AG.Tx{AG.Text.norm(String.trim(provider)), AG.Text.pick.m(Bool.and(Bool.not(String.is_empty(String.trim(model))), _), AG.Text.norm(String.trim(provider)), String.trim(model))} == AG.Tx{AG.Text.norm(String.trim(provider)), AG.Text.light(AG.Text.norm(String.trim(provider)))} : AG.Tx}
  %Equal.sym(Bool, Bool.and(Bool.not(String.is_empty(String.trim(model))), False{}), False{}, and_false_r(Bool.not(String.is_empty(String.trim(model))))) : {AG.Tx{AG.Text.norm(String.trim(provider)), AG.Text.pick.m(_, AG.Text.norm(String.trim(provider)), String.trim(model))} == AG.Tx{AG.Text.norm(String.trim(provider)), AG.Text.light(AG.Text.norm(String.trim(provider)))} : AG.Tx}
  {==}

def Laws.text_model_mismatch_codex():
  {==}

def Laws.text_model_mismatch_claude():
  {==}

def Laws.text_model_kept():
  {==}

def Laws.view_other_stays_shut(u, th, o, p, h):
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      %Equal.sym(Bool, pa, False{}, p) : {Bool.or(_, String.eq(th, sel)) == False{} : Bool}
      %Equal.sym(Bool, String.eq(th, sel), False{}, h) : {Bool.or(False{}, _) == False{} : Bool}
      {==}

def Laws.browser_page_own(th):
  {==}

# Hierarchy laws
# --------------

def Laws.hier_resolve_absolute(parent, file, h):
  %Equal.sym(Bool, String.starts_with(file, "/"), True{}, h) : {HI.Hier.resolve.k(_, parent, file) == file : String}
  {==}

def Laws.hier_resolve_beside():
  {==}

def Laws.hier_loop_listed_once():
  {==}

def Laws.hier_root_is_top(rs):
  match rs:
    case Nil{}:
      {==}
    case r <> t:
      match r:
        case HI.Row{d, n, f, l}:
          {==}

def Laws.hier_up_after_open():
  {==}

def Laws.draft_kept(u, text):
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      {==}

def Laws.draft_keep_thread(text):
  {==}

def Laws.draft_keep_no_thread(text):
  {==}

def Laws.draft_keep_no_scratch(text):
  {==}

def Laws.draft_keep_no_path(text):
  {==}

def Laws.ui_seq_counts_unknown():
  {==}

def Laws.ui_seq_known_unchanged():
  {==}

def Laws.ui_seq_log_counts_unknown():
  {==}

def Laws.reset_keeps_viewer():
  {==}

def Laws.reset_keeps_card():
  {==}

def Laws.reset_asks_card_again():
  {==}

def Laws.reset_keeps_hand():
  {==}

def Laws.reset_new_origin_drops_viewer():
  {==}

def Laws.reset_drops_results():
  {==}

def Laws.far_feed_after_stops():
  {==}

def Laws.draft_restore_fills(saved):
  {==}

def Laws.draft_restore_keeps_typing(c, t, saved):
  {==}

def Laws.interrupt_idle_nothing(s, now, client, id, thread):
  {==}

def Laws.interrupt_claude_input(th, r):
  {==}

def Laws.interrupt_codex_stops(th, r):
  {==}

def Laws.interrupt_grok_stops(th, r):
  {==}

def Laws.interrupt_muse_stops(th, r):
  {==}

def Laws.muse_run_turn(th, cwd, m, s, r, i, t, u):
  {==}

def Laws.msp_mode_supervised_asks():
  {==}

def Laws.msp_mode_full_allows():
  {==}

def Laws.archive_running_stops():
  {==}

def Laws.archive_idle_no_turn():
  {==}

def Laws.archive_closes_approval():
  {==}

def Laws.delete_running_stops():
  {==}

def Laws.interrupt_closes_approval():
  {==}

def Laws.interrupt_keeps_plan():
  {==}

def Laws.interrupt_ends_child_task():
  {==}

def Laws.gone_closes_approval():
  {==}

def Laws.retire_stop_tells():
  {==}

def Laws.archive_running_tells():
  {==}

def Laws.delete_running_tells():
  {==}

def Laws.archive_idle_tells_nothing():
  {==}

def Laws.island_sync_on_start():
  {==}

def Laws.island_sync_on_archive():
  {==}

def Laws.island_sync_on_delete():
  {==}

def Laws.island_sync_not_twice():
  {==}

def Laws.island_sync_quiet():
  {==}

def Laws.retire_answers_empty():
  {==}

def Laws.claude_result_after_stop_quiet():
  {==}

def Laws.claude_result_after_stop_interrupted():
  {==}

def Laws.claude_result_running_completes():
  {==}

def Laws.claude_delta_after_stop_quiet():
  {==}

def Laws.claude_delta_running_streams():
  {==}

def Laws.stopped_turn_quiet(s, now, thread, line):
  {==}

def Laws.recover_interrupts_running():
  {==}

def Laws.recover_closes_asks():
  {==}

def Laws.recover_leaves_idle():
  {==}

def Laws.recover_empty_nothing(now):
  {==}

def Laws.carry_continues():
  {==}

def Laws.carry_leaves_idle():
  {==}

def Laws.carry_off_nothing():
  {==}

def Laws.carry_once():
  {==}

def Laws.carry_twice_once():
  {==}

def Laws.carry_skips_archived():
  {==}

def Laws.ask_makes_pending():
  {==}

def Laws.answer_clears_pending():
  {==}

def Laws.settle_waits_for_children():
  {==}

def Laws.settle_children_done():
  {==}

def Laws.tag_thread():
  {==}

def Laws.tag_pid():
  {==}

def Laws.tag_untagged():
  {==}

def Laws.tag_old_process_unheard():
  {==}

def Laws.tag_untagged_heard(r):
  {==}

def Laws.follow_default_claude():
  {==}

def Laws.follow_default_codex():
  {==}

def Laws.follow_setting_queue(p):
  {==}

def Laws.follow_setting_steer(p):
  {==}

def Laws.follow_alt_same(s, p, a):
  {==}

def Laws.tailnet_link_token():
  {==}

def Laws.fold_roles_message_shown(i, th, r, x, a, t, info, running):
  {==}

def Laws.work_message_shown(live, u, b, info, now):
  match live:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.work_live_open(u, e, t, now):
  {==}

def Laws.btw_command():
  {==}

def Laws.btw_not_a_word():
  {==}

def Laws.btw_question():
  {==}

def Laws.btw_never_logged(s, c, id, th, q):
  {==}

def Laws.view_sheet_key():
  {==}

def Laws.view_board_no_sheet():
  {==}

def Laws.view_board_version_key():
  {==}

def Laws.view_bare_key():
  {==}

def Laws.pick_off_skipped():
  {==}

def Laws.kicad_ipc_needs_fork():
  {==}

def Laws.kicad_missing_said():
  {==}

def Laws.kicad_missing_prompt():
  {==}

def Laws.claude_kistack_plugin():
  {==}

def Laws.runtime_no_kistack_block():
  {==}

def Laws.mech_render_path():
  {==}

def Laws.mech_part_renders():
  {==}

def Laws.mech_key_path():
  {==}

def Laws.mech_default_assembly():
  {==}

def Laws.mech_default_first():
  {==}

def Laws.mech_gui_linux():
  {==}

def Laws.mech_gui_mac():
  {==}

def Laws.png_crc_iend():
  {==}

def Laws.freecad_missing_said():
  {==}

def Laws.freecad_missing_prompt():
  {==}

def Laws.took_seconds():
  {==}

def Laws.took_minutes():
  {==}

def Laws.took_hours():
  {==}

def Laws.follow_stop_idle(b):
  {==}

def Laws.follow_stop_typed(r):
  match r:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.follow_stop_running():
  {==}

def Laws.follow_stop_interrupts():
  {==}

def Laws.queue_mode_delivers(s, now, th, msg, role, text):
  {==}

def Laws.steer_claude_in_place(th):
  {==}

def Laws.steer_codex_restarts():
  {==}

def Laws.queue_joined():
  {==}

def Laws.queue_edit_in_place():
  {==}

def Laws.queue_remove_one():
  {==}

def Laws.queue_move_first():
  {==}

def Laws.queue_move_unknown():
  {==}

def Laws.queue_steer_one_leaves_rest():
  {==}

def Laws.queue_row_running(m):
  {==}

def Laws.queue_row_held(m):
  {==}

def Laws.hand_newest_fit():
  {==}

def Laws.hand_no_holes(fits, line, left, ls, om):
  {==}

def Laws.hand_cut_says():
  {==}

def Laws.handoff_never_runs_alone():
  {==}

def Laws.handoff_with_message_runs():
  {==}

def Laws.handoff_goes_first():
  {==}

def Laws.fork_records_source():
  {==}

def Laws.fork_waits_with_context():
  {==}

def Laws.back_needs_fork():
  {==}

def Laws.switch_between_turns():
  {==}

def Laws.switch_same_nothing():
  {==}

def Laws.switch_new_session():
  {==}

def Laws.switch_keeps_sessions():
  {==}

def Laws.switch_keeps_sessions_both():
  {==}

def Laws.switch_hands_new_provider():
  {==}

def Laws.switch_back_missed_nothing():
  {==}

def Laws.mcp_launch_in_project():
  {==}

def Laws.mcp_other_project_refused(run, s, now, t):
  {==}

def Laws.mcp_interrupt_idle_nothing(s, now, t):
  {==}

def Laws.mcp_done_idle():
  {==}

def Laws.mcp_not_done_running():
  {==}

def Laws.wait_at_least_a_second():
  {==}

def Laws.wait_at_most_ten_minutes():
  {==}
# Start-up
# --------

law cache_json_step:
  for b: Bool
  for +c: Char
  for +t: String
  for +mt: String
  for +e: {Char.is_eq(c, BT.Cache.nl()) == b : Bool}
  for ih: {BT.Cache.json(BT.Cache.show(BT.Cache.one(t), mt)) == BT.Cache.one(t) : String}
  {BT.Cache.json(BT.Cache.show(BT.Cache.one.k(Char.is_eq(c, BT.Cache.nl()), c, BT.Cache.one(t)), mt)) == BT.Cache.one.k(Char.is_eq(c, BT.Cache.nl()), c, BT.Cache.one(t)) : String}

def cache_json_step(b, c, t, mt, e, ih):
  match b:
    case True{}:
      %Equal.sym(Bool, Char.is_eq(c, BT.Cache.nl()), True{}, e) : {BT.Cache.json(BT.Cache.show(BT.Cache.one.k(_, c, BT.Cache.one(t)), mt)) == BT.Cache.one.k(_, c, BT.Cache.one(t)) : String}
      ih
    case False{}:
      %Equal.sym(Bool, Char.is_eq(c, BT.Cache.nl()), False{}, e) : {BT.Cache.json(BT.Cache.show(BT.Cache.one.k(_, c, BT.Cache.one(t)), mt)) == BT.Cache.one.k(_, c, BT.Cache.one(t)) : String}
      %Equal.sym(Bool, Char.is_eq(c, BT.Cache.nl()), False{}, e) : {BT.Cache.json.k(_, c, BT.Cache.json(BT.Cache.show(BT.Cache.one(t), mt))) == SCon{c, BT.Cache.one(t)} : String}
      Equal.cong(String, String, r => SCon{c, r}, BT.Cache.json(BT.Cache.show(BT.Cache.one(t), mt)), BT.Cache.one(t), ih)

def Laws.models_cache_json(ms, mt):
  match ms:
    case SNil{}:
      {==}
    case SCon{+c, +t}:
      cache_json_step(Char.is_eq(c, BT.Cache.nl()), c, t, mt, {==}, Laws.models_cache_json(t, mt))

law cache_text_step:
  for b: Bool
  for +c: Char
  for +t: String
  for +mt: String
  for +e: {Char.is_eq(c, BT.Cache.nl()) == b : Bool}
  for ih: {BT.Cache.text(BT.Cache.show(BT.Cache.one(t), mt)) == mt : String}
  {BT.Cache.text(BT.Cache.show(BT.Cache.one.k(Char.is_eq(c, BT.Cache.nl()), c, BT.Cache.one(t)), mt)) == mt : String}

def cache_text_step(b, c, t, mt, e, ih):
  match b:
    case True{}:
      %Equal.sym(Bool, Char.is_eq(c, BT.Cache.nl()), True{}, e) : {BT.Cache.text(BT.Cache.show(BT.Cache.one.k(_, c, BT.Cache.one(t)), mt)) == mt : String}
      ih
    case False{}:
      %Equal.sym(Bool, Char.is_eq(c, BT.Cache.nl()), False{}, e) : {BT.Cache.text(BT.Cache.show(BT.Cache.one.k(_, c, BT.Cache.one(t)), mt)) == mt : String}
      %Equal.sym(Bool, Char.is_eq(c, BT.Cache.nl()), False{}, e) : {BT.Cache.text.k(_, BT.Cache.show(BT.Cache.one(t), mt), BT.Cache.text(BT.Cache.show(BT.Cache.one(t), mt))) == mt : String}
      ih

def Laws.models_cache_text(ms, mt):
  match ms:
    case SNil{}:
      {==}
    case SCon{+c, +t}:
      cache_text_step(Char.is_eq(c, BT.Cache.nl()), c, t, mt, {==}, Laws.models_cache_text(t, mt))

def Laws.models_quiet(ms):
  %Equal.sym(Bool, String.eq(ms, ms), True{}, Laws.string_eq_refl(ms)) : {Bool.not(_) == False{} : Bool}
  {==}

law store_put_st:
  for d: Maybe<&2, M.Change>
  for +rest: List<&2, M.Change>
  for +st: M.State
  {M.State.replay(rest, ST.Store.apply(d, st)) == M.State.replay(ST.Store.put(d, rest), st) : M.State}

def store_put_st(d, rest, st):
  match d:
    case Some{c}:
      {==}
    case None{}:
      {==}

law store_step_st:
  for m: Maybe<&2, J.Json>
  for +l: String
  for a: ST.Loaded
  for +rest: List<&2, M.Change>
  {M.State.replay(rest, ST.Loaded.st(ST.Store.step(m, l, a))) == M.State.replay(ST.Store.change(m, rest), ST.Loaded.st(a)) : M.State}

def store_step_st(m, l, a, rest):
  match m a:
    case Some{+o} ST.Loaded{+st, ws}:
      store_put_st(C.Change.decode(o), rest, st)
    case None{} _:
      {==}

def Laws.store_state_is_replay(ls, a):
  match ls:
    case Nil{}:
      {==}
    case +l <> +t:
      Equal.trans(M.State, ST.Loaded.st(ST.Store.load(t, ST.Store.step(J.Json.parse(l), l, a))),
        M.State.replay(ST.Store.changes(t), ST.Loaded.st(ST.Store.step(J.Json.parse(l), l, a))),
        M.State.replay(ST.Store.change(J.Json.parse(l), ST.Store.changes(t)), ST.Loaded.st(a)),
        Laws.store_state_is_replay(t, ST.Store.step(J.Json.parse(l), l, a)),
        store_step_st(J.Json.parse(l), l, a, ST.Store.changes(t)))

def Laws.store_load_append(xs, ys, a):
  match xs:
    case Nil{}:
      {==}
    case +l <> t:
      Laws.store_load_append(t, ys, ST.Store.step(J.Json.parse(l), l, a))

# Frames
# ------

law link_more_step:
  for b: Bool
  for +c: Char
  for +t: String
  for +n: String
  for +e: {Char.is_eq(c, M.Link.colon()) == b : Bool}
  for ih: {M.Link.more(String.append(M.Link.bare(t), SCon{M.Link.colon(), n})) == True{} : Bool}
  {M.Link.more(String.append(M.Link.bare.k(Char.is_eq(c, M.Link.colon()), c, M.Link.bare(t)), SCon{M.Link.colon(), n})) == True{} : Bool}

def link_more_step(b, c, t, n, e, ih):
  match b:
    case True{}:
      %Equal.sym(Bool, Char.is_eq(c, M.Link.colon()), True{}, e) : {M.Link.more(String.append(M.Link.bare.k(_, c, M.Link.bare(t)), SCon{M.Link.colon(), n})) == True{} : Bool}
      ih
    case False{}:
      %Equal.sym(Bool, Char.is_eq(c, M.Link.colon()), False{}, e) : {M.Link.more(String.append(M.Link.bare.k(_, c, M.Link.bare(t)), SCon{M.Link.colon(), n})) == True{} : Bool}
      %Equal.sym(Bool, Char.is_eq(c, M.Link.colon()), False{}, e) : {M.Link.more.k(_, M.Link.more(String.append(M.Link.bare(t), SCon{M.Link.colon(), n}))) == True{} : Bool}
      ih

law link_more:
  for +s: String
  for +n: String
  {M.Link.more(String.append(M.Link.bare(s), SCon{M.Link.colon(), n})) == True{} : Bool}

def link_more(s, n):
  match s:
    case SNil{}:
      {==}
    case SCon{+c, +t}:
      link_more_step(Char.is_eq(c, M.Link.colon()), c, t, n, {==}, link_more(t, n))

law link_field_step:
  for b: Bool
  for +c: Char
  for +t: String
  for +n: String
  for +e: {Char.is_eq(c, M.Link.colon()) == b : Bool}
  for ih: {M.Link.field(String.append(M.Link.bare(t), SCon{M.Link.colon(), n})) == M.Link.bare(t) : String}
  {M.Link.field(String.append(M.Link.bare.k(Char.is_eq(c, M.Link.colon()), c, M.Link.bare(t)), SCon{M.Link.colon(), n})) == M.Link.bare.k(Char.is_eq(c, M.Link.colon()), c, M.Link.bare(t)) : String}

def link_field_step(b, c, t, n, e, ih):
  match b:
    case True{}:
      %Equal.sym(Bool, Char.is_eq(c, M.Link.colon()), True{}, e) : {M.Link.field(String.append(M.Link.bare.k(_, c, M.Link.bare(t)), SCon{M.Link.colon(), n})) == M.Link.bare.k(_, c, M.Link.bare(t)) : String}
      ih
    case False{}:
      %Equal.sym(Bool, Char.is_eq(c, M.Link.colon()), False{}, e) : {M.Link.field(String.append(M.Link.bare.k(_, c, M.Link.bare(t)), SCon{M.Link.colon(), n})) == M.Link.bare.k(_, c, M.Link.bare(t)) : String}
      %Equal.sym(Bool, Char.is_eq(c, M.Link.colon()), False{}, e) : {M.Link.field.k(_, c, M.Link.field(String.append(M.Link.bare(t), SCon{M.Link.colon(), n}))) == SCon{c, M.Link.bare(t)} : String}
      Equal.cong(String, String, r => SCon{c, r}, M.Link.field(String.append(M.Link.bare(t), SCon{M.Link.colon(), n})), M.Link.bare(t), ih)

law link_field:
  for +s: String
  for +n: String
  {M.Link.field(String.append(M.Link.bare(s), SCon{M.Link.colon(), n})) == M.Link.bare(s) : String}

def link_field(s, n):
  match s:
    case SNil{}:
      {==}
    case SCon{+c, +t}:
      link_field_step(Char.is_eq(c, M.Link.colon()), c, t, n, {==}, link_field(t, n))

def Laws.link_from_id(from, n):
  %Equal.sym(Bool, M.Link.more(String.append(M.Link.bare(from), SCon{M.Link.colon(), n})), True{}, link_more(from, n)) : {M.Link.from.e(_, String.append(M.Link.bare(from), SCon{M.Link.colon(), n})) == M.Link.bare(from) : String}
  link_field(from, n)

def Laws.link_from_plain(c, t, e):
  %Equal.sym(Bool, Char.is_eq(c, Chr{116}), False{}, e) : {M.Link.from.b(_, t) == "" : String}
  {==}

def Laws.tl_memos_hit(px, sel, its, lv, ms):
  %Equal.sym(Bool, Q.Eq.str(sel, sel), True{}, eq_str_refl(sel)) : {L.TL.memos.pick.k(_, L.TLMemo{px, sel, its, lv}, L.TL.memos.pick(L.TL.memos.cut(Nat.sub(L.TL.memos.max(), 1n), L.TL.memos.drop(ms, sel)), sel)) == L.TLMemo{px, sel, its, lv} : L.TLMemo}
  {==}

law memos_cut_le:
  for +n: Nat
  for ms: List<&2, L.TLMemo>
  {Nat.is_le(List.length(&2, L.TLMemo, L.TL.memos.cut(n, ms)), n) == True{} : Bool}

def memos_cut_le(n, ms):
  match n ms:
    case 0n _:
      {==}
    case 1n+k Nil{}:
      {==}
    case 1n+k m <> t:
      memos_cut_le(k, t)

def Laws.tl_memos_bounded(m, ms):
  memos_cut_le(Nat.sub(L.TL.memos.max(), 1n), L.TL.memos.drop(ms, L.TL.memo.sel(m)))

def Laws.window_state_private(s):
  match s:
    case M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw}:
      {==}

def Laws.window_state_seq(s):
  match s:
    case M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw}:
      {==}

def reselect_state(same: Bool, +u: U.Ui, +old: String) -> {U.Ui.state(U.Ui.reselect.k(same, u, old)) == U.Ui.state(u) : M.State}:
  match same u:
    case True{} _: {==}
    case False{} U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}: {==}

def wanted_bot_state(m: Maybe<&2, BOT.Bot>, +u: U.Ui) -> {U.Ui.state(U.Bots.wanted.m(m, u)) == U.Ui.state(u) : M.State}:
  match m u:
    case Some{BOT.Bot{i, n, th, lk, p, c, d}} U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}: {==}
    case None{} _: {==}

def wanted_thread_state(found: Bool, +u: U.Ui, +id: String) -> {U.Ui.state(U.Want.thread.k(found, u, id)) == U.Ui.state(u) : M.State}:
  match found u:
    case True{} U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}: {==}
    case False{} _: {==}

def wanted_state(bound: Bool, +u: U.Ui) -> {U.Ui.state(U.Bots.wanted.k(bound, u)) == U.Ui.state(u) : M.State}:
  match bound:
    case False{}: {==}
    case True{}:
      +m = BOT.World.find(M.State.bots(U.Ui.state.of(u)), U.Ui.scratch(u, "@bot.want"))
      +found = Bool.and(Bool.not(String.is_empty(U.Ui.scratch(u, "@thread.want"))), U.Want.thread.exists(U.Ui.state.of(u), U.Ui.scratch(u, "@thread.want")))
      +id = U.Ui.scratch(u, "@thread.want")
      Equal.trans(M.State, U.Ui.state(U.Want.thread.k(found, U.Bots.wanted.m(m, u), id)), U.Ui.state(U.Bots.wanted.m(m, u)), U.Ui.state(u),
        wanted_thread_state(found, U.Bots.wanted.m(m, u), id), wanted_bot_state(m, u))

def wanted_any_state(pending: Bool, +u: U.Ui) -> {U.Ui.state(U.Bots.wanted.any(pending, u)) == U.Ui.state(u) : M.State}:
  match pending:
    case False{}: {==}
    case True{}: wanted_state(U.Want.bound(u), u)

def installed_state(+u: U.Ui) -> {U.Ui.state(U.Ui.installed(u)) == U.Ui.state(u) : M.State}:
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}: {==}

def adopt_raw_state(+u: U.Ui, +st: M.State) -> {U.Ui.state(U.Ui.adopt.raw(u, st)) == st : M.State}:
  match u:
    case U.Ui{s0, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}: {==}

def Laws.window_adopt(u, st):
  match u st:
    case +uu +ss:
      +raw = U.Ui.adopt.raw(uu, ss)
      +ins = U.Ui.installed(raw)
      +wanted = U.Bots.wanted(ins)
      Equal.trans(M.State, U.Ui.state(U.Ui.reselect(U.Ui.sel(uu), wanted)), U.Ui.state(wanted), ss,
        reselect_state(String.eq(U.Ui.sel(uu), U.Ui.sel(wanted)), wanted, U.Ui.sel(uu)),
        Equal.trans(M.State, U.Ui.state(wanted), U.Ui.state(ins), ss, wanted_any_state(Bool.or(Bool.not(String.is_empty(U.Ui.scratch(ins, "@bot.want"))), Bool.not(String.is_empty(U.Ui.scratch(ins, "@thread.want")))), ins),
          Equal.trans(M.State, U.Ui.state(ins), U.Ui.state(raw), ss, installed_state(raw), adopt_raw_state(uu, ss))))

def Laws.todo_write_whole_list():
  {==}

def Laws.todo_line_active_form():
  {==}

def Laws.todo_task_created():
  {==}

def Laws.todo_task_result_number():
  {==}

def Laws.todo_task_update():
  {==}

def Laws.todo_task_delete():
  {==}

def Laws.todo_codex_completed():
  {==}

def Laws.todo_acp_plan():
  {==}

def Laws.todo_shown_while_running():
  {==}

def Laws.todo_hidden_when_done():
  {==}

def Laws.todo_rows_around_open():
  {==}

def Laws.todo_rows_last():
  {==}

def Laws.todo_rows_short():
  {==}

def Laws.todo_write_sets_list():
  {==}

def Laws.todo_shown_while_left(run):
  match run:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.usage_keeps_window():
  {==}

def Laws.usage_keeps_used():
  {==}

def Laws.usage_text_share():
  {==}

def Laws.usage_text_tokens():
  {==}

def Laws.usage_nothing_yet(w):
  {==}

def Laws.usage_same_nothing():
  {==}

def Laws.usage_change_logged():
  {==}

def Laws.view_drag_rotates():
  {==}

def Laws.view_drag_ctrl_pans():
  {==}

def Laws.view_drag_shift_zooms():
  {==}

def Laws.view_drag_alt_rolls():
  {==}

def Laws.view_key_top():
  {==}

def Laws.view_key_iso():
  {==}

def Laws.view_key_ctrl_digit_free():
  {==}

def Laws.view_key_arrow_turns():
  {==}

def Laws.view_key_shift_arrow_quarter():
  {==}

def Laws.view_key_alt_arrow_rolls():
  {==}

def Laws.view_key_ctrl_arrow_pans():
  {==}

def Laws.view_key_z_out():
  {==}

def Laws.view_key_shift_z_in():
  {==}

def Laws.view_key_other_none():
  {==}

def Laws.sign_stale_rejected(secret, now, ts, body, sig, h):
  %Equal.sym(Bool, SG.Sign.fresh(now, ts), False{}, h) : {SG.Sign.check.k(_, secret, ts, body, sig) == False{} : Bool}
  {==}

def Laws.sign_window_300():
  {==}

def Laws.seen_add_has(s, k, now):
  match s:
    case SG.Seen{items}:
      %Equal.sym(Bool, String.eq(k, k), True{}, Laws.string_eq_refl(k)) : {Bool.or(_, SG.Seen.any(SG.Seen.prune(items, now), k)) == True{} : Bool}
      {==}

def Laws.invite_roundtrip():
  {==}

# Routine schedules
# -----------------

law cron_guard_ok:
  for +a: Nat
  for b: Bool
  for x: Nat
  for h: {Nat.is_gt(x, a) == b : Bool}
  {CR.Cron.ok(a, CR.Cron.guard.k(b, x)) == True{} : Bool}

def cron_guard_ok(a, b, x, h):
  match b:
    case True{}:
      match x:
        case 0n:
          {==}
        case 1n+k:
          h
    case False{}:
      {==}

def Laws.cron_next_after(c, after, off, east):
  cron_guard_ok(after, Nat.is_gt(CR.Cron.raw(c, after, off, east), after), CR.Cron.raw(c, after, off, east), {==})

def Laws.cron_due_fires_once(c, now, off, east):
  %Equal.sym(Cmp, Nat.cmp(now, now), EQ{}, nat_cmp_refl(now)) : {CR.Cron.due.k(_, CR.Cron.next(c, now, off, east), now) == False{} : Bool}
  {==}
# Bot spaces
# ----------

# taking n blocks leaves at most n
law take_le:
  for xs: List<&2, SPC.Block>
  for n: Nat
  {Nat.is_le(List.length(&2, SPC.Block, List.take(&2, SPC.Block, xs, n)), n) == True{} : Bool}

def take_le(xs, n):
  match xs n:
    case Nil{} 0n:
      {==}
    case Nil{} 1n+p:
      {==}
    case h <> t 0n:
      {==}
    case h <> t 1n+p:
      take_le(t, p)

law starts_with_empty:
  for s: String
  {String.starts_with(s, "") == True{} : Bool}

def starts_with_empty(s):
  match s:
    case SNil{}:
      {==}
    case SCon{h, t}:
      {==}

def Laws.space_caps(j):
  take_le(SPC.Sp.all(j), 200n)

def Laws.space_progress_max(p):
  min_le_k(Nat.is_le(p, 1000n), p, 1000n, {==})

def Laws.space_message_prefix(action, value):
  starts_with_empty(SPC.Sp.msg.k(String.is_empty(value), action, value))

def Laws.space_page_locked(html):
  starts_with_empty(html)

def Laws.space_link_only_ours(u, h):
  %Equal.sym(Bool, String.starts_with(u, "space:"), False{}, h) : {SPC.Sp.link.k(_, u) == Nil{} : List<&2, String>}
  {==}
# Google laws
# -----------

def Laws.google_scopes_nonempty():
  {==}

def Laws.google_b64url_roundtrip_example():
  {==}

def Laws.google_code_of_example():
  {==}

def Laws.google_denied_no_code():
  {==}

def Laws.google_header_no_newline_example():
  {==}

def Laws.google_send_asks():
  {==}

def Laws.google_create_asks():
  {==}

def google_add60_gt(n: Nat) -> {GT{} == Nat.cmp(Nat.add(n, 60n), n) : Cmp}:
  match n:
    case 0n:
      {==}
    case 1n+p:
      google_add60_gt(p)

def Laws.google_expiring_not_fresh(now):
  %google_add60_gt(now) : {Cmp.is_lt(_) == False{} : Bool}
  {==}

def Laws.google_state_home_example():
  {==}

def Laws.google_hop_other():
  {==}

def Laws.google_hop_mine():
  {==}

def Laws.google_hop_once():
  {==}

def Laws.google_hop_tailnet_only():
  {==}

def Laws.google_hop_no_path():
  {==}

def Laws.google_hop_origin_header_injection_denied():
  {==}

def Laws.google_hop_query_header_injection_denied():
  {==}

def Laws.google_hop_wrapping_port_denied():
  {==}

def Laws.google_hop_raw_fragment_denied():
  {==}

def Laws.google_loopback_phone():
  {==}

def Laws.google_loopback_none():
  {==}

def Laws.google_loopback_not_host():
  {==}

def Laws.google_loopback_not_userinfo():
  {==}

def Laws.google_loopback_not_path():
  {==}

def Laws.google_loopback_no_port():
  {==}

def Laws.google_own_client_wins(mine, sh, h):
  %Equal.sym(Bool, GO.Client.has(mine), True{}, h) : {GO.Google.client.k(_, mine, sh) == mine : GO.Client}
  {==}

def Laws.google_shared_scopes_all():
  {==}

def Laws.google_shared_covers_all(tool):
  {==}

def Laws.google_own_covers_all(tool):
  {==}

def Laws.google_accts_pick_first(a, t):
  {==}

def Laws.google_accts_put_replaces(a, t, b, h):
  %Equal.sym(Bool, String.eq(GO.Acct.email(a), GO.Acct.email(b)), True{}, h) : {GO.Accts.put.go(a <> t, _, b, GO.Acct.email(b)) == b <> t : List<&2, GO.Acct>}
  {==}

def Laws.google_accts_put_new(b):
  {==}

def Laws.google_accts_drop_named(a, t, e, h):
  %Equal.sym(Bool, String.eq(GO.Acct.email(a), e), True{}, h) : {GO.Accts.drop.go(a <> t, _, e) == GO.Accts.drop(t, e) : List<&2, GO.Acct>}
  {==}

# Bots
# ----

def Laws.bot_hop_step(ok, n, h):
  match ok:
    case True{}:
      h
    case False{}:
      {==}

def Laws.bot_hop_bounded(h):
  Laws.bot_hop_step(Nat.is_le(1n+h, BOT.Hop.max()), 1n+h, {==})

def Laws.bot_hop_busy_keeps(c, h):
  match c:
    case BOT.Hops{n, w, held}:
      {==}

def Laws.bot_hop_idle_starts(c, h):
  {==}

def Laws.bot_hop_queue_takes(n, w, h):
  {==}

def Laws.bot_hop_wakes_keep(hs, c):
  match hs:
    case Nil{}:
      {==}
    case +h <> t:
      Equal.trans(Nat, BOT.Hops.now(BOT.Hop.wakes(t, BOT.Hop.woke(True{}, c, h))), BOT.Hops.now(BOT.Hop.woke(True{}, c, h)), BOT.Hops.now(c),
        Laws.bot_hop_wakes_keep(t, BOT.Hop.woke(True{}, c, h)), Laws.bot_hop_busy_keeps(c, h))

def Laws.bot_hop_fanout(hs, c, h):
  Equal.cong(Nat, Maybe<&2, Nat>, x => BOT.Hop.next(x), BOT.Hops.now(BOT.Hop.wakes(hs, BOT.Hop.woke(False{}, c, h))), h,
    Laws.bot_hop_wakes_keep(hs, BOT.Hop.woke(False{}, c, h)))

def Laws.bot_loop_up(ok, b, h, q, e):
  match ok:
    case True{}:
      match b:
        case True{}:
          q
        case False{}:
          q
    case False{}:
      e

def Laws.bot_loop_bounded(bs, h, e):
  match bs:
    case Nil{}:
      e
    case +b <> t:
      Laws.bot_loop_bounded(t, BOT.Hop.up.k(BOT.Hop.next(h), b, h), Laws.bot_loop_up(Nat.is_le(1n+h, BOT.Hop.max()), b, h, {==}, e))

def Laws.bot_loop_stops():
  {==}

def Laws.bot_held_wakes_none(st):
  {==}

def Laws.bot_room_no_takeover(i, n, c, s, pid):
  {==}

def Laws.bot_room_deleted_stays(i, n, ms, c, s, pid):
  {==}

# Each room once
# --------------

law rooms_or_false_l:
  for a: Bool
  for -b: Bool
  for h: {Bool.or(a, b) == False{} : Bool}
  {a == False{} : Bool}

def rooms_or_false_l(a, b, h):
  match a:
    case False{}:
      {==}
    case True{}:
      Empty.absurd({True{} == False{} : Bool}, false_ne_true(Equal.sym(Bool, True{}, False{}, h)))

law rooms_or_false_r:
  for a: Bool
  for b: Bool
  for h: {Bool.or(a, b) == False{} : Bool}
  {b == False{} : Bool}

def rooms_or_false_r(a, b, h):
  match a:
    case False{}:
      h
    case True{}:
      Empty.absurd({b == False{} : Bool}, false_ne_true(Equal.sym(Bool, True{}, False{}, h)))

# keeping each key once only drops items: a key not among xs is not among
# what is kept
law rooms_once_has_step:
  for -A: Data
  for b: Bool
  for +x: String
  for v: A
  for +t: List<&2, BU.RK<A>>
  for +k: String
  for +e: {BU.BU.once.has(A, t, x) == b : Bool}
  for hx: {String.eq(x, k) == False{} : Bool}
  for ih: {BU.BU.once.has(A, BU.BU.once.of(A, t), k) == False{} : Bool}
  {BU.BU.once.has(A, BU.BU.once.step(A, BU.BU.once.has(A, t, x), x, v, BU.BU.once.of(A, t)), k) == False{} : Bool}

def rooms_once_has_step(A, b, x, v, t, k, e, hx, ih):
  match b:
    case True{}:
      %Equal.sym(Bool, BU.BU.once.has(A, t, x), True{}, e) : {BU.BU.once.has(A, BU.BU.once.step(A, _, x, v, BU.BU.once.of(A, t)), k) == False{} : Bool}
      ih
    case False{}:
      %Equal.sym(Bool, BU.BU.once.has(A, t, x), False{}, e) : {BU.BU.once.has(A, BU.BU.once.step(A, _, x, v, BU.BU.once.of(A, t)), k) == False{} : Bool}
      %Equal.sym(Bool, String.eq(x, k), False{}, hx) : {Bool.or(_, BU.BU.once.has(A, BU.BU.once.of(A, t), k)) == False{} : Bool}
      %Equal.sym(Bool, BU.BU.once.has(A, BU.BU.once.of(A, t), k), False{}, ih) : {Bool.or(False{}, _) == False{} : Bool}
      {==}

law rooms_once_has:
  for -A: Data
  for xs: List<&2, BU.RK<A>>
  for +k: String
  for +h: {BU.BU.once.has(A, xs, k) == False{} : Bool}
  {BU.BU.once.has(A, BU.BU.once.of(A, xs), k) == False{} : Bool}

def rooms_once_has(A, xs, k, h):
  match xs:
    case Nil{}:
      {==}
    case BU.RK{+x, v} <> +t:
      rooms_once_has_step(A, BU.BU.once.has(A, t, x), x, v, t, k, {==},
        rooms_or_false_l(String.eq(x, k), BU.BU.once.has(A, t, k), h),
        rooms_once_has(A, t, k, rooms_or_false_r(String.eq(x, k), BU.BU.once.has(A, t, k), h)))

law rooms_once_step:
  for -A: Data
  for b: Bool
  for +x: String
  for v: A
  for +t: List<&2, BU.RK<A>>
  for +e: {BU.BU.once.has(A, t, x) == b : Bool}
  for ih: {BU.BU.once.distinct(A, BU.BU.once.of(A, t)) == True{} : Bool}
  {BU.BU.once.distinct(A, BU.BU.once.step(A, BU.BU.once.has(A, t, x), x, v, BU.BU.once.of(A, t))) == True{} : Bool}

def rooms_once_step(A, b, x, v, t, e, ih):
  match b:
    case True{}:
      %Equal.sym(Bool, BU.BU.once.has(A, t, x), True{}, e) : {BU.BU.once.distinct(A, BU.BU.once.step(A, _, x, v, BU.BU.once.of(A, t))) == True{} : Bool}
      ih
    case False{}:
      %Equal.sym(Bool, BU.BU.once.has(A, t, x), False{}, e) : {BU.BU.once.distinct(A, BU.BU.once.step(A, _, x, v, BU.BU.once.of(A, t))) == True{} : Bool}
      %Equal.sym(Bool, BU.BU.once.has(A, BU.BU.once.of(A, t), x), False{}, rooms_once_has(A, t, x, e)) : {Bool.and(Bool.not(_), BU.BU.once.distinct(A, BU.BU.once.of(A, t))) == True{} : Bool}
      ih

# what is kept names each key once, whatever the list
law rooms_once_distinct:
  for -A: Data
  for xs: List<&2, BU.RK<A>>
  {BU.BU.once.distinct(A, BU.BU.once.of(A, xs)) == True{} : Bool}

def rooms_once_distinct(A, xs):
  match xs:
    case Nil{}:
      {==}
    case BU.RK{+x, v} <> +t:
      rooms_once_step(A, BU.BU.once.has(A, t, x), x, v, t, {==}, rooms_once_distinct(A, t))

def Laws.rooms_listed_once(u):
  +rs = BOT.World.rooms(M.State.bots(U.Ui.state(u)))
  rooms_once_distinct(BU.RRow, BU.BU.rooms.go(rs, BU.BU.rooms.names(rs), BU.BU.room.sel(u)))

def Laws.hubs_rooms_listed_once(ss, many):
  rooms_once_distinct(J.Json, Hb.Hubs.rkeyed(Hb.Hubs.all(ss, "rooms", many)))

def Laws.rooms_same_id_once():
  {==}

def Laws.rooms_same_name_both():
  {==}

def Laws.bot_mood_wait(talked, idle):
  {==}

def Laws.bot_mood_input(talked, idle):
  {==}

def Laws.bot_mood_sleep_still():
  {==}

def Laws.bot_mood_rest(talked):
  {==}
# Cats
# ----

def Laws.cat_sleep_still():
  {==}

def Laws.cat_error_still():
  {==}

def Laws.cat_away_still():
  {==}

# a greyed rig carries no animation
law cat_grey_still:
  for ps: List<&2, CAT.Part>
  {CAT.Cat.anims.count(CAT.Cat.grey.all(ps)) == 0n : Nat}

def cat_grey_still(ps):
  match ps:
    case Nil{}:
      {==}
    case CAT.Part{n, d, f, l, w, a, as} <> t:
      cat_grey_still(t)

def Laws.cat_away_holds_still(look):
  cat_grey_still(CAT.Cat.rig.k(CAT.Cat.look(look), 7))

def Laws.cat_moods_known():
  {==}

law cat_add_zero:
  for r: Nat
  {Nat.add(r, 0n) == r : Nat}

def cat_add_zero(r):
  match r:
    case 0n:
      {==}
    case 1n+p:
      %cat_add_zero(p) : {1n+Nat.add(p, 0n) == 1n+_ : Nat}
      {==}

law cat_lt_succ_add:
  for r: Nat
  for -m: Nat
  {Nat.is_lt(r, 1n+Nat.add(r, m)) == True{} : Bool}

def cat_lt_succ_add(r, m):
  match r:
    case 0n:
      {==}
    case 1n+p:
      cat_lt_succ_add(p, m)

# Nat.divmod's loop keeps r + m = b - 1, so the remainder stays below b
law cat_mod_lt:
  for n: Nat
  for +m: Nat
  for -d: Nat
  for +r: Nat
  for -bp: Nat
  for e: {Nat.add(r, m) == bp : Nat}
  {Nat.is_lt(Pair.snd(Nat, Nat, Nat.divmod.go(n, m, d, r)), 1n+bp) == True{} : Bool}

def cat_mod_lt(n, m, d, r, bp, e):
  match n m:
    case 0n _:
      %e : {Nat.is_lt(r, 1n+_) == True{} : Bool}
      cat_lt_succ_add(r, m)
    case 1n+np 0n:
      cat_mod_lt(np, r, 1n+d, 0n, bp, Equal.trans(Nat, r, Nat.add(r, 0n), bp, Equal.sym(Nat, Nat.add(r, 0n), r, cat_add_zero(r)), e))
    case 1n+np 1n+mp:
      cat_mod_lt(np, mp, d, 1n+r, bp, Equal.trans(Nat, 1n+Nat.add(r, mp), Nat.add(r, 1n+mp), bp, Equal.sym(Nat, Nat.add(r, 1n+mp), 1n+Nat.add(r, mp), add_succ_r(r, mp)), e))

def Laws.cat_look_total(s):
  cat_mod_lt(s, 71n, 0n, 0n, 71n, {==})

def Laws.index_stale_rejected(secret, now, ts, delivery, trigger, test, body, sig, h):
  %Equal.sym(Bool, SG.Sign.fresh(now, ts), False{}, h) : {SG.Sign.index.k(_, secret, ts, delivery, trigger, test, body, sig) == False{} : Bool}
  {==}

def Laws.bearer_needs_token(given):
  {==}

def Laws.ring_form_json():
  {==}

def Laws.tailnet_serve_https():
  {==}

def Laws.bot_spawn_step(ok, n, h):
  match ok:
    case True{}:
      h
    case False{}:
      {==}

def Laws.bot_spawn_bounded(d):
  Laws.bot_spawn_step(Nat.is_le(1n+d, BHS.Spawn.max()), 1n+d, {==})

def Laws.bot_request_keep(dm, r, i, n, ms, now, h, e):
  match dm:
    case True{}:
      %Equal.sym(Bool, String.starts_with(i, "dm:"), False{}, h) : {Bool.not(_) == True{} : Bool}
      {==}
    case False{}:
      %Equal.sym(Bool, BOT.Room.is_dm(r), False{}, e) : {Bool.not(_) == True{} : Bool}
      {==}

def Laws.bot_request_in_room(m, i, n, ms, now, h):
  match m:
    case Some{+r}:
      Laws.bot_request_keep(BOT.Room.is_dm(r), r, i, n, ms, now, h, {==})
    case None{}:
      %Equal.sym(Bool, String.starts_with(i, "dm:"), False{}, h) : {Bool.not(_) == True{} : Bool}
      {==}

def Laws.bot_request_fresh_id(now, seq):
  {==}

def Laws.bot_delegate_needs_project(s, now, t, proj, p):
  {==}

def Laws.kids_bot_child_shown(m, c):
  {==}

def Laws.bot_mood_idle_still():
  {==}
def Laws.perf_errors_always_logged(th):
  {==}

def Laws.perf_debug_quiet_by_default():
  {==}

def Laws.perf_gauge_window_is_reading(name, n, sum, max, last, bs, old):
  {==}

def Laws.perf_pct_empty(name, per):
  {==}

def Laws.perf_pct_bucket_top():
  {==}

def Laws.perf_pct_capped_by_max():
  {==}

def Laws.perf_hud_first_frame(cur, at, now):
  {==}

def Laws.perf_hud_off_draws_nothing(ty, text, w):
  {==}

# Dictation
# ---------

law voice_cap_le:
  for -A: Data
  for n: Nat
  for xs: List<&2, A>
  {Nat.is_le(List.length(&2, A, VO.Voice.cap(A, n, xs)), n) == True{} : Bool}

def voice_cap_le(A, n, xs):
  match n xs:
    case 0n _:
      {==}
    case 1n+p Nil{}:
      {==}
    case 1n+p h <> t:
      voice_cap_le(A, p, t)

law voice_or_true:
  for a: Bool
  {Bool.or(a, True{}) == True{} : Bool}

def voice_or_true(a):
  match a:
    case True{}:
      {==}
    case False{}:
      {==}

law voice_has_snoc:
  for ds: List<&2, String>
  for +x: String
  for +k: String
  for acc: Bool
  for e: {String.eq(String.to_lower(x), k) == True{} : Bool}
  {VO.Voice.dict.has.go(List.append(&2, String, ds, [x]), k, acc) == True{} : Bool}

def voice_has_snoc(ds, x, k, acc, e):
  match ds:
    case Nil{}:
      %Equal.sym(Bool, String.eq(String.to_lower(x), k), True{}, e) : {Bool.or(acc, _) == True{} : Bool}
      voice_or_true(acc)
    case h <> t:
      voice_has_snoc(t, x, k, Bool.or(acc, String.eq(String.to_lower(h), k)), e)

law voice_add_has_k:
  for b: Bool
  for +ds: List<&2, String>
  for +term: String
  for e: {VO.Voice.dict.has(ds, term) == b : Bool}
  for h: {String.is_empty(String.trim(term)) == False{} : Bool}
  {VO.Voice.dict.has(VO.Voice.dict.add.k(b, String.is_empty(String.trim(term)), ds, String.trim(term)), term) == True{} : Bool}

def voice_add_has_k(b, ds, term, e, h):
  match b:
    case True{}:
      e
    case False{}:
      %Equal.sym(Bool, String.is_empty(String.trim(term)), False{}, h) : {VO.Voice.dict.has(VO.Voice.dict.add.k(False{}, _, ds, String.trim(term)), term) == True{} : Bool}
      voice_has_snoc(ds, String.trim(term), VO.Voice.key(term), False{}, Laws.string_eq_refl(String.to_lower(String.trim(term))))

def Laws.voice_add_idem(ds, term, h):
  %Equal.sym(Bool, VO.Voice.dict.has(ds, term), True{}, h) : {VO.Voice.dict.add.k(_, String.is_empty(String.trim(term)), ds, String.trim(term)) == ds : List<&2, String>}
  {==}

def Laws.voice_add_has(ds, term, h):
  voice_add_has_k(VO.Voice.dict.has(ds, term), ds, term, {==}, h)

def Laws.voice_fix_same(t, w):
  match w:
    case SNil{}:
      {==}
    case SCon{+h, +tl}:
      %Equal.sym(Bool, String.eq(SCon{h, tl}, SCon{h, tl}), True{}, Laws.string_eq_refl(SCon{h, tl})) : {VO.Voice.fix.k(_, t, SCon{h, tl}, SCon{h, tl}) == t : String}
      {==}

def Laws.voice_fix_empty(t, r):
  {==}

def Laws.voice_unsure_bounded(ds, ig, text):
  voice_cap_le(VO.Unsure, 5n, VO.Voice.un.go(VO.Voice.words(text), VO.Voice.dts(ds), ds, ig, VO.Us{False{}, Nil{}, 0n}))

def Laws.voice_keywords_bounded(ds):
  voice_cap_le(String, 100n, ds)

def Laws.voice_put_blank(firm, d, at):
  {==}

# Each bot once
# -------------
# Once.go keeps a key only when it is not among those seen, and adds it:
# so what it keeps is fresh against what it had seen and has no key twice;
# and a list with no key twice, fresh against nothing, comes back whole.

# fresh against k <> s means nothing in R equals k
law once_fresh_dup.k:
  for b: Bool
  for c: Bool
  for t: List<&2, String>
  for +s: List<&2, String>
  for +k: String
  for h: {Bool.and(Bool.not(Bool.or(b, c)), ON.Once.fresh(t, k <> s)) == True{} : Bool}
  for ih: {ON.Once.fresh(t, k <> s) == True{} : Bool} -> {ON.Once.dup(t, k) == False{} : Bool}
  {Bool.or(b, ON.Once.dup(t, k)) == False{} : Bool}

def once_fresh_dup.k(b, c, t, s, k, h, ih):
  match b c:
    case True{} _:
      Empty.absurd({Bool.or(True{}, ON.Once.dup(t, k)) == False{} : Bool}, false_ne_true(h))
    case False{} True{}:
      Empty.absurd({Bool.or(False{}, ON.Once.dup(t, k)) == False{} : Bool}, false_ne_true(h))
    case False{} False{}:
      ih(h)

law once_fresh_dup:
  for r: List<&2, String>
  for +s: List<&2, String>
  for +k: String
  for h: {ON.Once.fresh(r, k <> s) == True{} : Bool}
  {ON.Once.dup(r, k) == False{} : Bool}

def once_fresh_dup(r, s, k, h):
  match r:
    case Nil{}:
      {==}
    case +y <> +t:
      once_fresh_dup.k(String.eq(y, k), ON.Once.in(s, y), t, s, k, h, x => once_fresh_dup(t, s, k, x))

# fresh against k <> s is fresh against s
law once_fresh_tail.k:
  for b: Bool
  for c: Bool
  for t: List<&2, String>
  for +s: List<&2, String>
  for +k: String
  for h: {Bool.and(Bool.not(Bool.or(b, c)), ON.Once.fresh(t, k <> s)) == True{} : Bool}
  for ih: {ON.Once.fresh(t, k <> s) == True{} : Bool} -> {ON.Once.fresh(t, s) == True{} : Bool}
  {Bool.and(Bool.not(c), ON.Once.fresh(t, s)) == True{} : Bool}

def once_fresh_tail.k(b, c, t, s, k, h, ih):
  match b c:
    case True{} _:
      Empty.absurd({Bool.and(Bool.not(c), ON.Once.fresh(t, s)) == True{} : Bool}, false_ne_true(h))
    case False{} True{}:
      Empty.absurd({Bool.and(Bool.not(True{}), ON.Once.fresh(t, s)) == True{} : Bool}, false_ne_true(h))
    case False{} False{}:
      ih(h)

law once_fresh_tail:
  for r: List<&2, String>
  for +s: List<&2, String>
  for +k: String
  for h: {ON.Once.fresh(r, k <> s) == True{} : Bool}
  {ON.Once.fresh(r, s) == True{} : Bool}

def once_fresh_tail(r, s, k, h):
  match r:
    case Nil{}:
      {==}
    case +y <> +t:
      once_fresh_tail.k(String.eq(y, k), ON.Once.in(s, y), t, s, k, h, x => once_fresh_tail(t, s, k, x))

# fresh against s, and without k, is fresh against k <> s
law once_fresh_add.k:
  for b: Bool
  for c: Bool
  for t: List<&2, String>
  for +s: List<&2, String>
  for +k: String
  for h1: {Bool.and(Bool.not(c), ON.Once.fresh(t, s)) == True{} : Bool}
  for h2: {Bool.or(b, ON.Once.dup(t, k)) == False{} : Bool}
  for ih: {ON.Once.fresh(t, s) == True{} : Bool} -> {ON.Once.dup(t, k) == False{} : Bool} -> {ON.Once.fresh(t, k <> s) == True{} : Bool}
  {Bool.and(Bool.not(Bool.or(b, c)), ON.Once.fresh(t, k <> s)) == True{} : Bool}

def once_fresh_add.k(b, c, t, s, k, h1, h2, ih):
  match b c:
    case _ True{}:
      Empty.absurd({Bool.and(Bool.not(Bool.or(b, True{})), ON.Once.fresh(t, k <> s)) == True{} : Bool}, false_ne_true(h1))
    case True{} False{}:
      Empty.absurd({Bool.and(Bool.not(Bool.or(True{}, False{})), ON.Once.fresh(t, k <> s)) == True{} : Bool}, false_ne_true(Equal.sym(Bool, True{}, False{}, h2)))
    case False{} False{}:
      ih(h1)(h2)

law once_fresh_add:
  for r: List<&2, String>
  for +s: List<&2, String>
  for +k: String
  for h1: {ON.Once.fresh(r, s) == True{} : Bool}
  for h2: {ON.Once.dup(r, k) == False{} : Bool}
  {ON.Once.fresh(r, k <> s) == True{} : Bool}

def once_fresh_add(r, s, k, h1, h2):
  match r:
    case Nil{}:
      {==}
    case +y <> +t:
      once_fresh_add.k(String.eq(y, k), ON.Once.in(s, y), t, s, k, h1, h2, x => z => once_fresh_add(t, s, k, x, z))

# what Once.go keeps is fresh against what it had seen
law once_go_fresh.k:
  for -A: Data
  for c: Bool
  for +k: String
  for v: A
  for r: List<&2, ON.Ky<A>>
  for +seen: List<&2, String>
  for hc: {ON.Once.in(seen, k) == c : Bool}
  for ih: {ON.Once.fresh(ON.Once.keys(A, r), k <> seen) == True{} : Bool}
  {ON.Once.fresh(ON.Once.keys(A, ON.Once.step(A, c, k, v, r)), seen) == True{} : Bool}

def once_go_fresh.k(A, c, k, v, r, seen, hc, ih):
  match c:
    case True{}:
      once_fresh_tail(ON.Once.keys(A, r), seen, k, ih)
    case False{}:
      %Equal.sym(Bool, ON.Once.in(seen, k), False{}, hc) : {Bool.and(Bool.not(_), ON.Once.fresh(ON.Once.keys(A, r), seen)) == True{} : Bool}
      once_fresh_tail(ON.Once.keys(A, r), seen, k, ih)

law once_go_fresh:
  for -A: Data
  for xs: List<&2, ON.Ky<A>>
  for +seen: List<&2, String>
  {ON.Once.fresh(ON.Once.keys(A, ON.Once.go(A, xs, seen)), seen) == True{} : Bool}

def once_go_fresh(A, xs, seen):
  match xs:
    case Nil{}:
      {==}
    case ON.Ky{+k, +v} <> +t:
      once_go_fresh.k(A, ON.Once.in(seen, k), k, v, ON.Once.go(A, t, k <> seen), seen, {==}, once_go_fresh(A, t, k <> seen))

# what Once.go keeps has no key twice
law once_go_distinct.k:
  for -A: Data
  for c: Bool
  for +k: String
  for v: A
  for +r: List<&2, ON.Ky<A>>
  for +seen: List<&2, String>
  for ihd: {ON.Once.distinct(ON.Once.keys(A, r)) == True{} : Bool}
  for ihf: {ON.Once.fresh(ON.Once.keys(A, r), k <> seen) == True{} : Bool}
  {ON.Once.distinct(ON.Once.keys(A, ON.Once.step(A, c, k, v, r))) == True{} : Bool}

def once_go_distinct.k(A, c, k, v, r, seen, ihd, ihf):
  match c:
    case True{}:
      ihd
    case False{}:
      %Equal.sym(Bool, ON.Once.dup(ON.Once.keys(A, r), k), False{}, once_fresh_dup(ON.Once.keys(A, r), seen, k, ihf))
        : {Bool.and(Bool.not(_), ON.Once.distinct(ON.Once.keys(A, r))) == True{} : Bool}
      ihd

law once_go_distinct:
  for -A: Data
  for xs: List<&2, ON.Ky<A>>
  for +seen: List<&2, String>
  {ON.Once.distinct(ON.Once.keys(A, ON.Once.go(A, xs, seen))) == True{} : Bool}

def once_go_distinct(A, xs, seen):
  match xs:
    case Nil{}:
      {==}
    case ON.Ky{+k, +v} <> +t:
      once_go_distinct.k(A, ON.Once.in(seen, k), k, v, ON.Once.go(A, t, k <> seen), seen, once_go_distinct(A, t, k <> seen), once_go_fresh(A, t, k <> seen))

def Laws.once_distinct(A, xs):
  once_go_distinct(A, xs, Nil{})

# a list whose keys are distinct and unseen comes back whole
law once_go_keeps.k:
  for -A: Data
  for c: Bool
  for d: Bool
  for +k: String
  for +v: A
  for +t: List<&2, ON.Ky<A>>
  for +seen: List<&2, String>
  for hd: {ON.Once.dup(ON.Once.keys(A, t), k) == d : Bool}
  for hf: {Bool.and(Bool.not(c), ON.Once.fresh(ON.Once.keys(A, t), seen)) == True{} : Bool}
  for hx: {Bool.and(Bool.not(d), ON.Once.distinct(ON.Once.keys(A, t))) == True{} : Bool}
  for ih: {ON.Once.fresh(ON.Once.keys(A, t), k <> seen) == True{} : Bool} -> {ON.Once.distinct(ON.Once.keys(A, t)) == True{} : Bool} -> {ON.Once.go(A, t, k <> seen) == t : List<&2, ON.Ky<A>>}
  {ON.Once.step(A, c, k, v, ON.Once.go(A, t, k <> seen)) == ON.Ky{k, v} <> t : List<&2, ON.Ky<A>>}

def once_go_keeps.k(A, c, d, k, v, t, seen, hd, hf, hx, ih):
  match c d:
    case True{} _:
      Empty.absurd({ON.Once.step(A, True{}, k, v, ON.Once.go(A, t, k <> seen)) == ON.Ky{k, v} <> t : List<&2, ON.Ky<A>>}, false_ne_true(hf))
    case False{} True{}:
      Empty.absurd({ON.Once.step(A, False{}, k, v, ON.Once.go(A, t, k <> seen)) == ON.Ky{k, v} <> t : List<&2, ON.Ky<A>>}, false_ne_true(hx))
    case False{} False{}:
      Equal.cong(List<&2, ON.Ky<A>>, List<&2, ON.Ky<A>>, x => ON.Ky{k, v} <> x, ON.Once.go(A, t, k <> seen), t,
        ih(once_fresh_add(ON.Once.keys(A, t), seen, k, hf, hd))(hx))

law once_go_keeps:
  for -A: Data
  for xs: List<&2, ON.Ky<A>>
  for +seen: List<&2, String>
  for hf: {ON.Once.fresh(ON.Once.keys(A, xs), seen) == True{} : Bool}
  for hx: {ON.Once.distinct(ON.Once.keys(A, xs)) == True{} : Bool}
  {ON.Once.go(A, xs, seen) == xs : List<&2, ON.Ky<A>>}

def once_go_keeps(A, xs, seen, hf, hx):
  match xs:
    case Nil{}:
      {==}
    case ON.Ky{+k, +v} <> +t:
      once_go_keeps.k(A, ON.Once.in(seen, k), ON.Once.dup(ON.Once.keys(A, t), k), k, v, t, seen, {==}, hf, hx, x => z => once_go_keeps(A, t, k <> seen, x, z))

law once_fresh_nil:
  for r: List<&2, String>
  {ON.Once.fresh(r, Nil{}) == True{} : Bool}

def once_fresh_nil(r):
  match r:
    case Nil{}:
      {==}
    case +y <> +t:
      once_fresh_nil(t)

def Laws.once_keeps(A, xs, hx):
  once_go_keeps(A, xs, Nil{}, once_fresh_nil(ON.Once.keys(A, xs)), hx)

def Laws.bot_rows_once(u):
  Laws.once_distinct(BU.BRow, BU.BU.rows.all(u))

def Laws.bot_rows_shown(u):
  {==}

def Laws.bot_rows_keep(u, hx):
  Laws.once_keeps(BU.BRow, BU.BU.rows.all(u), hx)

def Laws.bot_picks_once(u):
  Laws.once_distinct(BU.Pick, BU.BU.picks.src(u))

def Laws.bot_picks_shown(u):
  {==}

def Laws.hubs_bots_once(ss, hs, many):
  Laws.once_distinct(J.Json, Hb.Hubs.bots.src(ss, hs, many))

def Laws.hubs_bots_shown(ss, hs, many):
  {==}

def Laws.reply_peer_person():
  {==}

def Laws.reply_peer_bot():
  {==}

def Laws.reply_peer_room():
  {==}

def Laws.reply_said_newest():
  {==}

def Laws.reply_person_wakes_none(s, now, from, text):
  {==}

def Laws.far_machine_magicdns():
  {==}

def Laws.far_machine_self():
  {==}

def Laws.far_machine_other_port():
  {==}

def Laws.far_machine_stranger():
  {==}

def Laws.tailnet_domain():
  {==}

def Laws.bot_newproj_asks(th):
  {==}

def Laws.bot_newproj_once():
  {==}

def Laws.bot_newproj_decline():
  {==}

def Laws.bot_newproj_no_answer():
  {==}

def Laws.refs_plain_offers_nothing(s, sel, now):
  {==}

def Laws.refs_plain_not_typing():
  {==}

def Laws.refs_far_typing():
  {==}

def Laws.refs_agent_plain(s, sel):
  {==}

def Laws.refs_own_bot_never(named, id, name):
  match named:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.bot_mention_once(s, now, p):
  {==}

def Laws.sum_never_made_due(now):
  {==}

def Laws.sum_not_due_asks_nothing(s, id, kind):
  {==}

def Laws.sum_not_due_stamps_nothing(id, now):
  {==}

def Laws.bg_start_progress_end():
  {==}

def Laws.bg_added_once():
  {==}

def Laws.bg_progress_unlisted():
  {==}

def Laws.bg_doing():
  {==}

def Laws.status_waits_on_subagents():
  {==}

def Laws.status_starting_works():
  {==}

def Laws.status_work_before_waiting(q):
  {==}

def Laws.status_input_before_waiting(t, q):
  {==}

def Laws.status_no_subagents_same(s):
  {==}

def Laws.shows_other_delta(u, th, text, h):
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      %Equal.sym(Bool, String.eq(th, sel), False{}, h) : {_ == False{} : Bool}
      {==}

def Laws.shows_own_delta(u, text):
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      Laws.string_eq_refl(sel)

def Laws.hubs_delta_elsewhere(hs, key, o, h):
  %Equal.sym(Bool, String.eq(key, Hb.Hubs.cur(hs)), False{}, h) : {Hb.Hubs.shows.k(_, hs, key, o) == False{} : Bool}
  {==}

def Laws.hubs_delta_other_thread(hs, th, text, h):
  %Equal.sym(Bool, String.eq(Hb.Hubs.cur(hs), Hb.Hubs.cur(hs)), True{}, Laws.string_eq_refl(Hb.Hubs.cur(hs))) : {Hb.Hubs.shows.k(_, hs, Hb.Hubs.cur(hs), J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{th}}, J.KV{"text", J.Str{text}}])) == False{} : Bool}
  Laws.shows_other_delta(Hb.Hubs.ui(hs, Hb.Hubs.cur(hs)), th, text, h)

def Laws.hubs_redraw_changes_elsewhere(hs, key, items):
  {==}

def Laws.hubs_redraw_delta_elsewhere(hs, key, th, text, h):
  Laws.hubs_delta_elsewhere(hs, key, J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{th}}, J.KV{"text", J.Str{text}}]), h)

def Laws.hubs_redraw_delta_other_thread(hs, th, text, h):
  Laws.hubs_delta_other_thread(hs, th, text, h)

def Laws.alerts_none_for_delta(u, th, text):
  {==}

def Laws.island_doing_mine_same():
  {==}

def Laws.island_doing_mine_text():
  {==}

def Laws.hubs_screen_unfocused_thread_null():
  {==}

def Laws.shows_changes(u, items):
  {==}

def Laws.idx_page_own():
  {==}

def Laws.idx_page_sent():
  {==}

def Laws.idx_sent_filed():
  {==}

def Laws.idx_page_short():
  {==}

def Laws.idx_links():
  {==}

def Laws.idx_links_found():
  {==}

def Laws.plot_tint_own_colour():
  {==}

def Laws.tint_code_again():
  {==}

def Laws.select_leaves_settings():
  {==}

def Laws.room_leaves_settings():
  {==}

def Laws.hits_chat_line_slides(x0):
  {==}

def Laws.chat_notch_tile():
  {==}

def Laws.set_scroll_48():
  {==}

def Laws.bots_still_cat_posed_once(c):
  {==}

def Laws.bots_away_cat_posed_once(c):
  {==}

def Laws.bots_moving_cat_follows_clock(c):
  {==}

def Laws.bots_cats_calm_on_input(ms):
  %Equal.sym(Nat, Nat.sub(ms, ms), 0n, sub_self(ms)) : {Nat.sub(1000n, _) == 1000n : Nat}
  {==}

def Laws.far_id_parts():
  {==}

def Laws.far_id_here():
  {==}

def Laws.far_change_names_link(l, th, msg, role, text, at):
  {==}

def Laws.far_sent_names_sender():
  {==}

def Laws.far_change_bot_named(l, id, name, th, look, persona, at):
  {==}

def Laws.far_settings_stay_home(s, k, v, at):
  {==}

def Laws.far_peers_stay_home(s, i, n, u, r, at):
  {==}

def Laws.far_hooks_stay_home(s, i, b, n, r, at):
  {==}

def Laws.far_devices_stay_home(s, tk, pl, k, th, env, bu, at):
  {==}

def Laws.far_rooms_stay_home(s, i, r, f, t, h, at):
  {==}

def Laws.far_gap_takes_nothing(xs):
  {==}

def Laws.far_old_takes_nothing(a, b):
  {==}

def Laws.far_overlap_takes_new(a, b, c):
  {==}

def Laws.far_batch_again(a, b):
  {==}

def Laws.far_route_here_stays():
  {==}

def Laws.far_route_goes_home():
  {==}

def Laws.far_allowed_only_methods(s, p):
  {==}

def Laws.far_no_delete(s, p):
  {==}

def Laws.far_where_here(n):
  {==}

def Laws.far_where_far():
  {==}

def Laws.far_bot_tabs():
  {==}

def Laws.far_bot_tabs_here():
  {==}

def Laws.far_row_act_mirror(i, n, lk, m, no, se, pe, l):
  {==}

def Laws.far_row_act_reported(i, n, lk, m, no, se, pe, l):
  {==}

def Laws.bot_row_title_far(i, th, lk, m, no, se, l):
  {==}

def Laws.bot_row_title_here(i, n, th, lk, m, no, se, pe, l):
  {==}

def Laws.bot_head_remote(u, id, t, lk, m):
  {==}

def Laws.bot_head_remote_view(u, id):
  {==}

def Laws.far_bot_head_tabs(u, t, lk, m):
  {==}

def Laws.far_bot_head_stays_home(u, t, lk, m):
  {==}

def Laws.bot_head_here_owns(u, t, lk, m):
  {==}

def Laws.hubs_bots_mirror_dropped(far, want, hs, from, r, rest):
  {==}

# Text fitting: early exit preserves the original recurrence.

def Laws.fit_count_guard(s, fits, cache, f, max, used):
  match s fits:
    case SNil{} False{}:
      {==}
    case SCon{Chr{cp}, t} False{}:
      {==}
    case SNil{} True{}:
      {==}
    case SCon{Chr{cp}, t} True{}:
      {==}

def Laws.fit_count_step(cp, s, cache, f, max, used):
  Laws.fit_count_guard(s, U32.is_le((used + X.TG.adv(X.Text.get(cache, f, cp)) : U32), max),
    cache, f, max, (used + X.TG.adv(X.Text.get(cache, f, cp)) : U32))

def Laws.fit_chunk_progress(n):
  match n:
    case 0n:
      (0n, {==})
    case 1n+k:
      (k, {==})

def Laws.fit_wrap_unicode():
  {==}

# Per-peer remote bot directories and bounded refreshes
def Laws.remote_dir_failed_peer(xs, p):
  {==}

def Laws.remote_dir_away_seen(p, n, look, seen):
  {==}

def Laws.remote_dir_arrival_order():
  {==}

def Laws.remote_dir_other_unchanged(o):
  {==}

def Laws.remote_dir_obsolete_ignored(xs, p, r):
  {==}

def Laws.remote_dir_known_not_queued(a, q, p):
  {==}

def Laws.remote_dir_four_slots():
  {==}

def Laws.remote_dir_freed_slot():
  {==}

def Laws.want_other_hub(u, generation, installed):
  match generation installed:
    case True{} True{}: {==}
    case True{} False{}: {==}
    case False{} True{}: {==}
    case False{} False{}: {==}

def Laws.want_old_generation(u, hub, installed):
  match hub installed:
    case True{} True{}: {==}
    case True{} False{}: {==}
    case False{} True{}: {==}
    case False{} False{}: {==}

def Laws.want_before_install(u, hub, generation):
  match hub generation:
    case True{} True{}: {==}
    case True{} False{}: {==}
    case False{} True{}: {==}
    case False{} False{}: {==}

def Laws.machine_clears_selection(u, hub):
  match u:
    case U.Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}: {==}

def Laws.timeline_hidden_settings(selected, thread):
  match selected thread:
    case True{} True{}: {==}
    case True{} False{}: {==}
    case False{} True{}: {==}
    case False{} False{}: {==}

def Laws.timeline_hidden_pane(settings, selected):
  match settings selected:
    case True{} True{}: {==}
    case True{} False{}: {==}
    case False{} True{}: {==}
    case False{} False{}: {==}

def home_short_step(home: Bool, +v: U.HubView, +rest: List<&2, U.HubView>, p: {List.drop(&2, U.HubView, rest, 1n) == Nil{} : List<&2, U.HubView>}) -> {List.drop(&2, U.HubView, U.HubCache.home.k(home, v, rest), 1n) == Nil{} : List<&2, U.HubView>}:
  match home:
    case True{}: {==}
    case False{}: p

def Laws.hub_cache_home_short(vs):
  match vs:
    case Nil{}: {==}
    case +v <> +t:
      home_short_step(String.is_empty(U.HubView.hub(v)), v, U.HubCache.home(t), Laws.hub_cache_home_short(t))

def remote_short_step(home: Bool, +v: U.HubView, +rest: List<&2, U.HubView>, p: {List.drop(&2, U.HubView, rest, 1n) == Nil{} : List<&2, U.HubView>}) -> {List.drop(&2, U.HubView, U.HubCache.remote.k(home, v, rest), 1n) == Nil{} : List<&2, U.HubView>}:
  match home:
    case True{}: p
    case False{}: {==}

def Laws.hub_cache_remote_short(vs):
  match vs:
    case Nil{}: {==}
    case +v <> +t:
      remote_short_step(String.is_empty(U.HubView.hub(v)), v, U.HubCache.remote(t), Laws.hub_cache_remote_short(t))

def cache_bounded_step(home: Bool, +v: U.HubView, +vs: List<&2, U.HubView>) -> {List.drop(&2, U.HubView, U.HubCache.put.k(home, v, vs), 2n) == Nil{} : List<&2, U.HubView>}:
  match home:
    case True{}: Laws.hub_cache_remote_short(vs)
    case False{}: Laws.hub_cache_home_short(vs)

def Laws.hub_cache_bounded(v, vs):
  cache_bounded_step(String.is_empty(U.HubView.hub(v)), v, vs)

def Laws.far_project_named(l, id, title, root, at):
  {==}

def Laws.far_project_shared(s):
  {==}

def Laws.far_project_thread_shared():
  {==}

def Laws.far_label_here(s, id, title):
  {==}

def Laws.far_label_away():
  {==}

def Laws.far_cursor_keeps_marker():
  {==}

def Laws.far_routine_routes_home():
  {==}

def Laws.hubs_catalog_mirror_dropped(r, rest):
  {==}

def Laws.hubs_sidebar_view_everywhere():
  {==}

def Laws.far_epoch_legacy(old):
  {==}

def Laws.far_epoch_same():
  {==}

def Laws.far_epoch_reset():
  {==}

def Laws.far_reset_keeps_sequence(s, l):
  match s:
    case M.State{_, _, _, _, _, _, _, _, _, _, _}:
      {==}

def Laws.far_loaded_live_wins():
  {==}

def Laws.far_reply_preserves_error():
  {==}

def Laws.far_draft_kept(text):
  {==}

def Laws.hubs_fold_null(older, hs, from):
  {==}

def Laws.hubs_origin_here(p, name):
  {==}

def Laws.hubs_origin_elsewhere():
  {==}

def Laws.far_arrival_no_choice(u):
  {==}

def Laws.far_arrival_not_shared(u):
  {==}

def Laws.far_arrival_thread_selected(u, id, name, thread):
  match u:
    case U.Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      {==}

def Laws.far_arrival_focus_unchanged(n):
  {==}

def Laws.hubs_origin_label_plain(v, name, online):
  {==}

def Laws.hubs_origin_title_here(title, name):
  {==}

def Laws.hubs_origin_title_away(title, name):
  {==}
def Laws.bg_start_keeps_call():
  {==}

def Laws.bg_call_title():
  {==}

def Laws.subs_agent_at_work():
  {==}

def Laws.subs_agent_done_shut():
  {==}

def Laws.subs_shell_named():
  {==}

def Laws.subs_shell_not_agent():
  {==}

def Laws.subs_badge():
  {==}

def Laws.desk_phone_never(thread, now):
  {==}

def Laws.desk_unfocused_never(d, th, at, thread, now):
  match d:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.desk_look_fresh():
  {==}

def Laws.desk_look_expires():
  {==}

def Laws.desk_seen_holds(key):
  {==}

def Laws.desk_seen_nobody(xs):
  {==}

def Laws.desk_none_nobody():
  {==}

def Laws.alert_watched_quiet(kind, id, title, st, es, rest):
  {==}

def Laws.desk_due_first(last_at, thread, focused, now):
  {==}

# Design history
# --------------

def Laws.hist_mark_self(xs):
  match xs:
    case Nil{}:
      {==}
    case B.Item{+k, +s, +l, +p, +i} <> +t:
      %Equal.sym((String & String) & Cmp, String.cmp(k, k), ((k, k), EQ{}), string_cmp_refl(k))
        : {HS.Mark.go(B.Item{k, s, l, p, i} <> t, B.Item{k, s, l, p, i} <> t, HS.Mark.key.cmp.fin(_)) == HS.Mark.dims(B.Item{k, s, l, p, i} <> t) : List<&2, B.Item>}
      %Equal.sym(Bool, U32.is_eq(s, s), True{}, u32_eq_refl(s))
        : {HS.Mark.same.k(_, B.Item{k, s, l, p, i}, B.Item{k, s, l, p, i}, HS.Mark.go(t, t, HS.Mark.heads(t, t))) == HS.Mark.dims(B.Item{k, s, l, p, i} <> t) : List<&2, B.Item>}
      Equal.cong(List<&2, B.Item>, List<&2, B.Item>, ys => HS.Mark.dim(B.Item{k, s, l, p, i}) <> ys,
        HS.Mark.go(t, t, HS.Mark.heads(t, t)), HS.Mark.dims(t), Laws.hist_mark_self(t))

def Laws.hist_version_family():
  {==}

def Laws.hist_compare_family():
  {==}

def Laws.hist_obj_folds():
  {==}

def Laws.hist_sheet_same_commit():
  {==}

def Laws.hist_fwd_last_live(n):
  %Equal.sym(Bool, Nat.is_ge(n, n), True{}, Nat.ge_refl(n)) : {Bool.pick(Nat, Bool.or(Nat.is_eq(n, 0n), _), 0n, 1n+n) == 0n : Nat}
  %Equal.sym(Bool, Bool.or(Nat.is_eq(n, 0n), True{}), True{}, or_true_r(Nat.is_eq(n, 0n))) : {Bool.pick(Nat, _, 0n, 1n+n) == 0n : Nat}
  {==}

def Laws.hist_back_live_last(n):
  {==}

def Laws.hist_index_live(ss):
  {==}

def Laws.hist_record_nothing(e):
  {==}

def Laws.look_plain_board(layer):
  {==}

def Laws.look_plain_sch(layer):
  {==}

def Laws.look_marks_distinct():
  {==}

def Laws.look_names_marks():
  {==}

def Laws.scratch_pinned_kept(now, w, id, ti, c, a, tu, pe, ar, o, sa, ua, env, br, wt, pr, mo, rt, im, se, de):
  {==}

def Laws.scratch_running_kept(now, w, id, ti, c, a, pe, ar, o, sa, ua, pin, env, br, wt, pr, mo, rt, im, se, de):
  match pin:
    case True{}:
      {==}
    case False{}:
      match de:
        case True{}:
          {==}
        case False{}:
          {==}

def Laws.scratch_project_kept(now, w, id, ti, c, a, tu, pe, ar, o, sa, ua, pin, env, br, wt, pr, mo, rt, im, se, de):
  {==}

def Laws.scratch_expires():
  {==}

def Laws.scratch_not_yet():
  {==}

def Laws.scratch_days_default():
  {==}

def Laws.scratch_days_most():
  {==}

def Laws.scratch_made():
  {==}

def Laws.scratch_removed_stays():
  {==}

def Laws.scratch_always_live(now):
  {==}

def Laws.give_command():
  {==}

def Laws.give_not_a_word():
  {==}

def Laws.give_not_btw():
  {==}

def Laws.give_nothing_refused(c, id, os):
  {==}

def Laws.keys_name_env():
  {==}

def Laws.keys_name_digit():
  {==}

def Laws.keys_quote_single():
  {==}

def Laws.keys_set_replaces():
  {==}

def Laws.keys_drop_keeps_others():
  {==}

def Laws.keys_names_of_lines():
  {==}

def Laws.keys_prompt_none():
  {==}

def Laws.md_head_first(cs):
  {==}

def Laws.md_head_only_first(cs, r, rs):
  {==}

def Laws.md_fence_any(s):
  match s:
    case SNil{}:
      {==}
    case SCon{h, t}:
      {==}

def Laws.fm_kind_board():
  {==}

def Laws.fm_kind_schematic():
  {==}

def Laws.fm_kind_model():
  {==}

def Laws.fm_kind_pdf():
  {==}

def Laws.fm_kind_image():
  {==}

def Laws.fm_kind_other():
  {==}

def Laws.fm_safe_up():
  {==}

def Laws.fm_safe_abs():
  {==}

def Laws.fm_safe_down():
  {==}

def Laws.fm_parent_top():
  {==}

def Laws.fm_parent_down():
  {==}

def Laws.fm_join_root():
  {==}

def Laws.emb_summary_round():
  {==}
def Laws.emb_size_round():
  {==}
def Laws.emb_pinned_live(touched, last, now):
  {==}
def Laws.emb_message_ends():
  {==}
def Laws.emb_fresh_live():
  {==}
def Laws.emb_idle_ends():
  {==}
def Laws.emb_note_none():
  {==}
def Laws.emb_focus_round():
  {==}
def Laws.cmds_foreign_result_quiet():
  {==}

def Laws.cmds_foreign_result_keeps_running():
  {==}

def Laws.cmds_result_waits_lc():
  {==}

def Laws.cmds_lc_completed_ends():
  {==}

def Laws.cmds_lc_end_completes():
  {==}

def Laws.cmds_pair_stays_running():
  {==}

def Laws.cmds_tomb_error_quiet():
  {==}

def Laws.cmds_tomb_cancel_quiet():
  {==}

def Laws.cmds_stop_then_new_error_quiet():
  {==}

def Laws.cmds_stop_then_new():
  {==}

def Laws.cmds_wake_start():
  {==}

def Laws.cmds_wake_runs():
  {==}

def Laws.cmds_wake_result_ends():
  {==}

def Laws.cmds_wake_with_queued_user():
  {==}

def Laws.cmds_tomb_started_resumes():
  {==}

def Laws.cmds_tomb_started_says():
  {==}

def Laws.cmds_prep_counts():
  {==}

def Laws.cmds_mode_version():
  {==}

def Laws.cmds_mode_old():
  {==}

def Laws.cmds_mode_old_legacy():
  {==}

def Laws.cmds_mode_lc():
  {==}

def Laws.cmds_legacy_retires_sent():
  {==}

def Laws.cmds_legacy_learned_retires():
  {==}

def Laws.cmds_legacy_prep_stays():
  {==}

def Laws.cmds_lifecycle_keeps_sent():
  {==}

def Laws.cmds_legacy_same(s, now, thread, line, text):
  {==}

def Laws.wake_act_once():
  {==}

def Laws.bg_sync_ends_absent():
  {==}

def Laws.bg_sync_never_adds(ls, open):
  match ls:
    case Nil{}:
      {==}
    case l <> t:
      %Laws.bg_sync_never_adds(t, open) : {1n+List.length(&2, String, M.Bg.sync(t, open)) == 1n+_ : Nat}
      {==}

def Laws.bg_updated_ends_not_drops():
  {==}

def Laws.bg_notification_drops():
  {==}

def Laws.bg_empty_snapshot_keeps_pending():
  {==}

def Laws.bg_turn_end_settles_ended():
  {==}

def Laws.workset_fold_same_as_setting(s, th, kind, rows, at):
  {==}

def Laws.store_workset_encoded():
  {==}

def Laws.far_workset_shared():
  {==}

def Laws.far_workset_mapped(l, th, kind, rows, at):
  {==}

def Laws.far_forget_drops_work():
  {==}

def Laws.bg_progress_coalesced():
  {==}

def Laws.bg_progress_one_held_per_task():
  {==}

def Laws.bg_progress_beat_waits():
  {==}

def Laws.bg_progress_latest_lands():
  {==}

def Laws.bg_progress_is_own_line():
  {==}

def Laws.far_forget_equiv():
  {==}

def Laws.far_forget_equiv_none():
  {==}

def Laws.prompt_background_work():
  {==}

def Laws.work_busy_counts_ended_rows():
  {==}

def Laws.work_ready_idle():
  {==}

def Laws.work_not_ready_busy():
  {==}

def Laws.notice_decide_w_idle_same(f, b, a, p, fi, m):
  {==}

def Laws.notice_busy_quiet(n, b, p, fi, m):
  {==}

def Laws.notice_busy_fail_alerts(busy, b, a, p, fi, m):
  match busy:
    case 0n:
      {==}
    case 1n+k:
      {==}

def Laws.notice_busy_ask_kept(busy, p, fi, m):
  match busy:
    case 0n:
      {==}
    case 1n+k:
      {==}

def Laws.wait_busy_not_done():
  {==}

def Laws.wait_idle_done():
  {==}

def Laws.settle_waits_for_work():
  {==}

def Laws.settle_without_work():
  {==}

def Laws.scratch_busy_kept(now, w, t, n):
  and_false_r(M.Scratch.expired(now, w, t))

def Laws.worktree_busy_kept(s, t, now, window, n):
  and_false_r(M.Worktree.drop(s, t, now, window))

def Laws.island_busy_shows():
  {==}

def Laws.island_gone_hidden():
  {==}

def Laws.island_unheard_empty(ts, st):
  {==}

def Laws.island_heard_online(now, heard, fresh):
  {==}

def Laws.island_unheard_late():
  {==}

def Laws.island_unheard_recent():
  {==}

def Laws.island_lines_capped():
  {==}

def Laws.upload_thread_local():
  {==}

def Laws.upload_thread_plain():
  {==}

def Laws.task_waits_for_child_work():
  {==}

def Laws.task_ends_when_ready():
  {==}

def Laws.task_ended_last_change():
  {==}

def Laws.task_failed_ends_even_busy():
  {==}

def Laws.bot_reply_captured_once():
  {==}

def Laws.bot_reply_waits_for_work():
  {==}

def Laws.phase_failed_before_monitoring(n, q):
  {==}

def Laws.status_wait_f_keeps_failed(any):
  {==}

def Laws.commit_text_joins():
  {==}

def Laws.task_finish_delivered_last():
  {==}

def Laws.task_repair_delivers():
  {==}

def Laws.task_repair_delivers_once():
  {==}

def Laws.task_repair_activates_idle():
  {==}

def Laws.task_repair_running_marks_only():
  {==}

def Laws.task_repair_quiet_when_done():
  {==}

def Laws.store_torn_tail_resyncs(rest):
  {==}

def Laws.store_torn_tail_newline():
  {==}

def Laws.store_fail_names_repair():
  {==}

def Laws.task_repair_finishes_ended_child():
  {==}

def Laws.task_repair_leaves_working_child():
  {==}

def Laws.recover_lost_told():
  {==}

def Laws.recover_lost_clears():
  {==}

def Laws.recover_no_work_quiet():
  {==}

def Laws.recover_lost_once():
  {==}

def Laws.lost_off_logs_and_keeps():
  {==}

def Laws.gone_idle_told():
  {==}

def Laws.gone_running_told():
  {==}

def Laws.replaced_told_once():
  {==}

def Laws.replaced_keeps_new_rows():
  {==}

def Laws.carry_keeps_child_task():
  {==}

def Laws.carry_off_fails_child_task():
  {==}

def Laws.carry_child_result_once():
  {==}

def Laws.rec_burst_bounded():
  {==}

def Laws.rec_release_bounded():
  {==}

def Laws.rec_release_carry_marker_on():
  {==}

def Laws.rec_release_carry_marker_off():
  {==}

def Laws.rec_release_takes_held():
  {==}

def Laws.update_wait_busy(now, c, id):
  {==}

def Laws.update_wait_idle(now, c, id):
  {==}

def Laws.update_wait_goes_when_done():
  {==}

def Laws.update_wait_holds_while_busy():
  {==}

def Laws.update_ready_holds_busy(now):
  {==}

def Laws.update_ready_idle_restarts(now):
  {==}

def Laws.update_ready_forced_restarts(now):
  {==}

def Laws.update_held_goes_when_done():
  {==}

def Laws.update_held_waits_while_busy():
  {==}

def Laws.update_wait_late_forces():
  {==}

def Laws.update_wait_early_not_forced():
  {==}

def Laws.lost_text_not_carry_text(ls):
  {==}

def Laws.kill_never_low_pid():
  {==}

def Laws.spawn_gen_accepts_current():
  {==}

def Laws.spawn_gen_drops_stale():
  {==}

def Laws.stale_spawn_fail_quiet(gen):
  {==}

def Laws.mcp_stale_gen_refused():
  {==}

def Laws.mcp_stale_gen_reads():
  {==}

def Laws.prep_stale_dropped():
  {==}

def Laws.gen_mcp_path():
  {==}

def Laws.quiet_once():
  {==}

def Laws.quiet_never_when_busy(heard, now, n):
  {==}

def Laws.quiet_never_idle(heard, now, said, busy):
  match said:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.exit_stale_gen_quiet():
  {==}

def Laws.final_partial_line_flushed():
  {==}

def Laws.watch_intent_before_launch():
  {==}

def Laws.watch_end_once():
  {==}

def Laws.watch_end_wakes_idle():
  {==}

def Laws.watch_end_delivers_running():
  {==}

def Laws.watch_timeout_unknown_not_success():
  {==}

def Laws.watch_pid_identity_unknown():
  {==}

def Laws.watch_cancel_keeps_until_confirmed():
  {==}

def Laws.watch_cancel_confirmed():
  {==}

def Laws.watch_end_retired_silent():
  {==}

def Laws.watch_retire_unwatches():
  {==}

def Laws.watch_target_roundtrip():
  {==}

def Laws.watch_target_one_line():
  {==}

def Laws.watch_target_old_row_as_written():
  {==}

def Laws.watch_unwatch_says_may_run():
  {==}

def Laws.recover_rearms_watches():
  {==}

def Laws.mirror_never_awaits():
  {==}

def Laws.prompt_background_work_b():
  {==}

def Laws.credit_drop_over_margin():
  {==}

def Laws.credit_stale_gen_quiet(n, b):
  {==}

def Laws.credit_drained_counts():
  {==}

def Laws.credit_drained_never_back():
  {==}

def Laws.boot_room_after_drain():
  {==}

def Laws.outbox_fifo():
  {==}

def Laws.join_chunks_cover():
  {==}

def Laws.join_chunks_flat():
  {==}

def Laws.join_oversized_alone():
  {==}

def Laws.items_one_frame():
  {==}

def Laws.items_split_by_bytes():
  {==}

def Laws.items_split_keeps_boot():
  {==}

def Laws.items_split_marks_more():
  {==}

def Laws.items_whole_not_marked():
  {==}

def Laws.store_hold_len():
  {==}

def Laws.store_hold_err():
  {==}

def Laws.store_trunc_args():
  {==}

def Laws.ui_chunked_join_equals_whole():
  {==}

def Laws.boot_on_until_last():
  {==}

def Laws.boot_no_alerts():
  {==}

def Laws.boot_done_alerts():
  {==}

def Laws.boot_catchup_no_alerts():
  {==}

def Laws.pong_rate_bounded():
  {==}

def Laws.wire_all_bin_eq(t):
  {==}

def Laws.credit_behind_catches():
  {==}

def Laws.join_window_same():
  {==}

def Laws.live_snapshot_replaces():
  {==}

def Laws.live_snapshot_idempotent():
  {==}

def Laws.live_snapshot_clears_absent():
  {==}

def Laws.live_end_clears():
  {==}

def Laws.live_turn_end_clears():
  {==}

def Laws.live_joined_exact():
  {==}

def Laws.live_old_gen_quiet():
  {==}

def Laws.live_trunc_marked():
  {==}

def Laws.want_new_ignores_other_threads():
  {==}

def Laws.made_selects_own():
  {==}

def Laws.made_selects_own_later():
  {==}

def Laws.replay_cannot_select():
  {==}

def Laws.made_stale_quiet():
  {==}

def Laws.made_fork_selects_own():
  {==}

def Laws.made_waiting_at_once():
  {==}

def Laws.made_waiting_ends_made():
  {==}

def Laws.made_waiting_ends_failed():
  {==}

def Laws.far_create_routes():
  {==}

def Laws.far_fork_routes():
  {==}

def Laws.far_create_ids_local():
  {==}

def Laws.far_machine_routes():
  {==}

def Laws.far_machine_none_here():
  {==}

def Laws.far_create_in_shared_project():
  {==}

def Laws.far_create_not_in_unshared_project():
  {==}

def Laws.far_create_not_in_a_mirror():
  {==}

def Laws.far_pin_only_shared_thread():
  {==}

def Laws.far_settings_not_remote():
  {==}

def Laws.far_reply_names_thread():
  {==}

def Laws.far_pin_shared():
  {==}

def Laws.made_far_waits_for_mirror():
  {==}

def Laws.made_far_selects_when_mirrored():
  {==}

def Laws.made_far_done_when_mirrored():
  {==}

def Laws.made_far_failed_not_waiting():
  {==}

def Laws.proj_machine_cycle_ends_here():
  {==}

def Laws.proj_machine_cycle_next():
  {==}

def Laws.proj_on_request_names_machine():
  {==}

def Laws.proj_here_request_unchanged(p):
  {==}

def Laws.side_rows_ix_eq():
  {==}

def Laws.side_phase_ix_eq():
  {==}

def Laws.subs_count_ix_eq():
  {==}

def Laws.active_rows_ix_eq():
  {==}

def Laws.settle_ix_eq():
  {==}

def Laws.alerts_ids_same():
  {==}

def Laws.alerts_ids_raise():
  {==}

def Laws.far_fail_keeps_items(a, b):
  {==}

def Laws.far_ack_removes_batch(a):
  {==}

def Laws.far_ack_keeps_waiting(a, b):
  {==}

def Laws.far_retry_order(a, b):
  {==}

def Laws.far_retry_begins(a, b):
  {==}

def Laws.far_hold_waits(a):
  {==}

def Laws.far_release_resends(a):
  {==}

def Laws.far_backoff():
  {==}

def Laws.far_epoch_on_unknown_lines():
  {==}

def Laws.hb_shows_nothing(u):
  {==}

def Laws.hubs_redraw_hb(hs, key):
  {==}

def Laws.hb_ignored_by_fold():
  {==}

def Laws.alive_due():
  {==}

def Laws.claude_text_ids_distinct(b):
  {==}

def Laws.bots_say_once(live, s, now, client, id, bot, text, msg):
  {==}

def Laws.bots_post_once(mid, m, s, now, client, id, text):
  {==}

def Laws.bots_tell_once(mid, remote, s, now, client, id, to, text):
  {==}
def Laws.exit_why_killed():
  {==}
















def Laws.live_split_fence_safe():
  {==}

def Laws.live_split_fence_closed():
  {==}

def Laws.live_split_none():
  {==}

def Laws.live_head_rest():
  {==}

def Laws.live_prefix_exact():
  {==}

def Laws.live_prefix_exact_open_fence():
  {==}

# A streamed reply's layout: the settled prefix's blocks, a gap and the
# tail's blocks are the whole's blocks (the window keeps the prefix's)
law tl_append_assoc:
  for a: List<&2, L.TLine>
  for +b: List<&2, L.TLine>
  for +c: List<&2, L.TLine>
  {List.append(&2, L.TLine, List.append(&2, L.TLine, a, b), c) == List.append(&2, L.TLine, a, List.append(&2, L.TLine, b, c)) : List<&2, L.TLine>}

def tl_append_assoc(a, b, c):
  match a:
    case Nil{}:
      {==}
    case +p <> +pt:
      Equal.cong(List<&2, L.TLine>, List<&2, L.TLine>, l => p <> l, List.append(&2, L.TLine, List.append(&2, L.TLine, pt, b), c),
        List.append(&2, L.TLine, pt, List.append(&2, L.TLine, b, c)), tl_append_assoc(pt, b, c))

law dm_append_nil:
  for xs: List<&2, DM.Node>
  {List.append(&2, DM.Node, xs, Nil{}) == xs : List<&2, DM.Node>}

def dm_append_nil(xs):
  match xs:
    case Nil{}:
      {==}
    case +p <> +pt:
      Equal.cong(List<&2, DM.Node>, List<&2, DM.Node>, l => p <> l, List.append(&2, DM.Node, pt, Nil{}), pt, dm_append_nil(pt))

law tl_blocks_append:
  for +hs: List<&2, DM.Node>
  for +ty: L.Ty
  for +px: U32
  for +h: DM.Node
  for +t: DM.Node
  for +ts: List<&2, DM.Node>
  {L.TL.blocks(ty, List.append(&2, DM.Node, h <> hs, t <> ts), px) == List.append(&2, L.TLine, L.TL.blocks(ty, h <> hs, px), L.TLine{"", 0, 4, "", 0} <> L.TL.blocks(ty, t <> ts, px)) : List<&2, L.TLine>}

def tl_blocks_append(hs, ty, px, h, t, ts):
  match hs:
    case Nil{}:
      {==}
    case +m <> +mr:
      Equal.trans(List<&2, L.TLine>,
        List.append(&2, L.TLine, L.TL.block(ty, h, px), L.TLine{"", 0, 4, "", 0} <> L.TL.blocks(ty, List.append(&2, DM.Node, m <> mr, t <> ts), px)),
        List.append(&2, L.TLine, L.TL.block(ty, h, px), L.TLine{"", 0, 4, "", 0} <> List.append(&2, L.TLine, L.TL.blocks(ty, m <> mr, px), L.TLine{"", 0, 4, "", 0} <> L.TL.blocks(ty, t <> ts, px))),
        List.append(&2, L.TLine, List.append(&2, L.TLine, L.TL.block(ty, h, px), L.TLine{"", 0, 4, "", 0} <> L.TL.blocks(ty, m <> mr, px)), L.TLine{"", 0, 4, "", 0} <> L.TL.blocks(ty, t <> ts, px)),
        Equal.cong(List<&2, L.TLine>, List<&2, L.TLine>, l => List.append(&2, L.TLine, L.TL.block(ty, h, px), L.TLine{"", 0, 4, "", 0} <> l),
          L.TL.blocks(ty, List.append(&2, DM.Node, m <> mr, t <> ts), px),
          List.append(&2, L.TLine, L.TL.blocks(ty, m <> mr, px), L.TLine{"", 0, 4, "", 0} <> L.TL.blocks(ty, t <> ts, px)),
          tl_blocks_append(mr, ty, px, m, t, ts)),
        Equal.sym(List<&2, L.TLine>, List.append(&2, L.TLine, List.append(&2, L.TLine, L.TL.block(ty, h, px), L.TLine{"", 0, 4, "", 0} <> L.TL.blocks(ty, m <> mr, px)), L.TLine{"", 0, 4, "", 0} <> L.TL.blocks(ty, t <> ts, px)),
          List.append(&2, L.TLine, L.TL.block(ty, h, px), L.TLine{"", 0, 4, "", 0} <> List.append(&2, L.TLine, L.TL.blocks(ty, m <> mr, px), L.TLine{"", 0, 4, "", 0} <> L.TL.blocks(ty, t <> ts, px))),
          tl_append_assoc(L.TL.block(ty, h, px), L.TLine{"", 0, 4, "", 0} <> L.TL.blocks(ty, m <> mr, px), L.TLine{"", 0, 4, "", 0} <> L.TL.blocks(ty, t <> ts, px))))

# the prefix kept and the tail laid out after it is the whole's layout
law live_cat_exact:
  for +hs: List<&2, DM.Node>
  for +ty: L.Ty
  for +px: U32
  for +text: String
  for +ts: List<&2, DM.Node>
  {L.TL.live.cat(ty, L.TL.live.fresh.k(ty, text, hs, px), ts, px) == L.TL.blocks(ty, List.append(&2, DM.Node, hs, ts), px) : List<&2, L.TLine>}

def live_cat_exact(hs, ty, px, text, ts):
  match hs:
    case Nil{}:
      {==}
    case +h <> +hr:
      match ts:
        case Nil{}:
          Equal.sym(List<&2, L.TLine>, L.TL.blocks(ty, List.append(&2, DM.Node, h <> hr, Nil{}), px), L.TL.blocks(ty, h <> hr, px),
            Equal.cong(List<&2, DM.Node>, List<&2, L.TLine>, ns => L.TL.blocks(ty, ns, px), List.append(&2, DM.Node, h <> hr, Nil{}), h <> hr, dm_append_nil(h <> hr)))
        case +t <> +tr:
          Equal.sym(List<&2, L.TLine>, L.TL.blocks(ty, List.append(&2, DM.Node, h <> hr, t <> tr), px),
            List.append(&2, L.TLine, L.TL.blocks(ty, h <> hr, px), L.TLine{"", 0, 4, "", 0} <> L.TL.blocks(ty, t <> tr, px)), tl_blocks_append(hr, ty, px, h, t, tr))
def Laws.qr_format_m():
  {==}
def Laws.qr_format_m1():
  {==}
def Laws.qr_version7_bits():
  {==}
def Laws.qr_version_fits():
  {==}
def Laws.qr_version_grows():
  {==}
def Laws.qr_version_none():
  {==}
def Laws.qr_gmul_wraps():
  {==}
def Laws.qr_link_escapes():
  {==}
def Laws.qr_link_none():
  {==}
def Laws.pair_qr_none():
  {==}

def Laws.onboard_steps_sound():
  {==}

def Laws.onboard_nav_every_step():
  {==}

def Laws.onboard_back_floor():
  {==}

def Laws.onboard_next_step():
  {==}

def Laws.onboard_back_step():
  {==}

def Laws.onboard_next_ends():
  {==}

def Laws.onboard_end_stays():
  {==}

def Laws.onboard_skip_ends():
  {==}

def Laws.onboard_fresh_first():
  {==}

def Laws.onboard_done_quiet():
  {==}

def Laws.onboard_auto_moved():
  {==}

def Laws.onboard_done_closes_auto():
  {==}

def Laws.onboard_hand_stays():
  {==}

def Laws.onboard_hand_offline():
  {==}

def Laws.onboard_wild_ends():
  {==}

def Laws.onboard_phone_no_target():
  {==}

def Laws.onboard_keys():
  {==}

def Laws.onboard_syms():
  {==}

def Laws.onboard_unsynced_hidden():
  {==}

def Laws.onboard_synced_shows():
  {==}

def Laws.onboard_finish_sets_done():
  {==}

def Laws.onboard_elsewhere_closes():
  {==}

def Laws.onboard_reopen():
  {==}

def Laws.onboard_waits(done):
  {==}

def Laws.onboard_live_log_shows():
  {==}

def Laws.onboard_join_waits():
  {==}

def Laws.onboard_join_shows():
  {==}

def Laws.onboard_cache_unsynced():
  {==}

def Laws.onboard_phone_cache_unsynced():
  {==}

def Laws.onboard_focus_thread():
  {==}

def Laws.onboard_focus_none():
  {==}

def Laws.onboard_focus_remote():
  {==}

def Laws.onboard_focus_room():
  {==}

def Laws.onboard_focus_settings():
  {==}

def Laws.onboard_head_thread():
  {==}

def Laws.onboard_head_remote():
  {==}

def Laws.onboard_head_settings():
  {==}

def Laws.held_set_has():
  {==}

def Laws.held_other_key():
  {==}

def Laws.held_let_frees():
  {==}

def Laws.held_empty_passes_press():
  {==}

def Laws.held_empty_passes_release():
  {==}

def Laws.held_none_at_first():
  {==}

def Laws.held_swallowed_without_tour():
  {==}

def Laws.tour_scroll_down():
  {==}

def Laws.tour_scroll_down_stops():
  {==}

def Laws.tour_scroll_up():
  {==}

def Laws.tour_scroll_up_stops():
  {==}

def Laws.tour_scroll_none():
  {==}

def Laws.mate_lower_asks():
  {==}

def Laws.mate_higher_waits():
  {==}

def Laws.mate_linked_never_asks():
  {==}

def Laws.mate_off_never_asks():
  {==}

def Laws.mate_no_self_never_asks():
  {==}

def Laws.mate_answer_invites():
  {==}

def Laws.mate_answer_off():
  {==}

def Laws.mate_answer_linked():
  {==}

def Laws.mate_off_hidden():
  {==}

def Laws.mate_on_shown():
  {==}

def Laws.far_catalog_stands_in():
  {==}

def Laws.far_catalog_syncing():
  {==}

def Laws.far_catalog_caught_up():
  {==}

def Laws.far_catalog_here_never():
  {==}

def Laws.far_catalog_titled():
  {==}

def Laws.far_scope_thread():
  {==}

def Laws.far_scope_project():
  {==}

def Laws.mate_unnamed_without_tailnet():
  {==}

def Laws.mate_answer_stranger():
  {==}

def Laws.refs_machine_word():
  {==}

def Laws.refs_colon_after_stays_here():
  {==}

def Laws.mate_loopback_lower_asks():
  {==}

def Laws.mate_loopback_higher_waits():
  {==}

def Laws.fview_file_up():
  {==}

def Laws.fview_file_abs():
  {==}

def Laws.fview_file_hidden():
  {==}

def Laws.fview_file_hidden_deep():
  {==}

def Laws.fview_file_pick():
  {==}

def Laws.fview_file_ok():
  {==}

def Laws.fview_file_empty():
  {==}

def Laws.fview_dir_root():
  {==}

def Laws.fview_dir_up():
  {==}

def Laws.fview_reads_disjoint():
  {==}

def Laws.fview_no_open():
  {==}

def Laws.fview_no_pdf():
  {==}

def Laws.fview_no_fs():
  {==}

def Laws.fview_allowed_not_write(s, p):
  {==}

def Laws.fview_allowed_path(s):
  {==}

def Laws.fview_allowed_want(s):
  {==}

def Laws.fview_allowed_dir(s):
  {==}

def Laws.fview_cache_under():
  {==}

def Laws.fview_cache_dotdot():
  {==}

def Laws.fview_cache_link_slash():
  {==}

def Laws.fview_cache_link_dotdot():
  {==}

def Laws.fview_cache_relative():
  {==}

def Laws.fview_cache_of_file():
  {==}

def Laws.fview_cache_of_outside():
  {==}

def Laws.fview_cache_of_prefix():
  {==}

def Laws.fview_fix_thread():
  {==}

def Laws.fview_fix_root():
  {==}

def Laws.fview_fix_again():
  {==}

def Laws.fview_fix_leaves_error():
  {==}
def Laws.fmc_taken_tool():
  {==}

def Laws.fmc_only_listed_tools():
  {==}

def Laws.fmc_no_terminal_or_delegate():
  {==}

def Laws.fmc_permit_keeps_client_rules(s, p):
  {==}

def Laws.fmc_unshared_refused():
  {==}

def Laws.fmc_unshared_changes_nothing():
  {==}

def Laws.fmc_unshared_interrupt_refused():
  {==}

def Laws.fmc_plan_here_stays():
  {==}

def Laws.fmc_plan_wait_polls_here():
  {==}

def Laws.fmc_plan_unlinked_never_sent():
  {==}

def Laws.fmc_plan_unlinked_machine_never_sent():
  {==}

def Laws.fmc_plan_machine_list_answered():
  {==}

def Laws.fmc_plan_other_tools_here():
  {==}

def Laws.fmc_args_made_local():
  {==}

def Laws.fmc_args_other_link_kept():
  {==}

def Laws.fmc_reply_ids_named_by_link():
  {==}

def Laws.fmc_give_unlinked_nothing():
  {==}

def Laws.fmc_give_local_drops_machines():
  {==}
def Laws.flive_delta_appends():
  {==}

def Laws.flive_gap_asks():
  {==}

def Laws.flive_gap_changes_nothing():
  {==}

def Laws.flive_end_blocks_old():
  {==}

def Laws.flive_end_no_resurrect():
  {==}

def Laws.flive_delta_ended_dropped():
  {==}

def Laws.flive_end_keeps_newer():
  {==}

def Laws.flive_end_before_any():
  {==}

def Laws.flive_next_gen_taken():
  {==}

def Laws.flive_snap_replaces():
  {==}

def Laws.flive_snap_older_stale():
  {==}

def Laws.flive_ended_ids():
  {==}

def Laws.flive_user_message_keeps():
  {==}

def Laws.flive_body_no_items():
  {==}

def Laws.flive_body_has_live():
  {==}

def Laws.flive_queue_joins():
  {==}

def Laws.flive_queue_gap_owes_whole():
  {==}

def Laws.flive_queue_whole_absorbs():
  {==}

def Laws.flive_queue_threads_apart():
  {==}

def Laws.flive_one_request_out():
  {==}

def Laws.flive_due_after_answer():
  {==}

def Laws.flive_quiet_drops():
  {==}

def Laws.flive_quiet_ends():
  {==}

def Laws.flive_whole_from_buffer():
  {==}

def Laws.flive_whole_ended_dropped():
  {==}

def Laws.flive_link_gone():
  {==}

def Laws.flive_far_message_ends_live():
  {==}

def Laws.flive_far_activity_keeps_live():
  {==}

def Laws.flive_far_delta_shows():
  {==}

def Laws.notice_far_key_same():
  {==}

def Laws.notice_far_fail_key_same():
  {==}

def Laws.notice_local_key_kept():
  {==}

def Laws.island_far_once():
  {==}

def Laws.island_far_owner_wins():
  {==}

def Laws.island_far_alone_kept():
  {==}

def Laws.rview_at_under():
  {==}

def Laws.rview_at_climbs():
  {==}

def Laws.rview_at_relative():
  {==}

def Laws.rview_back_at():
  {==}

def Laws.rview_back_outside():
  {==}

def Laws.rview_want_hidden():
  {==}

def Laws.rview_want_file():
  {==}

def Laws.rview_ask_get_up():
  {==}

def Laws.rview_ask_get_abs():
  {==}

def Laws.rview_ask_get_hidden():
  {==}

def Laws.rview_ask_get_ok():
  {==}

def Laws.rview_ask_ver_hidden():
  {==}

def Laws.rview_ask_no_folder():
  {==}

def Laws.rview_ask_other():
  {==}

def Laws.rview_git_option():
  {==}

def Laws.rview_git_dir():
  {==}

def Laws.rview_git_climbs():
  {==}

def Laws.rview_git_ok():
  {==}

def Laws.rview_git_other_repo():
  {==}

def Laws.rview_git_hidden():
  {==}

def Laws.rview_root_unknown():
  {==}

def Laws.rview_root_far_cache():
  {==}

def Laws.rview_enc_unreserved():
  {==}

def Laws.rview_enc_breaks():
  {==}

def Laws.rview_spec_local():
  {==}

def Laws.rview_root_far_other_link():
  {==}

def Laws.rview_root_far_climbs():
  {==}

def Laws.rview_root_own_ignores_claim():
  {==}

def Laws.lyr_board_order_kicad():
  {==}

def Laws.lyr_plain_as_is():
  {==}

def Laws.lyr_dim_quarter():
  {==}

def Laws.lyr_only_hides():
  {==}

def Laws.lyr_active_shows():
  {==}

def Laws.lyr_copper_keeps_vias():
  {==}

def Laws.lyr_outline_stays():
  {==}

def Laws.lyr_pick_again_all():
  {==}

def Laws.lyr_cycle_round():
  {==}

def Laws.lyr_kind_round():
  {==}

def Laws.lyr_kind_base():
  {==}

def Laws.lyr_masks_only():
  {==}

# Design review: comparing
def Laws.rv_diff_on_all(a, b):
  {==}

def Laws.rv_mark_on_all(olds, news):
  {==}

def Laws.rv_sheet_plain():
  {==}

def Laws.rv_sheet_compared():
  {==}

def Laws.rv_bits_plain(kind):
  {==}

# Design review (bom)

def Laws.bom_nat_c2_c10():
  {==}

def Laws.bom_nat_r10_r2():
  {==}

def Laws.bom_diff_self():
  {==}

def Laws.bom_group_qty():
  {==}

def Laws.bom_group_csv():
  {==}

def Laws.bom_csv_quote():
  {==}

def Laws.bom_csv_quote_inner():
  {==}

def Laws.bom_csv_plain():
  {==}

def Laws.bom_diff_line():
  {==}

def Laws.bom_diff_md_none():
  {==}

def Laws.bom_md_pipe():
  {==}

# Design review (checks, cad, release)

def Laws.cad_of_kicad_board():
  {==}

def Laws.cad_of_kicad_schematic():
  {==}

def Laws.cad_of_case_ignored():
  {==}

def Laws.cad_of_altium_not_drawn():
  {==}

def Laws.cad_of_unknown():
  {==}

def Laws.cad_fill_puts_vars_in():
  {==}

def Laws.cad_fill_keeps_other_braces():
  {==}

def Laws.cad_shell_quotes_a_quote():
  {==}

def Laws.cad_shell_words():
  {==}

def Laws.checklist_toggle_twice():
  {==}

def Laws.checklist_toggle_flips_one():
  {==}

def Laws.checklist_toggle_past_end():
  {==}

def Laws.checks_gate_no_findings():
  {==}

def Laws.checks_gate_no_findings_strict():
  {==}

def Laws.checks_md_empty():
  {==}

def Laws.checks_erc_error_counted():
  {==}

def Laws.release_next_dotted():
  {==}

def Laws.release_next_letter():
  {==}

def Laws.release_next_plain():
  {==}

def Laws.release_next_first():
  {==}

def Laws.release_tag_valid():
  {==}

def Laws.release_tag_no_space():
  {==}

def Laws.release_tag_no_dotdot():
  {==}

def Laws.release_tag_no_dash_lead():
  {==}

def Laws.release_tag_no_lock_end():
  {==}

def Laws.release_tag_not_empty():
  {==}

def Laws.release_dir_in_repo():
  {==}

def Laws.release_gh_command():
  {==}

# Design review (review.bend)

def Laws.review_fold_empty():
  {==}

def Laws.review_prompt_none():
  {==}

def Laws.review_unknown_ignored():
  {==}

def Laws.review_resolve():
  {==}

def Laws.review_reopen():
  {==}

def Laws.review_prompt_resolved():
  {==}

def Laws.review_drop():
  {==}

def Laws.review_reply_no_pin():
  {==}

def Laws.review_reply_counted():
  {==}

def Laws.review_orphan_ignored():
  {==}

def Laws.review_roundtrip_resolve():
  {==}

def Laws.review_roundtrip_edit():
  {==}

def Laws.review_roundtrip_drop():
  {==}

def Laws.review_other_tone():
  {==}

def Laws.review_moved():
  {==}

def Laws.review_not_moved():
  {==}

# Slash commands in the composer (slash.bend)
def Laws.slash_typing_word():
  {==}

def Laws.slash_typing_alone():
  {==}

def Laws.slash_not_after_space():
  {==}

def Laws.slash_not_midline():
  {==}

def Laws.slash_claude_prefix():
  {==}

def Laws.slash_others_own_only():
  {==}

def Laws.slash_none_unknown():
  {==}

# --- uberbot: uber_*, plane_* (thread t1791834693) ---

def Laws.uber_promote_far_refused(st):
  {==}

def Laws.uber_demote_clears():
  {==}

def Laws.uber_promote_sets():
  {==}

def Laws.uber_promote_again_nothing():
  {==}

def Laws.plane_each_once(u, ix, now):
  Laws.once_distinct(PLN.PRow, PLN.Plane.all.src(u, ix, now))

def Laws.plane_sections_once():
  {==}

def Laws.plane_need_ranked():
  {==}

def Laws.plane_hit_opens(t, at, pt, u, ix, st):
  {==}

# --- uberbot onboarding: onboard_uber_* ---

def Laws.onboard_uber_none_creates():
  {==}

def Laws.onboard_uber_pick_most_used():
  {==}

def Laws.onboard_uber_have_quiet():
  {==}

def Laws.onboard_uber_existing_shows():
  {==}

def Laws.onboard_uber_existing_cached():
  {==}

def Laws.onboard_uber_answered_quiet():
  {==}

def Laws.onboard_uber_have_no_ask():
  {==}

def Laws.onboard_uber_skip_answers():
  {==}

def Laws.onboard_uber_single_no_back():
  {==}

def Laws.hubs_plane_once(rs):
  Laws.once_distinct(J.Json, Hb.Hubs.pl.src(rs))
def Laws.uber_merge_comm():
  {==}

def Laws.uber_merge_idem():
  {==}

def Laws.uber_forget_wins_older():
  {==}

def Laws.uber_forget_loses_newer():
  {==}

def Laws.uber_one_holder_per_term():
  {==}

def Laws.uber_higher_term_wins():
  {==}

def Laws.uber_fire_holder_only(mine, holder):
  {==}

def Laws.uber_pinned_stays(mine, holder):
  {==}

def Laws.uber_slot_once(pinned, mine, holder):
  {==}

def Laws.uber_fire_state():
  {==}

def Laws.uber_fire_state_other():
  {==}

def Laws.uber_slot_once_state():
  {==}

def Laws.uber_pinned_state():
  {==}

def Laws.uber_derive_n_no_ops(n, b, me, o, at):
  match n:
    case 0n:
      {==}
    case 1n+p:
      match p:
        case 0n:
          {==}
        case 1n+p:
          match p:
            case 0n:
              {==}
            case 1n+p:
              match p:
                case 0n:
                  {==}
                case 1n+p:
                  match p:
                    case 0n:
                      {==}
                    case 1n+p:
                      match p:
                        case 0n:
                          {==}
                        case 1n+p:
                          match p:
                            case 0n:
                              {==}
                            case 1n+p:
                              match p:
                                case 0n:
                                  {==}
                                case 1n+p:
                                  {==}

def Laws.uber_derived_no_ops(win, st, o, at):
  match win:
    case True{}:
      Laws.uber_derive_n_no_ops(UBS.Ub.kind.n(UBO.Ub.field(o, "k")), UBR.Uber.bot(st), UBS.Ub.me(st), o, at)
    case False{}:
      {==}

def Laws.uber_superseded_nop():
  {==}

def Laws.uber_superseded_keeps():
  {==}

def Laws.uber_seed_once():
  {==}

# --- uberbot inc 3: plane_tally_*, plane_text_*, uber_home_* ---

def Laws.side_work_ix_eq():
  {==}

def Laws.plane_tally_counts():
  {==}

def Laws.plane_tally_counts_rich():
  {==}

def Laws.plane_text_need_first():
  {==}

def Laws.plane_text_need_first_rich():
  {==}

def Laws.fmc_plan_plane_read_here():
  {==}

def Laws.fmc_plan_plane_read_unlinked_never_sent():
  {==}

def Laws.fmc_plane_read_allowed():
  {==}

def Laws.uber_home_takes_term():
  {==}

def Laws.uber_home_holder_nothing():
  {==}

def Laws.plane_home_button():
  {==}

def Laws.plane_home_button_held():
  {==}

def Laws.plane_list_same():
  {==}

def Laws.plane_list_same_rich():
  {==}

def Laws.launch_reports_turn_end():
  {==}

def Laws.launch_ask_at_once():
  {==}

def Laws.launch_fail_told():
  {==}

def Laws.launch_once_per_end():
  {==}

def Laws.launch_muted_quiet():
  {==}

def Laws.launch_waits_for_work():
  {==}

def Laws.launch_said_quiet():
  {==}

def Laws.launch_own_stop_quiet():
  {==}

def Laws.launch_stop_told():
  {==}

def Laws.launch_delivered_in_order():
  {==}

def Laws.launch_loop_bounded():
  {==}

def Laws.launch_far_once():
  {==}

def Laws.launch_far_order():
  {==}

def Laws.launch_list_once():
  {==}

def Laws.launched_up_launcher():
  {==}

def Laws.subs_launched_live_kept():
  {==}

def Laws.notice_report_quiet():
  {==}

def Laws.launch_send_launcher():
  {==}
def Laws.interrupt_antigravity_stops(th, r):
  {==}

def Laws.antigravity_run_turn(th, cwd, m, s, r, i, t, u):
  {==}

def Laws.agy_mode_supervised_asks():
  {==}

def Laws.agy_mode_full_allows():
  {==}

def Laws.agy_default_api_key():
  {==}

def Laws.agy_google_warns():
  {==}

def Laws.agy_callback_needs_signin(b):
  {==}


# ios-snappy: nav
def Laws.mnav_bot_sel(hub, id, tail):
  {==}

def Laws.mnav_room_local(hub, id):
  {==}

def Laws.mnav_remote_local(hub, id):
  {==}

def Laws.mnav_bot_thread(hub, id, thread, some, own):
  %Equal.sym(Bool, String.is_empty(thread), False{}, some) : {Hb.Hubs.nav.qual(hub, MBN.MBots.nav.k(_, id, thread)) == Hb.Hubs.id(hub, thread) : String}
  %Equal.sym(Bool, Hb.Hubs.nav.tag(thread), False{}, own) : {Bool.pick(String, _, thread, Hb.Hubs.id(hub, thread)) == Hb.Hubs.id(hub, thread) : String}
  {==}

# harness-coord: wakes
def Laws.wake_once_per_event():
  {==}

def Laws.wake_once_drops():
  {==}

def Laws.wake_two_events_one_message():
  {==}

def Laws.wake_not_self():
  {==}

def Laws.wake_bot_thread_delivered():
  {==}

def Laws.wake_pr_merged():
  {==}

def Laws.wake_pr_repeat_quiet():
  {==}

def Laws.codec_wake_roundtrip():
  {==}

def Laws.codec_wake_dropped_roundtrip():
  {==}

def Laws.wake_message_one_paragraph():
  {==}

def Laws.wake_event_independent_of_step():
  {==}

# harness-coord: CLI path
def Laws.clipath_keeps_current():
  {==}

def Laws.clipath_no_dupes():
  {==}

def Laws.clipath_idempotent():
  {==}

def Laws.clipath_nvm_newest_first():
  {==}
# ---- notif-policy ----

def Laws.policy_no_alert_unless():
  {==}

def Laws.policy_settle_once():
  {==}

def Laws.policy_settle_after_launch():
  {==}

def Laws.policy_fail_person_only():
  {==}

def Laws.policy_ask_alerts():
  {==}

def Laws.policy_ask_alerted_out():
  {==}

def Laws.policy_ask_off_silent():
  {==}

def Laws.policy_merge_alerts():
  {==}

def Laws.policy_end_alerted():
  {==}

def Laws.policy_setting_off():
  {==}

def Laws.policy_all_legacy():
  {==}

def Laws.policy_link_not_person():
  {==}

def Laws.policy_person_direct():
  {==}

def Laws.notify_refuses_chatter():
  {==}

def Laws.notify_reminder_routine_only():
  {==}

def Laws.notify_needs_you_once():
  {==}

def Laws.notify_done_settles():
  {==}

def Laws.notify_setting_off():
  {==}

def Laws.desk_redraw_not_input():
  {==}

def Laws.desk_key_input(c, t, m, d, k, rest):
  {==}

def Laws.policy_phone_raises():
  {==}

def Laws.policy_phone_same_none():
  {==}

def Laws.policy_phone_empty_none():
  {==}

def Laws.policy_phone_unseen_none():
  {==}

def Laws.policy_phone_watched_held():
  {==}

def Laws.policy_phone_note_not_held():
  {==}

def Laws.policy_phone_ids_record():
  {==}

def Laws.policy_phone_frame_raises():
  {==}

def Laws.policy_phone_turn_end_quiet():
  {==}

def Laws.policy_phone_boot_quiet():
  {==}

def Laws.policy_reminder_alerts():
  {==}

def Laws.notify_rows_defaults():
  {==}

def Laws.island_compact_one_line():
  {==}

def Laws.island_no_children():
  {==}

def Laws.island_line_last_sent():
  {==}

# ---- end notif-policy ----

# ---- told-rules-board ----
def Laws.told_setting_empty_removes():
  {==}

def Laws.told_mark_key_norm():
  {==}

def Laws.told_check_finds_marked():
  {==}

def Laws.told_check_text_overlap():
  {==}

def Laws.told_prune_old():
  {==}

def Laws.told_quiet_end_marks_nothing():
  {==}

def Laws.rules_block_empty_when_none():
  {==}

def Laws.rules_block_scoped():
  {==}

def Laws.rules_refuse_enforced():
  {==}

def Laws.rules_word_unlocks():
  {==}

def Laws.rules_mcp_prefix():
  {==}

def Laws.rules_off_ignored():
  {==}

def Laws.tb_order_open_first():
  {==}

def Laws.tb_import_checkbox():
  {==}

def Laws.tb_import_idempotent():
  {==}

def Laws.tb_update_merges():
  {==}

def Laws.tb_due_shape():
  {==}

def Laws.told_report_quiet():
  {==}

def Laws.rules_report_never_unlocks():
  {==}

def Laws.rules_link_never_unlocks():
  {==}

def Laws.told_note_marked():
  {==}

def Laws.told_quiet_record_nothing():
  {==}

# ---- end told-rules-board ----
def Laws.deploy_waits_for_tool():
  {==}

def Laws.deploy_waits_for_lock():
  {==}

def Laws.deploy_goes_when_clear():
  {==}

def Laws.deploy_go_clears_wait():
  {==}

def Laws.deploy_go_writes_done():
  {==}

def Laws.deploy_idle_nothing():
  {==}

def Laws.deploy_after_note_once():
  {==}

def Laws.deploy_after_clears():
  {==}

def Laws.deploy_after_second_start_quiet():
  {==}

def Laws.deploy_requester_bot_thread():
  {==}

def Laws.deploy_cut_requester_no_second():
  {==}

def Laws.deploy_cancel_clears():
  {==}

# harness-coord: locks

def Laws.lock_exclusive(l):
  match l:
    case LCK.NamedLock{None{}, _, _, _}:
      {==}
    case LCK.NamedLock{Some{_}, _, _, _}:
      {==}

def Laws.lock_fifo(holder, first, rest):
  {==}

def Laws.lock_holder_only(l):
  {==}

def Laws.lock_release_next():
  {==}

def Laws.lock_deleted_holder_frees():
  {==}

def Laws.lock_work_releases():
  {==}

def Laws.lock_grant_once():
  {==}

def Laws.lock_ready_idempotent(l):
  match l:
    case LCK.NamedLock{_, _, _, _}:
      {==}

def Laws.codec_lock_roundtrip():
  {==}

# harness-coord: locks fix

def Laws.lock_status_requeues_lost_client():
  {==}

def Laws.lock_renew_requeues_lost_client():
  {==}

def Laws.lock_requeue_keeps_fifo_for_others():
  {==}

def Laws.lock_restart_extends_client_leases():
  {==}
# ---- phone-sch-viewer: exe path ----

def Laws.exe_path_plain():
  {==}

def Laws.exe_path_deleted():
  {==}

def Laws.exe_child_plain():
  {==}

def Laws.exe_child_deleted():
  {==}

def Laws.exe_gone_plain():
  {==}

def Laws.exe_gone_deleted():
  {==}

# ---- end phone-sch-viewer: exe path ----

# routines-cal
def Laws.cal_patch_only_given():
  {==}

def Laws.cal_update_asks():
  {==}

def Laws.cal_delete_asks():
  {==}

def Laws.cal_id_no_slash():
  {==}

# routines-cal: one-time routines, zones
def Laws.tz_fixed_wall(east, secs, t):
  {==}

def Laws.sched_done_never(s, id, bot, name, cron, prompt, on, last, created, deleted, once, tz, kind, slot, now, mo, me):
  {==}

def Laws.sched_once_due(s, id, bot, name, cron, prompt, last, created, k, tz, kind, slot, now, mo, me):
  {==}

# the machine's offset reads a schedule the same either way
def sched_legacy_k(m: Maybe<&2, CR.Cron>, last: Nat, now: Nat, mo: Nat, me: Bool) -> {SCH.Sched.cron.due(m, last, now, mo, me) == BHS.Bots.due.k(m, last, now, BHS.Tz{mo, me}) : Bool}:
  match m:
    case Some{c}:
      {==}
    case None{}:
      {==}

def Laws.sched_legacy_same(s, id, bot, name, cron, prompt, on, last, created, deleted, kind, slot, now, mo, me):
  match on deleted:
    case True{} True{}:
      {==}
    case True{} False{}:
      sched_legacy_k(CR.Cron.parse(cron), last, now, mo, me)
    case False{} _:
      {==}

# a schedule is not due at a time no later than the one it last fired at
def due_not_later(o: Cmp, +x: Nat, +now: Nat) -> {Bool.and(Cmp.is_le(o), CR.Cron.due.k(o, x, now)) == False{} : Bool}:
  match o:
    case LT{}:
      {==}
    case EQ{}:
      {==}
    case GT{}:
      {==}

def due_not_after(+c: CR.Cron, +last: Nat, +now: Nat, +off: Nat, +east: Bool) -> {Bool.and(Nat.is_le(now, last), CR.Cron.due(c, last, now, off, east)) == False{} : Bool}:
  due_not_later(Nat.cmp(now, last), CR.Cron.next(c, last, off, east), now)

def due_slot(+g: Bool, +w2: Nat, +slot: Nat, +wl: Nat, +c: CR.Cron) -> {Bool.and(g, Bool.and(Nat.is_le(w2, slot), CR.Cron.due(c, Bool.pick(Nat, g, slot, wl), w2, 0n, True{}))) == False{} : Bool}:
  match g:
    case True{}:
      due_not_after(c, slot, w2, 0n, True{})
    case False{}:
      {==}

def Laws.sched_zoned_once_per_slot(c, rule, last, now2, slot):
  due_slot(Nat.is_gt(slot, 0n), TZ.Tz.wall(rule, now2), slot, TZ.Tz.wall(rule, last), c)

def Laws.when_in_minutes():
  {==}

def Laws.when_in_hours():
  {==}
def Laws.task_secret_never_stored(v, label):
  {==}

def Laws.task_risky_needs_confirm(n, kind):
  {==}

def Laws.run_confirm_runs(n, k, r):
  {==}

def Laws.run_stuck_alerts_once(n, k, r, s):
  {==}

def Laws.run_pause_alerts(n, k, r):
  {==}

def Laws.run_stop_ends(n, k, r):
  {==}

def Laws.drive_person_locks_bot(w):
  {==}

def Laws.task_slug_nonempty(x):
  match x:
    case SNil{}:
      {==}
    case SCon{c, t}:
      {==}

def Laws.task_slug_empty():
  {==}

# gmail: replies, attachments, drafts
def Laws.gmail_reply_subject_adds_re():
  {==}

def Laws.gmail_reply_subject_keeps_re():
  {==}

def Laws.gmail_references_appends():
  {==}

def Laws.gmail_reply_to_prefers_reply_to():
  {==}

def Laws.gmail_mime_type_pdf():
  {==}

def Laws.gmail_mime_type_default():
  {==}

def Laws.gmail_signed_plain():
  {==}

def Laws.gmail_signed_html_escapes():
  {==}

def Laws.gmail_signed_empty_unchanged():
  {==}

def Laws.gmail_draft_no_ask():
  {==}

def Laws.gmail_size_kb():
  {==}

def Laws.gmail_review_omits_empty_cc():
  {==}

def Laws.ask_head_gmail():
  {==}

def Laws.ask_head_draft():
  {==}

def Laws.ask_head_other_tool():
  {==}

def Laws.ask_head_cal_update():
  {==}
# end gmail: replies, attachments, drafts
# harness-coord: deploy inflight
def Laws.tool_open_shut_pair():
  {==}

def Laws.tool_turn_end_clears():
  {==}

def Laws.deploy_goes_between_calls():
  {==}

def Laws.deploy_waits_open_call():
  {==}
# ios-snappy: lost work

def Laws.lostwork_exit_clears():
  {==}

def Laws.lostwork_exit_clears_off():
  {==}

def Laws.lostwork_exit_clears_running():
  {==}

def Laws.lostwork_exit_keeps_child():
  {==}

def Laws.lostwork_exit_keeps_watch():
  {==}

def Laws.lostwork_exit_island_hides():
  {==}

def Laws.lostwork_recover_none_open():
  {==}

def Laws.lostwork_recover_none_open_off():
  {==}

def Laws.lostwork_recover_none_held():
  {==}

def Laws.lostwork_recover_none_held_carried():
  {==}

def Laws.lostwork_recover_island_hides():
  {==}

def Laws.lostwork_recover_still_told():
  {==}

def Laws.lostwork_recover_nothing_left_aside():
  {==}

def Laws.lostwork_recover_old_rows_go():
  {==}

def Laws.lostwork_recover_old_rows_go_off():
  {==}

def Laws.teach_mode_idle():
  {==}

def Laws.teach_mode_recording():
  {==}

def Laws.teach_mode_review():
  {==}

def Laws.teach_mode_takeover():
  {==}

def Laws.teach_input_only_when_driving(d):
  {==}

def Laws.teach_bot_drives_no_input():
  {==}

def Laws.teach_risky_confirms():
  {==}

def Laws.teach_plain_continues():
  {==}

def Laws.teach_risky_default_options():
  {==}

def Laws.teach_risky_button(id):
  {==}

def Laws.teach_secret_no_chip(act, l):
  {==}

def Laws.teach_secret_row_no_chip(n, t, l, c, m, p):
  {==}

def Laws.teach_tasks_tab():
  {==}

def Laws.teach_clock():
  {==}

def Laws.teach_preview_refused_teaching():
  {==}

def Laws.teach_preview_refused_takeover():
  {==}

def Laws.teach_preview_other_bot_free():
  {==}

def Laws.teach_preview_allowed_idle():
  {==}

def Laws.teach_handback_returns_browser():
  {==}

def Laws.teach_info_idle():
  {==}

def Laws.teach_info_recording():
  {==}

def Laws.teach_info_review():
  {==}

def Laws.teach_pause_options_risky():
  {==}

def Laws.teach_pause_options_plain():
  {==}

def Laws.teach_scale_corner():
  {==}

def Laws.teach_scale_middle():
  {==}

def Laws.teach_run_no_slash():
  {==}

def Laws.teach_run_no_dots():
  {==}

def Laws.teach_file_no_traversal():
  {==}

def Laws.teach_run_slug_roundtrip():
  {==}

def Laws.teach_recording_expires():
  {==}

def Laws.teach_recording_not_early():
  {==}

def Laws.teach_draft_secret_no_value():
  {==}

def Laws.mobile_browser_ask_card(a, d, nf):
  {==}

def Laws.mobile_other_ask_standard(a, d, nf):
  {==}

def Laws.mobile_browser_card_only_browser(t, rest):
  {==}
# ios-snappy: tabs
def Laws.local_view_wins():
  {==}

def Laws.local_view_wins_projects():
  {==}

def Laws.local_view_unset_hub():
  {==}

def Laws.local_view_unset_hub_projects():
  {==}

def Laws.local_view_junk_ignored():
  {==}

def Laws.hub_settings_moves_focus():
  {==}
# ---- end ios-snappy tabs ----
# ---- hub-stream-fast: busy count ----
def Laws.busy_stream_inert():
  {==}

def Laws.busy_stream_keeps_count():
  {==}

def Laws.busy_demo_count():
  {==}

def Laws.busy_turn_moves():
  {==}

def Laws.busy_rows_move():
  {==}
# ---- end hub-stream-fast: busy count ----
# ---- hub-fanout: commit lines ----
def Laws.commit_line_is_wire():
  {==}

def Laws.commit_line_device_hides_token():
  {==}
# ---- end hub-fanout: commit lines ----
# win-lean-frame: caret blink
law caret_keep_k:
  for b: Bool
  for +x0: U32
  for +y0: U32
  for +x1: U32
  for +y1: U32
  for +c: U32
  for +a: U32
  for +s: R.Shape
  for +on: Bool
  for +rest: List<&2, R.Op>
  {R.Region.keep.k(b, R.Op{x0, y0, x1, y1, c, T.Caret.alpha(c, a, on), s}, T.Caret.set(rest, on)) == T.Caret.set(R.Region.keep.k(b, R.Op{x0, y0, x1, y1, c, a, s}, rest), on) : List<&2, R.Op>}

def caret_keep_k(b, x0, y0, x1, y1, c, a, s, on, rest):
  match b:
    case True{}:
      {==}
    case False{}:
      {==}

law caret_keep_op:
  for hit: Bool
  for +x0: U32
  for +y0: U32
  for +x1: U32
  for +y1: U32
  for +c: U32
  for +a: U32
  for +s: R.Shape
  for +on: Bool
  for +px: U32
  for +py: U32
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +rest: List<&2, R.Op>
  {R.Region.keep.op(hit, R.Op{x0, y0, x1, y1, c, T.Caret.alpha(c, a, on), s}, px, py, rx, ry, n, T.Caret.set(rest, on)) == T.Caret.set(R.Region.keep.op(hit, R.Op{x0, y0, x1, y1, c, a, s}, px, py, rx, ry, n, rest), on) : List<&2, R.Op>}

def caret_keep_op(hit, x0, y0, x1, y1, c, a, s, on, px, py, rx, ry, n, rest):
  match hit:
    case True{}:
      caret_keep_k(Bool.not(R.Region.empty(R.Region.move(s, px, py, rx, ry, n))), x0, y0, x1, y1, c, a, R.Region.move(s, px, py, rx, ry, n), on, rest)
    case False{}:
      {==}

def Laws.caret_keep(ops, on, px, py, rx, ry, n):
  match ops:
    case Nil{}:
      {==}
    case R.Op{+x0, +y0, +x1, +y1, +c, +a, +s} <> +t:
      ih = Laws.caret_keep(t, on, px, py, rx, ry, n)
      %Equal.sym(List<&2, R.Op>, R.Region.keep(T.Caret.set(t, on), px, py, rx, ry, n), T.Caret.set(R.Region.keep(t, px, py, rx, ry, n), on), ih) : {R.Region.keep.op(R.Region.hits(x0, y0, x1, y1, rx, ry, n), R.Op{x0, y0, x1, y1, c, T.Caret.alpha(c, a, on), s}, px, py, rx, ry, n, _) == T.Caret.set(R.Region.keep.op(R.Region.hits(x0, y0, x1, y1, rx, ry, n), R.Op{x0, y0, x1, y1, c, a, s}, px, py, rx, ry, n, R.Region.keep(t, px, py, rx, ry, n)), on) : List<&2, R.Op>}
      caret_keep_op(R.Region.hits(x0, y0, x1, y1, rx, ry, n), x0, y0, x1, y1, c, a, s, on, px, py, rx, ry, n, R.Region.keep(t, px, py, rx, ry, n))

def Laws.caret_loc(ops, on, ox, oy, n):
  match ops:
    case Nil{}:
      {==}
    case R.Op{+x0, +y0, +x1, +y1, +c, +a, +s} <> +t:
      ih = Laws.caret_loc(t, on, ox, oy, n)
      %ih : {T.Loc.op(R.Op{x0, y0, x1, y1, c, T.Caret.alpha(c, a, on), s}, ox, oy, n) <> T.Loc.ops(T.Caret.set(t, on), ox, oy, n) == T.Caret.op(T.Loc.op(R.Op{x0, y0, x1, y1, c, a, s}, ox, oy, n), on) <> _ : List<&2, R.Op>}
      {==}

# the walk's flag is true when it answers true
law caret_fresh_ok:
  for ops: List<&2, R.Op>
  for +on: Bool
  for ok: Bool
  for h: {T.Caret.fresh.go(ops, on, ok) == True{} : Bool}
  {ok == True{} : Bool}

def caret_fresh_ok(ops, on, ok, h):
  match ops ok:
    case _ True{}:
      {==}
    case Nil{} False{}:
      Empty.absurd({False{} == True{} : Bool}, false_ne_true(h))
    case _ <> _ False{}:
      Empty.absurd({False{} == True{} : Bool}, false_ne_true(h))

law caret_same_op:
  for o: R.Op
  for +on: Bool
  for h: {T.Caret.same(o, on) == True{} : Bool}
  {T.Caret.op(o, on) == o : R.Op}

def caret_same_op(o, on, h):
  match o:
    case R.Op{+x0, +y0, +x1, +y1, +c, +a, +s}:
      e = u32_eq_sound(T.Caret.alpha(c, a, on), a, h)
      %e : {R.Op{x0, y0, x1, y1, c, T.Caret.alpha(c, a, on), s} == R.Op{x0, y0, x1, y1, c, _, s} : R.Op}
      {==}

def Laws.caret_fresh(ops, on, ok, h):
  match ops ok:
    case Nil{} _:
      {==}
    case +x <> +xt False{}:
      Empty.absurd({T.Caret.set(x <> xt, on) == x <> xt : List<&2, R.Op>}, false_ne_true(h))
    case +x <> +xt True{}:
      e0 = caret_same_op(x, on, caret_fresh_ok(xt, on, T.Caret.same(x, on), h))
      e1 = Laws.caret_fresh(xt, on, T.Caret.same(x, on), h)
      %e0 : {T.Caret.op(x, on) <> T.Caret.set(xt, on) == _ <> xt : List<&2, R.Op>}
      %e1 : {T.Caret.op(x, on) <> T.Caret.set(xt, on) == T.Caret.op(x, on) <> _ : List<&2, R.Op>}
      {==}

law blink_leaf_k:
  for f: Bool
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +base: Image
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +on: Bool
  for +h: {T.Caret.fresh(ops, on) == f : Bool}
  {T.Tile.blink.k(f, e, T.Tile.make(e, ops, base, rx, ry, n), on) == T.Tile.make(e, T.Caret.set(ops, on), base, rx, ry, n) : T.Tile}

def blink_leaf_k(f, e, ops, base, rx, ry, n, on, h):
  match f:
    case True{}:
      ef = Laws.caret_fresh(ops, on, True{}, h)
      %Equal.sym(List<&2, R.Op>, T.Caret.set(ops, on), ops, ef) : {T.Tile.make(e, ops, base, rx, ry, n) == T.Tile.make(e, _, base, rx, ry, n) : T.Tile}
      {==}
    case False{}:
      el = Laws.caret_loc(ops, on, rx, ry, T.Tile.size(e))
      %Equal.sym(List<&2, R.Op>, T.Loc.ops(T.Caret.set(ops, on), rx, ry, T.Tile.size(e)), T.Caret.set(T.Loc.ops(ops, rx, ry, T.Tile.size(e)), on), el) : {T.Tile{T.Caret.set(ops, on), T.Caret.set(T.Loc.ops(ops, rx, ry, T.Tile.size(e)), on), base, T.Tile.draw(e, T.Caret.set(T.Loc.ops(ops, rx, ry, T.Tile.size(e)), on), base)} == T.Tile{T.Caret.set(ops, on), _, base, T.Tile.draw(e, _, base)} : T.Tile}
      {==}

# one quadrant: the law one level down (ih), its ops cut from the
# blinked ops (caret_keep)
law blink_child:
  for +j: Nat
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +bq: Image
  for +px: U32
  for +py: U32
  for +cx: U32
  for +cy: U32
  for +h: U32
  for +on: Bool
  for ih: {T.Tiles.blink(T.Tiles.make(j, e, R.Region.keep(ops, px, py, cx, cy, h), bq, cx, cy, h), e, on) == T.Tiles.make(j, e, T.Caret.set(R.Region.keep(ops, px, py, cx, cy, h), on), bq, cx, cy, h) : T.Tiles}
  {T.Tiles.blink(T.Tiles.make(j, e, R.Region.keep(ops, px, py, cx, cy, h), bq, cx, cy, h), e, on) == T.Tiles.make(j, e, R.Region.keep(T.Caret.set(ops, on), px, py, cx, cy, h), bq, cx, cy, h) : T.Tiles}

def blink_child(j, e, ops, bq, px, py, cx, cy, h, on, ih):
  ek = Laws.caret_keep(ops, on, px, py, cx, cy, h)
  %Equal.sym(List<&2, R.Op>, R.Region.keep(T.Caret.set(ops, on), px, py, cx, cy, h), T.Caret.set(R.Region.keep(ops, px, py, cx, cy, h), on), ek) : {T.Tiles.blink(T.Tiles.make(j, e, R.Region.keep(ops, px, py, cx, cy, h), bq, cx, cy, h), e, on) == T.Tiles.make(j, e, _, bq, cx, cy, h) : T.Tiles}
  ih

law blink_node_k:
  for f: Bool
  for +j: Nat
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +base: Image
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +on: Bool
  for +h: {T.Caret.fresh(ops, on) == f : Bool}
  for ha: {T.Tiles.blink(T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)), e, on) == T.Tiles.make(j, e, R.Region.keep(T.Caret.set(ops, on), rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)) : T.Tiles}
  for hb: {T.Tiles.blink(T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)), e, on) == T.Tiles.make(j, e, R.Region.keep(T.Caret.set(ops, on), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)) : T.Tiles}
  for hc: {T.Tiles.blink(T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)), e, on) == T.Tiles.make(j, e, R.Region.keep(T.Caret.set(ops, on), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)) : T.Tiles}
  for hd: {T.Tiles.blink(T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), e, on) == T.Tiles.make(j, e, R.Region.keep(T.Caret.set(ops, on), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)) : T.Tiles}
  {T.Tiles.blink.go(T.Tiles.make(1n+j, e, ops, base, rx, ry, n), e, on, f) == T.Tiles.make(1n+j, e, T.Caret.set(ops, on), base, rx, ry, n) : T.Tiles}

def blink_node_k(f, j, e, ops, base, rx, ry, n, on, h, ha, hb, hc, hd):
  match f:
    case True{}:
      ef = Laws.caret_fresh(ops, on, True{}, h)
      %Equal.sym(List<&2, R.Op>, T.Caret.set(ops, on), ops, ef) : {T.Tiles.make(1n+j, e, ops, base, rx, ry, n) == T.Tiles.make(1n+j, e, _, base, rx, ry, n) : T.Tiles}
      {==}
    case False{}:
      tnode_cong(T.Caret.set(ops, on), base,
        T.Tiles.blink(T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)), e, on), T.Tiles.make(j, e, R.Region.keep(T.Caret.set(ops, on), rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)),
        T.Tiles.blink(T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)), e, on), T.Tiles.make(j, e, R.Region.keep(T.Caret.set(ops, on), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)),
        T.Tiles.blink(T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)), e, on), T.Tiles.make(j, e, R.Region.keep(T.Caret.set(ops, on), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)),
        T.Tiles.blink(T.Tiles.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), e, on), T.Tiles.make(j, e, R.Region.keep(T.Caret.set(ops, on), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)),
        ha, hb, hc, hd)

def Laws.tiles_blink_exact(k, e, ops, base, rx, ry, n, on):
  match k:
    case 0n:
      Equal.cong(T.Tile, T.Tiles, t => T.TLeaf{t}, T.Tile.blink.k(T.Caret.fresh(ops, on), e, T.Tile.make(e, ops, base, rx, ry, n), on), T.Tile.make(e, T.Caret.set(ops, on), base, rx, ry, n),
        blink_leaf_k(T.Caret.fresh(ops, on), e, ops, base, rx, ry, n, on, {==}))
    case 1n+j0:
      +j = j0
      blink_node_k(T.Caret.fresh(ops, on), j, e, ops, base, rx, ry, n, on, {==},
        blink_child(j, e, ops, T.Img.q(base, 0n), rx, ry, rx, ry, U32.shr(n), on, Laws.tiles_blink_exact(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n), on)),
        blink_child(j, e, ops, T.Img.q(base, 1n), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n), on, Laws.tiles_blink_exact(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n), on)),
        blink_child(j, e, ops, T.Img.q(base, 2n), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n), on, Laws.tiles_blink_exact(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n), on)),
        blink_child(j, e, ops, T.Img.q(base, 3n), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n), on, Laws.tiles_blink_exact(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n), on)))
# ---- end win-lean-frame caret blink ----

# ---- perf-lead: phone-lean (src/mobile) ----
def Laws.alerts_none_for_hb(u):
  {==}

def Laws.alerts_none_for_delta_end(u, th):
  {==}

def Laws.hubs_redraw_delta_end(hs, key, th):
  {==}

def Laws.policy_phone_untouched_quiet():
  {==}

def Laws.policy_phone_touched_raises():
  {==}

def Laws.hubs_lean_no_live():
  {==}

def Laws.hubs_lean_keeps_state():
  {==}
# ---- end perf-lead: phone-lean ----
# ui-panel-paste-lag: subagents

def Laws.subs_finished_hidden(rs):
  {==}

def Laws.subs_ageout_hidden(now, at, old):
  %Equal.sym(Bool, SB.Subs.old(now, at), True{}, old) : {Bool.not(_) == False{} : Bool}
  {==}

def Laws.subs_old_row_not_listed():
  {==}

def Laws.subs_panel_new_reopens():
  {==}

def Laws.subs_panel_shrinks_shut():
  {==}

def Laws.subs_panel_attention_reopens():
  {==}

def Laws.subs_model_alias():
  {==}

def Laws.subs_panel_resolved_stays_shut():
  {==}

# ui-panel-paste-lag: paste helpers
def paste_append_empty(s: String) -> {s ++ "" == s : String}:
  match s:
    case SNil{}:
      {==}
    case SCon{h, t}:
      %paste_append_empty(t) : {SCon{h, t ++ ""} == SCon{h, _} : String}
      {==}

def paste_take_zero(s: String) -> {String.take(s, 0n) == "" : String}:
  match s:
    case SNil{}:
      {==}
    case SCon{_, _}:
      {==}

# ui-panel-paste-lag: paste
def Laws.paste_name_png():
  {==}
def Laws.paste_name_jpeg():
  {==}
def Laws.paste_name_webp():
  {==}
def Laws.paste_name_gif():
  {==}
def Laws.paste_name_bmp():
  {==}
def Laws.paste_name_nonempty(mime):
  {==}
def Laws.paste_split_rejoins(s, width):
  match s width:
    case SNil{} _:
      {==}
    case SCon{h, t} 0n:
      {==}
    case SCon{h, t} 1n+k:
      %Laws.paste_split_rejoins(t, k) : {SCon{h, String.take(t, k) ++ String.drop(t, k)} == SCon{h, _} : String}
      {==}
def Laws.paste_parts_rejoin(fuel, width, s):
  match fuel:
    case 0n:
      # String.append(s, "") is not definitionally s.
      paste_append_empty(s)
    case 1n+k:
      %Equal.sym(String, AT.Att.paste.join(AT.Att.paste.parts.go(k, width, String.drop(s, width))), String.drop(s, width), Laws.paste_parts_rejoin(k, width, String.drop(s, width))) : {String.take(s, width) ++ _ == s : String}
      Laws.paste_split_rejoins(s, width)
def Laws.paste_full_piece(prefix, rest):
  match prefix:
    case SNil{}:
      paste_take_zero(rest)
    case SCon{h, t}:
      %Laws.paste_full_piece(t, rest) : {SCon{h, String.take(t ++ rest, String.length(t))} == SCon{h, _} : String}
      {==}
def Laws.paste_chunk_quartets():
  {==}
def Laws.paste_chunk_triples():
  {==}
def Laws.paste_tour_ignored(n, mime, size, data, key):
  {==}

def Laws.paste_parts_k_rejoin(zero, s, width):
  match zero:
    case True{}:
      paste_append_empty(s)
    case False{}:
      Laws.paste_parts_rejoin(Nat.div(Nat.sub(String.length(s), 1n), width), width, s)
def Laws.paste_upload_rejoins(s, width):
  Laws.paste_parts_k_rejoin(Nat.is_eq(width, 0n), s, width)
def Laws.paste_full_piece_size(prefix, rest):
  %Laws.paste_full_piece(prefix, rest) : {String.length(String.take(prefix ++ rest, String.length(prefix))) == String.length(_) : Nat}
  {==}

def Laws.paste_key_length(word):
  {==}
# ---- agent-adopt ----
def Laws.adopt_demo_kept():
  {==}

def Laws.adopt_demo_stopped():
  {==}

def Laws.adopt_off_stops_all_live():
  {==}

def Laws.adopt_one_per_thread():
  {==}

def Laws.adopt_mismatch_left():
  {==}

def Laws.adopt_low_pid_left():
  {==}

def Laws.cmds_json_round():
  {==}

def Laws.adopt_kept_turn_runs():
  {==}

def Laws.adopt_kept_never_carried():
  {==}

def Laws.adopt_not_kept_carried():
  {==}
# ---- end agent-adopt ----
# ---- hub-followups: stuck turns ----
def Laws.stuck_ends_ok():
  {==}

def Laws.stuck_ends_failed_or_stopped():
  {==}

def Laws.stuck_waits():
  {==}

def Laws.stuck_step_ends_turn():
  {==}
# ---- end hub-followups: stuck turns ----
# win-lean-frame: cat frames
law keep_k_cat:
  for b: Bool
  for +o: R.Op
  for +x: List<&2, R.Op>
  for +y: List<&2, R.Op>
  {R.Region.keep.k(b, o, T.Ops.cat(x, y)) == T.Ops.cat(R.Region.keep.k(b, o, x), y) : List<&2, R.Op>}

def keep_k_cat(b, o, x, y):
  match b:
    case True{}:
      {==}
    case False{}:
      {==}

law keep_op_cat:
  for hit: Bool
  for +x0: U32
  for +y0: U32
  for +x1: U32
  for +y1: U32
  for +c: U32
  for +a: U32
  for +s: R.Shape
  for +px: U32
  for +py: U32
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +x: List<&2, R.Op>
  for +y: List<&2, R.Op>
  {R.Region.keep.op(hit, R.Op{x0, y0, x1, y1, c, a, s}, px, py, rx, ry, n, T.Ops.cat(x, y)) == T.Ops.cat(R.Region.keep.op(hit, R.Op{x0, y0, x1, y1, c, a, s}, px, py, rx, ry, n, x), y) : List<&2, R.Op>}

def keep_op_cat(hit, x0, y0, x1, y1, c, a, s, px, py, rx, ry, n, x, y):
  match hit:
    case True{}:
      keep_k_cat(Bool.not(R.Region.empty(R.Region.move(s, px, py, rx, ry, n))), R.Op{x0, y0, x1, y1, c, a, R.Region.move(s, px, py, rx, ry, n)}, x, y)
    case False{}:
      {==}

def Laws.keep_cat(a, b, px, py, rx, ry, n):
  match a:
    case Nil{}:
      {==}
    case R.Op{+x0, +y0, +x1, +y1, +c, +al, +s} <> +t:
      ih = Laws.keep_cat(t, b, px, py, rx, ry, n)
      %Equal.sym(List<&2, R.Op>, R.Region.keep(T.Ops.cat(t, b), px, py, rx, ry, n), T.Ops.cat(R.Region.keep(t, px, py, rx, ry, n), R.Region.keep(b, px, py, rx, ry, n)), ih) : {R.Region.keep.op(R.Region.hits(x0, y0, x1, y1, rx, ry, n), R.Op{x0, y0, x1, y1, c, al, s}, px, py, rx, ry, n, _) == T.Ops.cat(R.Region.keep.op(R.Region.hits(x0, y0, x1, y1, rx, ry, n), R.Op{x0, y0, x1, y1, c, al, s}, px, py, rx, ry, n, R.Region.keep(t, px, py, rx, ry, n)), R.Region.keep(b, px, py, rx, ry, n)) : List<&2, R.Op>}
      keep_op_cat(R.Region.hits(x0, y0, x1, y1, rx, ry, n), x0, y0, x1, y1, c, al, s, px, py, rx, ry, n, R.Region.keep(t, px, py, rx, ry, n), R.Region.keep(b, px, py, rx, ry, n))

def Laws.ops_after_cat(cs, rest):
  match cs:
    case Nil{}:
      {==}
    case +x <> +t:
      Laws.ops_after_cat(t, rest)

# one quadrant: the law one level down (ih), the quadrant's ops split
# into its cats and its rest (keep_cat)
law recat_child:
  for +j: Nat
  for +e: Nat
  for +cs: List<&2, R.Op>
  for +cs2: List<&2, R.Op>
  for +rest: List<&2, R.Op>
  for +bq: Image
  for +px: U32
  for +py: U32
  for +cx: U32
  for +cy: U32
  for +h: U32
  for ih: {T.Tiles.recat(j, e, R.Region.keep(cs, px, py, cx, cy, h), R.Region.keep(cs2, px, py, cx, cy, h), T.Tiles.make(j, e, T.Ops.cat(R.Region.keep(cs, px, py, cx, cy, h), R.Region.keep(rest, px, py, cx, cy, h)), bq, cx, cy, h), cx, cy, h) == T.Tiles.make(j, e, T.Ops.cat(R.Region.keep(cs2, px, py, cx, cy, h), R.Region.keep(rest, px, py, cx, cy, h)), bq, cx, cy, h) : T.Tiles}
  {T.Tiles.recat(j, e, R.Region.keep(cs, px, py, cx, cy, h), R.Region.keep(cs2, px, py, cx, cy, h), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs, rest), px, py, cx, cy, h), bq, cx, cy, h), cx, cy, h) == T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs2, rest), px, py, cx, cy, h), bq, cx, cy, h) : T.Tiles}

def recat_child(j, e, cs, cs2, rest, bq, px, py, cx, cy, h, ih):
  e1 = Laws.keep_cat(cs, rest, px, py, cx, cy, h)
  e2 = Laws.keep_cat(cs2, rest, px, py, cx, cy, h)
  %Equal.sym(List<&2, R.Op>, R.Region.keep(T.Ops.cat(cs, rest), px, py, cx, cy, h), T.Ops.cat(R.Region.keep(cs, px, py, cx, cy, h), R.Region.keep(rest, px, py, cx, cy, h)), e1) : {T.Tiles.recat(j, e, R.Region.keep(cs, px, py, cx, cy, h), R.Region.keep(cs2, px, py, cx, cy, h), T.Tiles.make(j, e, _, bq, cx, cy, h), cx, cy, h) == T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs2, rest), px, py, cx, cy, h), bq, cx, cy, h) : T.Tiles}
  %Equal.sym(List<&2, R.Op>, R.Region.keep(T.Ops.cat(cs2, rest), px, py, cx, cy, h), T.Ops.cat(R.Region.keep(cs2, px, py, cx, cy, h), R.Region.keep(rest, px, py, cx, cy, h)), e2) : {T.Tiles.recat(j, e, R.Region.keep(cs, px, py, cx, cy, h), R.Region.keep(cs2, px, py, cx, cy, h), T.Tiles.make(j, e, T.Ops.cat(R.Region.keep(cs, px, py, cx, cy, h), R.Region.keep(rest, px, py, cx, cy, h)), bq, cx, cy, h), cx, cy, h) == T.Tiles.make(j, e, _, bq, cx, cy, h) : T.Tiles}
  ih

law recat_node_k:
  for s: Bool
  for +j: Nat
  for +e: Nat
  for +cs: List<&2, R.Op>
  for +cs2: List<&2, R.Op>
  for +rest: List<&2, R.Op>
  for +base: Image
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +h: {T.Eq.ops(cs, cs2) == s : Bool}
  for ha: {T.Tiles.recat(j, e, R.Region.keep(cs, rx, ry, rx, ry, U32.shr(n)), R.Region.keep(cs2, rx, ry, rx, ry, U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs, rest), rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)), rx, ry, U32.shr(n)) == T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs2, rest), rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)) : T.Tiles}
  for hb: {T.Tiles.recat(j, e, R.Region.keep(cs, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), R.Region.keep(cs2, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs, rest), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)), (rx + U32.shr(n) : U32), ry, U32.shr(n)) == T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs2, rest), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)) : T.Tiles}
  for hc: {T.Tiles.recat(j, e, R.Region.keep(cs, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), R.Region.keep(cs2, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs, rest), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)), rx, (ry + U32.shr(n) : U32), U32.shr(n)) == T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs2, rest), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)) : T.Tiles}
  for hd: {T.Tiles.recat(j, e, R.Region.keep(cs, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), R.Region.keep(cs2, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs, rest), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)) == T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs2, rest), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)) : T.Tiles}
  {T.Tiles.recat.go(1n+j, e, cs, cs2, T.Tiles.make(1n+j, e, T.Ops.cat(cs, rest), base, rx, ry, n), rx, ry, n, s) == T.Tiles.make(1n+j, e, T.Ops.cat(cs2, rest), base, rx, ry, n) : T.Tiles}

def recat_node_k(s, j, e, cs, cs2, rest, base, rx, ry, n, h, ha, hb, hc, hd):
  match s:
    case True{}:
      ec = eq_ops_sound(cs, cs2, h)
      %ec : {T.Tiles.make(1n+j, e, T.Ops.cat(cs, rest), base, rx, ry, n) == T.Tiles.make(1n+j, e, T.Ops.cat(_, rest), base, rx, ry, n) : T.Tiles}
      {==}
    case False{}:
      ea = Laws.ops_after_cat(cs, rest)
      %Equal.sym(List<&2, R.Op>, T.Ops.after(cs, T.Ops.cat(cs, rest)), rest, ea) : {T.TNode{T.Ops.cat(cs2, _), base, T.Tiles.recat(j, e, R.Region.keep(cs, rx, ry, rx, ry, U32.shr(n)), R.Region.keep(cs2, rx, ry, rx, ry, U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs, rest), rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)), rx, ry, U32.shr(n)), T.Tiles.recat(j, e, R.Region.keep(cs, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), R.Region.keep(cs2, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs, rest), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)), (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Tiles.recat(j, e, R.Region.keep(cs, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), R.Region.keep(cs2, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs, rest), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)), rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Tiles.recat(j, e, R.Region.keep(cs, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), R.Region.keep(cs2, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs, rest), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n))} == T.Tiles.make(1n+j, e, T.Ops.cat(cs2, rest), base, rx, ry, n) : T.Tiles}
      tnode_cong(T.Ops.cat(cs2, rest), base,
        T.Tiles.recat(j, e, R.Region.keep(cs, rx, ry, rx, ry, U32.shr(n)), R.Region.keep(cs2, rx, ry, rx, ry, U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs, rest), rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)), rx, ry, U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs2, rest), rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n)),
        T.Tiles.recat(j, e, R.Region.keep(cs, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), R.Region.keep(cs2, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs, rest), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)), (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs2, rest), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)),
        T.Tiles.recat(j, e, R.Region.keep(cs, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), R.Region.keep(cs2, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs, rest), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)), rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs2, rest), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)),
        T.Tiles.recat(j, e, R.Region.keep(cs, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), R.Region.keep(cs2, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs, rest), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Tiles.make(j, e, R.Region.keep(T.Ops.cat(cs2, rest), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)),
        ha, hb, hc, hd)

law recat_leaf_k:
  for s: Bool
  for +e: Nat
  for +cs: List<&2, R.Op>
  for +cs2: List<&2, R.Op>
  for +rest: List<&2, R.Op>
  for +base: Image
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +h: {T.Eq.ops(cs, cs2) == s : Bool}
  {T.Tiles.recat.go(0n, e, cs, cs2, T.Tiles.make(0n, e, T.Ops.cat(cs, rest), base, rx, ry, n), rx, ry, n, s) == T.Tiles.make(0n, e, T.Ops.cat(cs2, rest), base, rx, ry, n) : T.Tiles}

def recat_leaf_k(s, e, cs, cs2, rest, base, rx, ry, n, h):
  match s:
    case True{}:
      ec = eq_ops_sound(cs, cs2, h)
      %ec : {T.Tiles.make(0n, e, T.Ops.cat(cs, rest), base, rx, ry, n) == T.Tiles.make(0n, e, T.Ops.cat(_, rest), base, rx, ry, n) : T.Tiles}
      {==}
    case False{}:
      ea = Laws.ops_after_cat(cs, rest)
      %Equal.sym(List<&2, R.Op>, T.Ops.after(cs, T.Ops.cat(cs, rest)), rest, ea) : {T.TLeaf{T.Tile.of(e, T.Ops.cat(cs2, _), T.Loc.ops(T.Ops.cat(cs2, _), rx, ry, T.Tile.size(e)), base)} == T.Tiles.make(0n, e, T.Ops.cat(cs2, rest), base, rx, ry, n) : T.Tiles}
      {==}

def Laws.tiles_recat_exact(k, e, cs, cs2, rest, base, rx, ry, n):
  match k:
    case 0n:
      recat_leaf_k(T.Eq.ops(cs, cs2), e, cs, cs2, rest, base, rx, ry, n, {==})
    case 1n+j0:
      +j = j0
      recat_node_k(T.Eq.ops(cs, cs2), j, e, cs, cs2, rest, base, rx, ry, n, {==},
        recat_child(j, e, cs, cs2, rest, T.Img.q(base, 0n), rx, ry, rx, ry, U32.shr(n), Laws.tiles_recat_exact(j, e, R.Region.keep(cs, rx, ry, rx, ry, U32.shr(n)), R.Region.keep(cs2, rx, ry, rx, ry, U32.shr(n)), R.Region.keep(rest, rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), rx, ry, U32.shr(n))),
        recat_child(j, e, cs, cs2, rest, T.Img.q(base, 1n), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n), Laws.tiles_recat_exact(j, e, R.Region.keep(cs, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), R.Region.keep(cs2, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), R.Region.keep(rest, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n))),
        recat_child(j, e, cs, cs2, rest, T.Img.q(base, 2n), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n), Laws.tiles_recat_exact(j, e, R.Region.keep(cs, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), R.Region.keep(cs2, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), R.Region.keep(rest, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n))),
        recat_child(j, e, cs, cs2, rest, T.Img.q(base, 3n), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n), Laws.tiles_recat_exact(j, e, R.Region.keep(cs, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), R.Region.keep(cs2, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), R.Region.keep(rest, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n))))
# ---- end win-lean-frame cat frames ----

# ---- gfx-runs: tile plans that build only changed nodes' ops (laws keep_eq,
# plan_cut_same), and frame_par_exact over them (kept last: a proof checked
# before Laws.when_in_hours changed how deep that one's 8200n compare ran) ----
law ops_go_false:
  for xs: List<&2, R.Op>
  for ys: List<&2, R.Op>
  {T.Eq.ops.go(xs, ys, False{}) == False{} : Bool}

def ops_go_false(xs, ys):
  match xs ys:
    case Nil{} Nil{}:
      {==}
    case Nil{} _ <> _:
      {==}
    case _ <> _ Nil{}:
      {==}
    case _ <> _ _ <> _:
      {==}

law eq_op_f:
  for +x0: U32
  for +y0: U32
  for +x1: U32
  for +y1: U32
  for +c: U32
  for +a: U32
  for +s: R.Shape
  for y: R.Op
  {T.Eq.op.f(x0, y0, x1, y1, c, a, s, y) == T.Eq.op(R.Op{x0, y0, x1, y1, c, a, s}, y) : Bool}

def eq_op_f(x0, y0, x1, y1, c, a, s, y):
  match y:
    case R.Op{+b0, +b1, +b2, +b3, +b4, +b5, +b6}:
      {==}

law keep_step_k:
  for k: Bool
  for +s2: R.Shape
  for +x0: U32
  for +y0: U32
  for +x1: U32
  for +y1: U32
  for +c: U32
  for +a: U32
  for +rest: List<&2, R.Op>
  for ys: List<&2, R.Op>
  {T.Eq.ops.go(rest, T.Keep.ys.k(k, ys), T.Keep.ok.k(k, x0, y0, x1, y1, c, a, s2, ys)) == T.Eq.ops.go(R.Region.keep.k(k, R.Op{x0, y0, x1, y1, c, a, s2}, rest), ys, True{}) : Bool}

def keep_step_k(k, s2, x0, y0, x1, y1, c, a, rest, ys):
  match k ys:
    case False{} _:
      {==}
    case True{} Nil{}:
      ops_go_false(rest, Nil{})
    case True{} +y <> +yt:
      %eq_op_f(x0, y0, x1, y1, c, a, s2, y) : {T.Eq.ops.go(rest, yt, T.Eq.op.f(x0, y0, x1, y1, c, a, s2, y)) == T.Eq.ops.go(rest, yt, _) : Bool}
      {==}

law keep_step:
  for hit: Bool
  for +x0: U32
  for +y0: U32
  for +x1: U32
  for +y1: U32
  for +c: U32
  for +a: U32
  for +s: R.Shape
  for +px: U32
  for +py: U32
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +rest: List<&2, R.Op>
  for +ys: List<&2, R.Op>
  {T.Eq.ops.go(rest, T.Keep.ys.h(hit, s, px, py, rx, ry, n, ys), T.Keep.ok.h(hit, x0, y0, x1, y1, c, a, s, px, py, rx, ry, n, ys)) == T.Eq.ops.go(R.Region.keep.op(hit, R.Op{x0, y0, x1, y1, c, a, s}, px, py, rx, ry, n, rest), ys, True{}) : Bool}

def keep_step(hit, x0, y0, x1, y1, c, a, s, px, py, rx, ry, n, rest, ys):
  match hit:
    case True{}:
      keep_step_k(Bool.not(R.Region.empty(R.Region.move(s, px, py, rx, ry, n))), R.Region.move(s, px, py, rx, ry, n), x0, y0, x1, y1, c, a, rest, ys)
    case False{}:
      {==}

def Laws.keep_eq(ops, px, py, rx, ry, n, ys, ok):
  match ops ok:
    case Nil{} False{}:
      Equal.sym(Bool, T.Eq.ops.go(Nil{}, ys, False{}), False{}, ops_go_false(Nil{}, ys))
    case +o <> +t False{}:
      Equal.sym(Bool, T.Eq.ops.go(R.Region.keep(o <> t, px, py, rx, ry, n), ys, False{}), False{}, ops_go_false(R.Region.keep(o <> t, px, py, rx, ry, n), ys))
    case Nil{} True{}:
      {==}
    case R.Op{+x0, +y0, +x1, +y1, +c, +a, +s} <> +t True{}:
      Equal.trans(Bool, T.Keep.eq(t, px, py, rx, ry, n, T.Keep.ys.h(R.Region.hits(x0, y0, x1, y1, rx, ry, n), s, px, py, rx, ry, n, ys), T.Keep.ok.h(R.Region.hits(x0, y0, x1, y1, rx, ry, n), x0, y0, x1, y1, c, a, s, px, py, rx, ry, n, ys)),
        T.Eq.ops.go(R.Region.keep(t, px, py, rx, ry, n), T.Keep.ys.h(R.Region.hits(x0, y0, x1, y1, rx, ry, n), s, px, py, rx, ry, n, ys), T.Keep.ok.h(R.Region.hits(x0, y0, x1, y1, rx, ry, n), x0, y0, x1, y1, c, a, s, px, py, rx, ry, n, ys)),
        T.Eq.ops.go(R.Region.keep.op(R.Region.hits(x0, y0, x1, y1, rx, ry, n), R.Op{x0, y0, x1, y1, c, a, s}, px, py, rx, ry, n, R.Region.keep(t, px, py, rx, ry, n)), ys, True{}),
        Laws.keep_eq(t, px, py, rx, ry, n, T.Keep.ys.h(R.Region.hits(x0, y0, x1, y1, rx, ry, n), s, px, py, rx, ry, n, ys), T.Keep.ok.h(R.Region.hits(x0, y0, x1, y1, rx, ry, n), x0, y0, x1, y1, c, a, s, px, py, rx, ry, n, ys)),
        keep_step(R.Region.hits(x0, y0, x1, y1, rx, ry, n), x0, y0, x1, y1, c, a, s, px, py, rx, ry, n, R.Region.keep(t, px, py, rx, ry, n), ys))

law cut_eq:
  for c: T.Cut
  for +ys: List<&2, R.Op>
  {T.Cut.eq(c, ys) == T.Eq.ops(T.Cut.get(c), ys) : Bool}

def cut_eq(c, ys):
  match c:
    case T.CAll{+o}:
      {==}
    case T.CIn{+o, +px, +py, +rx, +ry, +n}:
      Laws.keep_eq(o, px, py, rx, ry, n, ys, True{})

law cut_tile:
  for +c: T.Cut
  for +ys: List<&2, R.Op>
  for +a: Image
  for +b: Image
  {T.Cut.tile(c, ys, a, b) == T.Eq.tile(T.Cut.get(c), ys, a, b) : Bool}

def cut_tile(c, ys, a, b):
  %cut_eq(c, ys) : {T.Eq.tile.k(T.Cut.eq(c, ys), a, b) == T.Eq.tile.k(_, a, b) : Bool}
  {==}

law cut_same:
  for +c: T.Cut
  for +base: Image
  for t: T.Tiles
  {T.Cut.same(c, base, t) == T.Tiles.same(T.Cut.get(c), base, t) : Bool}

def cut_same(c, base, t):
  match t:
    case T.TLeaf{+x}:
      {==}
    case T.TNode{+o, +b, +pa, +pb, +pc, +pd}:
      cut_tile(c, o, base, b)

law tile_plan2_k:
  for s: Bool
  for +e: Nat
  for +c: T.Cut
  for +base: Image
  for +t: T.Tile
  for +p: T.Past
  for +rx: U32
  for +ry: U32
  for +n: U32
  {T.Tile.plan2.k(s, e, c, base, t, p, rx, ry, n) == T.Tile.plan.k(s, e, T.Cut.get(c), base, t, p, rx, ry, n) : T.Plan}

def tile_plan2_k(s, e, c, base, t, p, rx, ry, n):
  match s:
    case True{}:
      {==}
    case False{}:
      {==}

law tile_plan2:
  for +e: Nat
  for +c: T.Cut
  for +base: Image
  for t: T.Tile
  for +p: T.Past
  for +rx: U32
  for +ry: U32
  for +n: U32
  {T.Tile.plan2(e, c, base, t, p, rx, ry, n) == T.Tile.plan(e, T.Cut.get(c), base, t, p, rx, ry, n) : T.Plan}

def tile_plan2(e, c, base, t, p, rx, ry, n):
  match t:
    case T.Tile{+o, +l, +b, +i}:
      %cut_tile(c, o, base, b) : {T.Tile.plan2.k(T.Cut.tile(c, o, base, b), e, c, base, T.Tile{o, l, b, i}, p, rx, ry, n) == T.Tile.plan.k(_, e, T.Cut.get(c), base, T.Tile{o, l, b, i}, p, rx, ry, n) : T.Plan}
      tile_plan2_k(T.Cut.tile(c, o, base, b), e, c, base, T.Tile{o, l, b, i}, p, rx, ry, n)

law plan2_child:
  for +j: Nat
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +px: U32
  for +py: U32
  for +cx: U32
  for +cy: U32
  for +h: U32
  for +q: Image
  for +pa: T.Tiles
  for +p: T.Past
  for ih: {T.Tiles.plan2.go(j, e, T.CIn{ops, px, py, cx, cy, h}, q, pa, p, cx, cy, h, T.Cut.same(T.CIn{ops, px, py, cx, cy, h}, q, pa)) == T.Tiles.plan.go(j, e, R.Region.keep(ops, px, py, cx, cy, h), q, pa, p, cx, cy, h, T.Cut.same(T.CIn{ops, px, py, cx, cy, h}, q, pa)) : T.Plan}
  {T.Tiles.plan2.go(j, e, T.CIn{ops, px, py, cx, cy, h}, q, pa, p, cx, cy, h, T.Cut.same(T.CIn{ops, px, py, cx, cy, h}, q, pa)) == T.Tiles.plan.go(j, e, R.Region.keep(ops, px, py, cx, cy, h), q, pa, p, cx, cy, h, T.Tiles.same(R.Region.keep(ops, px, py, cx, cy, h), q, pa)) : T.Plan}

def plan2_child(j, e, ops, px, py, cx, cy, h, q, pa, p, ih):
  %cut_same(T.CIn{ops, px, py, cx, cy, h}, q, pa) : {T.Tiles.plan2.go(j, e, T.CIn{ops, px, py, cx, cy, h}, q, pa, p, cx, cy, h, T.Cut.same(T.CIn{ops, px, py, cx, cy, h}, q, pa)) == T.Tiles.plan.go(j, e, R.Region.keep(ops, px, py, cx, cy, h), q, pa, p, cx, cy, h, _) : T.Plan}
  ih

law plan_node_cong:
  for -o: List<&2, R.Op>
  for -bs: Image
  for -a1: T.Plan
  for -a2: T.Plan
  for -b1: T.Plan
  for -b2: T.Plan
  for -c1: T.Plan
  for -c2: T.Plan
  for -d1: T.Plan
  for -d2: T.Plan
  for ea: {a1 == a2 : T.Plan}
  for eb: {b1 == b2 : T.Plan}
  for ec: {c1 == c2 : T.Plan}
  for ed: {d1 == d2 : T.Plan}
  {T.Plan.node(o, bs, a1, b1, c1, d1) == T.Plan.node(o, bs, a2, b2, c2, d2) : T.Plan}

def plan_node_cong(o, bs, a1, a2, b1, b2, c1, c2, d1, d2, ea, eb, ec, ed):
  %ea : {T.Plan.node(o, bs, a1, b1, c1, d1) == T.Plan.node(o, bs, _, b2, c2, d2) : T.Plan}
  %eb : {T.Plan.node(o, bs, a1, b1, c1, d1) == T.Plan.node(o, bs, a1, _, c2, d2) : T.Plan}
  %ec : {T.Plan.node(o, bs, a1, b1, c1, d1) == T.Plan.node(o, bs, a1, b1, _, d2) : T.Plan}
  %ed : {T.Plan.node(o, bs, a1, b1, c1, d1) == T.Plan.node(o, bs, a1, b1, c1, _) : T.Plan}
  {==}

law plan2_node:
  for +j: Nat
  for +e: Nat
  for +c: T.Cut
  for +base: Image
  for +o: List<&2, R.Op>
  for +b: Image
  for +pa: T.Tiles
  for +pb: T.Tiles
  for +pc: T.Tiles
  for +pd: T.Tiles
  for +p: T.Past
  for +rx: U32
  for +ry: U32
  for +n: U32
  for ha: {T.Tiles.plan2.go(j, e, T.CIn{T.Cut.get(c), rx, ry, rx, ry, U32.shr(n)}, T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n), T.Cut.same(T.CIn{T.Cut.get(c), rx, ry, rx, ry, U32.shr(n)}, T.Img.q(base, 0n), pa)) == T.Tiles.plan.go(j, e, R.Region.keep(T.Cut.get(c), rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n), T.Cut.same(T.CIn{T.Cut.get(c), rx, ry, rx, ry, U32.shr(n)}, T.Img.q(base, 0n), pa)) : T.Plan}
  for hb: {T.Tiles.plan2.go(j, e, T.CIn{T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)}, T.Img.q(base, 1n), pb, p, (rx + U32.shr(n) : U32), ry, U32.shr(n), T.Cut.same(T.CIn{T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)}, T.Img.q(base, 1n), pb)) == T.Tiles.plan.go(j, e, R.Region.keep(T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), pb, p, (rx + U32.shr(n) : U32), ry, U32.shr(n), T.Cut.same(T.CIn{T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)}, T.Img.q(base, 1n), pb)) : T.Plan}
  for hc: {T.Tiles.plan2.go(j, e, T.CIn{T.Cut.get(c), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)}, T.Img.q(base, 2n), pc, p, rx, (ry + U32.shr(n) : U32), U32.shr(n), T.Cut.same(T.CIn{T.Cut.get(c), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)}, T.Img.q(base, 2n), pc)) == T.Tiles.plan.go(j, e, R.Region.keep(T.Cut.get(c), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), pc, p, rx, (ry + U32.shr(n) : U32), U32.shr(n), T.Cut.same(T.CIn{T.Cut.get(c), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)}, T.Img.q(base, 2n), pc)) : T.Plan}
  for hd: {T.Tiles.plan2.go(j, e, T.CIn{T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)}, T.Img.q(base, 3n), pd, p, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n), T.Cut.same(T.CIn{T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)}, T.Img.q(base, 3n), pd)) == T.Tiles.plan.go(j, e, R.Region.keep(T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), pd, p, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n), T.Cut.same(T.CIn{T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)}, T.Img.q(base, 3n), pd)) : T.Plan}
  {T.Tiles.plan2.go(1n+j, e, c, base, T.TNode{o, b, pa, pb, pc, pd}, p, rx, ry, n, False{}) == T.Tiles.plan.go(1n+j, e, T.Cut.get(c), base, T.TNode{o, b, pa, pb, pc, pd}, p, rx, ry, n, False{}) : T.Plan}

def plan2_node(j, e, c, base, o, b, pa, pb, pc, pd, p, rx, ry, n, ha, hb, hc, hd):
  plan_node_cong(T.Cut.get(c), base, T.Tiles.plan2.go(j, e, T.CIn{T.Cut.get(c), rx, ry, rx, ry, U32.shr(n)}, T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n), T.Cut.same(T.CIn{T.Cut.get(c), rx, ry, rx, ry, U32.shr(n)}, T.Img.q(base, 0n), pa)),
    T.Tiles.plan.go(j, e, R.Region.keep(T.Cut.get(c), rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n), T.Tiles.same(R.Region.keep(T.Cut.get(c), rx, ry, rx, ry, U32.shr(n)), T.Img.q(base, 0n), pa)),
    T.Tiles.plan2.go(j, e, T.CIn{T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)}, T.Img.q(base, 1n), pb, p, (rx + U32.shr(n) : U32), ry, U32.shr(n), T.Cut.same(T.CIn{T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)}, T.Img.q(base, 1n), pb)),
    T.Tiles.plan.go(j, e, R.Region.keep(T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), pb, p, (rx + U32.shr(n) : U32), ry, U32.shr(n), T.Tiles.same(R.Region.keep(T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), T.Img.q(base, 1n), pb)),
    T.Tiles.plan2.go(j, e, T.CIn{T.Cut.get(c), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)}, T.Img.q(base, 2n), pc, p, rx, (ry + U32.shr(n) : U32), U32.shr(n), T.Cut.same(T.CIn{T.Cut.get(c), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)}, T.Img.q(base, 2n), pc)),
    T.Tiles.plan.go(j, e, R.Region.keep(T.Cut.get(c), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), pc, p, rx, (ry + U32.shr(n) : U32), U32.shr(n), T.Tiles.same(R.Region.keep(T.Cut.get(c), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 2n), pc)),
    T.Tiles.plan2.go(j, e, T.CIn{T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)}, T.Img.q(base, 3n), pd, p, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n), T.Cut.same(T.CIn{T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)}, T.Img.q(base, 3n), pd)),
    T.Tiles.plan.go(j, e, R.Region.keep(T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), pd, p, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n), T.Tiles.same(R.Region.keep(T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), T.Img.q(base, 3n), pd)),
    plan2_child(j, e, T.Cut.get(c), rx, ry, rx, ry, U32.shr(n), T.Img.q(base, 0n), pa, p, ha),
    plan2_child(j, e, T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n), T.Img.q(base, 1n), pb, p, hb),
    plan2_child(j, e, T.Cut.get(c), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n), T.Img.q(base, 2n), pc, p, hc),
    plan2_child(j, e, T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n), T.Img.q(base, 3n), pd, p, hd))

law plan2_go:
  for k: Nat
  for +e: Nat
  for +c: T.Cut
  for +base: Image
  for prev: T.Tiles
  for +p: T.Past
  for +rx: U32
  for +ry: U32
  for +n: U32
  for s: Bool
  {T.Tiles.plan2.go(k, e, c, base, prev, p, rx, ry, n, s) == T.Tiles.plan.go(k, e, T.Cut.get(c), base, prev, p, rx, ry, n, s) : T.Plan}

def plan2_go(k, e, c, base, prev, p, rx, ry, n, s):
  match k prev s:
    case 0n T.TLeaf{+t} _:
      tile_plan2(e, c, base, t, p, rx, ry, n)
    case 0n T.TNode{+o, +b, +pa, +pb, +pc, +pd} _:
      {==}
    case 1n+j T.TLeaf{+t} _:
      {==}
    case 1n+j0 T.TNode{+o, +b, +pa, +pb, +pc, +pd} True{}:
      {==}
    case 1n+j0 T.TNode{+o, +b, +pa, +pb, +pc, +pd} False{}:
      +j = j0
      plan2_node(j, e, c, base, o, b, pa, pb, pc, pd, p, rx, ry, n,
        plan2_go(j, e, T.CIn{T.Cut.get(c), rx, ry, rx, ry, U32.shr(n)}, T.Img.q(base, 0n), pa, p, rx, ry, U32.shr(n), T.Cut.same(T.CIn{T.Cut.get(c), rx, ry, rx, ry, U32.shr(n)}, T.Img.q(base, 0n), pa)),
        plan2_go(j, e, T.CIn{T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)}, T.Img.q(base, 1n), pb, p, (rx + U32.shr(n) : U32), ry, U32.shr(n), T.Cut.same(T.CIn{T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)}, T.Img.q(base, 1n), pb)),
        plan2_go(j, e, T.CIn{T.Cut.get(c), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)}, T.Img.q(base, 2n), pc, p, rx, (ry + U32.shr(n) : U32), U32.shr(n), T.Cut.same(T.CIn{T.Cut.get(c), rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)}, T.Img.q(base, 2n), pc)),
        plan2_go(j, e, T.CIn{T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)}, T.Img.q(base, 3n), pd, p, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n), T.Cut.same(T.CIn{T.Cut.get(c), rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)}, T.Img.q(base, 3n), pd)))

def Laws.plan_cut_same(k, e, ops, base, prev, p, rx, ry, n):
  %cut_same(T.CAll{ops}, base, prev) : {T.Tiles.plan2.go(k, e, T.CAll{ops}, base, prev, p, rx, ry, n, T.Cut.same(T.CAll{ops}, base, prev)) == T.Tiles.plan.go(k, e, ops, base, prev, p, rx, ry, n, _) : T.Plan}
  plan2_go(k, e, T.CAll{ops}, base, prev, p, rx, ry, n, T.Cut.same(T.CAll{ops}, base, prev))

law plan_run_cut:
  for k: Nat
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +base: Image
  for +prev: T.Tiles
  for +p: T.Past
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +d: Nat
  {T.Plan.run(T.Tiles.plan2(k, e, ops, base, prev, p, rx, ry, n), d) == T.Plan.run(T.Tiles.plan(k, e, ops, base, prev, p, rx, ry, n), d) : T.Tiles}

def plan_run_cut(k, e, ops, base, prev, p, rx, ry, n, d):
  Equal.cong(T.Plan, T.Tiles, q => T.Plan.run(q, d), T.Tiles.plan2(k, e, ops, base, prev, p, rx, ry, n), T.Tiles.plan(k, e, ops, base, prev, p, rx, ry, n), Laws.plan_cut_same(k, e, ops, base, prev, p, rx, ry, n))

law frame_par_k:
  for s: Bool
  for +w: U32
  for +h: U32
  for +ops: List<&2, R.Op>
  for +base: Image
  for +prev: T.Tiles
  for +dy: U32
  {T.Plan.run(T.Frame.plan.k(s, w, h, ops, base, prev, dy), T.Frame.edge(w, h)) == T.Frame.again.k(s, w, h, ops, base, prev, dy) : T.Tiles}

def frame_par_k(s, w, h, ops, base, prev, dy):
  match s:
    case True{}:
      Equal.trans(T.Tiles, T.Plan.run(T.Tiles.plan2(T.Frame.levels(w, h), T.Frame.edge(w, h), ops, base, prev, T.Past{T.Frame.levels(w, h), prev, U32.shln(1, R.Frame.depth(w, h)), dy}, 0, 0, U32.shln(1, R.Frame.depth(w, h))), T.Frame.edge(w, h)), T.Plan.run(T.Tiles.plan(T.Frame.levels(w, h), T.Frame.edge(w, h), ops, base, prev, T.Past{T.Frame.levels(w, h), prev, U32.shln(1, R.Frame.depth(w, h)), dy}, 0, 0, U32.shln(1, R.Frame.depth(w, h))), T.Frame.edge(w, h)), T.Frame.again.k(True{}, w, h, ops, base, prev, dy),
        plan_run_cut(T.Frame.levels(w, h), T.Frame.edge(w, h), ops, base, prev, T.Past{T.Frame.levels(w, h), prev, U32.shln(1, R.Frame.depth(w, h)), dy}, 0, 0, U32.shln(1, R.Frame.depth(w, h)), T.Frame.edge(w, h)),
        Laws.plan_fill_again(T.Frame.levels(w, h), T.Frame.edge(w, h), ops, base, prev, T.Past{T.Frame.levels(w, h), prev, U32.shln(1, R.Frame.depth(w, h)), dy}, 0, 0, U32.shln(1, R.Frame.depth(w, h)), 0n,
          T.Jobs.deal(T.Plan.jobs(T.Tiles.plan(T.Frame.levels(w, h), T.Frame.edge(w, h), ops, base, prev, T.Past{T.Frame.levels(w, h), prev, U32.shln(1, R.Frame.depth(w, h)), dy}, 0, 0, U32.shln(1, R.Frame.depth(w, h))), Nil{}))))
    case False{}:
      plan_make_fill(T.Frame.levels(w, h), T.Frame.edge(w, h), ops, base, 0, 0, U32.shln(1, R.Frame.depth(w, h)), 0n,
        T.Jobs.deal(T.Plan.jobs(T.Tiles.plan.make(T.Frame.levels(w, h), T.Frame.edge(w, h), ops, base, 0, 0, U32.shln(1, R.Frame.depth(w, h))), Nil{})))

def Laws.frame_par_exact(w, h, ops, base, prev, dy):
  match prev:
    case T.FNone{}:
      plan_make_fill(T.Frame.levels(w, h), T.Frame.edge(w, h), ops, base, 0, 0, U32.shln(1, R.Frame.depth(w, h)), 0n,
        T.Jobs.deal(T.Plan.jobs(T.Tiles.plan.make(T.Frame.levels(w, h), T.Frame.edge(w, h), ops, base, 0, 0, U32.shln(1, R.Frame.depth(w, h))), Nil{})))
    case T.FSome{pw, ph, pt}:
      frame_par_k(Bool.and(U32.is_eq(pw, w), U32.is_eq(ph, h)), w, h, ops, base, pt, dy)
# ---- end gfx-runs ----
# ---- sent-draft ----
def Laws.draft_clear_tells_host(u):
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      {==}

def Laws.draft_clear_empties():
  {==}

def Laws.draft_send_forgets_first(u, text, mode):
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      {==}

def Laws.draft_bot_say_forgets_first(u, bot, text):
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      {==}

def Laws.draft_btw_forgets_first(u, q):
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      {==}

def Laws.draft_give_forgets_first(u, text):
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      {==}

def Laws.draft_stale_same():
  {==}

def Laws.draft_stale_with_attachments():
  {==}

def Laws.draft_stale_longer_is_not():
  {==}

def Laws.draft_stale_empty_never(said):
  {==}

def Laws.draft_stale_last():
  {==}
# ---- end sent-draft ----

# ---- mail 1 ----
def Laws.mail_route_person_empty(from, to):
  {==}

def Laws.mail_route_thread_pair():
  {==}

def Laws.mail_route_launch(child, to):
  {==}

def Laws.mail_route_room():
  {==}

def Laws.mail_route_system(to):
  {==}

def Laws.mail_want_completed_fyi(own, direct, mentioned):
  {==}

def Laws.mail_want_failed_act():
  {==}

def Laws.mail_want_given_wins(kind, flags):
  {==}

def Laws.mail_want_given_act(kind, flags):
  {==}

def Laws.mail_want_room_fyi():
  {==}

def Laws.mail_want_mention_act():
  {==}

def Laws.mail_want_own_request():
  {==}

def Laws.mail_want_wake_act():
  {==}

def Laws.mail_want_lost_act():
  {==}

def Laws.mail_want_deploy_own():
  {==}

def Laws.mail_want_lock_own():
  {==}

def Laws.mail_want_status_fyi():
  {==}

def Laws.mail_want_routine_act():
  {==}

def Laws.mail_want_hook_act():
  {==}

def Laws.mail_want_direct_act():
  {==}

def Laws.mail_want_person_act():
  {==}

def Laws.mail_post_idempotent(b, m):
  {==}

def Laws.mail_unread_after_post():
  {==}

def Laws.mail_origin_keeps_outputs(st, thread, msg, origin, at):
  match st:
    case H.Step{cs, os}:
      {==}

# ---- end mail 1 ----
# ---- links (ref) ----

def Laws.ref_uri_parse_thread():
  {==}

def Laws.ref_uri_parse_bot():
  {==}

def Laws.ref_uri_parse_room():
  {==}

def Laws.ref_uri_parse_person():
  {==}

def Laws.ref_uri_parse_project():
  {==}

def Laws.ref_uri_parse_routine():
  {==}

def Laws.ref_uri_parse_task():
  {==}

def Laws.ref_uri_parse_pr():
  {==}

def Laws.ref_uri_parse_work():
  {==}

def Laws.ref_uri_parse_file():
  {==}

def Laws.ref_uri_parse_machine():
  {==}

def Laws.ref_uri_parse_conv():
  {==}

def Laws.ref_uri_parse_mail():
  {==}

def Laws.ref_uri_parse_mailroom():
  {==}

def Laws.ref_uri_parse_decision():
  {==}

def Laws.ref_uri_parse_charter():
  {==}

def Laws.ref_web_parse():
  {==}

def Laws.ref_web_parse_slash():
  {==}

def Laws.ref_addr_roundtrip_thread():
  {==}

def Laws.ref_addr_roundtrip_bot():
  {==}

def Laws.ref_addr_roundtrip_room():
  {==}

def Laws.ref_addr_roundtrip_person():
  {==}

def Laws.ref_addr_roundtrip_mail():
  {==}

def Laws.ref_addr_roundtrip_conv():
  {==}

def Laws.ref_thread_addr(id):
  {==}

def Laws.ref_pct_safe():
  {==}

def Laws.ref_pct_bytes():
  {==}

def Laws.ref_pct_three():
  {==}

def Laws.ref_pct_four():
  {==}

def Laws.ref_pct_encoding():
  {==}

def Laws.ref_pct_malformed():
  {==}

def Laws.ref_unknown_kind():
  {==}

def Laws.ref_no_script(s):
  {==}

def Laws.ref_legacy_open():
  {==}

def Laws.ref_bare_thread():
  {==}

def Laws.ref_bare_bot():
  {==}

def Laws.ref_bare_task():
  {==}

def Laws.ref_bare_project():
  {==}

def Laws.ref_bare_room():
  {==}

def Laws.ref_bare_routine():
  {==}

def Laws.ref_bare_remote():
  {==}

def Laws.ref_bare_not_short():
  {==}

def Laws.ref_bare_not_word():
  {==}

def Laws.ref_bare_not_incomplete():
  {==}

def Laws.ref_bare_not_unknown():
  {==}

def Laws.link_ref_uri():
  {==}

def Laws.link_ref_bare():
  {==}

def Laws.link_ref_md():
  {==}

def Laws.link_ref_plain_unchanged():
  {==}

# ---- i18n ----

law i18n_ago_same_k:
  for m: Bool
  for h: Bool
  for d: Bool
  for +s: Nat
  {I18.I18n.ago.en(m, h, d, s) == M.Time.ago.k(m, h, d, s) : String}

def i18n_ago_same_k(m, h, d, s):
  match m h d:
    case True{} _ _: {==}
    case False{} True{} _: {==}
    case False{} False{} True{}: {==}
    case False{} False{} False{}: {==}

law i18n_took_same_k:
  for m: Bool
  for h: Bool
  for +s: Nat
  {I18.I18n.took.en(m, h, s) == M.Time.took.k(m, h, s) : String}

def i18n_took_same_k(m, h, s):
  match m h:
    case True{} _: {==}
    case False{} True{}: {==}
    case False{} False{}: {==}

def Laws.i18n_plural_ja_other(n):
  {==}

def Laws.i18n_plural_en_one():
  {==}

def Laws.i18n_plural_en_zero():
  {==}

def Laws.i18n_plural_en_many(k):
  {==}

def Laws.i18n_resolve_override_en(os):
  {==}

def Laws.i18n_resolve_override_ja(os):
  {==}

def Laws.i18n_resolve_auto(os):
  {==}

def Laws.i18n_of_tag_posix():
  {==}

def Laws.i18n_of_tag_c():
  {==}

def Laws.i18n_of_tag_empty():
  {==}

def Laws.i18n_of_tag_bcp():
  {==}

def Laws.i18n_ago_en_same(now, at):
  i18n_ago_same_k(Nat.is_lt(Nat.sub(now, at), 60n), Nat.is_lt(Nat.sub(now, at), 3600n), Nat.is_lt(Nat.sub(now, at), 86400n), Nat.sub(now, at))

def Laws.i18n_took_en_same(s):
  i18n_took_same_k(Nat.is_lt(s, 60n), Nat.is_lt(s, 3600n), s)


def Laws.i18n_fallback_fixture_save():
  {==}

def Laws.i18n_fallback_fixture_plural(n):
  {==}
# ---- end i18n ----
# ---- usage-screen ----
def Laws.usage_tail_na():
  {==}

def Laws.usage_tail_claude_waits():
  {==}

def Laws.usage_tail_reading(p, t):
  {==}

def Laws.usage_note_held():
  {==}

def Laws.usage_note_no_window():
  {==}

def Laws.usage_pct_no_window(u):
  {==}

def Laws.usage_boat_due_never_read(now):
  {==}

def Laws.usage_boat_usd_exact():
  {==}

# ---- links (cards, trail) ----
def Laws.cards_get_fresh():
  {==}

def Laws.cards_get_fresh_settle():
  {==}

def Laws.cards_get_fresh_start():
  {==}

def Laws.cards_get_fresh_end():
  {==}

def Laws.cards_get_fresh_delete():
  {==}

def Laws.cards_get_fresh_project():
  {==}

def Laws.cards_peek_after_get():
  {==}

def Laws.cards_drop_other_kept():
  {==}

def Laws.trail_cap():
  {==}

def Laws.card_file_kind_shared():
  {==}

def Laws.trail_go_back(bs, fs):
  {==}

def Laws.trail_back_fwd(bs, b, c, fs):
  {==}

def Laws.trail_go_same(bs, fs):
  {==}

def Laws.trail_go_clears_fwd(bs, fs):
  {==}

def Laws.card_thread_no_timeline(es):
  {==}

def Laws.card_github_pr_no_state(st):
  {==}

def Laws.card_person_no_state(st):
  {==}
# ---- i18n cjk ----
def Laws.cjk_segs_lossless():
  {==}

def Laws.cjk_segs_lossless_mixed():
  {==}

def Laws.cjk_segs_lossless_spaces():
  {==}

def Laws.cjk_segs_lossless_empty():
  {==}

def Laws.cjk_no_start_stop():
  {==}

def Laws.cjk_no_end_quote():
  {==}

def Laws.cjk_kana_break():
  {==}

def Laws.cjk_ascii_no_break():
  {==}

def Laws.cjk_terminal_cells():
  {==}

def Laws.cjk_terminal_combining_cells():
  {==}

# ---- end i18n cjk ----

# ---- usage-tokens ----
def Laws.tok_claude_seen_nothing():
  {==}

def Laws.tok_rise_rose():
  {==}

def Laws.tok_rise_restarted():
  {==}

def Laws.tok_codex_restart_whole(was, i, v):
  {==}

def Laws.tok_add_one_turn():
  {==}

def Laws.tok_add_no_cost():
  {==}

def Laws.card_task_status_agrees():
  {==}

# ---- mail 2a ----

# Reversing onto a suffix is exactly reverse followed by append. The
# implementation may therefore emit its final newline without Base's ++.
def MailProof.reverse_suffix(+s: String, +acc: String, +rest: String) -> {String.reverse.go(s, acc ++ rest) == String.reverse.go(s, acc) ++ rest : String}:
  match s:
    case SNil{}: {==}
    case SCon{h, t}: MailProof.reverse_suffix(t, SCon{h, acc}, rest)

def Laws.body_line_terminated(b):
  MailProof.reverse_suffix(J.Json.emit.rev(MB.Body.encode(b), True{}, SNil{}), "", "\n")

def Laws.body_header_omits_text(c, m, a, b):
  {==}

def Laws.recover_seen_empty(b, box, seen):
  %Equal.sym(Bool, ML.Box.has(box, ML.Mail.id(MB.Body.mail(b))), True{}, seen) : {MB.Body.recover.guard(_, b, box) == Nil{} : List<&2, M.Change>}
  {==}

def Laws.recover_missing_header(c, m, text):
  {==}

def Laws.body_path_encoded_segment(home, conv):
  {==}

# ---- usage-boat-cap ----
def Laws.usage_boat_no_cap_nothing():
  {==}
# ---- agent failures: stderr ----
def Laws.errtail_scrub_token():
  {==}

def Laws.errtail_scrub_words():
  {==}

def Laws.errtail_say_exit():
  {==}

def Laws.errtail_say_signal():
  {==}
# ---- end agent failures: stderr ----
# ---- agent failures: the login keyring ----
def Laws.keyring_locked_never_starts():
  {==}

def Laws.keyring_unknown_or_open_starts():
  {==}

def Laws.keyring_answers():
  {==}

def Laws.keyring_hint_only_locked():
  {==}
# ---- end agent failures: the login keyring ----
# ---- riding mail ----
def Laws.ride_running_no_restart():
  {==}

def Laws.ride_posted_at_once():
  {==}

def Laws.ride_kept():
  {==}

def Laws.ride_carries_block():
  {==}

def Laws.ride_taken_empties():
  {==}

def Laws.ride_nothing_waiting_same(o, s, now, th, h):
  %Equal.sym(String, H.Ride.of(s, th), "", h) : {BHS.Ride.mcp.k(Bool.and(Bool.not(String.is_empty(_)), H.Hub.running(H.Hub.thread(s, th))), o, s, now, th) == o : MC.McpOut}
  {==}

def Laws.ride_turn_end_starts():
  {==}

def Laws.ride_interrupted_starts():
  {==}

def Laws.ride_turn_end_empties():
  {==}

def Laws.ride_after_running_same(s, now, th, os):
  {==}

# ---- end riding mail ----
# ---- resume after a restart ----
def Laws.resume_turn_process_stopped():
  {==}

def Laws.resume_turn_process_not_kept():
  {==}

def Laws.resume_carry_tells_cut():
  {==}
# ---- end resume after a restart ----
# ---- deploy waits for MCP calls ----
def Laws.deploy_mcp_call_waits():
  {==}

def Laws.deploy_mcp_answered_goes():
  {==}

def Laws.deploy_mcp_own_call_not_counted():
  {==}

def Laws.deploy_mcp_outlives_turn_end():
  {==}
# ---- end deploy waits for MCP calls ----
# ---- i18n ime ----

def Laws.ime_preedit_not_in_draft():
  {==}

def Laws.ime_shown_at_caret():
  {==}

def Laws.ime_range_replace_chars():
  {==}

def Laws.ime_delete_range():
  {==}

def Laws.ime_wide_counts_once():
  {==}

def Laws.ime_clear_is_off():
  {==}

def Laws.ime_focus_change_drops():
  {==}

def Laws.ime_owner_keeps():
  {==}

def Laws.ime_other_owner_never_merges():
  {==}

def Laws.ime_unknown_event_not_quit():
  {==}

def Laws.ime_unknown_event_is_junk():
  {==}

def Laws.ime_quit_is_quit():
  {==}

def Laws.ime_commit_event_id():
  {==}

# ---- end i18n ime ----

# ---- ordering ----

def Laws.order_buy_needs_parts():
  {==}

def Laws.order_buy_priced():
  {==}

def Laws.order_buy_unreachable_hidden():
  {==}

def Laws.order_buy_estimates_off():
  {==}

def Laws.intent_edit_round_trip():
  {==}

def Laws.intent_edit_is_intent():
  {==}

def Laws.order_tools_buy_only():
  {==}

def Laws.order_ready_names():
  {==}

def Laws.order_ready_board():
  {==}

def Laws.order_ready_nothing():
  {==}

def Laws.fab_ready_running_hidden(line):
  {==}

def Laws.fab_ready_needs_buy(line):
  {==}

def Laws.order_review_none():
  {==}

def Laws.order_review_current():
  {==}

def Laws.order_review_stale():
  {==}

def Laws.order_review_verdict_closed():
  {==}

def Laws.bd_name_folder():
  {==}

def Laws.bd_name_root():
  {==}

def Laws.bd_schematic_beside():
  {==}

def Laws.bd_board_itself():
  {==}

def Laws.bd_on_sheet():
  {==}

def Laws.bd_off_other():
  {==}

def Laws.bd_keep_schematic():
  {==}

def Laws.bd_keep_none(kind):
  {==}

def Laws.bd_shown_default():
  {==}

def Laws.bd_shown_picked(root, own):
  {==}

def Laws.buy_title_first(t):
  {==}

def Laws.buy_title_second():
  {==}

def Laws.order_ordered_paid():
  {==}

def Laws.order_ordered_quote_only(line):
  {==}

def Laws.fab_buy_again():
  {==}

def Laws.fab_buy_again_off():
  {==}

def Laws.fab_min_key():
  {==}

def Laws.fab_min_hides_bar(line):
  {==}

def Laws.fab_min_keeps_bar(line):
  {==}

def Laws.fab_min_header():
  {==}

def Laws.fab_min_header_shown_bar(line):
  {==}

def Laws.fab_min_header_nothing():
  {==}

def Laws.fab_min_header_ordered():
  {==}

def Laws.buy_server_default():
  {==}

def Laws.buy_server_set():
  {==}

def Laws.order_note_shipped():
  {==}

def Laws.order_note_expired(t):
  {==}

def Laws.order_state_cancelled(q):
  {==}

def Laws.order_state_paid():
  {==}

def Laws.fab_buy_off_hidden(label):
  {==}

def Laws.order_off_no_tools(buy):
  {==}

def Laws.order_off_by_default():
  {==}

def Laws.order_review_issues_kept():
  {==}

def Laws.order_review_issues_at_most():
  {==}

def Laws.order_ask_survives_retire():
  {==}

def Laws.order_approval_retired():
  {==}

def Laws.order_gate_passed():
  {==}

def Laws.order_gate_stale(v):
  {==}

def Laws.order_gate_none(v):
  {==}

def Laws.order_gate_issues():
  {==}

def Laws.order_pilot_paid():
  {==}

def Laws.order_pilot_off(st):
  {==}

def Laws.order_pilot_done():
  {==}

def Laws.order_pilot_said():
  {==}

def Laws.order_pilot_quiet():
  {==}

def Laws.order_gate_lists_issues():
  {==}

def Laws.order_gate_cuts_summary():
  {==}

def Laws.order_ask_alert_plain(q):
  {==}

def Laws.tool_summary_question():
  {==}

def Laws.order_off_says_where():
  {==}

def Laws.order_buy_board_only():
  {==}

def Laws.order_buy_board_unreachable():
  {==}

# ---- i18n lang ----
def Laws.lang_default_auto():
  {==}
def Laws.lang_default_unknown():
  {==}
def Laws.lang_gate(os):
  {==}
def Laws.lang_gate_preview(os):
  {==}
def Laws.lang_auto_preview():
  {==}
def Laws.lang_override_en(os, p):
  {==}
def Laws.prompt_lang_off():
  {==}
def Laws.prompt_lang_unknown():
  {==}
# ---- end i18n lang ----

# ---- mail view ----
def Laws.mv_visible_bounds(total, scroll, view):
  Laws.menu_scroll_in_list(MVW.Mv.first_raw(scroll), total, MVW.Mv.count(total, scroll, view), le_ge(MVW.Mv.count(total, scroll, view), total, min_le_k(Nat.is_le(MVW.Mv.want(scroll, view), total), MVW.Mv.want(scroll, view), total, {==})))

def Laws.mv_merge_no_dup():
  {==}

def Laws.mv_rows_stable():
  {==}

def Laws.mv_filter_key_separates():
  {==}

# ---- end mail view ----

# ---- usage-baseline ----
def Laws.tok_cost_unknown_without_start():
  {==}

def Laws.tok_cost_known_at_start(b):
  {==}

def Laws.tok_codex_start_zero():
  {==}

def Laws.tok_epoch_once():
  {==}

def Laws.tok_pre_epoch():
  {==}
# ---- mail index ----
def Laws.mail_index_wrong_position():
  {==}

def Laws.mail_index_forward_chain_rejected():
  {==}

def Laws.mail_index_overflow_rejected():
  {==}

def Laws.mail_index_hash_empty(h):
  {==}
# ---- end mail index ----
# ---- scroll-stable: reading while a thread grows ----
def Laws.hold_at_end_zero():
  {==}

def Laws.hold_label_nothing():
  {==}

def Laws.hold_fresh_seen(e, t, n):
  %Equal.sym(Bool, String.eq(M.Idx.id(e), M.Idx.id(e)), True{}, Laws.string_eq_refl(M.Idx.id(e))) : {U.Hold.fresh(e <> t, Bool.or(_, Nat.is_ge(n, U.Hold.most())), M.Idx.id(e), n) == n : Nat}
  {==}

def Laws.hold_off_still(off, t0, t1, over, h):
  %Equal.sym(Bool, Nat.is_le(Nat.sub(Nat.add(off, t1), t0), over), True{}, h) : {Bool.pick(Nat, _, Nat.sub(Nat.add(off, t1), t0), over) == Nat.sub(Nat.add(off, t1), t0) : Nat}
  {==}

def Laws.hold_end_follows(hold, off, top, over):
  {==}
# ---- end scroll-stable ----

def Laws.hold_boundary_zero():
  {==}

def Laws.hold_entry_moves(off, y0, y1, t0, t1, over, h):
  Laws.hold_off_still(Nat.add(Nat.sub(off, y0), y1), t0, t1, over, h)

def Laws.hold_page_free(es, base):
  {==}

# ---- usage-claude-resume ----
def Laws.tok_claude_start_zero():
  {==}

def Laws.tok_claude_same_session_keeps(s):
  {==}

def Laws.tok_recount_once():
  {==}

# ---- usage-boat-chip ----
def Laws.chip_none(now):
  {==}

def Laws.chip_running():
  {==}

def Laws.chip_warm_gone():
  {==}

def Laws.chip_warm_not_stale(gen, now):
  {==}

# ---- deploy main ----
def Laws.ship_covered_never_builds(c, cs, busy):
  match busy:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.ship_busy_never_builds(cover):
  {==}

def Laws.ship_building_is_busy():
  {==}

def Laws.ship_coalesce_at_most_one(sh, state, commit):
  match sh:
    case SHP.Ship{p, c, s, l, j, a, n, m}:
      match m:
        case True{}:
          {==}
        case False{}:
          {==}

def Laws.ship_coalesce_three():
  {==}

def Laws.ship_coalesce_askers():
  {==}

def Laws.ship_told_each_once():
  {==}

def Laws.ship_told_cleared():
  {==}

def Laws.ship_told_next_cleared(sh, state, commit):
  match sh:
    case SHP.Ship{p, c, s, l, j, a, n, m}:
      match m:
        case True{}:
          {==}
        case False{}:
          {==}

def Laws.ship_covered_joins_build():
  {==}

def Laws.ship_follow_every_minute(): {==}

def Laws.ship_follow_off_default():
  {==}

def Laws.ship_gone_not_ok():
  {==}
# ---- end deploy main ----
# ---- kicad-ipc ----
def Laws.kicad_status_missing(r, f, v, a):
  {==}

def Laws.kicad_status_broken(f, v, a):
  {==}

def Laws.kicad_status_stock(v, a):
  {==}

def Laws.kicad_notice_ok_none(c, v, w, i):
  {==}

def Laws.kicad_notice_checking_none(c, v, w, i):
  {==}

def Laws.kicad_notice_has_retry_missing(c, v, w, i):
  {==}

def Laws.kicad_notice_install_iff_missing(c, v, w, i):
  match i:
    case True{}: {==}
    case False{}: {==}

def Laws.kicad_notice_has_retry_stock(c, v, w, i):
  {==}

def Laws.kicad_notice_install_iff_stock(c, v, w, i):
  match i:
    case True{}: {==}
    case False{}: {==}

def Laws.kicad_notice_has_retry_old(c, v, w, i):
  {==}

def Laws.kicad_notice_install_iff_old(c, v, w, i):
  match i:
    case True{}: {==}
    case False{}: {==}

def Laws.kicad_notice_has_retry_broken(c, v, w, i):
  {==}

def Laws.kicad_notice_install_iff_broken(c, v, w, i):
  match i:
    case True{}: {==}
    case False{}: {==}

def Laws.prompt_kicad_ipc_only_ok_missing(i):
  match i:
    case True{}: {==}
    case False{}: {==}

def Laws.prompt_kicad_ipc_only_ok_stock(i):
  match i:
    case True{}: {==}
    case False{}: {==}

def Laws.prompt_kicad_ipc_only_ok_old(i):
  match i:
    case True{}: {==}
    case False{}: {==}

def Laws.prompt_kicad_ipc_only_ok_broken(i):
  match i:
    case True{}: {==}
    case False{}: {==}

def Laws.prompt_kicad_ipc_only_ok_checking(i):
  match i:
    case True{}: {==}
    case False{}: {==}

def Laws.mm_late_misses(key, ms, now, h):
  %Equal.sym(Bool, Nat.is_ge(now, MM.MainM.due(ms)), True{}, h) : {MM.Mm.use.k(_, MM.MainM.same(key, ms)) == False{} : Bool}
  {==}
# ---- mail 2b ----
law MailProof.append_nil:
  for +cs: List<&2, M.Change>
  {List.append(&2, M.Change, cs, Nil{}) == cs : List<&2, M.Change>}

def MailProof.append_nil(cs):
  match cs:
    case Nil{}: {==}
    case h <> t:
      %MailProof.append_nil(t) : {h <> List.append(&2, M.Change, t, Nil{}) == h <> _ : List<&2, M.Change>}
      {==}

law MailProof.keeps:
  for on: Bool
  for +st: H.Step
  for s: M.State
  for now: Nat
  for o: ML.Origin
  for to: String
  for text: String
  for msg: String
  {H.Queue.changes(H.Mail.tee.on(on, st, s, now, o, to, text, msg)) == H.Queue.changes(st) : List<&2, M.Change>}

def MailProof.keeps(on, st, s, now, o, to, text, msg):
  match on st:
    case False{} _: {==}
    case True{} H.Step{cs, os}: MailProof.append_nil(cs)

law MailProof.pending_one:
  for missing: Bool
  for c: ML.Conv
  for m: ML.Mail
  for text: String
  {H.Mail.pending.count(H.Mail.pending(missing, MB.Body{c, m, text})) == 1n : Nat}

def MailProof.pending_one(missing, c, m, text):
  match missing c m:
    case True{} ML.Conv{_, _, _, _, _, _, _, _, _} ML.Mail{_, _, _, _, _, _, _, _, _, _}: {==}
    case False{} _ ML.Mail{_, _, _, _, _, _, _, _, _, _}: {==}

def Laws.mail_off_unchanged(st, s, now, o, to, text, msg, off):
  %Equal.sym(String, M.Setting.get(s, "mail.on", "off"), "off", off) : {H.Mail.tee.on(Bool.and(String.eq(_, "on"), Bool.not(String.eq(ML.Origin.kind(o), "person"))), st, s, now, o, to, text, msg) == st : H.Step}
  {==}

def Laws.mail_file_never_waits(s, now, o, to, text, msg):
  {==}

def Laws.mail_tee_keeps_delivery(st, s, now, o, to, text, msg):
  MailProof.keeps(Bool.and(String.eq(M.Setting.get(s, "mail.on", "off"), "on"), Bool.not(String.eq(ML.Origin.kind(o), "person"))), st, s, now, o, to, text, msg)

law MailProof.pending_route:
  for missing: Bool
  for c: ML.Conv
  for +m: ML.Mail
  for text: String
  {H.Mail.pending.conv(H.Mail.pending(missing, MB.Body{c, m, text})) == ML.Mail.conv(m) : String}

def MailProof.pending_route(missing, c, m, text):
  match missing c m:
    case True{} ML.Conv{_, _, _, _, _, _, _, _, _} ML.Mail{_, _, _, _, _, _, _, _, _, _}: {==}
    case False{} _ ML.Mail{_, _, _, _, _, _, _, _, _, _}: {==}

def Laws.mail_file_one_conv(s, now, o, to, text, msg):
  +body = H.Mail.body(s, now, o, to, text, msg, ML.Mail.route(o, to), H.Mail.subject(String.eq(ML.Origin.kind(o), "launch"), s, o, to), "")
  +missing = String.is_empty(ML.Conv.id(ML.Box.conv(M.State.mail(s), ML.Mail.conv(MB.Body.mail(body)))))
  Equal.trans(String, H.Mail.pending.conv(H.Mail.pending(missing, body)), ML.Mail.conv(MB.Body.mail(body)), ML.Mail.route(o, to),
    MailProof.pending_route(missing, MB.Body.conv(body), MB.Body.mail(body), MB.Body.text(body)), {==})

def Laws.mail_room_once(s, now, from, room, text, msg, parts, subject):
  MailProof.pending_one(String.is_empty(ML.Conv.id(ML.Box.conv(M.State.mail(s), ML.Mail.route(ML.Origin{from, "room", room}, MREF.Ref.addr(MREF.Ref.room(room)))))),
    ML.Conv{ML.Mail.route(ML.Origin{from, "room", room}, MREF.Ref.addr(MREF.Ref.room(room))), subject, ML.Conv.mail_kind(ML.Mail.route(ML.Origin{from, "room", room}, MREF.Ref.addr(MREF.Ref.room(room))), "room"), parts, "", Nil{}, now, now, 0n},
    ML.Mail{msg, ML.Mail.route(ML.Origin{from, "room", room}, MREF.Ref.addr(MREF.Ref.room(room))), from, MREF.Ref.addr(MREF.Ref.room(room)), "room", room, "fyi", ML.Mail.head(text), H.Mail.bytes(text, 0n), now}, text)
# ---- i18n core ui ----
def Laws.title_shown_ja_new():
  {==}
def Laws.title_shown_en_new():
  {==}
def Laws.title_shown_other():
  {==}
def Laws.tl_entries_locale(ty, es, st, sel, px, pics):
  %Equal.sym(List<&2, M.Entry>, L.TL.memo.es(L.TL.memo.make(ty, es, px, sel)), es, tl_memo_es(ty, es, px, sel))
    : {L.TL.entries.mr(ty, L.TL.memo.make(ty, es, px, sel), L.TL.roles.loc(L.Ty.loc(ty), _, st, sel, pics), st, sel, px, pics, L.TL.stamps(_, st, sel, pics)) == L.TL.entries.loc(ty, es, st, sel, px, pics) : List<&2, L.TLine>}
  tl_entries_roles(ty, es, L.TL.roles.loc(L.Ty.loc(ty), es, st, sel, pics), st, sel, px, pics, L.TL.stamps(es, st, sel, pics))
# ---- end i18n core ui ----
# ---- kicad-ipc 2 ----
def Laws.kserve_argv_parent_pid_with_idle():
  {==}

def Laws.kserve_argv_parent_pid_without_idle():
  {==}

def Laws.kserve_argv_socket_in_dir():
  {==}

def Laws.kserve_argv_idle_with_cap():
  {==}

def Laws.kserve_argv_no_idle_without_cap():
  {==}

def Laws.kserve_ready_line():
  {==}

def Laws.kserve_ready_other_line():
  {==}

def Laws.kserve_version_ok_exact():
  {==}

def Laws.kserve_version_ok_describe():
  {==}

def Laws.kserve_version_other_build():
  {==}

def Laws.kserve_version_longer_build():
  {==}

def Laws.kserve_version_not_fork():
  {==}

def Laws.kserve_retry_first():
  {==}

def Laws.kserve_retry_second():
  {==}

def Laws.kserve_retry_third():
  {==}

def Laws.kserve_retry_gives_up():
  {==}

def Laws.kserve_retry_old_failures_forgotten():
  {==}

def Laws.kserve_clean_exit_not_a_failure():
  {==}

def Laws.kserve_crash_is_a_failure():
  {==}

def Laws.kserve_clean_exit_stops():
  {==}

def Laws.kserve_three_crashes_restart():
  {==}

def Laws.kserve_fourth_crash_fails():
  {==}

def Laws.kserve_evict_least_recent():
  {==}

def Laws.kserve_no_evict_with_room():
  {==}

def Laws.kserve_third_project_evicts():
  {==}

def Laws.kserve_third_project_busy_without_shutdown():
  {==}

def Laws.kserve_running_server_is_used():
  {==}

def Laws.kserve_refused_selection():
  {==}

def Laws.kserve_refused_view():
  {==}

def Laws.kserve_refused_run_action():
  {==}

def Laws.kserve_get_items_allowed():
  {==}

def Laws.kserve_eighteen_refused():
  {==}

def Laws.kserve_gate_off():
  {==}

def Laws.kserve_gate_ok():
  {==}

def Laws.kserve_unavailable_names_fallback():
  {==}

def Laws.kicad_min_version():
  {==}

def Laws.kicad_old_runtime_is_old():
  {==}

def Laws.kicad_new_runtime_is_ok():
  {==}

def Laws.prompt_kicad_ok_names_kicad_ipc():
  {==}

def Laws.prompt_kicad_ok_no_pcbnew_import():
  {==}

def Laws.prompt_kicad_ok_starts_no_server():
  {==}

def Laws.prompt_kicad_off_names_fallback():
  {==}

def Laws.kserve_desc_needs_cap(root): {==}
def Laws.kserve_desc_with_cap(root): {==}

def Laws.kserve_idle_needs_api2(xs): {==}

# ---- end kicad-ipc 2 ----
# ---- kicad-ipc 3 ----

def Laws.kexport_no_unmarked_clear(base): {==}
def Laws.kexport_no_unconsented_force(report): {==}
def Laws.kexport_block_force(): {==}
def Laws.kexport_lossy_force(): {==}
def Laws.kexport_no_outside_clear(): {==}

def Laws.kexport_no_foreign_base(base, marker): {==}

# ---- mail 2c ----
def Laws.foh_clean(cs):
  match cs:
    case Nil{}: {==}
    case M.ProjectCreated{_, _, _, _} <> +t: Laws.foh_clean(t)
    case M.ProjectRemoved{_, _} <> +t: Laws.foh_clean(t)
    case M.ThreadCreated{_, _, _, _, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.ThreadRenamed{_, _, _} <> +t: Laws.foh_clean(t)
    case M.MessagePosted{_, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.TurnChanged{_, _, _} <> +t: Laws.foh_clean(t)
    case M.PendingChanged{_, _, _} <> +t: Laws.foh_clean(t)
    case M.ActivityLogged{_, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.ThreadSettled{_, _, _} <> +t: Laws.foh_clean(t)
    case M.ThreadUnsettled{_, _} <> +t: Laws.foh_clean(t)
    case M.ThreadArchived{_, _, _} <> +t: Laws.foh_clean(t)
    case M.ThreadPinned{_, _, _} <> +t: Laws.foh_clean(t)
    case M.ThreadDeleted{_, _} <> +t: Laws.foh_clean(t)
    case M.WorktreeSet{_, _, _, _} <> +t: Laws.foh_clean(t)
    case M.SessionSet{_, _, _} <> +t: Laws.foh_clean(t)
    case M.ModesSet{_, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.TaskDelegated{_, _, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.TaskStateSet{_, _, _, _} <> +t: Laws.foh_clean(t)
    case M.TaskDelivered{_, _} <> +t: Laws.foh_clean(t)
    case M.SettingSet{_, _, _} <> +t: Laws.foh_clean(t)
    case M.TurnQueued{_, _, _, _} <> +t: Laws.foh_clean(t)
    case M.QueueTaken{_, _} <> +t: Laws.foh_clean(t)
    case M.AskOpened{_, _, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.AskClosed{_, _, _} <> +t: Laws.foh_clean(t)
    case M.DeviceRegistered{_, _, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.DeviceDropped{_, _} <> +t: Laws.foh_clean(t)
    case M.ThreadSnoozed{_, _, _} <> +t: Laws.foh_clean(t)
    case M.ThreadOrdered{_, _, _, _} <> +t: Laws.foh_clean(t)
    case M.WorktreeSkipped{_, _} <> +t: Laws.foh_clean(t)
    case M.ThreadMerged{_, _, _} <> +t: Laws.foh_clean(t)
    case M.WorktreeRemoved{_, _} <> +t: Laws.foh_clean(t)
    case M.ProjectOrdered{_, _, _} <> +t: Laws.foh_clean(t)
    case M.QueueEdited{_, _, _, _} <> +t: Laws.foh_clean(t)
    case M.QueueRemoved{_, _, _} <> +t: Laws.foh_clean(t)
    case M.QueueMoved{_, _, _, _} <> +t: Laws.foh_clean(t)
    case M.QueueTakenOne{_, _, _} <> +t: Laws.foh_clean(t)
    case M.ThreadForked{_, _, _} <> +t: Laws.foh_clean(t)
    case M.ProviderSet{_, _, _, _} <> +t: Laws.foh_clean(t)
    case M.TodosSet{_, _, _} <> +t: Laws.foh_clean(t)
    case M.UsageSet{_, _, _, _} <> +t: Laws.foh_clean(t)
    case M.BotSet{_, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.BotDeleted{_, _} <> +t: Laws.foh_clean(t)
    case M.RoomSet{_, _, _, _} <> +t: Laws.foh_clean(t)
    case M.RoomDeleted{_, _} <> +t: Laws.foh_clean(t)
    case M.RoomPosted{_, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.HookSet{_, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.HookHit{_, _} <> +t: Laws.foh_clean(t)
    case M.RoutineSet{_, _, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.RoutineDeleted{_, _} <> +t: Laws.foh_clean(t)
    case M.RoutineFired{_, _} <> +t: Laws.foh_clean(t)
    case M.MemorySaved{_, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.MemoryForgot{_, _, _} <> +t: Laws.foh_clean(t)
    case M.SpaceSet{_, _, _} <> +t: Laws.foh_clean(t)
    case M.PeerSet{_, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.BotWoke{_, _, _} <> +t: Laws.foh_clean(t)
    case M.SpacePageSet{_, _, _} <> +t: Laws.foh_clean(t)
    case M.WorkSet{_, _, _, _} <> +t: Laws.foh_clean(t)
    case M.UberSet{_, _, _} <> +t: Laws.foh_clean(t)
    case M.UberOp{_, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.ThreadLaunched{_, _, _, _} <> +t: Laws.foh_clean(t)
    case M.WakeSet{_, _, _, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.WakeDropped{_, _} <> +t: Laws.foh_clean(t)
    case M.LockChanged{_, _, _} <> +t: Laws.foh_clean(t)
    case M.RoutinePlan{_, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.RoutineSlot{_, _, _} <> +t: Laws.foh_clean(t)
    case M.RoutineDone{_, _} <> +t: Laws.foh_clean(t)
    case M.Reminded{_, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.TaskSet{_, _, _, _} <> +t: Laws.foh_clean(t)
    case M.TaskDeleted{_, _, _} <> +t: Laws.foh_clean(t)
    case M.MessageFrom{_, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.ConvSet{_, _, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.MailPosted{_, _, _, _, _, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.MailMarked{_, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.MailTagged{_, _, _, _} <> +t: Laws.foh_clean(t)
    case M.RunSet{_, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.CharterSet{_, _, _, _} <> +t: Laws.foh_clean(t)
    case M.DecisionLogged{_, _, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.TurnUsage{_, _, _, _, _, _, _, _, _, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.DeployAsked{_, _, _, _, _} <> +t: Laws.foh_clean(t)
    case M.DeployBuilt{_, _, _, _, _} <> +t: Laws.foh_clean(t)

def Laws.fyi_never_spawns(live): {==}

def Laws.act_one_run(live):
  match live:
    case SNil{}: {==}
    case SCon{_, _}: {==}

def Laws.run_fresh(th, cwd, provider, model, session, runtime, interaction): {==}

def Laws.run_one_live_per_conv(): {==}

def Laws.escalate_only_path(cs, os): Laws.foh_clean(cs)

def Laws.charter_gate_denies(text, tool, input, risky, unlisted):
  %Equal.sym(Bool, CH.Charter.risky(tool, input), True{}, risky) : {Bool.or(Bool.not(_), CH.Charter.allows(text, tool, input)) == False{} : Bool}
  %Equal.sym(Bool, CH.Charter.allows(text, tool, input), False{}, unlisted) : {Bool.or(Bool.not(True{}), _) == False{} : Bool}
  {==}

def Laws.charter_gate_allows_listed(text, tool, input, listed):
  %Equal.sym(Bool, CH.Charter.allows(text, tool, input), True{}, listed) : {Bool.or(Bool.not(CH.Charter.risky(tool, input)), _) == True{} : Bool}
  or_true_r(Bool.not(CH.Charter.risky(tool, input)))

def Laws.charter_risky_fixed(): {==}
# ---- i18n window ----
def Laws.window_notice_english():
  {==}
def Laws.window_notice_words_kept():
  {==}
def Laws.window_tools_english(h, sa, sv, ta, tv):
  {==}
def Laws.window_picker_default():
  {==}
def Laws.window_provider_missing():
  {==}
# ---- end i18n window ----

# --- browser-cef ---
def Laws.cef_engine_needs_setting(known, ok):
  {==}

def Laws.cef_engine_needs_embed(ok):
  {==}

def Laws.cef_engine_needs_runtime(known):
  match known:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.cef_engine_on():
  {==}

def Laws.cef_embed_window_here():
  {==}

def Laws.cef_embed_loopback_link():
  {==}

def Laws.cef_embed_remote_refused():
  {==}

def Laws.cef_loopback_v4():
  {==}

def Laws.cef_loopback_v6():
  {==}

def Laws.cef_tailnet_not_loopback():
  {==}

def Laws.cef_leave_clears_embed():
  {==}

def Laws.cef_leave_other_keeps_embed():
  {==}

def Laws.cef_embed_bad_xid_unknown():
  {==}

def Laws.cef_embed_once_per_join():
  {==}

def Laws.cef_embed_new_join():
  {==}

def Laws.cef_embed_not_remote():
  {==}

def Laws.cef_embed_needs_xid():
  {==}

def Laws.cef_shield_agent_drives():
  {==}

def Laws.cef_shield_takeover_off():
  {==}

def Laws.cef_shield_teach_off():
  {==}

def Laws.cef_shield_focus_opposite(d):
  {==}

def Laws.cef_shield_take_from_agent():
  {==}

def Laws.cef_shield_take_not_from_person():
  {==}

def Laws.cef_shield_due_same():
  {==}

def Laws.cef_shield_due_first():
  {==}

def Laws.cef_shield_due_person():
  {==}

def Laws.cef_covered_none():
  {==}

def Laws.cef_covered_palette():
  {==}

def Laws.cef_covered_picker():
  {==}

def Laws.cef_covered_menu():
  {==}

def Laws.cef_covered_look():
  {==}

def Laws.cef_covered_models():
  {==}

def Laws.cef_covered_tour():
  {==}

def Laws.cef_covered_perf():
  {==}

def Laws.cef_covered_lightbox():
  {==}

def Laws.cef_shows_needs_live(covered):
  {==}

def Laws.cef_shows_not_covered():
  {==}

def Laws.cef_place_same_nothing():
  {==}

def Laws.cef_place_hidden_nothing():
  {==}

def Laws.cef_place_new_shows():
  {==}

def Laws.cef_place_moved_shows():
  {==}

def Laws.cef_place_gone_hides():
  {==}

def Laws.cef_post_chrome_idle():
  {==}

def Laws.cef_post_chrome_forgets():
  {==}

def Laws.cef_post_native_places():
  {==}

def Laws.cef_pane_extent():
  {==}

def Laws.cef_extent_never_wraps():
  {==}

def Laws.cef_native_local_only():
  {==}

def Laws.cef_native_local_cef():
  {==}

def Laws.cef_native_chrome():
  {==}

def Laws.cef_native_skips_cast():
  {==}

def Laws.cef_web_keeps_cast():
  {==}

def Laws.cef_remote_keeps_cast():
  {==}

def Laws.cef_page_here_native():
  {==}

def Laws.cef_page_elsewhere_jpeg():
  {==}

def Laws.cef_fallback_keeps_handback():
  {==}

def Laws.cef_chrome_idle_strip_off():
  {==}

def Laws.cef_note_quiet():
  {==}

def Laws.cef_old_engine_reply_ignored():
  {==}

def Laws.cef_current_engine_reply_used():
  {==}

def Laws.cef_want_key_chrome(x, d):
  {==}

def Laws.cef_after_rpc():
  {==}

def Laws.cef_after_helper():
  {==}

def Laws.cef_not_after_agent_line():
  {==}

def Laws.cef_revive_first():
  {==}

def Laws.cef_revive_bounded():
  {==}

def Laws.cef_revive_not_chrome(placed):
  {==}

def Laws.cef_unknown_refused():
  {==}

def Laws.cef_thread_owner():
  {==}

def Laws.cef_empty_owner_refused(t, b):
  {==}

def Laws.cef_handback_names_page():
  {==}

def Laws.cef_handback_default_bot():
  {==}

def Laws.cef_takeover_names_page():
  {==}

def Laws.cef_other_actions_keep_bot():
  {==}
# --- end browser-cef ---

# --- browser-cef local window guard ---
def Laws.cef_web_origin_refused(proxy):
  match proxy:
    case SNil{}: {==}
    case SCon{_, _}: {==}

def Laws.cef_native_link_allowed():
  {==}
# --- end browser-cef local window guard ---
# ---- mail perf ----

def Laws.mail_credit_carried(text, charge): {==}

def Laws.mail_credit_native_kept(w, charge):
  match w:
    case MPCREDIT.WText{_}: {==}
    case MPCREDIT.WCharged{_, _}: {==}
    case MPCREDIT.WPong{_}: {==}
    case MPCREDIT.WBin{_}: {==}
    case MPCREDIT.WClose{}: {==}
    case MPCREDIT.WState{_, _}: {==}
    case MPCREDIT.WItems{_, _, _, _, _, _}: {==}
# ---- i18n web phone ----
def Laws.webphone_option_english(): {==}
def Laws.webphone_model_name_kept(): {==}
def Laws.webphone_count_ungrouped(): {==}
def Laws.webphone_count_japanese(): {==}
def Laws.webphone_idle_japanese(): {==}
# ---- end i18n web phone ----
# ---- one-hub ----
def Laws.cli_unknown_never_runs(xs):
  match xs:
    case Nil{}:
      {==}
    case +h <> t:
      {==}

def Laws.cli_unknown_after_flags_never_runs(xs):
  match xs:
    case Nil{}:
      {==}
    case +h <> t:
      {==}

def Laws.cli_word_never_runs(xs):
  match xs:
    case Nil{}:
      {==}
    case +h <> t:
      {==}

def Laws.cli_version_never_runs(xs):
  match xs:
    case Nil{}:
      {==}
    case +h <> t:
      {==}

def Laws.cli_version_after_flags(xs):
  match xs:
    case Nil{}:
      {==}
    case +h <> t:
      {==}

def Laws.cli_help_never_runs(xs):
  match xs:
    case Nil{}:
      {==}
    case +h <> t:
      {==}

def Laws.cli_h_never_runs(xs):
  match xs:
    case Nil{}:
      {==}
    case +h <> t:
      {==}

def Laws.cli_missing_value():
  {==}

def Laws.cli_value_not_a_flag(xs):
  match xs:
    case Nil{}:
      {==}
    case +h <> t:
      {==}

def Laws.cli_test_hub_runs():
  {==}

def Laws.onehub_ours():
  {==}

def Laws.onehub_held():
  {==}

def Laws.onehub_says():
  {==}

def Laws.standin_path_first():
  {==}

def Laws.standin_merge_behind():
  {==}
# ---- end one-hub ----
# ---- kicad-ipc 4 ----
def Laws.kicad4_bp1_old(): {==}

def Laws.kicad4_bp2_old(): {==}

def Laws.kicad4_bp3_old(): {==}

def Laws.kicad4_bp4_ok(): {==}

def Laws.kicad4_bp10_ok(): {==}

def Laws.kicad4_describe_part_reads_bp(): {==}

def Laws.kicad4_no_suffix_is_bp0(): {==}

def Laws.kicad4_bp3_needs_api2(): {==}

def Laws.kicad4_bp2_before_bp3(): {==}

def Laws.kicad4_bp10_after_bp3(): {==}

def Laws.kicad4_min_api(): {==}

def Laws.kicad4_pinned_version_is_min(): {==}

def Laws.kserve4_launcher_never_real(): {==}

def Laws.kserve4_launcher_kept(): {==}

def Laws.kserve4_argv_launcher(): {==}

def Laws.kserve4_bare_footprint_refused(): {==}

def Laws.kserve4_refusal_names_full_instance(): {==}

def Laws.kserve4_full_footprint_passes(): {==}

def Laws.kserve4_footprint_cap_lifts(): {==}

def Laws.kserve4_other_item_passes(): {==}

def Laws.kserve4_other_command_passes(): {==}

def Laws.kserve4_bare_symbol_refused(): {==}

def Laws.kserve4_symbol_advice(): {==}

def Laws.kserve4_symbol_with_definition_passes(): {==}

def Laws.kserve4_symbol_cap_lifts(): {==}

def Laws.kserve4_no_items_passes(): {==}

def Laws.prompt4_kicad_ok_names_the_rules(): {==}

def Laws.kserve4_nested_symbol_library_id_refused(): {==}

def Laws.kserve4_nested_symbol_cap_lifts(): {==}

# ---- end kicad-ipc 4 ----
# ---- kicad fork viewers ----
def Laws.fork_cache_legacy(id):
  {==}
def Laws.fork_cache_alias(id):
  {==}
# ---- end kicad fork viewers ----

# ---- kicad fork footprint placement ----
def Laws.fork_footprint_legacy_at(x):
  {==}
# ---- end kicad fork footprint placement ----

def Laws.mail_plain_count(b, bot): {==}

def Laws.mail_global_primary(): {==}

def Laws.mail_log_count_set(c, n):
  match c:
    case MPCREDIT.Cfg{home, web, host, port, prompt, dirs, version, MPCREDIT.Net{tailnet, https, token, models, mtext, rt, br, pk, tm, machines, pv, MPLOG.Bn{dir, seen, work, far, live, broken, fl, tools, tch, cef, wire_n}, managed, link, freecad, ks}, telemetry, views}:
      {==}

def Laws.mail_log_count_committed(c, fresh):
  match c:
    case MPCREDIT.Cfg{home, web, host, port, prompt, dirs, version, MPCREDIT.Net{tailnet, https, token, models, mtext, rt, br, pk, tm, machines, pv, MPLOG.Bn{dir, seen, work, far, live, broken, fl, tools, tch, cef, wire_n}, managed, link, freecad, ks}, telemetry, views}:
      {==}

# ---- native phone i18n ----
def Laws.native_language_english(): {==}
def Laws.native_language_preview(): {==}
def Laws.native_language_system(): {==}
def Laws.native_language_fallback(): {==}
# ---- end native phone i18n ----
# ---- build-speed: one executable, exact entry selection ----
def Laws.exe_server_name():
  {==}

def Laws.exe_server_path():
  {==}

def Laws.exe_app_name():
  {==}

def Laws.exe_server_exact():
  {==}
# ---- end build-speed ----

# ---- global provider fallback ----
def Laws.fallback_empty_exhausted(s, caps, now):
  {==}

def Laws.fallback_never_wraps():
  {==}

def Laws.fallback_auth_is_not_quota():
  {==}

def Laws.fallback_unknown_reset_held():
  {==}

def Laws.fallback_reset_boundary():
  {==}

def Laws.fallback_no_recovery_while_locked(s, now):
  {==}
# ---- end global provider fallback ----

# Sidebar row memos compare exact resolved inputs with the same typography.
law sr_clip_sound:
  for a: D.Clip
  for b: D.Clip
  for +h: {SR.Key.clip(a, b) == True{} : Bool}
  {a == b : D.Clip}

def sr_clip_sound(a, b, h):
  match a b:
    case D.Clip{+a0, +a1, +a2, +a3} D.Clip{+b0, +b1, +b2, +b3}:
      e0 = u32_eq_sound(a0, b0, and_l(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), U32.is_eq(a3, b3))), h))
      +hc1 = and_r(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), U32.is_eq(a3, b3))), h)
      e1 = u32_eq_sound(a1, b1, and_l(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), U32.is_eq(a3, b3)), hc1))
      +hc2 = and_r(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), U32.is_eq(a3, b3)), hc1)
      e2 = u32_eq_sound(a2, b2, and_l(U32.is_eq(a2, b2), U32.is_eq(a3, b3), hc2))
      +hc3 = and_r(U32.is_eq(a2, b2), U32.is_eq(a3, b3), hc2)
      e3 = u32_eq_sound(a3, b3, hc3)
      %e0 : {D.Clip{a0, a1, a2, a3} == D.Clip{_, b1, b2, b3} : D.Clip}
      %e1 : {D.Clip{a0, a1, a2, a3} == D.Clip{a0, _, b2, b3} : D.Clip}
      %e2 : {D.Clip{a0, a1, a2, a3} == D.Clip{a0, a1, _, b3} : D.Clip}
      %e3 : {D.Clip{a0, a1, a2, a3} == D.Clip{a0, a1, a2, _} : D.Clip}
      {==}

law sr_words_k:
  for ok: Bool
  for a: SRT.SideRowK
  for b: SRT.SideRowK
  for h: {SR.Key.same.text(ok, a, b) == True{} : Bool}
  {ok == True{} : Bool}

def sr_words_k(ok, a, b, h):
  match ok:
    case False{}: Empty.absurd({False{} == True{} : Bool}, false_ne_true(h))
    case True{}: {==}

law sr_text_k:
  for ok: Bool
  for a: SRT.SideRowK
  for b: SRT.SideRowK
  for h: {SR.Key.same.text(ok, a, b) == True{} : Bool}
  {SR.Key.same.text(True{}, a, b) == True{} : Bool}

def sr_text_k(ok, a, b, h):
  match ok:
    case False{}: Empty.absurd({SR.Key.same.text(True{}, a, b) == True{} : Bool}, false_ne_true(h))
    case True{}: h


def Laws.sidebar_row_key_sound(a, b, h):
  match a b:
    case SRT.SideRowK{+a0, +a1, +a2, +a3, +a4, +a5, +a6, +a7, +a8, +a9, +a10, +a11, +a12, +a13} SRT.SideRowK{+b0, +b1, +b2, +b3, +b4, +b5, +b6, +b7, +b8, +b9, +b10, +b11, +b12, +b13}:
      +ka = {SRT.SideRowK{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13} : SRT.SideRowK}
      +kb = {SRT.SideRowK{b0, b1, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13} : SRT.SideRowK}
      +hw = sr_words_k(SR.Key.same.words(ka, kb), ka, kb, h)
      +ht = sr_text_k(SR.Key.same.words(ka, kb), ka, kb, h)
      e0 = bool_eq_sound(a0, b0, and_l(Q.Eq.bool(a0, b0), Bool.and(Q.Eq.bool(a1, b1), Bool.and(Q.Eq.bool(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(SR.Key.clip(a10, b10), Bool.and(U32.is_eq(a11, b11), Nat.is_eq(a13, b13)))))))), hw))
      +hw1 = and_r(Q.Eq.bool(a0, b0), Bool.and(Q.Eq.bool(a1, b1), Bool.and(Q.Eq.bool(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(SR.Key.clip(a10, b10), Bool.and(U32.is_eq(a11, b11), Nat.is_eq(a13, b13)))))))), hw)
      e1 = bool_eq_sound(a1, b1, and_l(Q.Eq.bool(a1, b1), Bool.and(Q.Eq.bool(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(SR.Key.clip(a10, b10), Bool.and(U32.is_eq(a11, b11), Nat.is_eq(a13, b13))))))), hw1))
      +hw2 = and_r(Q.Eq.bool(a1, b1), Bool.and(Q.Eq.bool(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(SR.Key.clip(a10, b10), Bool.and(U32.is_eq(a11, b11), Nat.is_eq(a13, b13))))))), hw1)
      e6 = bool_eq_sound(a6, b6, and_l(Q.Eq.bool(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(SR.Key.clip(a10, b10), Bool.and(U32.is_eq(a11, b11), Nat.is_eq(a13, b13)))))), hw2))
      +hw3 = and_r(Q.Eq.bool(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(SR.Key.clip(a10, b10), Bool.and(U32.is_eq(a11, b11), Nat.is_eq(a13, b13)))))), hw2)
      e7 = u32_eq_sound(a7, b7, and_l(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(SR.Key.clip(a10, b10), Bool.and(U32.is_eq(a11, b11), Nat.is_eq(a13, b13))))), hw3))
      +hw4 = and_r(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(SR.Key.clip(a10, b10), Bool.and(U32.is_eq(a11, b11), Nat.is_eq(a13, b13))))), hw3)
      e8 = u32_eq_sound(a8, b8, and_l(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(SR.Key.clip(a10, b10), Bool.and(U32.is_eq(a11, b11), Nat.is_eq(a13, b13)))), hw4))
      +hw5 = and_r(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(SR.Key.clip(a10, b10), Bool.and(U32.is_eq(a11, b11), Nat.is_eq(a13, b13)))), hw4)
      e9 = u32_eq_sound(a9, b9, and_l(U32.is_eq(a9, b9), Bool.and(SR.Key.clip(a10, b10), Bool.and(U32.is_eq(a11, b11), Nat.is_eq(a13, b13))), hw5))
      +hw6 = and_r(U32.is_eq(a9, b9), Bool.and(SR.Key.clip(a10, b10), Bool.and(U32.is_eq(a11, b11), Nat.is_eq(a13, b13))), hw5)
      e10 = sr_clip_sound(a10, b10, and_l(SR.Key.clip(a10, b10), Bool.and(U32.is_eq(a11, b11), Nat.is_eq(a13, b13)), hw6))
      +hw7 = and_r(SR.Key.clip(a10, b10), Bool.and(U32.is_eq(a11, b11), Nat.is_eq(a13, b13)), hw6)
      e11 = u32_eq_sound(a11, b11, and_l(U32.is_eq(a11, b11), Nat.is_eq(a13, b13), hw7))
      +hw8 = and_r(U32.is_eq(a11, b11), Nat.is_eq(a13, b13), hw7)
      e13 = nat_eq_sound(a13, b13, hw8)
      e2 = eq_str_sound(a2, b2, and_l(Q.Eq.str(a2, b2), Bool.and(Q.Eq.str(a3, b3), Bool.and(Q.Eq.str(a4, b4), Bool.and(Q.Eq.str(a5, b5), Q.Eq.str(a12, b12)))), ht))
      +ht1 = and_r(Q.Eq.str(a2, b2), Bool.and(Q.Eq.str(a3, b3), Bool.and(Q.Eq.str(a4, b4), Bool.and(Q.Eq.str(a5, b5), Q.Eq.str(a12, b12)))), ht)
      e3 = eq_str_sound(a3, b3, and_l(Q.Eq.str(a3, b3), Bool.and(Q.Eq.str(a4, b4), Bool.and(Q.Eq.str(a5, b5), Q.Eq.str(a12, b12))), ht1))
      +ht2 = and_r(Q.Eq.str(a3, b3), Bool.and(Q.Eq.str(a4, b4), Bool.and(Q.Eq.str(a5, b5), Q.Eq.str(a12, b12))), ht1)
      e4 = eq_str_sound(a4, b4, and_l(Q.Eq.str(a4, b4), Bool.and(Q.Eq.str(a5, b5), Q.Eq.str(a12, b12)), ht2))
      +ht3 = and_r(Q.Eq.str(a4, b4), Bool.and(Q.Eq.str(a5, b5), Q.Eq.str(a12, b12)), ht2)
      e5 = eq_str_sound(a5, b5, and_l(Q.Eq.str(a5, b5), Q.Eq.str(a12, b12), ht3))
      +ht4 = and_r(Q.Eq.str(a5, b5), Q.Eq.str(a12, b12), ht3)
      e12 = eq_str_sound(a12, b12, ht4)
      %e0 : {ka == SRT.SideRowK{_, b1, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13} : SRT.SideRowK}
      %e1 : {ka == SRT.SideRowK{a0, _, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13} : SRT.SideRowK}
      %e2 : {ka == SRT.SideRowK{a0, a1, _, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13} : SRT.SideRowK}
      %e3 : {ka == SRT.SideRowK{a0, a1, a2, _, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13} : SRT.SideRowK}
      %e4 : {ka == SRT.SideRowK{a0, a1, a2, a3, _, b5, b6, b7, b8, b9, b10, b11, b12, b13} : SRT.SideRowK}
      %e5 : {ka == SRT.SideRowK{a0, a1, a2, a3, a4, _, b6, b7, b8, b9, b10, b11, b12, b13} : SRT.SideRowK}
      %e6 : {ka == SRT.SideRowK{a0, a1, a2, a3, a4, a5, _, b7, b8, b9, b10, b11, b12, b13} : SRT.SideRowK}
      %e7 : {ka == SRT.SideRowK{a0, a1, a2, a3, a4, a5, a6, _, b8, b9, b10, b11, b12, b13} : SRT.SideRowK}
      %e8 : {ka == SRT.SideRowK{a0, a1, a2, a3, a4, a5, a6, a7, _, b9, b10, b11, b12, b13} : SRT.SideRowK}
      %e9 : {ka == SRT.SideRowK{a0, a1, a2, a3, a4, a5, a6, a7, a8, _, b10, b11, b12, b13} : SRT.SideRowK}
      %e10 : {ka == SRT.SideRowK{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, _, b11, b12, b13} : SRT.SideRowK}
      %e11 : {ka == SRT.SideRowK{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, _, b12, b13} : SRT.SideRowK}
      %e12 : {ka == SRT.SideRowK{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, _, b13} : SRT.SideRowK}
      %e13 : {ka == SRT.SideRowK{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, _} : SRT.SideRowK}
      {==}


def Laws.sidebar_row_paint_exact(ty, a, b, h):
  %Laws.sidebar_row_key_sound(a, b, h) : {SR.Key.paint(ty, a) == SR.Key.paint(ty, _) : L.Out}
  {==}

def Laws.visible_cats_empty_preserves_cache(c, clock):
  {==}

def Laws.visible_sleep_cat_posed_once(clock):
  {==}
# ---- mail ui ----
def Laws.mailroom_tab_after_chat():
  {==}

def Laws.mailroom_uber_default(u, id):
  {==}

def Laws.mailroom_run_collapsed_default(now):
  {==}

def Laws.mailroom_chip_filters(f, bot):
  match f:
    case MVW.Filter{_, _, _, _}: {==}

def Laws.mailroom_chip_keeps_selection(u, bot):
  match u:
    case U.Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}: {==}

def Laws.mailroom_covers_plane(p):
  {==}

def Laws.mail_body_kept_output(k, o, at, due): {==}

def Laws.mail_age_relative_deadline(): {==}

def Laws.mail_plane_indexed_build(u): {==}

def Laws.mail_plane_indexed_hidden(u, ix): {==}
def Laws.auth_foreign_origin_denied(trusted, path, token):
  {==}

def Laws.auth_empty_remote_token_denied():
  {==}

def Laws.auth_cookie_prefix_denied():
  {==}

def Laws.auth_oauth_post_denied(trusted, hs, path, token):
  {==}

def Laws.auth_oauth_other_route_denied(trusted, hs, token):
  {==}

def Laws.auth_oauth_foreign_origin_denied(trusted, token):
  {==}

def Laws.auth_oauth_local_document_reaches_state_check(token):
  {==}

def Laws.auth_oauth_document_does_not_authorize_websocket(trusted, token):
  {==}

# ---- PCB viewer text ----
def Laws.pcb2_hidden_text_draws_nothing(at, angle, x, text, up, key, sig, ctx):
  {==}

# ---- Repeated native 2D viewer requests ----
def Laws.viewer_reopen_board_keeps_scene(items, now):
  {==}

def Laws.viewer_reopen_schematic_keeps_scene(items, now):
  {==}

# --- browser-cef default (October19) ---
def Laws.cef_default_request():
  {==}
def Laws.cef_unset_request():
  {==}
def Laws.cef_explicit_chrome_request():
  {==}
def Laws.cef_explicit_cef_request():
  {==}
# --- end browser-cef default ---

# --- browser-cef fallback (October19) ---
def Laws.cef_unavailable_default_is_explained():
  {==}
def Laws.cef_explicit_chrome_has_no_warning():
  {==}
def Laws.cef_fallback_keeps_error(err):
  match err:
    case SNil{}: {==}
    case SCon{_, _}: {==}
# --- end browser-cef fallback ---

def Laws.run_model_codex_unset_inherits(uber): {==}
def Laws.run_model_codex_stale_claude_uses_default(parent, uber): {==}
def Laws.run_model_codex_compatible_override_kept(parent, uber): {==}
def Laws.run_model_muse_keeps_exact_full_selector(uber): {==}
def Laws.run_model_claude_uber_default_kept(parent): {==}
def Laws.run_model_claude_worker_default_kept(parent): {==}
def Laws.run_model_codex_stale_parent_uses_cli_default(uber): {==}

def Laws.wt_seed_digit_step(rest, acc): {==}
def Laws.wt_seed_skips_separator(rest, acc): {==}
def Laws.wt_seed_legacy_small(): {==}
def Laws.wt_seed_large_id(): {==}
def Laws.wt_seed_same_second_next(): {==}
def Laws.wt_seed_wrap(): {==}
# telemetry

def Laws.tm_optout_no_id():
  {==}
def Laws.tm_unasked_no_id():
  {==}
def Laws.tm_sandbox_never(se, tm, dnt, ci, url, kid, key):
  {==}
def Laws.tm_gate_no_key():
  {==}
def Laws.tm_clean_drops_path():
  {==}
def Laws.tm_clean_drops_email():
  {==}
def Laws.tm_clean_drops_name():
  {==}
def Laws.tm_active_once_a_day():
  {==}
def Laws.tm_queue_cap():
  {==}
def Laws.tm_sent_202_drops():
  {==}
def Laws.tm_sent_500_keeps():
  {==}
def Laws.tm_state_roundtrip():
  {==}
def Laws.share_on_sets_asked():
  {==}
def Laws.share_step_before_uber():
  {==}
def Laws.bug_url_only_version():
  {==}

def Laws.tm_reset_invalidates_worker():
  {==}
def Laws.tm_optout_invalidates_worker():
  {==}
def Laws.tm_loopback_rejects_userinfo():
  {==}
def Laws.tm_package_nightly_channel():
  {==}

def Laws.tm_new_usage_marker():
  {==}
def Laws.tm_cached_uber_step_kept():
  {==}
def Laws.tm_marked_usage_step_kept():
  {==}
# --- browser controls (CEF owner) ---
def Laws.browser_url_hostname(): {==}
def Laws.browser_url_localhost(): {==}
def Laws.browser_url_explicit(path): {==}
def Laws.browser_url_no_script(): {==}
def Laws.browser_toolbar_no_eval(): {==}
def Laws.browser_toolbar_tabs_allowed(): {==}
# --- end browser controls ---

def Laws.refresh_revision_changed(size_changed):
  match size_changed:
    case True{}: {==}
    case False{}: {==}

def Laws.refresh_size_changed(same_revision):
  match same_revision:
    case True{}: {==}
    case False{}: {==}

def Laws.refresh_unchanged_quiet(): {==}
# --- browser-cef release archive (October19) ---
def Laws.cef_release_uses_paired_archive(): {==}
def Laws.cef_release_arm_archive_unchanged(): {==}
def Laws.cef_release_mac_archive_unchanged(): {==}
def Laws.cef_release_old_updater_cannot_match(): {==}
# --- end browser-cef release archive ---

def Laws.ix_rename_global_invalidation_retained(id, t):
  match t:
    case MM.TouchK{n, g, r, m, l}: {==}

def Laws.ix_rename_relationship_invalidation_retained(id, t):
  match t:
    case MM.TouchK{n, g, r, m, l}: {==}

def Laws.ix_old_touch_global_fallback(n, g, r, l):
  {==}

def Laws.subs_phase_index_supplied_exact(st):
  {==}

def Laws.subs_children_indexed_exact(some, ts, ids, st, now):
  match some:
    case False{}: {==}
    case True{}: {==}

def Laws.subs_rows_indexed_exact(st, info, th, now):
  +ts = U.Tasks.of(st, th)
  +ids = M.Launch.of(st, th)
  +some = Bool.or(Bool.not(List.is_empty(&2, M.Task, ts)), Bool.not(List.is_empty(&2, String, ids)))
  Equal.cong(List<&2, SB.SubRow>, List<&2, SB.SubRow>,
    rs => SB.Subs.group(List.concat(&2, SB.SubRow, [rs, SB.Subs.wakes(st, th), SB.Subs.locks.go(M.Locks.names(st), st, th), SB.Subs.agents(st, info, th, now)]), info, th),
    SB.Subs.children.at(some, ts, ids, st, now, [U.Side.ix(st)]),
    SB.Subs.children(some, ts, ids, st, now),
    Laws.subs_children_indexed_exact(some, ts, ids, st, now))

def Laws.mem_score_w_strict_equivalence(w, hay, text):
  MSCORE.strict_equivalence(w, hay, text)

def Laws.tl_live_single_exact(ty, empty, ph, text, memo, px):
  match empty:
    case False{}: {==}
    case True{}: {==}

def Laws.textscale_default(): {==}
def Laws.textscale_invalid(): {==}
def Laws.textscale_real_font_size(): {==}

def Laws.mail_continuity_read_open_unresolved(): {==}
def Laws.mail_continuity_unread_unresolved(): {==}
def Laws.mail_continuity_current_escalation_once(token, forwarded, status):
  %Equal.sym(Bool, String.eq(token, token), True{}, Laws.string_eq_refl(token)) : {Bool.and(Bool.not(MCN.Continuity.disposed(status)), Bool.and(Bool.not(_), Bool.not(String.eq(token, forwarded)))) == False{} : Bool}
  and_false_r(Bool.not(MCN.Continuity.disposed(status)))
def Laws.mail_continuity_committed_reply_resolves(token, escalated, forwarded, status): {==}

def Laws.mail_disposition_inactive_refused(input, current): {==}
def Laws.mail_disposition_ordinary_codex_unchanged(): {==}
def Laws.mail_disposition_codex_single_tool(): {==}
def Laws.mail_disposition_done_notice_only(): {==}
def Laws.mail_disposition_defer_notice_only(): {==}
def Laws.mail_disposition_no_product_action(): {==}

def Laws.mail_disposition_bot_router_scoped_tools(s, now, thread, models, version, dir, extra, o): {==}
def Laws.mail_disposition_plain_router_scoped_tools(s, now, thread, models, version, dir, extra, o): {==}

def Laws.mail_ui_dispatch_available(i, s, now, client, id, p): {==}
def Laws.mail_ui_body_read_available(s, now, client, id, p): {==}
def Laws.mail_ui_invalid_target_no_effect(s, now, thread, p, msg): {==}
def Laws.mail_ui_owned_target_uses_mailbox(s, now, thread, p, msg): {==}

def Laws.mail_room_invalid_request_is_error(s, now, client, id, p): {==}
def Laws.mail_room_invalid_room_is_error(s, now, client, id, p, r): {==}
def Laws.mail_room_send_no_eager_events(s, now, client, id, p, room, subject, members, made): {==}
def Laws.mail_room_set_updates_client_index(u, room, name, members, at): {==}
def Laws.mail_room_post_updates_client_index(u, id, room, from, text, hop, at): {==}
def Laws.mail_room_post_affects_room_only(id, room, from, text, hop, at): {==}

def Laws.mail_room_target_nonroom_unchanged(os): {==}
def Laws.mail_room_target_single_empty_option(room): {==}
def Laws.mail_room_target_duplicate_dropped(addr, label, rest): {==}

def Laws.desktop_public_row_no_document(row, index): {==}
def Laws.desktop_public_row_no_lease(row, index): {==}
def Laws.desktop_public_row_no_capture(row, index): {==}
def Laws.desktop_lost_guard_no_cdp(b, owner): {==}
def Laws.desktop_lost_guard_no_reinitialize(b, owner): {==}
def Laws.desktop_no_generic_file_assignment(): {==}
def Laws.desktop_no_public_interception_toggle(): {==}
def Laws.desktop_unknown_owner_has_no_drive(drive): {==}

def Laws.desktop_connection_frame_bytes_unchanged(text, version, hub): {==}
def Laws.desktop_connection_close_bytes_unchanged(text, version, hub): {==}

# The focused universal presentation/credit proofs are reused unchanged.

def Laws.presentation_state_kept(u, p): PRESENTPROOF.state_kept(u, p)
def Laws.presentation_seq_kept(u, p): PRESENTPROOF.seq_kept(u, p)
def Laws.presentation_bodies_kept(u, p): PRESENTPROOF.bodies_kept(u, p)
def Laws.presentation_history_generation_kept(c, at, more, rest): PRESENTPROOF.credit_generation_kept(c, at, more, rest)
def Laws.presentation_history_room_kept(c, at, more, rest): PRESENTPROOF.credit_room_kept(c, at, more, rest)
def Laws.presentation_history_over_kept(c, at, more, rest): PRESENTPROOF.credit_over_kept(c, at, more, rest)

def Laws.cache_identity_post_refused(hs, token): {==}
def Laws.cache_identity_empty_cookie(https): {==}

def Laws.oprep_task_not_in_chat(): {==}
def Laws.oprep_task_delivered_with_filing(): {==}
def Laws.oprep_other_thread_unchanged(): {==}
