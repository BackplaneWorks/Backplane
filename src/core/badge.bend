import Base
import ./model.bend as M

# A project's badge: a colour and an icon, shown by its name in the
# sidebar, on its threads and in a thread's header. Both are settings
# (project.color.<id>, project.icon.<id>); without one a project gets a
# colour from its id and the folder icon (laws badge_*).

type Tint is Data:
  Tint{name: String, rgb: U32, css: String}

def Badge.tints() -> List<&2, Tint>:
  [Tint{"blue", 6003957, "#5b9cf5"}, Tint{"teal", 4177840, "#3fbfb0"}, Tint{"green", 6079594, "#5cc46a"},
   Tint{"lime", 10930250, "#a6c84a"}, Tint{"amber", 14725184, "#e0b040"}, Tint{"orange", 15238474, "#e8854a"},
   Tint{"red", 15033450, "#e5646a"}, Tint{"pink", 14708660, "#e06fb4"}, Tint{"violet", 10781672, "#a483e8"},
   Tint{"slate", 9082792, "#8a97a8"}]

def Badge.names.go(ts: List<&2, Tint>) -> List<&2, String>:
  match ts:
    case Nil{}:
      Nil{}
    case Tint{n, _, _} <> t:
      n <> Badge.names.go(t)

def Badge.colors() -> List<&2, String>:
  Badge.names.go(Badge.tints())

# the icons a project may take (each in core/icons.bend: law badge_icons_exist)
def Badge.icons() -> List<&2, String>:
  ["folder", "circuit-board", "cpu", "zap", "radio", "box", "layers", "code", "terminal", "globe", "wrench", "flask-conical", "book", "star"]

def Badge.has(xs: List<&2, String>, +x: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.or(String.eq(h, x), Badge.has(t, x))

# the k-th colour, any k past the list taking the last
def Badge.nth(k: Nat) -> String:
  match k:
    case 0n:
      "blue"
    case 1n:
      "teal"
    case 2n:
      "green"
    case 3n:
      "lime"
    case 4n:
      "amber"
    case 5n:
      "orange"
    case 6n:
      "red"
    case 7n:
      "pink"
    case 8n:
      "violet"
    case _:
      "slate"

def Badge.sum(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{c}, t}:
      Badge.sum(t, Nat.add(acc, U32.to_nat(c)))

# a project's colour when none is set: from its id, so it stays put
def Badge.auto(+id: String) -> String:
  Badge.nth(Nat.mod(Badge.sum(id, 0n), 10n))

# a set colour when it is one of the tints, else the automatic one
def Badge.color.of(+set: String, +id: String) -> String:
  Bool.pick(String, Badge.has(Badge.colors(), set), set, Badge.auto(id))

def Badge.icon.of(+set: String) -> String:
  Bool.pick(String, Badge.has(Badge.icons(), set), set, "folder")

def Badge.color_key(+id: String) -> String:
  "project.color." ++ id

def Badge.icon_key(+id: String) -> String:
  "project.icon." ++ id

def Badge.color(+st: M.State, +id: String) -> String:
  Badge.color.of(M.Setting.get(st, Badge.color_key(id), ""), id)

def Badge.icon(+st: M.State, +id: String) -> String:
  Badge.icon.of(M.Setting.get(st, Badge.icon_key(id), ""))

# a project whose icon was picked (not the folder every project starts with)
def Badge.picked(+st: M.State, +id: String) -> Bool:
  Bool.not(String.eq(Badge.icon(st, id), "folder"))

def Badge.split.all(ws: List<&2, String>, +c: Char) -> List<&2, String>:
  match ws:
    case Nil{}:
      Nil{}
    case w <> t:
      List.append(&2, String, String.split(w, c), Badge.split.all(t, c))

def Badge.words.k(empty: Bool, w: String, rest: List<&2, String>) -> List<&2, String>:
  match empty:
    case True{}:
      rest
    case False{}:
      w <> rest

def Badge.words.keep(ws: List<&2, String>) -> List<&2, String>:
  match ws:
    case Nil{}:
      Nil{}
    case +w <> t:
      Badge.words.k(String.is_empty(w), w, Badge.words.keep(t))

# a title's words, parted at spaces, '-', '_' and '.'
def Badge.words(+title: String) -> List<&2, String>:
  Badge.words.keep(Badge.split.all(Badge.split.all(Badge.split.all(String.split(String.trim(title), ' '), '-'), '_'), '.'))

def Badge.initials.k(ws: List<&2, String>) -> String:
  match ws:
    case Nil{}:
      ""
    case +a <> t:
      match t:
        case Nil{}:
          String.take(a, 2n)
        case b <> _:
          String.take(a, 1n) ++ String.take(b, 1n)

# what a project's badge shows where only a letter or two fit (the folded
# sidebar): the first letters of its first two words, or a one-word title's
# first two, in capitals ("sensor-node" SN, "PROJ" PR)
def Badge.initials(+title: String) -> String:
  String.to_upper(Badge.initials.k(Badge.words(title)))

def Badge.find(ts: List<&2, Tint>, +name: String, +d: Tint) -> Tint:
  match ts:
    case Nil{}:
      d
    case Tint{+n, +r, +c} <> t:
      Bool.pick(Tint, String.eq(n, name), Tint{n, r, c}, Badge.find(t, name, d))

def Badge.tint(+name: String) -> Tint:
  Badge.find(Badge.tints(), name, Tint{"slate", 9082792, "#8a97a8"})

def Tint.rgb(t: Tint) -> U32:
  match t:
    case Tint{_, r, _}:
      r

def Tint.css(t: Tint) -> String:
  match t:
    case Tint{_, _, c}:
      c

def Badge.rgb(+name: String) -> U32:
  Tint.rgb(Badge.tint(name))

def Badge.css(+name: String) -> String:
  Tint.css(Badge.tint(name))

# a project's colour as the window draws it, and as CSS
def Badge.rgb_of(+st: M.State, +id: String) -> U32:
  Badge.rgb(Badge.color(st, id))

def Badge.css_of(+st: M.State, +id: String) -> String:
  Badge.css(Badge.color(st, id))
