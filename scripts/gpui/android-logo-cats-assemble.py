#!/usr/bin/env python3
"""Side-by-side web|android images and the check manifest for one scheme.
usage: android-logo-cats-assemble.py SCHEME ANDROID_BOXES.json   (boxes measured from the emulator's UI dump)
Writes build/gpui/shots/logo-cats/android-<scheme>-<n>-<name>.png (web left, android right), ref/ crops of the web's
logos and cats, and merges items into test/gpui/visual/android-logo-cats.manifest.json (form A of check_logo_cats.py)."""
import json, os, sys
from PIL import Image, ImageDraw
root = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
S = root + "/build/gpui/shots/logo-cats"; raw = S + "/raw"; web = S + "/web-raw"; ref = S + "/ref"; os.makedirs(ref, exist_ok=True)
scheme, boxes_file = sys.argv[1], sys.argv[2]
A = json.load(open(boxes_file)); W = json.load(open(f"{web}/web-{scheme}-boxes.json"))
names = {"1-threads": "threads", "2-bots": "bots", "3-bot": "bot"}
for key, scr in (("1-threads", "threads"), ("2-bots", "bots"), ("3-bot", "bot"), ("4-mention", ""), ("5-room", ""), ("6-looks", "")):
    a = Image.open(f"{raw}/android-{scheme}-{key}.png").convert("RGB"); w = Image.open(f"{web}/web-{scheme}-{key}.png").convert("RGB")
    h = max(a.height, w.height); out = Image.new("RGB", (w.width + a.width + 24, h + 44), (128, 128, 128))
    out.paste(w, (0, 44)); out.paste(a, (w.width + 24, 44)); d = ImageDraw.Draw(out)
    d.text((10, 14), f"WEB ({scheme}) {w.size}", fill=(255, 255, 255)); d.text((w.width + 34, 14), f"ANDROID ({scheme}) {a.size}", fill=(255, 255, 255))
    out.save(f"{S}/android-{scheme}-{key}.png")
mf = root + "/test/gpui/visual/android-logo-cats.manifest.json"
m = json.load(open(mf)) if os.path.exists(mf) else {"items": []}
m["items"] = [i for i in m["items"] if i["theme"] != scheme]
rel = lambda p: os.path.relpath(p, os.path.dirname(mf))
def crop(img, box, path):
    x, y, w, h = box; Image.open(img).convert("RGB").crop((x, y, x + w, y + h)).save(path); return path
# logo (threads screen)
wl = [b for b in W["threads"] if b["kind"] == "logo"][0]["rect"]
r = crop(f"{web}/web-{scheme}-1-threads.png", wl, f"{ref}/web-{scheme}-logo.png")
m["items"].append({"id": f"logo-{scheme}", "kind": "logo", "theme": scheme, "image": rel(f"{raw}/android-{scheme}-1-threads.png"), "box": A["logo"], "reference": rel(r)})
# cats: bot rows and the bot page header
moods = {"Bub": "idle", "Mochi": "work", "Pixel": "error", "Ember": "idle", "Nori": "idle", "Zed": "idle"}
for b in [b for b in W["bots"] if b["kind"] == "cat" and b["where"] == "row"]:
    n = b["name"]
    if n not in A["rows"]: continue
    r = crop(f"{web}/web-{scheme}-2-bots.png", b["rect"], f"{ref}/web-{scheme}-cat-row-{n}.png")
    m["items"].append({"id": f"cat-row-{n}-{scheme}", "kind": "cat", "theme": scheme, "image": rel(f"{raw}/android-{scheme}-2-bots.png"), "box": A["rows"][n], "reference": rel(r), "mood": moods.get(n, "idle")})
hb = [b for b in W["bot"] if b["where"] == "header"][0]
r = crop(f"{web}/web-{scheme}-3-bot.png", hb["rect"], f"{ref}/web-{scheme}-cat-header.png")
m["items"].append({"id": f"cat-header-Mochi-{scheme}", "kind": "cat", "theme": scheme, "image": rel(f"{raw}/android-{scheme}-3-bot.png"), "box": A["header"], "reference": rel(r), "mood": "work"})
# room posts: the web draws a bot's post with look 0 "away" (the post's author is the bot's name, not its id): the same here
for b in [b for b in W.get("room", []) if b["where"] == "post"]:
    n = b["name"]
    if n not in A.get("posts", {}): continue
    r = crop(f"{web}/web-{scheme}-5-room.png", b["rect"], f"{ref}/web-{scheme}-cat-post-{n}.png")
    m["items"].append({"id": f"cat-post-{n}-{scheme}", "kind": "cat", "theme": scheme, "image": rel(f"{raw}/android-{scheme}-5-room.png"), "box": A["posts"][n], "reference": rel(r), "mood": "away"})
# the look picker: the first six looks as idle cats
wl = [b for b in W.get("looks", []) if b["where"] == "look"]
for i, box in enumerate(A.get("looks", [])):
    if i >= len(wl): break
    r = crop(f"{web}/web-{scheme}-6-looks.png", wl[i]["rect"], f"{ref}/web-{scheme}-look-{i}.png")
    m["items"].append({"id": f"look-{i}-{scheme}", "kind": "cat", "theme": scheme, "image": rel(f"{raw}/android-{scheme}-6-looks.png"), "box": box, "reference": rel(r), "mood": "idle"})
json.dump(m, open(mf, "w"), indent=1); print("manifest items:", len(m["items"]))
