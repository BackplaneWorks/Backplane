#!/usr/bin/env python3
"""The copy button and Run in terminal, in the GPUI window (gpui-f-rows.py's harness: private Xvfb + i3, real X input, the real X clipboard).
usage: gpui-copy-rows.py CLIENT_BIN HUB_BIN ASSETS_DIR OUT [PORT] [DISPLAY_N] [rows...]   rows: F23 (copy on each chat surface), F24 (Run in terminal)
A project thread, the uberbot Bub's chat and a room each hold a reply with an inline command, a fenced command and a commit hash."""
import os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
os.environ['ROWS_SEED'] = os.path.join(HERE, 'gpui-copy-rows.seed.py'); os.environ['ROWS_FILE'] = os.path.join(HERE, 'gpui-copy-rows.rows.py')
sys.argv = [os.path.join(HERE, 'gpui-f-rows.py')] + sys.argv[1:]
try: exec(open(sys.argv[0]).read())
except BaseException:
    if 'cleanup' in globals(): cleanup()   # the harness's: its hub, X server, window and clipboard owners go too
    raise
