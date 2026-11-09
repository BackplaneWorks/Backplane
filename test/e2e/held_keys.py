#!/usr/bin/env python3
"""The window's keyboard shortcuts pressed with real keys (XTest), and the hints held Ctrl/Alt show, each screenshotted: held_keys.py OUTDIR XINPUT.py
Needs DISPLAY set to a private X server with the client running on a hub whose sidebar has three projects (the third with four threads, the second its
second thread named "Firmware pin map"), e.g. the seeded rig in the n1109 notes. Judged by eye from the shots (OUTDIR/NN-name.png) and by what reached the hub."""
import os, subprocess, sys, time
OUT, XIN = sys.argv[1], sys.argv[2]
assert os.environ.get('DISPLAY') not in (None, '', ':0'), 'a private display only'
os.makedirs(OUT, exist_ok=True)
n = [0]
def x(*a): subprocess.run(['python3', XIN, *a], check=True)
def shot(name, crop=None):
    n[0] += 1; time.sleep(0.6)
    subprocess.run(['import', '-window', 'root', *(['-crop', crop] if crop else []), f'{OUT}/{n[0]:02d}-{name}.png'], check=False)
def held(mod, name, crop=None): x('down', mod); time.sleep(0.4); shot(name, crop); x('up', mod); time.sleep(0.3)
def chord(*k): x('chord', *k); time.sleep(0.5)
def key(k): x('key', k); time.sleep(0.4)

x('focus', 'Backplane')
x('click', '200', '462'); time.sleep(1)                                  # a thread of the third project
held('Control_L', 'ctrl-held')                                          # thread digits, rail keys, Ctrl K, Ctrl M, the legend
held('Alt_L', 'alt-held', '320x620+0+0')                                # project digits
chord('ctrl', '1'); shot('ctrl-1-first-thread', '700x40+0+0')
chord('alt', '2'); shot('alt-2-second-project')
chord('ctrl', '2'); shot('ctrl-2-thread-of-that-project', '700x40+0+0')
chord('ctrl', 'shift', 'bracketright'); shot('ctrl-shift-]-next-thread', '700x40+0+0')
chord('ctrl', 'shift', 'bracketleft'); shot('ctrl-shift-[-prev-thread', '700x40+0+0')
chord('ctrl', 'k'); shot('ctrl-k-palette'); held('Control_L', 'ctrl-held-under-palette-no-hints'); key('Escape'); shot('esc-shuts-palette')
chord('ctrl', 'shift', 'p'); shot('ctrl-shift-p-palette'); chord('ctrl', 'shift', 'p'); shot('ctrl-shift-p-again-shuts')
chord('ctrl', 'b'); shot('ctrl-b-fold'); chord('ctrl', 'b'); shot('ctrl-b-unfold')
chord('ctrl', 'shift', 'f'); shot('ctrl-shift-f-search', '320x120+0+0'); key('Escape')
chord('ctrl', 'm'); shot('ctrl-m-model-menu'); key('Escape'); shot('esc-shuts-model-menu')
chord('ctrl', 'j'); shot('ctrl-j-terminal'); held('Control_L', 'ctrl-held-in-terminal-no-hints'); chord('ctrl', 'j'); shot('ctrl-j-hides')
chord('super', 'grave'); shot('super-grave-terminal'); chord('shift', 'Escape'); shot('shift-esc-leaves-terminal')
key('Escape'); shot('esc-leaves-the-message-box')                       # nothing to put away: the keyboard leaves the composer
key('b'); time.sleep(2); shot('b-board')
held('Control_L', 'ctrl-held-board-lit')
key('Escape'); time.sleep(1); shot('esc-shuts-the-viewer')
key('Escape'); key('s'); time.sleep(2); shot('s-schematic'); key('Escape'); time.sleep(1)
chord('ctrl', 'd'); time.sleep(1); shot('ctrl-d-diff'); chord('ctrl', 'd'); time.sleep(1)
chord('ctrl', 'p'); time.sleep(1); shot('ctrl-p-files'); key('Escape'); time.sleep(1)
key('Escape'); key('slash'); x('type', 'hello from the keys'); shot('slash-focuses-the-message-box')
chord('ctrl', 'Return'); time.sleep(1.5); shot('ctrl-enter-sends')
chord('ctrl', 'period'); shot('ctrl-period-stop')
chord('ctrl', 'shift', 's'); time.sleep(1); shot('ctrl-shift-s-settle')
chord('ctrl', 'shift', 's'); time.sleep(1)
chord('ctrl', 'shift', 'a'); shot('ctrl-shift-a-account-menu'); key('Escape')
chord('ctrl', 'shift', 'h'); shot('ctrl-shift-h-perf'); chord('ctrl', 'shift', 'h')
chord('ctrl', 'n'); time.sleep(1.5); shot('ctrl-n-new-thread')
print('shots:', n[0])
