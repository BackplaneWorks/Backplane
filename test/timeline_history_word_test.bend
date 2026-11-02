import Base
import ./check.bend as C
import ../src/web/timeline-history.bend as H

# H.word.go: whether a class list (words split at spaces) holds a word
def w(+s: String, +word: String) -> String: Bool.show(H.H.word.go(s, word, ""))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a word alone", w("live", "live"), "True")
    C.Check.str("a word among others", w("msg live sending", "live"), "True")
    C.Check.str("the last word", w("msg sending", "sending"), "True")
    C.Check.str("a word's part is not the word", w("alive lively", "live"), "False")
    C.Check.str("double spaces", w("a  live  b", "live"), "True")
    C.Check.str("leading and trailing spaces", w(" live ", "live"), "True")
    C.Check.str("an empty list", w("", "live"), "False")
    C.Check.str("non-ASCII words", w("ζ live·x", "live·x"), "True")
