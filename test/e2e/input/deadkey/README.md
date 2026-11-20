# Dead acute through ibus over XIM (Boat only)

`../deadkey-run.mjs`: on Xvfb with ibus (`XMODIFIERS=@im=ibus`, engine `xkb:us:intl:eng`) and `setxkbmap -layout us -variant intl`, click the composer,
press `dead_acute` then `e`, send. Pass: the hub receives "é" and the client is alive. Exit 1 = the app failed, 3 = harness.

ibus sends the accent's preedit as the COMPOUND_TEXT byte 0xB4 and the commit as 0xE9. A client whose XIM decoder reads them as UTF-8 panics
(`zed-xim client.rs:237: Encoding Error: Utf8Error ... bytes: [180]`); fixed by the vendored xim-ctext and zed-xim (`native/gpui/vendor/`).

How it ran (dev builds need the desktop's glibc, newer than a Boat box's 24.04, so it runs in an Ubuntu 26.04 container on the box):

    boat new --no-env --type default                      # a box: never the person's display
    # on the box, in ~/xim: before/ and after/ = a dev build folder each (rsync -L: libcef and the paks are symlinks), bin/bun,
    # ws/ = `git ls-files test` plus native/host/src/wire_dict.rs, and this folder's Dockerfile and row.sh
    sudo docker build -t xim-deadkey .
    sudo docker run --rm -u u -v $HOME/xim:/xim -v $HOME/xim/ws:/ws xim-deadkey bash /xim/row.sh before   # or after
