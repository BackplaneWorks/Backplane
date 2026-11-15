"""What the reference edits (oracles/*.py) share: the hub's kicad_ipc through MCP, one document, one commit."""
import json, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mcp import Mcp
C = "kiapi.common.commands."; B = "kiapi.board.commands."; S = "kiapi.schematic.commands."; BT = "type.googleapis.com/kiapi.board.types."; ST = "type.googleapis.com/kiapi.schematic.types."
def nm(mm): return str(round(mm * 1e6))
def xy(x, y): return {"x_nm": nm(x), "y_nm": nm(y)}
class Doc:
    def __init__(self, m, path, kind=None):
        self.m, self.path = m, path
        kind = kind or ("DOCTYPE_PCB" if path.endswith(".kicad_pcb") else "DOCTYPE_SCHEMATIC")
        msg = {"type": kind, "path": path}
        try: r = m.ipc(path, "OpenDocument", msg)
        except RuntimeError as e:
            lock = re.search(r"lock_acquire (\S+?)[,; ]", str(e))
            if lock: m.call("lock_acquire", {"name": lock.group(1)})
            try: r = m.ipc(path, "OpenDocument", msg)
            except RuntimeError as e2:
                if "already open" not in str(e2): raise
                r = {"document": m.ipc(path, "GetOpenDocuments", {"type": kind})["documents"][0]}
        self.doc = r["document"]; self.h = {"document": self.doc}
    def ipc(self, ty, msg=None): return self.m.ipc(self.path, ty, msg)
    def items(self, *types): return self.ipc("GetItems", {"header": self.h, "types": list(types)}).get("items", [])
    def begin(self): self.cid = self.ipc("BeginCommit", {"header": self.h})["id"]
    def end(self, message="edit"): self.ipc("EndCommit", {"header": self.h, "id": self.cid, "action": "CMA_COMMIT", "message": message})
    def update(self, items): return self.ipc("UpdateItems", {"header": self.h, "items": items})
    def create(self, items): return self.ipc("CreateItems", {"header": self.h, "items": items})
    def delete(self, ids): return self.ipc("DeleteItems", {"header": self.h, "item_ids": [{"value": i} for i in ids]})
    def save(self): return self.m.call("kicad_ipc", {"path": self.path, "type": C + "SaveDocument", "message": {"document": self.doc}})
def ref(f): return f["reference_field"]["text"]["text"]["text"]
def main(fn):
    m = Mcp(sys.argv[1]); fn(m, sys.argv[2]); print("calls", m.calls, "bytes", m.bytes)
