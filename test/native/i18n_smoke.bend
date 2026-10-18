import Base
import ../check.bend as C
import ../../src/core/i18n.bend as I
import ../../src/core/msgs.bend as T
import ../fixtures/i18n_fallback.bend as F

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("native Japanese literal", T.T.common.cancel(I.Ja{}), "キャンセル")
    C.Check.str("native Japanese save", T.T.common.save(I.Ja{}), "保存")
    C.Check.str("native English plural", T.T.thread.count(I.En{}, 1n), "1 thread")
    C.Check.str("native Japanese count", T.T.thread.count(I.Ja{}, 1234n), "スレッド1,234件")
    C.Check.str("native exact plural", T.T.files.changed(I.Ja{}, 0n), "変更なし")
    C.Check.str("native select", T.T.turn.by(I.Ja{}, "agent"), "エージェント")
    C.Check.str("native interpolation", T.T.thread.delete.ask(I.Ja{}, "試作機"), "「試作機」を削除しますか？")
    C.Check.str("native literal fallback", F.T.common.save(I.Ja{}), "Save")
    C.Check.str("native plural fallback", F.T.files.count(I.Ja{}, 1n), "1 file")
