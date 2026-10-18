import Base
import ../core/client.bend as U
import ../core/winnote.bend as NT
import ./win.bend as W
import ../core/json.bend as J
import ./viewer.bend as V
import ../core/hier.bend as HI
import ../core/board.bend as B
import ../core/glb.bend as G
import ../gfx/mesh.bend as MS
import ../gfx/face.bend as FA
import ../gfx/bitmap.bend as BM
import ../core/vt.bend as VT
import ../core/attach.bend as AT
import ../core/links.bend as LK
import ../core/lightbox.bend as LB
import ../core/edit.bend as ED
import ../core/ime.bend as IM
import ../core/model.bend as M
import ../core/bot.bend as BT
import ../core/perf.bend as PF
import ../core/hist.bend as HS
import ../core/rview.bend as RV
import ../core/emb.bend as EM
import ../core/onboard.bend as OB
import ../core/layers.bend as LY

# The native window's state around the shared client state (Ui): size,
# pointer, scroll, focus and the composer caret. Input is pure: an event
# goes in, the next state and the commands to run come out.

# a clickable region from the last layout (topmost first)
type Hit is Data:
  Hit{x0: U32, y0: U32, x1: U32, y1: U32, action: String, value: String}

type Nui is Data:
  Nui{
    ui: U.Ui,
    w: U32,
    h: U32,
    mx: U32,
    my: U32,
    side: U32,
    scroll: U32,
    caret: Nat,
    focus: String,
    path: String,
    hits: List<&2, Hit>,
    ms: Nat,
    vw: V.Vw
  }

# what the window loop must do
type NCmd is Data:
  NSend{text: String}
  NCopy{text: String}
  NPaste{}
  NQuit{}
  NConnect{url: String}
  # open a web address in the system's browser
  NOpen{url: String}
  # write a thread's draft down now ("" forgets it)
  NKeep{thread: String, text: String}
  # a desktop notification from the hub (core/desk.bend)
  NNotify{thread: String, title: String, body: String}
  # run a client action after ms (client.bend's Later)
  NLater{ms: Nat, action: String, value: String}

type Step is Data:
  Step{nui: Nui, cmds: List<&2, NCmd>, dirty: Bool}

def Nui.init(ui: U.Ui, +w: U32, +h: U32) -> Nui:
  Nui{ui, w, h, 0, 0, 0, 0, 0n, "composer", "", Nil{}, 0n, V.Vw.init()}

# Keys (X keysyms)
# ----------------

def Key.backspace() -> U32:
  65288

def Key.tab() -> U32:
  65289

def Key.enter() -> U32:
  65293

def Key.escape() -> U32:
  65307

def Key.delete() -> U32:
  65535

def Key.home() -> U32:
  65360

def Key.left() -> U32:
  65361

def Key.right() -> U32:
  65363

def Key.end() -> U32:
  65367

def Key.up() -> U32:
  65362

def Key.down() -> U32:
  65364

def Key.kp_enter() -> U32:
  65421

# Commands from the client state
# ------------------------------

def Nui.cmd(c: U.Cmd, rest: List<&2, NCmd>) -> List<&2, NCmd>:
  match c:
    case U.Send{t}:
      NSend{t} <> rest
    case U.Copy{t}:
      NCopy{t} <> rest
    case U.Connect{url}:
      NConnect{url} <> rest
    case U.Keep{id, t}:
      NKeep{id, t} <> rest
    case U.Notify{th, ti, b, _}:
      NNotify{th, ti, b} <> rest
    case U.Later{ms, a, v}:
      NLater{ms, a, v} <> rest
    case _:
      rest

def Nui.cmds(cs: List<&2, U.Cmd>) -> List<&2, NCmd>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      Nui.cmd(c, Nui.cmds(t))

# Form fields
# -----------
# The bots' forms keep their text in the client's scratch ("@f.<name>",
# as the "bfield" action sets it). A focused field's id is "f:" then its
# name, the action Enter runs, and a prefix of that action's value (the
# field's text follows it), apart by \u{1e}. The client's own focus ids
# for these fields are named here.

def Fld.sep() -> String:
  SCon{Chr{30}, SNil{}}

def Fld.id(+name: String, +action: String, +pre: String) -> String:
  "f:" ++ name ++ Fld.sep() ++ action ++ Fld.sep() ++ pre

def Fld.alias(+id: String) -> String:
  Bool.pick(String, String.eq(id, "kicad-path"), Fld.id("kpath", "setting", "kicad.path="),
    Bool.pick(String, String.eq(id, "bot-name"), Fld.id("name", "bot-create", ""),
    Bool.pick(String, String.eq(id, "room-name"), Fld.id("rname", "room-save", ""),
      Bool.pick(String, String.eq(id, "room-post"), Fld.id("rpost", "room-post", ""),
        Bool.pick(String, String.eq(id, "remote-send"), Fld.id("rtext", "remote-send", ""),
          Bool.pick(String, String.eq(id, "routine-name"), Fld.id("oname", "routine-save", ""),
            Bool.pick(String, String.eq(id, "voice-fix"), Fld.id("vfix", "voice-fixed", ""),
              Bool.pick(String, String.eq(id, "proj-find"), Fld.id("pq", "proj-go", ""),
                Bool.pick(String, String.eq(id, "thread-title"), Fld.id("title", "rename-save", ""), id)))))))))

# Focus and scroll requests from the client state apply here
def Nui.effects(cs: List<&2, U.Cmd>, n: Nui) -> Nui:
  match cs n:
    case Nil{} _:
      n
    case U.Focus{+id} <> t Nui{u, w, h, mx, my, sd, sc, ca, _, pa, hi, ms, vw}:
      Nui.effects(t, Nui{u, w, h, mx, my, sd, sc, ca, Fld.alias(id), pa, hi, ms, vw})
    case U.ScrollEnd{} <> t Nui{u, w, h, mx, my, sd, _, ca, fo, pa, hi, ms, vw}:
      Nui.effects(t, Nui{U.Hold.end(u, ""), w, h, mx, my, sd, 0, ca, fo, pa, hi, ms, vw})
    case U.Jump{_} <> t Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}:
      Nui.effects(t, Nui{U.Ui.scratch_set(u, "@jump.go", "1"), w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw})
    case _ <> t _:
      Nui.effects(t, n)

# run a client action and fold its commands in
def Nui.act.k(a: U.Act, n: Nui) -> Step:
  match a n:
    case U.Act{u, +cs} Nui{_, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}:
      Step{Nui.effects(cs, Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}), Nui.cmds(cs), True{}}

def Nui.act(n: Nui, +action: String, +value: String) -> Step:
  match n:
    case Nui{+u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}:
      Nui.act.k(U.Ui.act(u, action, value), Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw})

# Server messages
# ---------------

# a bot's browser has a new frame ({"t":"shot","bot":...,"n":...}): the
# window notes its number ("shot.<bot>") and loads it when shown
def Shot.recv(shot: Bool, +u: U.Ui, +o: J.Json) -> U.Ui:
  match shot:
    case True{}:
      U.Ui.info_set(u, "shot." ++ J.Json.get_str(o, "bot", ""), Nat.show(J.Json.get_nat(o, "n", 0n)))
    case False{}:
      u

def Nui.recv.k(n: Nui, +o: J.Json) -> Nui:
  match n:
    case Nui{+u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}:
      Nui{Shot.recv(String.eq(J.Json.get_str(o, "t", ""), "shot"), U.Ui.recv(u, o), o), w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}

# a message through the client's own receive (U.Ui.recv_act): the commands
# it gives run too, the outbox sent again after a reconnect's log and a
# desktop notification ({"t":"notify"}) among them, as on the web and the
# phones
def Nui.recv.act.k(a: U.Act, +o: J.Json) -> U.Act:
  match a:
    case U.Act{u, cs}:
      U.Act{Shot.recv(String.eq(J.Json.get_str(o, "t", ""), "shot"), u, o), cs}

def Nui.with_focus(n: Nui, fo: String) -> Nui:
  match n:
    case Nui{u, w, h, mx, my, sd, sc, ca, _, pa, hi, ms, vw}:
      Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}

# Editing a text field
# --------------------
# Normal editor keys: arrows (ctrl: by word), Home/End (the line; ctrl: the
# text), Up/Down between lines, shift with any of them selects, ctrl+a
# selects all, ctrl+c/ctrl+x copy and cut, ctrl+backspace/ctrl+delete
# delete a word, ctrl+z undoes and ctrl+shift+z (or ctrl+y) redoes. Typing
# or pasting replaces the selection. The motions are core/edit.bend's.

# the text, the caret and the selection's other end (the caret when none)
# after a key; whether it submits, whether the text changed, what it
# copies ("" nothing) and its kind: 0 a move, 1 typing, 2 deleting, 3
# another change, 4 undo, 5 redo
type Ed is Data:
  Ed{text: String, caret: Nat, mark: Nat, submit: Bool, changed: Bool, clip: String, kind: Nat}

def Ed.at(+s: String, +c: Nat, +m: Nat) -> Ed:
  Ed{s, c, m, False{}, False{}, "", 0n}

# a move; with shift the selection's other end stays
def Ed.go(+shift: Bool, +s: String, +c: Nat, +m: Nat) -> Ed:
  Ed.at(s, c, Bool.pick(Nat, shift, m, c))

# the text between a and b becomes x, the caret after it
def Ed.put(+s: String, +a: Nat, +b: Nat, +x: String, +k: Nat) -> Ed:
  Ed{ED.Edit.splice(s, a, b, x), Nat.add(ED.Edit.lo(a, b), String.length(x)), Nat.add(ED.Edit.lo(a, b), String.length(x)), False{}, True{}, "", k}

def Ed.insert(+s: String, +i: Nat, +m: Nat, +x: String) -> Ed:
  Ed.put(s, i, m, x, 3n)

def Ed.cut(sel: Bool, +s: String, +i: Nat, +m: Nat) -> Ed:
  match sel:
    case True{}:
      Ed{ED.Edit.splice(s, i, m, ""), ED.Edit.lo(i, m), ED.Edit.lo(i, m), False{}, True{}, ED.Edit.slice(s, i, m), 3n}
    case False{}:
      Ed.at(s, i, m)

def Ed.back(sel: Bool, +ctrl: Bool, +s: String, +i: Nat, +m: Nat) -> Ed:
  match sel:
    case True{}:
      Ed.put(s, i, m, "", 2n)
    case False{}:
      Bool.pick(Ed, Nat.is_eq(i, 0n), Ed.at(s, i, i), Ed.put(s, Bool.pick(Nat, ctrl, ED.Edit.word_left(s, i), Nat.sub(i, 1n)), i, "", 2n))

def Ed.del(sel: Bool, +ctrl: Bool, +s: String, +i: Nat, +m: Nat) -> Ed:
  match sel:
    case True{}:
      Ed.put(s, i, m, "", 2n)
    case False{}:
      Bool.pick(Ed, Nat.is_ge(i, String.length(s)), Ed.at(s, i, i), Ed.put(s, i, Bool.pick(Nat, ctrl, ED.Edit.word_right(s, i), 1n+i), "", 2n))

# left/right without shift first close a selection at its near end
def Ed.side(close: Bool, +shift: Bool, +s: String, +end: Nat, +c: Nat, +m: Nat) -> Ed:
  match close:
    case True{}:
      Ed.at(s, end, end)
    case False{}:
      Ed.go(shift, s, c, m)

def Ed.do.k(op: Nat, +shift: Bool, +ctrl: Bool, +sel: Bool, +s: String, +i: Nat, +m: Nat, +n: Nat, +text: U32) -> Ed:
  match op:
    case 1n:
      Bool.pick(Ed, shift, Ed.put(s, i, m, "\n", 3n), Ed{s, i, m, True{}, False{}, "", 0n})
    case 2n:
      Ed.back(sel, ctrl, s, i, m)
    case 3n:
      Ed.del(sel, ctrl, s, i, m)
    case 4n:
      Ed.side(Bool.and(sel, Bool.not(shift)), shift, s, ED.Edit.lo(i, m), Bool.pick(Nat, ctrl, ED.Edit.word_left(s, i), Nat.sub(i, 1n)), m)
    case 5n:
      Ed.side(Bool.and(sel, Bool.not(shift)), shift, s, ED.Edit.hi(i, m), Bool.pick(Nat, ctrl, ED.Edit.word_right(s, i), Nat.min(1n+i, n)), m)
    case 6n:
      Ed.go(shift, s, Bool.pick(Nat, ctrl, 0n, ED.Edit.line_start(s, i)), m)
    case 7n:
      Ed.go(shift, s, Bool.pick(Nat, ctrl, n, ED.Edit.line_end(s, i)), m)
    case 8n:
      Ed.go(shift, s, ED.Edit.up(s, i), m)
    case 9n:
      Ed.go(shift, s, ED.Edit.down(s, i), m)
    case 10n:
      Ed.put(s, i, m, SCon{Chr{text}, SNil{}}, Bool.pick(Nat, Nat.is_eq(ED.Edit.kind(text), 0n), 3n, 1n))
    case 11n:
      Ed.at(s, n, 0n)
    case 12n:
      Ed{s, i, m, False{}, False{}, ED.Edit.slice(s, i, m), 0n}
    case 13n:
      Ed.cut(sel, s, i, m)
    case 14n:
      Ed{s, i, m, False{}, False{}, "", 4n}
    case 15n:
      Ed{s, i, m, False{}, False{}, "", 5n}
    case _:
      Ed.at(s, i, m)

def Ed.do(op: Nat, +shift: Bool, +ctrl: Bool, +s: String, +i: Nat, +m: Nat, +text: U32) -> Ed:
  +sel = Bool.not(Nat.is_eq(i, m))
  +n = String.length(s)
  +i1 = Nat.min(i, n)
  +m1 = Nat.min(m, n)
  Ed.do.k(op, shift, ctrl, sel, s, i1, m1, n, text)

# the index of the first True (0 for none), counting from i
def Ed.first(bs: List<&2, Bool>, +i: Nat) -> Nat:
  match bs:
    case Nil{}:
      0n
    case True{} <> _:
      i
    case False{} <> t:
      Ed.first(t, 1n+i)

# ctrl with the letter c (either case)
def Ed.ctrl(+code: U32, +mods: U32, +c: U32) -> Bool:
  Bool.and(U32.is_ne(U32.and(mods, 2), 0), U32.is_eq(U32.or(code, 32), c))

# which edit a key is (see Ed.do)
def Ed.op(+code: U32, +text: U32, +mods: U32) -> Nat:
  +shift = U32.is_ne(U32.and(mods, 1), 0)
  Ed.first([Bool.or(U32.is_eq(code, Key.enter()), U32.is_eq(code, Key.kp_enter())), U32.is_eq(code, Key.backspace()),
    U32.is_eq(code, Key.delete()), U32.is_eq(code, Key.left()), U32.is_eq(code, Key.right()),
    U32.is_eq(code, Key.home()), U32.is_eq(code, Key.end()), U32.is_eq(code, Key.up()), U32.is_eq(code, Key.down()),
    Bool.and(U32.is_ne(text, 0), U32.is_eq(U32.and(mods, 6), 0)),
    Ed.ctrl(code, mods, 97), Ed.ctrl(code, mods, 99), Ed.ctrl(code, mods, 120),
    Bool.and(Ed.ctrl(code, mods, 122), Bool.not(shift)), Bool.or(Ed.ctrl(code, mods, 121), Bool.and(Ed.ctrl(code, mods, 122), shift))], 1n)

# one key on a field holding s with the caret at i and the selection's
# other end at m
def Ed.key(+code: U32, +text: U32, +mods: U32, +s: String, +i: Nat, +m: Nat) -> Ed:
  Ed.do(Ed.op(code, text, mods), U32.is_ne(U32.and(mods, 1), 0), U32.is_ne(U32.and(mods, 2), 0), s, i, m, text)

# Keys
# ----

def Nui.with_caret(n: Nui, +c: Nat) -> Nui:
  match n:
    case Nui{u, w, h, mx, my, sd, sc, _, fo, pa, hi, ms, vw}:
      Nui{u, w, h, mx, my, sd, sc, c, fo, pa, hi, ms, vw}

def Nui.with_path(n: Nui, s: String, +c: Nat) -> Nui:
  match n:
    case Nui{u, w, h, mx, my, sd, sc, _, fo, _, hi, ms, vw}:
      Nui{u, w, h, mx, my, sd, sc, c, fo, s, hi, ms, vw}

def Step.caret(s: Step, +c: Nat) -> Step:
  match s:
    case Step{n, cs, d}:
      Step{Nui.with_caret(n, c), cs, d}

def Mark.of(+r: String, +sig: String, +c: Nat) -> Nat:
  Bool.pick(Nat, String.starts_with(r, sig), Maybe.default(&2, Nat, Nat.read(String.drop(r, String.length(sig))), c), c)

def Clip.also.k(none: Bool, s: Step, +cl: String) -> Step:
  match none s:
    case True{} _:
      s
    case False{} Step{n, cs, d}:
      Step{n, List.append(&2, NCmd, cs, [NCopy{cl}]), d}

# a copy or cut puts its text on the clipboard
def Clip.also(s: Step, +cl: String) -> Step:
  Clip.also.k(String.is_empty(cl), s, cl)

# completing a `$skill` puts the caret after it
def Nui.skill.k(s: Step) -> Step:
  match s:
    case Step{Nui{+u, w, h, mx, my, sd, sc, _, fo, pa, hi, ms, vw}, cs, d}:
      Step{Nui{u, w, h, mx, my, sd, sc, String.length(U.Ui.draft_text(u)), fo, pa, hi, ms, vw}, cs, d}

def Nui.skill(n: Nui) -> Step:
  Nui.skill.k(Nui.act(n, "skill", ""))

def Ed.submit(e: Ed) -> Bool:
  match e:
    case Ed{_, _, _, s, _, _, _}:
      s

def Nui.skills_on(+u: U.Ui) -> Bool:
  Bool.not(List.is_empty(&2, U.SkillRow, U.Ui.skills(u)))

def Nui.focus_of(n: Nui) -> String:
  match n:
    case Nui{_, _, _, _, _, _, _, _, fo, _, _, _, vw}:
      fo

def Nui.focus(n: Nui, fo: String) -> Step:
  match n:
    case Nui{u, w, h, mx, my, sd, sc, ca, _, pa, hi, ms, vw}:
      Step{Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}, Nil{}, True{}}

# Going back to an entry (client.bend's Jump): the next frame lays the
# thread out with the entry's rule and says how far to scroll ("tl-jump",
# layout.bend's TL.jump.sc); the scroll is taken once, and one more frame
# draws it there

def Jump.hit(hs: List<&2, Hit>) -> String:
  match hs:
    case Nil{}:
      ""
    case Hit{_, _, _, _, +a, +v} <> t:
      Bool.pick(String, String.eq(a, "tl-jump"), v, Jump.hit(t))

def Jump.to(go: Bool, s: Step) -> Step:
  match go s:
    case True{} Step{Nui{u, w, h, mx, my, sd, _, ca, fo, pa, +hi, ms, vw}, cs, _}:
      Step{Nui{U.Ui.scratch_set(u, "@jump.go", ""), w, h, mx, my, sd, U32.from_nat(M.Switch.num(Jump.hit(hi), 0n)), ca, fo, pa, hi, ms, vw}, cs, True{}}
    case False{} _:
      s

# Pointer
# -------

def Hit.inside(h: Hit, +x: U32, +y: U32) -> Bool:
  match h:
    case Hit{x0, y0, x1, y1, _, _}:
      Bool.and(Bool.and(U32.is_ge(x, x0), U32.is_lt(x, x1)), Bool.and(U32.is_ge(y, y0), U32.is_lt(y, y1)))

def Hit.first(hs: List<&2, Hit>, +x: U32, +y: U32) -> Bool:
  match hs:
    case h <> _:
      Hit.inside(h, x, y)
    case Nil{}:
      False{}

def Hit.go(hs: List<&2, Hit>, hit: Bool, +x: U32, +y: U32) -> Maybe<&2, Hit>:
  match hs hit:
    case +h <> +r True{}:
      Some{h}
    case _ <> +t False{}:
      Hit.go(t, Hit.first(t, x, y), x, y)
    case Nil{} _:
      None{}

# the topmost region under the pointer
def Hit.find(+hs: List<&2, Hit>, +x: U32, +y: U32) -> Maybe<&2, Hit>:
  Hit.go(hs, Hit.first(hs, x, y), x, y)

# show another kind of view: the scene starts empty and refits
# (another kind or file starts its 3D scene over, so the next model is
# fitted; the same file shown again keeps its scene and camera)
def Vw.scene.k(same: Bool, t: FA.Scene3) -> FA.Scene3:
  match same:
    case True{}:
      t
    case False{}:
      FA.Scene3.none()

# (another kind of view starting over forgets the source it showed, so
# coming back to it, from the Review tab say, reads it again)
def Vw.kind_set.k(keep: Bool, v: V.Vw, +k: U32, +path: String) -> V.Vw:
  match keep v:
    case True{} V.Vw{s, vi, f, r, d, lx, ly, b, fr, cu, V.Vx{_, p, t, c, sh, bu, px, py, _, se, bm}}:
      V.Vw{s, vi, f, r, d, lx, ly, b, fr, cu, V.Vx{k, p, t, c, sh, bu, px, py, path, se, bm}}
    case False{} V.Vw{_, vi, _, r, d, lx, ly, b, _, cu, V.Vx{+k0, _, t, c, +sh, bu, px, py, +p0, se, bm}}:
      V.Vw{Nil{}, V.View{1.0, 0.0, 0.0}, False{}, "", d, lx, ly, b, False{}, cu, V.Vx{k, "", Vw.scene.k(Bool.and(U32.is_eq(k, k0), String.eq(path, p0)), t), c, Bool.pick(String, U32.is_eq(k, k0), sh, ""), bu, px, py, path, se, bm}}

def Vw.path(v: V.Vw) -> String:
  match v:
    case V.Vw{_, _, _, _, _, _, _, _, _, _, V.Vx{_, _, _, _, _, _, _, _, p, _, _}}:
      p

def Vw.pick_set(v: V.Vw, +key: String) -> V.Vw:
  match v:
    case V.Vw{s, vi, f, r, d, lx, ly, b, fr, cu, V.Vx{k, _, t, c, sh, bu, px, py, pa, se, bm}}:
      V.Vw{s, vi, f, r, d, lx, ly, b, fr, cu, V.Vx{k, key, t, c, sh, bu, px, py, pa, se, bm}}

def Nui.pos(xs: List<&2, String>, +x: String, +i: Nat) -> Nat:
  match xs:
    case Nil{}:
      i
    case +h <> t:
      Bool.pick(Nat, String.eq(h, x), i, Nui.pos(t, x, 1n+i))

def Nui.hits(n: Nui) -> List<&2, Hit>:
  match n:
    case Nui{_, _, _, _, _, _, _, _, _, _, hi, _, vw}:
      hi

# a line of the thread (side False: "tl-line", or a room's post,
# "chat-line") or a thread's row in the sidebar (side True: "select" left
# of 264)
def Hits.kind(side: Bool, +x0: U32, +a: String) -> Bool:
  match side:
    case True{}:
      Bool.and(String.eq(a, "select"), U32.is_lt(x0, 264))
    case False{}:
      Bool.or(String.eq(a, "tl-line"), String.eq(a, "chat-line"))

def Hits.is_line(hs: List<&2, Hit>, +side: Bool) -> Bool:
  match hs:
    case Hit{+x0, _, _, _, +a, _} <> _:
      Hits.kind(side, x0, a)
    case Nil{}:
      False{}

def Hits.same_line(hs: List<&2, Hit>, +side: Bool, +v: String) -> Bool:
  match hs:
    case Hit{+x0, _, _, _, +a, +hv} <> _:
      Bool.and(Hits.kind(side, x0, a), String.eq(hv, v))
    case Nil{}:
      False{}

def Hits.y(hs: List<&2, Hit>) -> U32:
  match hs:
    case Hit{_, y0, _, _, _, _} <> _:
      y0
    case Nil{}:
      0

def Hits.v(hs: List<&2, Hit>) -> String:
  match hs:
    case Hit{_, _, _, _, _, v} <> _:
      v
    case Nil{}:
      ""

# where line v is in hits (found or not)
type Seen is Data:
  Seen{found: Bool, y: U32}

def Hits.find(hs: List<&2, Hit>, hit: Bool, +side: Bool, +v: String) -> Seen:
  match hs hit:
    case Nil{} _:
      Seen{False{}, 0}
    case +h <> +r True{}:
      Seen{True{}, Hits.y([h])}
    case _ <> t False{}:
      Hits.find(t, Hits.same_line(t, side, v), side, v)

# hits from the first line (or row) on
def Hits.first(hs: List<&2, Hit>, hit: Bool, +side: Bool) -> List<&2, Hit>:
  match hs hit:
    case Nil{} _:
      Nil{}
    case +h <> +r True{}:
      h <> r
    case _ <> t False{}:
      Hits.first(t, Hits.is_line(t, side), side)

def Hits.slide.k(a: Seen, b: Seen, +ya: U32, +yb: U32) -> U32:
  match a b:
    case Seen{True{}, +y} _:
      (ya - y : U32)
    case Seen{False{}, +y1} Seen{True{}, +y}:
      (y - yb : U32)
    case _ _:
      0

# how far the thread's lines (or the sidebar's rows) moved down since the
# last frame, modulo 2^32: a line on screen in both frames (the new
# frame's first, else the old one's), where it was and is
def Hits.slide.of(+new: List<&2, Hit>, +old: List<&2, Hit>, +side: Bool) -> U32:
  +fn = Hits.first(new, Hits.is_line(new, side), side)
  +fo = Hits.first(old, Hits.is_line(old, side), side)
  Hits.slide.k(Hits.find(old, Hits.same_line(old, side, Hits.v(fn)), side, Hits.v(fn)), Hits.find(new, Hits.same_line(new, side, Hits.v(fo)), side, Hits.v(fo)), Hits.y(fn), Hits.y(fo))

def Hits.slide.or.k(still: Bool, +dy: U32, +new: List<&2, Hit>, +old: List<&2, Hit>) -> U32:
  match still:
    case True{}:
      Hits.slide.of(new, old, True{})
    case False{}:
      dy

def Hits.slide.or(+dy: U32, +new: List<&2, Hit>, +old: List<&2, Hit>) -> U32:
  Hits.slide.or.k(U32.is_eq(dy, 0), dy, new, old)

# how far the content moved down since the last frame: the thread's, else
# the sidebar's (one scrolls at a time). A hint for the tiles
# (TI.Frame.again), which check what they reuse
def Hits.slide(+new: List<&2, Hit>, +old: List<&2, Hit>) -> U32:
  Hits.slide.or(Hits.slide.of(new, old, False{}), new, old)

# a wheel notch moves the thread one tile (TI.Frame.tile): the frame
# after it slides the tiles it has instead of drawing them again
def Nui.notch() -> U32:
  64

# the value of the first hit with this action, wherever it is ("" for none)
def Hit.value(hs: List<&2, Hit>, +action: String) -> String:
  match hs:
    case Nil{}:
      ""
    case Hit{_, _, _, _, +a, +v} <> t:
      Bool.pick(String, String.eq(a, action), v, Hit.value(t, action))

# how far the sidebar may scroll: its hit "side-most" (none: no limit)
def Side.most(hs: List<&2, Hit>) -> U32:
  Maybe.default(&2, U32, U32.read(Hit.value(hs, "side-most")), 4294967295)

# a notch down the sidebar, no further than most (a scroll left past the
# end, as the list shrank, comes back to it)
def Side.down(+sd: U32, +most: U32) -> U32:
  U32.min((sd + Nui.notch() : U32), Bool.pick(U32, U32.is_gt(sd, most), sd, most))

def Hit.has(hs: List<&2, Hit>, +action: String) -> Bool:
  match hs:
    case Nil{}:
      False{}
    case Hit{_, _, _, _, +a, _} <> t:
      Bool.or(String.eq(a, action), Hit.has(t, action))

# how far up the thread's timeline scrolls (its hit "tl-most"; none: no limit)
def Tl.most(hs: List<&2, Hit>) -> U32:
  Maybe.default(&2, U32, U32.read(Hit.value(hs, "tl-most")), 4294967295)

# a notch up the thread: within two screens of its top, while earlier
# entries exist (the hit "tl-more"), they are shown too, so the wheel
# keeps going back without a button (the view keeps its place: the
# timeline scrolls from the bottom)
def Tl.up(near: Bool, +n: Nui) -> Step:
  match near:
    case True{}:
      Nui.act(n, "earlier", "")
    case False{}:
      Step{n, Nil{}, True{}}

# a notch down the thread: back at the bottom, the pages the wheel added
# go (a frame lays out only the newest page again), and so does the pill
# (client.bend's Hold), in this very frame
def Tl.down(above: Bool, n: Nui) -> Step:
  match above n:
    case True{} _:
      Step{n, Nil{}, True{}}
    case False{} Nui{+u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}:
      Step{Nui{U.Hold.end(U.Ui.scratch_set(u, "@more." ++ U.Ui.sel(u), ""), ""), w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}, Nil{}, True{}}

def Nui.wheel.k(side: Bool, up: Bool, down: Bool, n: Nui, +sd: U32, +sc: U32, +most: U32, +tmost: U32, +more: Bool) -> Step:
  match side up down n:
    case True{} True{} _ Nui{u, w, h, mx, my, _, sc2, ca, fo, pa, hi, ms, vw}:
      Step{Nui{u, w, h, mx, my, Bool.pick(U32, U32.is_ge(sd, Nui.notch()), (sd - Nui.notch() : U32), 0), sc2, ca, fo, pa, hi, ms, vw}, Nil{}, U32.is_ne(sd, 0)}
    case True{} False{} True{} Nui{u, w, h, mx, my, _, sc2, ca, fo, pa, hi, ms, vw}:
      Step{Nui{u, w, h, mx, my, Side.down(sd, most), sc2, ca, fo, pa, hi, ms, vw}, Nil{}, U32.is_ne(Side.down(sd, most), sd)}
    case False{} True{} _ Nui{u, w, +h, mx, my, sd2, _, ca, fo, pa, hi, ms, vw}:
      Tl.up(Bool.and(more, U32.is_ge((sc + Nui.notch() + (h * 2 : U32) : U32), tmost)), Nui{u, w, h, mx, my, sd2, U32.min((sc + Nui.notch() : U32), tmost), ca, fo, pa, hi, ms, vw})
    case False{} False{} True{} Nui{u, w, h, mx, my, sd2, _, ca, fo, pa, hi, ms, vw}:
      Tl.down(U32.is_gt(sc, Nui.notch()), Nui{u, w, h, mx, my, sd2, Bool.pick(U32, U32.is_ge(sc, Nui.notch()), (sc - Nui.notch() : U32), 0), ca, fo, pa, hi, ms, vw})
    case _ _ _ _:
      Step{n, Nil{}, False{}}

# the wheel moves the sidebar and the timeline Nui.notch() a notch
def Nui.wheel(n: Nui, +x: U32, +dir: U32) -> Step:
  match n:
    case Nui{u, w, h, mx, my, +sd, +sc, ca, fo, +pa, +hi, ms, vw}:
      Nui.wheel.k(U32.is_lt(x, 264), U32.is_eq(dir, 0), U32.is_eq(dir, 1), Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}, sd, sc, Side.most(hi), Tl.most(hi), Hit.has(hi, "tl-more"))

def Nui.resize(n: Nui, +w: U32, +h: U32) -> Step:
  match n:
    case Nui{u, _, _, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}:
      Step{Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}, Nil{}, True{}}

# Hover
# -----
# The clickable region under the pointer lights up: half lit in the very
# next frame, fully lit Hover.ms() later; "hover.at" is "0" until the
# next frame stamps its start.

def Hover.ms() -> Nat:
  60n

# how lit the first frame is (0..255)
def Hover.floor() -> Nat:
  128n

def Hover.key(m: Maybe<&2, Hit>) -> String:
  match m:
    case Some{Hit{x0, y0, x1, y1, +a, _}}:
      Bool.pick(String, String.eq(a, "tl-line"), "", U32.show(x0) ++ "," ++ U32.show(y0) ++ "," ++ U32.show(x1) ++ "," ++ U32.show(y1))
    case None{}:
      ""

def Hover.alpha.k(+at: Nat, +ms: Nat) -> U32:
  Bool.pick(U32, Nat.is_ge(ms, Nat.add(at, Hover.ms())), 255,
    U32.from_nat(Nat.add(Hover.floor(), Nat.div(Nat.mul(Nat.sub(ms, at), Nat.sub(255n, Hover.floor())), Hover.ms()))))

def Nui.moved.k(n: Nui, +x: U32, +y: U32) -> Nui:
  match n:
    case Nui{u, w, h, _, _, sd, sc, ca, fo, pa, hi, ms, vw}:
      Nui{u, w, h, x, y, sd, sc, ca, fo, pa, hi, ms, vw}

def Step.merge(a: Step, b: Step) -> Step:
  match a b:
    case Step{_, c1, d1} Step{n2, c2, d2}:
      Step{n2, List.append(&2, NCmd, c1, c2), Bool.or(d1, d2)}

def Nui.ui(n: Nui) -> U.Ui:
  match n:
    case Nui{u, _, _, _, _, _, _, _, _, _, _, _, vw}:
      u

# Reading while the thread grows (client.bend's Hold, layout.bend's
# TL.hold): after each frame the timeline's place ("tl-at": sc answered,
# off, top, the page's first entry, visible entry and its content top) is kept as "@tl.at" with the thread,
# the window's sc becomes what is now below the view, and away from the
# end the page is pinned at its first entry ("@tl.pin.<thread>") and the
# client is told (U.Hold.away / U.Hold.end: the "N new" pill)

def Hold.part(+v: String, +i: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, String.split(v, '|'), i), "")

# the kept place, for the thread on show ("" when it was another's)
def Hold.of(+n: Nui) -> String:
  Bool.pick(String, String.eq(Hold.part(U.Ui.scratch(Nui.ui(n), "@tl.at"), 6n), U.Ui.sel(Nui.ui(n))), U.Ui.scratch(Nui.ui(n), "@tl.at"), "")

def Hold.pin(+u: U.Ui, +away: Bool, +first: String) -> U.Ui:
  U.Ui.scratch_set(u, "@tl.pin." ++ U.Ui.sel(u), Bool.pick(String, away, Bool.pick(String, String.is_empty(U.Ui.scratch(u, "@tl.pin." ++ U.Ui.sel(u))), first, U.Ui.scratch(u, "@tl.pin." ++ U.Ui.sel(u))), ""))

def Hold.side(away: Bool, +u: U.Ui) -> U.Ui:
  match away:
    case True{}:
      U.Hold.away(u, "")
    case False{}:
      U.Hold.end(u, "")

def Hold.ui(+u: U.Ui, +v: String, +sc: U32) -> U.Ui:
  Hold.pin(U.Ui.scratch_set(Hold.side(U32.is_ne(sc, 0), u), "@tl.at", v ++ "|" ++ U.Ui.sel(u)), U32.is_ne(sc, 0), Hold.part(v, 3n))

def Hold.take.k(has: Bool, +v: String, n: Nui) -> Nui:
  match has n:
    case True{} Nui{u, w, h, mx, my, sd, _, ca, fo, pa, hi, ms, vw}:
      Nui{Hold.ui(u, v, Maybe.default(&2, U32, U32.read(Hold.part(v, 0n)), 0)), w, h, mx, my, sd, Maybe.default(&2, U32, U32.read(Hold.part(v, 0n)), 0), ca, fo, pa, hi, ms, vw}
    case False{} _:
      n

def Hold.take(+n: Nui) -> Nui:
  Hold.take.k(Bool.not(String.is_empty(Hit.value(Nui.hits(n), "tl-at"))), Hit.value(Nui.hits(n), "tl-at"), n)

# the pill's click: back to the end (the pages the wheel added go too)
def Hold.bottom(n: Nui) -> Step:
  match n:
    case Nui{u, w, h, mx, my, sd, _, ca, fo, pa, hi, ms, vw}:
      Tl.down(False{}, Nui{u, w, h, mx, my, sd, 0, ca, fo, pa, hi, ms, vw})

def Nui.recv.act(+n: Nui, +o: J.Json) -> U.Act:
  Nui.recv.act.k(U.Ui.recv_act(Nui.ui(n), o), o)

def Jump.settle(s: Step) -> Step:
  match s:
    case Step{+n, cs, d}:
      Jump.to(Bool.and(String.eq(U.Ui.scratch(Nui.ui(n), "@jump.go"), "1"), Bool.not(String.is_empty(Jump.hit(Nui.hits(n))))), Step{n, cs, d})

# the picker closes itself once its project is added: the keys go back
# to the composer
def Nui.unpick(+n: Nui) -> Nui:
  Bool.pick(Nui, Bool.and(String.eq(Nui.focus_of(n), "proj"), Bool.not(U.Ui.flag(Nui.ui(n), "proj"))), Nui.with_focus(n, "composer"), n)

def Nui.unremote.k(promoted: Bool, n: Nui) -> Nui:
  match promoted n:
    case True{} Nui{+u, w, h, mx, my, sd, _, _, _, pa, hi, ms, vw}:
      Nui{u, w, h, mx, my, sd, 0, String.length(U.Ui.draft_text(u)), "composer", pa, hi, ms, vw}
    case False{} _:
      n

# A reported chat gaining its mirror replaces the form with the normal
# composer. Keep keyboard focus on the visible field after that change.
def Nui.unremote(+n: Nui) -> Nui:
  Nui.unremote.k(Bool.and(String.eq(Nui.focus_of(n), Fld.alias("remote-send")),
    Bool.and(String.is_empty(U.Ui.scratch(Nui.ui(n), "@remote")), Bool.not(String.is_empty(U.Ui.scratch(Nui.ui(n), "@bot"))))), n)

def Nui.recv(n: Nui, +o: J.Json) -> Nui:
  Nui.unpick(Nui.unremote(Nui.recv.k(n, o)))

def Undo.key(+n: Nui, +w: String) -> String:
  "@" ++ w ++ ":" ++ U.Ui.sel(Nui.ui(n))

def Undo.get(+n: Nui, +w: String) -> String:
  U.Ui.scratch(Nui.ui(n), Undo.key(n, w))

# how far the hover has faded in (0..255)
def Hover.alpha(+n: Nui, +ms: Nat) -> U32:
  Hover.alpha.k(Maybe.default(&2, Nat, Nat.read(U.Ui.info(Nui.ui(n), "hover.at")), 0n), ms)

# still fading in (the loop asks for frames only then)
def Hover.busy(+n: Nui, +ms: Nat) -> Bool:
  Bool.and(Bool.not(String.is_empty(U.Ui.info(Nui.ui(n), "hover"))), U32.is_lt(Hover.alpha(n, ms), 255))

# pasted text lands in the focused field at the caret
# The board viewer
# ----------------

def Nui.panel(n: Nui) -> Bool:
  match n:
    case Nui{U.Ui{_, _, _, _, _, _, _, _, pa, _, _, _, _, _, _, _, _}, _, _, _, _, _, _, _, _, _, _, _, _}:
      pa

def Nui.vx0.k(open: Bool, +w: U32) -> U32:
  Bool.pick(U32, open, (264 + U32.shr((w - 264 : U32)) : U32), w)

def Nui.width(n: Nui) -> U32:
  match n:
    case Nui{_, w, _, _, _, _, _, _, _, _, _, _, _}:
      w

# the viewer's left edge (the window's width when it is closed)
def Nui.vx0(+n: Nui) -> U32:
  Nui.vx0.k(Nui.panel(n), Nui.width(n))

def Nui.in_viewer(+n: Nui, +x: U32) -> Bool:
  Bool.and(Nui.panel(n), U32.is_ge(x, Nui.vx0(n)))

def Nui.with_vw(n: Nui, vw: V.Vw) -> Nui:
  match n:
    case Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, _}:
      Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}

def Nui.vw(n: Nui) -> V.Vw:
  match n:
    case Nui{_, _, _, _, _, _, _, _, _, _, _, _, vw}:
      vw

def Vw.drag(v: V.Vw, on: Bool, +x: U32, +y: U32) -> V.Vw:
  match v:
    case V.Vw{s, vi, f, r, _, _, _, b, fr, cu, vx}:
      V.Vw{s, vi, f, r, on, x, y, b, fr, cu, vx}

def Vx.kind(x: V.Vx) -> U32:
  match x:
    case V.Vx{k, _, _, _, _, _, _, _, _, _, bm}:
      k

def Vw.x(v: V.Vw) -> V.Vx:
  match v:
    case V.Vw{_, _, _, _, _, _, _, _, _, _, vx}:
      vx

def Vw.kind(v: V.Vw) -> U32:
  Vx.kind(Vw.x(v))

# moving to another version of the file on show (core/hist.bend: a step,
# a comparison, the live file again) keeps the scene and the view: the
# next read merges, so only what changed fades in; anything else starts over
def Vw.kind_set(+v: V.Vw, +k: U32, +path: String) -> V.Vw:
  Vw.kind_set.k(Bool.and(Bool.and(U32.is_eq(k, Vw.kind(v)), U32.is_lt(k, 2)),
      Bool.and(Bool.not(String.eq(path, Vw.path(v))), Bool.and(Bool.not(String.is_empty(HS.Hist.family(path))), String.eq(HS.Hist.family(path), HS.Hist.family(Vw.path(v)))))),
    v, k, path)

# the layers the user turned on or off in the kind on show (V.Look.on);
# kept per kind in the scratch, so a board's and a schematic's differ
def Nui.flip.key(+n: Nui) -> String:
  "@vw.flip." ++ U32.show(Vw.kind(Nui.vw(n)))

def Nui.flip(+n: Nui) -> U32:
  Maybe.default(&2, U32, U32.read(U.Ui.scratch(Nui.ui(n), Nui.flip.key(n))), 0)

# the layer focus of the kind on show (core/layers.bend: the active layer
# and how the others show), kept per kind like the flips
def Nui.lfocus.key(+n: Nui) -> String:
  "@vw.focus." ++ U32.show(Vw.kind(Nui.vw(n)))

def Nui.lfocus(+n: Nui) -> LY.Focus:
  LY.Lyr.read(U.Ui.scratch(Nui.ui(n), Nui.lfocus.key(n)))

# the focus as the viewer's kind word carries it (LY.Lyr.kind.with)
def Nui.lfocus.bits(+n: Nui) -> U32:
  LY.Lyr.kind.with(0, Nui.lfocus(n))

# Design review: a comparison's marks follow the active layer (client.bend's
# Rv.bits through the core action rv-bits)
def Rvw.bits(+n: Nui, +f: LY.Focus) -> Step:
  Nui.act(n, "rv-bits", U32.show(U.Rv.bits(Vw.kind(Nui.vw(n)), f)))

def Rvw.cat(cs: List<&2, NCmd>, s: Step) -> Step:
  match s:
    case Step{+n, +ds, d}:
      Step{n, List.append(&2, NCmd, cs, ds), True{}}

# Reset layers: every layer is compared again
def Rvw.reset(s: Step) -> Step:
  match s:
    case Step{+n, cs, d}:
      Rvw.cat(cs, Nui.act(n, "rv-bits", "0"))

# the viewer's light ground (its sun/moon button, "@vw.light"), and the
# window's light theme (Settings, Appearance: the hub's "ui.theme")
def Nui.vlight(+n: Nui) -> Bool:
  Bool.not(String.is_empty(U.Ui.scratch(Nui.ui(n), "@vw.light")))

def Nui.light(+n: Nui) -> Bool:
  String.eq(M.Setting.get(U.Ui.state.of(Nui.ui(n)), "ui.theme", ""), "light")

# what a frame draws with: the layers' flips and both grounds (viewer.bend's
# look word)
def Nui.look(+n: Nui) -> U32:
  U32.or(U32.or(U32.and(Nui.flip(n), 536870911), Bool.pick(U32, Nat.is_eq(HS.Hist.kind(Vw.path(Nui.vw(n))), 2n), V.Look.cmp.bit(), 0)),
    U32.or(Bool.pick(U32, Nui.vlight(n), V.Look.light.bit(), 0), Bool.pick(U32, Nui.light(n), V.Look.ui.bit(), 0)))

# SolidWorks' view controls (the 3D viewer)
# -----------------------------------------
# A drag (left or middle button) rotates the model about its centre; with
# ctrl it pans, with shift it zooms (up is in), with alt it rolls. The
# wheel zooms at the pointer, forward out. Keys: 1-7 the standard views,
# arrows turn 15 degrees (90 with shift), alt+left/right roll, ctrl+arrows
# pan, z out, shift+z in, f fits without turning.

# the drag's mode from the modifiers held: 0 rotate, 1 pan, 2 zoom, 3 roll
def Drag.mode(+mods: U32) -> U32:
  Bool.pick(U32, U32.is_ne(U32.and(mods, 2), 0), 1,
    Bool.pick(U32, U32.is_ne(U32.and(mods, 1), 0), 2,
      Bool.pick(U32, U32.is_ne(U32.and(mods, 4), 0), 3, 0)))

# the change of distance for a wheel notch: forward (dir 0) zooms out
def Wheel.dist(+dir: U32) -> F32:
  Bool.pick(F32, U32.is_eq(dir, 0), 1.2, 0.8333333)

# an arrow key as 0 left, 1 right, 2 up, 3 down; 4 for any other key
def Key3.arrow(+code: U32) -> U32:
  Bool.pick(U32, U32.is_eq(code, Key.left()), 0, Bool.pick(U32, U32.is_eq(code, Key.right()), 1,
    Bool.pick(U32, U32.is_eq(code, Key.up()), 2, Bool.pick(U32, U32.is_eq(code, Key.down()), 3, 4))))

def Key3.arrow.k(+ar: U32, +mods: U32) -> U32:
  Bool.pick(U32, U32.is_ne(U32.and(mods, 2), 0), (20 + ar : U32),
    Bool.pick(U32, U32.is_ne(U32.and(mods, 4), 0), Bool.pick(U32, U32.is_lt(ar, 2), (18 + ar : U32), 0),
      Bool.pick(U32, U32.is_ne(U32.and(mods, 1), 0), (14 + ar : U32), (10 + ar : U32))))

def Key3.act.k(+ar: U32, +code: U32, +mods: U32) -> U32:
  Bool.pick(U32, U32.is_lt(ar, 4), Key3.arrow.k(ar, mods),
    Bool.pick(U32, U32.is_ne(U32.and(mods, 6), 0), 0,
      Bool.pick(U32, Bool.and(U32.is_ge(code, 49), U32.is_le(code, 55)), (code - 48 : U32),
        Bool.pick(U32, U32.is_eq(code, 122), 8, Bool.pick(U32, U32.is_eq(code, 90), 9, 0)))))

# a key's view action, 0 for none: 1-7 front, back, left, right, top,
# bottom, isometric (SolidWorks' ctrl+1..7; bare here, as ctrl+digit picks a
# thread); 8 zoom out (z), 9 in (shift+z); arrows 10-13 turn 15 degrees
# (left, right, up, down), 14-17 with shift 90; 18, 19 alt+left/right roll;
# 20-23 ctrl+arrows pan
def Key3.act(+code: U32, +mods: U32) -> U32:
  Key3.act.k(Key3.arrow(code), code, mods)

# a key's action (Key3.act) on the camera; s world units a pixel
# 15 and 90 degrees
def Key3.q() -> F32:
  0.2617994

def Key3.h() -> F32:
  1.5707964

def Cam.key(+a: U32, c: MS.Cam, +s: F32) -> MS.Cam:
  match a:
    case 1:
      MS.Cam.face(c, 0.0, 0.0)
    case 2:
      MS.Cam.face(c, 3.1415927, 0.0)
    case 3:
      MS.Cam.face(c, F32.neg(Key3.h()), 0.0)
    case 4:
      MS.Cam.face(c, Key3.h(), 0.0)
    case 5:
      MS.Cam.face(c, 0.0, Key3.h())
    case 6:
      MS.Cam.face(c, 0.0, F32.neg(Key3.h()))
    case 7:
      MS.Cam.face(c, 0.7853982, 0.6154797)
    case 8:
      MS.Cam.zoom(c, 1.25)
    case 9:
      MS.Cam.zoom(c, 0.8)
    case 10:
      MS.Cam.orbit(c, F32.neg(Key3.q()), 0.0)
    case 11:
      MS.Cam.orbit(c, Key3.q(), 0.0)
    case 12:
      MS.Cam.orbit(c, 0.0, F32.neg(Key3.q()))
    case 13:
      MS.Cam.orbit(c, 0.0, Key3.q())
    case 14:
      MS.Cam.orbit(c, F32.neg(Key3.h()), 0.0)
    case 15:
      MS.Cam.orbit(c, Key3.h(), 0.0)
    case 16:
      MS.Cam.orbit(c, 0.0, F32.neg(Key3.h()))
    case 17:
      MS.Cam.orbit(c, 0.0, Key3.h())
    case 18:
      MS.Cam.roll(c, F32.neg(Key3.q()))
    case 19:
      MS.Cam.roll(c, Key3.q())
    case 20:
      MS.Cam.pan(c, F32.neg(48.0), 0.0, s)
    case 21:
      MS.Cam.pan(c, 48.0, 0.0, s)
    case 22:
      MS.Cam.pan(c, 0.0, F32.neg(48.0), s)
    case 23:
      MS.Cam.pan(c, 0.0, 48.0, s)
    case _:
      c

# a drag of (dx, dy) pixels in mode m (Drag.mode), s world units a pixel;
# (ox, oy) is the press, off the panel's centre (shift zooms about it)
def Cam.drag(+m: U32, c: MS.Cam, +dx: F32, +dy: F32, +s: F32, +ox: F32, +oy: F32) -> MS.Cam:
  match m:
    case 1:
      MS.Cam.pan(c, dx, dy, s)
    case 2:
      MS.Cam.zoom_at(c, F32.max(0.2, F32.add(1.0, F32.mul(dy, 0.005))), ox, oy, s)
    case 3:
      MS.Cam.roll(c, F32.mul(dx, 0.01))
    case _:
      MS.Cam.orbit(c, F32.mul(dx, 0.01), F32.mul(dy, 0.01))

# the panel the model is drawn in: its left edge, width and height (its
# top is at 52)
type Pnl is Data:
  Pnl{x0: U32, w: U32, h: U32}

def Pnl.cx(+p: Pnl, +x: U32) -> F32:
  match p:
    case Pnl{x0, w, _}:
      F32.sub(U32.to_f32(x), F32.add(U32.to_f32(x0), F32.mul(0.5, U32.to_f32(w))))

def Pnl.cy(+p: Pnl, +y: U32) -> F32:
  match p:
    case Pnl{_, _, h}:
      F32.sub(U32.to_f32(y), F32.add(52.0, F32.mul(0.5, U32.to_f32(h))))

def Pnl.unit(+p: Pnl, +c: MS.Cam) -> F32:
  match p:
    case Pnl{_, w, h}:
      MS.Cam.unit(c, w, h)

def Vx.drag(x: V.Vx, +m: U32, +dx: F32, +dy: F32, +pn: Pnl) -> V.Vx:
  match x:
    case V.Vx{k, p, t, +c, sh, bu, +px, +py, pa, se, bm}:
      V.Vx{k, p, t, [Cam.drag(m, V.Vx.cam(c), dx, dy, Pnl.unit(pn, V.Vx.cam(c)), Pnl.cx(pn, px), Pnl.cy(pn, py))], sh, bu, px, py, pa, se, bm}

def Vx.wheel(x: V.Vx, +f: F32, +mx: U32, +my: U32, +pn: Pnl) -> V.Vx:
  match x:
    case V.Vx{k, p, t, +c, sh, bu, px, py, pa, se, bm}:
      V.Vx{k, p, t, [MS.Cam.zoom_at(V.Vx.cam(c), f, Pnl.cx(pn, mx), Pnl.cy(pn, my), Pnl.unit(pn, V.Vx.cam(c)))], sh, bu, px, py, pa, se, bm}

def Vx.key(x: V.Vx, +a: U32, +pn: Pnl) -> V.Vx:
  match x:
    case V.Vx{k, p, t, +c, sh, bu, px, py, pa, se, bm}:
      V.Vx{k, p, t, [Cam.key(a, V.Vx.cam(c), Pnl.unit(pn, V.Vx.cam(c)))], sh, bu, px, py, pa, se, bm}

def Vx.zoom(x: V.Vx, +f: F32) -> V.Vx:
  match x:
    case V.Vx{k, p, t, +c, sh, bu, px, py, pa, se, bm}:
      V.Vx{k, p, t, [MS.Cam.zoom(V.Vx.cam(c), F32.div(1.0, f))], sh, bu, px, py, pa, se, bm}

# a new scene, seen from the first side
def Vx.fit(x: V.Vx) -> V.Vx:
  match x:
    case V.Vx{k, p, +t, _, sh, bu, px, py, pa, se, bm}:
      V.Vx{k, p, t, [FA.Scene3.fit(t)], sh, bu, px, py, pa, se, bm}

# zoom to fit, turned as it is
def Vx.refit(x: V.Vx) -> V.Vx:
  match x:
    case V.Vx{k, p, +t, +c, sh, bu, px, py, pa, se, bm}:
      V.Vx{k, p, t, [FA.Scene3.refit(t, V.Vx.cam(c))], sh, bu, px, py, pa, se, bm}

# a drag pans a board or schematic and moves a 3D model's camera (the
# modifiers pick how, Drag.mode)
def Vw.moved.k(three: Bool, v: V.Vw, +x: U32, +y: U32, +mods: U32, +pn: Pnl) -> V.Vw:
  match three v:
    case False{} V.Vw{s, vi, f, r, True{}, +lx, +ly, b, _, cu, vx}:
      V.Vw{s, V.View.pan(vi, F32.sub(U32.to_f32(x), U32.to_f32(lx)), F32.sub(U32.to_f32(y), U32.to_f32(ly))), f, r, True{}, x, y, b, False{}, cu, vx}
    case True{} V.Vw{s, vi, f, r, True{}, +lx, +ly, b, _, cu, vx}:
      V.Vw{s, vi, f, r, True{}, x, y, b, False{}, cu, Vx.drag(vx, Drag.mode(mods), F32.sub(U32.to_f32(x), U32.to_f32(lx)), F32.sub(U32.to_f32(y), U32.to_f32(ly)), pn)}
    case _ _:
      v

def Vw.moved(+v: V.Vw, +x: U32, +y: U32, +mods: U32, +pn: Pnl) -> V.Vw:
  Vw.moved.k(U32.is_eq(Vw.kind(v), 2), v, x, y, mods, pn)

def Vw.key(v: V.Vw, +a: U32, +pn: Pnl) -> V.Vw:
  match v:
    case V.Vw{s, vi, f, r, d, lx, ly, b, _, cu, vx}:
      V.Vw{s, vi, f, r, d, lx, ly, b, False{}, cu, Vx.key(vx, a, pn)}

def Vw.zoom.k(three: Bool, v: V.Vw, +f: F32, +x: U32, +y: U32) -> V.Vw:
  match three v:
    case False{} V.Vw{s, vi, fi, r, d, lx, ly, b, _, cu, vx}:
      V.Vw{s, V.View.zoom(vi, f, U32.to_f32(x), U32.to_f32(y)), fi, r, d, lx, ly, b, False{}, cu, vx}
    case True{} V.Vw{s, vi, fi, r, d, lx, ly, b, _, cu, vx}:
      V.Vw{s, vi, fi, r, d, lx, ly, b, False{}, cu, Vx.zoom(vx, f)}

def Vw.scroll(v: V.Vw, +up: Bool) -> V.Vw:
  match v:
    case V.Vw{s, V.View{z, ox, +oy}, fi, r, d, lx, ly, b, fr, cu, vx}:
      V.Vw{s, V.View{z, ox, Bool.pick(F32, up, F32.max(F32.sub(oy, 66.0), 0.0), F32.add(oy, 66.0))}, fi, r, d, lx, ly, b, fr, cu, vx}

# the diff view scrolls on the wheel; the others zoom
def Vw.zoom.d(diff: Bool, +v: V.Vw, +f: F32, +x: U32, +y: U32) -> V.Vw:
  match diff:
    case True{}:
      Vw.scroll(v, F32.is_gt(f, 1.0))
    case False{}:
      Vw.zoom.k(U32.is_eq(Vw.kind(v), 2), v, f, x, y)

def Vw.zoom(+v: V.Vw, +f: F32, +x: U32, +y: U32) -> V.Vw:
  Vw.zoom.d(U32.is_eq(Vw.kind(v), 4), v, f, x, y)

# the wheel zooms a 3D model at the pointer (SolidWorks: forward is out)
# and a board or schematic as before
def Vw.wheel.k(three: Bool, v: V.Vw, +dir: U32, +x: U32, +y: U32, +pn: Pnl) -> V.Vw:
  match three v:
    case True{} V.Vw{s, vi, fi, r, d, lx, ly, b, _, cu, vx}:
      V.Vw{s, vi, fi, r, d, lx, ly, b, False{}, cu, Vx.wheel(vx, Wheel.dist(dir), x, y, pn)}
    case False{} _:
      Vw.zoom(v, Bool.pick(F32, U32.is_eq(dir, 0), 1.25, 0.8), x, y)

def Vw.wheel(+v: V.Vw, +dir: U32, +x: U32, +y: U32, +pn: Pnl) -> V.Vw:
  Vw.wheel.k(U32.is_eq(Vw.kind(v), 2), v, dir, x, y, pn)

def Vw.refit(v: V.Vw) -> V.Vw:
  match v:
    case V.Vw{s, vi, _, r, d, lx, ly, b, _, cu, vx}:
      V.Vw{s, vi, False{}, r, d, lx, ly, b, False{}, cu, Vx.refit(vx)}

# a key press (releases do nothing)
# The command palette
# -------------------

def Nui.with_ui(n: Nui, u: U.Ui) -> Nui:
  match n:
    case Nui{_, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}:
      Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}

def Nui.sync.k(a: U.Act, n: Nui) -> Step:
  match a:
    case U.Act{u, cs}:
      Step{Nui.with_ui(n, u), Nui.cmds(cs), True{}}

# a message's state with what the client asks for after it (the
# Mechanical page's parts, client.bend's Mpage.sync)
def Nui.sync(+n: Nui) -> Step:
  Nui.sync.k(U.Mpage.sync(U.Act{Nui.ui(n), Nil{}}), n)

# the same, drawing a frame only when d (a message another thread's
# stream sent shows nothing: U.Ui.shows)
def Nui.sync.kd(a: U.Act, n: Nui, +d: Bool) -> Step:
  match a:
    case U.Act{u, +cs}:
      Step{Nui.with_ui(n, u), Nui.cmds(cs), Bool.or(d, Bool.not(List.is_empty(&2, U.Cmd, cs)))}

def Nui.sync.d(+n: Nui, +d: Bool) -> Step:
  Nui.sync.kd(U.Mpage.sync(U.Act{Nui.ui(n), Nil{}}), n, d)

def Undo.put(+n: Nui, +w: String, v: String) -> Nui:
  Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), Undo.key(n, w), v))

def Undo.clear(+n: Nui) -> Nui:
  Undo.put(Undo.put(Undo.put(n, "undo", ""), "redo", ""), "ekind", "")

# open the palette in a mode ("" commands, "search", "files") with a query
def Pal.open(+n: Nui, +mode: String, +q: String) -> Step:
  Nui.focus(Nui.with_ui(n, U.Palette.set(U.Palette.open(Nui.ui(n), mode), q, 0n)), "palette")

def Hover.set.k(same: Bool, +n: Nui, +key: String) -> Step:
  match same:
    case True{}:
      Step{n, Nil{}, False{}}
    case False{}:
      Step{Nui.with_ui(n, U.Ui.info_set(U.Ui.info_set(Nui.ui(n), "hover", key), "hover.at", "0")), Nil{}, True{}}

def Hover.set(+n: Nui, +key: String) -> Step:
  Hover.set.k(String.eq(U.Ui.info(Nui.ui(n), "hover"), key), n, key)

# the frame stamps when a new hover began
def Hover.stamp(+n: Nui, +ms: Nat) -> Nui:
  Bool.pick(Nui, String.eq(U.Ui.info(Nui.ui(n), "hover.at"), "0"), Nui.with_ui(n, U.Ui.info_set(Nui.ui(n), "hover.at", Nat.show(ms))), n)

# a frame's hits can move what is under a still pointer (a fold opened, a
# scroll, the hub's news): the hover is found again in them, so it never
# stays lit where a button was. Dirty when it changed
def Hover.again(n: Nui) -> Step:
  match n:
    case Nui{u, w, h, +mx, +my, sd, sc, ca, fo, pa, +hi, ms, vw}:
      Hover.set(Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}, Hover.key(Hit.find(hi, mx, my)))

# Blink
# -----
# A focused field's caret blinks: shown for Blink.ms(), hidden as long. It
# restarts shown whenever the focus or the caret moves (law
# caret_shows_when_moved). Only a focused field asks for frames, one per
# toggle.

def Blink.ms() -> Nat:
  530n

# shown in the even half-periods since the caret last moved
def Blink.shown(+d: Nat) -> Bool:
  Nat.is_eq(Nat.mod(Nat.div(d, Blink.ms()), 2n), 0n)

def Nui.caret_of(n: Nui) -> Nat:
  match n:
    case Nui{_, _, _, _, _, _, _, ca, _, _, _, _, vw}:
      ca

# The selection and undo
# ----------------------
# Kept in the scratch map (local, never sent). The selection's other end
# ("@mark") holds only while the field, thread and caret it was set with
# do, so anything else that moves the caret drops it. Each thread's draft
# has its own undo and redo history (core/edit.bend's Edit.push).

def Mark.sig(+n: Nui) -> String:
  Nui.focus_of(n) ++ ":" ++ U.Ui.sel(Nui.ui(n)) ++ ":" ++ Nat.show(Nui.caret_of(n)) ++ "|"

def Mark.save(+n: Nui, +m: Nat) -> Nui:
  Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), "@mark", Mark.sig(n) ++ Nat.show(m)))

def Mark.set(s: Step, +m: Nat) -> Step:
  match s:
    case Step{+n, cs, d}:
      Step{Mark.save(n, m), cs, d}

# the selection's other end in the focused field (the caret when nothing
# is selected)
def Nui.mark(+n: Nui) -> Nat:
  Mark.of(U.Ui.scratch(Nui.ui(n), "@mark"), Mark.sig(n), Nui.caret_of(n))

def Undo.here(+n: Nui, +h: String) -> String:
  ED.Edit.push(h, Nui.caret_of(n), U.Ui.draft_text(Nui.ui(n)))

def Undo.note.k(fresh: Bool, +n: Nui) -> Nui:
  match fresh:
    case True{}:
      Undo.put(n, "undo", Undo.here(n, Undo.get(n, "undo")))
    case False{}:
      n

# before a change of kind k the draft as it is becomes an undo step, unless
# it continues a run of typing; a change forgets what was undone
def Undo.note(+n: Nui, +k: Nat) -> Nui:
  Undo.put(Undo.put(Undo.note.k(Bool.or(Bool.not(Nat.is_eq(k, 1n)), Bool.not(String.eq(Undo.get(n, "ekind"), "1"))), n), "redo", ""), "ekind", Nat.show(k))

# undo (from "undo" to "redo") or redo (the other way): the newest step of
# `from` becomes the draft, the draft goes on `to`
def Undo.go(has: Bool, +n: Nui, +from: String, +to: String) -> Step:
  match has:
    case True{}:
      Step.caret(Nui.act(Undo.put(Undo.put(Undo.put(n, from, ED.Edit.pop(Undo.get(n, from))), to, Undo.here(n, Undo.get(n, to))), "ekind", ""), "draft",
        ED.Edit.top.text(Undo.get(n, from))), ED.Edit.top.caret(Undo.get(n, from)))
    case False{}:
      Step{n, Nil{}, False{}}

def Undo.run(+n: Nui, +from: String, +to: String) -> Step:
  Undo.go(Bool.not(String.is_empty(Undo.get(n, from))), n, from, to)

# Dictation
# ---------
# A finished segment (a voice "text" message) for the selected thread goes
# in at the caret while the composer has focus ("@voice.at"; the client's
# U.Voice.heard puts it there), the caret moves past it ("@voice.to"), and
# undo takes it back out; another thread's goes at its draft's end. Either
# way the draft is written down, as typing does.

def Hear.is(+o: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(o, "t", ""), "voice"), String.eq(J.Json.get_str(o, "ev", ""), "text"))

def Hear.here(+n: Nui, +o: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(o, "thread", ""), U.Ui.sel(Nui.ui(n))), String.eq(Nui.focus_of(n), "composer"))

def Hear.mark(+m: Nui, +c: Nat) -> Nui:
  Nui.with_ui(m, U.Ui.scratch_set(Nui.ui(m), "@voice.at", Nat.show(c)))

def Hear.at(here: Bool, +n: Nui) -> Nui:
  match here:
    case True{}:
      Hear.mark(Undo.note(n, 2n), Nui.caret_of(n))
    case False{}:
      n

def Hear.to(here: Bool, +n: Nui) -> Nui:
  match here:
    case True{}:
      Nui.with_caret(n, Maybe.default(&2, Nat, Nat.read(U.Ui.scratch(Nui.ui(n), "@voice.to")), Nui.caret_of(n)))
    case False{}:
      n

def Hear.kept(+n: Nui, +th: String) -> Step:
  Step{n, [NKeep{th, U.Ui.scratch(Nui.ui(n), th)}], True{}}

# after a message the keys stay on a visible field (Nui.recv's unpick and
# unremote, for the path through the client's own receive)
def Hear.refocus(s: Step) -> Step:
  match s:
    case Step{n, cs, d}:
      Step{Nui.unpick(Nui.unremote(n)), cs, d}

def Hear.k(text: Bool, +here: Bool, +n: Nui, +o: J.Json) -> Step:
  match text:
    case True{}:
      Hear.kept(Hear.to(here, Nui.recv(Hear.at(here, n), o)), J.Json.get_str(o, "thread", ""))
    case False{}:
      Hear.refocus(Nui.sync.kd(Nui.recv.act(n, o), n, U.Ui.shows(Nui.ui(n), o)))

# a message from the hub (another thread's streamed text draws nothing,
# U.Ui.shows)
def Nui.hear(+n: Nui, +o: J.Json) -> Step:
  Hear.k(Hear.is(o), Hear.here(n, o), n, o)

def Nui.composer.m(k: Nat, +n: Nui, +c: Nat, +m: Nat, +cl: String) -> Step:
  match k:
    case 4n:
      Undo.run(n, "undo", "redo")
    case 5n:
      Undo.run(n, "redo", "undo")
    case _:
      Mark.set(Clip.also(Step{Nui.with_caret(n, c), Nil{}, True{}}, cl), m)

# Enter sends; ctrl+Enter sends with the other follow-up mode (queue or
# steer, U.Follow.mode)
def Nui.composer.k(e: Ed, n: Nui, +alt: Bool) -> Step:
  match e:
    case Ed{_, _, _, True{}, _, _, _}:
      Step.caret(Nui.act(Undo.clear(n), Bool.pick(String, alt, "send-alt", "send"), ""), 0n)
    case Ed{+t, +c, +m, False{}, True{}, +cl, +k}:
      Mark.set(Clip.also(Step.caret(Nui.act(Undo.note(n, k), "draft", t), c), cl), m)
    case Ed{_, +c, +m, False{}, False{}, +cl, +k}:
      Nui.composer.m(k, n, c, m, cl)

def Nui.composer.sk(complete: Bool, e: Ed, n: Nui, +alt: Bool) -> Step:
  match complete:
    case True{}:
      Nui.skill(n)
    case False{}:
      Nui.composer.k(e, n, alt)

# up and down move the menu's highlight
def Nui.composer.ar(arrow: Bool, +code: U32, +e: Ed, n: Nui, +alt: Bool) -> Step:
  match arrow:
    case True{}:
      Nui.act(n, "skill-move", Bool.pick(String, U32.is_eq(code, Key.down()), "down", "up"))
    case False{}:
      Nui.composer.sk(Bool.or(U32.is_eq(code, Key.tab()), Ed.submit(e)), e, n, alt)

# while skills or references are offered, the arrow keys move the
# highlight and Tab and Enter complete the highlighted one
def Nui.composer.s(has: Bool, +code: U32, +e: Ed, n: Nui, +alt: Bool) -> Step:
  match has:
    case True{}:
      Nui.composer.ar(Bool.or(U32.is_eq(code, Key.up()), U32.is_eq(code, Key.down())), code, e, n, alt)
    case False{}:
      Nui.composer.k(e, n, alt)

def Blink.sig(+n: Nui) -> String:
  Nui.focus_of(n) ++ ":" ++ Nat.show(Nui.caret_of(n)) ++ ":" ++ U.Palette.query(Nui.ui(n)) ++ ":" ++ U.Proj.query(Nui.ui(n))

def Blink.at(+n: Nui) -> Nat:
  Maybe.default(&2, Nat, Nat.read(U.Ui.info(Nui.ui(n), "caret.at")), 0n)

def Blink.set(+n: Nui, +ms: Nat) -> Nui:
  Nui.with_ui(n, U.Ui.info_set(U.Ui.info_set(Nui.ui(n), "caret.sig", Blink.sig(n)), "caret.at", Nat.show(ms)))

def Blink.on.k(+n: Nui, +ms: Nat) -> Nui:
  Nui.with_ui(n, U.Ui.info_set(Nui.ui(n), "caret.on", Bool.pick(String, Blink.shown(Nat.sub(ms, Blink.at(n))), "1", "")))

# the frame notes whether the caret shows (restarting it when it moved)
def Blink.stamp(+n: Nui, +ms: Nat) -> Nui:
  Blink.on.k(Bool.pick(Nui, String.eq(U.Ui.info(Nui.ui(n), "caret.sig"), Blink.sig(n)), n, Blink.set(n, ms)), ms)

def Blink.on(+n: Nui) -> Bool:
  String.eq(U.Ui.info(Nui.ui(n), "caret.on"), "1")

# ms until the caret toggles; 0 when no field is focused
def Blink.wait(+n: Nui, +ms: Nat) -> Nat:
  Bool.pick(Nat, String.is_empty(Nui.focus_of(n)), 0n, Nat.sub(Blink.ms(), Nat.mod(Nat.sub(ms, Blink.at(n)), Blink.ms())))

def Nui.moved(+n: Nui, +x: U32, +y: U32) -> Step:
  Hover.set(Nui.moved.k(n, x, y), Hover.key(Hit.find(Nui.hits(n), x, y)))

def Pal.close(+n: Nui) -> Step:
  Nui.focus(Nui.with_ui(n, U.Ui.flag_off(Nui.ui(n), "palette")), "composer")

def Pal.moved(+n: Nui, +i: Nat) -> Step:
  Step{Nui.with_ui(n, U.Palette.set(Nui.ui(n), U.Palette.query(Nui.ui(n)), Nat.min(i, Nat.sub(List.length(&2, U.PItem, U.Palette.items(Nui.ui(n))), 1n)))), Nil{}, True{}}

def Pal.typed(e: Ed, +n: Nui) -> Step:
  match e:
    case Ed{+t, _, _, _, _, _, _}:
      Step{Nui.with_ui(n, U.Palette.set(Nui.ui(n), t, 0n)), Nil{}, True{}}

# Shortcuts (T3Code's, ctrl for mod)
# ---------------------------------

def Keys.ctrl(+mods: U32) -> Bool:
  U32.is_ne(U32.and(mods, 2), 0)

def Keys.shift(+mods: U32) -> Bool:
  U32.is_ne(U32.and(mods, 1), 0)

def Keys.name.k(ctrl: Bool, shift: Bool, term: Bool, +c: U32) -> String:
  match ctrl shift term:
    case True{} False{} False{}:
      Bool.pick(String, U32.is_eq(c, 106), "term", Bool.pick(String, U32.is_eq(c, 100), "diff", Bool.pick(String, U32.is_eq(c, 110), "new", Bool.pick(String, U32.is_eq(c, 107), "palette",
        Bool.pick(String, U32.is_eq(c, 112), "files", Bool.pick(String, Bool.and(U32.is_ge(c, 49), U32.is_le(c, 57)), "jump", Bool.pick(String, U32.is_eq(c, 104), "layers", "")))))))
    case True{} False{} True{}:
      Bool.pick(String, U32.is_eq(c, 106), "term", "")
    case True{} True{} _:
      Bool.pick(String, U32.is_eq(c, 123), "prev", Bool.pick(String, U32.is_eq(c, 125), "next", Bool.pick(String, U32.is_eq(c, 91), "prev",
        Bool.pick(String, U32.is_eq(c, 93), "next", Bool.pick(String, U32.is_eq(c, 115), "settle", Bool.pick(String, U32.is_eq(c, 112), "pin",
          Bool.pick(String, U32.is_eq(c, 118), "paste", Bool.pick(String, U32.is_eq(c, 102), "search", Bool.pick(String, U32.is_eq(c, 104), "perf", "")))))))))
    case _ _ _:
      ""

# the shortcut a key is ("" for none). In the terminal only ctrl+j and
# ctrl+shift keys are shortcuts; the rest belong to the shell
def Keys.name.alt(alt: Bool, +code: U32, +mods: U32, +term: Bool) -> String:
  match alt:
    case True{}:
      "project"
    case False{}:
      Keys.name.k(Keys.ctrl(mods), Keys.shift(mods), term, U32.or(code, 32))

# alt+1..9 (alt alone) picks a project, even in the terminal
def Keys.name(+code: U32, +mods: U32, +term: Bool) -> String:
  Keys.name.alt(Bool.and(U32.is_eq(U32.and(mods, 7), 4), Bool.and(U32.is_ge(code, 49), U32.is_le(code, 57))), code, mods, term)

def Keys.threads.k(keep: Bool, +v: String, rest: List<&2, String>) -> List<&2, String>:
  match keep:
    case True{}:
      v <> rest
    case False{}:
      rest

# the threads in sidebar order (the last frame's select regions)
def Keys.threads(hs: List<&2, Hit>) -> List<&2, String>:
  match hs:
    case Nil{}:
      Nil{}
    case Hit{_, _, _, _, +a, +v} <> t:
      Keys.threads.k(Bool.and(String.eq(a, "select"), Bool.not(String.is_empty(v))), v, Keys.threads(t))

def Keys.after(ts: List<&2, String>, +cur: String) -> String:
  match ts:
    case +a <> +b <> t:
      Bool.pick(String, String.eq(a, cur), b, Keys.after(b <> t, cur))
    case _:
      ""

def Keys.neighbour.k(+ts: List<&2, String>, +cur: String, next: Bool) -> String:
  Bool.pick(String, next, Keys.after(ts, cur), Keys.after(List.reverse(&2, String, ts), cur))

# the thread next to the selected one (dedup: the sidebar's rows come
# before other select regions in the frame)
def Keys.neighbour(+hs: List<&2, Hit>, +cur: String, +next: Bool) -> String:
  Keys.neighbour.k(Keys.threads(hs), cur, next)

# the terminal panel's size in cells, from the window's size
def Term.cols(+n: Nui) -> U32:
  ((Nui.vx0(n) - 264 - 24 : U32) / 8 : U32)

def Step.nui(s: Step) -> Nui:
  match s:
    case Step{n, _, _}:
      n

def Nui.focus.step(s: Step, +fo: String) -> Step:
  match s:
    case Step{+n, cs, d}:
      Step.merge(Step{n, cs, d}, Nui.focus(n, fo))

# window modifiers (1 shift, 2 ctrl, 4 alt) as xterm's (1 shift, 4 ctrl, 8 alt)
def Keys.vt(+m: U32) -> U32:
  U32.or(U32.or(U32.and(m, 1), (U32.and(m, 2) * 2 : U32)), (U32.and(m, 4) * 2 : U32))

def Vw.stale(v: V.Vw) -> V.Vw:
  match v:
    case V.Vw{s, vi, f, r, d, lx, ly, b, _, cu, vx}:
      V.Vw{s, vi, f, r, d, lx, ly, b, False{}, cu, vx}

def Nui.adopt.k(n: Nui, st: M.State, head: J.Json) -> Nui:
  match n:
    case Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}:
      Nui{U.Ui.adopt_log(u, st, head), w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}

def Nui.adopt.theme(changed: Bool, +n: Nui) -> Nui:
  match changed:
    case True{}:
      Nui.with_vw(n, Vw.stale(Nui.vw(n)))
    case False{}:
      n

# the hub's read model, handed over (main.bend's Shell.adopt); a new theme
# draws the viewer's base again (its plain part is the window's ground)
def Nui.adopt(+n: Nui, +st: M.State, head: J.Json) -> Nui:
  Nui.adopt.theme(Bool.not(String.eq(M.Setting.get(st, "ui.theme", ""), M.Setting.get(U.Ui.state.of(Nui.ui(n)), "ui.theme", ""))), Nui.adopt.k(n, st, head))

# the same, with the outbox sent again: the hub's own window joins without
# a socket, so what was waiting to be sent goes now (the hub stores a
# resent message once)
def Nui.adopt.out(+n: Nui) -> Step:
  Step{n, Nui.cmds(U.Ui.outbox(Nui.ui(n))), True{}}

def Nui.adopt.step(+n: Nui, +st: M.State, head: J.Json) -> Step:
  Nui.adopt.out(Nui.adopt(n, st, head))

# A new machine drops native focus, scroll, hits, selections and scene as
# well as core selectors. A cached read model is safe to display at once.
def Nui.connection(n: Nui, +tag: String, cached: Maybe<&2, U.HubView>) -> Nui:
  match n:
    case Nui{u, w, h, _, _, _, _, _, _, _, _, _, _}:
      Nui.init(U.Ui.cached(cached, U.Ui.connection(u, tag)), w, h)

# a board file arrived for a project root: same root merges (only what
# changed fades), a new root starts over (the whole board fades in)
def Vx.shown(x: V.Vx) -> String:
  match x:
    case V.Vx{_, _, _, _, sh, _, _, _, _, _, bm}:
      sh

def Vx.busy(x: V.Vx, bu: String) -> V.Vx:
  match x:
    case V.Vx{k, p, t, c, sh, _, px, py, pa, se, bm}:
      V.Vx{k, p, t, c, sh, bu, px, py, pa, se, bm}

# a scene file arrived for a source: the same source merges (only what
# changed fades), another starts over (the whole scene fades in)
def Vw.board.k(same: Bool, v: V.Vw, +root: String, +items: List<&2, B.Item>, +now: Nat) -> V.Vw:
  match same v:
    case True{} V.Vw{+s, vi, f, _, d, lx, ly, b, _, cu, vx}:
      V.Vw{V.Scene.update(s, items, now), vi, f, root, d, lx, ly, b, False{}, cu, Vx.busy(vx, "")}
    case False{} V.Vw{_, vi, _, _, d, lx, ly, b, _, cu, vx}:
      V.Vw{V.Scene.update(Nil{}, items, now), vi, False{}, root, d, lx, ly, b, False{}, cu, Vx.busy(vx, "")}

def Vx.mesh.k(fit: Bool, x: V.Vx) -> V.Vx:
  match fit:
    case True{}:
      Vx.fit(x)
    case False{}:
      x

# a new scene; the camera fits it only when there was none (a board
# saved again keeps the view)
def Vx.mesh(x: V.Vx, ts: FA.Scene3) -> V.Vx:
  match x:
    case V.Vx{k, p, old, c, sh, _, px, py, pa, se, bm}:
      Vx.mesh.k(FA.Scene3.is_empty(old), V.Vx{k, p, ts, c, sh, "", px, py, pa, se, bm})

# parts of the scene's board, arrived at `now`
def Vx.parts(x: V.Vx, +t0: Nat, ts: List<&2, G.Tri3>, +now: Nat) -> V.Vx:
  match x:
    case V.Vx{k, p, s, c, sh, bu, px, py, pa, se, bm}:
      V.Vx{k, p, FA.Scene3.add_if(s, t0, ts, now), c, sh, bu, px, py, pa, se, bm}

def Vw.parts(v: V.Vw, +t0: Nat, ts: List<&2, G.Tri3>, +now: Nat) -> V.Vw:
  match v:
    case V.Vw{s, vi, f, r, d, lx, ly, b, _, cu, vx}:
      V.Vw{s, vi, f, r, d, lx, ly, b, False{}, cu, Vx.parts(vx, t0, ts, now)}

def Vw.mesh(v: V.Vw, ts: FA.Scene3) -> V.Vw:
  match v:
    case V.Vw{s, vi, f, r, d, lx, ly, b, _, cu, vx}:
      V.Vw{s, vi, f, r, d, lx, ly, b, False{}, cu, Vx.mesh(vx, ts)}

def Vw.status(v: V.Vw, +text: String) -> V.Vw:
  match v:
    case V.Vw{s, vi, f, r, d, lx, ly, b, fr, cu, vx}:
      V.Vw{s, vi, f, r, d, lx, ly, b, fr, cu, Vx.busy(vx, text)}

def Vw.if(ok: Bool, v: V.Vw, k: V.Vw -> V.Vw) -> V.Vw:
  match ok:
    case True{}:
      k(v)
    case False{}:
      v

def Vw.root(v: V.Vw) -> String:
  match v:
    case V.Vw{_, _, _, r, _, _, _, _, _, _, vx}:
      r

def Vw.fresh_for(+v: V.Vw, +key: String) -> Bool:
  String.eq(Vx.shown(Vw.x(v)), key)

def Vw.board.same(+v: V.Vw, +key: String, +items: List<&2, B.Item>, +now: Nat) -> V.Vw:
  Vw.board.k(String.eq(HS.Hist.key.family(Vw.root(v)), HS.Hist.key.family(key)), v, key, items, now)

# results for a source other than the one on show are stale: dropped
def Nui.board(+n: Nui, +key: String, +items: List<&2, B.Item>, +now: Nat) -> Nui:
  Nui.with_vw(n, Vw.if(Vw.fresh_for(Nui.vw(n), key), Nui.vw(n), v => Vw.board.same(v, key, items, now)))

def Nui.mesh(+n: Nui, +key: String, ts: FA.Scene3) -> Nui:
  Nui.with_vw(n, Vw.if(Vw.fresh_for(Nui.vw(n), key), Nui.vw(n), v => Vw.mesh(v, ts)))

def Nui.parts(+n: Nui, +key: String, +t0: Nat, ts: List<&2, G.Tri3>, +now: Nat) -> Nui:
  Nui.with_vw(n, Vw.if(Vw.fresh_for(Nui.vw(n), key), Nui.vw(n), v => Vw.parts(v, t0, ts, now)))

def Vw.shot(v: V.Vw, b: BM.Bitmap) -> V.Vw:
  match v:
    case V.Vw{s, vi, f, r, d, lx, ly, bb, _, cu, V.Vx{k, p, t, c, sh, _, px, py, pa, se, _}}:
      V.Vw{s, vi, f, r, d, lx, ly, bb, False{}, cu, V.Vx{k, p, t, c, sh, "", px, py, pa, se, b}}

# a new frame of the agents' browser
def Nui.shot(+n: Nui, +key: String, b: BM.Bitmap) -> Nui:
  Nui.with_vw(n, Vw.if(Vw.fresh_for(Nui.vw(n), key), Nui.vw(n), v => Vw.shot(v, b)))

def Nui.status(+n: Nui, +key: String, +text: String) -> Nui:
  Nui.with_vw(n, Vw.if(Vw.fresh_for(Nui.vw(n), key), Nui.vw(n), v => Vw.status(v, text)))

# the browser (its page's frames come as images, layout.bend's Lay.page)
# and the diff (from the client state) read no file
def Nui.source.k(x: V.Vx, +root: String, +pre: String) -> String:
  match x:
    case V.Vx{+k, _, _, _, _, _, _, _, +pa, _, bm}:
      Bool.pick(String, U32.is_ge(k, 3), V.Src.key(k, "", ""), V.Src.key(k, RV.Rview.at(pre, root), RV.Rview.at(pre, pa)))

# the source the viewer should show now (Src.key)
def Nui.source(+n: Nui) -> String:
  Nui.source.k(Vw.x(Nui.vw(n)), U.Ui.root(Nui.ui(n)), U.Ui.info(Nui.ui(n), "local.remote.pre"))

def Vw.asked(v: V.Vw, +key: String) -> V.Vw:
  match v:
    case V.Vw{s, vi, f, r, d, lx, ly, b, fr, cu, V.Vx{+k, p, t, c, _, _, px, py, pa, se, bm}}:
      V.Vw{s, vi, f, r, d, lx, ly, b, fr, cu, V.Vx{k, p, t, c, key, Bool.pick(String, U32.is_eq(k, 2), NT.Notice.make("exporting", ""), Bool.pick(String, U32.is_eq(k, 3), NT.Notice.make("browser", ""), NT.Notice.make("loading", ""))), px, py, pa, se, bm}}

# the viewer asked for a source (the shell starts reading it)
def Nui.asked(+n: Nui, +key: String) -> Nui:
  Nui.with_vw(n, Vw.asked(Nui.vw(n), key))

# fit the view to the board the first time the viewer shows it
def Vw.fit(v: V.Vw, +x0: U32, +h: U32, +w: U32) -> V.Vw:
  match v:
    case V.Vw{+s, vi, False{}, r, d, lx, ly, b, _, cu, vx}:
      V.Vw{s, V.View.fit(s, U32.to_f32(x0), 52.0, U32.to_f32((w - x0 : U32)), U32.to_f32((h - 52 : U32))), Bool.not(List.is_empty(&2, V.Seen, s)), r, d, lx, ly, b, False{}, cu, vx}
    case _:
      v

def Nui.height(n: Nui) -> U32:
  match n:
    case Nui{_, _, h, _, _, _, _, _, _, _, _, _, _}:
      h

def Term.rows(+n: Nui) -> U32:
  (((Nui.height(n) * 2 / 5 : U32) - 34 : U32) / 18 : U32)

def Nui.term.toggle(open: Bool, +n: Nui) -> Step:
  match open:
    case True{}:
      Nui.focus.step(Nui.act(n, "flag", "term"), "composer")
    case False{}:
      Nui.focus.step(Nui.act.k(U.Ui.term_open(U.Ui.flag_on(Nui.ui(n), "term"), Term.cols(n), Term.rows(n)), n), "terminal")

# the file picker asks the server for the thread's files as it opens
def Pal.files(+n: Nui) -> Step:
  Nui.focus.step(Nui.act.k(U.Ui.act(U.Palette.open(Nui.ui(n), "files"), "files", ""), n), "palette")

# ctrl+1..9: the Nth thread the sidebar shows in this project
def Nui.jump(+n: Nui, +id: String) -> Step:
  Bool.pick(Step, String.is_empty(id), Step{n, Nil{}, False{}}, Nui.act(n, "select", id))

def Nui.goto.k(none: Bool, +n: Nui, +pid: String, +id: String) -> Step:
  match none:
    case True{}:
      Nui.act(n, "new-thread", pid)
    case False{}:
      Nui.act(n, "select", id)

# alt+1..9: the Nth project the sidebar shows (a thread there, or a new
# one when it has none)
def Nui.goto(+n: Nui, +pid: String) -> Step:
  Bool.pick(Step, String.is_empty(pid), Step{n, Nil{}, False{}}, Nui.goto.k(String.is_empty(U.Side.home(Nui.ui(n), pid)), n, pid, U.Side.home(Nui.ui(n), pid)))

# the focus changed: the scene is drawn again under it
def Nui.lfocus.set(+n: Nui, +f: LY.Focus) -> Step:
  Rvw.bits(Nui.with_vw(Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), Nui.lfocus.key(n), LY.Lyr.show(f))), Vw.stale(Nui.vw(n))), f)

# the kind of the scene on show (0 a board, 1 a schematic)
def Nui.look.kind(+n: Nui) -> U32:
  Vw.kind(Nui.vw(n))

def Keys.list() -> List<&2, String>:
  ["term", "diff", "new", "prev", "next", "settle", "pin", "paste", "palette", "jump", "search", "files", "project", "perf", "layers"]

def Nui.shortcut.k(i: Nat, +n: Nui, +code: U32) -> Step:
  match i:
    case 0n:
      Nui.term.toggle(U.Ui.flag(Nui.ui(n), "term"), n)
    case 1n:
      Nui.act(n, "diff", "")
    case 2n:
      Nui.act(n, "new-thread", U.Ui.thread_project(U.Ui.thread(U.Ui.state(Nui.ui(n)), U.Ui.sel(Nui.ui(n)))))
    case 3n:
      Nui.act(n, "select", Keys.neighbour(Nui.hits(n), U.Ui.sel(Nui.ui(n)), False{}))
    case 4n:
      Nui.act(n, "select", Keys.neighbour(Nui.hits(n), U.Ui.sel(Nui.ui(n)), True{}))
    case 5n:
      Nui.act(n, "settle", "")
    case 6n:
      Nui.act(n, "pin", "")
    case 8n:
      Pal.open(n, "", "")
    case 9n:
      Nui.jump(n, U.Side.nth(U.Side.here(Nui.ui(n)), U32.to_nat((U32.or(code, 32) - 49 : U32))))
    case 10n:
      Pal.open(n, "search", "")
    case 11n:
      Pal.files(n)
    case 12n:
      Nui.goto(n, U.Side.nth(U.Side.projects(Nui.ui(n)), U32.to_nat((code - 49 : U32))))
    case 13n:
      Nui.act(n, "flag", "perf")
    # ctrl+h: KiCad's high contrast, the other layers as they are, dimmed,
    # hidden and around again (core/layers.bend's Lyr.cycle)
    case 14n:
      Nui.lfocus.set(n, LY.Lyr.cycle(Nui.look.kind(n), Nui.lfocus(n)))
    case _:
      Step{n, [NPaste{}], False{}}

def Step.caret_end(s: Step) -> Step:
  match s:
    case Step{+n, cs, d}:
      Step{Nui.with_caret(n, String.length(U.Ui.draft_text(Nui.ui(n)))), cs, d}

def Pal.act(mention: Bool, +n: Nui, +a: String, +v: String) -> Step:
  match mention:
    case True{}:
      Step.caret_end(Nui.act(n, a, v))
    case False{}:
      Nui.act(n, a, v)

# Project picker
# --------------
# Opens on the projects; the client ranks them and the folders (fs.list).
# Enter runs the selected row, Tab opens the selected folder, Backspace on
# an empty filter goes up a folder, Esc goes back from the folders to the
# projects.

def Pick.open(+n: Nui) -> Step:
  Nui.focus.step(Nui.act(n, "proj-open", ""), "proj")

def Pick.close(+n: Nui) -> Step:
  Nui.focus(Nui.with_ui(n, U.Ui.flag_off(Nui.ui(n), "proj")), "composer")

def Pick.moved(+n: Nui, +i: Nat) -> Step:
  Step{Nui.with_ui(n, U.Proj.set(Nui.ui(n), U.Proj.query(Nui.ui(n)), Nat.min(i, Nat.sub(List.length(&2, U.PItem, U.Proj.items(Nui.ui(n))), 1n)))), Nil{}, True{}}

def Pick.typed(e: Ed, +n: Nui) -> Step:
  match e:
    case Ed{+t, _, _, _, _, _, _}:
      Step{Nui.with_ui(n, U.Proj.set(Nui.ui(n), t, 0n)), Nil{}, True{}}

# a row runs its own action (a row with none does nothing): going to or
# adding a project closes the picker; browsing and creating keep it open
def Pick.run.k(known: Bool, +n: Nui, +a: String, +v: String) -> Step:
  match known:
    case True{}:
      Nui.act(n, a, v)
    case False{}:
      Step{n, Nil{}, False{}}

def Pick.run(it: U.PItem, +n: Nui) -> Step:
  match it:
    case U.PItem{_, +a, +v}:
      Pick.run.k(Nat.is_lt(Nui.pos(U.Proj.acts(), a, 0n), List.length(&2, String, U.Proj.acts())), n, a, v)

def Pick.at(+n: Nui) -> U.PItem:
  U.Palette.at(U.Proj.items(Nui.ui(n)), U.Proj.index(Nui.ui(n)))

# Tab opens the selected row when it is a folder
def Pick.tab.k(it: U.PItem, +n: Nui) -> Step:
  match it:
    case U.PItem{_, +a, +v}:
      Bool.pick(Step, String.eq(a, "proj-cd"), Nui.act(n, "proj-cd", v), Step{n, Nil{}, False{}})

# Esc leaves the folders for the projects, and closes the projects
def Pick.esc(dir: Bool, +n: Nui) -> Step:
  match dir:
    case True{}:
      Nui.act(n, "proj-back", "")
    case False{}:
      Pick.close(n)

def Pick.key.k(esc: Bool, enter: Bool, up: Bool, down: Bool, tab: Bool, up_dir: Bool, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  match esc enter up down tab up_dir:
    case True{} _ _ _ _ _:
      Pick.esc(U.Proj.browsing(Nui.ui(n)), n)
    case _ True{} _ _ _ _:
      Pick.run(Pick.at(n), n)
    case _ _ True{} _ _ _:
      Pick.moved(n, Nat.sub(U.Proj.index(Nui.ui(n)), 1n))
    case _ _ _ True{} _ _:
      Pick.moved(n, 1n+U.Proj.index(Nui.ui(n)))
    case _ _ _ _ True{} _:
      Pick.tab.k(Pick.at(n), n)
    case _ _ _ _ _ True{}:
      Nui.act(n, "proj-cd", U.Proj.parent(U.Proj.dir(Nui.ui(n))))
    case _ _ _ _ _ _:
      Pick.typed(Ed.key(code, text, mods, U.Proj.query(Nui.ui(n)), String.length(U.Proj.query(Nui.ui(n))), String.length(U.Proj.query(Nui.ui(n)))), n)

def Pick.key(+n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  Pick.key.k(U32.is_eq(code, Key.escape()), Bool.or(U32.is_eq(code, Key.enter()), U32.is_eq(code, Key.kp_enter())), U32.is_eq(code, 65362), U32.is_eq(code, 65364),
    U32.is_eq(code, Key.tab()), Bool.and(Bool.and(U32.is_eq(code, Key.backspace()), String.is_empty(U.Proj.query(Nui.ui(n)))), U.Proj.browsing(Nui.ui(n))), n, code, text, mods)

def Pick.click(+n: Nui, +v: String) -> Step:
  Pick.run(U.Palette.at(U.Proj.items(Nui.ui(n)), Maybe.default(&2, Nat, Nat.read(v), 0n)), n)

# A press outside the model picker, the machine list or a project's badge
# editor closes it (laws menu_press_*). What is theirs: their rows, chips,
# swatches and boxes (the picker's hit "menu", its value "first
# shown|last first"), and the toggles that open them.
def Menu.own(+a: String, +v: String) -> Bool:
  Bool.or(Bool.or(Bool.or(Bool.or(String.eq(a, "model"), String.eq(a, "effort")), Bool.or(String.eq(a, "machine"), String.eq(a, "menu"))),
    Bool.and(String.eq(a, "flag"), Bool.or(String.eq(v, "models"), String.eq(v, "machines")))),
    Bool.or(Bool.or(Bool.or(String.eq(a, "look"), String.eq(a, "look-set")), String.eq(a, "look-bg")),
      Bool.or(String.eq(a, "row-delete"), String.eq(a, "proj-remove"))))

def Menu.inside(m: Maybe<&2, Hit>) -> Bool:
  match m:
    case Some{Hit{_, _, _, _, +a, +v}}:
      Menu.own(a, v)
    case None{}:
      False{}

def Menu.shut(inside: Bool, +n: Nui) -> Nui:
  match inside:
    case True{}:
      n
    case False{}:
      Nui.with_ui(n, U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.flag_off(U.Ui.flag_off(Nui.ui(n), "models"), "machines"), "@look", ""), "@menu", ""))

# the picker keeps where it was scrolled only while presses stay in it
def Menu.keeps_off(m: Maybe<&2, Hit>) -> Bool:
  match m:
    case Some{Hit{_, _, _, _, +a, _}}:
      Bool.or(String.eq(a, "menu"), String.eq(a, "effort"))
    case None{}:
      False{}

def Menu.off_reset(keep: Bool, +n: Nui) -> Nui:
  match keep:
    case True{}:
      n
    case False{}:
      Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), "@menu.off", ""))

# what a press on m does to the menus before anything else
def Menu.press(+m: Maybe<&2, Hit>, +n: Nui) -> Nui:
  Menu.off_reset(Menu.keeps_off(m), Menu.shut(Menu.inside(m), n))

# the value of the hit named `action` under (x, y), whatever lies on top
# of it ("" for none)
def Hit.value_at(hs: List<&2, Hit>, +action: String, +x: U32, +y: U32) -> String:
  match hs:
    case Nil{}:
      ""
    case Hit{+x0, +y0, +x1, +y1, +a, +v} <> t:
      Bool.pick(String, Bool.and(String.eq(a, action), Bool.and(Bool.and(U32.is_ge(x, x0), U32.is_lt(x, x1)), Bool.and(U32.is_ge(y, y0), U32.is_lt(y, y1)))), v, Hit.value_at(t, action, x, y))

# the picker's box under (x, y), as its hit's value ("" for none)
def Menu.box(+hs: List<&2, Hit>, +x: U32, +y: U32) -> String:
  Hit.value_at(hs, "menu", x, y)

# a wheel notch over the picker: one model up (dir 0) or down, within
# 0..last first (the layout clamps it again: law menu_scroll_in_list)
def Menu.scroll(+v: String, +dir: U32) -> String:
  +off = Maybe.default(&2, Nat, Nat.read(Maybe.default(&2, String, List.head(&2, String, String.split(v, '|')), "")), 0n)
  +last = Maybe.default(&2, Nat, Nat.read(Maybe.default(&2, String, List.last(&2, String, String.split(v, '|')), "")), 0n)
  Nat.show(Bool.pick(Nat, U32.is_eq(dir, 0), Nat.sub(off, 1n), Bool.pick(Nat, Nat.is_lt(off, last), 1n+off, last)))

def Nui.prepare.fit(open: Bool, +n: Nui) -> Nui:
  match open:
    case True{}:
      Nui.with_vw(n, Vw.fit(Nui.vw(n), Nui.vx0(n), (Nui.height(n) - U.HBar.h(U.Hs.bar(Nui.ui(n))) : U32), Nui.width(n)))
    case False{}:
      n

# fit a board, schematic or model when first shown (the diff view scrolls
# from the top instead)
def Nui.prepare.k(open: Bool, +n: Nui) -> Nui:
  Nui.prepare.fit(Bool.and(open, Bool.not(U32.is_eq(Vw.kind(Nui.vw(n)), 4))), n)

# a kind of view by name (View.kind_of reads it back)
def View.name(+k: U32) -> String:
  Bool.pick(String, U32.is_eq(k, 1), "schematic", Bool.pick(String, U32.is_eq(k, 2), "3d", Bool.pick(String, U32.is_eq(k, 3), "browser", Bool.pick(String, U32.is_eq(k, 4), "diff", Bool.pick(String, U32.is_eq(k, 5), "mech",
    Bool.pick(String, U32.is_eq(k, 6), "files", Bool.pick(String, U32.is_eq(k, 7), "image", Bool.pick(String, U32.is_eq(k, 8), "pdf", Bool.pick(String, U32.is_eq(k, 9), "review", "board")))))))))

# the kind of view for a requested file: named, else from its extension
def View.kind_of(+kind: String, +path: String) -> U32:
  Bool.pick(U32, String.eq(kind, "review"), 9,
  Bool.pick(U32, String.eq(kind, "files"), 6,
  Bool.pick(U32, String.eq(kind, "image"), 7,
  Bool.pick(U32, String.eq(kind, "pdf"), 8,
  Bool.pick(U32, String.eq(kind, "mech"), 5,
  Bool.pick(U32, String.eq(kind, "diff"), 4,
  Bool.pick(U32, String.eq(kind, "browser"), 3,
  Bool.pick(U32, String.eq(kind, "schematic"), 1,
    Bool.pick(U32, Bool.or(String.eq(kind, "3d"), String.eq(kind, "step")), 2,
      Bool.pick(U32, String.eq(kind, "board"), 0,
        Bool.pick(U32, String.ends_with(path, ".kicad_sch"), 1,
          Bool.pick(U32, Bool.or(String.ends_with(path, ".glb"), Bool.or(String.ends_with(path, ".step"), String.ends_with(path, ".stp"))), 2, 0))))))))))))

def Vx.seen(x: V.Vx) -> String:
  match x:
    case V.Vx{_, _, _, _, _, _, _, _, _, se, bm}:
      se

def Vw.seen_set(v: V.Vw, +seen: String) -> V.Vw:
  match v:
    case V.Vw{s, vi, f, r, d, lx, ly, b, fr, cu, V.Vx{k, p, t, c, sh, bu, px, py, pa, _, bm}}:
      V.Vw{s, vi, f, r, d, lx, ly, b, fr, cu, V.Vx{k, p, t, c, sh, bu, px, py, pa, seen, bm}}

def Nui.request.k(fresh: Bool, +n: Nui, +seen: String) -> Nui:
  match fresh:
    case True{}:
      Nui.with_vw(n, Vw.seen_set(Vw.kind_set(Nui.vw(n), View.kind_of(U.Ui.view_get(Nui.ui(n), "view.kind"), U.Ui.view_get(Nui.ui(n), "view.path")), U.Ui.view_get(Nui.ui(n), "view.path")), seen))
    case False{}:
      n

# which thread's view, and which change of it, the viewer shows
def Nui.view_at(+n: Nui) -> String:
  U.Ui.sel(Nui.ui(n)) ++ "#" ++ U.Ui.view_get(Nui.ui(n), "view.n")

# the viewer shows the selected thread's view: on a change of thread, or
# a viewer_show or kind picked for it, switch to that
def Nui.request(+n: Nui) -> Nui:
  Nui.request.k(Bool.not(String.eq(Nui.view_at(n), Vx.seen(Vw.x(Nui.vw(n))))), n, Nui.view_at(n))

# Each thread keeps its own viewer (camera, scene, what is picked, the
# layers shown): the window shows the selected thread's, and the others
# wait here by thread until theirs is selected again (main.bend's
# Shell.park swaps them)
type Park is Data:
  Park{thread: String, views: Map<&2, V.Vw>}

def Park.init() -> Park:
  Park{"", Map.new(&2, V.Vw)}

def Park.thread(p: Park) -> String:
  match p:
    case Park{t, vs}:
      t

# a viewer back on screen asks for its source again (the file may have
# changed while it was away; the scene it kept stays until the new one
# comes, and what did not change does not fade in again) and redraws
def Vw.back(v: V.Vw) -> V.Vw:
  match v:
    case V.Vw{s, vi, f, r, _, lx, ly, b, _, cu, V.Vx{k, p, t, c, _, bu, px, py, pa, se, bm}}:
      V.Vw{s, vi, f, r, False{}, lx, ly, b, False{}, cu, V.Vx{k, p, t, c, "", bu, px, py, pa, se, bm}}

def Park.got(r: Map<&2, V.Vw> & V.Vw) -> V.Vw:
  (_, v) = r
  Vw.back(v)

def Park.take(+vs: Map<&2, V.Vw>, +id: String) -> V.Vw:
  Park.got(Map.get(V.Vw, V.Vw.init(), vs, id))

def Park.swap.k(p: Park, +n: Nui, +sel: String) -> Nui & Park:
  match p:
    case Park{+old, +vs}:
      (Nui.with_vw(n, Park.take(vs, sel)), Park{sel, Map.set(&2, V.Vw, vs, old, Nui.vw(n))})

def Nui.prepare(+n: Nui) -> Nui:
  Nui.prepare.k(Nui.panel(n), Nui.request(n))

# pointer events on the viewer: drag pans (or orbits), the wheel zooms at
# the pointer, and a press released where it began picks an item; a 3D
# view drawn in draft while dragging is drawn again in full on release
def Vw.press(v: V.Vw, +x: U32, +y: U32) -> V.Vw:
  match v:
    case V.Vw{s, vi, f, r, _, _, _, b, fr, cu, V.Vx{k, p, t, c, sh, bu, _, _, pa, se, bm}}:
      V.Vw{s, vi, f, r, True{}, x, y, b, fr, cu, V.Vx{k, p, t, c, sh, bu, x, y, pa, se, bm}}

def Vw.release.k(click: Bool, v: V.Vw, +x: U32, +y: U32, +flip: U32, +fk: U32) -> V.Vw:
  match click v:
    case True{} V.Vw{+s, +vi, f, r, _, lx, ly, b, fr, cu, V.Vx{+k, _, t, c, sh, bu, px, py, pa, se, bm}}:
      V.Vw{s, vi, f, r, False{}, lx, ly, b, fr, cu, V.Vx{k, Bool.pick(String, U32.is_eq(k, 2), "", V.Pick.at(s, vi, U32.to_f32(x), U32.to_f32(y), U32.or(k, fk), flip)), t, c, sh, bu, px, py, pa, se, bm}}
    case False{} V.Vw{s, vi, f, r, _, lx, ly, b, fr, cu, +vx}:
      V.Vw{s, vi, f, r, False{}, lx, ly, b, Bool.and(fr, Bool.not(U32.is_eq(Vx.kind(vx), 2))), cu, vx}

def Vx.press_at(x: V.Vx) -> U32 & U32:
  match x:
    case V.Vx{_, _, _, _, _, _, px, py, _, _, bm}:
      (px, py)

def Vw.near(p: U32 & U32, +x: U32, +y: U32) -> Bool:
  (+px, +py) = p
  U32.is_lt((U32.max(x, px) - U32.min(x, px) + U32.max(y, py) - U32.min(y, py) : U32), 4)

def Vw.dragging(v: V.Vw) -> Bool:
  match v:
    case V.Vw{_, _, _, _, d, _, _, _, _, _, _}:
      d

def Vw.release(+v: V.Vw, +x: U32, +y: U32, +flip: U32, +fk: U32) -> V.Vw:
  Vw.release.k(Bool.and(Vw.dragging(v), Vw.near(Vx.press_at(Vw.x(v)), x, y)), v, x, y, flip, fk)

def Nui.view_press(+n: Nui, +x: U32, +y: U32, down: Bool) -> Step:
  match down:
    case True{}:
      Step{Nui.with_vw(n, Vw.press(Nui.vw(n), x, y)), Nil{}, False{}}
    case False{}:
      Step{Nui.with_vw(n, Vw.release(Nui.vw(n), x, y, Nui.flip(n), Nui.lfocus.bits(n))), Nil{}, Vw.dragging(Nui.vw(n))}

def Nui.dragging(n: Nui) -> Bool:
  match n:
    case Nui{_, _, _, _, _, _, _, _, _, _, _, _, V.Vw{_, _, _, _, d, _, _, _, _, _, vx}}:
      d

# the viewer's panel
def Nui.pnl(+n: Nui) -> Pnl:
  Pnl{Nui.vx0(n), (Nui.width(n) - Nui.vx0(n) : U32), (Nui.height(n) - 52 : U32)}

def Nui.view_move(+n: Nui, +x: U32, +y: U32, +mods: U32) -> Step:
  Step{Nui.with_vw(n, Vw.moved(Nui.vw(n), x, y, mods, Nui.pnl(n))), Nil{}, Nui.dragging(n)}

def Nui.view_wheel(+n: Nui, +x: U32, +y: U32, +dir: U32) -> Step:
  Step{Nui.with_vw(n, Vw.wheel(Nui.vw(n), dir, x, y, Nui.pnl(n))), Nil{}, True{}}

def Nui.nohit(m: Maybe<&2, Hit>) -> Bool:
  match m:
    case Some{_}:
      False{}
    case None{}:
      True{}

# Dragging a sidebar row
# ----------------------
# A press on a row selects it and arms a drag (the thread and where the
# press was, in the client's scratch state); a release far enough away
# over another row of the same shelf reorders (client action "reorder").
# A press on a project's name arms a drag of the project ("project:<id>");
# a release over another project's name or rows moves it there (client
# action "reorder-project").

def Row.arm(s: Step, +id: String, +y: U32) -> Step:
  match s:
    case Step{+n, cs, d}:
      Step{Nui.with_ui(n, U.Ui.scratch_set(U.Ui.scratch_set(Nui.ui(n), "@drag", id), "@drag.y", U32.show(y))), cs, d}

def Row.press.k(row: Bool, s: Step, +id: String, +y: U32) -> Step:
  match row:
    case True{}:
      Row.arm(s, id, y)
    case False{}:
      s

def Row.id(+n: Nui) -> String:
  U.Ui.scratch(Nui.ui(n), "@drag")

def Row.y(+n: Nui) -> U32:
  Maybe.default(&2, U32, U32.read(U.Ui.scratch(Nui.ui(n), "@drag.y")), 0)

# whether the pointer went far enough from the press to be a drag
def Row.far(+y0: U32, +y: U32) -> Bool:
  U32.is_ge((U32.max(y, y0) - U32.min(y, y0) : U32), 6)

def Row.drop.k(project: Bool, +n: Nui, +id: String, +a: String, +v: String) -> Step:
  match project:
    case True{}:
      Bool.pick(Step, Bool.or(String.eq(a, "select"), String.eq(a, "project")), Nui.act(n, "reorder-project", String.drop(id, 8n) ++ "|" ++ v), Step{n, Nil{}, True{}})
    case False{}:
      Bool.pick(Step, String.eq(a, "select"), Nui.act(n, "reorder", id ++ "|" ++ v), Step{n, Nil{}, True{}})

def Row.drop(m: Maybe<&2, Hit>, +n: Nui, +id: String) -> Step:
  match m:
    case Some{Hit{_, _, _, _, +a, +v}}:
      Row.drop.k(U.Drag.project(id), n, id, a, v)
    case None{}:
      Step{n, Nil{}, True{}}

def Row.release.k(far: Bool, +n: Nui, +id: String, +y: U32) -> Step:
  match far:
    case True{}:
      Row.drop(Hit.find(Nui.hits(n), 10, y), n, id)
    case False{}:
      Step{n, Nil{}, True{}}

def Row.clear(+n: Nui) -> Nui:
  Nui.with_ui(n, U.Ui.scratch_set(U.Ui.scratch_set(Nui.ui(n), "@drag", ""), "@drag.y", ""))

def Row.release(on: Bool, +n: Nui, +x: U32, +y: U32) -> Step:
  match on:
    case True{}:
      Row.release.k(Row.far(Row.y(n), y), Row.clear(n), Row.id(n), y)
    case False{}:
      Nui.view_press(n, x, y, False{})

def Row.move(on: Bool, +n: Nui, +x: U32, +y: U32) -> Step:
  match on:
    case True{}:
      Step{Step.nui(Nui.moved(n, x, y)), Nil{}, True{}}
    case False{}:
      Nui.moved(n, x, y)

def Nui.move.k(drag: Bool, +n: Nui, +x: U32, +y: U32, +mods: U32) -> Step:
  match drag:
    case True{}:
      Nui.view_move(n, x, y, mods)
    case False{}:
      Row.move(Bool.not(String.is_empty(Row.id(n))), n, x, y)

# The composer grows with its draft from 2 to Cs.most() lines; a longer
# draft scrolls. Its first shown line follows the caret unless the wheel
# chose one: "@cs" holds that choice while the field, thread and caret it
# was made with do (as "@mark" does). Layout names the composer's current
# top and furthest top in a zero-size "cs" region ("top,max").

def Cs.most() -> Nat:
  8n

# how many lines the composer shows for a draft of n wrapped lines
def Cs.rows(+n: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_le(n, 2n), 2n, Bool.pick(Nat, Nat.is_ge(n, Cs.most()), Cs.most(), n))

# the first shown line that keeps the caret's line (crow) in view
def Cs.follow(+n: Nat, +crow: Nat) -> Nat:
  +m = Nat.sub(n, Cs.rows(n))
  +c = Nat.sub(crow, Nat.sub(Cs.rows(n), 1n))
  Bool.pick(Nat, Nat.is_le(m, c), m, c)

# the first shown line: the wheel's choice (held) while it holds, else
# the caret's; never past the last full view
def Cs.top(+held: String, +n: Nat, +crow: Nat) -> Nat:
  +m = Nat.sub(n, Cs.rows(n))
  +t = Maybe.default(&2, Nat, Nat.read(held), Cs.follow(n, crow))
  Bool.pick(Nat, Nat.is_le(t, m), t, m)

# the first shown line after a wheel notch, 3 lines, within 0..max
def Cs.wheel(up: Bool, +top: Nat, +max: Nat) -> Nat:
  match up:
    case True{}:
      Nat.sub(top, 3n)
    case False{}:
      Bool.pick(Nat, Nat.is_le(Nat.add(top, 3n), max), Nat.add(top, 3n), max)

# the wheel's choice while it still holds ("" when it does not)
def Cs.held(+n: Nui) -> String:
  +r = U.Ui.scratch(Nui.ui(n), "@cs")
  Bool.pick(String, String.starts_with(r, Mark.sig(n)), String.drop(r, String.length(Mark.sig(n))), "")

def Cs.region(hs: List<&2, Hit>) -> String:
  match hs:
    case Nil{}:
      ""
    case Hit{_, _, _, _, +a, +v} <> t:
      Bool.pick(String, String.eq(a, "cs"), v, Cs.region(t))

def Cs.num(+v: String, +i: Nat) -> Nat:
  Maybe.default(&2, Nat, Nat.read(Maybe.default(&2, String, List.get(&2, String, String.split(v, ','), i), "")), 0n)

def Cs.over(m: Maybe<&2, Hit>) -> Bool:
  match m:
    case Some{Hit{_, _, _, _, +a, +v}}:
      Bool.and(String.eq(a, "focus"), String.eq(v, "composer"))
    case None{}:
      False{}

def Cs.spin.k(on: Bool, +n: Nui, +x: U32, +dir: U32, +v: String) -> Step:
  match on:
    case True{}:
      Step{Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), "@cs", Mark.sig(n) ++ Nat.show(Cs.wheel(U32.is_eq(dir, 0), Cs.num(v, 0n), Cs.num(v, 1n))))), Nil{}, True{}}
    case False{}:
      Nui.wheel(n, x, dir)

# the wheel over a composer too short for its draft scrolls the draft;
# anywhere else it moves the sidebar or the timeline
def Cs.spin(+n: Nui, +x: U32, +y: U32, +dir: U32) -> Step:
  +v = Cs.region(Nui.hits(n))
  Cs.spin.k(Bool.and(Cs.over(Hit.find(Nui.hits(n), x, y)), Nat.is_gt(Cs.num(v, 1n), 0n)), n, x, dir, v)

def Nui.spin(viewer: Bool, +n: Nui, +x: U32, +y: U32, +dir: U32) -> Step:
  match viewer:
    case True{}:
      Nui.view_wheel(n, x, y, dir)
    case False{}:
      Cs.spin(n, x, y, dir)

# the wheel over the settings screen scrolls it by step px a notch (48;
# a room's posts one tile, Nui.notch), within 0..the most its hit "set-bg"
# says
def Set.scroll.by(+now: String, +last: String, +dir: U32, +step: U32) -> String:
  +off = Maybe.default(&2, U32, U32.read(now), 0)
  +most = Maybe.default(&2, U32, U32.read(last), 0)
  U32.show(U32.min(Bool.pick(U32, U32.is_eq(dir, 0), Bool.pick(U32, U32.is_ge(off, step), (off - step : U32), 0), (off + step : U32)), most))

def Set.scroll(+now: String, +last: String, +dir: U32) -> String:
  Set.scroll.by(now, last, dir, 48)

# the wheel over a room's posts: up goes back ("@chat.sc", from the
# bottom, within 0..the most its hit "chat-bg" says), a tile a notch, so
# the frame after it reuses the tiles it slid (the posts' "chat-line"
# hits say how far)
def Chat.scroll(+n: Nui, +v: String, +dir: U32) -> Nui:
  Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), "@chat.sc", Set.scroll.by(U.Ui.scratch(Nui.ui(n), "@chat.sc"), v, Bool.pick(U32, U32.is_eq(dir, 0), 1, 0), Nui.notch())))

def Chat.spin(none: Bool, +v: String, +n: Nui, +x: U32, +y: U32, +dir: U32) -> Step:
  match none:
    case True{}:
      Nui.spin(Nui.in_viewer(n, x), n, x, y, dir)
    case False{}:
      Step{Chat.scroll(n, v, dir), Nil{}, True{}}

def Set.spin(none: Bool, +v: String, +n: Nui, +x: U32, +y: U32, +dir: U32) -> Step:
  match none:
    case True{}:
      Chat.spin(String.is_empty(Hit.value_at(Nui.hits(n), "chat-bg", x, y)), Hit.value_at(Nui.hits(n), "chat-bg", x, y), n, x, y, dir)
    case False{}:
      Step{Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), "@set.sc", Set.scroll(U.Ui.scratch(Nui.ui(n), "@set.sc"), v, dir))), Nil{}, True{}}

# A pane that scrolls by rows (the subagents panel, the todo card): its hit
# "pane" carries "<key>|<last>", and a wheel notch over it moves the first
# row shown by one, within 0..last, kept in the info as "pane:<key>" (the
# layout reads it as Pane.off). Elsewhere the wheel goes on as before.
def Pane.at(+v: String, +i: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, String.split(v, '|'), i), "")

def Pane.to(+cur: Nat, +last: Nat, +dir: U32) -> Nat:
  Bool.pick(Nat, U32.is_eq(dir, 0), Nat.sub(cur, 1n), Bool.pick(Nat, Nat.is_lt(cur, last), 1n+cur, last))

# the Review page's lines go by three
def Rpw.to(rp: Bool, +cur: Nat, +last: Nat, +dir: U32) -> Nat:
  match rp:
    case True{}:
      Bool.pick(Nat, U32.is_eq(dir, 0), Nat.sub(cur, 3n), Bool.pick(Nat, Nat.is_lt(Nat.add(cur, 3n), last), Nat.add(cur, 3n), last))
    case False{}:
      Pane.to(cur, last, dir)

def Pane.step(+n: Nui, +key: String, +last: Nat, +dir: U32) -> Step:
  Step{Nui.with_ui(n, U.Ui.info_set(Nui.ui(n), "pane:" ++ key,
    Nat.show(Rpw.to(String.starts_with(key, "rp:"), Maybe.default(&2, Nat, Nat.read(U.Ui.info(Nui.ui(n), "pane:" ++ key)), 0n), last, dir)))), Nil{}, True{}}

# the wheel over the open picker scrolls its models, elsewhere as before
def Menu.spin(none: Bool, +v: String, +n: Nui, +x: U32, +y: U32, +dir: U32) -> Step:
  match none:
    case True{}:
      Set.spin(String.is_empty(Hit.value_at(Nui.hits(n), "set-bg", x, y)), Hit.value_at(Nui.hits(n), "set-bg", x, y), n, x, y, dir)
    case False{}:
      Step{Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), "@menu.off", Menu.scroll(v, dir))), Nil{}, True{}}

def Pane.spin(none: Bool, +v: String, +n: Nui, +x: U32, +y: U32, +dir: U32) -> Step:
  match none:
    case True{}:
      Menu.spin(String.is_empty(Menu.box(Nui.hits(n), x, y)), Menu.box(Nui.hits(n), x, y), n, x, y, dir)
    case False{}:
      Pane.step(n, Pane.at(v, 0n), Maybe.default(&2, Nat, Nat.read(Pane.at(v, 1n)), 0n), dir)

def Nui.ms_of(n: Nui) -> Nat:
  match n:
    case Nui{_, _, _, _, _, _, _, _, _, _, _, ms, _}:
      ms

# whether a path field has the keys (a project to add, a file to attach)
def Nui.in_path(+n: Nui) -> Bool:
  Bool.or(String.eq(Nui.focus_of(n), "project-path"), String.eq(Nui.focus_of(n), "attach-path"))

# attach each pasted file
def Nui.attach.all(ps: List<&2, String>, +n: Nui) -> Step:
  match ps:
    case Nil{}:
      Step{n, Nil{}, True{}}
    case +p <> t:
      +s = Nui.act(n, "attach-file", p)
      Step.merge(s, Nui.attach.all(t, Step.nui(s)))

# files dragged over the window: the composer shows where they drop (a
# frame only when that changes)
def Nui.drag_over(+n: Nui) -> Step:
  Step{Nui.with_ui(n, U.Ui.drop_hover(Nui.ui(n), "1")), Nil{}, Bool.not(U.Ui.flag(Nui.ui(n), "drop"))}

def Nui.drag_leave(+n: Nui) -> Step:
  Step{Nui.with_ui(n, U.Ui.drop_hover(Nui.ui(n), "")), Nil{}, True{}}

# files dropped on the window (a text/uri-list): each attaches, the way
# a pasted file does
def Nui.drop(+n: Nui, text: String) -> Step:
  Nui.attach.all(AT.Att.uri_list(text), Nui.with_ui(n, U.Ui.drop_hover(Nui.ui(n), "")))

# a paste into the composer: files (file:// URIs, image paths) attach,
# anything else is text at the caret
def Nui.paste.comp(ps: List<&2, String>, n: Nui, +m: Nat, +text: String) -> Step:
  match ps n:
    case Nil{} Nui{+u, w, h, mx, my, sd, sc, +ca, fo, pa, hi, ms, vw}:
      Nui.composer.k(Ed.insert(U.Ui.draft_text(u), ca, m, text), Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}, False{})
    case +p <> +t _:
      Nui.attach.all(p <> t, n)

# Enter in a path field: add the project, or attach the file (and go back
# to the composer)
def Nui.path.done(attach: Bool, +n: Nui, +t: String) -> Step:
  match attach:
    case True{}:
      Nui.focus.step(Nui.act(Nui.with_path(n, "", 0n), "attach-file", t), "composer")
    case False{}:
      Nui.act(Nui.with_path(n, "", 0n), "add-project", t)

def Nui.path.k(e: Ed, +n: Nui) -> Step:
  match e:
    case Ed{+t, _, _, True{}, _, _, _}:
      Nui.path.done(String.eq(Nui.focus_of(n), "attach-path"), n, t)
    case Ed{t, +c, +m, False{}, _, +cl, _}:
      Mark.set(Clip.also(Step{Nui.with_path(n, t, c), Nil{}, True{}}, cl), m)

def Fld.on(+fo: String) -> Bool:
  String.starts_with(fo, "f:")

def Fld.part(+fo: String, +i: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, String.split(String.drop(fo, 2n), '\u{1e}'), i), "")

def Fld.key.of(+fo: String) -> String:
  "@f." ++ Fld.part(fo, 0n)

def Fld.text(+n: Nui) -> String:
  U.Ui.scratch(Nui.ui(n), Fld.key.of(Nui.focus_of(n)))

def Fld.set(+n: Nui, +t: String) -> Nui:
  Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), Fld.key.of(Nui.focus_of(n)), t))

# the caret at the end of the focused field's text
def Fld.end(s: Step) -> Step:
  match s:
    case Step{+n, cs, d}:
      Step{Nui.with_caret(n, String.length(Fld.text(n))), cs, d}

# a field whose action takes the text (a space's input) is cleared once sent
def Fld.clear(+pre: String, s: Step) -> Step:
  match s:
    case Step{+n, cs, d}:
      Step{Bool.pick(Nui, Bool.and(Fld.on(Nui.focus_of(n)), Bool.not(String.is_empty(pre))), Fld.set(n, ""), n), cs, d}

def Fld.submit.k(none: Bool, +n: Nui, +action: String, +pre: String) -> Step:
  match none:
    case True{}:
      Step{n, Nil{}, False{}}
    case False{}:
      Fld.end(Fld.clear(pre, Nui.act(n, action, pre ++ Fld.text(n))))

# Enter: the field's action
def Fld.submit(+n: Nui) -> Step:
  Fld.submit.k(String.is_empty(Fld.part(Nui.focus_of(n), 1n)), n, Fld.part(Nui.focus_of(n), 1n), Fld.part(Nui.focus_of(n), 2n))

def Fld.k(e: Ed, +n: Nui) -> Step:
  match e:
    case Ed{_, _, _, True{}, _, _, _}:
      Fld.submit(n)
    case Ed{+t, +c, +m, False{}, _, +cl, _}:
      Mark.set(Clip.also(Step{Nui.with_caret(Fld.set(n, t), c), Nil{}, True{}}, cl), m)

def Fld.key(on: Bool, +n: Nui, +m: Nat, +code: U32, +text: U32, +mods: U32) -> Step:
  match on:
    case True{}:
      Fld.k(Ed.key(code, text, mods, Fld.text(n), Nui.caret_of(n), m), n)
    case False{}:
      Step{n, Nil{}, False{}}

def Fld.paste(on: Bool, +n: Nui, +m: Nat, +text: String) -> Step:
  match on:
    case True{}:
      Fld.k(Ed.insert(Fld.text(n), Nui.caret_of(n), m, text), n)
    case False{}:
      Step{n, Nil{}, False{}}

# Up and Down in the composer, and a press or drag in it, ask layout to
# place the caret by the lines it shows ("@cq", layout.bend's Comp.pend)
def Cq.ask(+n: Nui, +q: String) -> Step:
  Step{Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), "@cq", q)), Nil{}, True{}}

def Cq.vert(+code: U32, +mods: U32) -> String:
  Bool.pick(String, U32.is_eq(code, Key.up()), "u", "d") ++ Bool.pick(String, U32.is_ne(U32.and(mods, 1), 0), "1", "0")

def Cq.is_vert(+code: U32, +mods: U32) -> Bool:
  Bool.and(Bool.or(U32.is_eq(code, Key.up()), U32.is_eq(code, Key.down())), U32.is_eq(U32.and(mods, 6), 0))

# Up/Down move the caret by lines only while the composer offers no menu;
# with one, they move its highlight (Nui.composer.ar)
def Cq.menu_free(vert: Bool, +n: Nui) -> Bool:
  Bool.and(vert, Bool.not(Nui.skills_on(Nui.ui(n))))

def Cq.at(+k: String, +x: U32, +y: U32) -> String:
  k ++ "," ++ U32.show(x) ++ "," ++ U32.show(y)

# a press in the composer focuses it, puts the caret there and starts a
# drag ("@cdrag") that selects until the button comes up
def Cq.press(+n: Nui, +x: U32, +y: U32) -> Step:
  Cq.ask(Nui.with_ui(Nui.with_focus(n, "composer"), U.Ui.scratch_set(Nui.ui(n), "@cdrag", "1")), Cq.at("p", x, y))

def Cq.dragging(+n: Nui) -> Bool:
  String.eq(U.Ui.scratch(Nui.ui(n), "@cdrag"), "1")

def Nui.composer.key(vert: Bool, n: Nui, +m: Nat, +code: U32, +text: U32, +mods: U32) -> Step:
  match vert n:
    case True{} +n2:
      Cq.ask(n2, Cq.vert(code, mods))
    case False{} Nui{+u, w, h, mx, my, sd, sc, +ca, fo, pa, hi, ms, vw}:
      Nui.composer.s(Nui.skills_on(u), code, Ed.key(code, text, mods, U.Ui.draft_text(u), ca, m), Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}, U32.is_ne(U32.and(mods, 2), 0))

def Nui.field.k(comp: Bool, path: Bool, n: Nui, +m: Nat, +code: U32, +text: U32, +mods: U32) -> Step:
  match comp path n:
    case True{} _ +n2:
      Nui.composer.key(Cq.menu_free(Cq.is_vert(code, mods), n2), n2, m, code, text, mods)
    case False{} True{} Nui{u, w, h, mx, my, sd, sc, +ca, fo, +pa, hi, ms, vw}:
      Nui.path.k(Ed.key(code, text, mods, pa, ca, m), Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw})
    case _ _ +n2:
      Fld.key(Fld.on(Nui.focus_of(n2)), n2, m, code, text, mods)

def Nui.paste.k(comp: Bool, path: Bool, n: Nui, +m: Nat, +text: String) -> Step:
  match comp path n:
    case True{} _ _:
      Nui.paste.comp(AT.Att.pasted(text), n, m, text)
    case False{} True{} Nui{u, w, h, mx, my, sd, sc, +ca, fo, +pa, hi, ms, vw}:
      Nui.path.k(Ed.insert(pa, ca, m, text), Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw})
    case _ _ +n2:
      Fld.paste(Fld.on(Nui.focus_of(n2)), n2, m, text)

# pasted text lands in the focused field at the caret
def Nui.paste.t(term: Bool, +n: Nui, text: String) -> Step:
  match term:
    case True{}:
      Nui.act.k(U.Ui.term_keys(Nui.ui(n), VT.Vt.paste(U.Ui.term(Nui.ui(n), U.Ui.sel(Nui.ui(n))), text)), n)
    case False{}:
      Nui.paste.k(String.eq(Nui.focus_of(n), "composer"), Nui.in_path(n), n, Nui.mark(n), text)

# a terminal in the chat with the focus takes it as its program would
def Nui.paste.e(none: Bool, +key: String, +n: Nui, text: String) -> Step:
  match none:
    case True{}:
      Nui.paste.t(String.eq(Nui.focus_of(n), "terminal"), n, text)
    case False{}:
      Nui.act.k(U.Ui.emb_keys(Nui.ui(n), key, VT.Vt.paste(U.Ui.term(Nui.ui(n), key), text)), n)

def Nui.paste(+n: Nui, text: String) -> Step:
  Nui.paste.e(String.is_empty(EM.Emb.focused(Nui.focus_of(n))), EM.Emb.focused(Nui.focus_of(n)), n, text)

def Nui.key.view3.k(none: Bool, +n: Nui, +a: U32) -> Step:
  match none:
    case True{}:
      Step{n, Nil{}, False{}}
    case False{}:
      Step{Nui.with_vw(n, Vw.key(Nui.vw(n), a, Nui.pnl(n))), Nil{}, True{}}

# the 3D viewer's keys (Key3.act), when it is open and no field has focus
def Nui.key.view3(on: Bool, +n: Nui, +code: U32, +mods: U32) -> Step:
  match on:
    case True{}:
      Nui.key.view3.k(U32.is_eq(Key3.act(code, mods), 0), n, Key3.act(code, mods))
    case False{}:
      Step{n, Nil{}, False{}}

def Nui.key.free(nofocus: Bool, fit: Bool, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  match nofocus fit:
    case True{} True{}:
      Step{Nui.with_vw(n, Vw.refit(Nui.vw(n))), Nil{}, True{}}
    case True{} False{}:
      Nui.key.view3(Bool.and(Nui.panel(n), U32.is_eq(Vw.kind(Nui.vw(n)), 2)), n, code, mods)
    case _ _:
      Nui.field.k(String.eq(Nui.focus_of(n), "composer"), Nui.in_path(n), n, Nui.mark(n), code, text, mods)

# Esc leaves a field; in the project search it also shuts the search
def Esc.field(find: Bool, +n: Nui) -> Step:
  match find:
    case True{}:
      Nui.focus(Nui.with_ui(n, U.Ui.scratch_set(U.Ui.flag_off(Nui.ui(n), "psearch"), "@f.pq", "")), "")
    case False{}:
      Nui.focus(n, "")

# Esc in the composer shuts the menu it offers (completions) and does
# nothing else; with none shown it leaves the field
def Esc.menu(menu: Bool, +n: Nui) -> Step:
  match menu:
    case True{}:
      Nui.act(n, "skill-close", "")
    case False{}:
      Esc.field(String.eq(Nui.focus_of(n), Fld.id("pq", "proj-go", "")), n)

def Nui.key.k(paste: Bool, esc: Bool, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  match paste esc:
    case True{} _:
      Step{n, [NPaste{}], False{}}
    case False{} True{}:
      Esc.menu(Bool.and(String.eq(Nui.focus_of(n), "composer"), Nui.skills_on(Nui.ui(n))), n)
    case False{} False{}:
      Nui.key.free(String.is_empty(Nui.focus_of(n)), U32.is_eq(code, 102), n, code, text, mods)

def Nui.key.term.go(term: Bool, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  match term:
    case True{}:
      Nui.act.k(U.Ui.term_keys(Nui.ui(n), VT.Vt.key(U.Ui.term(Nui.ui(n), U.Ui.sel(Nui.ui(n))), code, text, Keys.vt(mods))), n)
    case False{}:
      Nui.key.k(Bool.and(Keys.ctrl(mods), U32.is_eq(code, 118)), U32.is_eq(code, Key.escape()), n, code, text, mods)

def Nui.key.proj(proj: Bool, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  match proj:
    case True{}:
      Pick.key(n, code, text, mods)
    case False{}:
      Nui.key.term.go(False{}, n, code, text, mods)

def Pal.keyact(i: Nat, +n: Nui) -> Step:
  match i:
    case 0n:
      Nui.shortcut.k(0n, n, 0)
    case 1n:
      Nui.focus(Nui.with_path(n, "", 0n), "attach-path")
    case _:
      Step{n, Nil{}, False{}}

# the palette's key: items (the terminal; the attach path field)
# "key:attach" opens the attach path field; other "key:" rows run their shortcut
def Pal.key.named(attach: Bool, +n: Nui, +name: String) -> Step:
  match attach:
    case True{}:
      Pal.keyact(1n, n)
    case False{}:
      Nui.shortcut.k(U.Keys.pos(Keys.list(), name), n, 0)

def Pal.run.k(key: Bool, +n: Nui, +a: String, +v: String) -> Step:
  match key:
    case True{}:
      Pal.key.named(String.eq(String.drop(a, 4n), "attach"), n, String.drop(a, 4n))
    case False{}:
      Pal.act(String.eq(a, "mention"), n, a, v)

def Pal.run(it: U.PItem, +n: Nui) -> Step:
  match it:
    case U.PItem{_, +a, +v}:
      Step.merge(Pal.close(n), Pal.run.k(String.starts_with(a, "key:"), Step.nui(Pal.close(n)), a, v))

def Pal.key.k(esc: Bool, enter: Bool, up: Bool, down: Bool, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  match esc enter up down:
    case True{} _ _ _:
      Pal.close(n)
    case _ True{} _ _:
      Pal.run(U.Palette.at(U.Palette.items(Nui.ui(n)), U.Palette.index(Nui.ui(n))), n)
    case _ _ True{} _:
      Pal.moved(n, Nat.sub(U.Palette.index(Nui.ui(n)), 1n))
    case _ _ _ True{}:
      Pal.moved(n, 1n+U.Palette.index(Nui.ui(n)))
    case _ _ _ _:
      Pal.typed(Ed.key(code, text, mods, U.Palette.query(Nui.ui(n)), String.length(U.Palette.query(Nui.ui(n))), String.length(U.Palette.query(Nui.ui(n)))), n)

def Pal.key(+n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  Pal.key.k(U32.is_eq(code, Key.escape()), Bool.or(U32.is_eq(code, Key.enter()), U32.is_eq(code, Key.kp_enter())), U32.is_eq(code, 65362), U32.is_eq(code, 65364), n, code, text, mods)

# a click on a palette row runs it
def Pal.click(+n: Nui, +v: String) -> Step:
  Pal.run(U.Palette.at(U.Palette.items(Nui.ui(n)), Maybe.default(&2, Nat, Nat.read(v), 0n)), n)

# Layers and sheets
# -----------------
# "@vw.layers" is set while the layer list is open. "@vw.tree" holds the
# schematic's sheet tree (HI.Hier.show rows, sent by the shell) and
# "@vw.at" the row on show; the viewer's path is that row's file.

def Vx.path(x: V.Vx) -> String:
  match x:
    case V.Vx{_, _, _, _, _, _, _, _, pa, _, bm}:
      pa

def Nui.rows(+n: Nui) -> List<&2, HI.Row>:
  HI.Hier.read(U.Ui.scratch(Nui.ui(n), "@vw.tree"))

# the file on show: the viewer's path, or the tree's root when it has none
def Nui.sheet.file(+n: Nui) -> String:
  Bool.pick(String, String.is_empty(Vx.path(Vw.x(Nui.vw(n)))), HI.Hier.file(Nui.rows(n), 0n), Vx.path(Vw.x(Nui.vw(n))))

# the tree row on show
def Nui.sheet.at(+n: Nui) -> Nat:
  HI.Hier.at(Nui.rows(n), Maybe.default(&2, Nat, Nat.read(U.Ui.scratch(Nui.ui(n), "@vw.at")), 0n), Nui.sheet.file(n))

# show tree row i (a new scene: it fades in and refits)
def Nui.sheet.go(+n: Nui, +i: Nat) -> Step:
  Step{Nui.with_vw(Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), "@vw.at", Nat.show(i))),
    Vw.kind_set(Nui.vw(n), 1, U.Rv.sheet(Vw.path(Nui.vw(n)), HI.Hier.file(Nui.rows(n), i)))), Nil{}, True{}}

def Nui.sheet.open(m: Maybe<&2, Nat>, +n: Nui) -> Step:
  match m:
    case Some{+i}:
      Nui.sheet.go(n, i)
    case None{}:
      Step{n, Nil{}, False{}}

def Nui.layers.set(+n: Nui, +flip: U32) -> Step:
  Step{Nui.with_vw(Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), Nui.flip.key(n), U32.show(flip))), Vw.stale(Nui.vw(n))), Nil{}, True{}}

# Reset layers: every layer as it is by default, no focus
def Nui.layers.reset(+n: Nui) -> Step:
  Rvw.reset(Nui.layers.set(Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), Nui.lfocus.key(n), "")), 0))


# the viewer's sun/moon button: its own light or dark ground, whatever
# the window's theme; the scene is drawn again on it
def Nui.vlight.flip(+n: Nui) -> Step:
  Step{Nui.with_vw(Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), "@vw.light", Bool.pick(String, Nui.vlight(n), "", "1"))), Vw.stale(Nui.vw(n))), Nil{}, True{}}

def Nui.layers.open(+n: Nui) -> Step:
  Step{Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), "@vw.layers", Bool.pick(String, String.is_empty(U.Ui.scratch(Nui.ui(n), "@vw.layers")), "1", ""))), Nil{}, True{}}

def Nui.vw_act.more(i: Nat, +n: Nui, +value: String) -> Step:
  match i:
    case 0n:
      Nui.layers.open(n)
    case 1n:
      Nui.layers.set(n, V.Look.toggle(Nui.flip(n), Maybe.default(&2, U32, U32.read(value), 31)))
    case 2n:
      Nui.layers.reset(n)
    case 3n:
      Nui.sheet.go(n, Maybe.default(&2, Nat, Nat.read(value), 0n))
    case 4n:
      Nui.sheet.open(HI.Hier.child(Nui.rows(n), Nui.sheet.at(n), value), n)
    case 5n:
      Nui.sheet.go(n, HI.Hier.parent(Nui.rows(n), Nui.sheet.at(n)))
    case 6n:
      Nui.vlight.flip(n)
    case 7n:
      Nui.shortcut.k(U.Keys.pos(Keys.list(), value), n, 0)
    # a click on a layer's name: it stands out (LY.Lyr.pick)
    case 8n:
      Nui.lfocus.set(n, LY.Lyr.pick(Nui.lfocus(n), LY.Lyr.num(value, LY.Lyr.none())))
    # the list's All, Dim, Only
    case 9n:
      Nui.lfocus.set(n, LY.Lyr.set(Nui.look.kind(n), Nui.lfocus(n), LY.Lyr.num(value, 0)))
    # the "N new" pill over the timeline's foot (layout.bend's TL.pill)
    case 10n:
      Hold.bottom(n)
    case _:
      Step{n, Nil{}, False{}}

def Nui.vw_act.k(i: Nat, +n: Nui, +value: String) -> Step:
  match i:
    case 0n:
      Step{Nui.with_ui(n, U.Ui.view_pick.bd(Nui.ui(n), View.name(Maybe.default(&2, U32, U32.read(value), 0)))), Nil{}, True{}}
    case 1n:
      Step{Nui.with_vw(n, Vw.pick_set(Nui.vw(n), "")), Nil{}, True{}}
    case 2n:
      Pal.click(n, value)
    case 3n:
      Pal.close(n)
    case 4n:
      Pick.click(n, value)
    case 5n:
      Pick.close(n)
    case 6n:
      Pick.open(n)
    case 7n:
      Pal.open(n, "", value)
    case _:
      Nui.vw_act.more(Nat.sub(i, 8n), n, value)

# the window's own actions (they never reach the client state)
def Nui.vw_act(+n: Nui, +action: String, +value: String) -> Step:
  Nui.vw_act.k(Nui.pos(["vw-kind", "vw-unpick", "vw-pal", "vw-palx", "vw-proj", "vw-projx", "vw-projo", "vw-find", "vw-layers", "vw-layer", "vw-layers-reset", "vw-sheet", "vw-open", "vw-up", "vw-light", "vw-key", "vw-layer-pick", "vw-layer-mode", "vw-bottom"], action, 0n), n, value)

# a link in a message: only a web address is ever opened (law link_no_script)
def Nui.click.url(url: Bool, +n: Nui, +action: String, +value: String) -> Step:
  match url:
    case True{}:
      Step{n, Bool.pick(List<&2, NCmd>, LK.Link.is_web(value), [NOpen{value}], Nil{}), False{}}
    case False{}:
      Nui.act(n, action, value)

def Nui.focus.at(field: Bool, +n: Nui, +value: String) -> Step:
  match field:
    case True{}:
      Fld.end(Nui.focus(n, value))
    case False{}:
      Nui.focus(n, value)

# a bot's settings tab starts from its persona as it is
def Bots.persona(+n: Nui) -> String:
  BT.Bot.persona(BT.World.bot(M.State.bots(U.Ui.state(Nui.ui(n))), U.Ui.scratch(Nui.ui(n), "@bot")))

# another bot, room or tab starts its pane at the top
def Bots.reset(u: U.Ui, +persona: String) -> U.Ui:
  U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.scratch_set(u, "@set.sc", ""), "@chat.sc", ""), "@f.persona", persona)

def Bots.pre(+n: Nui, +action: String, +value: String) -> Nui:
  Bool.pick(Nui, Bool.or(Bool.or(String.eq(action, "bot"), String.eq(action, "bot-tab")), Bool.or(String.eq(action, "room"), String.eq(action, "remote"))),
    Nui.with_ui(n, Bots.reset(Nui.ui(n), Bool.pick(String, Bool.and(String.eq(action, "bot-tab"), String.eq(value, "settings")), Bots.persona(n), U.Ui.scratch(Nui.ui(n), "@f.persona")))), n)

def Nui.click.act(focus: Bool, +n: Nui, +action: String, +value: String) -> Step:
  match focus:
    case True{}:
      Nui.focus.at(Fld.on(value), n, value)
    case False{}:
      Bool.pick(Step, String.starts_with(action, "vw-"), Nui.vw_act(n, action, value), Nui.click.url(String.eq(action, "open-url"), Bots.pre(n, action, value), action, value))

def Nui.key.term.k(term: Bool, pal: Bool, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  match term pal:
    case False{} True{}:
      Pal.key(n, code, text, mods)
    case False{} False{}:
      Nui.key.proj(String.eq(Nui.focus_of(n), "proj"), n, code, text, mods)
    case _ _:
      Nui.key.term.go(term, n, code, text, mods)

# keys in the terminal go to the shell (as xterm would send them)
def Nui.key.term(term: Bool, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  Nui.key.term.k(term, String.eq(Nui.focus_of(n), "palette"), n, code, text, mods)

# keys for the bot's page while the person drives it (Playwright's names)
def Bb.keyname(+code: U32) -> String:
  Bool.pick(String, Bool.or(U32.is_eq(code, Key.enter()), U32.is_eq(code, Key.kp_enter())), "Enter",
    Bool.pick(String, U32.is_eq(code, Key.backspace()), "Backspace",
      Bool.pick(String, U32.is_eq(code, Key.tab()), "Tab",
        Bool.pick(String, U32.is_eq(code, Key.escape()), "Escape",
          Bool.pick(String, U32.is_eq(code, Key.delete()), "Delete",
            Bool.pick(String, U32.is_eq(code, Key.left()), "ArrowLeft",
              Bool.pick(String, U32.is_eq(code, Key.right()), "ArrowRight",
                Bool.pick(String, U32.is_eq(code, Key.up()), "ArrowUp",
                  Bool.pick(String, U32.is_eq(code, Key.down()), "ArrowDown",
                    Bool.pick(String, U32.is_eq(code, Key.home()), "Home",
                      Bool.pick(String, U32.is_eq(code, Key.end()), "End",
                        Bool.pick(String, U32.is_eq(code, 65365), "PageUp", Bool.pick(String, U32.is_eq(code, 65366), "PageDown", ""))))))))))))) 

def Bb.char(+text: U32) -> String:
  SCon{Chr{text}, SNil{}}

def Bb.key.c(chord: Bool, +n: Nui, +text: U32) -> Step:
  match chord:
    case True{}:
      Nui.act(n, "bb-key", "Control+" ++ Bb.char(U32.or(text, 32)))
    case False{}:
      Step{n, Nil{}, False{}}

def Bb.key.t(plain: Bool, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  match plain:
    case True{}:
      Nui.act(n, "bb-type", Bb.char(text))
    case False{}:
      Bb.key.c(Bool.and(Keys.ctrl(mods), U32.is_ne(text, 0)), n, text)

def Bb.key.k(named: Bool, +n: Nui, +name: String, +code: U32, +text: U32, +mods: U32) -> Step:
  match named:
    case True{}:
      Nui.act(n, "bb-key", Bool.pick(String, Bool.and(Keys.shift(mods), String.eq(name, "Tab")), "Shift+Tab", name))
    case False{}:
      Bb.key.t(Bool.and(U32.is_ne(text, 0), U32.is_eq(U32.and(mods, 6), 0)), n, code, text, mods)

def Bb.key(+n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  Bb.key.k(Bool.not(String.is_empty(Bb.keyname(code))), n, Bb.keyname(code), code, text, mods)

# the frame has the keyboard while the person drives the page
def Bb.keys(+n: Nui) -> Bool:
  Bool.and(String.eq(Nui.focus_of(n), "bb-frame"), U.Teach.input(Nui.ui(n), U.Bots.cur(Nui.ui(n))))

def Nui.key.bb(on: Bool, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  match on:
    case True{}:
      Bb.key(n, code, text, mods)
    case False{}:
      Nui.key.term(String.eq(Nui.focus_of(n), "terminal"), n, code, text, mods)

# keys in a terminal of the chat go to its program
def Nui.key.emb(none: Bool, +key: String, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  match none:
    case True{}:
      Nui.key.bb(Bb.keys(n), n, code, text, mods)
    case False{}:
      Nui.act.k(U.Ui.emb_keys(Nui.ui(n), key, VT.Vt.key(U.Ui.term(Nui.ui(n), key), code, text, Keys.vt(mods))), n)

def Nui.key.go.k(none: Bool, +name: String, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  match none:
    case False{}:
      Nui.shortcut.k(U.Keys.pos(Keys.list(), name), n, code)
    case True{}:
      Nui.key.emb(String.is_empty(EM.Emb.focused(Nui.focus_of(n))), EM.Emb.focused(Nui.focus_of(n)), n, code, text, mods)

def Nui.key.go(+name: String, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  Nui.key.go.k(String.is_empty(name), name, n, code, text, mods)

# Esc first closes an open menu (the model picker, the machine list, a
# badge editor)
def Nui.key.menu(menu: Bool, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  match menu:
    case True{}:
      Step{Menu.shut(False{}, n), Nil{}, True{}}
    case False{}:
      Nui.key.go(Keys.name(code, mods, Bool.or(String.eq(Nui.focus_of(n), "terminal"), String.starts_with(Nui.focus_of(n), "emb:"))), n, code, text, mods)

def Nui.key.m(+n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  Nui.key.menu(Bool.and(U32.is_eq(code, Key.escape()), Bool.or(Bool.or(U.Ui.flag(Nui.ui(n), "models"), U.Ui.flag(Nui.ui(n), "machines")),
    Bool.not(String.is_empty(U.Ui.scratch(Nui.ui(n), "@look") ++ U.Ui.scratch(Nui.ui(n), "@menu"))))), n, code, text, mods)

# Selecting thread text
# ---------------------
# A press on a timeline line starts a selection there ("@ts" is
# thread|line,x|line,x; layout.bend's TSel turns it into characters), a
# drag moves its far end while "@ts.on" is set, and a press anywhere else
# not on a control drops it. Ctrl+c copies it (the frame carries its text
# as the "tl-sel" region) unless the composer has a selection of its own.

def Sel.put(+n: Nui, +v: String, +on: String) -> Nui:
  Nui.with_ui(n, U.Ui.scratch_set(U.Ui.scratch_set(Nui.ui(n), "@ts", v), "@ts.on", on))

def Sel.start(+n: Nui, +line: String, +x: U32) -> Step:
  +p = line ++ "," ++ U32.show(x)
  Step{Sel.put(n, U.Ui.sel(Nui.ui(n)) ++ "|" ++ p ++ "|" ++ p, "1"), Nil{}, True{}}

def Sel.from(+n: Nui) -> String:
  String.join(List.take(&2, String, String.split(U.Ui.scratch(Nui.ui(n), "@ts"), '|'), 2n), "|")

def Sel.to(m: Maybe<&2, Hit>, +n: Nui, +x: U32) -> Step:
  match m:
    case Some{Hit{_, _, _, _, +a, +v}}:
      Bool.pick(Step, String.eq(a, "tl-line"), Step{Sel.put(n, Sel.from(n) ++ "|" ++ v ++ "," ++ U32.show(x), "1"), Nil{}, True{}}, Step{n, Nil{}, False{}})
    case None{}:
      Step{n, Nil{}, False{}}

def Sel.line.k(hit: Bool, h: Hit, rest: Maybe<&2, Hit>) -> Maybe<&2, Hit>:
  match hit:
    case True{}:
      Some{h}
    case False{}:
      rest

def Hit.is(h: Hit, +a: String) -> Bool:
  match h:
    case Hit{_, _, _, _, +b, _}:
      String.eq(a, b)

# the timeline line at (x, y), under whatever else is there
def Sel.line(hs: List<&2, Hit>, +x: U32, +y: U32) -> Maybe<&2, Hit>:
  match hs:
    case Nil{}:
      None{}
    case +h <> t:
      Sel.line.k(Bool.and(Hit.inside(h, x, y), Hit.is(h, "tl-line")), h, Sel.line(t, x, y))

def Sel.on(+n: Nui) -> Bool:
  String.eq(U.Ui.scratch(Nui.ui(n), "@ts.on"), "1")

def Nui.move.s(sel: Bool, drag: Bool, +n: Nui, +x: U32, +y: U32, +mods: U32) -> Step:
  match sel:
    case True{}:
      Sel.to(Sel.line(Nui.hits(n), x, y), n, x)
    case False{}:
      Nui.move.k(drag, n, x, y, mods)

def Nui.move.c(comp: Bool, drag: Bool, +n: Nui, +x: U32, +y: U32, +mods: U32) -> Step:
  match comp:
    case True{}:
      Cq.ask(n, Cq.at("m", x, y))
    case False{}:
      Nui.move.s(Sel.on(n), drag, n, x, y, mods)

def Nui.move(drag: Bool, +n: Nui, +x: U32, +y: U32, +mods: U32) -> Step:
  Nui.move.c(Cq.dragging(n), drag, n, x, y, mods)

def Sel.end(s: Step) -> Step:
  match s:
    case Step{+n, cs, d}:
      Step{Nui.with_ui(n, U.Ui.scratch_set(U.Ui.scratch_set(Nui.ui(n), "@ts.on", ""), "@cdrag", "")), cs, d}

def Sel.text.go(hs: List<&2, Hit>) -> String:
  match hs:
    case Nil{}:
      ""
    case Hit{_, _, _, _, +a, +v} <> t:
      Bool.pick(String, String.eq(a, "tl-sel"), v, Sel.text.go(t))

# the selected thread text the last frame showed ("" for none)
def Sel.text(+n: Nui) -> String:
  Sel.text.go(Nui.hits(n))

# ctrl+c with thread text selected copies it, unless the keys belong to
# the terminal or the composer has a selection of its own
def Nui.key.copy(copy: Bool, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  match copy:
    case True{}:
      Step{n, [NCopy{Sel.text(n)}], False{}}
    case False{}:
      Nui.key.m(n, code, text, mods)

def Nui.key(+n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  Nui.key.copy(Bool.and(Bool.and(Ed.ctrl(code, mods, 99), Bool.not(String.is_empty(Sel.text(n)))),
    Bool.and(Bool.not(String.eq(Nui.focus_of(n), "terminal")), Nat.is_eq(Nui.mark(n), Nui.caret_of(n)))), n, code, text, mods)

def Row.press.a(line: Bool, +n: Nui, +action: String, +value: String, +x: U32, +y: U32) -> Step:
  match line:
    case True{}:
      Sel.start(n, value, x)
    case False{}:
      Row.press.k(Bool.and(Bool.or(String.eq(action, "select"), String.eq(action, "project")), U32.is_lt(x, 264)), Nui.click.act(String.eq(action, "focus"), n, action, value),
        Bool.pick(String, String.eq(action, "project"), "project:" ++ value, value), y)

# The bot's page while the person drives it (client.bend's Teach, layout.bend's
# Lay.page): a press on the frame is a click at that spot, in thousandths of
# the frame ("bb-frame" hit, value "x y w h" of its box), and takes the
# keyboard; the wheel scrolls it, typed text and keys go to the page.
def Bb.part(+v: String, +i: Nat) -> U32:
  U32.from_nat(M.Switch.num(Maybe.default(&2, String, List.get(&2, String, String.split(v, ' '), i), ""), 0n))

def Bb.thou(+p: U32, +o: U32, +w: U32) -> U32:
  U32.min(1000, (((Bool.pick(U32, U32.is_ge(p, o), (p - o : U32), 0)) * 1000 : U32) / U32.max(w, 1) : U32))

def Bb.at(+v: String, +x: U32, +y: U32) -> String:
  U32.show(Bb.thou(x, Bb.part(v, 0n), Bb.part(v, 2n))) ++ " " ++ U32.show(Bb.thou(y, Bb.part(v, 1n), Bb.part(v, 3n)))

def Bb.press(+n: Nui, +v: String, +x: U32, +y: U32) -> Step:
  Nui.focus.step(Nui.act(n, "bb-click", Bb.at(v, x, y)), "bb-frame")

def Row.press.b(frame: Bool, +n: Nui, +action: String, +value: String, +x: U32, +y: U32) -> Step:
  match frame:
    case True{}:
      Bb.press(n, value, x, y)
    case False{}:
      Row.press.a(String.eq(action, "tl-line"), n, action, value, x, y)

def Row.press.c(comp: Bool, +n: Nui, +action: String, +value: String, +x: U32, +y: U32) -> Step:
  match comp:
    case True{}:
      Cq.press(n, x, y)
    case False{}:
      Row.press.b(String.eq(action, "bb-frame"), n, action, value, x, y)

def Row.press(m: Maybe<&2, Hit>, +n: Nui, +x: U32, +y: U32) -> Step:
  match m:
    case Some{Hit{_, _, _, _, +action, +value}}:
      Row.press.c(Bool.and(String.eq(action, "focus"), String.eq(value, "composer")), n, action, value, x, y)
    case None{}:
      Step{Sel.put(n, "", ""), Nil{}, True{}}

def Nui.press.k(drag: Bool, +n: Nui, +x: U32, +y: U32) -> Step:
  match drag:
    case True{}:
      Nui.view_press(n, x, y, True{})
    case False{}:
      Row.press(Hit.find(Nui.hits(n), x, y), n, x, y)

# a right-click on a sidebar row opens its menu where the pointer is
# (core's Menu: "row-menu"), on a project's name or badge the project's
# ("proj-menu"), on a file link in a message the file's ("file-menu"); it
# never acts on the row by itself
def Row.menu.at(+n: Nui, +x: U32, +y: U32) -> Nui:
  Nui.with_ui(n, U.Ui.scratch_set(U.Ui.scratch_set(Nui.ui(n), "@menu.x", U32.show(x)), "@menu.y", U32.show(y)))

def Row.menu.f(file: Bool, +n: Nui, +value: String, +x: U32, +y: U32) -> Step:
  match file:
    case True{}:
      Nui.act(Row.menu.at(n, x, y), "file-menu", value)
    case False{}:
      Step{n, Nil{}, False{}}

def Row.menu.k(row: Bool, proj: Bool, file: Bool, +n: Nui, +value: String, +x: U32, +y: U32) -> Step:
  match row proj:
    case True{} _:
      Nui.act(Row.menu.at(n, x, y), "row-menu", value)
    case False{} True{}:
      Nui.act(Row.menu.at(n, x, y), "proj-menu", value)
    case False{} False{}:
      Row.menu.f(file, n, value, x, y)

# Review comments (client.bend's Rc): a right-click on the board or the
# schematic starts a note at that place, "rv-note" with "x\ty\tlayer\titem"
# (mm in the file's coordinates, the item under the pointer), and puts the
# keyboard in the note's field
def Rcw.at(v: V.Vw, +x: U32, +y: U32, +fk: U32, +flip: U32) -> String:
  match v:
    case V.Vw{+ss, V.View{+z, +ox, +oy}, _, _, _, _, _, _, _, _, V.Vx{+k, _, _, _, _, _, _, _, _, _, _}}:
      F32.show(F32.div(F32.sub(U32.to_f32(x), ox), z)) ++ "\t" ++ F32.show(F32.div(F32.sub(U32.to_f32(y), oy), z)) ++ "\t\t"
        ++ V.Pick.at(ss, V.View{z, ox, oy}, U32.to_f32(x), U32.to_f32(y), U32.or(k, fk), flip)

def Rcw.start(s: Step) -> Step:
  match s:
    case Step{n, cs, d}:
      Step{Nui.with_caret(Nui.with_focus(n, Fld.id("rcnote", "rv-note-send", "")), 0n), cs, d}

# a right press on nothing (Row.menu's None): on the board or schematic,
# a note at that place; elsewhere nothing
def Rcw.none(on: Bool, +n: Nui, +x: U32, +y: U32) -> Step:
  match on:
    case True{}:
      Rcw.start(Nui.act(n, "rv-note", Rcw.at(Nui.vw(n), x, y, Nui.lfocus.bits(n), Nui.flip(n))))
    case False{}:
      Step{n, Nil{}, False{}}

def Rcw.here(+n: Nui, +x: U32, +y: U32) -> Bool:
  Bool.and(Bool.and(Nui.in_viewer(n, x), U32.is_ge(y, 52)), U32.is_lt(Vw.kind(Nui.vw(n)), 2))

def Row.menu(m: Maybe<&2, Hit>, +n: Nui, +x: U32, +y: U32) -> Step:
  match m:
    case Some{Hit{_, _, _, _, +action, +value}}:
      Row.menu.k(Bool.and(String.eq(action, "select"), U32.is_lt(x, 264)),
        Bool.and(Bool.or(String.eq(action, "project"), String.eq(action, "look")), U32.is_lt(x, 264)), String.eq(action, "file-open"), n, value, x, y)
    case None{}:
      Rcw.none(Rcw.here(n, x, y), n, x, y)

# a press on the viewer's own buttons is a click, elsewhere in it a drag
def Nui.press(viewer: Bool, +n: Nui, +x: U32, +y: U32) -> Step:
  Nui.press.k(Bool.and(viewer, Nui.nohit(Hit.find(Nui.hits(n), x, y))), n, x, y)

# the middle button drags in the viewer too (SolidWorks rotates with it)
def Nui.mpress(viewer: Bool, +n: Nui, +x: U32, +y: U32) -> Step:
  match viewer:
    case True{}:
      Nui.view_press(n, x, y, True{})
    case False{}:
      Step{n, Nil{}, False{}}

# Hotkey hints
# ------------
# While alt is held the sidebar numbers its projects (alt+1..9), while
# ctrl is held the threads of this project (ctrl+1..9). "@hint" is "alt",
# "ctrl" or "".

def Hint.of(+code: U32) -> String:
  Bool.pick(String, Bool.or(U32.is_eq(code, 65513), U32.is_eq(code, 65514)), "alt",
    Bool.pick(String, Bool.or(U32.is_eq(code, 65507), U32.is_eq(code, 65508)), "ctrl", ""))

def Hint.get(+n: Nui) -> String:
  U.Ui.scratch(Nui.ui(n), "@hint")

def Hint.set(+n: Nui, +h: String) -> Nui:
  Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), "@hint", h))

# the hint's modifier is still down
def Hint.held(+h: String, +mods: U32) -> Bool:
  Bool.or(Bool.and(String.eq(h, "alt"), U32.is_ne(U32.and(mods, 4), 0)), Bool.and(String.eq(h, "ctrl"), U32.is_ne(U32.and(mods, 2), 0)))

# the hint after a key goes down: alt or ctrl starts one; a key pressed
# without its modifier ends it (a release we never saw)
def Hint.down(+h: String, +code: U32, +mods: U32) -> String:
  Bool.pick(String, String.is_empty(Hint.of(code)), Bool.pick(String, Hint.held(h, mods), h, ""), Hint.of(code))

def Hint.dirty(s: Step, d: Bool) -> Step:
  match s:
    case Step{n, cs, dd}:
      Step{n, cs, Bool.or(dd, d)}

def Hint.key(+h: String, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  Hint.dirty(Nui.key(Hint.set(n, h), code, text, mods), Bool.not(String.eq(h, Hint.get(n))))

# releasing the hint's modifier ends it
def Hint.up(+code: U32, +n: Nui) -> Step:
  Bool.pick(Step, Bool.and(String.eq(Hint.of(code), Hint.get(n)), Bool.not(String.is_empty(Hint.get(n)))),
    Step{Hint.set(n, ""), Nil{}, True{}}, Step{n, Nil{}, False{}})

# the window lost focus: whatever was held is let go (and the cats stop
# asking for frames: "@blur")
def Blur.set(+n: Nui, +v: String) -> Nui:
  Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), "@blur", v))

def Hint.blur(+n: Nui) -> Step:
  Step{Blur.set(Hint.set(n, ""), "1"), Nil{}, Bool.not(String.is_empty(Hint.get(n)))}

# the window has the keyboard again: a frame, so moving cats go on
def Nui.focused(+n: Nui) -> Step:
  Step{Blur.set(n, ""), Nil{}, True{}}

# whether the window has the keyboard
def Nui.has_focus(+n: Nui) -> Bool:
  String.is_empty(U.Ui.scratch(Nui.ui(n), "@blur"))

# this window is a desk (core/desk.bend): after its input it may tell the
# hub which thread it shows and whether it has the focus (no frame)
def Nui.desk.k(a: U.Act, n: Nui) -> Step:
  match a:
    case U.Act{u, +cs}:
      Step{Nui.with_ui(n, u), Nui.cmds(cs), False{}}

# the person's own input: what keeps a desk in use (core/desk.bend). A
# redraw, a resize, a drag passing over or the window closing is not: a
# window left open and focused stops holding the phone's alerts once the
# person stops using it (law desk_redraw_not_input)
def Nui.input.one(e: W.WinEv) -> Bool:
  match e:
    case W.WExpose{}:
      False{}
    case W.WSize{_, _}:
      False{}
    case W.WQuit{}:
      False{}
    case W.WDragOver{_, _}:
      False{}
    case W.WDragLeave{}:
      False{}
    case _:
      True{}

def Nui.input(es: List<&2, W.WinEv>) -> Bool:
  match es:
    case Nil{}:
      False{}
    case e <> t:
      Bool.or(Nui.input.one(e), Nui.input(t))

def Nui.desk(+n: Nui, +now: Nat) -> Step:
  Nui.desk.k(U.Ui.desk(Nui.ui(n), Nui.has_focus(n), now), n)

# a new connection hears the look again
def Nui.desk.reset(+n: Nui) -> Nui:
  Nui.with_ui(n, U.Ui.desk.reset(Nui.ui(n)))

# a wheel notch: over the driven page it scrolls the page, else as before
def Bb.wheel(none: Bool, +n: Nui, +x: U32, +y: U32, +dir: U32) -> Step:
  match none:
    case True{}:
      Pane.spin(String.is_empty(Hit.value_at(Nui.hits(n), "pane", x, y)), Hit.value_at(Nui.hits(n), "pane", x, y), n, x, y, dir)
    case False{}:
      Nui.act(n, "bb-scroll", Bool.pick(String, U32.is_eq(dir, 0), "-120", "120"))

# Input method
# ------------
# What the input method composes (core/ime.bend) is kept in the scratch as
# "@ime" and is owned by the field that had the keys when it began (the
# thread and the focus id). It is drawn at that field's caret (layout.bend)
# and is never in the field's text, the draft sent to the hub, the undo
# history or the clipboard. The method's commit is typed text: it goes in at
# the field's selection as one undo step, whatever it looks like (a path or
# a URI is text, never an attachment).

def Imx.get(+n: Nui) -> IM.Ime:
  IM.Ime.read(U.Ui.scratch(Nui.ui(n), "@ime"))

def Imx.put(+n: Nui, +i: IM.Ime) -> Nui:
  Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), "@ime", IM.Ime.show(i)))

# whether a composition is showing
def Imx.live(+n: Nui) -> Bool:
  Bool.not(String.is_empty(U.Ui.scratch(Nui.ui(n), "@ime")))

# who owns a composition begun now
def Imx.owner(+n: Nui) -> String:
  U.Ui.sel(Nui.ui(n)) ++ "|" ++ Nui.focus_of(n)

# a field that takes text has the keys
def Imx.field(+n: Nui) -> Bool:
  Bool.or(Bool.or(String.eq(Nui.focus_of(n), "composer"), Nui.in_path(n)), Fld.on(Nui.focus_of(n)))

# the composition ends (cancelled, committed, or its field lost the keys)
def Imx.drop(+n: Nui) -> Nui:
  Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), "@ime", ""))

# the composition is not the focused field's any more
def Imx.stale(+n: Nui) -> Bool:
  Bool.and(Imx.live(n), Bool.not(String.eq(IM.Ime.owner(Imx.get(n)), Imx.owner(n))))

# the preedit changed: characters [first, first + len) became text, or
# only their feedback changed; the caret is inside the preedit
def Imx.draw.k(field: Bool, +only: Bool, +n: Nui, +first: U32, +len: U32, +text: String, +fb: String, +caret: U32) -> Step:
  match field:
    case True{}:
      Step{Imx.put(n, IM.Ime.at(Bool.pick(IM.Ime, only, IM.Ime.mark(IM.Ime.sync(Imx.get(n), Imx.owner(n)), U32.to_nat(first), U32.to_nat(len), fb),
        IM.Ime.draw(Imx.get(n), Imx.owner(n), U32.to_nat(first), U32.to_nat(len), text, fb)), U32.to_nat(caret))), Nil{}, True{}}
    case False{}:
      Step{n, Nil{}, False{}}

def Imx.draw(+n: Nui, +first: U32, +len: U32, text: String, fb: String, +caret: U32, only: Bool) -> Step:
  Imx.draw.k(Imx.field(n), only, n, first, len, text, fb, caret)

def Imx.caret(+n: Nui, +pos: U32) -> Step:
  Step{Imx.put(n, IM.Ime.at(Imx.get(n), U32.to_nat(pos))), Nil{}, Imx.live(n)}

# the composition ended with nothing committed
def Imx.end(+n: Nui) -> Step:
  Step{Imx.drop(n), Nil{}, Imx.live(n)}

# committed text typed in at the focused field
def Imx.type.k(comp: Bool, path: Bool, n: Nui, +m: Nat, +text: String) -> Step:
  match comp path n:
    case True{} _ Nui{+u, w, h, mx, my, sd, sc, +ca, fo, pa, hi, ms, vw}:
      Nui.composer.k(Ed.insert(U.Ui.draft_text(u), ca, m, text), Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}, False{})
    case False{} True{} Nui{u, w, h, mx, my, sd, sc, +ca, fo, +pa, hi, ms, vw}:
      Nui.path.k(Ed.insert(pa, ca, m, text), Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw})
    case _ _ +n2:
      Fld.paste(Fld.on(Nui.focus_of(n2)), n2, m, text)

def Imx.commit(+n: Nui, +text: String) -> Step:
  Imx.type.k(String.eq(Nui.focus_of(n), "composer"), Nui.in_path(n), Imx.drop(n), Nui.mark(n), text)

# The composer shows the composition in its draft at the caret (the near
# end of a selection, which is hidden meanwhile); the other fields are
# shown the same way by laying the frame out over Imx.display.
def Imx.mine(+n: Nui) -> Bool:
  Bool.and(Imx.live(n), Bool.and(String.eq(Nui.focus_of(n), "composer"), String.eq(IM.Ime.owner(Imx.get(n)), Imx.owner(n))))

def Imx.lo(+n: Nui) -> Nat:
  ED.Edit.lo(Nui.caret_of(n), Nui.mark(n))

def Imx.draft.k(mine: Bool, +n: Nui) -> String:
  match mine:
    case True{}:
      IM.Ime.shown(Imx.get(n), U.Ui.draft_text(Nui.ui(n)), Imx.lo(n))
    case False{}:
      U.Ui.draft_text(Nui.ui(n))

def Imx.draft(+n: Nui) -> String:
  Imx.draft.k(Imx.mine(n), n)

def Imx.at.k(mine: Bool, +n: Nui) -> Nat:
  match mine:
    case True{}:
      IM.Ime.caret(Imx.get(n), Imx.lo(n))
    case False{}:
      Nui.caret_of(n)

def Imx.at(+n: Nui) -> Nat:
  Imx.at.k(Imx.mine(n), n)

def Imx.mk.k(mine: Bool, +n: Nui) -> Nat:
  match mine:
    case True{}:
      IM.Ime.caret(Imx.get(n), Imx.lo(n))
    case False{}:
      Nui.mark(n)

def Imx.mk(+n: Nui) -> Nat:
  Imx.mk.k(Imx.mine(n), n)

def Imx.runs.k(mine: Bool, +n: Nui) -> List<&2, IM.Run>:
  match mine:
    case True{}:
      IM.Ime.runs(Imx.get(n), Imx.lo(n))
    case False{}:
      Nil{}

def Imx.runs(+n: Nui) -> List<&2, IM.Run>:
  Imx.runs.k(Imx.mine(n), n)

# a single-line field (not the composer) has the composition
def Imx.other(+n: Nui) -> Bool:
  Bool.and(Imx.live(n), Bool.and(Bool.not(String.eq(Nui.focus_of(n), "composer")), String.eq(IM.Ime.owner(Imx.get(n)), Imx.owner(n))))

def Imx.path_text(+n: Nui) -> String:
  match n:
    case Nui{_, _, _, _, _, _, _, _, _, pa, _, _, _}:
      pa

# the focused field's text as it is when laid out over Imx.display
def Imx.field_shown(+n: Nui) -> String:
  Bool.pick(String, Nui.in_path(n), Imx.path_text(n), Fld.text(n))

# where the composition sits in it: the caret was moved into it
def Imx.field_runs(+n: Nui) -> List<&2, IM.Run>:
  IM.Ime.runs(Imx.get(n), Nat.sub(Nui.caret_of(n), IM.Ime.caret(Imx.get(n), 0n)))

# the window as the frame is laid out: another field showing the
# composition in its text, the caret inside it (nothing of this is kept)
def Imx.display.t(path: Bool, +n: Nui) -> Nui:
  match path:
    case True{}:
      +c = IM.Ime.caret(Imx.get(n), Imx.lo(n))
      Mark.save(Nui.with_path(n, IM.Ime.shown(Imx.get(n), Imx.path_text(n), Imx.lo(n)), c), c)
    case False{}:
      +c = IM.Ime.caret(Imx.get(n), Imx.lo(n))
      Mark.save(Nui.with_caret(Fld.set(n, IM.Ime.shown(Imx.get(n), Fld.text(n), Imx.lo(n))), c), c)

def Imx.display.k(go: Bool, +n: Nui) -> Nui:
  match go:
    case True{}:
      Imx.display.t(Nui.in_path(n), n)
    case False{}:
      n

def Imx.display(+n: Nui) -> Nui:
  Imx.display.k(Bool.and(Imx.other(n), Bool.or(Nui.in_path(n), Fld.on(Nui.focus_of(n)))), n)

# keys that reach the app while a composition shows are not the app's:
# Enter does not send, and the arrows, Tab and Escape do nothing (Escape
# ends the composition)
def Imx.eats(+code: U32) -> Bool:
  Bool.or(Bool.or(Bool.or(U32.is_eq(code, Key.enter()), U32.is_eq(code, Key.kp_enter())), Bool.or(U32.is_eq(code, Key.up()), U32.is_eq(code, Key.down()))),
    Bool.or(U32.is_eq(code, Key.tab()), U32.is_eq(code, Key.escape())))

def Imx.key.k(eats: Bool, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  match eats:
    case True{}:
      Step{Bool.pick(Nui, U32.is_eq(code, Key.escape()), Imx.drop(n), n), Nil{}, True{}}
    case False{}:
      Hint.key(Hint.down(Hint.get(n), code, mods), n, code, text, mods)

def Imx.key(live: Bool, +n: Nui, +code: U32, +text: U32, +mods: U32) -> Step:
  match live:
    case True{}:
      Imx.key.k(Imx.eats(code), n, code, text, mods)
    case False{}:
      Hint.key(Hint.down(Hint.get(n), code, mods), n, code, text, mods)

# one window event
def Nui.event(+n: Nui, e: W.WinEv) -> Step:
  match e:
    case W.WKey{+code, text, +mods, True{}, _}:
      Imx.key(Imx.live(n), n, code, text, mods)
    case W.WKey{+code, _, _, False{}, _}:
      Hint.up(code, n)
    case W.WBlur{}:
      Hint.blur(Imx.drop(n))
    case W.WFocus{}:
      Nui.focused(n)
    case W.WCommit{_, t}:
      Imx.commit(n, t)
    case W.WPreedit{+first, +len, _, t, f, +caret, only}:
      Imx.draw(n, first, len, t, f, caret, only)
    case W.WPreCaret{+pos}:
      Imx.caret(n, pos)
    case W.WPreStart{}:
      Step{Imx.drop(n), Nil{}, False{}}
    case W.WPreEnd{}:
      Imx.end(n)
    case W.WJunk{}:
      Step{n, Nil{}, False{}}
    case W.WButton{+x, +y, 1, True{}}:
      Nui.press(Nui.in_viewer(n, x), Menu.press(Hit.find(Nui.hits(n), x, y), n), x, y)
    case W.WButton{+x, +y, 1, False{}}:
      Sel.end(Row.release(Bool.not(String.is_empty(Row.id(n))), n, x, y))
    case W.WButton{+x, +y, 3, True{}}:
      Row.menu(Hit.find(Nui.hits(n), x, y), Menu.press(Hit.find(Nui.hits(n), x, y), n), x, y)
    case W.WButton{+x, +y, 2, True{}}:
      Nui.mpress(Nui.in_viewer(n, x), n, x, y)
    case W.WButton{+x, +y, 2, False{}}:
      Nui.view_press(n, x, y, False{})
    case W.WButton{_, _, _, _}:
      Step{n, Nil{}, False{}}
    case W.WMove{+x, +y, +mods}:
      Nui.move(Nui.dragging(n), n, x, y, mods)
    case W.WWheel{+x, +y, +dir}:
      Bb.wheel(String.is_empty(Hit.value_at(Nui.hits(n), "bb-frame", x, y)), n, x, y, dir)
    case W.WSize{w, h}:
      Nui.resize(n, w, h)
    case W.WExpose{}:
      Step{n, Nil{}, True{}}
    case W.WQuit{}:
      Step{n, [NQuit{}], False{}}
    case W.WPaste{t}:
      Nui.paste(n, t)
    case W.WImage{}:
      Step{n, Nil{}, False{}}
    case W.WDragOver{_, _}:
      Nui.drag_over(n)
    case W.WDragLeave{}:
      Nui.drag_leave(n)
    case W.WDrop{t}:
      Nui.drop(n, t)

# The image lightbox
# ------------------
# A click on an image in a thread shows it over everything: "@lb" is its
# path, "@lb.v" the view (core/lightbox.bend's Lb, as text). While it is
# up the pointer and keys are its own: the wheel zooms at the pointer
# (eased), a drag pans, and Escape, the close square or a click off the
# picture closes it.

def Lbx.path(+n: Nui) -> String:
  U.Ui.scratch(Nui.ui(n), "@lb")

def Lbx.on(+n: Nui) -> Bool:
  Bool.not(String.is_empty(Lbx.path(n)))

def Lbx.get(+n: Nui) -> LB.Lb:
  LB.Lb.read(U.Ui.scratch(Nui.ui(n), "@lb.v"))

def Lbx.set(+n: Nui, lb: LB.Lb) -> Nui:
  Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), "@lb.v", LB.Lb.show(lb)))

# the picture's size: its full decode ("img:full:<path>") once there, else
# the thread's copy
def Lbx.size.k(+full: String, +thumb: String) -> String:
  Bool.pick(String, Bool.or(String.is_empty(full), String.starts_with(full, "err:")), thumb, full)

def Lbx.size(+n: Nui) -> String:
  Lbx.size.k(U.Ui.info(Nui.ui(n), "img:full:" ++ Lbx.path(n)), U.Ui.info(Nui.ui(n), "img:" ++ Lbx.path(n)))

def Lbx.dim(+s: String, +i: Nat) -> Nat:
  Maybe.default(&2, Nat, Nat.read(Maybe.default(&2, String, List.get(&2, String, String.split(s, 'x'), i), "")), 0n)

def Lbx.box(+n: Nui) -> LB.LbBox:
  LB.LbBox{Lbx.dim(Lbx.size(n), 0n), Lbx.dim(Lbx.size(n), 1n), U32.to_nat(Nui.width(n)), U32.to_nat(Nui.height(n))}

# still easing (the loop asks for frames only then)
def Lbx.busy(+n: Nui, +ms: Nat) -> Bool:
  Bool.and(Lbx.on(n), LB.Lb.busy(Lbx.get(n), ms))

# an ease waiting for its first frame starts at this one
def Lbx.stamp.k(on: Bool, +n: Nui, +ms: Nat) -> Nui:
  match on:
    case True{}:
      Lbx.set(n, LB.Lb.stamp(Lbx.get(n), ms))
    case False{}:
      n

def Lbx.stamp(+n: Nui, +ms: Nat) -> Nui:
  Lbx.stamp.k(Lbx.on(n), n, ms)

def Lbx.open(+n: Nui, +path: String) -> Step:
  Step{Nui.with_ui(n, U.Ui.scratch_set(U.Ui.scratch_set(Nui.ui(n), "@lb", path), "@lb.v", LB.Lb.show(LB.Lb.open()))), Nil{}, True{}}

def Lbx.close(+n: Nui) -> Step:
  Step{Nui.with_ui(n, U.Ui.scratch_set(U.Ui.scratch_set(Nui.ui(n), "@lb", ""), "@lb.v", "")), Nil{}, True{}}

def Lbx.step(+n: Nui, lb: LB.Lb) -> Step:
  Step{Lbx.set(n, lb), Nil{}, True{}}

def Lbx.release(closes: Bool, +n: Nui, +x: U32, +y: U32) -> Step:
  match closes:
    case True{}:
      Lbx.close(n)
    case False{}:
      Lbx.step(n, LB.Lb.release(Lbx.get(n), U32.to_nat(x), U32.to_nat(y), Lbx.box(n), Nui.ms_of(n)))

def Lbx.move(down: Bool, +n: Nui, +x: U32, +y: U32) -> Step:
  match down:
    case True{}:
      Lbx.step(n, LB.Lb.move(Lbx.get(n), U32.to_nat(x), U32.to_nat(y), Lbx.box(n), Nui.ms_of(n)))
    case False{}:
      Step{n, Nil{}, False{}}

def Lbx.key(esc: Bool, +n: Nui) -> Step:
  match esc:
    case True{}:
      Lbx.close(n)
    case False{}:
      Step{n, Nil{}, False{}}

# one window event while the lightbox is up
def Lbx.event(+n: Nui, e: W.WinEv) -> Step:
  match e:
    case W.WKey{+code, _, _, True{}, _}:
      Lbx.key(U32.is_eq(code, Key.escape()), n)
    case W.WButton{+x, +y, 1, True{}}:
      Lbx.step(n, LB.Lb.press(Lbx.get(n), U32.to_nat(x), U32.to_nat(y), Lbx.box(n), Nui.ms_of(n)))
    case W.WButton{+x, +y, 1, False{}}:
      Lbx.release(Bool.or(LB.Lb.closes(Lbx.get(n), U32.to_nat(x), U32.to_nat(y)), LB.Lb.close_hit(U32.to_nat(x), U32.to_nat(y), U32.to_nat(Nui.width(n)))), n, x, y)
    case W.WMove{+x, +y, _}:
      Lbx.move(LB.Lb.is_down(Lbx.get(n)), n, x, y)
    case W.WWheel{+x, +y, +dir}:
      Lbx.step(n, LB.Lb.wheel(Lbx.get(n), U32.is_eq(dir, 0), 100n, True{}, U32.to_nat(x), U32.to_nat(y), Lbx.box(n), Nui.ms_of(n)))
    case W.WSize{w, h}:
      Nui.resize(n, w, h)
    case W.WExpose{}:
      Step{n, Nil{}, True{}}
    case W.WQuit{}:
      Step{n, [NQuit{}], False{}}
    case _:
      Step{n, Nil{}, False{}}

def Lbx.hit(m: Maybe<&2, Hit>) -> String:
  match m:
    case Some{Hit{_, _, _, _, +a, +v}}:
      Bool.pick(String, String.eq(a, "img"), v, "")
    case None{}:
      ""

def Lbx.press(+path: String, +n: Nui, e: W.WinEv) -> Step:
  Bool.pick(Step, String.is_empty(path), Nui.event(n, e), Lbx.open(n, path))

def Lbx.pressed(+n: Nui, e: W.WinEv) -> Step:
  match e:
    case W.WButton{+x, +y, 1, True{}}:
      Lbx.press(Lbx.hit(Hit.find(Nui.hits(n), x, y)), n, W.WButton{x, y, 1, True{}})
    case _:
      Nui.event(n, e)

# one window event: the lightbox's while it is up; a press on an image
# opens it
def Nui.event.lb(open: Bool, +n: Nui, e: W.WinEv) -> Step:
  match open:
    case True{}:
      Lbx.event(n, e)
    case False{}:
      Lbx.pressed(n, e)

# The first-run tour (core/onboard.bend) is modal: Enter and the right
# arrow go on, the left arrow goes back, Esc ends it (OB.Ob.sym), a press on
# one of its buttons ("ob-" hits in layout.bend's Lay.tour, and the uberbot
# step's "uber-" choices) acts, and nothing
# else gets through but the window's own events: a resize, an expose, focus
# lost or gained (so "@blur" stays true to the window), and the quit.
# Every physical key (keycode) pressed while the tour is up is held in
# quarantine until its release, whichever way the tour closes (a button, Esc,
# the hub saying done, a lost connection): a press of a held key, a repeat
# included, does nothing at all, so a key held through Done never reaches the
# composer ("@q.<keycode>"). win.c tells real releases from a server's
# repeat pairs and, when the window gets the focus back, reports the releases
# it missed; a blur itself keeps the quarantine.
def Tour.key(+name: String, +n: Nui) -> Step:
  Bool.pick(Step, String.is_empty(name), Step{n, Nil{}, False{}}, Nui.act(n, name, ""))

def Tour.press(m: Maybe<&2, Hit>, +n: Nui) -> Step:
  match m:
    case Some{Hit{_, _, _, _, +a, +v}}:
      Bool.pick(Step, Bool.or(Bool.and(String.starts_with(a, "ob-"), Bool.not(String.eq(a, "ob-scroll"))), String.starts_with(a, "uber-")), Nui.act(n, a, v), Step{n, Nil{}, False{}})
    case None{}:
      Step{n, Nil{}, False{}}

# the wheel over the card scrolls its words two lines a notch, kept within
# what overflows (the card's hit says how many lines that is)
def Tour.scroll(+cur: Nat, +mx: Nat, up: Bool) -> Nat:
  Bool.pick(Nat, up, Bool.pick(Nat, Nat.is_lt(cur, 2n), 0n, Nat.sub(cur, 2n)), Bool.pick(Nat, Nat.is_lt(mx, Nat.add(cur, 2n)), mx, Nat.add(cur, 2n)))

def Tour.wheel.k(+n: Nui, +v: String, up: Bool) -> Step:
  Step{Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), "@tour.sc", Nat.show(Tour.scroll(Maybe.default(&2, Nat, Nat.read(U.Ui.scratch(Nui.ui(n), "@tour.sc")), 0n), Maybe.default(&2, Nat, Nat.read(v), 0n), up)))), Nil{}, True{}}

def Tour.wheel(m: Maybe<&2, Hit>, +n: Nui, up: Bool) -> Step:
  match m:
    case Some{Hit{_, _, _, _, +a, +v}}:
      Bool.pick(Step, String.eq(a, "ob-scroll"), Tour.wheel.k(n, v, up), Step{n, Nil{}, False{}})
    case None{}:
      Step{n, Nil{}, False{}}

def Tour.event(+n: Nui, e: W.WinEv) -> Step:
  match e:
    case W.WKey{+code, _, _, True{}, _}:
      Tour.key(OB.Ob.sym(code), n)
    case W.WButton{+x, +y, 1, True{}}:
      Tour.press(Hit.find(Nui.hits(n), x, y), n)
    case W.WWheel{+x, +y, +dir}:
      Tour.wheel(Hit.find(Nui.hits(n), x, y), n, U32.is_eq(dir, 0))
    case W.WBlur{}:
      Hint.blur(n)
    case W.WFocus{}:
      Nui.focused(n)
    case W.WSize{w, h}:
      Nui.resize(n, w, h)
    case W.WExpose{}:
      Step{n, Nil{}, True{}}
    case W.WQuit{}:
      Step{n, [NQuit{}], False{}}
    case _:
      Step{n, Nil{}, False{}}

# the physical key (keycode) an event presses (0: not a key press) or lets go
def Held.press(e: W.WinEv) -> U32:
  match e:
    case W.WKey{_, _, _, True{}, +k}:
      k
    case _:
      0

def Held.rel(e: W.WinEv) -> U32:
  match e:
    case W.WKey{_, _, _, False{}, +k}:
      k
    case _:
      0

def Held.mark(+k: U32) -> String:
  "@q." ++ U32.show(k)

def Held.has(+u: U.Ui, +k: U32) -> Bool:
  Bool.not(String.is_empty(U.Ui.scratch(u, Held.mark(k))))

def Held.set(+n: Nui, +k: U32) -> Nui:
  Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), Held.mark(k), "1"))

def Held.let(+n: Nui, +k: U32) -> Nui:
  Nui.with_ui(n, U.Ui.scratch_set(Nui.ui(n), Held.mark(k), ""))

def Nui.event.go(tour: Bool, +n: Nui, e: W.WinEv) -> Step:
  match tour:
    case True{}:
      Tour.event(n, e)
    case False{}:
      Nui.event.lb(Lbx.on(n), n, e)

# a key held in quarantine does nothing; while the tour is up a key
# pressed joins it
def Nui.event.press.k(drop: Bool, +tour: Bool, +c: U32, +n: Nui, e: W.WinEv) -> Step:
  match drop:
    case True{}:
      Step{n, Nil{}, False{}}
    case False{}:
      Nui.event.go(tour, Bool.pick(Nui, tour, Held.set(n, c), n), e)

def Nui.event.press(+tour: Bool, +c: U32, +n: Nui, e: W.WinEv) -> Step:
  Nui.event.press.k(Held.has(Nui.ui(n), c), tour, c, n, e)

def Nui.event.t.k(press: Bool, +tour: Bool, +c: U32, +r: U32, +n: Nui, e: W.WinEv) -> Step:
  match press:
    case True{}:
      Nui.event.press(tour, c, n, e)
    case False{}:
      Nui.event.go(tour, Bool.pick(Nui, U32.is_ne(r, 0), Held.let(n, r), n), e)

def Nui.event.t(tour: Bool, +n: Nui, +e: W.WinEv) -> Step:
  Nui.event.t.k(U32.is_ne(Held.press(e), 0), tour, Held.press(e), Held.rel(e), n, e)

# a paste or a drop while the tour is up goes nowhere (the window's own
# clipboard and drop reads still happen in main.bend)
def Nui.paste.m(tour: Bool, +n: Nui, +text: String) -> Step:
  match tour:
    case True{}:
      Step{n, Nil{}, False{}}
    case False{}:
      Nui.paste(n, text)

def Nui.drop.m(tour: Bool, +n: Nui, +text: String) -> Step:
  match tour:
    case True{}:
      Step{n, Nil{}, False{}}
    case False{}:
      Nui.drop(n, text)

# every event of a batch, in order
def Nui.events(es: List<&2, W.WinEv>, s: Step) -> Step:
  match es s:
    case Nil{} _:
      s
    case e <> t Step{+n, cs, d}:
      Nui.events(t, Step.merge(Step{n, cs, d}, Nui.event.t(U.Tour.on(Nui.ui(n)), n, e)))

# The performance overlay
# -----------------------
# ctrl+shift+h (flag "perf") shows the window's last few seconds of work.
# Each frame drawn while it is up hands Hud.feed the rows perf.c holds
# now; the text compares them with a reading 3 to 6 s old. The readings
# stay in the scratch: "@perf.next" becomes "@perf.base" once it is
# PF.Hud.span() old.

def Hud.roll(old: Bool, +u: U.Ui, +cur: String, +ms: Nat) -> U.Ui:
  match old:
    case True{}:
      U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.scratch_set(u, "@perf.base", U.Ui.scratch(u, "@perf.next")),
        "@perf.at", U.Ui.scratch(u, "@perf.next.at")), "@perf.next", cur), "@perf.next.at", Nat.show(ms))
    case False{}:
      u

def Hud.show(+u: U.Ui, +cur: String, +ms: Nat) -> U.Ui:
  U.Ui.info_set(u, "perf.hud", PF.Hud.text(cur, U.Ui.scratch(u, "@perf.base"), PF.Perf.nat(U.Ui.scratch(u, "@perf.at")), ms))

def Hud.feed(+n: Nui, +cur: String, +ms: Nat) -> Nui:
  Nui.with_ui(n, Hud.show(Hud.roll(Nat.is_ge(PF.Perf.minus(ms, PF.Perf.nat(U.Ui.scratch(Nui.ui(n), "@perf.next.at"))), PF.Hud.span()), Nui.ui(n), cur, ms), cur, ms))

# Images use attach.put, so a local clipboard works on another hub too.
def Nui.image.parts(ps: List<&2, String>, +n: Nui, +key: String, +name: String, +size: Nat, +i: Nat) -> Step:
  match ps:
    case Nil{}:
      Step{n, Nil{}, False{}}
    case p <> +t:
      +step = Nui.act(n, "attach", AT.Att.paste.piece(key, name, size, i, List.is_empty(&2, String, t), p))
      Step.merge(step, Nui.image.parts(t, Step.nui(step), key, name, size, Nat.add(i, 1n)))

def Nui.image.upload(big: Bool, +n: Nui, +mime: String, +size: Nat, data: String, +key: String) -> Step:
  match big:
    case True{}:
      Nui.act(n, "attach", AT.Att.paste.piece(key, AT.Att.paste.name(mime), size, 0n, True{}, ""))
    case False{}:
      Nui.image.parts(AT.Att.paste.parts(data, AT.Att.paste.width()), n,
        key, AT.Att.paste.name(mime), size, 0n)

# Only the composer and its attachment path can receive an image. A tour,
# terminal, project path or another field cannot turn it into an upload.
def Nui.image.k(on: Bool, +n: Nui, mime: String, +size: Nat, data: String, key: String) -> Step:
  match on:
    case True{}:
      Nui.image.upload(Nat.is_gt(size, AT.Att.cap()), n, mime, size, data, key)
    case False{}:
      Step{n, Nil{}, False{}}

def Nui.image.m(tour: Bool, +n: Nui, +mime: String, size: Nat, data: String, key: String) -> Step:
  match tour:
    case True{}:
      Step{n, Nil{}, False{}}
    case False{}:
      Nui.image.k(Bool.and(String.starts_with(mime, "image/"),
        Bool.or(String.eq(Nui.focus_of(n), "composer"), String.eq(Nui.focus_of(n), "attach-path"))), n, mime, size, data, key)
