import Base
import ./host.bend as H
import ./proc.bend as P

# Converter output is private until the converter succeeds and its staged
# file can be moved into the cache. Never publish a failed export.
def ModelCache.renamed(m: Result<&1, &1, U32 & String, Unit>, r: P.Ran) -> P.Ran:
  match m:
    case Done{_}: r
    case Fail{e}:
      (code, text) = e
      P.Ran{1, "Could not cache 3D model: " ++ text}

def ModelCache.nonempty(st: Result<&1, &1, U32 & String, U32 & U32 & U32>) -> Bool:
  match st:
    case Done{(kind, (size, _))}: Bool.and(U32.is_eq(kind, 0), U32.is_gt(size, 0))
    case Fail{_}: False{}

def ModelCache.ready(ok: Bool, r: P.Ran, +from: String, +out: String) -> IO(P.Ran):
  match ok:
    case True{}:
      IO.bind(Result<&1, &1, U32 & String, Unit>, P.Ran, H.FS.rename(from, out), m => IO.pure(P.Ran, ModelCache.renamed(m, r)))
    case False{}:
      IO.bind(Result<&1, &1, U32 & String, Unit>, P.Ran, H.FS.remove(from), _ => IO.pure(P.Ran, P.Ran{1, "Could not cache 3D model: converter produced no model"}))

def ModelCache.publish(r: P.Ran, +from: String, +out: String) -> IO(P.Ran):
  match r:
    case P.Ran{0, log}:
      IO.bind(Result<&1, &1, U32 & String, U32 & U32 & U32>, P.Ran, H.FS.stat(from), st => ModelCache.ready(ModelCache.nonempty(st), P.Ran{0, log}, from, out))
    case P.Ran{code, log}:
      IO.bind(Result<&1, &1, U32 & String, Unit>, P.Ran, H.FS.remove(from), _ => IO.pure(P.Ran, P.Ran{code, log}))
