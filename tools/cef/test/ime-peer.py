#!/usr/bin/python3
# Independent GTK editor, observed preedit and committed text from real IBus.
import gi
import json
import os
import sys
gi.require_version('Gtk', '4.0')
gi.require_version('GdkX11', '4.0')
gi.require_version('IBus', '1.0')
from gi.repository import Gtk, GdkX11, GLib, IBus
IBus.init()
bus = IBus.Bus.new()
if sys.argv[-1] == '--stop-private-ibus':
    if bus.is_connected():
        bus.exit(False)
    raise SystemExit(0)
if not bus.is_connected():
    raise RuntimeError('private IBus is not connected')
engines = [engine.get_name() for engine in bus.list_engines()]
if 'mozc-jp' not in engines:
    raise RuntimeError('real Mozc engine missing from private IBus')
if not bus.set_global_engine('mozc-jp'):
    raise RuntimeError('could not select Mozc')
app = Gtk.Application(application_id='dev.backplane.CefImeAcceptance')

def activate(app):
    win = Gtk.ApplicationWindow(application=app, title='Independent real Mozc baseline')
    win.set_decorated(False)
    win.set_default_size(400, 120)
    entry = Gtk.Entry(height_request=80)
    win.set_child(entry)
    entry.connect('changed', lambda e: print(json.dumps({'text': e.get_text()}), flush=True))
    entry.get_delegate().connect('preedit-changed', lambda _, text: print(json.dumps({'preedit': text}), flush=True))
    win.present()
    def ready():
        entry.grab_focus()
        print(json.dumps({'window': win.get_surface().get_xid(), 'engine': bus.get_global_engine().get_name()}), flush=True)
        return False
    GLib.idle_add(ready)

def eof(_fd, condition):
    if condition & GLib.IO_HUP or not os.read(0, 1024):
        app.quit()
        return False
    return True

app.connect('activate', activate)
GLib.io_add_watch(0, GLib.IO_IN | GLib.IO_HUP, eof)
app.run([sys.argv[0]])
