import Base
import ./check.bend as C
import ../src/core/board.bend as B
import ../src/core/plot.bend as P
import ../src/core/utf8.bend as U
import ../src/core/glb.bend as G

def board(+seg: String) -> String:
  "(kicad_pcb (version 20240108)
  (segment (start 10 10) (end 20 10) (width 0.25) (layer \"F.Cu\") (net 1) (uuid \"s1\"))
  (segment (start " ++ seg ++ ") (end 30 30) (width 0.25) (layer \"F.Cu\") (net 1) (uuid \"s2\"))
  (segment (start 5 5) (end 6 6) (width 0.5) (layer \"B.Cu\") (net 1) (uuid \"s3\"))
  (via (at 20 10) (size 0.6) (drill 0.3) (layers \"F.Cu\" \"B.Cu\") (net 1) (uuid \"v1\"))
  (gr_rect (start 0 0) (end 40 40) (stroke (width 0.1) (type default)) (fill none) (layer \"Edge.Cuts\") (uuid \"e1\"))
  (gr_line (start 1 1) (end 2 2) (stroke (width 0.1) (type default)) (layer \"F.CrtYd\") (uuid \"hidden\"))
  (zone (net 1) (net_name \"GND\") (layer \"F.Cu\") (uuid \"z1\")
    (filled_polygon (layer \"F.Cu\") (pts (xy 1 1) (xy 9 1) (xy 9 -4) (xy 1 -4))))
)"

def items(m: Maybe<&2, List<&2, B.Item>>) -> List<&2, B.Item>:
  match m:
    case Some{xs}:
      xs
    case None{}:
      Nil{}

def plot(+seg: String) -> P.Plot:
  P.Plot.of(items(B.Board.items(U.Utf8.encode(board(seg)))), 0)

def chunks(+seg: String) -> List<&2, P.Chunk>:
  P.Plot.chunks(plot(seg))

def strs(xs: List<&2, U32>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      U32.show(x) <> strs(t)

def nums(xs: List<&2, U32>) -> String:
  String.join(strs(xs), ",")

def box(b: P.Box) -> String:
  match b:
    case P.Box{x0, y0, x1, y1}:
      nums([x0, y0, x1, y1])

def bytes(s: String) -> List<&2, U32>:
  match s:
    case SNil{}:
      Nil{}
    case SCon{Chr{c}, t}:
      c <> bytes(t)

def solid(so: P.Solid) -> String:
  match so:
    case P.Solid{n, b, d}:
      U32.show(n) ++ " [" ++ nums(b) ++ "] " ++ nums(P.Dec.nums(String.reverse(d)))

def eqs(xs: List<&2, P.Chunk>, ys: List<&2, P.Chunk>) -> Bool:
  match xs ys:
    case Nil{} Nil{}:
      True{}
    case +x <> xt +y <> yt:
      Bool.and(P.Eq.chunk(x, y), eqs(xt, yt))
    case _ _:
      False{}

def desc(c: P.Chunk) -> String:
  match c:
    case P.Chunk{_, l, _, a, m, w, d, i}:
      U32.show(l) ++ "/" ++ U32.show(m) ++ "/" ++ U32.show(w) ++ ":" ++ nums(P.Dec.nums(String.reverse(d))) ++ " [" ++ String.join(String.split(String.reverse(i), Chr{0}), "|") ++ "]"

def descs(cs: List<&2, P.Chunk>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      desc(c) <> descs(t)

def main() -> IO(Unit):
  do IO<Unit>:
    +ns : List<&2, U32> = [0, 1, 31, 32, 1023, 1024, 123456789, 4294967295]
    C.Check.str("numbers round trip", nums(P.Dec.nums(P.Enc.text(ns))), nums(ns))
    C.Check.str("zigzag", nums([P.Num.zig(0), P.Num.zig(4294967295), P.Num.zig(1), P.Num.zig(4294967294)]), "0,1,2,3")
    C.Check.str("unzig", nums([P.Num.unzig(3), P.Num.unzig(4)]), "4294967294,2")
    C.Check.str("varint bytes", nums(bytes(P.Enc.text([0, 127, 128, 300, 4294967295]))), "0,127,128,1,172,2,255,255,255,255,15")
    +a : List<&2, P.Chunk> = chunks("20 10")
    # paint order: B.Cu (5), then F.Cu (12) fill before strokes (the two
    # tracks in two buckets), Edge.Cuts (22), vias (23), holes (24); the
    # courtyard is hidden, so not there. Numbers: each piece's index in its
    # chunk's info table, then a path's point count and zigzag deltas of
    # micrometres, or a dot's point
    C.Check.str("chunks", String.join(descs(a), " | "),
      "5/1/500:0,2,10000,10000,2000,2000 [track;net=1;layer=B.Cu;width=0.5|] | 12/0/0:0,4,2000,2000,16000,0,0,9999,15999,0 [zone;net=GND;layer=F.Cu|] | 12/1/250:0,2,40000,20000,20000,40000 [track;net=1;layer=F.Cu;width=0.25|] | 12/1/250:0,2,20000,20000,20000,0 [track;net=1;layer=F.Cu;width=0.25|] | 22/1/100:0,5,0,0,80000,0,0,80000,79999,0,0,79999 [gr_rect;layer=Edge.Cuts;width=0.1|] | 23/2/600:0,40000,20000 [via;net=1;size=0.6;drill=0.3|] | 24/2/300:0,40000,20000 [via;net=1;size=0.6;drill=0.3|]")
    C.Check.str("unchanged sends nothing", Nat.show(P.Plot.fresh(P.Plot.delta(a, a))), "0")
    C.Check.str("first plot sends all", Nat.show(P.Plot.fresh(P.Plot.delta(a, Nil{}))), Nat.show(List.length(&2, P.Chunk, a)))
    +b : List<&2, P.Chunk> = chunks("21 10")
    C.Check.str("one edit, one chunk", Nat.show(P.Plot.fresh(P.Plot.delta(b, a))), "1")
    # what a phone asks when it inspects the via (chunk 5, piece 0)
    C.Check.str("info of a tapped piece", P.Plot.info(a, 5n, 0n), "via;net=1;size=0.6;drill=0.3")
    C.Check.str("info of nothing", P.Plot.info(a, 50n, 0n), "")
    # the same through the info file the reading child writes
    C.Check.str("info file", P.Infos.find(P.Infos.file(a), P.Chunk.sig(P.Plot.at(5n, a)), 0n), "via;net=1;size=0.6;drill=0.3")
    C.Check.str("info file, other chunk", P.Infos.find(P.Infos.file(a), P.Chunk.sig(P.Plot.at(0n, a)), 0n), "track;net=1;layer=B.Cu;width=0.5")
    C.Check.str("info file, no such chunk", P.Infos.find(P.Infos.file(a), 12345, 0n), "")
    C.Check.str("delta rebuilds", Bool.show(eqs(P.Plot.apply(P.Plot.delta(b, a), a), b)), "True")
    C.Check.str("message", nums(P.Plot.msg("k", P.Box{0, 4294967295, 7, 8}, 1600, P.Box{1, 2, 3, 4}, [P.Keep{2n}, P.Fresh{P.Chunk{0, 12, 5, 200, 1, 250, "BA", "x"}}])),
      "166,1,100,112,108,111,116,24,31,97,107,99,98,111,120,132,0,32,7,8,101,116,104,105,99,107,25,6,64,100,101,100,103,101,132,1,2,3,4,98,99,115,130,2,167,97,115,0,97,108,12,97,99,5,97,97,24,200,97,109,1,97,119,24,250,97,100,66,65,66")
    # what a child process prints, scanned back by the hub, is the plot
    +pl : P.Plot = plot("20 10")
    +back : P.Plot = P.Scan.plot(P.Line.plot(pl))
    C.Check.str("lines round trip", Bool.show(eqs(P.Plot.chunks(back), P.Plot.chunks(pl))), "True")
    C.Check.str("lines keep the box", box(P.Plot.box(back)), box(P.Plot.box(pl)))
    C.Check.str("box", box(P.Plot.box(pl)), "0,4294963296,40000,40000")
    # the board's rectangle is its outline's alone (the zone below y 0 and
    # the stroke width left out)
    C.Check.str("edge", box(P.Plot.edge(pl)), "0,0,40000,40000")
    C.Check.str("lines keep the edge", box(P.Plot.edge(back)), box(P.Plot.edge(pl)))
    C.Check.str("a schematic has no edge", box(P.Plot.edge(P.Plot.of(items(B.Board.items(U.Utf8.encode(board("20 10")))), 1))), box(P.Box.empty()))
    # a model's triangles: board axes (glTF z is the board's y, glTF y is
    # up), 10 micrometre units; colour XOR then zigzag deltas
    +so : P.Solid = P.Solid.of([G.Tri3{0.001, 0.0016, 0.002, 0.003, 0.0016, 0.002, 0.001, 0.0016, 0.004, 255}])
    C.Check.str("solid", solid(so), "1 [100,200,160,300,400,160] 255,200,400,320,400,0,0,399,400,0")
    C.Check.str("solid lines round trip", solid(P.Plot.solid(P.Scan.plot(P.Line.plot(P.Plot.of.solid(so))))), solid(so))
