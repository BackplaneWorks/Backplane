#!/usr/bin/env python3
"""Zero-dependency KiCad 10 IPC client over NNG ipc:// REQ0 + raw Protobuf wire encoding.

Matches native/host/src/kicad/nng.rs and proto.rs without requiring pynng or google.protobuf.
"""
import json
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


# ---- NNG REQ0 over AF_UNIX -----------------------------------------------------------------------
class KiCadIpc:
    def __init__(self, sock_path: str, timeout: float = 8.0) -> None:
        self.sock_path = sock_path
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(timeout)
        self.sock.connect(sock_path)
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
            hdr = pb_field_str(1, self.token) + pb_field_str(2, "bp-embed-e2e")
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

        sys.exit(f"unknown command: {cmd}")
    finally:
        cli.close()


if __name__ == "__main__":
    main()
