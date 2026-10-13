import Base
import ./json.bend as J
import ./task.bend as TK
import ./bot.bend as BT

# Teaching bots browser tasks, the hub's side (pure; docs/teach.md). The
# hub keeps, per bot, who drives its page ("" the bot, "teach", "takeover")
# and the teach state ("" nothing, "rec" recording, "review" after stop)
# with the draft steps. None of it is in the event log: a restart ends a
# teach. It reaches clients as the info keys drive.<bot>, teach.<bot> and
# teach.draft.<bot>. Here are that state and its transitions, the info
# text, saving a draft as a task, the task_* tools' pure parts, the ask a
# pause opens, and the bot's task list for its prompt.

# State
# -----

type TeachBot is Data:
  TeachBot{bot: String, drive: String, mode: String, start: Nat, steps: List<&2, TK.TkStep>}

# every bot with a driver or a teach, and the pauses already alerted
type TeachSt is Data:
  TeachSt{bots: List<&2, TeachBot>, alerted: List<&2, String>}

def Teach.empty() -> TeachSt:
  TeachSt{Nil{}, Nil{}}

def Teach.blank(+bot: String) -> TeachBot:
  TeachBot{bot, "", "", 0n, Nil{}}

def Teach.bot.id(b: TeachBot) -> String:
  match b:
    case TeachBot{id, _, _, _, _}:
      id

def Teach.bot.drive(b: TeachBot) -> String:
  match b:
    case TeachBot{_, d, _, _, _}:
      d

def Teach.bot.mode(b: TeachBot) -> String:
  match b:
    case TeachBot{_, _, m, _, _}:
      m

def Teach.bot.start(b: TeachBot) -> Nat:
  match b:
    case TeachBot{_, _, _, s, _}:
      s

def Teach.bot.steps(b: TeachBot) -> List<&2, TK.TkStep>:
  match b:
    case TeachBot{_, _, _, _, xs}:
      xs

def Teach.bot.idle(+b: TeachBot) -> Bool:
  Bool.and(String.is_empty(Teach.bot.drive(b)), String.is_empty(Teach.bot.mode(b)))

def Teach.pick(hit: Bool, b: TeachBot, acc: TeachBot) -> TeachBot:
  match hit:
    case True{}:
      b
    case False{}:
      acc

def Teach.find.go(bs: List<&2, TeachBot>, +bot: String, acc: TeachBot) -> TeachBot:
  match bs:
    case Nil{}:
      acc
    case +b <> t:
      Teach.find.go(t, bot, Teach.pick(String.eq(Teach.bot.id(b), bot), b, acc))

def Teach.st.bots(s: TeachSt) -> List<&2, TeachBot>:
  match s:
    case TeachSt{bs, _}:
      bs

def Teach.st.alerted(s: TeachSt) -> List<&2, String>:
  match s:
    case TeachSt{_, al}:
      al

# a bot's state (idle when it has none)
def Teach.get(s: TeachSt, +bot: String) -> TeachBot:
  Teach.find.go(Teach.st.bots(s), bot, Teach.blank(bot))

def Teach.keep(drop: Bool, b: TeachBot, rest: List<&2, TeachBot>) -> List<&2, TeachBot>:
  match drop:
    case True{}:
      rest
    case False{}:
      b <> rest

def Teach.without(bs: List<&2, TeachBot>, +bot: String) -> List<&2, TeachBot>:
  match bs:
    case Nil{}:
      Nil{}
    case +b <> t:
      Teach.keep(String.eq(Teach.bot.id(b), bot), b, Teach.without(t, bot))

def Teach.put.k(idle: Bool, b: TeachBot, rest: List<&2, TeachBot>) -> List<&2, TeachBot>:
  match idle:
    case True{}:
      rest
    case False{}:
      b <> rest

# the bot's state replaced; an idle bot leaves the list
def Teach.put(+s: TeachSt, +b: TeachBot) -> TeachSt:
  TeachSt{Teach.put.k(Teach.bot.idle(b), b, Teach.without(Teach.st.bots(s), Teach.bot.id(b))), Teach.st.alerted(s)}

def Teach.drive.k(+d: String) -> TK.TkDrive:
  Bool.pick(TK.TkDrive, String.is_empty(d), TK.DBot{}, TK.DPerson{d})

# who drives the bot's page
def Teach.drive_of(s: TeachSt, +bot: String) -> TK.TkDrive:
  Teach.drive.k(Teach.bot.drive(Teach.get(s, bot)))

# whether the bot may use its own browser now (preview_* calls)
def Teach.bot_may(s: TeachSt, +bot: String) -> Bool:
  TK.Drive.can_bot(Teach.drive_of(s, bot))

def Teach.refusal() -> String:
  "The person has the browser (teaching or taking over); wait for them to hand it back."

# Transitions
# -----------

def Teach.recording(+b: TeachBot) -> Bool:
  String.eq(Teach.bot.mode(b), "rec")

def Teach.max_steps() -> Nat:
  400n

# teach begins: the person drives, recording, an empty draft
def Teach.start(+bot: String, +now: Nat) -> TeachBot:
  TeachBot{bot, "teach", "rec", now, Nil{}}

def Teach.add.k(rec: Bool, b: TeachBot, s: TK.TkStep) -> TeachBot:
  match rec b:
    case True{} TeachBot{id, d, m, st, xs}:
      TeachBot{id, d, m, st, List.append(&2, TK.TkStep, xs, [s])}
    case False{} _:
      b

# a record event of the helper (or a note) joins the draft while recording,
# numbered from 1; a secret's step carries its label only (TkStep.of_record)
def Teach.add(+b: TeachBot, +r: J.Json) -> TeachBot:
  Teach.add.k(Bool.and(Teach.recording(b), Nat.is_lt(List.length(&2, TK.TkStep, Teach.bot.steps(b)), Teach.max_steps())), b,
    TK.TkStep.of_record(r, Nat.add(List.length(&2, TK.TkStep, Teach.bot.steps(b)), 1n)))

# a typed note as a record
def Teach.note(+text: String) -> J.Json:
  J.Json.obj([J.KV{"do", J.Str{"note"}}, J.KV{"value", J.Str{text}}])

def Teach.stop.k(rec: Bool, b: TeachBot) -> TeachBot:
  match rec b:
    case True{} TeachBot{id, d, _, st, xs}:
      TeachBot{id, d, "review", st, xs}
    case False{} _:
      b

# recording off, the draft kept for review
def Teach.stop(+b: TeachBot) -> TeachBot:
  Teach.stop.k(Teach.recording(b), b)

# the person takes the browser (not while teaching: they have it)
def Teach.takeover.k(free: Bool, b: TeachBot) -> TeachBot:
  match free b:
    case True{} TeachBot{id, _, m, st, xs}:
      TeachBot{id, "takeover", m, st, xs}
    case False{} _:
      b

def Teach.takeover(+b: TeachBot) -> TeachBot:
  Teach.takeover.k(String.is_empty(Teach.bot.drive(b)), b)

def Teach.handback.k(taken: Bool, b: TeachBot) -> TeachBot:
  match taken b:
    case True{} TeachBot{id, _, m, st, xs}:
      TeachBot{id, "", m, st, xs}
    case False{} _:
      b

# the bot drives again (a take over ends; a teach ends only by stop/save)
def Teach.handback(+b: TeachBot) -> TeachBot:
  Teach.handback.k(String.eq(Teach.bot.drive(b), "takeover"), b)

# a recording lasts at most ten minutes
def Teach.limit() -> Nat:
  600n

def Teach.expired(+b: TeachBot, +now: Nat) -> Bool:
  Bool.and(Teach.recording(b), Nat.is_ge(Nat.sub(now, Teach.bot.start(b)), Teach.limit()))

# Info
# ----

def Teach.mode.text(+b: TeachBot) -> String:
  Bool.pick(String, Teach.recording(b), "rec " ++ Nat.show(Teach.bot.start(b)) ++ " " ++ Nat.show(List.length(&2, TK.TkStep, Teach.bot.steps(b))),
    Bool.pick(String, String.eq(Teach.bot.mode(b), "review"), "review", ""))

def Teach.draft.text(+b: TeachBot) -> String:
  Bool.pick(String, String.is_empty(Teach.bot.mode(b)), "", J.Json.show(J.Json.arr(TK.Tk.steps.json(Teach.bot.steps(b)))))

# the three info keys of a bot (empty values when idle: clients clear them)
def Teach.kvs(+b: TeachBot) -> List<&2, J.KV>:
  [J.KV{"drive." ++ Teach.bot.id(b), J.Str{Teach.bot.drive(b)}}, J.KV{"teach." ++ Teach.bot.id(b), J.Str{Teach.mode.text(b)}},
   J.KV{"teach.draft." ++ Teach.bot.id(b), J.Str{Teach.draft.text(b)}}]

def Teach.info.go(bs: List<&2, TeachBot>) -> List<&2, J.KV>:
  match bs:
    case Nil{}:
      Nil{}
    case +b <> t:
      List.append(&2, J.KV, Teach.kvs(b), Teach.info.go(t))

# every bot with something to say, for a joining client's info
def Teach.info(s: TeachSt) -> List<&2, J.KV>:
  Teach.info.go(Teach.st.bots(s))

# {"t":"info","info":{...}} for one bot, sent to every client when it changes
def Teach.msg(+b: TeachBot) -> String:
  J.Json.show(J.Json.obj([J.KV{"t", J.Str{"info"}}, J.KV{"info", J.Json.obj(Teach.kvs(b))}]))

# Alerts
# ------

def Teach.alerted(s: TeachSt, +key: String) -> Bool:
  TK.Tk.elem(Teach.st.alerted(s), key)

# the key noted as alerted (the newest 64 kept)
def Teach.alert_mark(+s: TeachSt, +key: String) -> TeachSt:
  TeachSt{Teach.st.bots(s), key <> List.take(&2, String, Teach.st.alerted(s), 63n)}

# Saving a draft as a task
# ------------------------

type PMark is Data:
  PMark{step: Nat, name: String, label: String}

def Teach.pname.u(+u: U32) -> U32:
  Bool.pick(U32, Bool.or(U32.is_eq(u, 32), U32.is_eq(u, 45)), 95, u)

def Teach.pname.ok(+u: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(u, 48), U32.is_le(u, 57)), Bool.and(U32.is_ge(u, 97), U32.is_le(u, 122))),
    Bool.or(Bool.and(U32.is_ge(u, 65), U32.is_le(u, 90)), U32.is_eq(u, 95)))

def Teach.pname.put(ok: Bool, +u: U32, rest: String) -> String:
  match ok:
    case True{}:
      SCon{Chr{u}, rest}
    case False{}:
      rest

def Teach.pname.c(c: Char, rest: String) -> String:
  match c:
    case Chr{+u}:
      Teach.pname.put(Teach.pname.ok(Teach.pname.u(u)), Teach.pname.u(u), rest)

# a parameter's name: letters, digits and _ (spaces and - become _)
def Teach.pname(s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{c, t}:
      Teach.pname.c(c, Teach.pname(t))

def Teach.marks.of(xs: List<&2, J.Json>) -> List<&2, PMark>:
  match xs:
    case Nil{}:
      Nil{}
    case +j <> t:
      PMark{J.Json.get_nat(j, "step", 0n), Teach.pname(String.trim(J.Json.get_str(j, "name", ""))), J.Json.get_str(j, "label", "")} <> Teach.marks.of(t)

def Teach.mark.hit(ms: List<&2, PMark>, +n: Nat) -> Bool:
  match ms:
    case PMark{s, _, _} <> _:
      Nat.is_eq(s, n)
    case Nil{}:
      False{}

def Teach.mark.go(ms: List<&2, PMark>, hit: Bool, +n: Nat) -> Maybe<&2, PMark>:
  match ms hit:
    case m <> _ True{}:
      Some{m}
    case _ <> +t False{}:
      Teach.mark.go(t, Teach.mark.hit(t, n), n)
    case Nil{} _:
      None{}

# the mark for step n, the first
def Teach.mark.of(+ms: List<&2, PMark>, +n: Nat) -> Maybe<&2, PMark>:
  Teach.mark.go(ms, Teach.mark.hit(ms, n), n)

def Teach.mark.name(m: Maybe<&2, PMark>) -> String:
  match m:
    case Some{PMark{_, n, _}}:
      n
    case None{}:
      ""

def Teach.mark.label(m: Maybe<&2, PMark>) -> String:
  match m:
    case Some{PMark{_, _, l}}:
      l
    case None{}:
      ""

def Teach.is_lit(v: TK.TkValue) -> Bool:
  match v:
    case TK.VLit{_}:
      True{}
    case _:
      False{}

def Teach.lit.text(v: TK.TkValue) -> String:
  match v:
    case TK.VLit{t}:
      t
    case _:
      ""

# a typed value marked as a parameter becomes {{name}}; a secret never does
def Teach.value.named(is_lit: Bool, v: TK.TkValue, +name: String) -> TK.TkValue:
  Bool.pick(TK.TkValue, Bool.and(is_lit, Bool.not(String.is_empty(name))), TK.VParam{name}, v)

def Teach.apply.step(s: TK.TkStep, +ms: List<&2, PMark>) -> TK.TkStep:
  match s:
    case TK.TkStep{+n, act, target, +v, wu, wt, url, risk, note}:
      TK.TkStep{n, act, target, Teach.value.named(Teach.is_lit(v), v, Teach.mark.name(Teach.mark.of(ms, n))), wu, wt, url, risk, note}

def Teach.apply(xs: List<&2, TK.TkStep>, +ms: List<&2, PMark>) -> List<&2, TK.TkStep>:
  match xs:
    case Nil{}:
      Nil{}
    case s <> t:
      Teach.apply.step(s, ms) <> Teach.apply(t, ms)

def Teach.step.sel(hit: Bool, v: TK.TkValue, acc: TK.TkValue) -> TK.TkValue:
  Bool.pick(TK.TkValue, hit, v, acc)

# the typed value of step n
def Teach.step.at(xs: List<&2, TK.TkStep>, +n: Nat, acc: TK.TkValue) -> TK.TkValue:
  match xs:
    case Nil{}:
      acc
    case TK.TkStep{+k, _, _, v, _, _, _, _, _} <> t:
      Teach.step.at(t, n, Teach.step.sel(Nat.is_eq(k, n), v, acc))

def Teach.param.put(skip: Bool, p: TK.TkParam, rest: List<&2, TK.TkParam>) -> List<&2, TK.TkParam>:
  match skip:
    case True{}:
      rest
    case False{}:
      p <> rest

def Teach.param.k(skip: Bool, +secret: Bool, +name: String, +label: String, example: String, rest: List<&2, TK.TkParam>) -> List<&2, TK.TkParam>:
  Teach.param.put(Bool.or(skip, secret), TK.TkParam{name, Bool.pick(String, String.is_empty(label), name, label), "text", example}, rest)

# the parameters marks name: each once, the example the step's typed value
def Teach.params(ms: List<&2, PMark>, +xs: List<&2, TK.TkStep>, +seen: List<&2, String>) -> List<&2, TK.TkParam>:
  match ms:
    case Nil{}:
      Nil{}
    case PMark{+s, +name, +label} <> t:
      Teach.param.k(Bool.or(String.is_empty(name), TK.Tk.elem(seen, name)), Bool.not(Teach.is_lit(Teach.step.at(xs, s, TK.VNone{}))), name, label, Teach.lit.text(Teach.step.at(xs, s, TK.VNone{})), Teach.params(t, xs, name <> seen))

def Teach.urls.add(yes: Bool, url: String, seen: List<&2, String>) -> List<&2, String>:
  match yes:
    case True{}:
      url <> seen
    case False{}:
      seen

# the urls the navigate steps visit, each once, in order
def Teach.urls(xs: List<&2, TK.TkStep>, +seen: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      List.reverse(&2, String, seen)
    case TK.TkStep{_, +act, _, _, _, _, +url, _, _} <> t:
      Teach.urls(t, Teach.urls.add(Bool.and(String.eq(act, "navigate"), Bool.and(Bool.not(String.is_empty(url)), Bool.not(TK.Tk.elem(seen, url)))), url, seen))

# where a task starts: its first navigate
def Teach.first_url(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case u <> _:
      u

def Teach.renumber(xs: List<&2, TK.TkStep>, +n: Nat) -> List<&2, TK.TkStep>:
  match xs:
    case Nil{}:
      Nil{}
    case TK.TkStep{_, act, target, v, wu, wt, url, risk, note} <> t:
      TK.TkStep{n, act, target, v, wu, wt, url, risk, note} <> Teach.renumber(t, Nat.add(n, 1n))

def Teach.build.k(+steps: List<&2, TK.TkStep>, params: List<&2, TK.TkParam>, +name: String, description: String, +now: Nat) -> TK.TkTask:
  TK.TkTask{TK.Task.slug(name), name, description, "show", Teach.first_url(Teach.urls(steps, Nil{})), params, steps, Teach.urls(steps, Nil{}), now, now}

# the draft with its marked values as parameters, as a task shown by the person
def Teach.build(+b: TeachBot, +name: String, description: String, +marks: List<&2, J.Json>, +now: Nat) -> TK.TkTask:
  Teach.build.k(Teach.apply(Teach.bot.steps(b), Teach.marks.of(marks)), Teach.params(Teach.marks.of(marks), Teach.bot.steps(b), Nil{}), name, description, now)

def Teach.task.text(t: TK.TkTask) -> String:
  J.Json.show(TK.Task.json(t))

# What the bot is told
# --------------------

def Teach.cut(+s: String, +n: Nat) -> String:
  Bool.pick(String, Nat.is_gt(String.length(s), n), String.take(s, n) ++ "...", s)

def Teach.what(t: TK.TkTarget, url: String) -> String:
  match t:
    case TK.TkTarget{+role, +name, +label, +placeholder, +text, _, +css, _, _, _}:
      TK.Tk.first(name, TK.Tk.first(label, TK.Tk.first(placeholder, TK.Tk.first(text, TK.Tk.first(css, url)))))

def Teach.value.text(v: TK.TkValue) -> String:
  match v:
    case TK.VLit{+t}:
      " \"" ++ Teach.cut(t, 40n) ++ "\""
    case TK.VParam{n}:
      " {{" ++ n ++ "}}"
    case TK.VSecret{l}:
      " <secret: " ++ l ++ ">"
    case TK.VNone{}:
      ""

def Teach.line(s: TK.TkStep) -> String:
  match s:
    case TK.TkStep{n, +act, target, v, _, _, url, _, note}:
      Nat.show(n) ++ " " ++ act ++ Bool.pick(String, String.eq(act, "note"), " " ++ Teach.cut(note, 80n), " " ++ Teach.cut(Teach.what(target, url), 60n) ++ Teach.value.text(v))

def Teach.lines(xs: List<&2, TK.TkStep>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case s <> t:
      Teach.line(s) <> Teach.lines(t)

# the message that wakes the bot after a task was saved from a recording
def Teach.taught(+name: String, t: TK.TkTask) -> String:
  "[taught: " ++ name ++ "] The person just showed you this task in your browser. Learn it from the recording: read it with task_get, improve the description and notes, mark decision points, and save it with task_save. Steps: "
    ++ String.join(Teach.lines(TK.Tk.steps_of(t)), "; ")

def Teach.run_msg(+slug: String, +params: String) -> String:
  "[task: " ++ slug ++ "] Run the task skill " ++ slug ++ " with " ++ params ++ "."

# Task tools (pure parts)
# -----------------------

def Teach.names() -> List<&2, String>:
  ["task_list", "task_get", "task_save", "task_delete", "task_run", "task_step", "task_pause", "task_end"]

# the tools the server answers (they use IO)
def Teach.io_names() -> List<&2, String>:
  ["task_run", "task_step", "task_pause", "task_end"]

def Teach.row.params(ps: List<&2, TK.TkParam>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case TK.TkParam{n, _, _, _} <> t:
      J.Str{n} <> Teach.row.params(t)

def Teach.row.k(m: Maybe<&2, TK.TkTask>, +slug: String) -> J.Json:
  match m:
    case Some{TK.TkTask{_, +name, +description, _, _, params, _, _, _, _}}:
      J.Json.obj([J.KV{"slug", J.Str{slug}}, J.KV{"name", J.Str{name}}, J.KV{"description", J.Str{description}}, J.KV{"params", J.Json.arr(Teach.row.params(params))}])
    case None{}:
      J.Json.obj([J.KV{"slug", J.Str{slug}}, J.KV{"name", J.Str{slug}}])

def Teach.rows.json(rs: List<&2, BT.TRow>) -> List<&2, J.Json>:
  match rs:
    case Nil{}:
      Nil{}
    case BT.TRow{+slug, json} <> t:
      Teach.row.k(TK.Task.read(json), slug) <> Teach.rows.json(t)

def Teach.params.names(ps: List<&2, TK.TkParam>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case TK.TkParam{n, _, _, _} <> t:
      n <> Teach.params.names(t)

def Teach.line.of(m: Maybe<&2, TK.TkTask>, +slug: String) -> String:
  match m:
    case Some{TK.TkTask{_, +name, +description, _, _, +params, _, _, _, _}}:
      "- " ++ slug ++ ": " ++ name ++ Bool.pick(String, String.is_empty(description), "", " - " ++ Teach.cut(description, 120n))
        ++ Bool.pick(String, List.is_empty(&2, TK.TkParam, params), "", " (params: " ++ String.join(Teach.params.names(params), ", ") ++ ")")
    case None{}:
      "- " ++ slug

def Teach.block.go(rs: List<&2, BT.TRow>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case BT.TRow{+slug, json} <> t:
      Teach.line.of(TK.Task.read(json), slug) <> Teach.block.go(t)

def Teach.block.k(+ls: List<&2, String>) -> String:
  Bool.pick(String, List.is_empty(&2, String, ls), "", "<tasks>\n" ++ String.join(ls, "\n") ++ "\n</tasks>")

# the <tasks> block in the bot's turn: its task skills (empty when none)
def Teach.block(rs: List<&2, BT.TRow>) -> String:
  Teach.block.k(Teach.block.go(rs))

# a task as the tools show it: its JSON
def Teach.get.res(m: Maybe<&2, String>) -> Maybe<&2, J.Json>:
  match m:
    case Some{t}:
      J.Json.parse(t)
    case None{}:
      None{}

# save: merge onto the existing task or make one
type Saved is Data:
  Saved{ok: Bool, err: String, slug: String, json: String}

def Teach.saved.bad(err: String) -> Saved:
  Saved{False{}, err, "", ""}

def Teach.has_arr(m: Maybe<&2, J.Json>) -> Bool:
  match m:
    case Some{J.Arr{_}}:
      True{}
    case _:
      False{}

def Teach.or_str(m: Maybe<&2, J.Json>, d: String) -> String:
  match m:
    case Some{J.Str{s}}:
      s
    case _:
      d

def Teach.params.pick(yes: Bool, a: List<&2, TK.TkParam>, b: List<&2, TK.TkParam>) -> List<&2, TK.TkParam>:
  match yes:
    case True{}:
      a
    case False{}:
      b

def Teach.steps.pick(yes: Bool, a: List<&2, TK.TkStep>, b: List<&2, TK.TkStep>) -> List<&2, TK.TkStep>:
  match yes:
    case True{}:
      a
    case False{}:
      b

def Teach.start.of(+start: String, steps: List<&2, TK.TkStep>) -> String:
  Bool.pick(String, String.is_empty(start), Teach.first_url(Teach.urls(steps, Nil{})), start)

def Teach.merge.k(slug: String, name: String, description: String, origin: String, +start: String, params: List<&2, TK.TkParam>, +steps: List<&2, TK.TkStep>, created: Nat, updated: Nat) -> TK.TkTask:
  TK.TkTask{slug, name, description, origin, Teach.start.of(start, steps), params, Teach.renumber(steps, 1n), Teach.urls(steps, Nil{}), created, updated}

def Teach.merge(base: TK.TkTask, +a: J.Json, +slug: String, +now: Nat) -> TK.TkTask:
  match base:
    case TK.TkTask{_, name, description, origin, start, params, steps, urls, created, _}:
      Teach.merge.k(slug, Teach.or_str(J.Json.get(a, "name"), name), Teach.or_str(J.Json.get(a, "description"), description), origin, Teach.or_str(J.Json.get(a, "start"), start),
        Teach.params.pick(Teach.has_arr(J.Json.get(a, "params")), TK.Tk.params.of(J.Json.elems(J.Json.get_obj(a, "params"))), params),
        Teach.steps.pick(Teach.has_arr(J.Json.get(a, "steps")), TK.Tk.steps.of(J.Json.elems(J.Json.get_obj(a, "steps"))), steps),
        created, now)

def Teach.task.name(t: TK.TkTask) -> String:
  match t:
    case TK.TkTask{_, n, _, _, _, _, _, _, _, _}:
      n

def Teach.task.slug(t: TK.TkTask) -> String:
  match t:
    case TK.TkTask{s, _, _, _, _, _, _, _, _, _}:
      s

def Teach.save.check(+t: TK.TkTask) -> Saved:
  Bool.pick(Saved, String.is_empty(String.trim(Teach.task.name(t))), Teach.saved.bad("name the task"),
    Bool.pick(Saved, Nat.is_gt(List.length(&2, TK.TkStep, TK.Tk.steps_of(t)), 200n), Teach.saved.bad("at most 200 steps"),
      Saved{True{}, "", Teach.task.slug(t), Teach.task.text(t)}))

# the slug a save uses: the one given (made a slug), else the name's
def Teach.slug.of(+given: String, +name: String) -> String:
  TK.Task.slug(Bool.pick(String, String.is_empty(String.trim(given)), name, given))

def Teach.blank_task(+slug: String, +now: Nat) -> TK.TkTask:
  TK.TkTask{slug, "", "", "tell", "", Nil{}, Nil{}, Nil{}, now, now}

def Teach.base.of(m: Maybe<&2, TK.TkTask>, +slug: String, +now: Nat) -> TK.TkTask:
  match m:
    case Some{t}:
      t
    case None{}:
      Teach.blank_task(slug, now)

def Teach.save.read(m: Maybe<&2, String>) -> Maybe<&2, TK.TkTask>:
  match m:
    case Some{t}:
      TK.Task.read(t)
    case None{}:
      None{}

def Teach.save.m(+a: J.Json, +slug: String, m: Maybe<&2, TK.TkTask>, +now: Nat) -> Saved:
  Teach.save.check(Teach.merge(Teach.base.of(m, slug, now), a, slug, now))

def Teach.save.k(+a: J.Json, +slug: String, existing: Maybe<&2, String>, +now: Nat) -> Saved:
  Teach.save.m(a, slug, Teach.save.read(existing), now)

# `existing` is the stored JSON of the slug's task, if any
def Teach.save(existing: Maybe<&2, String>, +a: J.Json, +now: Nat) -> Saved:
  Teach.save.k(a, Teach.slug.of(J.Json.get_str(a, "slug", ""), J.Json.get_str(a, "name", "")), existing, now)

# Running
# -------

# the object's fields as name/value pairs
def Teach.kvs.of(f: J.Json) -> List<&2, J.KV>:
  match f:
    case J.Field{k, v, t}:
      J.KV{k, v} <> Teach.kvs.of(t)
    case _:
      Nil{}

def Teach.args.of(o: J.Json) -> List<&2, J.KV>:
  match o:
    case J.Obj{fs}:
      Teach.kvs.of(fs)
    case _:
      Nil{}

def Teach.locs.json(ls: List<&2, String>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      J.Str{l} <> Teach.locs.json(t)

def Teach.plan.k(risk: String, s: TK.TkStep) -> J.Json:
  match s:
    case TK.TkStep{n, act, target, v, wu, wt, url, _, note}:
      J.Json.obj([J.KV{"n", J.Json.nat(n)}, J.KV{"do", J.Str{act}}, J.KV{"locators", J.Json.arr(Teach.locs.json(TK.Task.locators(target)))},
        J.KV{"value", TK.Tk.value.json(v)}, J.KV{"url", J.Str{url}}, J.KV{"wait", J.Json.obj([J.KV{"url", J.Str{wu}}, J.KV{"text", J.Str{wt}}])},
        J.KV{"risk", J.Str{risk}}, J.KV{"note", J.Str{note}}])

# one step of a filled plan with the selectors to try, best first
def Teach.plan.step(+s: TK.TkStep) -> J.Json:
  Teach.plan.k(TK.Task.risk(s), s)

def Teach.plan.steps(xs: List<&2, TK.TkStep>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case s <> t:
      Teach.plan.step(s) <> Teach.plan.steps(t)

def Teach.plan(t: TK.TkTask) -> J.Json:
  match t:
    case TK.TkTask{slug, name, description, _, start, _, steps, _, _, _}:
      J.Json.obj([J.KV{"slug", J.Str{slug}}, J.KV{"name", J.Str{name}}, J.KV{"description", J.Str{description}}, J.KV{"start", J.Str{start}}, J.KV{"steps", J.Json.arr(Teach.plan.steps(steps))}])

# run ids are <slug>-<seconds>; the slug is read back from the id
def Teach.run.id(+slug: String, +at: Nat) -> String:
  slug ++ "-" ++ Nat.show(at)

def Teach.run.slug.k(+ps: List<&2, String>) -> String:
  String.join(List.take(&2, String, ps, Nat.sub(List.length(&2, String, ps), 1n)), "-")

def Teach.run.slug(+run: String) -> String:
  Teach.run.slug.k(String.split(run, '-'))

# a name safe to use as one path part: letters, digits, - _ and (when
# `dot`) one inner . ; never empty, never ..
def Teach.part.ok(+u: U32, dot: Bool) -> Bool:
  Bool.or(Teach.pname.ok(u), Bool.or(U32.is_eq(u, 45), Bool.and(dot, U32.is_eq(u, 46))))

def Teach.part.char(c: Char, +dot: Bool) -> Bool:
  match c:
    case Chr{+u}:
      Teach.part.ok(u, dot)

def Teach.part.go(s: String, +dot: Bool, ok: Bool) -> Bool:
  match s:
    case SNil{}:
      ok
    case SCon{c, t}:
      Teach.part.go(t, dot, Bool.and(ok, Teach.part.char(c, dot)))

def Teach.part(+s: String, dot: Bool) -> Bool:
  Bool.and(Bool.not(String.is_empty(s)), Bool.and(Bool.not(String.contains(s, "..")), Bool.and(Nat.is_le(String.length(s), 120n), Teach.part.go(s, dot, True{}))))

# a run id, a screenshot's file name
def Teach.run.ok(+run: String) -> Bool:
  Bool.and(Teach.part(run, False{}), Bool.not(String.is_empty(Teach.run.slug(run))))

def Teach.file.ok(+f: String) -> Bool:
  Teach.part(f, True{})

# where a run's files are, relative to the bot's folder
def Teach.run.dir(+run: String) -> String:
  "tasks/" ++ Teach.run.slug(run) ++ "/runs/" ++ run

# the screenshot of a pause, and of a step
def Teach.pause.file(+n: Nat) -> String:
  "pause-" ++ Nat.show(n) ++ ".jpg"

def Teach.step.file(+n: Nat) -> String:
  Nat.show(n) ++ ".jpg"

# a run's state, kept as an Act's text
def Teach.act.text(+slug: String, +run: String, +status: String, n: Nat, +summary: String, shots: List<&2, String>) -> String:
  J.Json.show(J.Json.obj([J.KV{"slug", J.Str{slug}}, J.KV{"run", J.Str{run}}, J.KV{"status", J.Str{status}}, J.KV{"n", J.Json.nat(n)},
    J.KV{"summary", J.Str{summary}}, J.KV{"shots", J.Json.arr(Teach.locs.json(shots))}]))

# The pause ask
# -------------

# the answers a pause offers: a risky step goes on only by confirm
def Pause.options(risky: Bool) -> String:
  Bool.pick(String, risky, "confirm\nskip\nstop", "continue\nskip\nstop")

def Pause.shot_url(+bot: String, +run: String, +n: Nat) -> String:
  "/bots/" ++ bot ++ "/tasks/shot?run=" ++ run ++ "&n=" ++ Nat.show(n)

def Pause.detail(+bot: String, +run: String, +slug: String, +n: Nat, +kind: String, +question: String, risky: Bool) -> String:
  J.Json.show(J.Json.obj([J.KV{"bot", J.Str{bot}}, J.KV{"run", J.Str{run}}, J.KV{"slug", J.Str{slug}}, J.KV{"n", J.Json.nat(n)}, J.KV{"kind", J.Str{kind}},
    J.KV{"question", J.Str{question}}, J.KV{"risky", J.Flag{risky}}, J.KV{"shot", J.Str{Pause.shot_url(bot, run, n)}}]))

# what the bot hears when the pause is answered
def Pause.reply(+answer: String) -> String:
  Bool.pick(String, String.eq(answer, "confirm"), "The person confirmed. Do the step, then go on. The browser is yours again.",
    Bool.pick(String, String.eq(answer, "continue"), "The person says continue: go on from this step. The browser is yours again.",
      Bool.pick(String, String.eq(answer, "skip"), "The person says skip this step: go on with the next one. The browser is yours again.",
        Bool.pick(String, String.eq(answer, "stop"), "The person says stop: end the task now with task_end (status stopped).",
          "The person answered: " ++ answer))))

# a pause alerts once per run and step: not again for the same key
def Pause.alerts(s: TeachSt, +run: String, +n: Nat) -> Bool:
  Bool.not(Teach.alerted(s, TK.Run.alert_key(run, n)))

def Pause.title(+bot: String) -> String:
  bot ++ " needs you"

# the newest draft step as JSON text ("" when none): a line of the draft file
def Teach.last.go(xs: List<&2, TK.TkStep>, acc: String) -> String:
  match xs:
    case Nil{}:
      acc
    case s <> t:
      Teach.last.go(t, J.Json.show(TK.Tk.step.json(s)))

def Teach.last(b: TeachBot) -> String:
  Teach.last.go(Teach.bot.steps(b), "")

# a tools/call of one of the tools the server answers
def Teach.tool.is(+o: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(o, "method", ""), "tools/call"),
    TK.Tk.elem(Teach.io_names(), J.Json.get_str(J.Json.get_obj(o, "params"), "name", "")))
