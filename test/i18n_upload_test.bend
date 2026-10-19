import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/i18n.bend as I
import ../src/core/json.bend as J
import ../src/core/winnote.bend as NT

# The held error is the same value across locale switches; no reply is repeated.
def error.raw(u: U.Ui) -> String:
  match u:
    case U.Ui{_, _, _, _, _, _, _, _, _, _, _, _, e, _, _, _, _}: e

def locale(u: U.Ui, +tag: String) -> U.Ui:
  U.Lang.apply(U.Ui.scratch_set(u, "@lang.preview", "1"), tag, tag)

def initial(tag: String) -> U.Ui:
  locale(U.Act.ui(U.Ui.select(U.Ui.init(0n), "t1")), tag)

def failure(tag: String, payload: String) -> U.Ui:
  U.Act.ui(U.Ui.act(initial(tag), "attach-end", payload))

def shown(+u: U.Ui) -> String:
  U.Ui.error.shown(U.Lang.loc(u), error.raw(u))

def held() -> U.Ui:
  failure("en", J.Json.show(NT.Notice.upload.reply("h:1|t1", "upload.network", "")))

def cap(tag: String, action: String) -> U.Ui:
  U.Act.ui(U.Ui.act(initial(tag), action, "{\"name\":\"raw {name}.png\",\"size\":99999999,\"i\":0}"))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("held upload en", shown(held()), "the upload did not reach the hub")
    C.Check.str("held upload en to ja", shown(locale(held(), "ja")), "アップロードがハブに届きませんでした")
    C.Check.str("held upload en ja en", shown(locale(locale(held(), "ja"), "en")), "the upload did not reach the hub")
    C.Check.str("local producer ID ja", shown(failure("ja", "{\"uiError\":\"upload.network\",\"thread\":\"t1\",\"upload\":true}")), "アップロードがハブに届きませんでした")
    C.Check.str("unknown ID keeps raw", shown(failure("ja", "{\"uiError\":\"viewer.loading\",\"text\":\"raw {text} 日本語\",\"upload\":true}")), "raw {text} 日本語")
    C.Check.str("raw English is not a lookup key", shown(failure("ja", "{\"text\":\"the upload failed\",\"upload\":true}")), "the upload failed")
    C.Check.str("malformed reply en", shown(failure("en", "not json")), "the upload failed")
    C.Check.str("malformed reply ja", shown(failure("ja", "not json")), "アップロードに失敗しました")
    C.Check.str("null reply ja", shown(failure("ja", "null")), "アップロードに失敗しました")
    C.Check.str("array reply ja", shown(failure("ja", "[]")), "アップロードに失敗しました")
    C.Check.str("number reply ja", shown(failure("ja", "42")), "アップロードに失敗しました")
    C.Check.str("missing error body ja", shown(failure("ja", "{\"upload\":true}")), "アップロードに失敗しました")
    C.Check.str("incomplete reply ja", shown(failure("ja", J.Json.show(NT.Notice.upload.reply("t1", "upload.incomplete", "")))), "アップロードが途中で途切れました")
    C.Check.str("HTTP cap old client fallback", J.Json.get_str(NT.Notice.upload.reply("h:1|t1", "upload.cap", "10"), "text", ""), "attachments are limited to 10 MB")
    C.Check.str("HTTP reply keeps qualified thread", J.Json.get_str(NT.Notice.upload.reply("h:1|t1", "upload.cap", "10"), "thread", ""), "h:1|t1")
    C.Check.str("HTTP cap ja", shown(failure("ja", J.Json.show(NT.Notice.upload.reply("t1", "upload.cap", "10")))), "添付ファイルは10 MBまでです")
    C.Check.str("HTTP cap raw detail", U.Ui.error.shown(I.Ja{}, NT.Notice.upload("upload.cap", "1200", "")), "添付ファイルは1200 MBまでです")
    C.Check.str("phone cap en", shown(cap("en", "attach-up")), "attachments are limited to 10 MB")
    C.Check.str("phone cap ja", shown(cap("ja", "attach-up")), "添付ファイルは10 MBまでです")
    C.Check.str("piece cap ja", shown(cap("ja", "attach")), "添付ファイルは10 MBまでです")
    C.Check.str("cap locale switch", shown(locale(cap("en", "attach-up"), "ja")), "添付ファイルは10 MBまでです")
    C.Check.str("success clears spinner", U.Ui.uploading(failure("ja", "{\"attached\":\"/tmp/raw {name}.png\",\"thread\":\"t1\"}"), "t1"), "")
