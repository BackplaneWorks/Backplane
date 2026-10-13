# Teaching bots browser tasks

A bot's browser (the `preview_*` tools, `tools/browser/`, one page per bot)
can learn tasks: the person shows one in the bot's Browser tab, or tells it
in chat, and the bot keeps it as a **task skill** it can run again, on
demand or from a routine. When a run meets a login, CAPTCHA, 2FA, payment,
a risky step or a step that keeps failing, it pauses, alerts the person
once with a screenshot and a one-line question, and the person answers or
takes over the browser.

Pure logic lives in `src/core/task.bend` (laws `task_*`, `drive_*`,
`run_*`); test `test/task_test.bend`.

## Who drives the page

Each bot's page has a driver: the bot, or the person (teaching, or a take
over). While the person drives, the bot's `preview_*` calls answer "the
person has the browser" and do nothing. The Browser tab sends the person's
input as `bots.browser {bot, op, ...}` (ops `click` x/y in frame pixels,
`type`, `press`, `scroll`, `navigate`, `back`, `reload`); the hub scales
frame pixels to the viewport (`Task.scale`) and passes them to the helper.

## Recording

The helper's `record {on}` op (owner page only) turns on an in-page
recorder (`addInitScript` + `exposeBinding`). Only the person's input makes
DOM events while they drive, so the steps are theirs. Each element is
described semantically: role and accessible name, label, placeholder, text,
test id, a short stable CSS selector, nth among equal matches, frame URL,
nearest heading (`near`). No coordinates.

Secrets are masked in the page before anything leaves it: `type=password`,
autocomplete `*password`/`one-time-code`/`cc-*`, and names or labels like
card, cvv, cvc, iban, ssn, pin, otp, 2fa, security code send only
`{secret: true, label}`. No stored step ever carries a secret's value.

Notes (typed or dictated) become `note` steps. Recording stops at Stop &
save, Discard, 10 minutes, or the bot's deletion. On save the person names
it, describes it and marks typed values as parameters; the bot is then
woken to learn the task from the recording and refine it with `task_save`.

## The task skill

Events `TaskSet{bot, slug, json, at}` / `TaskDeleted{bot, slug, at}`, folded
into the bot world; a copy goes to `<home>/bots/<id>/tasks/<slug>.json`.

```json
{"v":1,"slug":"jlcpcb-order-status","name":"Check JLCPCB order status",
 "description":"...","origin":"show","start":"https://jlcpcb.com/...",
 "params":[{"name":"order","label":"Order number","kind":"text","example":"..."}],
 "steps":[{"n":1,"do":"click",
   "target":{"role":"button","name":"Search","label":"","placeholder":"","text":"","testid":"","css":"","nth":0,"frame":"","near":""},
   "value":"{{order}}","wait":{"url":"","text":""},"url":"...","risk":"none","note":""}],
 "urls":["..."],"created":0,"updated":0}
```

`do` is one of navigate, click, type, select, press, scroll, wait, note,
check. `value` is a literal, `{{param}}`, or `{"secret": "<label>"}`. `risk`
is none, send, purchase, delete, submit or confirm.

Runs log to `<home>/bots/<id>/tasks/<slug>/runs/<stamp>/` (`log.jsonl`,
JPEGs at key steps, never right after a secret field); the last 20 are kept.

## Tools (bots only)

`task_list`, `task_get {slug}`, `task_save {slug, ...}`, `task_delete
{slug}` (asks first), `task_run {slug, params}`, `task_step {run, n, ok,
note, shot}`, `task_pause {run, kind, question}`, `task_end {run, status,
summary}`.

## Stuck and take over

```
Running -pause(kind,q)-> Waiting(ask) -Take over-> PersonDrives -Hand back-> Running
                          |  '-Continue / Confirm-> Running   '-Skip step-> Running(n+1)
                          '-Stop task-> Ended(stopped)          Running -end-> Ended(ok|failed)
```

A pause is an ask of kind `browser`; the MCP reply is parked until it
closes. A risky step goes on only by Confirm. Each pause alerts once (key
`task-<run>-<n>`).
