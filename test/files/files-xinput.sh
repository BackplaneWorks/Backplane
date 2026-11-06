#!/bin/sh
# The Files tab driven by real X input (XTest, test/cef/xinput.py; XDND, xdnd_drop.py): click to select and focus, arrow keys, Return, BackSpace,
# right-click menu and its Rename, the filter field, Delete with its confirmation, a file dropped onto the window. Run by desktop-files-run.sh (FILES_XINPUT=1)
# with DISPLAY, R (the run dir), SH (shots dir) and N set. Rows of the folder are 28 px apart from y=170 at x~1100 on the 1440x900 display.
HERE=$(cd "$(dirname "$0")" && pwd); I() { python3 "$HERE/../cef/xinput.py" "$@"; }
shot() { import -window root "$SH/$N-$1.png"; }; say() { echo "XIN $*"; }
row() { echo $((170 + 28 * $1)); }
sleep 1; shot x00-start
# click a row: selects it and focuses the list (so the keys below reach it)
I click 1100 $(row 2); sleep 0.8; shot x01-click-select
I key Down; I key Down; sleep 0.6; shot x02-two-down
I key Up; I key Up; I key Up; I key Up; sleep 0.4; I key Return; sleep 2; shot x03-return-opens-folder
I key BackSpace; sleep 2; shot x04-backspace-up
# right-click a log row: the menu, then Rename
I click 1100 $(row 6) 3; sleep 0.8; shot x05-right-click-menu
I click 1130 $(( $(row 6) + 75 )); sleep 0.8; shot x06-rename-field
for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do I key BackSpace; done; I type "first.log"; sleep 0.4; shot x07-typed-name; I key Return; sleep 2.5; shot x08-renamed
# the filter field by click, then Escape
I click 1090 136; sleep 0.5; I type "kicad"; sleep 1; shot x09-filter-typed; I key Escape; sleep 0.8; shot x10-escape-clears
# Delete key asks first, then the Delete button
I click 1100 $(row 3); sleep 0.6; I key Delete; sleep 0.8; shot x11-delete-asks; I click 1254 175; sleep 2.5; shot x12-deleted
# an agent (here: the shell) adds a file on disk while the folder is on show: the hub's watcher pushes it, nobody presses anything
printf 'made by an agent\n' > "$R/proj/agent-added.txt"; sleep 3; shot x12b-agent-added-no-press
# a file dropped onto the window (XDND)
printf 'dropped by real DnD\n' > "$R/up/xdnd.txt"
XDND_DEBUG=1 python3 "$HERE/xdnd_drop.py" 1100 400 "$R/up/xdnd.txt" > "$R/xdnd.out" 2>&1; say "xdnd: $(tr "\n" "|" < "$R/xdnd.out")"; sleep 3; shot x13-after-drop
# the control: the same drop onto the composer (the attach path that existed before the Files tab did)
printf 'dropped on the composer\n' > "$R/up/composer.txt"
python3 "$HERE/xdnd_drop.py" 560 815 "$R/up/composer.txt" > "$R/xdnd2.out" 2>&1; say "xdnd composer: $(tr '\n' '|' < "$R/xdnd2.out")"; sleep 3; shot x14-after-composer-drop
# what the keys, clicks and drop did to the project
P=$R/proj
[ -f "$P/first.log" ] && say "rename ok: first.log exists" || say "rename FAILED: first.log missing"
[ ! -e "$P/log-1.log" ] && say "rename ok: log-1.log gone" || say "rename FAILED: log-1.log still there"
[ -f "$P/xdnd.txt" ] && say "drop ok: xdnd.txt uploaded" || say "drop FAILED: xdnd.txt not in the project"
ls "$P" | head -12 | tr '\n' ' '; echo
