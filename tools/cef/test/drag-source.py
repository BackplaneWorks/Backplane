#!/usr/bin/python3
# A real GTK drag peer; no DOM drag events or CDP file injection.
import gi
import json
import os
import pathlib
import sys
gi.require_version('Gtk', '4.0')
gi.require_version('GdkX11', '4.0')
from gi.repository import Gtk, Gdk, GdkX11, GLib

app = Gtk.Application(application_id='dev.backplane.CefDragAcceptance', flags=0)
window = None

def activate(app):
    global window
    window = Gtk.ApplicationWindow(application=app, title='CEF independent GTK drag peer')
    window.set_decorated(False)
    window.set_default_size(320, 320)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    window.set_child(box)
    uri = pathlib.Path(sys.argv[1]).as_uri() + '\r\n'
    for title, mime, data in [('Drag file', 'text/uri-list', uri), ('Drag text', 'text/plain;charset=utf-8', '日本語 drag text')]:
        label = Gtk.Label(label=title, height_request=80)
        source = Gtk.DragSource(actions=Gdk.DragAction.COPY)
        provider = Gdk.ContentProvider.new_for_bytes(mime, GLib.Bytes.new(data.encode('utf-8')))
        source.set_content(provider)
        source.connect("drag-begin", lambda *_: print(json.dumps({"drag": "begin"}), flush=True))
        source.connect("drag-end", lambda *_: print(json.dumps({"drag": "end"}), flush=True))
        source.connect("drag-cancel", lambda *_: print(json.dumps({"drag": "cancel"}), flush=True) or False)
        label.add_controller(source)
        box.append(label)
    target = Gtk.Label(label='Drop text here', height_request=160)
    drop = Gtk.DropTarget.new(str, Gdk.DragAction.COPY)
    def received(_target, value, x, y):
        print(json.dumps({'received': value}), flush=True)
        return True
    drop.connect('drop', received)
    target.add_controller(drop)
    box.append(target)
    window.present()
    def ready():
        print(json.dumps({'window': window.get_surface().get_xid()}), flush=True)
        return False
    GLib.idle_add(ready)

def eof(_fd, condition):
    if condition & GLib.IO_HUP or not os.read(0, 1024):
        app.quit()
        return False
    return True

app.connect('activate', activate)
GLib.io_add_watch(0, GLib.IO_IN | GLib.IO_HUP, eof)
# argv contains the file path for the provider, not GTK app arguments.
app.run([sys.argv[0]])
