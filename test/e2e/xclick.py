#!/usr/bin/env python3
"""A real pointer click on an X display, through XTest (no xdotool needed): xclick.py DISPLAY X Y [button]. Used to prove a control answers a click, not just
a scripted action. Coordinates are the screen's; a window's controls are at the window's absolute origin (xwininfo) plus the control's rect."""
import ctypes, ctypes.util, sys, time
x11 = ctypes.CDLL(ctypes.util.find_library("X11")); xt = ctypes.CDLL(ctypes.util.find_library("Xtst"))
x11.XOpenDisplay.restype = ctypes.c_void_p; x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
d = x11.XOpenDisplay(sys.argv[1].encode())
if not d: sys.exit("cannot open display " + sys.argv[1])
x, y, b = int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4]) if len(sys.argv) > 4 else 1
xt.XTestFakeMotionEvent(ctypes.c_void_p(d), -1, x, y, 0); x11.XFlush(ctypes.c_void_p(d)); time.sleep(0.15)
xt.XTestFakeButtonEvent(ctypes.c_void_p(d), b, 1, 0); x11.XFlush(ctypes.c_void_p(d)); time.sleep(0.08)
xt.XTestFakeButtonEvent(ctypes.c_void_p(d), b, 0, 0); x11.XFlush(ctypes.c_void_p(d)); time.sleep(0.1)
