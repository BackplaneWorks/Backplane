#!/usr/bin/env python3
"""XDND (v5) file drop source for Xvfb: drops `file://` URIs onto a window, like a file manager would.
usage: dnd.py DISPLAY TARGET_WINDOW_ID X Y FILE...   exits 0 when the target finished the drop (accepted), 2 when it refused, 3 on timeout"""
import sys, time
from Xlib import X, display, protocol, Xatom
disp, tid, px, py = sys.argv[1], int(sys.argv[2], 0), int(sys.argv[3]), int(sys.argv[4]); files = sys.argv[5:]
d = display.Display(disp); scr = d.screen(); root = scr.root
A = lambda n: d.intern_atom(n)
XdndEnter, XdndPosition, XdndStatus, XdndDrop, XdndFinished, XdndLeave = [A(n) for n in ('XdndEnter','XdndPosition','XdndStatus','XdndDrop','XdndFinished','XdndLeave')]
XdndSelection, XdndActionCopy, uri = A('XdndSelection'), A('XdndActionCopy'), A('text/uri-list')
src = root.create_window(0, 0, 1, 1, 0, scr.root_depth)
src.change_property(A('XdndAware'), Xatom.ATOM, 32, [5])
data = ''.join('file://%s\r\n' % f for f in files).encode()
src.set_selection_owner(XdndSelection, X.CurrentTime); d.flush()
tgt = d.create_resource_object('window', tid)
def send(atom, vals):
    ev = protocol.event.ClientMessage(window=tgt, client_type=atom, data=(32, (vals + [0]*5)[:5]))
    tgt.send_event(ev, event_mask=0); d.flush()
send(XdndEnter, [src.id, (5 << 24), uri, 0, 0])
send(XdndPosition, [src.id, 0, (px << 16) | py, X.CurrentTime, XdndActionCopy])
def pump(until, secs):
    end = time.time() + secs
    while time.time() < end:
        while d.pending_events():
            e = d.next_event()
            if e.type == X.SelectionRequest and e.selection == XdndSelection:
                prop = e.property or e.target
                e.requestor.change_property(prop, e.target, 8, data)
                e.requestor.send_event(protocol.event.SelectionNotify(time=e.time, requestor=e.requestor, selection=e.selection, target=e.target, property=prop), event_mask=0); d.flush()
            elif e.type == X.ClientMessage:
                r = until(e)
                if r is not None: return r
        time.sleep(0.01)
    return None
status = pump(lambda e: (e.data[1][1] & 1) if e.client_type == XdndStatus else None, 3)
if status is None: print('no XdndStatus (target not XDND-aware at that point?)'); sys.exit(3)
if not status: send(XdndLeave, [src.id]); print('target refused the drop at %d,%d' % (px, py)); sys.exit(2)
send(XdndDrop, [src.id, 0, X.CurrentTime])
fin = pump(lambda e: (1 + (e.data[1][1] & 1)) if e.client_type == XdndFinished else None, 5)
if fin is None: print('no XdndFinished'); sys.exit(3)
print('finished' if fin == 2 else 'finished, not accepted'); sys.exit(0 if fin == 2 else 2)
