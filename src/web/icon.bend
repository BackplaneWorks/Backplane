import Base
import ./dom.bend as D
import ../core/icons.bend as I

# Icons in the web client: the same path data the window rasterizes
# (core/icons.bend), written as inline <svg> drawn in the text colour.

def WIcon.path(l: I.Layer) -> D.Node:
  match l:
    case I.Layer{d, True{}}:
      D.El{"path", "", [D.At{"d", d}, D.At{"fill", "currentColor"}, D.At{"fill-rule", "evenodd"}, D.At{"stroke", "none"}], Nil{}}
    case I.Layer{d, False{}}:
      D.El{"path", "", [D.At{"d", d}], Nil{}}

def WIcon.paths(ls: List<&2, I.Layer>) -> List<&2, D.Node>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      WIcon.path(l) <> WIcon.paths(t)

# the icon, px square
def WIcon.svg(+name: String, +px: U32) -> D.Node:
  D.El{"svg", "", [D.At{"class", "ico"}, D.At{"data-icon", name}, D.At{"viewBox", "0 0 24 24"}, D.At{"width", U32.show(px)}, D.At{"height", U32.show(px)},
    D.At{"fill", "none"}, D.At{"stroke", "currentColor"}, D.At{"stroke-width", "2"}, D.At{"stroke-linecap", "round"}, D.At{"stroke-linejoin", "round"},
    D.At{"aria-hidden", "true"}], WIcon.paths(I.Icon.layers(name))}

# Hierarchy rows share exactly two immutable glyphs. Keep their changing
# labels, counts, targets and expanded state on the ordinary server button;
# only this fixed 14px picture is made by the existing client memo renderer.
def WIcon.tree(expanded: Bool) -> D.Node:
  D.El{"bp-memo", Bool.pick(String, expanded, "down", "right"), [D.At{"fn", "tree-icon"}], [D.Txt{""}]}

def WIcon.tree.make(key: String) -> D.Node:
  WIcon.svg(Bool.pick(String, String.eq(key, "down"), "chevron-down", "chevron-right"), 14)

def WIcon.label(label: String) -> List<&2, D.Node>:
  match label:
    case SNil{}:
      Nil{}
    case SCon{c, t}:
      [D.El{"span", "", Nil{}, [D.Txt{SCon{c, t}}]}]

# a button that sends `action` with `value`: the icon, then the label if any
def WIcon.button(cls: String, action: String, value: String, title: String, +name: String, +px: U32, label: String) -> D.Node:
  D.El{"button", "", [D.At{"class", cls}, D.At{"title", title}, D.At{"data-value", value}, D.On{"click", action}],
    WIcon.svg(name, px) <> WIcon.label(label)}

# a button with its label first and the icon after (a menu's chevron)
def WIcon.after(cls: String, action: String, value: String, title: String, label: String, +name: String, +px: U32) -> D.Node:
  D.El{"button", "", [D.At{"class", cls}, D.At{"title", title}, D.At{"data-value", value}, D.On{"click", action}],
    [D.El{"span", "", Nil{}, [D.Txt{label}]}, WIcon.svg(name, px)]}
