import Base
import ./check.bend as C
import ../src/core/voice.bend as V
import ../src/core/vdict.bend as D

def us(xs: List<&2, V.Unsure>) -> String:
  match xs:
    case Nil{}:
      ""
    case V.Unsure{w, g} <> t:
      w ++ "=" ++ g ++ ";" ++ us(t)

def fxs(xs: List<&2, D.VFx>) -> String:
  match xs:
    case Nil{}:
      ""
    case D.VFx{h, o} <> t:
      h ++ ">" ++ o ++ ";" ++ fxs(t)

def dict() -> D.VDict:
  D.Vdc.of(["KiCad = key cad", "STM32", "nRF52840", "SPI", "ESP32", "jason -> JSON", "!um", "# a comment", ""],
    [D.VPair{"backplain", "Backplane\t2\t0"}, D.VPair{"bend", "Bend\t1\t0"}, D.VPair{"kick ad", "KiCad\t3\t1"}, D.VPair{"differential pair", "diff pair\t2\t0"}])

# words with a probability and no timings
def w(+s: String, +p: Nat) -> D.VW:
  D.VW{s, p, 0n, 0n}

def sure(+s: String) -> List<&2, D.VW>:
  D.Vdc.words.text(s)

def fix(+ws: List<&2, D.VW>) -> String:
  D.Vdc.text(D.VFix.words(D.Vdc.fix(dict(), ws)))

def fixed(+ws: List<&2, D.VW>) -> String:
  fxs(D.VFix.fixes(D.Vdc.fix(dict(), ws)))

def asked(+ws: List<&2, D.VW>) -> String:
  us(D.VFix.ask(D.Vdc.fix(dict(), ws)))

# timed words: text, start, end, percent
def t(+s: String, +a: Nat, +b: Nat, +p: Nat) -> D.VW:
  D.VW{s, p, a, b}

def cm(+x: D.VStep) -> String:
  D.Vdc.text(D.VStep.commit(x)) ++ "|" ++ D.Vdc.text(D.VStep.tail(x))

def h1() -> List<&2, D.VW>:
  [t("Open", 0n, 24n, 95n), t("the", 24n, 32n, 100n), t("board.", 32n, 56n, 90n)]

def h2() -> List<&2, D.VW>:
  [t("Open", 0n, 24n, 95n), t("the", 32n, 40n, 100n), t("board", 40n, 64n, 100n), t("in", 64n, 80n, 85n), t("Kai", 80n, 104n, 72n), t("Kat.", 112n, 136n, 29n)]

def h3() -> List<&2, D.VW>:
  [t("Open", 0n, 40n, 98n), t("the", 40n, 48n, 100n), t("board", 48n, 72n, 100n), t("in", 72n, 88n, 99n), t("Kai", 88n, 112n, 57n), t("Cad", 120n, 136n, 53n), t("and", 144n, 168n, 100n)]

def h4() -> List<&2, D.VW>:
  [t("Open", 0n, 40n, 99n), t("the", 40n, 48n, 100n), t("board", 48n, 72n, 100n), t("in", 72n, 88n, 100n), t("Kai", 88n, 112n, 82n), t("CAD", 120n, 152n, 59n), t("and", 152n, 160n, 100n), t("run", 160n, 168n, 100n), t("the", 168n, 176n, 100n)]

def h5() -> List<&2, D.VW>:
  [t("Open", 0n, 40n, 99n), t("the", 40n, 48n, 100n), t("board", 48n, 72n, 100n), t("in", 72n, 88n, 100n), t("Kai", 88n, 128n, 74n), t("CAD", 128n, 160n, 62n), t("and", 160n, 168n, 100n), t("run", 168n, 184n, 100n), t("the", 184n, 192n, 100n), t("design", 192n, 230n, 100n)]

def hp() -> List<&2, D.VW>:
  [t("the", 0n, 20n, 100n), t("differential", 20n, 60n, 100n), t("pair", 60n, 80n, 100n)]

def s1() -> D.VSet:
  D.VStep.st(D.Vs.hyp(True{}, D.Vs.empty(), "a", h1(), 60n, Nil{}))

def s2() -> D.VSet:
  D.VStep.st(D.Vs.hyp(True{}, s1(), "a", h2(), 140n, Nil{}))

def s3() -> D.VSet:
  D.VStep.st(D.Vs.hyp(True{}, s2(), "a", h3(), 170n, Nil{}))

def s4() -> D.VSet:
  D.VStep.st(D.Vs.hyp(True{}, s3(), "a", h4(), 200n, Nil{}))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("key", D.Vdc.key("kicad") ++ " " ++ D.Vdc.key("kaicad") ++ " " ++ D.Vdc.key("keycad") ++ " " ++ D.Vdc.key("spi") ++ " " ++ D.Vdc.key("sbi") ++ " " ++ D.Vdc.key("esp32") ++ " " ++ D.Vdc.key("asp32"), "kt kt kt sp sp asp32 asp32")
    C.Check.str("ekey", D.Vdc.ekey("KiCad = key cad") ++ " " ++ D.Vdc.ekey(" Jason  -> JSON") ++ " " ++ D.Vdc.ekey("!Um"), "kicad jason> !um")
    C.Check.str("fix: nothing to fix", fix(sure("Open the board and run the check.")), "Open the board and run the check.")
    C.Check.str("fix: how it sounds", fix(sure("Open the board in key cad now.")), "Open the board in KiCad now.")
    C.Check.str("fix: spelled another way", fix(sure("The NRF52840 and the stm32.")), "The nRF52840 and the STM32.")
    C.Check.str("fix: alone", fix(sure("NRF52840")) ++ "/" ++ fix(sure("the NRF52840")) ++ "/" ++ fix(sure("NRF52840 and")), "nRF52840/the nRF52840/nRF52840 and")
    C.Check.str("fix: spaced", fix(sure("an stm 32 board")), "an STM32 board")
    C.Check.str("fix: spaced part number", fix(sure("the NRF 52840 over SPI")), "the nRF52840 over SPI")
    C.Check.str("settle: a part prefix waits for its digits", cm(D.Vs.hyp(False{}, D.VStep.st(D.Vs.hyp(False{}, D.Vs.empty(), "p", sure("the NRF 52840 over"), 0n, D.Vdc.waits(dict()))), "p", sure("the NRF 52840 over"), 0n, D.Vdc.waits(dict()))), "the|NRF 52840 over")
    C.Check.str("fix: replacement keeps punctuation", fix(sure("send the jason, please")), "send the JSON, please")
    C.Check.str("fix: a capital stays", fix(sure("Jason is fine")), "JSON is fine")
    C.Check.str("fix: drop word", fix(sure("so um, the board")), "so, the board")
    C.Check.str("fix: drop at the start", fix(sure("Um the board")), "the board")
    C.Check.str("fix: learned and trusted", fix(sure("open backplain")), "open Backplane")
    C.Check.str("fix: learned once is a guess", fix(sure("the bend thing")) ++ " / " ++ asked(sure("the bend thing")), "the bend thing / bend=Bend;")
    C.Check.str("fix: learned then undone is a guess", asked(sure("a kick ad file")), "kick ad=KiCad;")
    C.Check.str("fix: sounds alike, engine unsure", fix([w("over", 100n), w("SBI", 70n), w("at", 100n)]), "over SPI at")
    C.Check.str("fix: sounds alike, engine sure, a plain word", fix([w("the", 100n), w("spy", 99n)]) ++ " / " ++ asked([w("the", 100n), w("spy", 99n)]), "the spy / spy=SPI;")
    C.Check.str("fix: two words that sound like a term", fix([w("in", 100n), w("Kai", 72n), w("CAD", 59n), w("and", 100n)]), "in KiCad and")
    C.Check.str("fix: near the ESP32", fix([w("the", 100n), w("ASP32.", 80n)]), "the ESP32.")
    C.Check.str("fix: not a near miss", fix([w("the", 100n), w("kid", 40n)]), "the kid")
    C.Check.str("fixes listed", fixed(sure("um the key cad and jason")), "um>;key cad>KiCad;jason>JSON;")
    C.Check.str("asks: unsure words", us(D.Vdc.asks(dict(), Nil{}, D.Vdc.fix(dict(), [w("the", 100n), w("role", 40n), w("R12", 100n)]))), "role=;R12=;")
    C.Check.str("learn: first", D.Vdc.learn("", "KiCad", True{}), "KiCad\t1\t0")
    C.Check.str("learn: again", D.Vdc.learn("KiCad\t1\t0", "KiCad", True{}), "KiCad\t2\t0")
    C.Check.str("learn: undone", D.Vdc.learn("KiCad\t2\t0", "KiCad", False{}), "KiCad\t2\t1")
    C.Check.str("learn: another word starts over", D.Vdc.learn("KiCad\t2\t1", "Kicad", True{}), "Kicad\t1\t0")
    C.Check.str("settle: first hypothesis commits nothing", cm(D.Vs.hyp(True{}, D.Vs.empty(), "a", h1(), 60n, Nil{})), "|Open the board.")
    C.Check.str("settle: agreed words commit", cm(D.Vs.hyp(True{}, s1(), "a", h2(), 140n, Nil{})), "Open the|board in Kai Kat.")
    C.Check.str("settle: unsure words wait", cm(D.Vs.hyp(True{}, s2(), "a", h3(), 170n, Nil{})), "board in|Kai Cad and")
    C.Check.str("settle: committed words never come back", cm(D.Vs.hyp(True{}, s3(), "a", h4(), 200n, Nil{})), "|Kai CAD and run the")
    C.Check.str("settle: unsure words go with the next sure one", cm(D.Vs.hyp(True{}, s4(), "a", h5(), 240n, Nil{})), "Kai CAD and run the|design")
    C.Check.str("settle: done commits the rest", cm(D.Vs.done(True{}, s4(), "a", h5())), "Kai CAD and run the design|")
    C.Check.str("settle: a new item starts over", cm(D.Vs.done(True{}, s4(), "b", h1())), "Open the board.|")
    C.Check.str("settle untimed: two words stay open", cm(D.Vs.hyp(False{}, D.VStep.st(D.Vs.hyp(False{}, D.Vs.empty(), "c", sure("one two three"), 0n, Nil{})), "c", sure("one two three four"), 0n, Nil{})), "one two|three four")
    C.Check.str("settle: a phrase waits whole", cm(D.Vs.hyp(True{}, D.VStep.st(D.Vs.hyp(True{}, D.Vs.empty(), "p", hp(), 200n, D.Vdc.waits(dict()))), "p", hp(), 200n, D.Vdc.waits(dict()))), "the|differential pair")
    C.Check.str("fix: a technical word one sound from a term", fix([w("over", 100n), w("ASP32", 99n)]), "over ESP32")
    C.Check.str("settle untimed: done", cm(D.Vs.done(False{}, D.VStep.st(D.Vs.hyp(False{}, D.VStep.st(D.Vs.hyp(False{}, D.Vs.empty(), "c", sure("one two three"), 0n, Nil{})), "c", sure("one two three four"), 0n, Nil{})), "c", sure("one two three four five."))), "three four five.|")
