#!/usr/bin/env python3
"""Stand-in for xdg-desktop-portal's FileChooser on the session bus, so a client's `prompt_for_paths` (gpui -> ashpd) has something to talk to on a bare Boat sandbox.
It answers OpenFile with the file(s) in PORTAL_PICK (colon separated) and logs every call to PORTAL_LOG. Not a measurement of anything: it only lets the paperclip click be driven.
usage: PORTAL_PICK=/a.png PORTAL_LOG=/tmp/portal.log python3 portal_standin.py   (needs a session bus and `pip install dbus-next`)"""
import asyncio, os, sys, time
from dbus_next.aio import MessageBus
from dbus_next.service import ServiceInterface, method, signal, dbus_property
from dbus_next import Variant, BusType, PropertyAccess
log = open(os.environ.get('PORTAL_LOG', '/tmp/portal.log'), 'a', buffering=1)
def say(*a): log.write(time.strftime('%H:%M:%S ') + ' '.join(str(x) for x in a) + '\n')
class Request(ServiceInterface):
    def __init__(self): super().__init__('org.freedesktop.portal.Request')
    @method()
    def Close(self): pass
    @signal()
    def Response(self, response: 'u', results: 'a{sv}') -> 'ua{sv}': return [response, results]
class FileChooser(ServiceInterface):
    def __init__(self, bus): super().__init__('org.freedesktop.portal.FileChooser'); self.bus = bus; self.sender = None
    @dbus_property(access=PropertyAccess.READ)
    def version(self) -> 'u': return 3
    @method()
    def OpenFile(self, parent: 's', title: 's', options: 'a{sv}') -> 'o':
        token = options['handle_token'].value if 'handle_token' in options else 'e2e'
        sender = (self.sender or ':1.0')[1:].replace('.', '_')
        path = f'/org/freedesktop/portal/desktop/request/{sender}/{token}'
        say('OpenFile', repr(title), 'token', token, 'request', path, 'multiple', options.get('multiple', Variant('b', False)).value)
        req = Request(); self.bus.export(path, req)
        picks = [p for p in os.environ.get('PORTAL_PICK', '').split(':') if p]
        async def respond():
            await asyncio.sleep(0.4)
            req.Response(0, {'uris': Variant('as', ['file://' + p for p in picks])}); say('Response', picks)
        asyncio.get_event_loop().create_task(respond())
        return path
async def main():
    bus = await MessageBus(bus_type=BusType.SESSION).connect()
    fc = FileChooser(bus)
    def handler(msg):
        if msg.member == 'OpenFile': fc.sender = msg.sender
        return False
    bus.add_message_handler(handler)
    bus.export('/org/freedesktop/portal/desktop', fc)
    await bus.request_name('org.freedesktop.portal.Desktop')
    say('ready on', bus.unique_name); open(os.environ.get('PORTAL_LOG', '/tmp/portal.log') + '.ready', 'w').write('1')
    await asyncio.get_event_loop().create_future()
asyncio.run(main())
