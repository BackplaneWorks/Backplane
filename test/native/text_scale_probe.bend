import Base
import ../../src/server/argv.bend as AV
import ../../src/core/ref-origin.bend as O
import ../../src/core/links.bend as LK
import ../../src/core/images.bend as IM
import ../../src/core/refs.bend as RF
import ../../src/core/secret.bend as SEC
import ../../src/core/notice.bend as NT
import ../../src/core/hits.bend as HT
import ../../src/core/thin.bend as T
import ../../src/core/json.bend as J
import ../../src/core/ring.bend as RG
import ../../src/core/voice.bend as VC
import ../../src/core/attach.bend as AT
import ../../src/core/model.bend as M
import ../../src/web/dom.bend as D

# `text_scale_probe FN REPS`: one per-text function on a message-like text
# of REPS chunks (about 100 bytes each). Linear work takes 4x the time for
# 4x the text; a scan that rebuilds the rest of the text, or reads the head
# of a rope it grows, at every character takes 16x (Origin.scan did).
# Run: bend test/native/text_scale_probe.bend FN REPS

def chunk() -> String:
  "See `code` at backplane://thread/t and https://x.io/a.png @bob >t1 %p #r src/a.bend:12 **b** sk-abc done\n"

def text.go(n: Nat, acc: String) -> String:
  match n:
    case 0n: acc
    case 1n+k: text.go(k, chunk() ++ acc)

def run(f: String, +s: String) -> String:
  match f:
    case "origin": Nat.show(String.length(O.Origin.show(s, "peer")))
    case "links": Nat.show(List.length(&2, LK.Piece, LK.Link.pieces(s)))
    case "imgpaths": Nat.show(List.length(&2, String, IM.Img.paths(s)))
    case "md": Nat.show(List.length(&2, D.Node, D.Md.render(s)))
    case "refwords": Nat.show(List.length(&2, String, RF.Refs.sigil.words(s)))
    case "refmaybe": Bool.show(RF.Refs.maybe(s))
    case "reffound": Nat.show(List.length(&2, RF.Hit, RF.Refs.found(M.State.empty(), "", s)))
    case "refagent": Nat.show(String.length(RF.Refs.agent(M.State.empty(), "", s)))
    case "secmask": Nat.show(String.length(SEC.Sec.mask(SEC.Sec.vault(["sk-abc-secret-value"]), s)))
    case "secclean": Bool.show(SEC.Sec.clean(SEC.Sec.vault(["sk-abc-secret-value"]), s))
    case "alert": Nat.show(String.length(NT.Alert.clean(s)))
    case "snip": Nat.show(List.length(&2, String, HT.Snip.of(T.Words.of("bob done"), s)))
    case "jsonshow": Nat.show(String.length(J.Json.show(J.Str{s})))
    case "jsonparse": Nat.show(String.length(J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(J.Json.show(J.Json.obj([J.KV{"t", J.Str{s}}]))), J.Null{}), "t", "")))
    case "ring": Nat.show(String.length(RG.Ring.until(s, "zz")))
    case "voice": Nat.show(String.length(VC.Voice.fix(s, "bob", "rob")))
    case "attach": Nat.show(String.length(AT.Att.clean(s)))
    case "lines": Nat.show(List.length(&2, String, String.lines(s)))
    case _: "fn: origin links imgpaths md refwords refmaybe reffound refagent secmask secclean alert snip jsonshow jsonparse ring voice attach lines"

def probe.k(xs: List<String>) -> IO(Unit):
  match xs:
    case +f <> reps <> _:
      IO.print(f ++ " " ++ run(f, text.go(Maybe.default(&2, Nat, Nat.read(reps), 1n), "")))
    case _:
      IO.print("usage: text_scale_probe FN REPS")

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, AV.Argv.args(), probe.k)
