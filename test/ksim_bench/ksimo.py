"""What the `kicad_sim` oracles (oracles_sim/*.py) share: each task as the fewest calls of the hub's kicad_sim tool, no model, held to the same
check as an agent's run (`KSIM_ORACLES=oracles_sim bench.py e2e`). What a task still needs beside kicad_sim is said in its oracle."""
import json, os, sys
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, os.path.join(HERE, "..", "kipc_bench"))
from mcp import Mcp
sys.path.insert(0, HERE)
class K:
    def __init__(self, url, cwd): self.m = Mcp(url); self.cwd = cwd; self.path = os.path.join(cwd, next(f for f in sorted(os.listdir(cwd)) if f.endswith(".kicad_sch")))
    def sim(self, *ops):
        r = self.m.call("kicad_sim", {"path": self.path, "ops": list(ops)}); print(json.dumps(r)[:1800], flush=True)
        for x in r.get("results", []):
            if "failed" in x or "error" in x: raise RuntimeError(json.dumps(x)[:600])
        return r["results"]
    def edit(self, *ops): r = self.m.call("kicad_edit", {"path": self.path, "ops": list(ops)}); print(json.dumps(r)[:900], flush=True); return r
def main(fn):
    k = K(sys.argv[1], sys.argv[2]); out = fn(k); print("calls", k.m.calls, "bytes", k.m.bytes); print("RESULT " + " ".join(f"{a}={b:.6g}" for a, b in out.items()))
