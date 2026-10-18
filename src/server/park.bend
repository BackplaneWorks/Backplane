import Base
import ../core/mcp.bend as MC

# Agent permission requests waiting on the user: an open ask keeps its
# response channel here until answered. One actor owns the list; answers
# arrive by ask id, each at most once.

type PMsg is Data:
  PPark{ask: String, body: String, reply: Chan(String), omp: Bool}
  PAnswer{ask: String, answer: String}

type Parked is Data:
  Parked{ask: String, body: String, reply: Chan(String), omp: Bool}

type Taken is Data:
  Taken{hit: Maybe<&2, Parked>, rest: List<&2, Parked>}

def Taken.keep(p: Parked, t: Taken) -> Taken:
  match t:
    case Taken{h, r}:
      Taken{h, p <> r}

def Park.hit(ps: List<&2, Parked>, +ask: String) -> Bool:
  match ps:
    case Parked{a, _, _, _} <> _:
      String.eq(a, ask)
    case Nil{}:
      False{}

def Park.take(ps: List<&2, Parked>, hit: Bool, +ask: String) -> Taken:
  match ps hit:
    case p <> t True{}:
      Taken{Some{p}, t}
    case p <> +t False{}:
      Taken.keep(p, Park.take(t, Park.hit(t, ask), ask))
    case Nil{} _:
      Taken{None{}, Nil{}}

# the reply a parked request gets: its permission answer, or (parked
# with no body, as the server's own waits are) the user's answer itself
def Park.reply.permission(omp: Bool, +body: String, +answer: String) -> String:
  match omp:
    case True{}:
      MC.Mcp.permission.reply.once(body, answer)
    case False{}:
      MC.Mcp.permission.reply(body, answer)

def Park.reply(omp: Bool, +body: String, +answer: String) -> String:
  Bool.pick(String, String.is_empty(body), answer, Park.reply.permission(omp, body, answer))

def Park.send(t: Taken, +answer: String) -> IO(List<&2, Parked>):
  match t:
    case Taken{Some{Parked{_, +body, reply, omp}}, rest}:
      do IO<List<&2, Parked>>:
        ok : Result<&1, &1, String, Unit> <- Chan.send(String, reply, Park.reply(omp, body, answer))
        return rest
    case Taken{None{}, rest}:
      IO.pure(List<&2, Parked>, rest)

def Park.step(m: Maybe<&1, PMsg>, +ps: List<&2, Parked>, k: List<&2, Parked> -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{PPark{a, b, r, omp}}:
      k(Parked{a, b, r, omp} <> ps)
    case Some{PAnswer{+a, +an}}:
      IO.bind(List<&2, Parked>, Unit, Park.send(Park.take(ps, Park.hit(ps, a), a), an), k)
    case None{}:
      IO.pure(Unit, Unit{})

def Park.loop(fuel: Nat, +q: Chan(PMsg), ps: List<&2, Parked>) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, PMsg>, Unit, Chan.recv(PMsg, q), m => Park.step(m, ps, p2 => Park.loop(f, q, p2)))
