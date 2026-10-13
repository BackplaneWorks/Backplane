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

## Protocol (hub requests, info keys, asks)

Requests (client → hub, like `bots.routine`):

- `bots.browser {bot, op, ...}`: the person's input on the bot's page.
  `click {x, y}` and `mouse {type: down|move|up, x, y}` with x, y in
  thousandths of the frame (0..1000, `Task.scale` against the viewport);
  `type {text}`; `press {key}` (Playwright key names); `scroll {dy}`;
  `navigate {url}`; `back`; `reload`; `takeover` (the person drives); `handback`
  (the bot drives again). Input other than takeover is refused while the bot
  drives and no take over or teaching is on.
- `bots.teach {bot, op}`: `start` (the person drives, recording on),
  `note {text}`, `stop` (recording off, the draft kept for review),
  `discard`, `save {name, description, params: [{step, name, label}]}`
  (each param marks step n's typed value as `{{name}}`; a TaskSet, the
  bot woken with "[taught: <name>] ...").
- `bots.task {bot, op, slug, params?}`: `run` (wakes the bot with
  "[task: <slug>] Run the task skill <slug> with {params}"), `delete`
  (TaskDeleted).

Info (hub → clients, per bot):

- `drive.<bot>`: "" (the bot drives), "teach" or "takeover".
- `teach.<bot>`: "" , `rec <start secs> <steps>` while recording, `review`
  after stop until save or discard.
- `teach.draft.<bot>`: the draft steps as a JSON array (the record steps,
  numbered from 1, notes included) while recording or in review.

Asks: a pause opens `AskOpened{kind: "browser"}` on the bot's thread, tool
`task_pause`, detail a JSON object `{bot, run, slug, n, kind, question,
risky, shot}` (`shot` the hub URL of the pause's screenshot,
`/bots/<id>/tasks/shot?run=<run>&n=<n>`), options (the ask's `options`, one
per line) `confirm`, `skip`, `stop` for a risky pause, else `continue`, `skip`,
`stop`. Answering closes it; the person may take over first (`bots.browser
takeover`), and answering also hands back.

Runs: each run's steps go to its folder, and the run is an Act of tone
`task` on the bot's thread, text a JSON object `{slug, run, status, n,
summary, shots: [urls]}`, logged at its start and end.

## The hub's side

`src/core/teach.bend` (pure: the per-bot driver and draft, info text, saving
a draft as a task, the pause ask; laws `teach_*`; test `test/teach_test.bend`);
`src/server/server.bend`'s "Teaching a bot's browser" and "Task runs" (IO).
The state lives in the hub's `Cfg` (`BN.Bn.tch`), never in the event log, and
is sent as the info keys above (in every join's `info`, and as `{"t":"info"}`
when a bot's state changes). Recorder steps arrive as helper `record` events
(`Shot.msg` routes them to the hub as `BrowserEv`); input from the person is
scaled with `Task.scale` against the page's `status` size. A recording stops
by itself after ten minutes (the minute tick).

A bot's `preview_*` calls are answered "The person has the browser (teaching
or taking over); wait for them to hand it back." while `drive.<bot>` is set.

Task tools: `task_list`, `task_get`, `task_save`, `task_delete` are pure
(`Bots.call`); `task_run` (the filled plan, each step with its selectors),
`task_step`, `task_pause` and `task_end` are answered by the server. A run
is `<slug>-<seconds>`, its files `<home>/bots/<id>/tasks/<slug>/runs/<run>/`
(`log.jsonl`, `<n>.jpg`, `pause-<n>.jpg`); the newest twenty runs are kept.
`GET /bots/<id>/tasks/shot?run=<run>&n=<n>` (or `&f=<file>`) serves a
picture behind the same auth as `screen.jpg`, only from that bot's folder.
A pause alerts once per `task-<run>-<n>`: a desktop notification (kind
`ask`) to the desk used last, and an APNs alert whose payload is mutable and
carries the screenshot's path as a top-level `image`. Test:
`bun test/tools/teach_e2e.ts`.
