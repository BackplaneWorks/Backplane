import sys, os; sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from olib import *
def fld(x, k): return x.get(k, {}).get("text", {}).get("text", "")
def run(m, cwd):
    d = Doc(m, cwd + "/pic_programmer.kicad_sch")
    want = {**{"R%d" % i: ("10k", "RC0603FR-0710KL") for i in range(1, 8)}, "C6": ("100n", "CC0603KRX7R9BB104"), "C7": ("100n", "CC0603KRX7R9BB104")}
    ups = {}
    for s in d.items("KOT_SCH_SYMBOL"):
        r = fld(s, "reference_field")
        if r not in want: continue
        s.pop("definition", None); s["value_field"]["text"]["text"] = want[r][0]
        uf = [f for f in s.get("user_fields", []) if f.get("name") != "MPN"]
        uf.append({"name": "MPN", "text": {"position": s["position"], "text": want[r][1], "attributes": {"visible": False, "size": {"x_nm": "1270000", "y_nm": "1270000"}}}, "visible": False})
        s["user_fields"] = uf; ups.setdefault(json.dumps(s["path"]["path"]), []).append(s)
    d.begin()
    for path, items in ups.items():  # an item is updated on its own sheet
        r = d.ipc("UpdateItems", {"header": {"document": {**d.doc, "sheet_path": {"path": json.loads(path)}}}, "items": items}); print(len(items), [u["status"]["code"] for u in r["updated_items"]])
    d.end("values and MPNs"); print(d.save())
main(run)
