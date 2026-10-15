# oh-my-pi Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add oh-my-pi (`omp`) as a sixth agent provider, usable for threads and bots from the window, the web client and both phone apps, with approvals that work in every mode.

**Architecture:** A per-turn ACP process (`omp acp --approval-mode <mode>`), run the way Grok's runner runs (`Grok.run`, `acp.bend`). Approvals that omp sends as a form (`elicitation/create`, Approve/Deny) are answered through the existing approval card. A pure module, `src/core/omp.bend`, holds the mapping, argv, config steps, model parsing and the form test.

**Tech Stack:** Bend 2.0.36 (pure core, server, four clients); Bun/TypeScript for the e2e stand-in and recorded fixtures; omp 18.8.8 as the real agent.

**Spec:** docs/superpowers/specs/2026-10-15-omp-provider-design.md

## Global Constraints

- Provider id `omp`; command `omp`; turn argv `acp --approval-mode <mode>`; one process per turn.
- Approval mapping: Supervised → `always-ask`; AcceptEdits → `write`; AutoMode → `write`; FullAccess → `yolo`.
- Approval forms are accepted only when the Approve choice was given by the person; otherwise `decline`.
- `clientCapabilities.elicitation.form` is advertised only for omp; `Acp.init()` bytes stay identical for Grok.
- Grok's auth hint text stays `… (sign in once with \`grok login\`)`, byte for byte.
- No key value reaches a client, a log, or an agent prompt. omp reads its own login only.
- Persisted events keep their meaning; the provider string is additive; new providers are appended to the end of each provider list (codec index).
- Bend 2.0.36 rules from AGENTS.md. `scripts/check.sh` passes before every commit. LAWS.bend gains laws, none lost or weakened; PROOF.bend prints "All terms check."
- Tests use temp `BACKPLANE_HOME`, `HOME` and `PI_CODING_AGENT_DIR`. Fixtures live under `test/fixtures/omp/`.
- Every feature reaches the window, the web client and both phone apps; the phone bundle changes, so a TestFlight and an APK build follow the merge.

## Review Focus

1. An approval form with an unexpected shape (no `requestedSchema`, no `enum`, or a select that is not Approve/Deny) must get `decline`, never `accept`.
2. Deny, a dismissed card, or a timeout must refuse the write: `tool.done` with status failed, and no file created.
3. Supervised: each write and each shell command asks again; an Allow once never becomes a silent allow-all.
4. A lost session on resume (the session file gone) opens a fresh session and says so; it must not hang.
5. omp missing, or not logged in, must fail the turn at once with the install or login hint, not hang on `session/new`.

Each item gets its test in the owning task: 1 in Task 2 (form shape); 2 in Task 4 (the Deny reply) and Task 7 (no file after Deny); 3 in Task 7 (each write asks again); 4 and 5 in Task 3 (lost session, omp missing).

---

### Task 1: Pure omp module, laws, and recorded fixtures

**Files:**
- Create: `src/core/omp.bend`
- Create: `test/omp_test.bend`
- Create: `test/fixtures/omp/turn.jsonl`, `test/fixtures/omp/resume.jsonl` (agent lines of the recorded turn, from `/tmp/omp-fixtures/transcript.jsonl`, label `turn` and `resume`, with the version line first)
- Modify: `LAWS.bend` and `PROOF.bend` (new laws only)

**Interfaces:**
- Produces: `Omp.approval(r: M.RuntimeMode) -> String`; `Omp.args(r: M.RuntimeMode) -> List<&2, String>` (`["acp", "--approval-mode", Omp.approval(r)]`); `Omp.levels() -> List<&2, String>`; `Omp.models(text: String) -> List<&2, String>` (the `selector` of each entry with `kind` `chat`, from `omp models --json`, whose top level is `{"models": [...]}`); `Omp.install() -> String`; `Omp.hint(msg: String) -> String`.
- Consumes: `M.RuntimeMode` from `src/core/model.bend`.

- [ ] **Step 1: Write the failing tests**

In `test/omp_test.bend`, with the same shape as `test/agy_test.bend`:

```
C.Check.str("omp approval supervised", Omp.approval(M.Supervised{}), "always-ask")
C.Check.str("omp approval accept edits", Omp.approval(M.AcceptEdits{}), "write")
C.Check.str("omp approval auto", Omp.approval(M.AutoMode{}), "write")
C.Check.str("omp approval full access", Omp.approval(M.FullAccess{}), "yolo")
C.Check.str("omp args", String.join(Omp.args(M.Supervised{}), " "), "acp --approval-mode always-ask")
C.Check.str("omp levels", String.join(Omp.levels(), ","), "off,auto,low,medium,high,xhigh,max")
C.Check.str("omp models keeps chat only", String.join(Omp.models("{\"models\":[{\"kind\":\"chat\",\"selector\":\"a/b\"},{\"kind\":\"embedding\",\"selector\":\"c/d\"}]}"), ","), "a/b")
C.Check.str("omp models bad json", String.join(Omp.models("nope"), ","), "")
```

Add two laws to `LAWS.bend` (and their proofs to `PROOF.bend`): the mapping is total over `RuntimeMode`, and FullAccess is the only mode mapped to `yolo`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `bend test/omp_test.bend`
Expected: FAIL with an unbound name `Omp`.

- [ ] **Step 3: Implement `src/core/omp.bend`**

Imports follow `src/core/agy.bend`'s header. Keep it pure: no IO. `Omp.install()` returns: "Install oh-my-pi with `curl -fsSL https://omp.sh/install | sh` or `bun install -g @oh-my-pi/pi-coding-agent`, then sign in with `omp login`." `Omp.hint` appends " (sign in once with `omp login`)" to auth-looking messages, and returns the message unchanged otherwise, as `Acp.hint` does for Grok.

- [ ] **Step 4: Run tests to verify they pass**

Run: `bend test/omp_test.bend` and `bend PROOF.bend`
Expected: all checks pass; PROOF prints "All terms check."

- [ ] **Step 5: Commit**

`git commit -m "feat(omp): pure provider module, approval mapping, fixtures"`

### Task 2: ACP handshake and form approvals (`acp.bend`)

**Files:**
- Modify: `src/core/acp.bend` (`Acp.init` untouched; add `Acp.init.form`; add the `elicitation/create` branch in `Acp.request.k`; `Acp.line.k` method list)
- Test: `test/acp_test.bend` (add cases)

**Interfaces:**
- Produces: `Acp.init.form() -> String` (the `initialize` line with `"elicitation":{"form":{}}` in `clientCapabilities`); `Acp.elicit.approve(params: J.Json) -> Bool` (true when `requestedSchema.properties.value.enum` contains "Approve"); `Acp.elicit.reply(id: String, answer: String) -> String` (`accept` gives `{"action":"accept","content":{"value":"Approve"}}`; anything else gives `{"action":"decline"}`); `Acp.elicit.answer(body: String, answer: String) -> String` (the parked line's reply).
- Consumes: the existing `Acp.permit.route` pattern: `ask` True parks the raw line as an ask; `ask` False answers at once.

- [ ] **Step 1: Write the failing tests** (in `test/acp_test.bend`)

```
C.Check.str("acp init unchanged for grok", A.Acp.init(), <the current Acp.init() string, copied from the test's own run before the change>)
C.Check.str("acp init form advertises", yes(String.contains(A.Acp.init.form(), "\"elicitation\":{\"form\":{}}")), "yes")
C.Check.str("acp elicit parks in supervised", asks(A.Acp.line(a0, turn.ask(""), elicit("e1", approve_schema()))), <the raw line>)
C.Check.str("acp elicit accepts in full access", sendline(A.Acp.line(a0, turn.ask(""), …)), "")
C.Check.str("acp elicit approve at once", …, "{\"jsonrpc\":\"2.0\",\"id\":\"e1\",\"result\":{\"action\":\"accept\",\"content\":{\"value\":\"Approve\"}}}")
C.Check.str("acp elicit other form declines", …, "{\"jsonrpc\":\"2.0\",\"id\":\"e1\",\"result\":{\"action\":\"decline\"}}")
C.Check.str("acp elicit answer accept", A.Acp.elicit.answer(body, "accept"), "{\"jsonrpc\":\"2.0\",\"id\":\"e1\",\"result\":{\"action\":\"accept\",\"content\":{\"value\":\"Approve\"}}}")
C.Check.str("acp elicit answer deny", A.Acp.elicit.answer(body, "decline"), "{\"jsonrpc\":\"2.0\",\"id\":\"e1\",\"result\":{\"action\":\"decline\"}}")
C.Check.str("acp hint grok unchanged", A.Acp.hint("grok login", "Authentication required"), "Authentication required (sign in once with `grok login`)")
```

Build the test's `elicit(id, schema)` line by hand, with `method` `elicitation/create` and `params` `{mode:"form", sessionId, message, requestedSchema:{type:"object", properties:{value:{type:"string", enum:["Approve","Deny"]}}, required:["value"]}}`. Use `approve_schema()` for approvals and a schema whose enum is `["Yes","No"]` for the other-form case.

- [ ] **Step 2: Run tests to verify they fail**

Run: `bend test/acp_test.bend`
Expected: FAIL with unbound `Acp.init.form` and `Acp.elicit.*`.

- [ ] **Step 3: Implement**

`Acp.init.form` copies `Acp.init` and adds the `elicitation` object. `Acp.request.k` gets a third branch for `elicitation/create`; `Acp.line.k` lists the method (index 2). Full access (`ask` False) answers Approve at once; a form that is not an approval gets decline with no ask; an approval form with `ask` True parks as an ask, as permissions do. `Acp.elicit.answer` follows `Acp.permit.answer`. Helpers stay in `acp.bend`: `omp.bend` imports this file, never the reverse.

`Acp.hint` and `Acp.fail` take the login command from the caller: Grok passes `grok login`, omp passes `omp login`. Without this, omp's auth failures would tell the person to run `grok login`. Grok's output stays byte-identical.

- [ ] **Step 4: Run tests to verify they pass**

Run: `bend test/acp_test.bend` and `bend test/agy_test.bend` (Antigravity uses the same handler; it must still pass unchanged)
Expected: all pass.

- [ ] **Step 5: Commit**

`git commit -m "feat(acp): answer approval forms for omp, keep grok's handshake"`

### Task 3: Proto, runner, and config steps

**Files:**
- Modify: `src/core/msp.bend` (`Proto`: add `POmp{t: AC.AcpTurn, model: String, effort: String}`; cases in `Proto.line`, `Proto.init`, `Proto.cmd`, `Proto.name`, `Proto.install`, `Proto.gone`)
- Modify: `src/core/omp.bend` (the config-step sequence: `Omp.opened` and its reply handling)
- Modify: `src/server/server.bend` (`Omp.go`, placed beside `Grok.go`; the provider dispatch that calls `Agt.go` gets an `omp` case)
- Test: `test/omp_test.bend`

**Interfaces:**
- Produces: `Omp.go(+cfg: Cfg, +st: M.State, +thread: String, +cwd: String, +model: String, +session: String, +r: M.RuntimeMode, +text: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit)`, which calls `Grok.run(Omp.args(r), MS.POmp{…}, cwd, thread, gen, inbox)`.
- Consumes: `Grok.run`, `Grok.asks`, `Hub.mcp_url.g`, `Hub.prompt`, `AG.USkill.prompt`, `AG.Mention.expand("omp", …)`.
- Config order: after the `session/new` or `session/load` reply, send `session/set_config_option` for `model` (id 4) when chosen, wait for its reply; then `thinking` (id 5) when chosen, wait; only then `session/prompt` (id 3). Mirror `Agy.opened` and `Agy.resumed` in `src/core/agy.bend`.

- [ ] **Step 1: Write the failing tests** (in `test/omp_test.bend`)

Replay `test/fixtures/omp/turn.jsonl` through `Omp.line` with a turn that chose `anthropic/claude-haiku-5-5` and `high`. Expected, in order: the `session/new` reply sends `session/set_config_option` for model; its reply sends the thinking one; its reply sends `session/prompt`. Also: a turn with no model and no thinking sends `session/prompt` straight after `session/new`, with no config line. Check the methods in order with `method(...)` lists as in `test/agy_test.bend`.

Also: a `session/load` error gives a fresh `session/new` and a note in the turn, as Antigravity's resume does (review focus 4). A missing `omp` gives `turn.failed` with `Omp.install()` text, and a `session/new` error that names auth gives the `omp login` hint (review focus 5).

- [ ] **Step 2: Run tests to verify they fail**

Run: `bend test/omp_test.bend`
Expected: FAIL with unbound names.

- [ ] **Step 3: Implement**

Add the `POmp` constructor and its six cases; `Proto.cmd` gives `omp`, `Proto.name` gives `omp`, `Proto.install` gives `Omp.install()`, `Proto.init` gives `AC.Acp.init.form()`. `Omp.go` follows `Grok.go` (server.bend, about line 3709). Pass the model and the effort inside `POmp`, not through `AcpTurn`, so Grok's and Antigravity's constructors do not change.

- [ ] **Step 4: Run tests to verify they pass**

Run: `bend test/omp_test.bend`; `bend test/msp_test.bend`; `bend test/agy_test.bend`
Expected: pass; Muse and Antigravity unchanged.

- [ ] **Step 5: Commit**

`git commit -m "feat(omp): runner with config steps, one process per turn"`

### Task 4: Approval ask route for `elicitation/create` (`mcp.bend`)

**Files:**
- Modify: `src/core/mcp.bend` (`Mcp.handle` at about line 1020 and `Mcp.permission.reply` at about line 932: add `elicitation/create` to both method lists, route it through the approval-ask function that `session/request_permission` uses (`Grok.permit.k`'s pattern, which keeps its name), and reply through `Acp.elicit.answer`)
- Test: wherever `Mcp.handle` and `Mcp.permission.reply` are tested (find with `grep -rn "Mcp.handle" test/`)

**Interfaces:**
- Consumes: `Acp.elicit.approve`, `Acp.elicit.answer` (Task 2); `M.AskOpened{ask, thread, "approval", tool, detail, "", now}`.
- Produces: an approval ask for an approval form, with `tool` taken from the form's `message` and `detail` from the same text; the reply goes back to omp as Approve or decline.

- [ ] **Step 1: Write the failing tests**

An approval form for thread `t1` gives an `AskOpened` of kind `approval` and an empty reply line. Answering `accept` gives the Approve line; `decline` gives the decline line. A non-approval form gets a decline line and no ask.

- [ ] **Step 2: Run tests to verify they fail**

Run: the test file that covers `Mcp.handle`.
Expected: FAIL.

- [ ] **Step 3: Implement** the two method-list additions and the two branches.

- [ ] **Step 4: Run tests to verify they pass**, and `bend test/acp_test.bend` again (unchanged).

- [ ] **Step 5: Commit**

`git commit -m "feat(mcp): route omp approval forms through the approval card"`

### Task 5: Hub, effort, detection, models, and copy

**Files:**
- Modify: `src/core/hub.bend` (`Hub.run.k` dispatch at about line 421: append `"omp"` to the end of the list; `Hub.acp_like` at about line 2613: add `omp`)
- Modify: `src/core/agent.bend` (`Effort.levels` at about line 283: `omp` gives `Omp.levels()`; `Effort.args` needs no change: omp is not in its list, so it gives no argv)
- Modify: `src/server/server.bend` (`Models.detect` at about line 4664 adds `command -v omp`; `Models.present` at about line 4710 adds `omp` to its loop; `Models.found` (4518), `Models.text5` (4256) and `About` (4680) gain an omp entry from `omp models --json`, following the Antigravity entries)
- Modify: `src/core/mcp.bend` (lines 99 and 116, provider names in the tool text), `src/core/bothub.bend` (line 1596), `src/core/onboard.bend` (line 122)
- Test: `test/agent_test.bend` (add `effort omp levels`, `effort omp no argv`), `test/mbots_test.bend` (provider list `claude;codex;grok;muse;antigravity;omp;`)

**Interfaces:**
- Consumes: `Omp.levels`, `Omp.models`, `Omp.install` (Task 1).

- [ ] **Step 1: Write the failing tests**

`String.join(A.Effort.levels("omp"), ",")` equals `off,auto,low,medium,high,xhigh,max`. `String.join(A.Effort.args("omp", "high"), " ")` equals `""`. The bot provider list ends in `omp`.

- [ ] **Step 2: Run them to verify they fail**, then implement, then run them to pass (`bend test/agent_test.bend`, `bend test/mbots_test.bend`).

- [ ] **Step 3: Commit**

`git commit -m "feat(omp): hub dispatch, effort levels, detection and models"`

### Task 6: Chips in the four clients

**Files:**
- Modify: `src/core/client.bend` (`Chip.providers` at about line 3708: add `"@omp"`)
- Modify: `src/app/layout.bend` (about line 3478), `src/app/bots.bend` (about line 313)
- Modify: `src/web/view.bend` (about line 942), `src/web/bots.bend` (about line 124)
- Modify: `src/mobile/screen.bend` (about line 1179)
- Test: `test/composer_test.bend` (add `effort chips omp`, with the same shape as the grok case: `,off,auto,low,medium,high,xhigh,max`)

Native code names no provider, so no Swift or Kotlin change is needed.

- [ ] **Step 1: Write the failing test**, run it, implement, and run it again (`bend test/composer_test.bend`).

- [ ] **Step 2: Build and look at each surface** (house rule). Window: `scripts/build-app.sh src/app/main.bend build/backplane`, then `DISPLAY=:77 BACKPLANE_SNAP=/tmp/snap.ppm build/backplane --home /tmp/bp-x` and read the Settings chips in the frame. Web: `scripts/dev.sh web`, load the page. Phones: `scripts/build-mobile.sh`.

- [ ] **Step 3: Commit**

`git commit -m "feat(omp): provider chips in window, web and phones"`

### Task 7: End-to-end stand-in and shims

**Files:**
- Create: `test/tools/omp_standin.ts` (an ACP stand-in, modeled on `test/tools/agy_standin.ts`: `initialize`, `session/new` with `configOptions`, `session/set_config_option` acks, a `session/prompt` that streams text, makes a `tool_call` for a write, and sends an `elicitation/create` approval form)
- Create: `test/tools/omp_e2e.ts` (modeled on `test/tools/agy_e2e.ts`)
- Modify: each e2e script that writes fake `claude`, `codex`, `grok` and `muse` shims (about 50 files, found with `grep -rln '"muse"' test/tools`): add an `omp` shim (exit 1), or isolate `HOME` and `PI_CODING_AGENT_DIR`

- [ ] **Step 1: Write `omp_e2e.ts`** with these checks: a thread is created with `provider: "omp"`; a turn streams; a `write` shows an approval card in Supervised; Allow creates the file; Deny leaves no file; FullAccess shows no card.

- [ ] **Step 2: Run it and watch it fail, implement the stand-in, run it to pass**: `bun test/tools/omp_e2e.ts`

- [ ] **Step 3: Run the existing e2e scripts that were touched**: `bun test/tools/agy_e2e.ts`, `bun test/tools/bots_e2e.ts`. Expected: pass.

- [ ] **Step 4: Commit**

`git commit -m "test(omp): e2e with a stand-in agent, shims for omp"`

### Task 8: GATED: record one approval-form turn

**Do not run this task until the person says yes.** Ask first: one short Supervised turn on the default model (`anthropic/claude-haiku-5-5`), with a single write, costing a few tokens on their omp login.

**Files:**
- Create: `test/fixtures/omp/form.jsonl` (the `elicitation/create` line and the reply Backplane sent, from a turn recorded with the script from Task 1)
- Test: `test/omp_test.bend` (replay the form line; the reply is the Approve line when the person allows)

- [ ] **Step 1: Write the failing replay test** against `form.jsonl` (it fails until the fixture exists).

- [ ] **Step 2: Record the turn** (in `/tmp`, outside the repo; remove the session folder it creates under `~/.omp/agent/sessions/` afterwards), save the lines, and run the replay test: `bend test/omp_test.bend`. Expected: pass.

- [ ] **Step 3: Commit**

`git commit -m "test(omp): recorded approval form"`

### Task 9: Verification, docs, and delivery

**Files:**
- Modify: `AGENTS.md` (the "Agent providers" paragraph: add oh-my-pi), `docs/parity.md` (the provider list and the approvals note: omp answers approval forms through the card)

- [ ] **Step 1: Run the full checks**: `scripts/check.sh`, `scripts/test.sh`, `bun test/tools/omp_e2e.ts`, and `bend PROOF.bend` (prints "All terms check.").

- [ ] **Step 2: Build all four clients** (house rule): `scripts/build-app.sh src/app/main.bend build/backplane`, `scripts/build.sh` (web), `scripts/build-mobile.sh` (bridge and APK), and on a Mac `scripts/build-ios.sh archive` for TestFlight.

- [ ] **Step 3: Docs** as in the Files list.

- [ ] **Step 4: PR, merge, update the dev servers**: `scripts/dev.sh install h@backplane`, then `scripts/dev.sh install` here, last. Check each server's `/hello` shows the new version and that it is listening. Put the new iOS build on TestFlight; put the Android APK out per `mobile/README.md`.

- [ ] **Step 5: Commit**

`git commit -m "docs(omp): provider list and approvals"`
