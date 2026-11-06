# Project home

> Paths under `src/` below (`src/core`, `src/server`, `src/web`, `src/mobile`, `src/app`) name the Bend JS tree, removed from this checkout (tag `main-bend-final`). Where the current Rust code for each is, is unverified; look under `native/host/src/` and `native/gpui/`.

Click a project name to open its home. The adjacent chevron expands its
threads, the badge edits its appearance, and + creates an empty thread.
Typing in the home composer starts a conversation in that project with that
exact first message. Opening a home starts no conversation or agent.

A home is a projection of saved project contents and current conversation
state. It has no project-type switch. A project with a specification but no
board shows the specification. Software shows README, notes, conversations
and available repository information. A saved board adds the existing board,
schematic and 3D viewers, saved check results and fabrication metadata.
Mechanical models add their saved thumbnails and a Mech action.

## Ownership and data

The presentation target is `{kind:"project", id:projectId}`. Its local draft
and viewer owner is `project-home-<projectId>`, distinct from any conversation.
No synthetic `ThreadCreated` is persisted. `project.home` reads the canonical
project root, including explicit `.backplane.json` document selections; it
never substitutes the selected conversation's worktree.

`src/server/effects/project-home.js` resolves existing artifacts asynchronously.
External symlinks, missing paths and fab files whose source is missing do not
produce artifact panels. `src/core/project-home` projects the resolved
snapshot into panels; `home_missing_artifact_has_no_panel` proves the empty
path case over arbitrary panel kind, title and summary. Generic controls can
create notes when there is no intent file, without displaying an empty file
panel or writing files merely because a project was opened.

Replies require both the current request ID and the selected project ID.
Editor state is scoped by the home owner. A save carries the saved revision,
byte offset and replaced section length; it streams unchanged prefix and tail
into a temporary file before revision checking and atomic rename. Conflicting
saves retain the person's edit. Sending retains the captured draft until its
matching acknowledgement. The hub atomically creates the thread and first
message, keyed by the retained message ID, so retrying creates neither a second
conversation nor a second message.

## Viewers and fabrication

The embedded web canvas is the existing viewer. It loads after the home
snapshot and follows saved-file changes through the existing watcher. Native
phones open their existing viewers explicitly, with their existing selection
and presentation ownership checks. Board/3D, schematic and Mech destinations
are offered only when their source appears in the current contents snapshot.
BOM opens the existing review viewer for the saved schematic. DRC/ERC chips
show saved report counts and whether the source changed after the report;
opening a home never starts those checks.

The home viewer offers Split when both saved CAD documents exist, using the
exact board and root schematic paths from the snapshot. Its pane controls
show or hide the schematic, board and 3D model. CAD navigation and BOM stay
in the viewer toolbar while it is open; closing it restores the artifact
cards. The viewer uses the available project width, including when the sidebar
is folded, with a taller 3D workspace. The embedded view reserves no unused
external-viewer column.
Split pane handles work by pointer or keyboard and retain proportions per hub
and project. Arrow keys resize, Shift moves faster, Home/End reach the limits,
and Enter or double-click resets. The handles follow the layout's orientation
and keep both panes reachable on narrow screens. Schematic views fit the whole
sheet when opened and after pane resizing; board inspection cameras are retained.
Native phone Split views show all three panes, stacked in portrait and
side by side in landscape, with Expand returning to the selected full view.
The home composer carries its project owner through keyboard focus, local
draft paints, refreshes and sends. KiCad opens from that canonical project
root, so `.backplane.json` selects the same project as the saved-file viewer.

Fab panels read the source's `.fab.json` options and measured features.
An explicit price action uses the existing estimator and hub pricing
preferences, with bounded, freshly read metadata. It does not upload files,
start an order or bypass the ordering flow. No full project-card intent read
is used to obtain that estimate.

## Size, latency and controls

| Requirement | Mechanism | Qualification |
| --- | --- | --- |
| Cached opening within 100 ms | Existing bounded transition-picture cache, fenced by project owner, hub, pairing identity and epoch; project-name navigation takes the normal selection path | Isolated browser runner delays replies to distinguish cache paint from a fast network response |
| Lazy board loading | Asynchronous contents read followed by the existing saved-file viewer; no CAD/check job during navigation | Browser and viewer source assertions |
| No project-size refusal | At most 32 artifact panels, paged discovery and models, 32 conversations plus lookahead, bounded document/intent sections; no whole-file substitution for a preview | Large UTF-8 files, minified reports and model-directory tests |
| Useful controls | Explicit home action dispatch plus the existing viewer, review and new-thread handlers | Source inventory, actual `Ui.act` state/RPC assertions and rendered-control browser checks |
| One composer control row | Shared `ComposerBar.node`/overflow disclosure on web; one native horizontal control row | Actual emitted DOM measured at 1280 and 390 px |
| Artifact-driven panels | Pure panel projection over resolved existing contents; no project eligibility flag | law, missing-path/cross-project fixtures and software/spec-only browser checks |
| Phone support | Responsive web layout plus native SwiftUI/Compose cards from the same panel/control plan | Native builds and phone-width browser checks |

The source inventory from #790/#794 remains conservative: computed bindings
and image expressions elsewhere in the application are unresolved, rather
than silently counted as covered. Its literal-handler result and the runtime
home checks must be reported separately; they do not establish a universal
all-input UI proof.

`test/project_home_test` exercises the pure projection, project ownership,
atomic creation/retry and useful home handler effects. The asynchronous reader
is exercised by `desktop/test/project-home-io.test.cjs`. Transport/cache tests
extend the existing `thin-host` and `recent-views` suites.
`test/tools/project_home_e2e.mjs` owns a temporary hub with stand-in agents,
synthetic projects and a fresh invitation. It accepts explicit binary and web
asset paths, never an existing hub address, home, token or pairing invitation.

Names in the home header, thread rows and board/mechanical headings edit inline.
Enter saves through the shared rename dispatcher and Escape cancels. Thread
renames retain `thread.rename`; project titles use the canonical project upsert
with the same id, root and creation time. Board/part display labels are
project-relative settings, so filenames and fabrication source references stay
valid. The message composer has its own owner-scoped draft throughout.

The home header and sidebar read `Badge.icon` from the same project setting.
The header opens the existing icon/color picker and uses its `look-set` handler.
Native clients render the same semantic choices with platform glyphs.

The web home opens a saved board in 3D using `View.aside.if`, the same
`Solid` camera, `Workbench`, original-model Studio worker and authenticated
saved-revision asset as the full viewer. Board, Schematic and Split use their
existing viewers too. There is no home-specific model export, material recipe
or image pipeline. Loading remains lazy; preparation uses the same revision
fences and resource lifecycle as the full viewer.

Open viewer expands the mounted viewer in place. Back to project restores its
compact presentation. The owner, saved source, renderer, materials, selection
and camera stay with that same component; presentation never creates a thread.

`project_home_handler_test.py` runs the #790/#794 source inventory for every
literal home control, with a deleted-handler negative control. It is separate
from runtime behavior. `viewer_integration_e2e.mjs --home-parity --quality required`
uses a temporary isolated hub with blocked stand-in agents and reads the saved
project without modifying it. It compares real Studio canvas pixels at equal
dimensions, source/revision/GLB digest, material policy and default camera
between home and full 3D, then checks that expand/return retain the same canvas,
worker and edited camera. Pixel tolerance is 8 channel levels, at most 0.1%
differing pixels and mean channel error at most 0.3; blank frames are rejected.
The metric has negative checks for mirroring, a missing model and wrong sizes.
This browser gate requires qualified candidate assets; it must not run during
a build halt.

Split's 3D pane calls the same `View.solid.workbench` factory as standalone
3D. It does not bypass Studio by mounting a separate fast-only canvas. The
pixel gate also compares the retained pose and pixels after switching to Split.
On wide homes, the viewer and project details share an accessible separator
with its own saved proportion. Narrow layouts stack those sections; Open viewer
fills the main pane without changing the mounted viewer's identity.
