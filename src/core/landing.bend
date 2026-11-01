import Base

# What a reply to a page request redraws (host.js's landing): the regions
# its keyed delta wrote into, each named with its path in the reply's body
# (the timeline, the top bar), when every path the delta wrote (view-delta.js's
# ops) lies inside one of them or in the window (the page's cursors, never
# drawn); the whole page otherwise. A whole reply wrote everything: its one
# path is the root, inside none, so it always redraws the page (laws
# land_outside_page, land_root_page).

type LRegion is Data:
  LRegion{name: String, path: List<&2, String>}

# xs is the start of ys
def Land.prefix(xs: List<&2, String>, ys: List<&2, String>) -> Bool:
  match xs ys:
    case Nil{} _: True{}
    case +x <> +xt Nil{}: False{}
    case +x <> +xt +y <> +yt: Bool.and(String.eq(x, y), Land.prefix(xt, yt))

def Land.window() -> List<&2, String>:
  ["window"]

def Land.within(rs: List<&2, LRegion>, +p: List<&2, String>) -> Bool:
  match rs:
    case Nil{}: False{}
    case LRegion{_, +path} <> t: Bool.or(Land.prefix(path, p), Land.within(t, p))

# a path a region's own redraw covers: inside one, or in the window
def Land.inside(+rs: List<&2, LRegion>, +p: List<&2, String>) -> Bool:
  Bool.or(Land.prefix(Land.window(), p), Land.within(rs, p))

def Land.all(+rs: List<&2, LRegion>, ps: List<&2, List<&2, String>>) -> Bool:
  match ps:
    case Nil{}: True{}
    case +p <> t: Bool.and(Land.inside(rs, p), Land.all(rs, t))

# whether some path lies inside this region's path
def Land.hit(+path: List<&2, String>, ps: List<&2, List<&2, String>>) -> Bool:
  match ps:
    case Nil{}: False{}
    case +p <> t: Bool.or(Land.prefix(path, p), Land.hit(path, t))

def Land.name.k(hit: Bool, +name: String, acc: String) -> String:
  match hit:
    case True{}: acc ++ name ++ " "
    case False{}: acc

# the regions written into, each name followed by a space
def Land.names(rs: List<&2, LRegion>, +ps: List<&2, List<&2, String>>, acc: String) -> String:
  match rs:
    case Nil{}: acc
    case LRegion{+name, +path} <> t: Land.names(t, ps, Land.name.k(Land.hit(path, ps), name, acc))

def Land.region.k(all: Bool, +rs: List<&2, LRegion>, +ps: List<&2, List<&2, String>>) -> String:
  match all:
    case True{}: Land.names(rs, ps, "")
    case False{}: "page"

# "page", or the regions to redraw ("" when the delta wrote only the window)
def Land.region(+rs: List<&2, LRegion>, +ps: List<&2, List<&2, String>>) -> String:
  Land.region.k(Land.all(rs, ps), rs, ps)

# host.js's entry: a path a line, its steps tab-separated; a region's line
# starts with its name
def Land.steps(s: String, +step: String, acc: List<&2, String>) -> List<&2, String>:
  match s:
    case SNil{}: List.reverse(&2, String, step <> acc)
    case SCon{Chr{9}, t}: Land.steps(t, "", step <> acc)
    case SCon{c, t}: Land.steps(t, step ++ SCon{c, SNil{}}, acc)

def Land.lines(s: String, +line: String, acc: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  match s:
    case SNil{}: List.reverse(&2, List<&2, String>, Land.steps(line, "", Nil{}) <> acc)
    case SCon{Chr{10}, t}: Land.lines(t, "", Land.steps(line, "", Nil{}) <> acc)
    case SCon{c, t}: Land.lines(t, line ++ SCon{c, SNil{}}, acc)

# (no paths: a delta that wrote nothing; a whole reply passes one empty
# line, the root)
def Land.paths(paths: String) -> List<&2, List<&2, String>>:
  match paths:
    case SNil{}: Nil{}
    case SCon{c, t}: Land.lines(SCon{c, t}, "", Nil{})

def Land.region.of(xs: List<&2, String>) -> List<&2, LRegion>:
  match xs:
    case Nil{}: Nil{}
    case +name <> +path: [LRegion{name, path}]

def Land.regions(ls: List<&2, List<&2, String>>, acc: List<&2, LRegion>) -> List<&2, LRegion>:
  match ls:
    case Nil{}: List.reverse(&2, LRegion, acc)
    case +l <> t: Land.regions(t, List.append(&2, LRegion, List.reverse(&2, LRegion, Land.region.of(l)), acc))

def Land.of(+regions: String, +paths: String) -> String:
  Land.region(Land.regions(Land.paths(regions), Nil{}), Land.paths(paths))
