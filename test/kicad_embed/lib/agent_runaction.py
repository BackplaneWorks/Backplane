#!/usr/bin/env python3
"""One RunAction sent straight to KiCad's API as an agent's client (agent:<thread>), past the hub's own guard: what the fork
itself answers. Prints "refused: <why>" or "ran: RAS_<n>". Usage: agent_runaction.py <api.sock> <action> [client]"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
os.environ["BP_IPC_CLIENT"] = sys.argv[3] if len(sys.argv) > 3 else "agent:t1"
from gui_ipc_probe import KiCadIpc, pb_field_str, pb_decode_fields  # noqa: E402
k = KiCadIpc(sys.argv[1])
try:
    _, out = k.call("kiapi.common.commands.RunAction", pb_field_str(1, sys.argv[2]), retries=4)
except RuntimeError as e:
    print("refused:", str(e).split("->", 1)[-1].strip()); sys.exit(0)
st = pb_decode_fields(out).get(1, [0])[0]
names = {0: "RAS_UNKNOWN", 1: "RAS_OK", 2: "RAS_INVALID", 3: "RAS_FRAME_NOT_OPEN"}
print(("ran: " if st == 1 else "refused: ") + names.get(st, f"RAS_{st}"))
