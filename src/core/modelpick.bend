import Base
import ./json.bend as J

# The model picker under the composer (web and desktop): three columns,
# agent | model | effort, and a cursor the keys move. Tab and Shift+Tab
# go from column to column, the up and down arrows move within one, left
# and right move the effort from any column; Enter applies the cursor and
# shuts the picker, Escape only shuts it. A click applies at once (the
# picker stays open). The cursor is a draft until then: client.bend keeps
# it in the scratch "@mp" for the thread it was moved on (Pick.*), and
# this file decides everything about it. Test: test/modelpick_test.bend.

# col: 0 agent, 1 model, 2 effort (where the up and down arrows go)
type MpCur is Data:
  MpCur{col: Nat, agent: String, model: String, effort: String}

# a step of the effort meter, and whether the agent's model takes it
type MpStep is Data:
  MpStep{level: String, on: Bool}

# a bar of the meter as drawn: lit is the chosen step, fill the steps
# below it; i counts from 0 (the bar's height grows with it)
type MpBar is Data:
  MpBar{level: String, on: Bool, lit: Bool, fill: Bool, i: Nat}

# a request applying the cursor makes (sent about the selected thread)
type MpReq is Data:
  MpReq{method: String, args: List<&2, J.KV>}

def MpCur.col(c: MpCur) -> Nat:
  match c:
    case MpCur{col, _, _, _}: col

def MpCur.agent(c: MpCur) -> String:
  match c:
    case MpCur{_, a, _, _}: a

def MpCur.model(c: MpCur) -> String:
  match c:
    case MpCur{_, _, m, _}: m

def MpCur.effort(c: MpCur) -> String:
  match c:
    case MpCur{_, _, _, e}: e

# The agents
# ----------

# what each agent is, dim beside its name
def ModelPick.product(+agent: String) -> String:
  match agent:
    case "claude": "Claude Code"
    case "codex": "Codex CLI"
    case "grok": "Grok CLI"
    case "muse": "Muse Code"
    case "antigravity": "Google"
    case "opencode": "OpenCode"
    case "pi": "Pi"
    case "omp": "Oh My Pi"
    case _: ""

def ModelPick.has(xs: List<&2, String>, +x: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case +h <> t: Bool.or(String.eq(h, x), ModelPick.has(t, x))

def ModelPick.head(xs: List<&2, String>, +d: String) -> String:
  match xs:
    case Nil{}: d
    case +h <> _: h

# the item after cur in xs: cur itself when it is the last (no wrapping),
# xs's first when xs does not hold it
def ModelPick.after(xs: List<&2, String>, +cur: String, +first: String) -> String:
  match xs:
    case Nil{}: first
    case +x <> +t: Bool.pick(String, String.eq(x, cur), ModelPick.head(t, cur), ModelPick.after(t, cur, first))

def ModelPick.next(+xs: List<&2, String>, +cur: String) -> String:
  ModelPick.after(xs, cur, ModelPick.head(xs, cur))

def ModelPick.prev(+xs: List<&2, String>, +cur: String) -> String:
  ModelPick.after(List.reverse(&2, String, xs), cur, ModelPick.head(xs, cur))

# The effort meter
# ----------------
# Always the six steps of the common scale (auto, the agent's own default
# "", then low to max), with any other level the agent takes (off, none,
# minimal, omp's auto) in its place among them; a step the agent's model
# does not take is drawn but disabled. es: the levels it takes, "" first
# (client.bend's Effort.chips; none when it takes no effort at all).

def ModelPick.scale() -> List<&2, String>:
  ["", "off", "none", "auto", "minimal", "low", "medium", "high", "xhigh", "max"]

def ModelPick.common() -> List<&2, String>:
  ["", "low", "medium", "high", "xhigh", "max"]

def ModelPick.steps.k(shown: Bool, +x: String, on: Bool, rest: List<&2, MpStep>) -> List<&2, MpStep>:
  match shown:
    case True{}: MpStep{x, on} <> rest
    case False{}: rest

def ModelPick.extra(es: List<&2, String>) -> List<&2, MpStep>:
  match es:
    case Nil{}: Nil{}
    case +e <> t: ModelPick.steps.k(Bool.not(ModelPick.has(ModelPick.scale(), e)), e, True{}, ModelPick.extra(t))

def ModelPick.steps.go(xs: List<&2, String>, +es: List<&2, String>) -> List<&2, MpStep>:
  match xs:
    case Nil{}: ModelPick.extra(es)
    case +x <> t: ModelPick.steps.k(Bool.or(ModelPick.has(ModelPick.common(), x), ModelPick.has(es, x)), x, ModelPick.has(es, x), ModelPick.steps.go(t, es))

def ModelPick.steps(+es: List<&2, String>) -> List<&2, MpStep>:
  ModelPick.steps.go(ModelPick.scale(), es)

def ModelPick.levels.k(on: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match on:
    case True{}: l <> rest
    case False{}: rest

# the levels the meter lets you pick, in its order
def ModelPick.levels(ss: List<&2, MpStep>) -> List<&2, String>:
  match ss:
    case Nil{}: Nil{}
    case MpStep{l, on} <> t: ModelPick.levels.k(on, l, ModelPick.levels(t))

# the step drawn lit for effort e: e when the meter lets you pick it, else
# the agent's default; none (a level no step has) when it takes no effort
def ModelPick.lit(+ss: List<&2, MpStep>, +e: String) -> String:
  Bool.pick(String, ModelPick.has(ModelPick.levels(ss), e), e, Bool.pick(String, ModelPick.has(ModelPick.levels(ss), ""), "", "-"))

def ModelPick.bars.go(ss: List<&2, MpStep>, +lit: String, +below: Bool, +i: Nat) -> List<&2, MpBar>:
  match ss:
    case Nil{}: Nil{}
    case MpStep{+l, on} <> t:
      +hit = String.eq(l, lit)
      MpBar{l, on, hit, Bool.and(below, Bool.not(hit)), i} <> ModelPick.bars.go(t, lit, Bool.and(below, Bool.not(hit)), 1n+i)

# the meter's bars with effort e chosen
def ModelPick.bars(+ss: List<&2, MpStep>, +e: String) -> List<&2, MpBar>:
  ModelPick.bars.go(ss, ModelPick.lit(ss, e), ModelPick.has(ModelPick.levels(ss), ModelPick.lit(ss, e)), 0n)

# Moving the cursor
# -----------------

def ModelPick.col.next(+c: Nat) -> Nat:
  Nat.mod(1n+c, 3n)

def ModelPick.col.prev(+c: Nat) -> Nat:
  Nat.mod(Nat.add(c, 2n), 3n)

# the cursor on agent a: another agent starts on its default model, or on
# the thread's own model when a is the thread's agent (now)
def ModelPick.agent(c: MpCur, +a: String, +now: MpCur) -> MpCur:
  match c:
    case MpCur{col, +ca, m, e}:
      MpCur{col, a, Bool.pick(String, String.eq(a, ca), m, Bool.pick(String, String.eq(a, MpCur.agent(now)), MpCur.model(now), "")), e}

def ModelPick.model(c: MpCur, +m: String) -> MpCur:
  match c:
    case MpCur{col, a, _, e}: MpCur{col, a, m, e}

def ModelPick.effort(c: MpCur, +e: String) -> MpCur:
  match c:
    case MpCur{col, a, m, _}: MpCur{col, a, m, e}

def ModelPick.col(c: MpCur, +col: Nat) -> MpCur:
  match c:
    case MpCur{_, a, m, e}: MpCur{col, a, m, e}

# up (more) or down (less) the meter, from the step lit; a meter with
# nothing to pick leaves the effort as it is
def ModelPick.step(+c: MpCur, +ss: List<&2, MpStep>, more: Bool) -> MpCur:
  Bool.pick(MpCur, List.is_empty(&2, String, ModelPick.levels(ss)), c,
    ModelPick.effort(c, Bool.pick(String, more, ModelPick.next(ModelPick.levels(ss), ModelPick.lit(ss, MpCur.effort(c))), ModelPick.prev(ModelPick.levels(ss), ModelPick.lit(ss, MpCur.effort(c))))))

# up/down (down: True) within the cursor's column
def ModelPick.move.col(col: Nat, +c: MpCur, down: Bool, +agents: List<&2, String>, +models: List<&2, String>, +ss: List<&2, MpStep>, +now: MpCur) -> MpCur:
  match col:
    case 0n: ModelPick.agent(c, Bool.pick(String, down, ModelPick.next(agents, MpCur.agent(c)), ModelPick.prev(agents, MpCur.agent(c))), now)
    case 1n: ModelPick.model(c, Bool.pick(String, down, ModelPick.next(models, MpCur.model(c)), ModelPick.prev(models, MpCur.model(c))))
    case _: ModelPick.step(c, ss, Bool.not(down))

# what key k does to cursor c ("tab", "stab" for Shift+Tab, "up", "down",
# "left", "right"; anything else leaves it). agents and models are the
# columns' rows, ss the meter of c's agent and model, now the thread's
# own agent, model and effort.
def ModelPick.key(+k: String, +c: MpCur, +agents: List<&2, String>, +models: List<&2, String>, +ss: List<&2, MpStep>, +now: MpCur) -> MpCur:
  match k:
    case "tab": ModelPick.col(c, ModelPick.col.next(MpCur.col(c)))
    case "stab": ModelPick.col(c, ModelPick.col.prev(MpCur.col(c)))
    case "up": ModelPick.move.col(MpCur.col(c), c, False{}, agents, models, ss, now)
    case "down": ModelPick.move.col(MpCur.col(c), c, True{}, agents, models, ss, now)
    case "left": ModelPick.step(c, ss, False{})
    case "right": ModelPick.step(c, ss, True{})
    case _: c

# Applying it
# -----------
# Against the thread's own (now): another agent is one request carrying
# the model and, when it changed, the effort (the hub switches only between
# turns, and then takes all three, or none); the same agent asks for the
# model and the effort that changed. Nothing changed asks for nothing.

def ModelPick.effort.arg(same: Bool, +e: String) -> List<&2, J.KV>:
  match same:
    case True{}: Nil{}
    case False{}: [J.KV{"effort", J.Str{e}}]

def ModelPick.plan.same(+c: MpCur, +now: MpCur) -> List<&2, MpReq>:
  List.append(&2, MpReq,
    Bool.pick(List<&2, MpReq>, String.eq(MpCur.model(c), MpCur.model(now)), Nil{}, [MpReq{"thread.modes", [J.KV{"model", J.Str{MpCur.model(c)}}]}]),
    Bool.pick(List<&2, MpReq>, String.eq(MpCur.effort(c), MpCur.effort(now)), Nil{}, [MpReq{"thread.effort", [J.KV{"effort", J.Str{MpCur.effort(c)}}]}]))

def ModelPick.plan.k(other: Bool, +c: MpCur, +now: MpCur) -> List<&2, MpReq>:
  match other:
    case True{}:
      [MpReq{"thread.provider", J.KV{"provider", J.Str{MpCur.agent(c)}} <> J.KV{"model", J.Str{MpCur.model(c)}} <> ModelPick.effort.arg(String.eq(MpCur.effort(c), MpCur.effort(now)), MpCur.effort(c))}]
    case False{}:
      ModelPick.plan.same(c, now)

def ModelPick.plan(+c: MpCur, +now: MpCur) -> List<&2, MpReq>:
  ModelPick.plan.k(Bool.not(String.eq(MpCur.agent(c), MpCur.agent(now))), c, now)

# Keeping it
# ----------
# "thread\tcol\tagent\tmodel\teffort"; a cursor kept for another thread,
# or none, is the thread's own (now) in the model column.

def ModelPick.show(+thread: String, c: MpCur) -> String:
  match c:
    case MpCur{col, a, m, e}:
      thread ++ "\t" ++ Nat.show(col) ++ "\t" ++ a ++ "\t" ++ m ++ "\t" ++ e

def ModelPick.col.read(+s: String) -> Nat:
  match s:
    case "0": 0n
    case "2": 2n
    case _: 1n

def ModelPick.read.k(ps: List<&2, String>, +thread: String, now: MpCur) -> MpCur:
  match ps:
    case +th <> col <> a <> m <> e <> Nil{}: Bool.pick(MpCur, String.eq(th, thread), MpCur{ModelPick.col.read(col), a, m, e}, ModelPick.col(now, 1n))
    case _: ModelPick.col(now, 1n)

def ModelPick.read(+s: String, +thread: String, now: MpCur) -> MpCur:
  ModelPick.read.k(String.split(s, '\t'), thread, now)

# whether s keeps a cursor moved on thread
def ModelPick.kept(+s: String, +thread: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(thread)), String.starts_with(s, thread ++ "\t"))
