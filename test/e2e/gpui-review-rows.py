#!/usr/bin/env python3
"""Review mode in the GPUI window (#1305), on gpui-f-rows.py's harness (private Xvfb + i3, real X input, stand-in agents only).
usage: gpui-review-rows.py CLIENT_BIN HUB_BIN ASSETS_DIR OUT [PORT] [DISPLAY_N] [rows...]   rows: F26 (the review bar and its one-press switch)
A thread asked "review the board": the bar over the message box says it is read-only and why, the header marks it, Allow edits switches review mode off
(the hub logs it) and Read-only again switches it back on."""
import os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
os.environ['ROWS_SEED'] = os.path.join(HERE, 'gpui-review-rows.seed.py'); os.environ['ROWS_FILE'] = os.path.join(HERE, 'gpui-review-rows.rows.py')
sys.argv = [os.path.join(HERE, 'gpui-f-rows.py')] + sys.argv[1:]
try: exec(open(sys.argv[0]).read())
except BaseException:
    if 'cleanup' in globals(): cleanup()   # the harness's: its hub, X server and window go too
    raise
