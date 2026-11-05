# State digest: the differential oracle

Every recorded `events.jsonl` is folded twice, by Backplane's own `M.State.replay` (`native/bend/digest.bend`)
and by the Rust fold. Each prints one JSON object. `oracle.mjs` canonicalises both and compares their SHA-256.

```
{ "projects": [ {id, title, root, removed} ],
  "threads":  [ {id, project, title, turn, pending, archived, pinned, deleted, provider, model, session,
                 entries: [entry id, in State.mine order]} ],
  "settings": [ {key, value} ],
  "seq": <changes applied> }
```

- `turn` is one of idle, running, completed, interrupted, failed.
- Canonical form: object keys sorted; `projects` and `threads` sorted by `id`, `settings` by `key`; `entries` keep their order
  (order is part of the claim). Numbers are decimal integers. No whitespace.
- Events the Bend decoder does not read (unknown `$`) are skipped by both sides and counted in neither `seq`.
- A log is cut into chunk files of whole lines (default 4 MB) and both commands receive the chunk paths in order:
  `digest CHUNK1 CHUNK2 ...` and `<rust-command> CHUNK1 CHUNK2 ...`.

## Scope (what is compared today)
`seq`, `projects`, `threads` (with entry order) and every setting EXCEPT the families the Rust fold does not model yet:
`uber.*` (UberSet/UberOp), `bg.*`, `watch.*`, `wake.*`/`wakes.*`, `lock.*`/`locks.*`, and TurnUsage's token totals `tok.*`, `turn.last.*`, and `deploy.*` (DeployAsked/DeployBuilt). `oracle.mjs` prints the skipped families with counts
(`skippedSettings`) and `--skip none` compares everything. Events the fold counts but does not fully model (e.g. TodosChanged other than
`snapshot`) are listed by `rust-digest` on stderr.

## Privacy rule (store copy)
The digest holds project and thread titles, project paths, settings keys and full settings values. It must never be printed or kept.
`rust-digest` prints only counts unless `--json` is its first argument; the oracle passes `--json` to read it into memory and prints hashes and counts only;
`sdiff.mjs` prints key patterns and counts, never values, and must not be run on the store copy with its output kept. The oracle removes its temp chunk files on any exit.
