# Coordinator mailroom

The promoted uberbot is the person's conversational interface. Workers and
bots keep their own source timelines and rooms. Their reports enter internal
mail; they do not appear as incoming worker messages in the coordinator's chat.

A committed report, including FYI, enters the owning conversation's durable
FIFO. A private triage run binds exactly one input; newer reports wait for
separate runs and never inject a different actionable ID into its model turn. This also works when generic `mail.on` automation is off.
Room reports file once, then route to their member bots; the coordinator's
member wake goes through triage. Direct reports, launched-thread subscriptions,
task results, bot results, and explicit wakes share this path. Source work is
never started, stopped, archived or approved merely by acknowledging mail.

The run uses the owner's memory identity and project/room scope, with a private
system prompt that does not inherit the owner's front-of-house persona.
The brief includes relevant durable memory, recent messages from the original
conversation, task board, live asks, decisions and source links. The read-only
`mail_context` tool searches/pages the owner's full stored conversation, even
beyond that brief. Its owner is derived from the authenticated live run;
caller-supplied target fields are rejected. It returns explicit errors for an
unavailable store. `memory_recall`/`memory_list` retain the owner's identity.

Triage chooses a durable notice receipt, authorized coordination in source
threads/rooms, or escalation. Meaningful completions, blockers, decisions,
actual approvals and uncertain outcomes require attention. FYI, unchanged holds
and duplicates stay quiet. Inputs are processed oldest first, including equal
timestamps: a newer FYI cannot hide an older ask. `mail_dispose` requires the
exact input; a bound run returns empty `next_input` after its one receipt,
while newer arrivals receive separate runs. A legacy persisted run retains its
pending-input walk during upgrade. A receipt records only that notice. On successful
completion with all inputs handled, the hub marks the quiet conversation done.
An actual open source approval cannot be disposed as a routine notice. Decisions
and receipts never supply permission, answer an approval or complete source work.

`escalate` requires the exact input in coordinator runs. Both `bot` and `person`
destinations create a separate private coordinator turn, with its own persisted
thread and provider session. They never start or inject into the owner's
original user conversation. The private lane inherits the owner's existing
provider/model, runtime, account binding, bot identity, memory and project scope;
the cheaper assessor remains a distinct first stage. It retains normal tools,
including `thread_send`, source approval handling and memory operations, without
granting new source permissions. The shared run projection hides these lanes;
their source timelines remain readable by their owner. A fresh lane never
resumes a previously stopped one.

All private commentary, streaming output and final text remain on that lane.
An empty final completes normally, with no user message and no visible-output
retry or re-prompt. Completed private SDKs retire using the existing run lifecycle;
pending approvals or work retain them. Merely writing a final answer, a decision,
or an escalation cannot publish into the person's chat.

The dedicated `tell_person(text, key)` tool is the sole publication path from a
private coordinator turn. It accepts only a live owning coordinator, fixes the
target to that owner's original conversation, rejects caller-supplied targets,
and commits an assistant message, provenance, an owner/event dedup marker and a
told-ledger receipt together. Repeating the same key, even from another private
turn or after replay, publishes nothing again. The key identifies an exact
event/version, not a broad topic. Bub must choose this explicitly and supply a
fresh contextual answer with actionable source links. The ordinary front-of-house
person-initiated conversation still receives normal model replies. A private
`notify_person` attempt returns an instruction to use `tell_person`; it does not
recursively enqueue another report. Original
asks remain at their sources for the person to answer. Source alerts from
managed workers/private runs/other bots are suppressed; browsing their timelines
and approval cards remains available.

A private triage tool refusal neither escalates nor acknowledges its source
input. It returns a denial with permitted evidence alternatives, without raw
arguments or a new approval. Prefer `thread_read` for a source's status and
available timeline; its `status.turn` remains available even when entries are
empty. A refused lookup is an internal evidence limitation, not a worker result
or proof that the person must authorize something. Failed/unknown triage sends
only a concise private limitation with source/run links; raw errors and final
responses stay in the run timeline. Bub resolves covered reports and evidence
limits quietly, surfacing only verified material outcomes, real decisions,
genuine source approvals or actionable coordination blockers. Actual pending
source and run approvals are retained; no denial supplies authorization.

The model inherits the app's existing text-generation provider/model settings,
so the person can use a cheaper configured model. Optional advanced per-bot
settings `run.provider.<bot>` and `run.model.<bot>` override it; known mismatched
model families are rejected in favor of the selected provider's configured
choice/default. Existing ordinary-bot model policy is unchanged. Starts use the
normal global fallback chain and provider checks. Failed, denied or unknown
triage forwards every still-pending input privately to its owner, with explicit
uncertainty, rather than claiming success. Exhausted provider fallback follows
the same path. A missing triggering body fails the run explicitly. A failed
mail-journal append reports delivery failure through the independent private
ride, without committing a header or producer acknowledgement; an event-log
write failure uses the existing visible service write-failure banner. Durable
assessment context also remains in the owner’s next system prompt if its
previous delivery turn failed. This does not relax any provider approval, sandbox or charter.

Delivery is recorded per owner/input. An optional escalation `key` correlates
the same exact event/version across direct, room and launch reports; a changed
result requires a new key. This records queued delivery, not that the person was
already told. Bub consults `told_check` and records `told_mark` after writing the
actual user update. There is no parallel FYI digest producer. Identical wake
subscriptions reuse their id, and launched report subscriptions cover additional
wake requests for the same source. Legacy duplicate launch/wake subscriptions
are consumed without producing a second report. Explicit wakes never bypass
triage. Receipts and delivery markers survive replay/restart; adopted runs and
explicit disabled restart continuation retain their existing behavior.

Validation belongs to the shared integration batch:

- `bend test/private_mailroom_test.bend`: separate coordinator turns, empty
  replies without user messages or re-prompts, private nonempty finals, explicit
  owner-fixed publication, cross-turn dedup and source approvals.
- `bend test/mailtriage_test.bend`: routing, scope, quiet receipts, asks, private
  escalation, event keys, read/done, model configuration, rooms, bot reports,
  duplicate subscriptions, failure, replay, restart and adopted runs.
- `bend test/mailtriage_policy_proof.bend`: bounded checks of the two additive
  universal laws; full `bend PROOF.bend` remains the integration gate.
- Existing `mailrun`, `mailrun_model`, `maildisposition`, `mailcontinuity` and
  `oprep` regression tests.
- `bun test/tools/mailtriage_e2e.ts APP WIRE_DIR`: isolated compiled hub and
  authenticated agent MCP, deterministic provider stand-ins, real store reads,
  original-conversation authoring, source approvals and restart. No live user
  state, credentials, external mail, network services or hardware are used.

A source patch or pure test pass does not establish runtime acceptance or live
activation. Run the runtime test on the exact combined build before local handoff.

Coordinator FIFO settings contain only input IDs; source bodies stay in the
mail journal and each bound run reads its exact body, including one older than
the recent history window. Dequeue and new RunSet commit together. Duplicate
queue kicks cannot start a second active run. Pending IDs survive restart;
restart continuation disabled and explicit interruptions keep the queue held.
A quiet receipt cannot mark newer queued reports seen/done. Rejected
dispositions return the actual current input and never claim processing.

Explicit Stop/thread_interrupt of a running turn writes a durable
`triage.hold.<owner/conv>` register at the stop boundary. The hold persists
between runs; stopping an idle turn remains a no-op. Its undisposed seed stays at
the front of the FIFO. New arrivals only append while held; neither an old
queue kick nor restart continuation clears the register. Internal fallback or
restart interruptions do not invent a user stop. Clearing the exact hold via
an explicit authenticated setting change is required before queue processing
can resume; an arrival or generic restart setting is insufficient. Source
approvals are untouched.
