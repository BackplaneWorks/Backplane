import Base
import ./check.bend as C
import ../src/core/search-fold.bend as SF
import ../src/core/fuzzy.bend as FZ
import ../src/core/thin.bend as T
import ../src/core/hits.bend as H

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("case and canonical accents", SF.Fold.text("CAFÉ cafe\u{301}"), "cafe cafe")
    C.Check.str("Greek case, accent and final sigma", SF.Fold.text("ΔΟΚΙΜΉ δοκιμή ς Σ"), "δοκιμη δοκιμη σ σ")
    C.Check.str("full case folding and compatibility", SF.Fold.text("Straße ＣＡＦＥ ﬃ"), "strasse cafe ffi")
    C.Check.str("Hangul syllable and jamo", SF.Fold.text("각"), SF.Fold.text("각"))
    C.Check.str("Unicode project filter", Bool.show(Nat.is_gt(FZ.Fuzzy.score("Δοκιμή café", "ΔΟΚΙ"), 0n)), "True")
    C.Check.str("composed indexed word", Bool.show(T.Word.has(T.Words.of("CAFÉ"), "cafe")), "True")
    C.Check.str("decomposed indexed word", Bool.show(T.Word.has(T.Words.of("cafe\u{301}"), "cafe")), "True")
    C.Check.str("ASCII camel score stays", Nat.show(FZ.Fuzzy.score("BackPlane", "bp")), Nat.show(FZ.Fuzzy.normalized("BackPlane", "bp")))
