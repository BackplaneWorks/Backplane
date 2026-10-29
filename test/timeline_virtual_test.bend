import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/timeline-virtual.bend as V

def parsed(raw: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(raw), J.Null{})

def num(name: String, +got: V.Px, +want: V.Px) -> IO(Unit):
  C.Check.str(name, Bool.show(Bool.and(V.Px.le(got, want), V.Px.le(want, got))), "True")

def field(name: String, got: J.Json, key: String, want: String) -> IO(Unit):
  num(name, V.Px.get(got, key, "0"), V.Px.read(want))

def request(+keys: J.Json, +sizes: J.Json, previous: J.Json, top: String, mode: String, follow: Bool) -> J.Json:
  J.Json.obj([J.KV{"keys", keys}, J.KV{"sizes", sizes}, J.KV{"previous", previous},
    J.KV{"top", J.Num{top}}, J.KV{"view", J.Num{"100"}}, J.KV{"mode", J.Str{mode}}, J.KV{"follow", J.Flag{follow}}])

def view(request: J.Json, size: String) -> J.Json:
  match request:
    case J.Obj{fs}: J.Obj{J.Field{"view", J.Num{size}, fs}}
    case j: j

def readback(previous: J.Json, top: String) -> J.Json:
  V.plan(J.Json.obj([J.KV{"mode", J.Str{"readback"}}, J.KV{"previous", previous}, J.KV{"top", J.Num{top}}]))

def main() -> IO(Unit):
  do IO<Unit>:
    num("decimal beyond float32 precision", V.Px.add(V.Px.read("16777216.015625"), V.Px.read("0.015625")), V.Px.read("16777216.03125"))
    num("fractional signed cancellation", V.Px.sub(V.Px.read("123.0625"), V.Px.read("123.125")), V.Px.read("-0.0625"))
    num("center half exact", V.Px.half(V.Px.read("-123.125")), V.Px.read("-61.5625"))
    num("exponent input", V.Px.read("1.5625e-2"), V.Px.read("0.015625"))
    +keys : J.Json = parsed("[\"a\",\"b\",\"c\",\"d\",\"e\"]")
    +sizes : J.Json = parsed("{\"x\":90.375,\"a\":80.125,\"b\":80.125,\"c\":80.125,\"d\":80.125,\"e\":80.125}")
    +old : J.Json = V.plan(request(keys, sizes, J.Null{}, "150.25", "scroll", False{}))
    field("all measured geometry", old, "total", "464.625")
    field("scroll keeps DOM top", old, "top", "150.25")
    field("captured signed offset", old, "offset", "44.125")
    +prepended : J.Json = V.plan(request(parsed("[\"x\",\"a\",\"b\",\"c\",\"d\",\"e\"]"), sizes, old, "150.25", "update", True{}))
    field("prepend restores keyed offset", prepended, "top", "250.625")
    field("prepend same offset", prepended, "offset", "44.125")
    +moved : J.Json = V.plan(request(parsed("[\"x\",\"a\",\"b\",\"c\",\"d\",\"e\"]"), sizes, old, "170.5", "update", False{}))
    field("captures current DOM top rather than previous plan top", moved, "top", "270.875")
    +latest : J.Json = V.plan(request(keys, sizes, old, "150.25", "bottom", True{}))
    field("explicit bottom", latest, "top", "364.625")
    +larger : J.Json = parsed("{\"a\":80.125,\"b\":80.125,\"c\":80.125,\"d\":80.125,\"e\":120.375}")
    field("stream growth follows while pinned", V.plan(request(keys, larger, latest, "364.625", "measure", True{})), "top", "404.875")
    field("stream growth stays put while reading", V.plan(request(keys, larger, old, "150.25", "measure", True{})), "top", "150.25")
    field("disabled follow preserves anchor", V.plan(request(keys, larger, latest, "364.625", "measure", False{})), "top", "364.625")
    field("init starts at end", V.plan(request(keys, sizes, J.Null{}, "0", "init", True{})), "top", "364.625")
    field("previous viewport decides pinned follow", V.plan(view(request(keys, sizes, latest, "364.625", "measure", True{}), "50")), "top", "414.625")
    +withControl : J.Json = parsed("[\"control:work\",\"a\",\"b\",\"c\",\"d\",\"e\"]")
    +controlSizes : J.Json = parsed("{\"control:work\":0,\"x\":90.375,\"a\":80.125,\"b\":80.125,\"c\":80.125,\"d\":80.125,\"e\":80.125}")
    +controlOld : J.Json = V.plan(request(withControl, controlSizes, J.Null{}, "0", "scroll", False{}))
    C.Check.str("zero-height leading control is not anchor", J.Json.get_str(controlOld, "anchor", ""), "a")
    field("prepend past empty control preserves visible message", V.plan(request(parsed("[\"control:work\",\"x\",\"a\",\"b\",\"c\",\"d\",\"e\"]"), controlSizes, controlOld, "0", "update", False{})), "top", "100.375")
    +positiveControlSizes : J.Json = parsed("{\"control:work\":24.125,\"x\":90.375,\"a\":80.125,\"b\":80.125,\"c\":80.125,\"d\":80.125,\"e\":80.125}")
    +positiveControlOld : J.Json = V.plan(request(withControl, positiveControlSizes, J.Null{}, "0", "scroll", False{}))
    C.Check.str("positive control header is not anchor", J.Json.get_str(positiveControlOld, "anchor", ""), "a")
    field("prepend below positive header preserves visible message", V.plan(request(parsed("[\"control:work\",\"x\",\"a\",\"b\",\"c\",\"d\",\"e\"]"), positiveControlSizes, positiveControlOld, "0", "update", False{})), "top", "100.375")
    +chainSizes : J.Json = parsed("{\"control:work\":11,\"a\":80.125,\"b\":80.125,\"c\":80.125,\"d\":80.125,\"e\":80.125}")
    +chainOld : J.Json = V.plan(request(withControl, chainSizes, J.Null{}, "0", "scroll", False{}))
    field("smoke fixture starts message 37px below viewport", chainOld, "offset", "-37")
    +chainKeys : J.Json = parsed("[\"control:work\",\"x\",\"a\",\"b\",\"c\",\"d\",\"e\"]")
    +estimated : J.Json = V.plan(request(chainKeys, chainSizes, chainOld, "0", "update", False{}))
    C.Check.str("estimated prepend retains original reading anchor", J.Json.get_str(estimated, "anchor", ""), "a")
    field("estimated prepend keeps original signed offset", estimated, "offset", "-37")
    +measuredSizes : J.Json = parsed("{\"control:work\":11,\"x\":39.5,\"a\":80.125,\"b\":80.125,\"c\":80.125,\"d\":80.125,\"e\":80.125}")
    +measured : J.Json = V.plan(request(chainKeys, measuredSizes, estimated, "130", "measure", False{}))
    C.Check.str("measurement chain retains original key", J.Json.get_str(measured, "anchor", ""), "a")
    field("120 to 39.5 measurement preserves original message offset", measured, "offset", "-37")
    field("measurement compensates 80.5px without drift", measured, "top", "49.5")
    +echo : J.Json = V.plan(request(chainKeys, chainSizes, estimated, "130", "scroll", False{}))
    C.Check.str("own scroll-write echo keeps reading anchor", J.Json.get_str(echo, "anchor", ""), "a")
    +scrolled : J.Json = V.plan(request(chainKeys, chainSizes, estimated, "131", "scroll", False{}))
    C.Check.str("scroll mode deliberately captures fresh visible row", J.Json.get_str(scrolled, "anchor", ""), "x")
    +outside : J.Json = V.plan(request(withControl, chainSizes, chainOld, "300", "update", False{}))
    C.Check.str("user motion beyond old anchor captures fresh key", J.Json.get_str(outside, "anchor", ""), "d")
    +restored : J.Json = V.plan(parsed("{\"keys\":[\"a\",\"b\"],\"view\":100,\"sizes\":{\"a\":120.125,\"b\":80.125},\"mode\":\"restore\",\"bookmark\":{\"anchor\":\"b\",\"offset\":-5.5,\"top\":0,\"atEnd\":false}}"))
    field("restore signed leading-gap offset", restored, "top", "140.625")
    field("restore carries signed offset", restored, "offset", "-5.5")
    +center : J.Json = V.plan(parsed("{\"keys\":[\"a\",\"b\",\"c\"],\"view\":100,\"mode\":\"key\",\"key\":\"b\",\"align\":\"center\",\"offset\":0.125}"))
    field("unmounted key center", center, "top", "156.125")
    +roundedOld : J.Json = readback(old, "150")
    field("readback retains logical fractional position", roundedOld, "logicalTop", "150.25")
    field("negative residual normalizes to subpixel translation", roundedOld, "trackOffset", "0.75")
    field("normalized readback requests second integer write", roundedOld, "top", "151")
    field("readback preserves signed reading offset", roundedOld, "offset", "44.125")
    +settledReadback : J.Json = readback(roundedOld, "151")
    field("second readback settles without residual growth", settledReadback, "trackOffset", "0.75")
    +shiftedScroll : J.Json = V.plan(request(keys, sizes, settledReadback, "152", "scroll", False{}))
    field("physical user scroll converts to logical geometry", shiftedScroll, "logicalTop", "151.25")
    field("physical user scroll carries normalized translation", shiftedScroll, "trackOffset", "0.75")
    +roundKeys : J.Json = parsed("[\"x\",\"a\",\"b\",\"c\",\"d\",\"e\"]")
    +roundPrepend : J.Json = V.plan(request(roundKeys, sizes, settledReadback, "151", "update", False{}))
    field("prepend compensates using logical rather than physical top", roundPrepend, "logicalTop", "250.625")
    +roundPrependBack : J.Json = readback(roundPrepend, "251")
    field("repeated fractional readback preserves original anchor offset", roundPrependBack, "offset", "44.125")
    field("repeated residual remains less than one pixel", roundPrependBack, "trackOffset", "0.375")
    +roundMeasure : J.Json = V.plan(request(roundKeys, measuredSizes, roundPrependBack, "251", "measure", False{}))
    +roundMeasureBack : J.Json = readback(roundMeasure, "200")
    field("measurement after prior readback keeps fractional logical top", roundMeasureBack, "logicalTop", "199.75")
    field("measurement readback does not accumulate half-pixel anchor drift", roundMeasureBack, "offset", "44.125")
    field("measurement readback normalized translation", roundMeasureBack, "trackOffset", "0.25")
    +capped : J.Json = V.plan(J.Json.obj([J.KV{"mode", J.Str{"readback"}}, J.KV{"previous", settledReadback}, J.KV{"top", J.Num{"149"}}, J.KV{"readbackAttempt", J.Json.nat(4n)}]))
    field("fourth readback settles at actual clamped physical position", capped, "top", "149")
    field("fourth readback raw correction retains logical position", capped, "logicalTop", "150.25")
    field("fourth readback accepts raw translation to stop writes", capped, "trackOffset", "-1.25")
    C.Check.str("anchor forced into window", Bool.show(V.Window.keep(True{}, False{})), "True")
    C.Check.str("only bottom can follow", Bool.show(V.Follow.allowed(True{}, False{})), "False")
