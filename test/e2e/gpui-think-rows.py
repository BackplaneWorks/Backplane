#!/usr/bin/env python3
"""A thought folds and opens in the GPUI window (#1325; gpui-f-rows.py's harness: private Xvfb + i3, real X input).
usage: gpui-think-rows.py CLIENT_BIN HUB_BIN ASSETS_DIR OUT [PORT] [DISPLAY_N] [rows...]   rows: F27 (a thought shut, opened by a click, shut and opened by the keyboard)
A project thread holds a person's message, the agent's thinking (three paragraphs) and its reply, with Show thinking on."""
import os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
os.environ['ROWS_SEED'] = os.path.join(HERE, 'gpui-think-rows.seed.py'); os.environ['ROWS_FILE'] = os.path.join(HERE, 'gpui-think-rows.rows.py')
sys.argv = [os.path.join(HERE, 'gpui-f-rows.py')] + sys.argv[1:]
try: exec(open(sys.argv[0]).read())
except BaseException:
    if 'cleanup' in globals(): cleanup()   # the harness's: its hub, X server and window go too
    raise
