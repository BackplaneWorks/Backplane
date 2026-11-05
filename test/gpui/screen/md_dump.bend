import Base
import ../../../src/core/json.bend as J
import ../../../src/web/dom.bend as D
import ../../../src/core/links.bend as LK

# The Bend side of the markdown differential (native/host/src/bin/md-check.rs is the Rust side).
#   md_dump CORPUS.json   a JSON array of strings; prints Mob.nodes(Md.render(s)) as one JSON line each.

# Mob.nodes and what it needs, copied VERBATIM from src/mobile/screen.bend (importing that file would check the whole client);
# test/gpui/screen/md_oracle.sh fails if this block no longer matches screen.bend.
# BEGIN COPY
def Mob.num(n: Bool, +v: String, rest: List<&2, J.KV>) -> List<&2, J.KV>:
  match n:
    case True{}:
      J.KV{"n", J.Json.str(v)} <> rest
    case False{}:
      rest

def Mob.href.attrs(as: List<&2, D.Attr>) -> List<&2, J.KV>:
  match as:
    case Nil{}:
      Nil{}
    case D.At{+k, +v} <> t:
      Bool.pick(List<&2, J.KV>, String.eq(k, "href"), [J.KV{"href", J.Json.str(v)}, J.KV{"file", J.Json.bool(Bool.not(LK.Link.is_web(v)))}], Mob.num(String.eq(k, "value"), v, Mob.href.attrs(t)))
    case _ <> t:
      Mob.href.attrs(t)

# An anchor without href already falls through to plain text on both phones.
def Mob.ref.uri(xs: List<&2, D.Attr>) -> String:
  match xs:
    case Nil{}: ""
    case D.At{"data-ref", v} <> _: v
    case _ <> t: Mob.ref.uri(t)

def Mob.ref.bare(xs: List<&2, D.Attr>) -> Bool:
  match xs:
    case Nil{}: False{}
    case D.At{"data-bare", v} <> _: String.eq(v, "1")
    case _ <> t: Mob.ref.bare(t)

def Mob.href.ref(ref: String, +attrs: List<&2, D.Attr>) -> List<&2, J.KV>:
  match ref:
    case SNil{}: Mob.href.attrs(attrs)
    case SCon{h, t}: [J.KV{"ref", J.Json.str(SCon{h, t})}, J.KV{"bare", J.Json.bool(Mob.ref.bare(attrs))}]

def Mob.href(+attrs: List<&2, D.Attr>) -> List<&2, J.KV>:
  Mob.href.ref(Mob.ref.uri(attrs), attrs)

def Mob.nodes.go(ns: List<&2, D.Node>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match ns:
    case Nil{}:
      acc
    case D.El{tag, _, as, kids} <> t:
      Mob.nodes.go(t, J.Json.obj(J.KV{"tag", J.Json.str(tag)} <> J.KV{"kids", J.Json.arr(List.reverse(&2, J.Json, Mob.nodes.go(kids, Nil{})))} <> Mob.href(as)) <> acc)
    case D.Txt{text} <> t:
      Mob.nodes.go(t, J.Json.obj([J.KV{"text", J.Json.str(text)}]) <> acc)

def Mob.nodes(ns: List<&2, D.Node>) -> List<&2, J.Json>:
  List.reverse(&2, J.Json, Mob.nodes.go(ns, Nil{}))
# END COPY

def Dmp.line(s: String) -> String:
  J.Json.show(J.Json.arr(Mob.nodes(D.Md.render(s))))

def Dmp.each(j: J.Json) -> IO(Unit):
  match j:
    case J.Item{J.Str{s}, t}:
      do IO<Unit>:
        IO.print(Dmp.line(s))
        Dmp.each(t)
    case J.Item{_, t}:
      Dmp.each(t)
    case _:
      IO.pure(Unit, Unit{})

def Dmp.top(j: J.Json) -> IO(Unit):
  match j:
    case J.Arr{items}: Dmp.each(items)
    case _: IO.pure(Unit, Unit{})

def Dmp.read.k(r: File & Result<&1, &1, U32 & String, String>) -> IO(String):
  (f, res) = r
  do IO<String>:
    File.close(f)
    s : String <- IO.pass(String, res)
    return s

def Dmp.read(+path: String) -> IO(String):
  do IO<String>:
    f : File <- IO.try(File, File.open(path, "r"))
    r : File & Result<&1, &1, U32 & String, String> <- File.read(f, 2000000000)
    Dmp.read.k(r)

def Dmp.copy(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: x <> Dmp.copy(t)

def main() -> IO(Unit):
  do IO<Unit>:
    a : List<&1, String> <- IO.args()
    +xs : List<&2, String> = Dmp.copy(a)
    text : String <- Dmp.read(Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, 1n)), ""))
    Dmp.top(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))
