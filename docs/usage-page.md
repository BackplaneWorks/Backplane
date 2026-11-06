# Usage page

Open Settings → Usage on the machine you want to inspect. The phone's App
settings also retains a separate Usage section for each paired machine,
including its stable key and connection status. Cached offline readings stay
with that machine.

Choose Tokens, Reported cost or Limits. The default Month (30 days) rolls
through the latest 30 UTC dates, including today. 1 week, 2 weeks and 4 weeks
show the latest 7, 14 and 28 dates respectively; they cross week, month and
year boundaries. Today shows just the current UTC date. Today is partial until
midnight UTC. This calendar month retains the current UTC calendar month and
its monthly model/project breakdown. Every range displays its exact dates.
The page reads existing
`TurnUsage`/`Tok.*` rollups. It requests no transcript import, history replay or
new telemetry collection. Anonymous installation telemetry is unrelated.

Recorded tokens include uncached input, cache read, cache write and output.
Reasoning is included in output and is never added twice. Missing fields are
unknown: a recorded subtotal of zero does not establish that every provider
reported zero. Coverage gives the number of turns that supplied token readings.
Reported cost includes only costs a provider supplied and shows how many turns
reported one. A reported zero stays zero; an absent cost stays unknown. These
are provider API-price figures, not subscription charges. The page introduces
no model price estimates.

Turns without usage events are absent, rather than zero. In the inspected
backend, Claude and Codex produce those events; this page does not add usage
collectors for other providers. The coverage denominator counts recorded usage
turns, not every turn or session the person ran.

The Tokens view has provider tabs, limited to providers advertised as available
in this machine's `models` capability arrays (the same source as the fallback
picker). There is no separate provider enable switch. A saved selection falls
back to the first available provider if availability changes. Tabs support
Left/Right, Home/End and normal Tab/Enter operation. No provider is inferred
from a model name. Cached availability can be stale when disconnected.

Each tab plots recorded tokens by UTC day for the selected period, including
missing dates. Rolling ranges count back from today without resetting at a
calendar boundary; This calendar month starts on the first; Today is one day.
Solid bars
show recorded token subtotals, a baseline marks a reported zero, and hatched
columns mean no token reading. Expand Daily readings for exact values and
coverage. The native phone pages use the same readings, with dim missing-day
columns and an expandable list. Plot height is reserved before painting.

Daily provider/project dimensions are additive rollups from accepted
`TurnUsage` events: `tok.<epoch>.dv.<YYYY-MM-DD>|<provider>` and
`tok.<epoch>.dp.<YYYY-MM-DD>|<project>`. They use the same token components and
coverage counters as existing totals. `daily.since` marks the start of these
readings in the active counting epoch. Existing persisted monthly totals are
never divided into daily guesses. Earlier days, providers without usage
collectors, and turns without readings remain unknown. The first counted day
may be partial. No history import or new network endpoint is added.

Below the graph, Top 5 projects ranks positive recorded token subtotals across
all models and providers, independently of the selected provider tab. This
calendar month uses the existing `p.<project>|<YYYY-MM>` totals; Today and
rolling ranges sum daily project totals and share their coverage limitation.
All selected days are summed per project before choosing the top five; model
totals and reasoning are not added again. Equal titles remain
separate project IDs. Missing catalog entries retain their ID as Unavailable
project; an empty project identity is Unknown project. Unknown readings and
zero-token projects do not fill ranking slots. Fewer than five projects are
shown when fewer have positive recorded consumption.

The calendar month view's provider/model and project details remain paged in
groups of twelve below these views. Rolling ranges and Today do not claim a
daily model breakdown. Recorded token totals and reported cost sum the selected
daily counters, including their coverage counts. A completely empty range stays
unknown. Limits and context retain their existing source and missing-data
behavior. No model pricing or cost-to-token conversion is used.

Focused validation: `test/usage_graph_test` with
`test/tools/usage_graph.mjs` exercises the production rollups, provider actions,
UTC boundaries, cross-model rankings, unavailable/unknown projects, reset
isolation, legacy missing dimensions, web nodes. `usage_graph_mobile_test` and its driver cover phone chart payload and machine-scoped provider actions; native application builds are separate targets. Review
renders use explicit deterministic TurnUsage fixtures, never claimed as live
account readings.

## Reference

The authentic repository is [pingdotgg/t3code](https://github.com/pingdotgg/t3code),
inspected at `5b2ee393c3d6576bcf44c5d2b74565770943e189` on 2026-10-20:

- [User documentation](https://github.com/pingdotgg/t3code/blob/5b2ee393c3d6576bcf44c5d2b74565770943e189/docs/user/usage.md)
- [Usage page](https://github.com/pingdotgg/t3code/blob/5b2ee393c3d6576bcf44c5d2b74565770943e189/apps/web/src/components/usage/UsagePage.tsx)
- [Usage data contract](https://github.com/pingdotgg/t3code/blob/5b2ee393c3d6576bcf44c5d2b74565770943e189/packages/contracts/src/usage.ts)
- [Server aggregation service](https://github.com/pingdotgg/t3code/blob/5b2ee393c3d6576bcf44c5d2b74565770943e189/apps/server/src/usage/UsageService.ts)

T3 Code's useful patterns here are separate metrics, environment selection,
progressive results and honest cost/coverage labels. Backplane retains its
existing Settings navigation and machine isolation. Rolling ranges use the
existing daily counters; pricing overrides and
historical scans are outside this page.

## Token stats in threads

Settings → Usage → Token stats (off until turned on) puts a chip by a
thread's send button: how much of the model's context the session holds
("62k · 31%"), or, from a provider that gives no context reading, the tokens
the thread's turns used. While it is on, the model chip drops its own
"ctx" tail. Clicking the chip opens a panel with what the provider told:
the context held and the room left, the last turn's input, cache read,
cache write, output (and reasoning) and cost, the thread's totals with its
cache hit share, and for Claude its models this session (helpers and
subagents included) from `modelUsage`. A part a provider did not give shows
a dash, never 0.

Speed is Claude's alone. Its result line carries the process's running API
time (`duration_api_ms`, every model call's own time added up) and output
tokens per model; a turn's speed is its share of output over its share of
that time, so tool runs and waits are not counted (and calls that ran side
by side each count their own time). A session already running when the hub
first saw it gets no speed until its next turn. No other provider reports
call times, and a turn's wall time would count tools as the model's, so
they get none. The readings are the settings `turn.last.<thread>`,
`turn.speed.<thread>` and `turn.models.<thread>` (hub only, never mirrored).
