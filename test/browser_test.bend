import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/server/browser.bend as BR

def yes(b: Bool) -> String:
  match b:
    case True{}:
      "yes"
    case False{}:
      "no"

def js(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("safe shared", BR.Br.safe(""), "shared")
    C.Check.str("safe keeps", BR.Br.safe("bot-7_A"), "bot-7_A")
    C.Check.str("safe maps", BR.Br.safe("a.b/c d"), "a_b_c_d")
    C.Check.str("shot file", BR.Br.shot_file("/h", "x/y"), "/h/shots/x_y.jpg")
    C.Check.str("paged sets page", J.Json.show(BR.Browser.paged("b1", js("{\"url\":\"x\"}"))), "{\"page\":\"b1\",\"url\":\"x\"}")
    C.Check.str("paged drops a page from the args", J.Json.show(BR.Browser.paged("", js("{\"page\":\"b2\",\"x\":1,\"page\":\"b3\"}"))), "{\"page\":\"\",\"x\":1}")
    C.Check.str("paged no args", J.Json.show(BR.Browser.paged("b1", J.Null{})), "{\"page\":\"b1\"}")
    C.Check.str("request", BR.Browser.request(3n, "open", BR.Browser.paged("b1", js("{\"url\":\"x\"}"))), "{\"id\":3,\"op\":\"open\",\"page\":\"b1\",\"url\":\"x\"}")
    C.Check.str("page frame is a shot", yes(BR.Browser.is_shot(js("{\"event\":\"frame\",\"page\":\"b1\",\"n\":4,\"path\":\"/p\"}"))), "yes")
    C.Check.str("shared page frame is a shot", yes(BR.Browser.is_shot(js("{\"event\":\"frame\",\"page\":\"\",\"n\":1}"))), "yes")
    C.Check.str("raw frame is not", yes(BR.Browser.is_shot(js("{\"event\":\"frame\",\"w\":7,\"h\":8,\"seq\":3,\"path\":\"/p\"}"))), "no")
    C.Check.str("navigated is not", yes(BR.Browser.is_shot(js("{\"event\":\"navigated\",\"page\":\"b1\",\"url\":\"u\"}"))), "no")
    C.Check.str("reply is not", yes(BR.Browser.is_shot(js("{\"id\":2,\"ok\":true,\"result\":{}}"))), "no")
    C.Check.str("record reaches the hub", yes(BR.Browser.is_page_event(js("{\"event\":\"record\",\"page\":\"b1\",\"step\":{\"do\":\"click\"}}"))), "yes")
    C.Check.str("navigated reaches the hub", yes(BR.Browser.is_page_event(js("{\"event\":\"navigated\",\"page\":\"b1\",\"url\":\"u\"}"))), "yes")
    C.Check.str("load reaches the hub", yes(BR.Browser.is_page_event(js("{\"event\":\"load\",\"page\":\"b1\",\"url\":\"u\"}"))), "yes")
    C.Check.str("page frame reaches the hub", yes(BR.Browser.is_page_event(js("{\"event\":\"frame\",\"page\":\"b1\",\"n\":4,\"path\":\"/p\"}"))), "yes")
    C.Check.str("raw frame stays", yes(BR.Browser.is_page_event(js("{\"event\":\"frame\",\"w\":7,\"h\":8,\"seq\":3,\"path\":\"/p\"}"))), "no")
    C.Check.str("record without a page stays", yes(BR.Browser.is_page_event(js("{\"event\":\"record\",\"step\":{}}"))), "no")
    C.Check.str("reply stays", yes(BR.Browser.is_page_event(js("{\"id\":2,\"ok\":true,\"result\":{}}"))), "no")
    C.Check.str("shot message", BR.Browser.shot("{\"event\":\"frame\",\"page\":\"b1\",\"n\":12,\"path\":\"/p\"}"), "{\"t\":\"shot\",\"bot\":\"b1\",\"n\":12}")
