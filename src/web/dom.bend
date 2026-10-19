import Base
import ../core/i18n.bend as I
import ../core/msgs.bend as MSG
import ../core/ref.bend as R
import ../core/links.bend as LK
import ../core/diff.bend as DF

# A view is a tree of Nodes; the host patches the DOM to match it. Events
# carry an action name back to Ui.act together with a value: the element's
# `data-value` (literal, or "#id" for the value of the element with that
# id), else the element's own value.

type Attr is Data:
  At{name: String, value: String}
  On{event: String, action: String}

type Node is Data:
  El{tag: String, key: String, attrs: List<&2, Attr>, kids: List<&2, Node>}
  Txt{text: String}

def Dom.el(tag: String, attrs: List<&2, Attr>, kids: List<&2, Node>) -> Node:
  El{tag, "", attrs, kids}

def Dom.div(cls: String, kids: List<&2, Node>) -> Node:
  El{"div", "", [At{"class", cls}], kids}

def Dom.span(cls: String, text: String) -> Node:
  El{"span", "", [At{"class", cls}], [Txt{text}]}

# a button that sends `action` with `value`
def Dom.button(cls: String, action: String, value: String, title: String, label: String) -> Node:
  El{"button", "", [At{"class", cls}, At{"title", title}, At{"data-value", value}, On{"click", action}], [Txt{label}]}

def Dom.copy.loc(+lc: I.Loc, text: String) -> Node:
  El{"button", "", [At{"class", "copy"}, At{"title", MSG.T.common.copy(lc)}, At{"data-value", text}, On{"click", "copy"}], [Txt{MSG.T.common.copy(lc)}]}

def Dom.copy(text: String) -> Node:
  Dom.copy.loc(I.En{}, text)

def Dom.when(b: Bool, n: Node) -> List<&2, Node>:
  match b:
    case True{}:
      [n]
    case False{}:
      Nil{}

def Dom.cat(a: List<&2, Node>, b: List<&2, Node>) -> List<&2, Node>:
  List.append(&2, Node, a, b)

# Inline markdown
# ---------------

type Inl is Data:
  IText{acc: String}
  IStar{acc: String}
  ICode{acc: String}
  IBold{acc: String}
  IBoldStar{acc: String}
  IEm{acc: String}

type Glyph is Data:
  KTick{}
  KStar{}
  KSpace{}
  KOther{}

def Md.glyph.k(tick: Bool, star: Bool, space: Bool) -> Glyph:
  match tick star space:
    case True{} _ _:
      KTick{}
    case False{} True{} _:
      KStar{}
    case False{} False{} True{}:
      KSpace{}
    case False{} False{} False{}:
      KOther{}

def Md.glyph(c: Char) -> Glyph:
  match c:
    case Chr{+u}:
      Md.glyph.k(U32.is_eq(u, 96), U32.is_eq(u, 42), Bool.or(U32.is_eq(u, 32), U32.is_eq(u, 9)))

# a link to a file: a click asks the hub to open it ("file-open"), a
# right-click opens its menu ("file-menu"); href is the path as written
def Dom.file(+path: String, kids: List<&2, Node>) -> Node:
  El{"a", "", [At{"class", "link file"}, At{"href", path}, At{"title", path}, On{"click", "file-open"}, On{"contextmenu", "file-menu"}, At{"data-value", path}], kids}

def Dom.link.k(web: Bool, +label: String, +url: String) -> Node:
  match web:
    case True{}:
      El{"a", "", [At{"class", "link"}, At{"href", url}, At{"target", "_blank"}, At{"rel", "noopener noreferrer"}, At{"title", url}], [Txt{label}]}
    case False{}:
      Dom.file(url, [Txt{label}])

# Refs carry navigation metadata but display as plain text in spec 1.
def Dom.ref(+label: String, +uri: String, bare: Bool) -> Node:
  El{"a", "", [At{"class", "ref"}, At{"href", uri}, At{"data-ref", uri}, At{"data-bare", Bool.pick(String, bare, "1", "0")}], [Txt{label}]}

def Dom.link.ref(m: Maybe<&2, R.Ref>, label: String, +url: String) -> Node:
  match m:
    case Some{r}: Dom.ref(label, R.Ref.uri(r), False{})
    case None{}: Dom.link.k(LK.Link.is_web(url), label, url)

# A ref stays plain; a web link opens a new tab, a file opens on the hub.
def Dom.link(+label: String, +url: String) -> Node:
  Dom.link.ref(R.Ref.parse(url), label, url)

# text with its links (LK.Link.pieces) as nodes, onto out (reversed)
def Dom.pieces(ps: List<&2, LK.Piece>, out: List<&2, Node>) -> List<&2, Node>:
  match ps:
    case Nil{}:
      out
    case LK.PText{s} <> t:
      Dom.pieces(t, Txt{s} <> out)
    case LK.PLink{l, u} <> t:
      Dom.pieces(t, Dom.link(l, u) <> out)
    case LK.PRef{l, u, b} <> t:
      Dom.pieces(t, Dom.ref(l, u, b) <> out)

# text as nodes, its links clickable
def Dom.linked(+s: String) -> List<&2, Node>:
  List.reverse(&2, Node, Dom.pieces(LK.Link.pieces(s), Nil{}))

# a finished run of plain text (acc is reversed); its links become <a>
def Md.text(acc: String, out: List<&2, Node>) -> List<&2, Node>:
  match acc:
    case SNil{}:
      out
    case SCon{h, t}:
      Dom.pieces(LK.Link.pieces(String.reverse(SCon{h, t})), out)

def Md.code.k(path: Bool, +s: String) -> Node:
  match path:
    case True{}:
      Dom.file(s, [El{"code", "", Nil{}, [Txt{s}]}])
    case False{}:
      El{"code", "", Nil{}, [Txt{s}]}

# inline code; a path in backticks is a file's link
def Md.code(+acc: String) -> Node:
  Md.code.k(LK.Link.is_path(String.reverse(acc)), String.reverse(acc))

def Md.bold(acc: String) -> Node:
  El{"strong", "", Nil{}, [Txt{String.reverse(acc)}]}

def Md.em(acc: String) -> Node:
  El{"em", "", Nil{}, [Txt{String.reverse(acc)}]}

def Md.istep(st: Inl, k: Glyph, c: Char, out: List<&2, Node>) -> Inl & List<&2, Node>:
  match st k:
    case IText{acc} KTick{}:
      (ICode{SNil{}}, Md.text(acc, out))
    case IText{acc} KStar{}:
      (IStar{acc}, out)
    case IText{acc} _:
      (IText{SCon{c, acc}}, out)
    case IStar{acc} KStar{}:
      (IBold{SNil{}}, Md.text(acc, out))
    case IStar{acc} KSpace{}:
      (IText{SCon{c, SCon{'*', acc}}}, out)
    case IStar{acc} _:
      (IEm{SCon{c, SNil{}}}, Md.text(acc, out))
    case IEm{acc} KStar{}:
      (IText{SNil{}}, Md.em(acc) <> out)
    case IEm{acc} _:
      (IEm{SCon{c, acc}}, out)
    case ICode{acc} KTick{}:
      (IText{SNil{}}, Md.code(acc) <> out)
    case ICode{acc} _:
      (ICode{SCon{c, acc}}, out)
    case IBold{acc} KStar{}:
      (IBoldStar{acc}, out)
    case IBold{acc} _:
      (IBold{SCon{c, acc}}, out)
    case IBoldStar{acc} KStar{}:
      (IText{SNil{}}, Md.bold(acc) <> out)
    case IBoldStar{acc} _:
      (IBold{SCon{c, SCon{'*', acc}}}, out)

def Md.istep.pair(+c: Char, r: Inl & List<&2, Node>) -> Inl & List<&2, Node>:
  (st, out) = r
  Md.istep(st, Md.glyph(c), c, out)

def Md.ifin(r: Inl & List<&2, Node>) -> List<&2, Node>:
  (st, out) = r
  match st:
    case IText{acc}:
      List.reverse(&2, Node, Md.text(acc, out))
    case IStar{acc}:
      List.reverse(&2, Node, Md.text(SCon{'*', acc}, out))
    case ICode{acc}:
      List.reverse(&2, Node, Md.text(String.append(acc, "`"), out))
    case IBold{acc}:
      List.reverse(&2, Node, Md.text(String.append(acc, "**"), out))
    case IBoldStar{acc}:
      List.reverse(&2, Node, Md.text(String.append(SCon{'*', acc}, "**"), out))
    case IEm{acc}:
      List.reverse(&2, Node, Md.text(String.append(acc, "*"), out))

def Md.igo(s: String, r: Inl & List<&2, Node>) -> List<&2, Node>:
  match s:
    case SNil{}:
      Md.ifin(r)
    case SCon{+c, t}:
      Md.igo(t, Md.istep.pair(c, r))

# inline nodes of one line or paragraph
def Md.inline(s: String) -> List<&2, Node>:
  Md.igo(s, (IText{SNil{}}, Nil{}))

# Blocks
# ------

# a table row: the one a separator row (|---|) follows is the head
type Row is Data:
  RHead{cells: List<&2, String>}
  RBody{cells: List<&2, String>}

# an ordered list's item, its number as written
type OItem is Data:
  OItem{n: String, text: String}

type Blk is Data:
  BNone{}
  BPara{lines: List<&2, String>}
  BCode{lines: List<&2, String>}
  BList{items: List<&2, String>}
  BOl{items: List<&2, OItem>}
  BQuote{lines: List<&2, String>}
  BTable{rows: List<&2, Row>}

type Line is Data:
  LFence{}
  LHead{level: Nat, text: String}
  LItem{text: String}
  LNum{n: String, text: String}
  LQuote{text: String}
  LRow{cells: List<&2, String>}
  LSep{text: String}
  LHr{}
  LBlank{}
  LText{text: String}

def Md.is_u(s: String, +u: U32) -> Bool:
  match s:
    case SNil{}:
      False{}
    case SCon{Chr{+v}, _}:
      U32.is_eq(v, u)

def Md.is_digit(s: String) -> Bool:
  match s:
    case SNil{}:
      False{}
    case SCon{h, _}:
      Char.is_digit(h)

# how many leading characters pass (h: whether the head does), and the rest
def Md.run.hash(s: String, h: Bool, +n: Nat) -> Nat & String:
  match s h:
    case SCon{_, +t} True{}:
      Md.run.hash(t, Md.is_u(t, 35), 1n+n)
    case _ _:
      (n, s)

def Md.run.digit(s: String, h: Bool, +n: Nat) -> Nat & String:
  match s h:
    case SCon{_, +t} True{}:
      Md.run.digit(t, Md.is_digit(t), 1n+n)
    case _ _:
      (n, s)

# every character one of | - : and space
def Md.sep.ok(s: String, ok: Bool) -> Bool:
  match s:
    case SNil{}:
      ok
    case SCon{Chr{+u}, t}:
      Md.sep.ok(t, Bool.and(ok, Bool.or(Bool.or(U32.is_eq(u, 124), U32.is_eq(u, 45)), Bool.or(U32.is_eq(u, 58), U32.is_eq(u, 32)))))

# a table's separator row: | --- | :-: |
def Md.is_sep(+t: String) -> Bool:
  Bool.and(String.contains(t, "-"), Md.sep.ok(String.trim(t), True{}))

# every character u
def Md.all(s: String, +u: U32, ok: Bool) -> Bool:
  match s:
    case SNil{}:
      ok
    case SCon{Chr{+v}, t}:
      Md.all(t, u, Bool.and(ok, U32.is_eq(v, u)))

# a rule: three or more of - * or _
def Md.is_hr(+t: String) -> Bool:
  Bool.and(Nat.is_ge(String.length(t), 3n), Bool.or(Md.all(t, 45, True{}), Bool.or(Md.all(t, 42, True{}), Md.all(t, 95, True{}))))

def Md.cells.end(+xs: List<&2, String>) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(String.trim(Maybe.default(&2, String, List.last(&2, String, xs), "x"))),
    List.take(&2, String, xs, Nat.sub(List.length(&2, String, xs), 1n)), xs)

def Md.trims(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      String.trim(x) <> Md.trims(t)

# a row's cells: | a | b | gives a and b
def Md.cells(+t: String) -> List<&2, String>:
  Md.trims(Md.cells.end(List.drop(&2, String, String.split(String.trim(t), '|'), 1n)))

def Md.line.text(blank: Bool, +l: String) -> Line:
  match blank:
    case True{}:
      LBlank{}
    case False{}:
      LText{l}

def Md.line.num.k(num: Bool, +n: Nat, +r: String, +t: String, +l: String) -> Line:
  match num:
    case True{}:
      LNum{String.take(t, n), String.drop(r, 2n)}
    case False{}:
      Md.line.text(String.is_empty(String.trim(t)), l)

def Md.line.num(p: Nat & String, +t: String, +l: String) -> Line:
  (+n, +r) = p
  Md.line.num.k(Bool.and(Bool.and(Nat.is_gt(n, 0n), Nat.is_lt(n, 10n)), Bool.or(String.starts_with(r, ". "), String.starts_with(r, ") "))), n, r, t, l)

def Md.line.quote(q: Bool, +t: String, +l: String) -> Line:
  match q:
    case True{}:
      LQuote{String.trim_start(String.drop(t, 1n))}
    case False{}:
      Md.line.num(Md.run.digit(t, Md.is_digit(t), 0n), t, l)

def Md.line.row(sep: Bool, +t: String, +l: String) -> Line:
  match sep:
    case True{}:
      LSep{l}
    case False{}:
      LRow{Md.cells(t)}

def Md.line.table(pipe: Bool, +t: String, +l: String) -> Line:
  match pipe:
    case True{}:
      Md.line.row(Md.is_sep(t), t, l)
    case False{}:
      Md.line.quote(String.starts_with(t, ">"), t, l)

def Md.line.hr(hr: Bool, +t: String, +l: String) -> Line:
  match hr:
    case True{}:
      LHr{}
    case False{}:
      Md.line.table(String.starts_with(t, "|"), t, l)

def Md.line.item(item: Bool, +t: String, +l: String) -> Line:
  match item:
    case True{}:
      LItem{String.drop(t, 2n)}
    case False{}:
      Md.line.hr(Md.is_hr(String.trim(t)), t, l)

def Md.line.head.k(head: Bool, +n: Nat, +r: String, +t: String, +l: String) -> Line:
  match head:
    case True{}:
      LHead{n, String.trim_start(r)}
    case False{}:
      Md.line.item(Bool.or(Bool.or(String.starts_with(t, "- "), String.starts_with(t, "* ")), String.starts_with(t, "+ ")), t, l)

# a heading: one to six #, then a space
def Md.line.head(p: Nat & String, +t: String, +l: String) -> Line:
  (+n, +r) = p
  Md.line.head.k(Bool.and(Bool.and(Nat.is_gt(n, 0n), Nat.is_lt(n, 7n)), String.starts_with(r, " ")), n, r, t, l)

def Md.line.k(fence: Bool, +t: String, +l: String) -> Line:
  match fence:
    case True{}:
      LFence{}
    case False{}:
      Md.line.head(Md.run.hash(t, Md.is_u(t, 35), 0n), t, l)

def Md.line(+l: String) -> Line:
  +t = String.trim_start(l)
  Md.line.k(String.starts_with(t, "```"), t, l)

def Md.nl(first: Bool, acc: String) -> String:
  match first:
    case True{}:
      acc
    case False{}:
      SCon{'\n', acc}

# lines kept newest first, joined oldest first with "\n"; tail-recursive
# (String.join recursed once per line)
def Md.unlines.go(ls: List<&2, String>, first: Bool, acc: String) -> String:
  match ls:
    case Nil{}:
      acc
    case l <> t:
      Md.unlines.go(t, False{}, String.reverse.go(String.reverse(l), Md.nl(first, acc)))

def Md.unlines(ls: List<&2, String>) -> String:
  Md.unlines.go(ls, True{}, SNil{})

def Md.para(ls: List<&2, String>) -> Node:
  El{"p", "", Nil{}, Md.inline(Md.unlines(ls))}

def Md.li(s: String) -> Node:
  El{"li", "", Nil{}, Md.inline(s)}

def Md.lis(xs: List<&2, String>) -> List<&2, Node>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      Md.li(x) <> Md.lis(t)

def Md.oli(i: OItem) -> Node:
  match i:
    case OItem{n, t}:
      El{"li", "", [At{"value", n}], Md.inline(t)}

def Md.olis(xs: List<&2, OItem>) -> List<&2, Node>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      Md.oli(x) <> Md.olis(t)

def Md.tds(xs: List<&2, String>, +tag: String) -> List<&2, Node>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      El{tag, "", Nil{}, Md.inline(x)} <> Md.tds(t, tag)

def Md.tr(r: Row) -> Node:
  match r:
    case RHead{cs}:
      El{"tr", "", Nil{}, Md.tds(cs, "th")}
    case RBody{cs}:
      El{"tr", "", Nil{}, Md.tds(cs, "td")}

def Md.trs(rs: List<&2, Row>) -> List<&2, Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      Md.tr(r) <> Md.trs(t)

# a separator after the first row makes it the head
def Md.headed(rs: List<&2, Row>) -> List<&2, Row>:
  match rs:
    case RBody{cs} <> Nil{}:
      [RHead{cs}]
    case _:
      rs

def Md.codeblock.loc(+lc: I.Loc, ls: List<&2, String>) -> Node:
  +text = Md.unlines(ls)
  El{"div", "", [At{"class", "codeblock"}], [Dom.copy.loc(lc, text), El{"pre", "", Nil{}, [El{"code", "", Nil{}, [Txt{text}]}]}]}

# the node a finished block contributes

def Md.codeblock(ls: List<&2, String>) -> Node:
  Md.codeblock.loc(I.En{}, ls)

def Md.close.loc(+lc: I.Loc, b: Blk, out: List<&2, Node>) -> List<&2, Node>:
  match b:
    case BNone{}:
      out
    case BPara{ls}:
      Md.para(ls) <> out
    case BCode{ls}:
      Md.codeblock.loc(lc, ls) <> out
    case BList{xs}:
      El{"ul", "", Nil{}, Md.lis(List.reverse(&2, String, xs))} <> out
    case BOl{xs}:
      El{"ol", "", Nil{}, Md.olis(List.reverse(&2, OItem, xs))} <> out
    case BQuote{ls}:
      El{"blockquote", "", Nil{}, Md.inline(Md.unlines(ls))} <> out
    case BTable{rs}:
      El{"table", "", Nil{}, Md.trs(List.reverse(&2, Row, rs))} <> out

def Md.close(b: Blk, out: List<&2, Node>) -> List<&2, Node>:
  Md.close.loc(I.En{}, b, out)

def Md.head(n: Nat, t: String) -> Node:
  match n:
    case 1n:
      El{"h3", "", Nil{}, Md.inline(t)}
    case 2n:
      El{"h4", "", Nil{}, Md.inline(t)}
    case _:
      El{"h5", "", Nil{}, Md.inline(t)}

def Md.bstep.loc(+lc: I.Loc, b: Blk, ln: Line, +raw: String, out: List<&2, Node>) -> Blk & List<&2, Node>:
  match b ln:
    case BCode{ls} LFence{}:
      (BNone{}, Md.codeblock.loc(lc, ls) <> out)
    case BCode{ls} _:
      (BCode{raw <> ls}, out)
    case _ LFence{}:
      (BCode{Nil{}}, Md.close.loc(lc, b, out))
    case _ LHead{n, t}:
      (BNone{}, Md.head(n, t) <> Md.close.loc(lc, b, out))
    case BList{xs} LItem{t}:
      (BList{t <> xs}, out)
    case _ LItem{t}:
      (BList{[t]}, Md.close.loc(lc, b, out))
    case BOl{xs} LNum{n, t}:
      (BOl{OItem{n, t} <> xs}, out)
    case _ LNum{n, t}:
      (BOl{[OItem{n, t}]}, Md.close.loc(lc, b, out))
    case BQuote{ls} LQuote{t}:
      (BQuote{t <> ls}, out)
    case _ LQuote{t}:
      (BQuote{[t]}, Md.close.loc(lc, b, out))
    case BTable{rs} LRow{cs}:
      (BTable{RBody{cs} <> rs}, out)
    case _ LRow{cs}:
      (BTable{[RBody{cs}]}, Md.close.loc(lc, b, out))
    case BTable{rs} LSep{_}:
      (BTable{Md.headed(rs)}, out)
    case BPara{ls} LSep{t}:
      (BPara{t <> ls}, out)
    case _ LSep{t}:
      (BPara{[t]}, Md.close.loc(lc, b, out))
    case _ LHr{}:
      (BNone{}, El{"hr", "", Nil{}, Nil{}} <> Md.close.loc(lc, b, out))
    case _ LBlank{}:
      (BNone{}, Md.close.loc(lc, b, out))
    case BPara{ls} LText{t}:
      (BPara{t <> ls}, out)
    case _ LText{t}:
      (BPara{[t]}, Md.close.loc(lc, b, out))

def Md.bstep(b: Blk, ln: Line, +raw: String, out: List<&2, Node>) -> Blk & List<&2, Node>:
  Md.bstep.loc(I.En{}, b, ln, raw, out)

def Md.bstep.pair.loc(+lc: I.Loc, +l: String, r: Blk & List<&2, Node>) -> Blk & List<&2, Node>:
  (b, out) = r
  Md.bstep.loc(lc, b, Md.line(l), l, out)

def Md.bstep.pair(+l: String, r: Blk & List<&2, Node>) -> Blk & List<&2, Node>:
  Md.bstep.pair.loc(I.En{}, l, r)

def Md.bfin.loc(+lc: I.Loc, r: Blk & List<&2, Node>) -> List<&2, Node>:
  (b, out) = r
  List.reverse(&2, Node, Md.close.loc(lc, b, out))

def Md.bfin(r: Blk & List<&2, Node>) -> List<&2, Node>:
  Md.bfin.loc(I.En{}, r)

def Md.bgo.loc(+lc: I.Loc, ls: List<&2, String>, r: Blk & List<&2, Node>) -> List<&2, Node>:
  match ls:
    case Nil{}:
      Md.bfin.loc(lc, r)
    case +l <> t:
      Md.bgo.loc(lc, t, Md.bstep.pair.loc(lc, l, r))

# markdown text as nodes. Lines come from the tail-recursive Diff.lines:
# String.lines recurses once per character, and a long reply overflowed
# the phone's JavaScriptCore stack

def Md.bgo(ls: List<&2, String>, r: Blk & List<&2, Node>) -> List<&2, Node>:
  Md.bgo.loc(I.En{}, ls, r)

def Md.render.loc(+lc: I.Loc, s: String) -> List<&2, Node>:
  Md.bgo.loc(lc, DF.Diff.lines(s), (BNone{}, Nil{}))

def Md.render(s: String) -> List<&2, Node>:
  Md.render.loc(I.En{}, s)
