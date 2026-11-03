# Canonical conversation fallback

This supersedes the source-row display fallback in #804. Production now calls
`Window.prepare` in `src/server/conversation-window.bend`. The old `Frame.*`
display helpers remain for existing checked laws; the server does not draw
conversation content through them.

The negotiated web path reuses the original canonical render after the source
window has been bounded to at most 32 entries and 32 KiB of encoded previews.
Fixed chrome no longer reduces that window to a single row. A compact command
navigator substitutes only for the sidebar; the complete canonical outer tree,
message roles, visibility, folds, live/pending messages, source read boundary,
body continuations, requested viewer and canvas keys remain intact. The actual
stream/window metadata is retained, without rebuilding it from a blank stream.

A fixed root reference and its correlated read-only region are adopted
atomically before normal presentation validation and DOM diffing. The existing
captured-request draft refill also applies to the restored root. Region replies
bypass old delta/sidebar-reference composition and force a full paint; only an
accepted current picture retires those caches. Navigation ACKs remain ACKs.
Errors retire unfinished control collections. The receiver limits accumulation
to 64 MiB and 16,384 parts; it adopts no partial state or effects.

Wire framing applies to the new host offer `windowProjection: 1, viewControl: 1`.
Partial/mixed capability offers are excluded from that runtime framing claim.
Legacy requests retain the ordinary whole-tree format with compact sidebar.
Each control fragment has at most 32 KiB of escaped text payload, plus headers;
this is not a claim that the whole packet is exactly 32 KiB. Aggregate control,
logical root bytes and DOM workload are distinct from the per-frame wire cap.
The region records its logical byte count.

Three additional laws bind actual production code: the final reference envelope
fits 262,144 bytes for every prior result under only the independent request-ID
admission (`Json.bytes(id) <= 32`), its wire picture is constant, and its canonical
region is retained exactly. The checked worst-size request-ID witness is 244 JSON
bytes for the complete reference envelope. There is no root-size or output-fits
premise. Existing no-refusal, clipping and paging laws remain unchanged.

## Qualification

Work `w-t1793675079-1331425-1346309` passed on main `65a6bbfdb` plus this
follow-up, with the approved Bend/Bun kit and all 66 compiler resources verified.
Its public input manifest SHA256 was
`bc7f6aa0e4c3fcd2159e36f8dadd6575586ea4240fbca54bfb6c295161097d81`.

- 45 Bend assertions: 12 canonical-root cases, all 24 full-output sidebar cases,
  and nine Diff/catalog cases. The generated-message dependency pin was updated
  only after the sidebar cases passed. The two existing Diff count messages were
  registered in English/Japanese catalogs to repair main's stale generated file.
- 235 JavaScript assertions passed in work `1345292` on the identical transport,
  draft-refill and root-restoration sources. These include changed-region paints,
  a 300 KB Unicode draft, stale navigation, control correlation and error cleanup.
- A matching fresh hub/web build passed unchanged native/unmarshal guards.
- Trusted, isolated Chromium replay of the read-only 18,409-entry Bub copy and
  55,228-entry triple painted content, without a status bar or LIMIT. Paging,
  thread and bot navigation passed. A 51-tool prefix automatically reached the
  genuine preceding assistant message.
- Requested 3D retained the exact viewer, workbench, canvas, WebGL context, source
  key and scope across pressure. This uses a synthetic GLB/software WebGL fixture;
  it does not claim the reader's separate strict eight-case hardware gate.
- `scripts/check.sh proof` exited zero in 471 seconds for all 12 conversation
  laws. Raw Bend output listed only 2,581 existing unsafe/foreign definitions;
  the unchanged `bendok` accepted it. It did not print literal All terms check.

Qualified backend SHA256:
`5e311796e3e0b3a45d658b52aa0cc4b1a24cd96d3d74f520298adf37ae2e68d6`.
Qualified web chunk SHA256:
`35afe86f4ae58f2552cbde55f8cadbb2fdf543e306ce7451a868a1166c5b30b0`.
Window source SHA256:
`f1538750dc09596e7dff8900317d442e7c443a417041b473efbb0829efb572e8`.

These are pre-merge branch receipts. The installer owns the selected merged
payload and deployment. Private history is not included in the repository.
