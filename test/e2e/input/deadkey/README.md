# Dead acute through ibus over XIM (Boat only)

`../deadkey-run.mjs`: on Xvfb with ibus (`XMODIFIERS=@im=ibus`, engine `xkb:us:intl:eng`) and `setxkbmap -layout us -variant intl`, click the composer,
press `dead_acute` then `e`, send. Pass: the hub receives "é" and the client is alive. Exit 1 = the app failed, 3 = harness.

ibus sends the accent's preedit as the COMPOUND_TEXT byte 0xB4 and the commit as 0xE9. A client whose XIM decoder reads them as UTF-8 panics
(`zed-xim client.rs:237: Encoding Error: Utf8Error ... bytes: [180]`); fixed by the vendored xim-ctext and zed-xim (`native/gpui/vendor/`, PR 1484).

    test/e2e/boat-run.sh input-deadkey <dev build>/backplane-hub <dev build>/backplane-gpui

A dev build needs the desktop's glibc, newer than a Boat box's, so `boat-inner.sh` runs the row inside `test/e2e/desktop.Dockerfile` (Ubuntu 26.04) there.
Results: `artifacts/e2e/deadkey/deadkey-summary.json` in the job folder `scripts/boat-job.sh` names ("logs <folder>"), with the client's stderr and screenshots.
