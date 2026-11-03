# Project home and sidebar cache inputs

The project name now navigates to a project home; a separate chevron retains
thread expansion. Selection highlighting reads the selected owner already in
the private sidebar key. Project icons and names read canonical project/settings
state. The existing appearance picker gains a Close button; SVGs gain a stable
semantic icon attribute. None of these reads conversation body drafts.

The new home renderer is reached from the main pane, outside the sidebar
renderer. Its message draft uses `project-home-<project>` as an owner. Inline
names use `@ren`, `@ren.kind`, `@ren.owner`, `@ren.inline` and `@f.title`;
notes use owner-scoped `@home.edit` and `@f.home-intent` keys. All these scratch
keys retain the existing conservative `@` dependency, including unknown keys.
Viewer labels read artifact display-name settings, with canonical paths retained.
The native sidebar adds its project icon from the same settings state.

The project target separates draft/viewer ownership from thread-history ownership.
`Actor.safe_input` and refresh use the explicit target owner for its draft.
Initial refresh may issue a read-only `project.home`; it never creates a thread.
Home contents/documents are retained session info, fenced by request and project.
The complete main picture and session inputs still retain the complete draft.
No private sidebar filter/key, cache authority, source scanner, unknown-key
retention or recipient fence is weakened.

Reviewed changed dependency files: `client.bend`, `presentation.bend`,
`project-home.bend` and `thin-wire.bend` in core; mobile `hubs.bend` and
`screen.bend`; server `thin-actor.bend` and `thin-view.bend`; web `icon.bend`,
`project-home.bend` and `view.bend`. The new modules are included in the guarded
import closure. All previous review records remain in the manifest.

Qualification used pinned Bend 2.0.36 on the exact combined Bend source.
`test/project_home_test.bend` passed; all 24 full-output assertions in
`test/sidebar_draft_image_test.bend` passed, including current body/IME, unknown
scratch fields, search, recipient info, online/fold state and complete-page
comparisons. Full type checking and `PROOF.bend` also passed. The full-output
sidebar log SHA256 is
`f3b8821a6757e35f4f3f58736b2d8ae35ef9b716af4f9460aba19fe6ddfe93ec`.
Work receipt: `w-t1793685091-1352063-1355196` (Android compilation subsequently
needed an annotation; no Bend source changed for that repair).

This receipt qualifies the sidebar dependency projection. It does not claim
browser, native device, installation or universal computed-handler acceptance.

Follow-up qualification (`w-t1793685091-1352063-1355587`) passed on the viewer
handoff repair, lazy main-screen selection, owner-specific Send routing,
single appearance overlay and bounded/reordered main cards. The viewer uses
the existing one-shot `local.hub.view` mark; the following actor merge then
returns panel ownership to the client. No sidebar body draft reader, cache
filter, unknown scratch retention or authority changes. All 24 full-output
cases and the full type/proof gates passed again before refreshing these pins.
