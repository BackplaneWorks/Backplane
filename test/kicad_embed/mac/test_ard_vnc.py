"""ard_vnc against a fake Screen Sharing server that speaks type 30 with the Mac mini's own recorded parameters.

The recorded part (hs-mac-mini, macOS 26.5, probed without credentials): RFB 003.889, security types [30, 33, 36, 2, 35],
type 30 with generator 2 and a 128-byte prime (RFC 2409's Oakley group 2, the same on every connection). The server's
private half is the fake's own, so the test can decrypt what the client sends and check it.
    python3 test/kicad_embed/mac/test_ard_vnc.py
"""
import hashlib
import os
import socket
import struct
import sys
import tempfile
import threading
import unittest

from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ard_vnc  # noqa: E402

PRIME = bytes.fromhex(
    'ffffffffffffffffc90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b139b22514a08798e3404ddef9519b3cd3a431b'
    '302b0a6df25f14374fe1356d6d51c245e485b576625e7ec6f44c42e9a637ed6b0bff5cb6f406b7edee386bfb5a899fa5ae9f24117c4b1fe6'
    '49286651ece65381ffffffffffffffff')
TYPES = bytes([30, 33, 36, 2, 35])


def rx(c, n):
    b = b''
    while len(b) < n:
        d = c.recv(n - len(b))
        if not d:
            raise ConnectionError
        b += d
    return b


class FakeMac(threading.Thread):
    """One connection: the handshake as the mini answers it, then a 4x2 framebuffer, then it records input messages."""

    def __init__(self, user, password):
        super().__init__(daemon=True)
        self.user, self.password = user, password
        self.srv = socket.socket()
        self.srv.bind(('127.0.0.1', 0))
        self.srv.listen(1)
        self.port = self.srv.getsockname()[1]
        self.seen = {}
        self.input = []

    def run(self):
        c, _ = self.srv.accept()
        try:
            c.sendall(b'RFB 003.889\n')
            self.seen['version'] = rx(c, 12)
            c.sendall(bytes([len(TYPES)]) + TYPES)
            self.seen['type'] = rx(c, 1)[0]
            p = int.from_bytes(PRIME, 'big')
            y = int.from_bytes(os.urandom(128), 'big') % (p - 3) + 2
            c.sendall(b'\x00\x02' + struct.pack('>H', 128) + PRIME + pow(2, y, p).to_bytes(128, 'big'))
            ct, client_pub = rx(c, 128), rx(c, 128)
            key = hashlib.md5(pow(int.from_bytes(client_pub, 'big'), y, p).to_bytes(128, 'big')).digest()
            dec = Cipher(algorithms.AES(key), modes.ECB()).decryptor()
            creds = dec.update(ct) + dec.finalize()
            user = creds[:64].split(b'\0')[0].decode()
            password = creds[64:].split(b'\0')[0].decode()
            self.seen['user'] = user
            ok = (user, password) == (self.user, self.password)
            self.seen['password_ok'] = ok
            if not ok:
                c.sendall(struct.pack('>I', 1) + struct.pack('>I', 6) + b'denied')
                return
            c.sendall(struct.pack('>I', 0))
            self.seen['shared'] = rx(c, 1)[0]
            c.sendall(struct.pack('>HH', 4, 2) + bytes(16) + struct.pack('>I', 4) + b'mini')
            self.seen['pixel_format'] = rx(c, 20)
            head = rx(c, 4)
            self.seen['encodings'] = struct.unpack(f'>{head[3]}i', rx(c, head[3] * 4))
            rx(c, 10)  # the update request
            # a bell, then a 4x2 update in two rectangles: (1, 0) red, the rest blue
            px = lambda r, g, b: bytes([b, g, r, 0])  # noqa: E731
            c.sendall(b'\x02' + b'\x00\x00' + struct.pack('>H', 2)
                      + struct.pack('>HHHHi', 0, 0, 4, 1, 0) + px(0, 0, 255) + px(255, 0, 0) + px(0, 0, 255) * 2
                      + struct.pack('>HHHHi', 0, 1, 4, 1, 0) + px(0, 0, 255) * 4)
            while True:
                kind = rx(c, 1)[0]
                self.input.append((kind, rx(c, 7 if kind == 4 else 5)))
        except ConnectionError:
            pass
        finally:
            c.close()


class ArdVnc(unittest.TestCase):
    def secret(self, text):
        f = tempfile.NamedTemporaryFile('w', delete=False)
        f.write(text + '\n')
        f.close()
        self.addCleanup(os.unlink, f.name)
        return f.name

    def test_type_30_login_framebuffer_and_input(self):
        fake = FakeMac('h', 'correct horse')
        fake.start()
        c = ard_vnc.Client('127.0.0.1', fake.port).connect('h', self.secret('correct horse'))
        self.assertEqual(fake.seen['version'], b'RFB 003.008\n')
        self.assertEqual(fake.seen['type'], 30)
        self.assertTrue(fake.seen['password_ok'])
        self.assertEqual(fake.seen['shared'], 1)
        self.assertEqual((c.width, c.height, c.name), (4, 2, 'mini'))
        # the password is nowhere in the client once connected
        self.assertNotIn('correct horse', repr(vars(c)))
        c.refresh()
        self.assertEqual(fake.seen['encodings'], (0, -223))
        self.assertEqual(c.pixel(1, 0), (255, 0, 0))
        self.assertEqual(c.pixel(3, 1), (0, 0, 255))
        out = tempfile.NamedTemporaryFile(suffix='.png', delete=False).name
        self.addCleanup(os.unlink, out)
        self.assertTrue(open(c.png(out), 'rb').read().startswith(b'\x89PNG'))
        # Cmd+K: Super_L down, k down, k up, Super_L up; then "Ab": Shift with the capital's own keysym; a click at (3, 1)
        c.chord('cmd+k', hold=0)
        c.type_text('Ab', gap=0)
        c.click(3, 1)
        c.close()
        fake.join(5)
        keys = [(struct.unpack('>BxxI', d)) for k, d in fake.input if k == 4]
        self.assertEqual(keys, [(1, 0xffeb), (1, ord('k')), (0, ord('k')), (0, 0xffeb),
                                (1, 0xffe1), (1, ord('A')), (0, ord('A')), (0, 0xffe1), (1, ord('b')), (0, ord('b'))])
        ptr = [struct.unpack('>BHH', d) for k, d in fake.input if k == 5]
        self.assertEqual(ptr, [(0, 3, 1), (1, 3, 1), (0, 3, 1)])

    def test_a_wrong_password_is_refused_with_the_server_reason(self):
        fake = FakeMac('h', 'right')
        fake.start()
        with self.assertRaises(ard_vnc.AuthError) as e:
            ard_vnc.Client('127.0.0.1', fake.port).connect('h', self.secret('wrong'))
        self.assertIn('denied', str(e.exception))
        self.assertNotIn('wrong', str(e.exception))

    def test_a_masked_screenshot_is_black(self):
        c = ard_vnc.Client.__new__(ard_vnc.Client)
        c.width, c.height, c.fb, c._masked = 2, 1, bytearray([0, 0, 255, 0] * 2), 1
        out = tempfile.NamedTemporaryFile(suffix='.png', delete=False).name
        self.addCleanup(os.unlink, out)
        import zlib
        data = open(c.png(out), 'rb').read()
        idat = data[data.index(b'IDAT') + 4:data.index(b'IEND') - 8]
        self.assertEqual(zlib.decompress(idat), b'\0' + bytes(6))

    def test_capitals_and_symbols_are_typed_with_shift_and_their_own_keysym(self):
        self.assertEqual(ard_vnc.strokes('A'), (True, 'A'))
        self.assertEqual(ard_vnc.keysym('A'), ord('A'))
        self.assertEqual(ard_vnc.strokes('!'), (True, '!'))
        self.assertEqual(ard_vnc.strokes('a'), (False, 'a'))
        self.assertEqual(ard_vnc.strokes('7'), (False, '7'))
        self.assertEqual(len(ard_vnc.SHIFTED), 21)

    def test_the_password_fields_dots_are_counted_and_the_caret_is_not(self):
        c = ard_vnc.Client.__new__(ard_vnc.Client)
        c.width, c.height = 60, 3
        dark, white = bytes([75, 95, 75, 0]), bytes([250, 250, 250, 0])
        row = [dark] * 60
        for x0 in (4, 17, 30):           # three dots, 8 px wide
            row[x0:x0 + 8] = [white] * 8
        row[45:47] = [white] * 2         # the caret, 2 px
        c.fb = bytearray(b''.join(row) * 3)
        self.assertEqual(c.count_dots(0, 60, 1), 3)
        self.assertEqual(c.count_dots(0, 20, 1), 1)  # a dot cut by the edge is short: not counted
        row[52:60] = [white] * 8         # a dot touching the right edge
        c.fb = bytearray(b''.join(row) * 3)
        self.assertEqual(c.count_dots(0, 60, 1), 4)

    def test_the_mini_s_recorded_login_field_row_reads_four_dots(self):
        # row 1958 of the mini's login field with "xxxx" typed (every 3rd pixel from x=1760, min channel), as recorded
        rec = [82, 69, 69, 69, 69, 69, 69, 69, 69, 68, 68, 170, 170, 169, 68, 68, 148, 169, 169, 127, 67, 67, 168, 168, 168, 66, 66,
               146, 168, 168, 126, 168, 115, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 67, 67, 67]
        c = ard_vnc.Client.__new__(ard_vnc.Client)
        c.width, c.height = len(rec) * 3, 1
        c.fb = bytearray(b''.join(bytes([v, v, v, 0]) * 3 for v in rec))
        self.assertEqual(c.count_dots(0, c.width, 0), 4)
        self.assertEqual(c.dots_in_field(0, c.width, 0), 4)

    def test_credentials_are_nul_terminated_and_filled_to_64(self):
        f = ard_vnc._field('h', b'\xaa' * 64)
        self.assertEqual(len(f), 64)
        self.assertEqual(f[:2], b'h\0')
        with self.assertRaises(ValueError):
            ard_vnc._field('x' * 64, bytes(64))


if __name__ == '__main__':
    unittest.main()
