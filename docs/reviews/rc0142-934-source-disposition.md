# Lane B scoped source-review disposition: the 4 sidebar-closure files changed by #934

Scope: metadata/source review only (no test, proof or full-output run). Not a 24/24 full-output
PASS claim. Main 2a87e6526 (#934 merge); pre-#934 parent 2a87e6526^1.

| file | manifest pin (= pre-#934 blob) | 2a87e6526 blob |
|---|---|---|
| src/core/kexport.bend | e7c768d51b790976... | 2ce372d77c5342db... |
| src/core/refview.bend | aad18d93039af1c4... | 8a8c95b92e1752bf... |
| src/mobile/hubs.bend  | 5ca0102839562f41... | e8d78ec982828a80... |
| src/web/view.bend     | 7eb118c140ef4c6d... | 1eda9f91b5ce7b88... |
(Full 64-hex values: sha256 of `git show <rev>:<file>`.)

## What changed in these 4 files (and nothing else in them)
Each changed def keeps its signature and calls a new helper defined just above it; the helper
matches a copy of the scrutinee. Rows, row order and row bodies are unchanged; catch-all binders
become the parameter. No import added or removed; no new def reads Ui scratch, drafts, @ fields,
info, image/cache keys or recipient data.
- kexport: KX.entry -> KX.entry.by (KReport accumulation; no Ui access).
- refview: Card.of -> Card.of.kind, Cards.deps.object -> Cards.deps.object.by (Ref -> Card lookup over M.State).
- hubs: One.act.home -> One.act.home.by (same three action rows; catch-all One.act.mail unchanged).
- view: VLabel.choice/VLabel.effort -> .by (label strings); View.local.keys -> View.local.key per attribute.

## Draft-reader / key boundary
- The only draft-adjacent code is View.local.keys: the "data-enter" attribute value
  `U.Draft.enter(rows, d)` is computed exactly as before (same call, same arguments), now inside
  View.local.key; other attributes pass through unchanged. It reads no new draft input.
- No change to sidebar draft filtering, @ retention, the private sidebar-image key, cache/key
  derivation, recipient authority, bridges, or the reviewed closure's file set (static guard logic on
  2a87e6526: closure set identical to the manifest; only these 4 hashes differ).

## Equivalence evidence for these defs (not a sidebar full-output run)
Boat j20261104140529-2864299 (one gated job): orig(case) == new(case) by {==} on every listed known,
malformed, near-miss and unknown input, with negative controls rejected: Card.of 84, Cards.deps.object
84, One.act.home 11, VLabel.choice 13, VLabel.effort 13, View.local.keys 5 (incl. data-enter,
data-tab/arrows/esc, near-miss names, D.On), KX.entry 9.

Disposition: source-equivalent refactor; draft readers and the cached-reader/key boundary unchanged.
Pin refresh for these 4 files is supported on source review; the 24-case full-output requalification
remains NOT RUN by lane B.
