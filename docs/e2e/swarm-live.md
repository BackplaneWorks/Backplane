# Swarms: the live run (Nov 11, 2026)

These were real model turns on a throwaway hub (a temp home, port 3811, `--no-tailscale`), driven over data/1
the way a phone drives it. The members were Opus 5.5 (through Antigravity), Gemini 4 Carbon (Antigravity) and
Codex (the CLI default). The toy problem was a tested `debounce.py` with a one-page DESIGN.md
(`swarm-live/toy-DESIGN.md` is what they wrote). The boards are in `swarm-live/*.board.txt`.

## Run 1: "debounce" (it ended with a synthesis)

- **Per-member failover.** The first Opus member was started on the CLI's own Claude login. That login was
  at its weekly limit, so the hub's fallback moved the member to Codex on its own ("Switched to Codex: Claude
  ran out").
- **Removing and adding a member.** The person removed that member and added Opus 5.5 through Antigravity.
  The board shows "left" and "joined". The new member read the board and built on the proposal already there.
- **Wake-ups and mentions:**
  - Gemini opened with an integrator proposal that @mentioned the others.
  - Each post woke the members it was for (turns 3 → 29).
  - Opus critiqued the proposal and claimed `test_debounce.py`.
  - Gemini claimed `debounce.py`.
- **Claims and dones.** Each `done` answered its claim and freed its lock. Opus reviewed Gemini's code and
  found 39/39 passing.
- **The person steering.** Codex had hung (see the bugs below). The person's post (`you`) reached it at once,
  and it claimed and finished DESIGN.md.
- **The synthesis.** Opus posted it. The swarm became `done`, and the members were told the swarm was over.

## Run 2: "naming" (the loop guard fired)

- **The setup.** The seed asked the members to debate a function name, defend their picks and never
  claim anything.
- **The guard.** After 12 member posts in a row with no claim and no done, the chatter guard held the 12th
  post and paused the room: "Paused: 12 posts in a row with no claim and no done".
- **While paused.** A post Gemini finished from a turn already under way was held (`HELD:paused`). The member
  turn count stayed at 24 until the person spoke.
- **The person steering.** The person's decision resumed the room and reached all three at once. Gemini
  claimed the docstring change and posted done, and Opus posted the synthesis.

## Bugs the run found, all fixed with tests

- **Codex hung forever.** `codex exec` 0.167 reads a piped stdin to EOF as more prompt, and the hub kept every
  agent's stdin open, so every Codex turn waited with no output. This was not swarm-only.
  - Fix: `Driver::closes_stdin`, set for Codex.
  - Test: `agents::tests::a_codex_turn_gets_eof_on_stdin_and_finishes`.
- **Peer mentions restarted Codex.** A mention steered a running non-Claude member, which restarts its turn:
  five restarts and no post. Now only the person's post steers; a peer's mention reaches a running
  Codex turn with its next tool result.
- **A requested synthesis was refused.** The rate guard counted posts from before the person's post, so it
  refused the synthesis the person had just asked for. The person's post now starts the rate count afresh,
  as it already did for the other guards.
- **data/1 times were in seconds.** `swarm.get` and `swarm.list` answered seconds, but data/1 times are ms.
  The Android lane caught it.
- **Deadlock after a synthesis.** A synthesis, a stop or a done released claims under the bots world lock,
  and handing a waiting peer its lock needs that same lock. The GPUI lane caught it on its rig.
