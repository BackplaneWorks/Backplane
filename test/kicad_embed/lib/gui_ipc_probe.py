#!/usr/bin/env python3
"""Zero-dependency KiCad 10 IPC client over NNG ipc:// REQ0 + raw Protobuf wire encoding.

Matches native/host/src/kicad/nng.rs and proto.rs without requiring pynng or google.protobuf.
"""
import json
import os
import socket
import struct
import sys
import time

NNG_HELLO_REQ = b"\x00SP\x00\x00\x30\x00\x00"
NNG_HELLO_REP = b"\x00SP\x00\x00\x31\x00\x00"

DOCTYPE_MAP = {"sch": 1, "schematic": 1, "sym": 2, "pcb": 3, "board": 3, "fp": 4}
STATUS_NAMES = [
    "AS_UNKNOWN",
    "AS_OK",
    "AS_TIMEOUT",
    "AS_BAD_REQUEST",
    "AS_NOT_READY",
    "AS_UNHANDLED",
    "AS_TOKEN_MISMATCH",
    "AS_BUSY",
    "AS_UNIMPLEMENTED",
    "AS_INTERNAL_ERROR",
]


# ---- Minimal Protobuf wire codec -----------------------------------------------------------------
def pb_varint(val: int) -> bytes:
    if val < 0:
        val = (1 << 64) + val  # int64 two's complement varint
    out = bytearray()
    while True:
        b = val & 0x7F
        val >>= 7
        if val:
            out.append(b | 0x80)
        else:
            out.append(b)
            break
    return bytes(out)


def pb_field_varint(tag: int, val: int) -> bytes:
    if val == 0:
        return b""
    return pb_varint((tag << 3) | 0) + pb_varint(val)


def pb_field_bytes(tag: int, data: bytes) -> bytes:
    if not data:
        return b""
    return pb_varint((tag << 3) | 2) + pb_varint(len(data)) + data


def pb_field_msg(tag: int, data: bytes) -> bytes:
    # Always emit submessage even if empty when explicitly needed
    return pb_varint((tag << 3) | 2) + pb_varint(len(data)) + data


def pb_field_str(tag: int, text: str) -> bytes:
    return pb_field_bytes(tag, text.encode("utf-8"))


def pb_decode_fields(buf: bytes) -> dict[int, list]:
    out: dict[int, list] = {}
    i = 0
    n = len(buf)
    while i < n:
        key, i = _read_varint(buf, i)
        tag = key >> 3
        wtype = key & 0x07
        if wtype == 0:
            v, i = _read_varint(buf, i)
            out.setdefault(tag, []).append(v)
        elif wtype == 1:
            v = buf[i : i + 8]
            i += 8
            out.setdefault(tag, []).append(v)
        elif wtype == 2:
            ln, i = _read_varint(buf, i)
            v = buf[i : i + ln]
            i += ln
            out.setdefault(tag, []).append(v)
        elif wtype == 5:
            v = buf[i : i + 4]
            i += 4
            out.setdefault(tag, []).append(v)
        else:
            raise ValueError(f"unsupported wire type {wtype}")
    return out


def _read_varint(buf: bytes, i: int) -> tuple[int, int]:
    shift = 0
    res = 0
    while True:
        b = buf[i]
        i += 1
        res |= (b & 0x7F) << shift
        if not (b & 0x80):
            return res, i
        shift += 7


# ---- NNG REQ0 over AF_UNIX (Unix) or Named Pipe \\.\pipe\<path> (Windows) -----------------------
class _WinPipeStream:
    def __init__(self, sock_path: str) -> None:
        import ctypes
        from ctypes import wintypes

        self._k32 = ctypes.WinDLL("kernel32", use_last_error=True)
        self._k32.CreateFileW.argtypes = [
            wintypes.LPCWSTR,
            wintypes.DWORD,
            wintypes.DWORD,
            ctypes.c_void_p,
            wintypes.DWORD,
            wintypes.DWORD,
            wintypes.HANDLE,
        ]
        self._k32.CreateFileW.restype = wintypes.HANDLE
        self._k32.ReadFile.argtypes = [
            wintypes.HANDLE,
            ctypes.c_void_p,
            wintypes.DWORD,
            ctypes.POINTER(wintypes.DWORD),
            ctypes.c_void_p,
        ]
        self._k32.ReadFile.restype = wintypes.BOOL
        self._k32.WriteFile.argtypes = [
            wintypes.HANDLE,
            ctypes.c_void_p,
            wintypes.DWORD,
            ctypes.POINTER(wintypes.DWORD),
            ctypes.c_void_p,
        ]
        self._k32.WriteFile.restype = wintypes.BOOL
        self._k32.CloseHandle.argtypes = [wintypes.HANDLE]
        self._k32.CloseHandle.restype = wintypes.BOOL

        pipe_name = sock_path if sock_path.startswith("\\\\.\\pipe\\") else f"\\\\.\\pipe\\{sock_path}"
        invalid = ctypes.c_void_p(-1).value
        handle = invalid
        for _ in range(50):
            handle = self._k32.CreateFileW(pipe_name, 0xC0000000, 0, None, 3, 0, None)
            if handle not in (None, 0, invalid):
                break
            err = ctypes.get_last_error()
            if err == 231:  # ERROR_PIPE_BUSY
                time.sleep(0.02)
                continue
            raise OSError(err, f"CreateFileW({pipe_name}) failed (winerr={err})")
        if handle in (None, 0, invalid):
            err = ctypes.get_last_error()
            raise OSError(err, f"CreateFileW({pipe_name}) busy/failed (winerr={err})")
        self._h = handle
        self._ctypes = ctypes
        self._wintypes = wintypes

    def sendall(self, data: bytes) -> None:
        mv = memoryview(data)
        sent = 0
        while sent < len(mv):
            chunk = bytes(mv[sent:])
            written = self._wintypes.DWORD(0)
            ok = self._k32.WriteFile(self._h, chunk, len(chunk), self._ctypes.byref(written), None)
            if not ok or written.value == 0:
                err = self._ctypes.get_last_error()
                raise OSError(err, f"WriteFile failed (winerr={err})")
            sent += written.value

    def recv(self, n: int) -> bytes:
        buf = self._ctypes.create_string_buffer(n)
        got = self._wintypes.DWORD(0)
        ok = self._k32.ReadFile(self._h, buf, n, self._ctypes.byref(got), None)
        if not ok:
            err = self._ctypes.get_last_error()
            if err == 109:  # ERROR_BROKEN_PIPE
                return b""
            raise OSError(err, f"ReadFile failed (winerr={err})")
        return buf.raw[: got.value]

    def close(self) -> None:
        if getattr(self, "_h", None):
            self._k32.CloseHandle(self._h)
            self._h = None


class KiCadIpc:
    def __init__(self, sock_path: str, timeout: float = 8.0) -> None:
        self.sock_path = sock_path
        if os.name == "nt":
            self.sock = _WinPipeStream(sock_path)
        else:
            s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            s.settimeout(timeout)
            s.connect(sock_path)
            self.sock = s
        self.sock.sendall(NNG_HELLO_REQ)
        hello = self._recv_exact(8)
        if hello != NNG_HELLO_REP:
            raise RuntimeError(f"unexpected NNG hello: {hello!r}")
        self.seq = 1
        self.token = ""

    def _recv_exact(self, n: int) -> bytes:
        chunks = bytearray()
        while len(chunks) < n:
            part = self.sock.recv(n - len(chunks))
            if not part:
                raise EOFError("socket closed during NNG read")
            chunks.extend(part)
        return bytes(chunks)

    def close(self) -> None:
        try:
            self.sock.close()
        except Exception:
            pass

    def call(self, type_name: str, payload: bytes = b"", retries: int = 12) -> tuple[str, bytes]:
        for attempt in range(retries):
            req_id = 0x80000000 | self.seq
            self.seq += 1
            hdr = pb_field_str(1, self.token) + pb_field_str(2, os.environ.get("BP_IPC_CLIENT", "bp-embed-e2e"))
            any_msg = pb_field_str(1, f"type.googleapis.com/{type_name}") + pb_field_bytes(2, payload)
            api_req = pb_field_msg(1, hdr) + pb_field_msg(2, any_msg)

            frame = b"\x01" + struct.pack(">QI", 4 + len(api_req), req_id) + api_req
            self.sock.sendall(frame)

            f_hdr = self._recv_exact(9)
            if f_hdr[0] != 1:
                raise RuntimeError(f"unexpected NNG frame type {f_hdr[0]}")
            body_len = struct.unpack(">Q", f_hdr[1:9])[0]
            body = self._recv_exact(body_len)
            got_id = struct.unpack(">I", body[:4])[0]
            if got_id != req_id:
                raise RuntimeError(f"NNG req_id mismatch: {got_id:#x} != {req_id:#x}")

            resp = pb_decode_fields(body[4:])
            if 1 in resp:
                rh = pb_decode_fields(resp[1][0])
                if 1 in rh:
                    self.token = rh[1][0].decode("utf-8", errors="replace")

            st_code = 0
            st_err = ""
            if 2 in resp:
                rs = pb_decode_fields(resp[2][0])
                st_code = rs.get(1, [0])[0]
                if 2 in rs:
                    st_err = rs[2][0].decode("utf-8", errors="replace")

            if st_code in (6, 7) and attempt + 1 < retries:
                # AS_TOKEN_MISMATCH (refresh token) or AS_BUSY: retry smoothly
                time.sleep(0.18)
                continue
            if st_code != 1:
                st_name = STATUS_NAMES[st_code] if 0 <= st_code < len(STATUS_NAMES) else f"AS_{st_code}"
                raise RuntimeError(f"{type_name} -> {st_name}: {st_err}")

            if 3 not in resp:
                return "", b""
            any_out = pb_decode_fields(resp[3][0])
            out_url = any_out.get(1, [b""])[0].decode("utf-8", errors="replace")
            out_val = any_out.get(2, [b""])[0]
            return out_url, out_val
        raise RuntimeError(f"{type_name} exhausted retries")

    def first_open_doc_bytes(self, kind: str) -> bytes:
        dt = DOCTYPE_MAP[kind]
        _, val = self.call("kiapi.common.commands.GetOpenDocuments", pb_field_varint(1, dt))
        fields = pb_decode_fields(val)
        docs = fields.get(1, [])
        if not docs:
            raise RuntimeError(f"no open document for kind={kind}")
        return docs[0]


def decode_doc_spec(raw: bytes) -> dict:
    f = pb_decode_fields(raw)
    dtype = f.get(1, [0])[0]
    board_fn = f.get(4, [b""])[0].decode("utf-8", errors="replace") if 4 in f else ""
    sheet_hr = ""
    if 3 in f:
        sp = pb_decode_fields(f[3][0])
        if 2 in sp:
            sheet_hr = sp[2][0].decode("utf-8", errors="replace")
    proj_name = ""
    proj_path = ""
    if 5 in f:
        pr = pb_decode_fields(f[5][0])
        proj_name = pr.get(1, [b""])[0].decode("utf-8", errors="replace")
        proj_path = pr.get(2, [b""])[0].decode("utf-8", errors="replace")
    return {"type": dtype, "board_filename": board_fn, "sheet_path": sheet_hr, "project_name": proj_name, "project_path": proj_path}


EMBED_KIND_MAP = {"pcb": 1, "board": 1, "sch": 2, "schematic": 2, "sym": 3, "fp": 4, "3d": 5, "gerbview": 7}


def _decode_frame(raw: bytes) -> dict:
    """embed.FrameInfo: kind, document, title, window, shown, focused, realized, lib_id (bp.13), files (bp.13)."""
    f = pb_decode_fields(raw)
    s = lambda n: f.get(n, [b""])[0].decode("utf-8", "replace")
    v = lambda n: int(f.get(n, [0])[0])
    return {"kind": v(1), "document": s(2), "title": s(3), "window": v(5), "shown": bool(v(8)), "focused": bool(v(9)),
            "realized": bool(v(11)), "lib_id": s(12), "files": [x.decode("utf-8", "replace") for x in f.get(13, [])]}


def _decode_double(raw: bytes) -> float:
    return struct.unpack("<d", raw[:8])[0] if len(raw) >= 8 else 0.0


def _decode_box_mm(raw: bytes) -> dict:
    f = pb_decode_fields(raw)
    return {
        "x_min": _decode_double(f.get(1, [b""])[0]),
        "y_min": _decode_double(f.get(2, [b""])[0]),
        "x_max": _decode_double(f.get(3, [b""])[0]),
        "y_max": _decode_double(f.get(4, [b""])[0]),
    }


def _decode_item_names(raw: bytes) -> dict:
    f = pb_decode_fields(raw)
    s = lambda tag: [b.decode("utf-8", errors="replace") for b in f.get(tag, [])]
    return {"references": s(1), "nets": s(2), "pins": s(3)}


def _decode_comment(raw: bytes) -> dict:
    f = pb_decode_fields(raw)
    items = []
    for iraw in f.get(3, []):
        it = pb_decode_fields(iraw)
        items.append(
            {
                "kind": it.get(1, [b""])[0].decode("utf-8", errors="replace"),
                "id": it.get(2, [b""])[0].decode("utf-8", errors="replace"),
                "reference": it.get(3, [b""])[0].decode("utf-8", errors="replace"),
                "pin": it.get(4, [b""])[0].decode("utf-8", errors="replace"),
                "net": it.get(5, [b""])[0].decode("utf-8", errors="replace"),
                "layer": it.get(6, [b""])[0].decode("utf-8", errors="replace"),
                "sheet_path": it.get(7, [b""])[0].decode("utf-8", errors="replace"),
                "x_mm": _decode_double(it.get(8, [b""])[0]),
                "y_mm": _decode_double(it.get(9, [b""])[0]),
                "bbox": _decode_box_mm(it.get(10, [b""])[0]) if 10 in it else None,
                "value": it.get(11, [b""])[0].decode("utf-8", errors="replace"),
            }
        )
    return {
        "note": f.get(1, [b""])[0].decode("utf-8", errors="replace"),
        "names": _decode_item_names(f.get(2, [b""])[0]) if 2 in f else {"references": [], "nets": [], "pins": []},
        "items": items,
        "truncated": bool(f.get(4, [0])[0]),
        "bbox": _decode_box_mm(f.get(5, [b""])[0]) if 5 in f else None,
        "empty_selection": bool(f.get(6, [0])[0]),
        "sheet_path": f.get(7, [b""])[0].decode("utf-8", errors="replace"),
        "screenshot": f.get(8, [b""])[0].decode("utf-8", errors="replace"),
        "screenshot_region": _decode_box_mm(f.get(9, [b""])[0]) if 9 in f else None,
        "screenshot_width": int(f.get(10, [0])[0]),
        "screenshot_height": int(f.get(11, [0])[0]),
        "from_api": bool(f.get(12, [0])[0]),
    }


def mm_to_nm(mm: float) -> int:
    return int(round(mm * 1_000_000))


def encode_vec2_mm(x_mm: float, y_mm: float) -> bytes:
    return pb_field_varint(1, mm_to_nm(x_mm)) + pb_field_varint(2, mm_to_nm(y_mm))


def encode_distance_mm(d_mm: float) -> bytes:
    return pb_field_varint(1, mm_to_nm(d_mm))


def main() -> None:
    if len(sys.argv) < 3:
        sys.exit("usage: gui_ipc_probe.py <cmd> <sock_path> [args...]")
    cmd = sys.argv[1]
    sock_path = sys.argv[2]
    args = sys.argv[3:]

    cli = KiCadIpc(sock_path)
    try:
        if cmd == "ping":
            cli.call("kiapi.common.commands.Ping")
            print(json.dumps({"ok": True, "token": cli.token}))
            return

        if cmd == "caps":
            # kiapi.backplane.GetCapabilities on this KiCad (the embedded GUI's own, not the headless manifest): every string in it
            _, val = cli.call("kiapi.backplane.GetCapabilities")
            out = []
            def strings(buf: bytes, depth: int) -> None:
                if depth > 4:
                    return
                try:
                    fields = pb_decode_fields(buf)
                except Exception:
                    return
                for vals in fields.values():
                    for v in vals:
                        if isinstance(v, (bytes, bytearray)):
                            try:
                                t = v.decode("utf-8")
                                if t.isprintable():
                                    out.append(t)
                                    continue
                            except UnicodeDecodeError:
                                pass
                            strings(v, depth + 1)
            strings(val, 0)
            print(json.dumps({"names": out}))
            return

        if cmd == "version":
            _, val = cli.call("kiapi.common.commands.GetVersion")
            outer = pb_decode_fields(val)
            ver = pb_decode_fields(outer.get(1, [b""])[0])
            print(
                json.dumps(
                    {
                        "major": ver.get(1, [0])[0],
                        "minor": ver.get(2, [0])[0],
                        "patch": ver.get(3, [0])[0],
                        "full_version": ver.get(4, [b""])[0].decode("utf-8", errors="replace"),
                        "token": cli.token,
                    }
                )
            )
            return

        if cmd == "open-docs":
            kind = args[0]
            dt = DOCTYPE_MAP[kind]
            _, val = cli.call("kiapi.common.commands.GetOpenDocuments", pb_field_varint(1, dt))
            docs = [decode_doc_spec(raw) for raw in pb_decode_fields(val).get(1, [])]
            print(json.dumps({"count": len(docs), "documents": docs}))
            return

        if cmd == "modified":
            doc_bytes = cli.first_open_doc_bytes(args[0])
            try:
                _, val = cli.call("kiapi.common.commands.GetDocumentModifiedState", pb_field_msg(1, doc_bytes))
                st = pb_decode_fields(val).get(1, [0])[0]
                print(json.dumps({"state": st, "modified": st == 2, "unhandled": False}))
            except RuntimeError as exc:
                if "AS_UNHANDLED" in str(exc):
                    print(json.dumps({"state": -1, "modified": None, "unhandled": True}))
                else:
                    raise
            return

        if cmd == "save-doc":
            doc_bytes = cli.first_open_doc_bytes(args[0])
            cli.call("kiapi.common.commands.SaveDocument", pb_field_msg(1, doc_bytes))
            print(json.dumps({"ok": True}))
            return

        if cmd == "run-action":
            action = args[0]
            _, val = cli.call("kiapi.common.commands.RunAction", pb_field_str(1, action))
            st = pb_decode_fields(val).get(1, [0])[0]
            print(json.dumps({"action": action, "status": st, "ok": st == 1}))
            sys.exit(0 if st == 1 else 1)

        if cmd == "selection":
            # GetSelection on the open PCB or schematic: the selected items' types, and every reference-like string in them (a footprint's
            # or symbol's reference text), read without the schema by walking the nested messages
            doc_bytes = cli.first_open_doc_bytes(args[0])
            _, val = cli.call("kiapi.common.commands.GetSelection", pb_field_msg(1, pb_field_msg(1, doc_bytes)))
            import re
            refs, types = set(), []
            def walk(buf: bytes, depth: int) -> None:
                if depth > 12:
                    return
                try:
                    fields = pb_decode_fields(buf)
                except Exception:
                    return
                for vals in fields.values():
                    for v in vals:
                        if not isinstance(v, (bytes, bytearray)):
                            continue
                        try:
                            t = v.decode("utf-8")
                            if re.fullmatch(r"[A-Z]{1,6}[0-9]{1,6}[A-Z]?", t):
                                refs.add(t)
                        except UnicodeDecodeError:
                            pass
                        walk(v, depth + 1)
            for item in pb_decode_fields(val).get(1, []):
                anyf = pb_decode_fields(item)
                types.append(anyf.get(1, [b""])[0].decode("utf-8", errors="replace"))
                walk(anyf.get(2, [b""])[0], 0)
            print(json.dumps({"count": len(types), "types": types, "refs": sorted(refs)}))
            return

        if cmd == "items-count":
            kind = args[0]
            kot = int(args[1])
            doc_bytes = cli.first_open_doc_bytes(kind)
            item_hdr = pb_field_msg(1, doc_bytes)
            # Proto3 repeated enum defaults to packed encoding (wire type 2), though unpacked varints are also accepted
            req = pb_field_msg(1, item_hdr) + pb_field_bytes(2, pb_varint(kot))
            _, val = cli.call("kiapi.common.commands.GetItems", req)
            f = pb_decode_fields(val)
            irs = f.get(2, [0])[0]
            items = f.get(3, [])
            print(json.dumps({"irs": irs, "kot": kot, "count": len(items)}))
            sys.exit(0 if irs == 1 else 1)

        if cmd == "undo-agent-latest":
            # bp.11 co-edit: the newest change an agent made (GetChanges, actor non-empty, something created), taken back by
            # UndoAgentChange: how the person undoes an agent's edit (their own Ctrl+Z skips it: per-actor undo)
            doc_bytes = cli.first_open_doc_bytes("pcb")
            _, val = cli.call("kiapi.backplane.coedit.GetChanges", pb_field_varint(1, 0) + pb_field_str(2, "pcb"))
            best = None
            for raw in pb_decode_fields(val).get(1, []):
                ch = pb_decode_fields(raw)
                seq = ch.get(1, [0])[0]
                actor = ch.get(3, [b""])[0]
                actor = actor.decode() if isinstance(actor, bytes) else str(actor)
                if actor and ch.get(5) and not ch.get(9, [0])[0] and (best is None or seq > best[0]):
                    best = (seq, actor)
            if best is None:
                print(json.dumps({"ok": False, "error": "no agent change"})); sys.exit(1)
            _, uval = cli.call("kiapi.backplane.coedit.UndoAgentChange", pb_field_msg(1, doc_bytes) + pb_field_varint(2, best[0]))
            u = pb_decode_fields(uval)
            err = u.get(2, [b""])[0]
            print(json.dumps({"ok": bool(u.get(1, [0])[0]), "seq": best[0], "actor": best[1], "error": err.decode() if isinstance(err, bytes) else str(err)}))
            return

        if cmd == "agent-add-track":
            x1, y1, x2, y2, w_mm = map(float, args[:5])
            layer = int(args[5]) if len(args) > 5 else 3  # 3 = BL_F_Cu
            doc_bytes = cli.first_open_doc_bytes("pcb")
            item_hdr = pb_field_msg(1, doc_bytes)

            _, bc_val = cli.call("kiapi.common.commands.BeginCommit", pb_field_msg(1, item_hdr))
            kiid_bytes = pb_decode_fields(bc_val).get(1, [b""])[0]

            track_msg = (
                pb_field_msg(2, encode_vec2_mm(x1, y1))
                + pb_field_msg(3, encode_vec2_mm(x2, y2))
                + pb_field_msg(4, encode_distance_mm(w_mm))
                + pb_field_varint(6, layer)
            )
            any_track = pb_field_str(1, "type.googleapis.com/kiapi.board.types.Track") + pb_field_bytes(2, track_msg)
            ci_req = pb_field_msg(1, item_hdr) + pb_field_msg(2, any_track)
            _, ci_val = cli.call("kiapi.common.commands.CreateItems", ci_req)
            ci_f = pb_decode_fields(ci_val)
            irs = ci_f.get(2, [0])[0]
            created = ci_f.get(3, [])

            ec_req = (
                pb_field_msg(1, kiid_bytes)
                + pb_field_varint(2, 1)  # CMA_COMMIT = 1
                + pb_field_str(3, "agent add track")
                + pb_field_msg(4, item_hdr)
            )
            cli.call("kiapi.common.commands.EndCommit", ec_req)
            print(json.dumps({"ok": irs == 1 and len(created) == 1, "irs": irs, "created": len(created)}))
            sys.exit(0 if (irs == 1 and len(created) == 1) else 1)

        if cmd == "agent-add-wire":
            x1, y1, x2, y2 = map(float, args[:4])
            doc_bytes = cli.first_open_doc_bytes("sch")
            item_hdr = pb_field_msg(1, doc_bytes)

            _, bc_val = cli.call("kiapi.common.commands.BeginCommit", pb_field_msg(1, item_hdr))
            kiid_bytes = pb_decode_fields(bc_val).get(1, [b""])[0]

            wire_msg = (
                pb_field_msg(2, encode_vec2_mm(x1, y1))
                + pb_field_msg(3, encode_vec2_mm(x2, y2))
                + pb_field_varint(4, 1)  # SLT_WIRE = 1
            )
            any_wire = pb_field_str(1, "type.googleapis.com/kiapi.schematic.types.SchematicLine") + pb_field_bytes(2, wire_msg)
            ci_req = pb_field_msg(1, item_hdr) + pb_field_msg(2, any_wire)
            _, ci_val = cli.call("kiapi.common.commands.CreateItems", ci_req)
            ci_f = pb_decode_fields(ci_val)
            irs = ci_f.get(2, [0])[0]
            created = ci_f.get(3, [])

            ec_req = (
                pb_field_msg(1, kiid_bytes)
                + pb_field_varint(2, 1)
                + pb_field_str(3, "agent add schematic wire")
                + pb_field_msg(4, item_hdr)
            )
            cli.call("kiapi.common.commands.EndCommit", ec_req)
            print(json.dumps({"ok": irs == 1 and len(created) == 1, "irs": irs, "created": len(created)}))
            sys.exit(0 if (irs == 1 and len(created) == 1) else 1)

        if cmd == "select-items":
            fk = EMBED_KIND_MAP[args[0]]
            refs = [r.strip() for r in (args[1].split(",") if len(args) > 1 and args[1] else []) if r.strip()]
            zoom = int(args[2]) != 0 if len(args) > 2 else True
            req = pb_field_varint(1, fk)
            for r in refs:
                req += pb_field_str(2, r)
            if zoom:
                req += pb_field_varint(5, 1)
            _, val = cli.call("kiapi.backplane.embed.SelectItems", req)
            f = pb_decode_fields(val)
            found = _decode_item_names(f.get(1, [b""])[0]) if 1 in f else {"references": [], "nets": [], "pins": []}
            not_found = _decode_item_names(f.get(2, [b""])[0]) if 2 in f else {"references": [], "nets": [], "pins": []}
            print(json.dumps({"ok": True, "found": found, "not_found": not_found}))
            return

        if cmd == "sym-value":  # the Value of the schematic symbol whose reference is REF (GetItems KOT_SCH_SYMBOL, decoded by hand)
            doc = cli.first_open_doc_bytes("sch")
            _, val = cli.call("kiapi.common.commands.GetItems", pb_field_msg(1, pb_field_msg(1, doc)) + pb_field_varint(2, 35))
            got = None
            for anyb in pb_decode_fields(val).get(3, []):
                sym = pb_decode_fields(pb_decode_fields(anyb).get(2, [b""])[0])
                txt = lambda n: pb_decode_fields(pb_decode_fields(sym.get(n, [b""])[0]).get(3, [b""])[0]).get(5, [b""])[0].decode("utf-8", "replace")
                if txt(7) == args[0]: got = txt(8); break
            print(json.dumps({"ok": got is not None, "ref": args[0], "value": got}))
            return

        if cmd == "capabilities":  # GetCapabilities: capability names and every registered command (the embedded GUI's own)
            _, val = cli.call("kiapi.backplane.GetCapabilities", b"")
            f = pb_decode_fields(val); dec = lambda n: [x.decode("utf-8", "replace") for x in f.get(n, [])]
            print(json.dumps({"ok": True, "capabilities": dec(5), "commands": dec(6)}))
            return

        if cmd == "frames":  # GetFrames: every embedded frame
            _, val = cli.call("kiapi.backplane.embed.GetFrames", b"")
            print(json.dumps({"ok": True, "frames": [_decode_frame(x) for x in pb_decode_fields(val).get(1, [])]}))
            return

        if cmd == "events":  # NextEvents since SEQ (no wait): [{seq, type, frame}] and next; reading does not take them from the host
            _, val = cli.call("kiapi.backplane.embed.NextEvents", pb_field_varint(1, int(args[0]) if args else 0))
            f = pb_decode_fields(val); evs = []
            for raw in f.get(1, []):
                e = pb_decode_fields(raw)
                ev_checks = {}
                if 9 in e:
                    c = pb_decode_fields(e[9][0]); ci = lambda n: int(c.get(n, [0])[0])
                    ev_checks = {"errors": ci(2), "warnings": ci(3), "exclusions": ci(4), "unconnected": ci(5), "cancelled": bool(ci(6)),
                                 "progress_id": ci(7), "error": c.get(9, [b""])[0].decode("utf-8", "replace")}
                ev_marker = {}
                if 8 in e:
                    mr = pb_decode_fields(e[8][0]); ev_marker = {"set": mr.get(1, [b""])[0].decode(), "id": mr.get(2, [b""])[0].decode()}
                evs.append({"seq": int(e.get(1, [0])[0]), "type": int(e.get(2, [0])[0]), "checks": ev_checks, "marker": ev_marker,
                            "drc_marker": 10 in e,
                            "frame": _decode_frame(e[3][0]) if 3 in e else {}, "detail": e.get(4, [b""])[0].decode("utf-8", "replace")})
            print(json.dumps({"ok": True, "events": evs, "next": int(f.get(2, [0])[0])}))
            return

        if cmd == "run-checks":  # RunChecks KIND (pcb: DRC, sch: ERC): its progress_id
            _, val = cli.call("kiapi.backplane.embed.RunChecks", pb_field_varint(1, EMBED_KIND_MAP[args[0]]))
            print(json.dumps({"ok": True, "progress_id": int(pb_decode_fields(val).get(1, [0])[0])}))
            return

        if cmd == "get-markers":  # GetMarkers KIND [host]: counts by severity and group, and the first few
            req = pb_field_varint(1, EMBED_KIND_MAP[args[0]]) + (pb_field_varint(2, 1) if len(args) > 1 and args[1] == "host" else b"")
            _, val = cli.call("kiapi.backplane.embed.GetMarkers", req)
            ms = []
            for raw in pb_decode_fields(val).get(1, []):
                f = pb_decode_fields(raw); g = lambda n: f.get(n, [b""])[0].decode("utf-8", "replace")
                ms.append({"type": g(1), "severity": g(2), "group": g(13), "host": bool(f.get(8))})
            cnt = {}
            for m in ms:
                k = "host" if m["host"] else f"{m['severity']}/{m['group'] or '-'}"; cnt[k] = cnt.get(k, 0) + 1
            print(json.dumps({"ok": True, "n": len(ms), "counts": cnt}))
            return

        if cmd == "set-host-markers":  # SetHostMarkers KIND SET JSON: [{"id", "title", "severity": 0-2, "x_mm", "y_mm"}] ([] clears)
            req = pb_field_varint(1, EMBED_KIND_MAP[args[0]]) + pb_field_str(2, args[1])
            for m in json.loads(args[2]):
                at = pb_field_varint(1, int(round(m["x_mm"] * 1e6))) + pb_field_varint(2, int(round(m["y_mm"] * 1e6)))
                req += pb_field_msg(3, pb_field_str(1, m["id"]) + pb_field_str(2, m.get("title", m["id"]))
                                    + (pb_field_varint(3, m.get("severity", 0)) if m.get("severity") else b"") + pb_field_msg(4, at))
            _, val = cli.call("kiapi.backplane.embed.SetHostMarkers", req)
            print(json.dumps({"ok": True, "count": int(pb_decode_fields(val).get(1, [0])[0])}))
            return

        if cmd == "open-gerbers":  # OpenGerbers FILES(comma) [X Y W H]: shown at that holder rect when given, else hidden (a prewarm)
            req = b"".join(pb_field_str(1, x) for x in args[0].split(",") if x)
            if len(args) >= 5:
                req += pb_field_varint(2, 1) + b"".join(pb_field_varint(n, int(a)) for n, a in zip((3, 4, 5, 6), args[1:5]))
            _, val = cli.call("kiapi.backplane.embed.OpenGerbers", req)
            f = pb_decode_fields(val); dec = lambda n: [x.decode("utf-8", "replace") for x in f.get(n, [])]
            print(json.dumps({"ok": True, "frame": _decode_frame(f[1][0]) if 1 in f else {}, "loaded": dec(2), "failed": dec(3), "errors": dec(4)}))
            return

        if cmd == "show-frame":  # ShowFrame KIND [ITEM] (as the host would; no geometry: the frame's current one)
            req = pb_field_varint(1, EMBED_KIND_MAP[args[0]]) + (pb_field_str(3, args[1]) if len(args) > 1 and args[1] else b"")
            _, val = cli.call("kiapi.backplane.embed.ShowFrame", req)
            f = pb_decode_fields(val)
            print(json.dumps({"ok": True, "frame": _decode_frame(f[1][0]) if 1 in f else {}}))
            return

        if cmd == "hide-frame":  # HideFrame KIND
            cli.call("kiapi.backplane.embed.HideFrame", pb_field_varint(1, EMBED_KIND_MAP[args[0]]))
            print(json.dumps({"ok": True}))
            return

        if cmd == "comment-selection":
            fk = EMBED_KIND_MAP[args[0]]
            note = args[1] if len(args) > 1 else ""
            req = pb_field_varint(1, fk) + pb_field_str(2, note)
            _, val = cli.call("kiapi.backplane.embed.CommentSelection", req)
            f = pb_decode_fields(val)
            seq = int(f.get(1, [0])[0])
            comment = _decode_comment(f.get(2, [b""])[0]) if 2 in f else {}
            print(json.dumps({"ok": True, "seq": seq, "comment": comment}))
            return

        sys.exit(f"unknown command: {cmd}")
    finally:
        cli.close()


if __name__ == "__main__":
    main()
