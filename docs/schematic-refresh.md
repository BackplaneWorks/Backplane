# Schematic save refresh

Same-size schematic saves in the same second could wait for the watcher's
seven-tick fallback reread. The filesystem was polled promptly, but `FS.stat`
returned only whole-second mtime, so the intervening polls did not reread.

`FS.revision` adds a separate, exact metadata revision: device, inode, full
size, and nanosecond mtime/ctime. Schematic polls in the native watcher and
the shared hub plot actor compare this revision. File identity covers atomic
replacement; ctime covers writers preserving mtime; atime is excluded so
reading a file does not invalidate it. Fields are kept as decimal strings,
without hashing or floating-point timestamp conversion. `FS.stat` retains
its existing contract. The 150 ms interval and seven-tick fallback remain;
board and 3D sources keep their existing whole-second observation.

No hierarchy or rendering algorithm changes are involved. Root and child
schematics continue through the existing read/parse/scene and plot-child
paths. Canonical files and metadata companions are never rewritten.

## Measured native component

Both binaries ran on the same Linux desktop, CPU affinity 8–9, two runtime
threads, 2 GiB/two-core admission, and a fresh Xvfb `:187` with TCP disabled.
Each batch had one untraced run and one syscall-traced diagnostic run.
The 184-byte synthetic fixture contains `REV000` text and a wire from
(10,10) to (20,10) mm. Rapid saves change the text to `REV001`/`REV002`
and move the endpoint to 21/22 mm, retaining file size and real timestamp
second. Later steps exercise atomic replacement, an incomplete write, and
final `REV005` with a 25 mm endpoint.

The real native `Watch.loop` sends the `Board` channel message, then the
probe updates the scene, prepares viewer operations and presents through
`Win.present`. X11 readback must equal the submitted native buffer. Pixel
checks verify the edited wire endpoint, the expected revision text against
baseline captures, monotonically advancing revisions, and final revision 5.
No user CAD or event-log fixture was used or uploaded.

| Measurement | Before | After |
| --- | ---: | ---: |
| Second rapid save to X11 presentation, untraced | 1044.392 ms | 143.415 ms |
| Same measurement, traced diagnostic | 1061.176 ms | 141.699 ms |
| Second rapid save to first stat, diagnostic | 139.530 ms | 132.273 ms |
| Second rapid save to first file open, diagnostic | 1044.057 ms | 132.394 ms |
| Presentation work per frame, untraced | 7–8 ms | 7–9 ms |
| Batch child peak RSS | 107860 KiB | 108260 KiB |

Reading, parsing, sorting, hierarchy loading, scene merge and operation
preparation each measured below 1 ms on the small untraced fixture. The
measured delay was before rereading. These numbers do not characterize
large hierarchical designs, general chat FPS, or physical display/device
release acceptance. The component intentionally isolates the native file,
channel, scene and X11 path; the release owner retains full-app acceptance.
Filesystems returning identical metadata for multiple writes can still
need the preserved fallback reread. New saves do not bypass existing
partial-file parsing safeguards.

Before: main `947b9c346a35875a19e9e58e926899c0a9aef7f9`, component SHA256
`b91b10414802acc3fbb5408c752546bdd1b859decf4a7cddfe72adbf7ead6d89`.
Baseline probe source SHA256
`b4403c24dd8d84fbe835f1357bc1c89b5b0ef0d19403a22b8123328f6015dcb1`
matches its Boat source manifest. Batch: 2026-10-19 21:00:20.117–21:00:44.841 UTC.

After: component SHA256
`0f359b87f3b685b6736c3971f0a0555b1a2befb3ef87de62ed83e0c74bef20c9`.
Batch: 2026-10-20 00:34:16.402–00:34:41.022 UTC. Both batches used the same
fixture geometry/text and runtime/display settings. All captured native,
trace, screenshot and Xvfb processes exited, and the display socket/lock
were absent afterward. Local detailed records remain under
`build/schematic-refresh/{baseline-2,after-1}`.

## Checks and reproduction

Passed: 489-file source manifest check; full `scripts/check.sh proof`
(191 seconds, stock accepted foreign-effect dependency notice); three new
append-only laws; six revision-decision checks; 62 schematic, fork,
hierarchy, reopen and stale-source checks; JS filesystem effect checks;
actual C effect checks for POSIX plus the Darwin field-name branch.
The latter compiles against Linux field aliases, not a Darwin/Windows
runtime or package acceptance claim.

The real hub plot actor and its own native `--plot` children also pass
same-second, atomic, partial/final revision and metadata-companion tests
for both a root schematic and a watched child of a canonical root. These
exercise the shared payload path consumed by web and phone viewers,
including chunk reuse and monotonic final geometry. Second rapid edits
arrived in 151.949 ms (root) and 150.547 ms (child) on Boat, under the test's
500 ms bound. No HTTP/browser/device package acceptance is implied.

Build the small components with the normal admitted build workflow (on Bun):

```sh
scripts/build-js.sh test/native/schematic_refresh_probe.bend build/refresh-native
scripts/build-js.sh test/native/refresh_plot_probe.bend build/refresh-plot
python3 test/tools/refresh_plot_test.py build/refresh-plot build/refresh-root
python3 test/tools/refresh_plot_test.py build/refresh-plot build/refresh-child --child
bun test/tools/refresh_effects_test.ts
```

For a reserved local display timing slot, set `SCHEMATIC_REFRESH_XVFB` to
the host's Xvfb executable and invoke `test/tools/schematic_refresh_batch.py`
with a fresh output name and component path under the required CPU/memory
admission. `SCHEMATIC_REFRESH_REFERENCE` may point to a prior run's
`result.json` to verify expected revision text pixels. The script creates
its own schematic copies, display, captures and cleanup receipt; it never
accepts a user CAD path. Native acceptance is kept separate from off-host
proof/build work and from other owners' controlled timing slots.
