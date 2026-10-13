import Base
import ./json.bend as J

# Browser task skills, pure (docs/teach.md). A bot learns a task from the
# person (a recording in its Browser tab) or from chat and keeps it as a
# task: steps aimed at elements by meaning (role, name, label, ...), never
# by coordinates. This file holds the task and its JSON, turning the
# recorder's events into steps, filling parameters, the selectors to try,
# which steps are risky, the pixel scaling of the person's clicks, who
# drives the page, and the run's state machine (a pause alerts once).
#
# A secret's value is never part of a task: the recorder sends only
# {secret: true, label}, and of_record drops any "value" beside it.

# Types
# -----

type TkTarget is Data:
  TkTarget{role: String, name: String, label: String, placeholder: String, text: String, testid: String, css: String, nth: Nat, frame: String, near: String}

# a step's typed value: a literal, a parameter's name, a secret the person
# gives each time (its label only), or none
type TkValue is Data:
  VLit{text: String}
  VParam{name: String}
  VSecret{label: String}
  VNone{}

# `act` is the JSON's "do": navigate, click, type, select, press, scroll,
# wait, note, check
type TkStep is Data:
  TkStep{n: Nat, act: String, target: TkTarget, value: TkValue, wait_url: String, wait_text: String, url: String, risk: String, note: String}

type TkParam is Data:
  TkParam{name: String, label: String, kind: String, example: String}

type TkTask is Data:
  TkTask{slug: String, name: String, description: String, origin: String, start: String, params: List<&2, TkParam>, steps: List<&2, TkStep>, urls: List<&2, String>, created: Nat, updated: Nat}

# a point in pixels
type TkPt is Data:
  TkPt{x: Nat, y: Nat}

# Strings
# -------

def Tk.first(+a: String, +b: String) -> String:
  Bool.pick(String, String.is_empty(a), b, a)

# JSON-ish escaping of a selector's quoted text: \ and " get a backslash
def Tk.esc.c(q: Bool, c: Char, rest: String) -> String:
  match q:
    case True{}:
      SCon{'\\', SCon{c, rest}}
    case False{}:
      SCon{c, rest}

def Tk.esc(s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{+c, t}:
      Tk.esc.c(Bool.or(Char.is_eq(c, '"'), Char.is_eq(c, '\\')), c, Tk.esc(t))

def Tk.quoted(s: String) -> String:
  "\"" ++ Tk.esc(s) ++ "\""

# Slugs
# -----

def Tk.slug.alnum(+u: U32) -> Bool:
  Bool.or(Bool.and(U32.is_ge(u, 97), U32.is_le(u, 122)), Bool.and(U32.is_ge(u, 48), U32.is_le(u, 57)))

def Tk.slug.lower(+u: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ge(u, 65), U32.is_le(u, 90)), U32.add(u, 32), u)

# the walk's state: whether the last char written was a dash (or the start),
# and what is written, backwards
type TkSl is Data:
  TkSl{prev: Bool, acc: String}

def Tk.slug.dash(prev: Bool, acc: String) -> TkSl:
  match prev:
    case True{}:
      TkSl{True{}, acc}
    case False{}:
      TkSl{True{}, SCon{'-', acc}}

def Tk.slug.step.k(ok: Bool, u: U32, prev: Bool, acc: String) -> TkSl:
  match ok:
    case True{}:
      TkSl{False{}, SCon{Char.from_u32(u), acc}}
    case False{}:
      Tk.slug.dash(prev, acc)

def Tk.slug.step.u(+u: U32, prev: Bool, acc: String) -> TkSl:
  Tk.slug.step.k(Tk.slug.alnum(u), u, prev, acc)

def Tk.slug.step(c: Char, st: TkSl) -> TkSl:
  match st:
    case TkSl{prev, acc}:
      Tk.slug.step.u(Tk.slug.lower(Char.to_u32(c)), prev, acc)

def Tk.slug.go(s: String, st: TkSl) -> TkSl:
  match s:
    case SNil{}:
      st
    case SCon{c, t}:
      Tk.slug.go(t, Tk.slug.step(c, st))

def Tk.slug.acc(st: TkSl) -> String:
  match st:
    case TkSl{_, acc}:
      acc

def Tk.slug.trim.k(dash: Bool, c: Char, t: String) -> String:
  match dash:
    case True{}:
      t
    case False{}:
      SCon{c, t}

# a backwards string without its leading dash
def Tk.slug.trim(s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{+c, t}:
      Tk.slug.trim.k(Char.is_eq(c, '-'), c, t)

def Tk.slug.fwd(name: String) -> String:
  String.reverse(Tk.slug.trim(Tk.slug.acc(Tk.slug.go(String.take(name, 200n), TkSl{True{}, ""}))))

# at most 48 chars, no dash at the end
def Tk.slug.cut(s: String) -> String:
  String.reverse(Tk.slug.trim(String.reverse(String.take(s, 48n))))

# "task" for nothing at all
def Tk.slug.or(s: String) -> String:
  match s:
    case SNil{}:
      "task"
    case SCon{c, t}:
      SCon{c, t}

# a name as a slug: lowercase, [a-z0-9-], dashes collapsed and trimmed, at
# most 48 chars, "task" if nothing is left
def Task.slug(name: String) -> String:
  Tk.slug.or(Tk.slug.cut(Tk.slug.fwd(name)))

# Values and parameters
# ---------------------

def Tk.braces(s: String) -> String:
  "{{" ++ s ++ "}}"

def Tk.value.lit.k(empty: Bool, s: String) -> TkValue:
  match empty:
    case True{}:
      VNone{}
    case False{}:
      VLit{s}

def Tk.value.lit(+s: String) -> TkValue:
  Tk.value.lit.k(String.is_empty(s), s)

def Tk.value.k(param: Bool, +s: String) -> TkValue:
  match param:
    case True{}:
      VParam{String.take(String.drop(s, 2n), Nat.sub(String.length(s), 4n))}
    case False{}:
      Tk.value.lit(s)

# a JSON string as a value: "{{name}}" is a parameter, "" none
def Tk.value.str(+s: String) -> TkValue:
  Tk.value.k(Bool.and(Bool.and(String.starts_with(s, "{{"), String.ends_with(s, "}}")), Nat.is_ge(String.length(s), 5n)), s)

def Tk.value.secret(label: String) -> TkValue:
  VSecret{label}

def Tk.value.j(+j: J.Json) -> TkValue:
  match j:
    case J.Str{t}:
      Tk.value.str(t)
    case J.Obj{_}:
      Tk.value.secret(J.Json.get_str(j, "secret", ""))
    case _:
      VNone{}

def Tk.value.of(m: Maybe<&2, J.Json>) -> TkValue:
  match m:
    case Some{j}:
      Tk.value.j(j)
    case None{}:
      VNone{}

def Tk.value.json(v: TkValue) -> J.Json:
  match v:
    case VLit{t}:
      J.Str{t}
    case VParam{n}:
      J.Str{Tk.braces(n)}
    case VSecret{l}:
      J.Json.obj([J.KV{"secret", J.Str{l}}])
    case VNone{}:
      J.Str{""}

# JSON
# ----

def Tk.target.of(+t: J.Json, frame: String) -> TkTarget:
  TkTarget{J.Json.get_str(t, "role", ""), J.Json.get_str(t, "name", ""), J.Json.get_str(t, "label", ""), J.Json.get_str(t, "placeholder", ""), J.Json.get_str(t, "text", ""), J.Json.get_str(t, "testid", ""), J.Json.get_str(t, "css", ""), J.Json.get_nat(t, "nth", 0n), Tk.first(frame, J.Json.get_str(t, "frame", "")), J.Json.get_str(t, "near", "")}

def Tk.target.json(t: TkTarget) -> J.Json:
  match t:
    case TkTarget{role, name, label, placeholder, text, testid, css, nth, frame, near}:
      J.Json.obj([J.KV{"role", J.Str{role}}, J.KV{"name", J.Str{name}}, J.KV{"label", J.Str{label}}, J.KV{"placeholder", J.Str{placeholder}}, J.KV{"text", J.Str{text}}, J.KV{"testid", J.Str{testid}}, J.KV{"css", J.Str{css}}, J.KV{"nth", J.Json.nat(nth)}, J.KV{"frame", J.Str{frame}}, J.KV{"near", J.Str{near}}])

def Tk.step.of(+j: J.Json) -> TkStep:
  TkStep{J.Json.get_nat(j, "n", 0n), J.Json.get_str(j, "do", ""), Tk.target.of(J.Json.get_obj(j, "target"), ""), Tk.value.of(J.Json.get(j, "value")), J.Json.get_str(J.Json.get_obj(j, "wait"), "url", ""), J.Json.get_str(J.Json.get_obj(j, "wait"), "text", ""), J.Json.get_str(j, "url", ""), J.Json.get_str(j, "risk", "none"), J.Json.get_str(j, "note", "")}

def Tk.step.json(s: TkStep) -> J.Json:
  match s:
    case TkStep{n, act, target, value, wait_url, wait_text, url, risk, note}:
      J.Json.obj([J.KV{"n", J.Json.nat(n)}, J.KV{"do", J.Str{act}}, J.KV{"target", Tk.target.json(target)}, J.KV{"value", Tk.value.json(value)}, J.KV{"wait", J.Json.obj([J.KV{"url", J.Str{wait_url}}, J.KV{"text", J.Str{wait_text}}])}, J.KV{"url", J.Str{url}}, J.KV{"risk", J.Str{risk}}, J.KV{"note", J.Str{note}}])

def Tk.steps.of(xs: List<&2, J.Json>) -> List<&2, TkStep>:
  match xs:
    case Nil{}:
      Nil{}
    case +j <> t:
      Tk.step.of(j) <> Tk.steps.of(t)

def Tk.steps.json(xs: List<&2, TkStep>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case s <> t:
      Tk.step.json(s) <> Tk.steps.json(t)

def Tk.param.of(+j: J.Json) -> TkParam:
  TkParam{J.Json.get_str(j, "name", ""), J.Json.get_str(j, "label", ""), J.Json.get_str(j, "kind", "text"), J.Json.get_str(j, "example", "")}

def Tk.param.json(p: TkParam) -> J.Json:
  match p:
    case TkParam{name, label, kind, example}:
      J.Json.obj([J.KV{"name", J.Str{name}}, J.KV{"label", J.Str{label}}, J.KV{"kind", J.Str{kind}}, J.KV{"example", J.Str{example}}])

def Tk.params.of(xs: List<&2, J.Json>) -> List<&2, TkParam>:
  match xs:
    case Nil{}:
      Nil{}
    case +j <> t:
      Tk.param.of(j) <> Tk.params.of(t)

def Tk.params.json(xs: List<&2, TkParam>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case p <> t:
      Tk.param.json(p) <> Tk.params.json(t)

def Tk.urls.of(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case J.Str{u} <> t:
      u <> Tk.urls.of(t)
    case _ <> t:
      Tk.urls.of(t)

def Tk.urls.json(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case u <> t:
      J.Str{u} <> Tk.urls.json(t)

def Tk.task.of(+o: J.Json) -> TkTask:
  TkTask{J.Json.get_str(o, "slug", ""), J.Json.get_str(o, "name", ""), J.Json.get_str(o, "description", ""), J.Json.get_str(o, "origin", ""), J.Json.get_str(o, "start", ""), Tk.params.of(J.Json.elems(J.Json.get_obj(o, "params"))), Tk.steps.of(J.Json.elems(J.Json.get_obj(o, "steps"))), Tk.urls.of(J.Json.elems(J.Json.get_obj(o, "urls"))), J.Json.get_nat(o, "created", 0n), J.Json.get_nat(o, "updated", 0n)}

def Tk.task.is_obj(+o: J.Json) -> Bool:
  match o:
    case J.Obj{_}:
      Bool.not(String.is_empty(J.Json.get_str(o, "slug", "")))
    case _:
      False{}

def Tk.task.k(ok: Bool, +o: J.Json) -> Maybe<&2, TkTask>:
  match ok:
    case True{}:
      Some{Tk.task.of(o)}
    case False{}:
      None{}

# a task from its JSON; None unless it is an object with a slug
def Task.parse(+o: J.Json) -> Maybe<&2, TkTask>:
  Tk.task.k(Tk.task.is_obj(o), o)

def Task.json(t: TkTask) -> J.Json:
  match t:
    case TkTask{slug, name, description, origin, start, params, steps, urls, created, updated}:
      J.Json.obj([J.KV{"v", J.Json.nat(1n)}, J.KV{"slug", J.Str{slug}}, J.KV{"name", J.Str{name}}, J.KV{"description", J.Str{description}}, J.KV{"origin", J.Str{origin}}, J.KV{"start", J.Str{start}}, J.KV{"params", J.Json.arr(Tk.params.json(params))}, J.KV{"steps", J.Json.arr(Tk.steps.json(steps))}, J.KV{"urls", J.Json.arr(Tk.urls.json(urls))}, J.KV{"created", J.Json.nat(created)}, J.KV{"updated", J.Json.nat(updated)}])

def Tk.read.k(m: Maybe<&2, J.Json>) -> Maybe<&2, TkTask>:
  match m:
    case Some{j}:
      Task.parse(j)
    case None{}:
      None{}

# a task from its stored JSON text
def Task.read(+text: String) -> Maybe<&2, TkTask>:
  Tk.read.k(J.Json.parse(text))

# The recorder's events
# ---------------------

# a typed value: a secret's is its label, whatever value came with it
def TkStep.value_of(secret: Bool, v: String, label: String) -> TkValue:
  match secret:
    case True{}:
      VSecret{label}
    case False{}:
      Tk.value.lit(v)

# a note's words are its text; the rest keep a value
def TkStep.value_for(note: Bool, secret: Bool, v: String, label: String) -> TkValue:
  match note:
    case True{}:
      VNone{}
    case False{}:
      TkStep.value_of(secret, v, label)

def TkStep.note_of(note: Bool, v: String) -> String:
  match note:
    case True{}:
      v
    case False{}:
      ""

# a submit is a click that sends something
def TkStep.act_of(+act: String) -> String:
  Bool.pick(String, String.eq(act, "submit"), "click", act)

def TkStep.risk_of(+act: String) -> String:
  Bool.pick(String, String.eq(act, "submit"), "submit", "none")

def TkStep.rec.k(+note: Bool, +act: String, +secret: Bool, +v: String, label: String, +r: J.Json, n: Nat) -> TkStep:
  TkStep{n, TkStep.act_of(act), Tk.target.of(J.Json.get_obj(r, "target"), J.Json.get_str(r, "frame", "")), TkStep.value_for(note, secret, v, label), "", "", J.Json.get_str(r, "url", ""), TkStep.risk_of(act), TkStep.note_of(note, v)}

def TkStep.rec(+act: String, secret: Bool, +v: String, label: String, +r: J.Json, n: Nat) -> TkStep:
  TkStep.rec.k(String.eq(act, "note"), act, secret, v, label, r, n)

def TkStep.label_of(+r: J.Json) -> String:
  Tk.first(J.Json.get_str(r, "label", ""), Tk.first(J.Json.get_str(J.Json.get_obj(r, "target"), "label", ""), J.Json.get_str(J.Json.get_obj(r, "target"), "name", "")))

# a record-event step of the browser helper as a step. A record with
# secret:true becomes a Secret(label) and its "value", if any, is ignored.
def TkStep.of_record(+r: J.Json, n: Nat) -> TkStep:
  TkStep.rec(J.Json.get_str(r, "do", ""), J.Json.get_bool(r, "secret", False{}), J.Json.get_str(r, "value", ""), TkStep.label_of(r), r, n)

# Parameters
# ----------

def Tk.arg.val(+v: J.Json) -> String:
  match v:
    case J.Str{s}:
      s
    case J.Num{r}:
      r
    case _:
      ""

def Tk.arg.hit(hit: Bool, v: J.Json, rest: Maybe<&2, String>) -> Maybe<&2, String>:
  match hit:
    case True{}:
      Some{Tk.arg.val(v)}
    case False{}:
      rest

def Tk.arg(+ps: List<&2, J.KV>, +name: String) -> Maybe<&2, String>:
  match ps:
    case Nil{}:
      None{}
    case J.KV{+k, v} <> t:
      Tk.arg.hit(String.eq(k, name), v, Tk.arg(t, name))

def Tk.has.k(m: Maybe<&2, String>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

def Tk.has(+ps: List<&2, J.KV>, name: String) -> Bool:
  Tk.has.k(Tk.arg(ps, name))

def Tk.elem(xs: List<&2, String>, +x: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.or(String.eq(h, x), Tk.elem(t, x))

def Tk.need.put(seen: Bool, n: String, acc: List<&2, String>) -> List<&2, String>:
  match seen:
    case True{}:
      acc
    case False{}:
      List.append(&2, String, acc, [n])

def Tk.need.val(v: TkValue, +acc: List<&2, String>) -> List<&2, String>:
  match v:
    case VParam{+n}:
      Tk.need.put(Tk.elem(acc, n), n, acc)
    case _:
      acc

def Tk.need.step(s: TkStep, acc: List<&2, String>) -> List<&2, String>:
  match s:
    case TkStep{_, _, _, v, _, _, _, _, _}:
      Tk.need.val(v, acc)

def Tk.need(xs: List<&2, TkStep>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      acc
    case s <> t:
      Tk.need(t, Tk.need.step(s, acc))

def Tk.missing.put(have: Bool, n: String, rest: List<&2, String>) -> List<&2, String>:
  match have:
    case True{}:
      rest
    case False{}:
      n <> rest

def Tk.missing.go(ns: List<&2, String>, +ps: List<&2, J.KV>) -> List<&2, String>:
  match ns:
    case Nil{}:
      Nil{}
    case +n <> t:
      Tk.missing.put(Tk.has(ps, n), n, Tk.missing.go(t, ps))

def Tk.steps_of(t: TkTask) -> List<&2, TkStep>:
  match t:
    case TkTask{_, _, _, _, _, _, steps, _, _, _}:
      steps

# the parameters a task's steps use that `ps` (name/value) does not give,
# each once, in order
def Task.missing(t: TkTask, +ps: List<&2, J.KV>) -> List<&2, String>:
  Tk.missing.go(Tk.need(Tk.steps_of(t), Nil{}), ps)

def Tk.fill.some(m: Maybe<&2, String>, n: String) -> TkValue:
  match m:
    case Some{s}:
      VLit{s}
    case None{}:
      VParam{n}

def Tk.fill.val(v: TkValue, +ps: List<&2, J.KV>) -> TkValue:
  match v:
    case VParam{+n}:
      Tk.fill.some(Tk.arg(ps, n), n)
    case _:
      v

def Tk.fill.step(s: TkStep, +ps: List<&2, J.KV>) -> TkStep:
  match s:
    case TkStep{n, act, target, v, wu, wt, url, risk, note}:
      TkStep{n, act, target, Tk.fill.val(v, ps), wu, wt, url, risk, note}

def Tk.fill.steps(xs: List<&2, TkStep>, +ps: List<&2, J.KV>) -> List<&2, TkStep>:
  match xs:
    case Nil{}:
      Nil{}
    case s <> t:
      Tk.fill.step(s, ps) <> Tk.fill.steps(t, ps)

def Tk.fill.task(t: TkTask, +ps: List<&2, J.KV>) -> TkTask:
  match t:
    case TkTask{slug, name, description, origin, start, params, steps, urls, created, updated}:
      TkTask{slug, name, description, origin, start, params, Tk.fill.steps(steps, ps), urls, created, updated}

def Tk.fill.k(ms: List<&2, String>, t: TkTask, +ps: List<&2, J.KV>) -> Result<&2, &2, String, TkTask>:
  match ms:
    case Nil{}:
      Done{Tk.fill.task(t, ps)}
    case n <> _:
      Fail{"missing parameter: " ++ n}

# a task with its parameters put in (every step's value a literal), or the
# first parameter that is missing
def Task.fill(+t: TkTask, +ps: List<&2, J.KV>) -> Result<&2, &2, String, TkTask>:
  Tk.fill.k(Task.missing(t, ps), t, ps)

# Selectors
# ---------

def Tk.nth.k(more: Bool, nth: Nat) -> String:
  match more:
    case True{}:
      " >> nth=" ++ Nat.show(nth)
    case False{}:
      ""

def Tk.nth(+nth: Nat) -> String:
  Tk.nth.k(Nat.is_gt(nth, 0n), nth)

def Tk.loc.add(have: Bool, s: String, rest: List<&2, String>) -> List<&2, String>:
  match have:
    case True{}:
      s <> rest
    case False{}:
      rest

def Tk.loc.both(+a: String, +b: String) -> Bool:
  Bool.not(Bool.or(String.is_empty(a), String.is_empty(b)))

def Tk.loc.has(+a: String) -> Bool:
  Bool.not(String.is_empty(a))

# Playwright selectors for a target, best first: role and name (the nth
# match when it is not the first), aria-label, placeholder, test id, text,
# then the css the recorder kept
def Task.locators(t: TkTarget) -> List<&2, String>:
  match t:
    case TkTarget{+role, +name, +label, +placeholder, +text, +testid, +css, nth, _, _}:
      Tk.loc.add(Tk.loc.both(role, name), "role=" ++ role ++ "[name=" ++ Tk.quoted(name) ++ "]" ++ Tk.nth(nth),
        Tk.loc.add(Tk.loc.has(label), "[aria-label=" ++ Tk.quoted(label) ++ "]",
          Tk.loc.add(Tk.loc.has(placeholder), "[placeholder=" ++ Tk.quoted(placeholder) ++ "]",
            Tk.loc.add(Tk.loc.has(testid), "[data-testid=" ++ Tk.quoted(testid) ++ "]",
              Tk.loc.add(Tk.loc.has(text), "text=" ++ Tk.quoted(text),
                Tk.loc.add(Tk.loc.has(css), css, Nil{}))))))

# Risk
# ----

def Tk.words(+s: String, ws: List<&2, String>) -> Bool:
  match ws:
    case Nil{}:
      False{}
    case +w <> t:
      Bool.or(String.contains(s, w), Tk.words(s, t))

def Tk.risk.delete() -> List<&2, String>:
  ["delete", "remove"]

def Tk.risk.purchase() -> List<&2, String>:
  ["place order", "checkout", "purchase", "buy", "pay", "submit payment"]

def Tk.risk.send() -> List<&2, String>:
  ["send", "transfer"]

def Tk.risk.confirm() -> List<&2, String>:
  ["confirm"]

def Tk.risk.pick.s(send: Bool, +s: String) -> String:
  match send:
    case True{}:
      "send"
    case False{}:
      Bool.pick(String, Tk.words(s, Tk.risk.confirm()), "confirm", "none")

def Tk.risk.pick.p(buy: Bool, +s: String) -> String:
  match buy:
    case True{}:
      "purchase"
    case False{}:
      Tk.risk.pick.s(Tk.words(s, Tk.risk.send()), s)

def Tk.risk.pick.d(del: Bool, +s: String) -> String:
  match del:
    case True{}:
      "delete"
    case False{}:
      Tk.risk.pick.p(Tk.words(s, Tk.risk.purchase()), s)

def Tk.risk.pick(+s: String) -> String:
  Tk.risk.pick.d(Tk.words(s, Tk.risk.delete()), s)

# the short strings that name what a step touches, lowercased
def Tk.risk.text(t: TkTarget) -> String:
  match t:
    case TkTarget{_, name, label, _, text, _, _, _, _, _}:
      String.to_lower(String.take(name ++ " " ++ label ++ " " ++ text, 80n))

def Tk.risk.acts(+act: String) -> Bool:
  Bool.or(String.eq(act, "click"), Bool.or(String.eq(act, "press"), String.eq(act, "submit")))

def Tk.risk.own(+risk: String) -> Bool:
  Bool.not(Bool.or(String.is_empty(risk), String.eq(risk, "none")))

def Tk.risk.words(acts: Bool, t: TkTarget) -> String:
  match acts:
    case True{}:
      Tk.risk.pick(Tk.risk.text(t))
    case False{}:
      "none"

def Tk.risk.k(own: Bool, acts: Bool, risk: String, t: TkTarget) -> String:
  match own:
    case True{}:
      risk
    case False{}:
      Tk.risk.words(acts, t)

# how risky a step is: its own mark if it has one; else a click, press or
# submit on something named pay, buy, send, delete, confirm...
def Task.risk(s: TkStep) -> String:
  match s:
    case TkStep{_, +act, target, _, _, _, _, +risk, _}:
      Tk.risk.k(Tk.risk.own(risk), Tk.risk.acts(act), risk, target)

# Frame pixels to viewport pixels
# -------------------------------

def Tk.scale.clamp(+a: Nat, +hi: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_gt(a, hi), hi, a)

def Tk.scale.div(zero: Bool, v: Nat, f: Nat, w: Nat) -> Nat:
  match zero:
    case True{}:
      v
    case False{}:
      Nat.div(Nat.mul(v, w), f)

def Tk.scale.one(+v: Nat, +f: Nat, +w: Nat) -> Nat:
  Tk.scale.clamp(Tk.scale.div(Nat.is_eq(f, 0n), v, f, w), Nat.sub(w, 1n))

# a click at (x, y) in a frame of fw by fh, as a point of a viewport of vw by vh
def Task.scale(x: Nat, y: Nat, fw: Nat, fh: Nat, vw: Nat, vh: Nat) -> TkPt:
  TkPt{Tk.scale.one(x, fw, vw), Tk.scale.one(y, fh, vh)}

# Who drives the page
# -------------------

# the bot, or the person: why ("teach" or "takeover")
type TkDrive is Data:
  DBot{}
  DPerson{why: String}

def Drive.can_bot(d: TkDrive) -> Bool:
  match d:
    case DBot{}:
      True{}
    case DPerson{_}:
      False{}

# A run
# -----

type TkRun is Data:
  Running{n: Nat}
  Waiting{n: Nat, kind: String, risky: Bool}
  Person{n: Nat}
  Ended{status: String}

type TkIn is Data:
  Pause{kind: String, risky: Bool}
  Answer{choice: String}
  HandBack{}
  Advance{}
  Finish{status: String}

# the answer as a number: confirm 1, continue 2, skip 3, takeover 4, stop 5
def Run.choice(+c: String) -> Nat:
  Bool.pick(Nat, String.eq(c, "confirm"), 1n,
    Bool.pick(Nat, String.eq(c, "continue"), 2n,
      Bool.pick(Nat, String.eq(c, "skip"), 3n,
        Bool.pick(Nat, String.eq(c, "takeover"), 4n,
          Bool.pick(Nat, String.eq(c, "stop"), 5n, 0n)))))

# continue goes on only when the step is not risky
def Run.cont(risky: Bool, n: Nat, kind: String) -> TkRun:
  match risky:
    case True{}:
      Waiting{n, kind, True{}}
    case False{}:
      Running{n}

def Run.wait(code: Nat, n: Nat, kind: String, risky: Bool) -> TkRun:
  match code:
    case 1n:
      Running{n}
    case 2n:
      Run.cont(risky, n, kind)
    case 3n:
      Running{Nat.add(n, 1n)}
    case 4n:
      Person{n}
    case 5n:
      Ended{"stopped"}
    case _:
      Waiting{n, kind, risky}

def Run.running.stop(code: Nat, n: Nat) -> TkRun:
  match code:
    case 5n:
      Ended{"stopped"}
    case _:
      Running{n}

def Run.running(i: TkIn, n: Nat) -> TkRun:
  match i:
    case Advance{}:
      Running{Nat.add(n, 1n)}
    case Finish{s}:
      Ended{s}
    case Pause{kind, risky}:
      Waiting{n, kind, risky}
    case Answer{c}:
      Run.running.stop(Run.choice(c), n)
    case HandBack{}:
      Running{n}

def Run.person.stop(code: Nat, n: Nat) -> TkRun:
  match code:
    case 5n:
      Ended{"stopped"}
    case _:
      Person{n}

def Run.person(i: TkIn, n: Nat) -> TkRun:
  match i:
    case HandBack{}:
      Running{n}
    case Answer{c}:
      Run.person.stop(Run.choice(c), n)
    case _:
      Person{n}

def Run.waiting(i: TkIn, n: Nat, kind: String, risky: Bool) -> TkRun:
  match i:
    case Answer{c}:
      Run.wait(Run.choice(c), n, kind, risky)
    case _:
      Waiting{n, kind, risky}

# the run after one input (see docs/teach.md)
def Run.step(st: TkRun, i: TkIn) -> TkRun:
  match st:
    case Running{n}:
      Run.running(i, n)
    case Waiting{n, kind, risky}:
      Run.waiting(i, n, kind, risky)
    case Person{n}:
      Run.person(i, n)
    case Ended{s}:
      Ended{s}

def Run.alert_key(run: String, n: Nat) -> String:
  "task-" ++ run ++ "-" ++ Nat.show(n)

def Run.alerts.in(after: TkRun) -> Bool:
  match after:
    case Waiting{_, _, _}:
      True{}
    case _:
      False{}

# whether a change of state asks the person: only on entering a pause, so a
# run that keeps waiting alerts once
def Run.alerts(before: TkRun, after: TkRun) -> Bool:
  match before:
    case Waiting{_, _, _}:
      False{}
    case _:
      Run.alerts.in(after)
