import Base
import ../core/i18n.bend as I
import ../core/fm.bend as FM
import ../core/images.bend as IM
import ./dom.bend as D

# A Markdown note opened from the Files tab (view.bend's View.md) is drawn
# with the chat's Markdown (Md.render); what its paths mean is decided here,
# once its nodes are made (pure; test/note_test.bend):
# - a path in a link is from the note's folder, not the thread's
# - an image (![alt](path), which the chat leaves as text) is shown, and
#   opens in the lightbox as an image in Files does
# - a file Files previews (another note, a board, a PDF) opens in Files
#   (fm-open, its path from the project's root); any other file opens as a
#   chat's file link does (file-open, its absolute path)
# - a link to a place in the page ([label](#part)) is its label
# A path that leaves the project, a web image and code stay as written.

# Paths
# -----

type NPath is Data:
  NIn{segs: List<&2, String>}
  NOut{}

def Note.pop(acc: List<&2, String>) -> NPath:
  match acc:
    case Nil{}:
      NOut{}
    case _ <> t:
      NIn{t}

def Note.seg.k(up: Bool, skip: Bool, +p: String, acc: List<&2, String>) -> NPath:
  match up skip:
    case True{} _:
      Note.pop(acc)
    case False{} True{}:
      NIn{acc}
    case False{} False{}:
      NIn{p <> acc}

# one segment more ("." and "" stay, ".." goes up; above the root is out)
def Note.seg(+p: String, n: NPath) -> NPath:
  match n:
    case NOut{}:
      NOut{}
    case NIn{acc}:
      Note.seg.k(String.eq(p, ".."), Bool.or(String.is_empty(p), String.eq(p, ".")), p, acc)

def Note.segs(ps: List<&2, String>, n: NPath) -> NPath:
  match ps:
    case Nil{}:
      n
    case +p <> t:
      Note.segs(t, Note.seg(p, n))

def Note.join(n: NPath) -> String:
  match n:
    case NOut{}:
      ""
    case NIn{acc}:
      String.join(List.reverse(&2, String, acc), "/")

def Note.first(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case x <> _:
      x

def Note.path.k(none: Bool, +dir: String, +file: String) -> String:
  match none:
    case True{}:
      ""
    case False{}:
      Note.join(Note.segs(String.split(file, '/'), Note.segs(String.split(dir, '/'), NIn{Nil{}})))

# a link's path from the note's folder (dir, from the project's root) as a
# path from the root, its anchor dropped: "" when it leaves the project or
# names no file
def Note.path(+dir: String, +link: String) -> String:
  +file = Note.first(String.split(link, '#'))
  Note.path.k(String.is_empty(file), dir, file)

# absolute, or under ~/, as a chat's file link may be
def Note.far(+link: String) -> Bool:
  Bool.or(String.starts_with(link, "/"), String.starts_with(link, "~/"))

# a web address, or any other scheme: not a file of the project
def Note.web(+link: String) -> Bool:
  String.contains(link, "://")

# a link's file from the project's root ("" when it is not in the project)
def Note.rel(+root: String, +dir: String, +link: String) -> String:
  Bool.pick(String, Note.web(link), "", Bool.pick(String, Note.far(link), FM.Fm.rel(root, link), Note.path(dir, link)))

# a link's file as an absolute path: as written when absolute or under ~/,
# else from the note's folder ("" when it leaves the project)
def Note.abs(+root: String, +dir: String, +link: String) -> String:
  +rel = Note.rel(root, dir, link)
  Bool.pick(String, Note.web(link), "", Bool.pick(String, Note.far(link), link, Bool.pick(String, String.is_empty(rel), "", FM.Fm.abs(root, rel))))

# the folder a note is in, from the project's root
def Note.dir(+root: String, +path: String) -> String:
  FM.Fm.parent(FM.Fm.rel(root, path))

# Links and images
# ----------------

def Note.attr(as: List<&2, D.Attr>, +name: String) -> String:
  match as:
    case Nil{}:
      ""
    case D.At{+n, +v} <> t:
      Bool.pick(String, String.eq(n, name), v, Note.attr(t, name))
    case _ <> t:
      Note.attr(t, name)

def Note.file.k(k: Nat, +rel: String, +abs: String, kids: List<&2, D.Node>) -> D.Node:
  match k:
    case 0n:
      D.El{"span", "", Nil{}, kids}
    case 1n:
      D.El{"a", "", [D.At{"class", "link file"}, D.At{"href", rel}, D.At{"title", rel}, D.On{"click", "fm-open"}, D.At{"data-value", rel}], kids}
    case _:
      D.Dom.file(abs, kids)

# a file link (Dom.file) in a note: a file Files previews opens there, any
# other on the hub's machine; one outside the project is only its label
def Note.file(+root: String, +dir: String, +link: String, kids: List<&2, D.Node>) -> D.Node:
  +rel = Note.rel(root, dir, link)
  +abs = Note.abs(root, dir, link)
  Note.file.k(Bool.pick(Nat, String.is_empty(abs), 0n, Bool.pick(Nat, Bool.and(Bool.not(String.is_empty(rel)), Bool.not(String.is_empty(FM.Fm.preview(rel)))), 1n, 2n)), rel, abs, kids)

def Note.link(file: Bool, +root: String, +dir: String, +k: String, +as: List<&2, D.Attr>, kids: List<&2, D.Node>) -> D.Node:
  match file:
    case True{}:
      Note.file(root, dir, Note.attr(as, "data-value"), kids)
    case False{}:
      D.El{"a", k, as, kids}

def Note.img.k(shown: Bool, +url: String, +alt: String, +link: String) -> D.Node:
  match shown:
    case True{}:
      D.El{"a", "", [D.At{"class", "fm-md-img"}, D.At{"href", url}, D.At{"data-lightbox", url}, D.At{"title", alt}],
        [D.El{"img", "", [D.At{"src", url}, D.At{"alt", alt}, D.At{"loading", "lazy"}], Nil{}}]}
    case False{}:
      D.Txt{"![" ++ alt ++ "](" ++ link ++ ")"}

# ![alt](link): the image when it is one in the project, else as written
def Note.img(+root: String, +dir: String, +alt: String, +link: String) -> D.Node:
  +abs = Note.abs(root, dir, link)
  Note.img.k(Bool.and(Bool.not(String.is_empty(abs)), IM.Img.is_image(abs)), IM.Img.url(abs), alt, link)

# Text
# ----
# One pass over a text node, char by char, for ![alt](link) and
# [label](#part) (the chat's Markdown keeps both as text). Text so far, the
# label and the link are kept reversed.

type NSc is Data:
  NText{acc: String}
  NBang{acc: String}
  NLab{acc: String, img: Bool, lab: String}
  NMid{acc: String, img: Bool, lab: String}
  NUrl{acc: String, img: Bool, lab: String, url: String}

def Note.bang(img: Bool, acc: String) -> String:
  match img:
    case True{}:
      SCon{'!', acc}
    case False{}:
      acc

# a "[" or "![" and the label read so far, back to text (reversed)
def Note.raw(img: Bool, lab: String, acc: String) -> String:
  String.append(lab, SCon{'[', Note.bang(img, acc)})

# "[label](link" back to text (reversed)
def Note.raw.url(img: Bool, lab: String, url: String, acc: String) -> String:
  String.append(url, SCon{'(', SCon{']', Note.raw(img, lab, acc)}})

def Note.flush(acc: String, out: List<&2, D.Node>) -> List<&2, D.Node>:
  match acc:
    case SNil{}:
      out
    case SCon{h, t}:
      D.Txt{String.reverse(SCon{h, t})} <> out

# a ")" closing [label](link): an image, a place in the page (its label),
# or anything else as written
def Note.close(k: Nat, +root: String, +dir: String, acc: String, +img: Bool, +lab: String, +url: String, out: List<&2, D.Node>) -> NSc & List<&2, D.Node>:
  match k:
    case 0n:
      (NText{SNil{}}, Note.img(root, dir, String.reverse(lab), String.reverse(url)) <> Note.flush(acc, out))
    case 1n:
      (NText{String.append(lab, acc)}, out)
    case _:
      (NText{SCon{')', Note.raw.url(img, lab, url, acc)}}, out)

def Note.url.k(k: Nat, +root: String, +dir: String, acc: String, +img: Bool, +lab: String, +url: String, +c: Char, out: List<&2, D.Node>) -> NSc & List<&2, D.Node>:
  match k:
    case 1n:
      Note.close(Bool.pick(Nat, img, 0n, Bool.pick(Nat, String.starts_with(String.reverse(url), "#"), 1n, 2n)), root, dir, acc, img, lab, url, out)
    case 2n:
      (NText{SCon{c, Note.raw.url(img, lab, url, acc)}}, out)
    case _:
      (NUrl{acc, img, lab, SCon{c, url}}, out)

def Note.step(+root: String, +dir: String, st: NSc, +c: Char, out: List<&2, D.Node>) -> NSc & List<&2, D.Node>:
  match st:
    case NText{+acc}:
      (Bool.pick(NSc, Char.is_eq(c, '!'), NBang{acc}, Bool.pick(NSc, Char.is_eq(c, '['), NLab{acc, False{}, SNil{}}, NText{SCon{c, acc}})), out)
    case NBang{+acc}:
      (Bool.pick(NSc, Char.is_eq(c, '['), NLab{acc, True{}, SNil{}}, Bool.pick(NSc, Char.is_eq(c, '!'), NBang{SCon{'!', acc}}, NText{SCon{c, SCon{'!', acc}}})), out)
    case NLab{+acc, +img, +lab}:
      (Bool.pick(NSc, Char.is_eq(c, ']'), NMid{acc, img, lab}, Bool.pick(NSc, Char.is_eq(c, '\n'), NText{SCon{c, Note.raw(img, lab, acc)}}, NLab{acc, img, SCon{c, lab}})), out)
    case NMid{+acc, +img, +lab}:
      (Bool.pick(NSc, Char.is_eq(c, '('), NUrl{acc, img, lab, SNil{}}, NText{SCon{c, SCon{']', Note.raw(img, lab, acc)}}}), out)
    case NUrl{acc, img, lab, url}:
      Note.url.k(Bool.pick(Nat, Char.is_eq(c, ')'), 1n, Bool.pick(Nat, Bool.or(Char.is_eq(c, ' '), Char.is_eq(c, '\n')), 2n, 0n)), root, dir, acc, img, lab, url, c, out)

def Note.step.pair(+root: String, +dir: String, +c: Char, r: NSc & List<&2, D.Node>) -> NSc & List<&2, D.Node>:
  (st, out) = r
  Note.step(root, dir, st, c, out)

def Note.fin(r: NSc & List<&2, D.Node>) -> List<&2, D.Node>:
  (st, out) = r
  match st:
    case NText{acc}:
      List.reverse(&2, D.Node, Note.flush(acc, out))
    case NBang{acc}:
      List.reverse(&2, D.Node, Note.flush(SCon{'!', acc}, out))
    case NLab{acc, img, lab}:
      List.reverse(&2, D.Node, Note.flush(Note.raw(img, lab, acc), out))
    case NMid{acc, img, lab}:
      List.reverse(&2, D.Node, Note.flush(SCon{']', Note.raw(img, lab, acc)}, out))
    case NUrl{acc, img, lab, url}:
      List.reverse(&2, D.Node, Note.flush(Note.raw.url(img, lab, url, acc), out))

def Note.go(+root: String, +dir: String, s: String, r: NSc & List<&2, D.Node>) -> List<&2, D.Node>:
  match s:
    case SNil{}:
      Note.fin(r)
    case SCon{+c, t}:
      Note.go(root, dir, t, Note.step.pair(root, dir, c, r))

def Note.text.k(has: Bool, +root: String, +dir: String, +s: String) -> List<&2, D.Node>:
  match has:
    case True{}:
      Note.go(root, dir, s, (NText{SNil{}}, Nil{}))
    case False{}:
      [D.Txt{s}]

def Note.text(+root: String, +dir: String, +s: String) -> List<&2, D.Node>:
  Note.text.k(String.contains(s, "]("), root, dir, s)

# Nodes
# -----

# every link and text of a note's nodes in its folder (code kept as is)
def Note.nodes(+root: String, +dir: String, ns: List<&2, D.Node>) -> List<&2, D.Node>:
  match ns:
    case Nil{}:
      Nil{}
    case D.Txt{s} <> t:
      List.append(&2, D.Node, Note.text(root, dir, s), Note.nodes(root, dir, t))
    case D.El{"a", +k, +as, kids} <> t:
      Note.link(String.contains(Note.attr(as, "class"), "file"), root, dir, k, as, kids) <> Note.nodes(root, dir, t)
    case D.El{"pre", k, as, kids} <> t:
      D.El{"pre", k, as, kids} <> Note.nodes(root, dir, t)
    case D.El{"code", k, as, kids} <> t:
      D.El{"code", k, as, kids} <> Note.nodes(root, dir, t)
    case D.El{tag, k, as, kids} <> t:
      D.El{tag, k, as, Note.nodes(root, dir, kids)} <> Note.nodes(root, dir, t)

# a note's nodes: its text as the chat's Markdown, its paths from path's
# folder in the project at root
def Note.render(+lc: I.Loc, +root: String, +path: String, +text: String) -> List<&2, D.Node>:
  Note.nodes(root, Note.dir(root, path), D.Md.render.loc(lc, text))
