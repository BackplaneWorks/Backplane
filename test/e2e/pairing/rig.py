#!/usr/bin/env python3
"""What gpui.sh asks of a hub: rig.py BASE TOKEN_FILE invite FILE | devices | uninvite ID | revoke-all NAME | wait. Prints no credential."""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from proto import Hub
hub = Hub(sys.argv[1], open(sys.argv[2]).read().strip())
cmd, arg = sys.argv[3], sys.argv[4] if len(sys.argv) > 4 else ""
if cmd == "invite":
    st, i = hub.invite()
    with open(os.open(arg, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600), "w") as f:
        json.dump({"link": i["link"], "code": i["code"], "id": i["id"]}, f)
elif cmd == "devices":
    for d in hub.devices()[1]["devices"]:
        print(d.get("name"), "revoked" if d.get("revoked") else "live", d["id"][:8])
elif cmd == "uninvite":
    hub.revoke_invite(arg)
elif cmd == "revoke-all":
    for d in hub.devices()[1]["devices"]:
        if d.get("name") == arg and not d.get("revoked"):
            hub.revoke_device(d["id"])
elif cmd == "wait":
    sys.exit(0 if hub.wait_up() else 1)
