"""A small RFB (VNC) client for macOS Screen Sharing with Apple Remote Desktop authentication (security type 30).

The Mac mini gate drives the Mac's desktop through its Screen Sharing from the Linux machine: screensharingd is already trusted
by macOS, so no Accessibility or Screen Recording grant is needed (TCC charges an ssh session's input and capture to
sshd-keygen-wrapper; this path never asks). It is the path a person's own Screen Sharing takes.

Type 30 (what macOS 26 answers; its RSA variant, type 33, closes the connection): the server sends a Diffie-Hellman generator,
key length, prime and its public key; the client answers AES-128-ECB(MD5(shared secret), user[64] + password[64]) followed by
its own public key. User and password are each NUL-terminated and filled with random bytes to 64.

The password is read from a file (Backplane's secret_run `files`), used once for the handshake and once if `login()` types it
at the login window, and never kept, logged or put on a command line. While it is being typed, `png()` writes a black frame.

    python3 ard_vnc.py HOST --user h --password-file $PW shot out.png
"""
import argparse
import contextlib
import hashlib
import os
import socket
import struct
import time
import zlib

from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

# X keysyms for the keys the gate uses (macOS Screen Sharing: Super_L is Command, Alt_L is Option)
KEYSYMS = {
    'cmd': 0xffeb, 'ctrl': 0xffe3, 'shift': 0xffe1, 'alt': 0xffe9, 'option': 0xffe9,
    'return': 0xff0d, 'enter': 0xff0d, 'escape': 0xff1b, 'esc': 0xff1b, 'tab': 0xff09, 'backspace': 0xff08,
    'delete': 0xffff, 'space': 0x20, 'left': 0xff51, 'up': 0xff52, 'right': 0xff53, 'down': 0xff54,
    'home': 0xff50, 'end': 0xff57, 'pageup': 0xff55, 'pagedown': 0xff56, 'grave': 0x60, '`': 0x60,
    **{f'f{i}': 0xffbe + i - 1 for i in range(1, 13)},
}


# US layout: the characters typed with Shift. The X convention, which macOS Screen Sharing follows: Shift held and the shifted
# character's own keysym ("A", "!"). Shift with the unshifted keysym ("a") typed "a" at the login window: the password arrived
# with its capitals in lower case (loginwindow: ODErrorCredentialsInvalid while the same password passed ARD's own check).
SHIFTED = set('~!@#$%^&*()_+{}|:"<>?')


def strokes(ch):
    """The keys for one character: (Shift held, the keysym's name)."""
    return (ch.isascii() and ch.isupper()) or ch in SHIFTED, ch


def keysym(name):
    k = KEYSYMS.get(name.lower())
    if k is not None:
        return k
    if len(name) == 1:
        return ord(name)
    raise ValueError(f'unknown key {name!r}')


def _field(text, fill):
    b = text.encode('utf-8')
    if len(b) > 63:
        raise ValueError('an ARD credential is at most 63 bytes')
    return b + b'\0' + fill[:63 - len(b)]


def ard_response(generator, prime, server_pub, user, password, private=None, fill=None):
    """The client's answer to type 30: 128 bytes of encrypted credentials, then its public key (as long as the prime)."""
    n = len(prime)
    p = int.from_bytes(prime, 'big')
    g = int.from_bytes(generator, 'big')
    x = private if private is not None else int.from_bytes(os.urandom(n), 'big') % (p - 3) + 2
    pub = pow(g, x, p).to_bytes(n, 'big')
    shared = pow(int.from_bytes(server_pub, 'big'), x, p).to_bytes(n, 'big')
    key = hashlib.md5(shared).digest()
    fill = fill if fill is not None else os.urandom(128)
    creds = _field(user, fill[:64]) + _field(password, fill[64:])
    enc = Cipher(algorithms.AES(key), modes.ECB()).encryptor()
    return enc.update(creds) + enc.finalize() + pub


class AuthError(Exception):
    pass


class Client:
    """One Screen Sharing connection: the framebuffer (BGRX, 32 bpp) and the keyboard and mouse."""

    def __init__(self, host, port=5900, timeout=20):
        self.host, self.port = host, port
        self.sock = socket.create_connection((host, port), timeout=timeout)
        self.width = self.height = 0
        self.name = ''
        self.fb = bytearray()
        self.buttons = 0
        self.pos = (0, 0)
        self._masked = 0

    # ---- wire ----
    def _rx(self, n):
        b = bytearray()
        while len(b) < n:
            c = self.sock.recv(n - len(b))
            if not c:
                raise ConnectionError(f'the server closed the connection ({len(b)} of {n} bytes)')
            b += c
        return bytes(b)

    def _u8(self):
        return self._rx(1)[0]

    def _u16(self):
        return struct.unpack('>H', self._rx(2))[0]

    def _u32(self):
        return struct.unpack('>I', self._rx(4))[0]

    # ---- handshake ----
    def connect(self, user, password_file):
        intro = self._rx(12)
        if not intro.startswith(b'RFB '):
            raise ConnectionError(f'not a VNC server: {intro!r}')
        self.sock.sendall(b'RFB 003.008\n')
        count = self._u8()
        if count == 0:
            raise AuthError(self._rx(self._u32()).decode('utf-8', 'replace'))
        types = set(self._rx(count))
        if 30 not in types:
            raise AuthError(f'no Apple Remote Desktop authentication (type 30) offered: {sorted(types)}')
        self.sock.sendall(bytes([30]))
        generator = self._rx(2)
        n = self._u16()
        prime, server_pub = self._rx(n), self._rx(n)
        with open(password_file, encoding='utf-8') as f:
            answer = ard_response(generator, prime, server_pub, user, f.read().rstrip('\r\n'))
        self.sock.sendall(answer)
        if self._u32() != 0:
            reason = ''
            with contextlib.suppress(Exception):
                reason = self._rx(self._u32()).decode('utf-8', 'replace')
            raise AuthError(f'authentication failed {reason}'.strip())
        self.sock.sendall(b'\x01')  # ClientInit: shared (a person's own session may stay connected)
        self.width, self.height = self._u16(), self._u16()
        self._rx(16)  # the server's pixel format: ours is set next
        self.name = self._rx(self._u32()).decode('utf-8', 'replace')
        # SetPixelFormat: 32 bpp, depth 24, little endian, true colour, red at 16, green at 8, blue at 0 (BGRX in memory)
        self.sock.sendall(b'\x00\0\0\0' + struct.pack('>BBBBHHHBBB3x', 32, 24, 0, 1, 255, 255, 255, 16, 8, 0))
        # SetEncodings: raw, and DesktopSize (the framebuffer follows a display change)
        self.sock.sendall(struct.pack('>BxHii', 2, 2, 0, -223))
        self.fb = bytearray(self.width * self.height * 4)
        return self

    def close(self):
        with contextlib.suppress(Exception):
            self.sock.close()

    # ---- framebuffer ----
    def refresh(self, incremental=False, timeout=10.0):
        """Ask for the screen and read until one framebuffer update has been applied."""
        self.sock.sendall(struct.pack('>BBHHHH', 3, 1 if incremental else 0, 0, 0, self.width, self.height))
        end = time.time() + timeout
        while time.time() < end:
            kind = self._u8()
            if kind == 0:
                self._update()
                return self
            if kind == 1:  # colour map entries (not used at true colour)
                self._rx(1)
                self._rx(2)
                self._rx(self._u16() * 6)
            elif kind == 2:  # bell
                pass
            elif kind == 3:  # server cut text
                self._rx(3)
                self._rx(self._u32())
            else:
                raise ConnectionError(f'unknown server message {kind}')
        raise TimeoutError('no framebuffer update')

    def _update(self):
        self._rx(1)
        for _ in range(self._u16()):
            x, y, w, h = struct.unpack('>HHHH', self._rx(8))
            enc = struct.unpack('>i', self._rx(4))[0]
            if enc == 0:
                data = self._rx(w * h * 4)
                for row in range(h):
                    at = ((y + row) * self.width + x) * 4
                    self.fb[at:at + w * 4] = data[row * w * 4:(row + 1) * w * 4]
            elif enc == -223:
                self.width, self.height = w, h
                self.fb = bytearray(w * h * 4)
            else:
                raise ConnectionError(f'unexpected encoding {enc}')

    def pixel(self, x, y):
        at = (y * self.width + x) * 4
        b, g, r = self.fb[at:at + 3]
        return r, g, b

    @contextlib.contextmanager
    def masked(self):
        """Every png() while this is open (a password being typed, and the second after) is black."""
        self._masked += 1
        try:
            yield
        finally:
            time.sleep(1.0)
            self._masked -= 1

    def png(self, path):
        w, h = self.width, self.height
        if self._masked:
            raw = b''.join(b'\0' + bytes(w * 3) for _ in range(h))
        else:
            rows = []
            for y in range(h):
                line = self.fb[y * w * 4:(y + 1) * w * 4]
                rgb = bytearray(w * 3)
                rgb[0::3], rgb[1::3], rgb[2::3] = line[2::4], line[1::4], line[0::4]
                rows.append(b'\0' + bytes(rgb))
            raw = b''.join(rows)

        def chunk(t, d):
            return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d))
        with open(path, 'wb') as f:
            f.write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0))
                    + chunk(b'IDAT', zlib.compress(raw, 6)) + chunk(b'IEND', b''))
        return path

    # ---- input ----
    def key(self, name, down):
        self.sock.sendall(struct.pack('>BBxxI', 4, 1 if down else 0, keysym(name)))

    def chord(self, combo, hold=0.03):
        """'cmd+k', 'ctrl+shift+tab', 'escape': modifiers down in order, the key tapped, modifiers up in reverse."""
        keys = combo.split('+') if combo != '+' else ['+']
        for k in keys:
            self.key(k, True)
            time.sleep(hold)
        for k in reversed(keys):
            self.key(k, False)
            time.sleep(hold)

    def type_text(self, text, gap=0.04):
        for ch in text:
            shift, k = strokes(ch)
            if shift:
                self.key('shift', True)
                time.sleep(gap / 2)
            self.key(k, True)
            self.key(k, False)
            if shift:
                time.sleep(gap / 2)
                self.key('shift', False)
            time.sleep(gap)

    def move(self, x, y):
        self.pos = (int(x), int(y))
        self.sock.sendall(struct.pack('>BBHH', 5, self.buttons, *self.pos))

    def button(self, down, which=1):
        mask = 1 << (which - 1)
        self.buttons = self.buttons | mask if down else self.buttons & ~mask
        self.sock.sendall(struct.pack('>BBHH', 5, self.buttons, *self.pos))

    def click(self, x, y, which=1):
        self.move(x, y)
        time.sleep(0.05)
        self.button(True, which)
        time.sleep(0.05)
        self.button(False, which)

    def drag(self, x0, y0, x1, y1, steps=30, seconds=0.6):
        self.move(x0, y0)
        time.sleep(0.05)
        self.button(True)
        for i in range(1, steps + 1):
            self.move(x0 + (x1 - x0) * i / steps, y0 + (y1 - y0) * i / steps)
            time.sleep(seconds / steps)
        self.button(False)

    def count_dots(self, x0, x1, y, bright=140, min_run=6):
        """The bright runs along row y between x0 and x1 (a password field's dots; the caret, 1-3 px, is too thin to count).
        Read from the framebuffer in memory: nothing is written. macOS 26's login field at 3840x2160: dots about 10 px wide,
        6 px apart, at ~170 on a ~68 field; the caret right after the last dot, ~125 between them."""
        runs, run = 0, 0
        for x in range(x0, x1):
            r, g, b = self.pixel(x, y)
            if min(r, g, b) >= bright:
                run += 1
            else:
                runs += run >= min_run
                run = 0
        return runs + (run >= min_run)

    def dots_in_field(self, x0, x1, y, reach=25):
        """The most dots on any row within `reach` of the field's middle row."""
        return max(self.count_dots(x0, x1, yy) for yy in range(max(0, y - reach), min(self.height, y + reach + 1)))

    # ---- the login window ----
    def login(self, password_file, settle=8.0, field=(0.4583, 0.5417, 0.9083)):
        """Type the password at the login window (the account shown is the one auto-login names) and press Return.
        `field` is the password field as fractions of the screen: its left and right edge and its middle row (macOS 26's
        login window: bottom centre, under the account's picture). Before Return, the field's dots are counted in memory
        and must be as many as the password's characters; otherwise the field is cleared and nothing is submitted
        (LoginAbort says only that they differ). Screenshots stay black while it is typed."""
        x0, x1, y = int(self.width * field[0]), int(self.width * field[1]), int(self.height * field[2])
        with self.masked():
            self.click((x0 + x1) // 2, y)
            time.sleep(0.5)
            self.chord('cmd+a')  # whatever the field held goes
            self.chord('backspace')
            time.sleep(0.3)
            with open(password_file, encoding='utf-8') as f:
                secret = f.read().rstrip('\r\n')
            self.type_text(secret, gap=0.08)
            time.sleep(1.0)  # Return straight after the last key was not taken as the login button (no attempt logged)
            self.refresh()
            same = self.dots_in_field(x0, x1, y) == len(secret)
            del secret
            if not same:
                self.chord('cmd+a')
                self.chord('backspace')
                raise LoginAbort('the password field does not hold as many characters as were typed: nothing submitted')
            self.chord('return')
        time.sleep(settle)


class LoginAbort(Exception):
    pass


def main():
    a = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    a.add_argument('host')
    a.add_argument('--port', type=int, default=5900)
    a.add_argument('--user', required=True)
    a.add_argument('--password-file', required=True)
    sub = a.add_subparsers(dest='cmd', required=True)
    s = sub.add_parser('shot')
    s.add_argument('out')
    sub.add_parser('info')
    sub.add_parser('login')
    o = a.parse_args()
    c = Client(o.host, o.port).connect(o.user, o.password_file)
    try:
        print(f'connected: {c.name!r} {c.width}x{c.height}', flush=True)
        if o.cmd == 'login':
            c.login(o.password_file)
            c.refresh()
            print(f'after login: {c.width}x{c.height}', flush=True)
        elif o.cmd == 'shot':
            c.refresh()
            print(c.png(o.out), flush=True)
    finally:
        c.close()


if __name__ == '__main__':
    main()
