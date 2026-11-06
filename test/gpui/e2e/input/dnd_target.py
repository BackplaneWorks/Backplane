#!/usr/bin/env python3
"""A tiny XDND-aware window to self-test dnd.py (prints the dropped paths). usage: dnd_target.py DISPLAY TITLE OUTFILE"""
import sys, time
from Xlib import X, display, protocol, Xatom
disp, title, out = sys.argv[1:4]
d = display.Display(disp); scr = d.screen(); root = scr.root
A = lambda n: d.intern_atom(n)
w = root.create_window(100, 100, 400, 300, 0, scr.root_depth, event_mask=X.StructureNotifyMask | X.ExposureMask)
w.set_wm_name(title); w.change_property(A('XdndAware'), Xatom.ATOM, 32, [5]); w.map(); d.flush()
XdndEnter, XdndPosition, XdndStatus, XdndDrop, XdndFinished, XdndSelection, Copy, uri = [A(n) for n in ('XdndEnter','XdndPosition','XdndStatus','XdndDrop','XdndFinished','XdndSelection','XdndActionCopy','text/uri-list')]
src = None; dropping = None; prop = A('E2E_DROP')
open(out + '.ready', 'w').write('1')
end = time.time() + 40
while time.time() < end:
    while d.pending_events():
        e = d.next_event()
        if e.type == X.ClientMessage:
            dat = e.data[1]
            if e.client_type == XdndEnter: src = dat[0]
            elif e.client_type == XdndPosition:
                s = d.create_resource_object('window', dat[0])
                s.send_event(protocol.event.ClientMessage(window=s, client_type=XdndStatus, data=(32, [w.id, 1, 0, 0, Copy])), event_mask=0); d.flush()
            elif e.client_type == XdndDrop:
                dropping = dat[0]; w.convert_selection(XdndSelection, uri, prop, X.CurrentTime); d.flush()
        elif e.type == X.SelectionNotify and dropping:
            r = w.get_full_property(prop, X.AnyPropertyType)
            open(out, 'w').write(bytes(r.value).decode() if r else 'EMPTY')
            s = d.create_resource_object('window', dropping)
            s.send_event(protocol.event.ClientMessage(window=s, client_type=XdndFinished, data=(32, [w.id, 1, Copy, 0, 0])), event_mask=0); d.flush()
            time.sleep(0.3); sys.exit(0)
    time.sleep(0.01)
sys.exit(4)
