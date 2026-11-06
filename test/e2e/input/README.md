# GPUI input driver (real OS input on Xvfb; Boat only)

`xinput.mjs` (xdotool/XTEST: click, double/triple click, drag with motion, key combos, unicode typing, wheel; refuses `:0`), `clip.mjs` (xclip: text, image/png,
file lists, PRIMARY, TARGETS), `dnd.py` (an XDND v5 source that drops `file://` URIs onto a window; exit 0 = the target accepted the drop),
`ime.mjs` (ibus + the Hangul engine, `g k s space` composes 한), `driver.mjs` (the actions a journey needs; each returns `{ok, evidence}` taken from outside the client:
clipboard, PRIMARY, screenshots, the hub's own log). `selftest.mjs` proves every mechanism against xev / xclip / an XDND window.

Use (mechanics): `test/e2e/boat-run.sh input-selftest` (mechanisms), `test/e2e/boat-run.sh input-run HUB GPUI` (the must-pass actions against the real client;
results in `out/e2e/input/input-summary.json` with a screenshot before and after each action). From a script on Boat:
`const inp=new Input(':64', outDir); const win=inp.win('backplane'); await inp.dragSelect(win,[x1,y1],[x2,y2]); await inp.copy(); inp.dropFile(win,'/path/file.png',[x,y]);`
Judge results by what the hub received (see `input-run.mjs`: messages from `events.jsonl`), not by changed pixels: a streaming thread changes pixels on its own.
Never run it on this desktop (xinput refuses :0; the person's word is needed for any desktop display).
