import Base
import ../fixtures/i18n_webphone.bend as F
import ../../src/core/json.bend as J
import ../../src/mobile/screen.bend as S

# Three JSON lines, in home/thread/Settings order. No hub or device needed.
def main() -> IO(Unit):
  do IO<Unit>:
    IO.print(J.Json.show(S.Mob.screen(F.Fixture.ui("ja"))))
    IO.print(J.Json.show(S.Mob.screen(F.Fixture.thread("ja"))))
    IO.print(J.Json.show(S.Mob.screen(F.Fixture.settings("ja"))))
