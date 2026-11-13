#!/usr/bin/env python3
"""The routing plan over KiCad's API (fork bp.13+: kiapi.backplane.plan), and the board's active layer, for the real-use harness.
  plan_probe.py get SOCK                 GetPlan: {"revision", "json"} (the plan document)
  plan_probe.py propose SOCK FILE [merge] ProposePlan of the JSON in FILE (replace-proposed unless "merge"): {"revision", "applied", "refused"}
  plan_probe.py layer SOCK [NAME]        GetActiveLayer, or SetActiveLayer to a copper layer by name (F.Cu, In1.Cu.., B.Cu)
Uses gui_ipc_probe.py's client (same folder)."""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gui_ipc_probe import KiCadIpc, pb_field_msg, pb_field_str, pb_field_varint, pb_decode_fields  # noqa: E402

def layer_enum(name):
    if name == "F.Cu": return 3
    if name == "B.Cu": return 34
    if name.startswith("In") and name.endswith(".Cu"): return 3 + int(name[2:-3])
    raise SystemExit(f"not a copper layer: {name}")
def layer_name(v):
    return {3: "F.Cu", 34: "B.Cu"}.get(v, f"In{v - 3}.Cu" if 4 <= v <= 33 else str(v))

def main():
    cmd, sock, args = sys.argv[1], sys.argv[2], sys.argv[3:]
    cli = KiCadIpc(sock)
    try:
        doc = pb_field_msg(1, cli.first_open_doc_bytes("pcb"))
        if cmd == "get":
            _, val = cli.call("kiapi.backplane.plan.GetPlan", doc)
            f = pb_decode_fields(val)
            print(json.dumps({"revision": int(f.get(2, [0])[0]), "json": f.get(1, [b""])[0].decode("utf-8", "replace")}))
        elif cmd == "propose":
            body = open(args[0]).read(); mode = 0 if (len(args) > 1 and args[1] == "merge") else 1
            _, val = cli.call("kiapi.backplane.plan.ProposePlan", doc + pb_field_str(2, body) + pb_field_varint(3, mode))
            f = pb_decode_fields(val)
            print(json.dumps({"revision": int(f.get(1, [0])[0]), "applied": [x.decode() for x in f.get(2, [])], "refused": [x.decode() for x in f.get(3, [])],
                              "reasons": f.get(4, [b""])[0].decode("utf-8", "replace")}))
        elif cmd == "layer":
            if args:
                cli.call("kiapi.board.commands.SetActiveLayer", doc + pb_field_varint(2, layer_enum(args[0])))
            _, val = cli.call("kiapi.board.commands.GetActiveLayer", doc)
            print(json.dumps({"layer": layer_name(int(pb_decode_fields(val).get(1, [0])[0]))}))
        else:
            raise SystemExit("unknown " + cmd)
    finally:
        cli.close()

if __name__ == "__main__":
    main()
