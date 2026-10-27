Native viewer handoff, 2026-10-27
================================

The native viewer patch is independent of the web renderer. Android GLES3 and
iOS Metal receive the same compact Solid triangle wire as before. Geometry,
board-axis transforms, native per-piece picking and Bend selection decisions
are preserved.

Both PlotStore implementations now demultiplex solids onto a separate serial
mesh queue. Any newer solid response (including a source switch A → B), or a connection
reset, cancels outdated decode/publication while completed source caches remain. Cancellation is checked every 256 triangles. Exact
mesh bytes plus triangle count identify reusable geometry; the cache targets
64 MiB and four sources, allowing one current model over the budget when wire
bytes are included. `cachedMesh(key)` supplies immediate warm reopen without
waiting for another model response. Malformed, truncated, oversized and overlong
varint payloads fail explicitly. Wire bounds are interpreted as signed integers
before converting 10 µm units to micrometres. Previous successful geometry may
remain visible during decode/failure, with an explicit previous-revision label.
Native decoding is off-thread; it publishes received/decoding and complete/error
states, rather than claiming progressive triangle prefixes.

Per-thread/source/mode camera poses are kept separately in a bounded 64-entry
memory cache. The scope is the existing screen `sel`. Same-source remeshing
keeps the camera; a reopened canvas restores its orbit/pan/zoom, keeping the
2D viewport centre on resize. Native keyboard arrows orbit (Shift pans), +/-
zooms and F fits; existing one/two-finger gestures remain. System reduced motion
suppresses automatic fades and inertial coasting. No continuous repaint loop is
introduced.

The shader port implements the web's linear-RGB colour-estimated materials,
studio lighting, highlight shoulder and explicit sRGB display transfer.
Materials remain estimates: the compact wire contains no authored material IDs.
The optional thermal tint is display-only. This patch does not implement native
depth of field, path tracing, or the new original-GLB renderer.

Reflow contract
---------------

Native engine calls run on the existing engine worker:

* `Backplane.reflowDefaults()` returns the profile JSON string.
* `Backplane.reflowAlloys()` returns alloy metadata JSON.
* `Backplane.reflow(profileJSONString, alloyID)` returns raw planner JSON with
  `duration`, `samples` ([second, furnace °C, board-mean °C, liquid fraction]),
  or an `error` string.

These functions require the integration parent's new `core/reflow.bend`, mobile
app exports and `bridge.js` wrappers. They are not supplied by this native-only
patch. The Swift Engine/AppModel adapters are a separate small commit so the
iOS transport/integration owners can apply the equivalent wrappers safely.
The Android Core/AppModel adapters follow the same worker contract.

`ReflowView.swift` and `ReflowView.kt` provide alloy/composition/source, native
temperature and fraction graphs, a validated JSON profile-point editor,
play/pause/restart/speed/scrub controls and an optional heat overlay. They call
the shared Bend planner once per profile/alloy change; playback interpolates
returned samples and never reimplements the thermal model. Closing/backgrounding
pauses playback. The preview and its assumptions are labelled illustrative.
Native import/export file controls are not included.

Validation
----------

* `test/tools/native_mesh_cache_test.py` compiles the actual Kotlin decoder and
  cache extracted verbatim, and runs them on the reviewed 2,776-triangle board
  fixture. It passes signed bounds, finite geometry, cancellation, malformed and
  oversized input, exact revision reuse, retired-hub queued messages, reset/late
  results and eviction. It uses cached Kotlin 2.0.21 compiler dependencies and
  only temporary output files.
* `test/tools/native_studio_shader_test.mjs` compiles, links and draws the actual
  Android GLES3 shaders in isolated headless Chromium WebGL2. It passes mask hue,
  dark detail, white silk, distinct material responses and thermal uniforms.
  This is shader execution, not Android device validation.
* Full Android `:app:compileDebugKotlin` passed in 21 seconds with JDK 17,
  one worker, 2 GiB and offline dependencies. The SDK has Build Tools 35/36;
  the repository's default 34 is missing. A temporary Gradle init script selected
  installed 35 for that build only. No repository build settings changed.
* No phone runtime, APK installation, TestFlight or public release was performed.
  No local SwiftUI/Metal compiler is available. A read-only Mac toolchain probe
  failed SSH authentication; the dedicated iOS integration owner is arranging
  the actual Xcode/simulator build. iOS source is explicitly not build-validated.

No global proof ran in this child. The integration owner runs the single final
proof gate for combined Bend changes.
