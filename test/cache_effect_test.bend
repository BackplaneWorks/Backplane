import Base
import ./check.bend as C
import ../src/server/cacheio.bend as K
import ../src/server/castio.bend as CAST

def cached(m: Maybe<&2, String>) -> String:
  match m:
    case None{}: "missing"
    case Some{s}: s

def clients(xs: List<&2, U32>) -> String:
  match xs:
    case Nil{}: ""
    case x <> t: U32.show(x) ++ "," ++ clients(t)

def main() -> IO(Unit):
  do IO<Unit>:
    ok : Bool <- K.Cache.put(K.BrowserLease{}, "other-page", "payload 日本語")
    C.Check.str("cache admits a bounded string", Bool.show(ok), "True")
    value : Maybe<&2, String> <- K.Cache.get(K.BrowserLease{}, "other-page")
    C.Check.str("cache effect returns the typed value", cached(value), "payload 日本語")
    K.Cache.remove(K.BrowserLease{}, "other-page")
    gone : Maybe<&2, String> <- K.Cache.get(K.BrowserLease{}, "other-page")
    C.Check.str("cache removal is a miss", cached(gone), "missing")
    a : Bool <- CAST.Cast.seen(1, "page")
    b : Bool <- CAST.Cast.seen(2, "page")
    C.Check.str("shared browser lease is due once", Bool.show(a) ++ Bool.show(b), "TrueFalse")
    to : List<&2, U32> <- CAST.Cast.to("page")
    C.Check.str("browser subscription targets", clients(to), "1,2,")
    CAST.Cast.drop(1)
    left : List<&2, U32> <- CAST.Cast.to("page")
    C.Check.str("browser disconnect cleanup", clients(left), "2,")
