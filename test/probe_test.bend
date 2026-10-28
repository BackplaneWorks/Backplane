import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/hier.bend as H
import ../src/core/utf8.bend as T

# Cross-probing's client side (client.bend's Probe): the chip's words, the
# part a probe names, and the parts each sheet's file places (hier.bend's
# Hier.refs) that say which sheet a part is on.

def sheet() -> String:
  "(kicad_sch (lib_symbols (symbol \"Device:R\" (property \"Reference\" \"R\" (at 0 0 0))))
  (symbol (lib_id \"Device:R\") (at 1 1 0) (property \"Reference\" \"R12\" (at 1 1 0)))
  (symbol (lib_id \"Device:R\") (at 2 2 0) (property \"Reference\" \"R?\" (at 1 1 0)))
  (symbol (lib_id \"power:GND\") (at 3 3 0) (property \"Reference\" \"#PWR01\" (at 1 1 0)))
  (symbol (lib_id \"Amp:LM358\") (at 4 4 0) (property \"Reference\" \"U3\" (at 1 1 0))))"

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a part's chip is its ref", U.Probe.label("c\tR12"), "R12")
    C.Check.str("a net's chip is its name", U.Probe.label("n\t/Power/VBUS\tR1.2 C3.1"), "Net /Power/VBUS")
    C.Check.str("a net not named yet: a pin of it", U.Probe.label("n\t\tU1.3 U1.4"), "Net U1.3")
    C.Check.str("an unlabelled net says so itself", U.Probe.label("n\tNet-(Q1-G)\tQ1.1"), "Net-(Q1-G)")
    C.Check.str("nothing probed, no chip", U.Probe.label(""), "")
    C.Check.str("a part probe names its ref", U.Probe.ref("c\tU3"), "U3")
    C.Check.str("a net probe names none", U.Probe.ref("n\tGND\t"), "")
    C.Check.str("the parts a sheet places", String.join(H.Hier.refs(T.Utf8.encode(sheet())), " "), "R12 U3")
    C.Check.str("a line per file, as the boxes'", H.Hier.refs.line("/p/a.kicad_sch", T.Utf8.encode(sheet())), "/p/a.kicad_sch\tR12\tU3")
    return Unit{}
