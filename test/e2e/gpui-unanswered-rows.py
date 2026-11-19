#!/usr/bin/env python3
"""#1304: the "hasn't answered" notice in the GPUI window (gpui-f-rows.py's harness: private Xvfb + i3, a private hub, the stand-in `claude`).
usage: gpui-unanswered-rows.py CLIENT_BIN HUB_BIN ASSETS_DIR OUT [PORT] [DISPLAY_N] [rows...]   rows: F26
A long turn runs (the stand-in answers nothing while it does); the person sends "Status?" into it; a minute later the window shows the hub's notice
over the composer, and Send again stops the turn and sends "Status?" as a new one."""
import os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
os.environ['ROWS_SEED'] = os.path.join(HERE, 'gpui-unanswered-rows.seed.py'); os.environ['ROWS_FILE'] = os.path.join(HERE, 'gpui-unanswered-rows.rows.py')
sys.argv = [os.path.join(HERE, 'gpui-f-rows.py')] + sys.argv[1:]
try: exec(open(sys.argv[0]).read())
except BaseException:
    if 'cleanup' in globals(): cleanup()
    raise
