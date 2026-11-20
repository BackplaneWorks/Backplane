# Infinite context for bots

With its flag on, every bot's thread keeps one conversation that never ends, after UniiChat
(github.com/VictorTaelin/uniichat, `Memory.ts`). The code is
`native/host/src/bots/recall/` (`tree.rs` is the pure part) and the seam
`native/host/src/agents/recall.rs`.

- **The tree.** Message i is the i-th of the thread's own rows that are a
  message: the person's and others' messages (`user`), the bot's replies
  (`you`), its tool calls (`tool`), Backplane's notes (`note`). Leaf (0, i)
  is message i; node (l, i) merges (l-1, 2i) and (l-1, 2i+1). Every node is
  one line of at most 512 bytes: a short message is its own line, word for
  word; anything else is written by the compactor.
- **Kept in the log.** A compactor call's lines are one `RecallNoted`
  event; the view a batch merge left is a `RecallView`. Both fold into the
  bots world (checkpointed), so a restart has the tree and the compactor
  goes on with what is not written. The message numbering is the log's
  own and is read from the model, not stored.
- **The view.** Up to 128 KB: recent messages one line each, older ones
  covered by broader lines. Between batches it only grows at its end, so the
  provider's prompt cache keeps all of it but its end. Past 128 KB a batch
  merges the most due pairs (oldest since their last message, in halves)
  down to half, and is kept only when it brings the view under 128 KB.
  While the compactor is behind, the oldest lines are left out, said in one
  fixed line.
- **Turns.** A thread with infinite context runs each new turn in a fresh
  provider session (no `--resume`). Its message starts with how to read
  the view and the view in `<chat>`, then the standing rules, the
  `<memory>` block and the message. A message into a running turn stays in
  that turn's session. No hand-over is added: the view holds the
  conversation.
- **memory_zoom(id, n)** on the agents' MCP server (listed only in such a
  thread) opens line `id+n` into its two halves; `n = 1` gives the message
  whole, with who wrote it and when.
- **The compactor.** Settings `recall.provider` / `recall.model` (Haiku on
  Claude by default), through the agents' one-shot seam: a login that can
  take it in the failover's order (`shot_healthy`). Up to 16 lines per call,
  `recall.jobs` calls at once (2). A failed call holds the thread (a minute,
  doubling to half an hour); a node a call leaves unanswered three times is
  its source, cut. A thread's whole history is the tree from the start: its
  recent long messages are written first, then the rest oldest first.
  Nothing runs on a copy of a log or in safe mode.
- **Switches.** Off by default: Settings > Agents, "Bots: infinite
  context (beta)" (`recall.bots` = on). While it is off nothing runs: no
  thread has it, the compactor writes nothing, memory_zoom is not listed.
  With it on, every bot has it unless its Settings tab turns it off
  (`recall.bot.<id>` = off), and `recall.thread.<id>` = on gives any
  other long-lived thread the same.
- **Progress.** `progress.rs` measures a thread's tree after each compactor
  call and each round (only when its messages, lines, kept view or hold
  moved): the tree's lines that have their line, of all 2n - (roots) of
  them; the messages whose own line is not written (behind); the view's
  bytes as the next turn would read it, against 128 KB; and a hold's retry
  time. Its words are the hub's (`memory: 92% written · view 64/128 KB`,
  `caught up`, `12 messages behind`, `held: retry at 14:05`) and every
  client draws them: the bot's Settings tab (two bars: lines, view), the
  thread's context chip on the desktop (a small bar and the words instead
  of the session's context), and over the composer on the phones and the
  web (data/1 `thread.memory`, null for a thread without infinite
  context). A change is pushed with no log line: a `memory` frame re-sends
  the thread's row to the phones and the web, and the desktop's views are
  drawn again.
