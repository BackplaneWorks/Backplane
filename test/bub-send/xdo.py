#!/usr/bin/env python3
"""XTest driver: xdo.py [click X Y [btn]] [dclick X Y] [drag X1 Y1 X2 Y2] [type TEXT] [key ctrl+v] [move X Y] [wheel X Y N]  (DISPLAY from the environment)"""
import ctypes, ctypes.util, sys, time
x11 = ctypes.CDLL(ctypes.util.find_library("X11")); xt = ctypes.CDLL(ctypes.util.find_library("Xtst"))
x11.XOpenDisplay.restype = ctypes.c_void_p; x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
x11.XStringToKeysym.restype = ctypes.c_ulong; x11.XStringToKeysym.argtypes=[ctypes.c_char_p]
x11.XKeysymToKeycode.restype = ctypes.c_ubyte; x11.XKeysymToKeycode.argtypes=[ctypes.c_void_p, ctypes.c_ulong]
d = ctypes.c_void_p(x11.XOpenDisplay(__import__("os").environ["DISPLAY"].encode()))
def fl(): x11.XFlush(d)
def move(x,y): xt.XTestFakeMotionEvent(d,-1,x,y,0); fl(); time.sleep(0.12)
def btn(b,dn): xt.XTestFakeButtonEvent(d,b,dn,0); fl(); time.sleep(0.07)
def click(x,y,b=1): move(x,y); btn(b,1); btn(b,0)
def kc(name):
    ks = x11.XStringToKeysym(name.encode()); return x11.XKeysymToKeycode(d, ks), ks
def key(combo):
    parts = combo.split('+'); mods=[kc({'ctrl':'Control_L','shift':'Shift_L','alt':'Alt_L'}[m])[0] for m in parts[:-1]]
    k = kc(parts[-1])[0]
    for m in mods: xt.XTestFakeKeyEvent(d,m,1,0)
    xt.XTestFakeKeyEvent(d,k,1,0); xt.XTestFakeKeyEvent(d,k,0,0)
    for m in reversed(mods): xt.XTestFakeKeyEvent(d,m,0,0)
    fl(); time.sleep(0.08)
SYM={' ':'space','.':'period',',':'comma','/':'slash','&':'ampersand','@':'at','>':'greater','%':'percent','#':'numbersign','$':'dollar','-':'minus','_':'underscore',':':'colon','\n':'Return'}
def typ(s):
    for ch in s:
        n = SYM.get(ch, ch); code, ks = kc(n)
        shift = ch.isupper() or ch in '&@>%#$_:'
        if shift: xt.XTestFakeKeyEvent(d,kc('Shift_L')[0],1,0)
        xt.XTestFakeKeyEvent(d,code,1,0); xt.XTestFakeKeyEvent(d,code,0,0)
        if shift: xt.XTestFakeKeyEvent(d,kc('Shift_L')[0],0,0)
        fl(); time.sleep(0.05)
a = sys.argv[1:]; i = 0
while i < len(a):
    c = a[i]
    if c=='click': click(int(a[i+1]),int(a[i+2])); i+=3
    elif c=='rclick': click(int(a[i+1]),int(a[i+2]),3); i+=3
    elif c=='dclick': click(int(a[i+1]),int(a[i+2])); click(int(a[i+1]),int(a[i+2])); i+=3
    elif c=='drag':
        x1,y1,x2,y2=map(int,a[i+1:i+5]); move(x1,y1); btn(1,1)
        for k in range(1,11): move(x1+(x2-x1)*k//10, y1+(y2-y1)*k//10)
        btn(1,0); i+=5
    elif c=='move': move(int(a[i+1]),int(a[i+2])); i+=3
    elif c=='type': typ(a[i+1]); i+=2
    elif c=='key': key(a[i+1]); i+=2
    elif c=='wheel': move(int(a[i+1]),int(a[i+2])); [ (btn(4 if int(a[i+3])>0 else 5,1), btn(4 if int(a[i+3])>0 else 5,0)) for _ in range(abs(int(a[i+3])))]; i+=4
    elif c=='sleep': time.sleep(float(a[i+1])); i+=2
    else: sys.exit('bad '+c)
