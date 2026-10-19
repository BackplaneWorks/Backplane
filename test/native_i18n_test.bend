import Base
import ./check.bend as C
import ../src/mobile/app.bend as A
import ../src/mobile/hubs.bend as H
import ../src/core/client.bend as U
import ../src/core/json.bend as J

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("native English override", A.native_locale("en", "ja-JP", "1"), "en")
    C.Check.str("native Japanese preview", A.native_locale("ja", "en-US", "1"), "ja")
    C.Check.str("native follows OS", A.native_locale("auto", "ja_JP.UTF-8", "1"), "ja")
    C.Check.str("native unknown language fallback", A.native_locale("auto", "de-DE", "1"), "en")
    C.Check.str("native shares preview gate", A.native_locale("ja", "ja-JP", ""), U.Lang.resolve("ja", "ja-JP", False{}))
    C.Check.str("unpaired Japanese screen", J.Json.get_str(A.screen_list_in(H.Hubs.init(0n, "fixture"), "ja"), "empty", "?"), "ハブとペアリングしてください。")
    C.Check.str("unpaired English screen", J.Json.get_str(A.screen_list_in(H.Hubs.init(0n, "fixture"), "en"), "empty", "?"), "Pair with a hub to start.")
