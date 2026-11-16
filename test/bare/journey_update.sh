# Sourced by journey_project.sh in the gate's update mode (UPDATE_FROM=<an older Backplane KiCad version>, bare-gate.sh --update-from): a
# person's home that already has that runtime (placed by journey.sh before the first launch, where the hub's installer puts one) meets the
# build's pin at the first Edit. Backplane 0.16.0's rule: the update is offered where the person works, a row of its own above the pane
# (screens/kicad_update.rs), never a silent 1 GB download; one press installs it and the pane reopens on it.
#   U1 Edit opens on the old runtime (KiCad in the pane runs from <home>/kicad/<UPDATE_FROM>)
#   U2 the hub offers the update (info kicad.update: from, to, reason, size) and the row shows it with Update and Not now
#   U3 the row and its buttons are outside the pane's rect (on macOS KiCad's frame sits above Backplane's window: an overlay would hide)
#   U4 Update installs the pin's runtime beside the old one (the old one stays)
#   U5 the pane reopens on the new runtime by itself (no unsaved edits), and the row is gone
phase "update at Edit ($UPDATE_FROM)"
WP=$(cat "$HH/.client.pid" 2>/dev/null); [ -n "$WP" ] && in_box kill "$WP" 2>/dev/null; sleep 2
cat >"$HH/update.json" <<J
[{"wait":3000},{"click":"project:$pid_"},{"wait":2000},{"click":"page-thread:$tid"},{"wait":2500},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":120000,"kind":"PCB","tag":"old"}},{"kicad":{"report":true,"tag":"old"}},{"mark":"old-ready"},{"wait":2400000},{"mark":"uend"}]
J
: >"$HH/win.log"
launch --url http://127.0.0.1:$(hub_port) --token-file $PH/.backplane/hub/token --script $PH/update.json
umark() { for _ in $(seq ${2:-600}); do grep -q "\"name\":\"$1\"" "$HH/win.log" 2>/dev/null && return 0; sleep 0.25; done; return 1; }
# the latest drawn rects of the row, its buttons and the pane (window coordinates; the client's probes, as embed_update_row_e2e.sh reads them)
urects() { python3 - "$HH/win.log" <<'PY'
import json, sys
last = {}
for l in open(sys.argv[1], errors="ignore"):
    if "GPUI_METRIC " in l and '"rects":[' in l:
        try: rs = json.loads(l.split("GPUI_METRIC ", 1)[1])["rects"]
        except Exception: continue
        if isinstance(rs, list) and any(r.get("id", "").startswith(("kicad:pane", "kicad-update")) for r in rs):
            for r in rs: last[r["id"]] = r
            last["_ids"] = [r["id"] for r in rs]
print(json.dumps(last))
PY
}
krun() { in_box sh -c "pgrep -af 'kicad.real|bin/kicad ' 2>/dev/null" | grep -v pgrep | grep -- '--embedded' | head -1; }
# the version this build pins for Linux (what the update must bring)
NEWV=$(grep -o 'pub const LINUX: Package = Package { platform: "linux-x86_64", version: "[^"]*"' "$ROOT/native/host/src/kicad/install.rs" | sed 's/.*version: "//; s/"$//')
if ! umark old-ready 720; then bad "update: Edit never showed KiCad on the old runtime ($UPDATE_FROM)"; cut_ 1 update "a home on $UPDATE_FROM: Edit did not open"
else
  sleep 2; shot 30-update-offer
  K=$(krun); echo "kicad at Edit: $K" >"$OUT/logs/update-kicad-old.txt"
  case $K in *"/kicad/$UPDATE_FROM/"*) ok "U1 Edit opens on the home's runtime ($UPDATE_FROM)";; *) bad "U1 Edit did not run the home's $UPDATE_FROM ($(echo "$K" | cut -c1-200))";; esac
  hub info kicad.update >"$OUT/logs/update-offer.json"
  OFFER=$(python3 -c 'import json,sys; v=json.load(open(sys.argv[1])).get("kicad.update") or {}; v=json.loads(v) if isinstance(v,str) and v else v
print(json.dumps(v) if isinstance(v,dict) else "")' "$OUT/logs/update-offer.json" 2>/dev/null)
  python3 -c 'import json,sys; v=json.loads(sys.argv[1] or "{}"); sys.exit(0 if v.get("offer") and v.get("from")==sys.argv[2] and v.get("to")==sys.argv[3] and v.get("reason") else 1)' "$OFFER" "$UPDATE_FROM" "$NEWV" \
    && ok "U2 the hub offers $UPDATE_FROM -> $NEWV ($(python3 -c 'import json,sys; v=json.loads(sys.argv[1]); print(v["size"], "-", v["reason"])' "$OFFER"))" \
    || bad "U2 the hub offers no update from $UPDATE_FROM to $NEWV (info kicad.update: ${OFFER:-none})"
  # the row: drawn within a minute of Edit, with Update and Not now, and none of it inside the pane's rect
  for _ in $(seq 60); do urects | grep -q '"kicad-update:go"' && break; sleep 1; done
  R=$(urects); echo "$R" >"$OUT/logs/update-rects.json"
  read -r SHOWN APART <<<"$(python3 -c 'import json,sys
r=json.loads(sys.argv[1]); ids=set(r.get("_ids",[])); pane=r.get("kicad:pane"); row=r.get("kicad-update")
def hit(a,b): return a and b and a["x"] < b["x"]+b["w"] and b["x"] < a["x"]+a["w"] and a["y"] < b["y"]+b["h"] and b["y"] < a["y"]+a["h"]
shown=int(bool(row) and {"kicad-update:go","kicad-update:later"} <= ids)
apart=int(bool(row and pane) and not any(hit(r.get(k), pane) for k in ("kicad-update","kicad-update:go","kicad-update:later")))
print(shown, apart)' "$R")"
  [ "$SHOWN" = 1 ] && ok "U2 the update row shows above the pane with Update and Not now" || { bad "U2 no update row at Edit (rects: $(echo "$R" | cut -c1-200))"; cut_ 1 update "a home on $UPDATE_FROM is not offered $NEWV at Edit"; }
  [ "$APART" = 1 ] && ok "U3 the update row and its buttons are outside the pane's rect" || bad "U3 the update row overlaps the pane's rect, or the pane has none ($(echo "$R" | cut -c1-200))"
  # Update: one press
  GO=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]).get("kicad-update:go"); print("%d %d" % (r["x"] + r["w"] / 2, r["y"] + r["h"] / 2) if r else "")' "$R")
  if [ -n "$GO" ]; then t=$(date +%s); X mousemove ${GO% *} ${GO#* } click 1
    for _ in $(seq 3600); do [ -x "$KH/$NEWV/bin/kicad-cli" ] && ! ls -d "$KH"/.install-*.lock >/dev/null 2>&1 && break; sleep 0.5; done
    cp "$KH/.install.log" "$OUT/logs/update-install.log" 2>/dev/null
    if [ -x "$KH/$NEWV/bin/kicad-cli" ]; then ok "U4 Update installed $NEWV in $(( $(date +%s) - t )) s, beside $UPDATE_FROM"; tm update-install "$(( $(date +%s) - t ))"
      [ -x "$KH/$UPDATE_FROM/bin/kicad-cli" ] || bad "U4 the old runtime $UPDATE_FROM is gone after the update"
      # the pane on the new runtime, by itself: nothing was edited, so the client restarts the session on it
      for _ in $(seq 360); do case $(krun) in *"/kicad/$NEWV/"*) break;; esac; sleep 0.5; done; K=$(krun); echo "kicad after update: $K" >"$OUT/logs/update-kicad-new.txt"; shot 31-update-done
      case $K in *"/kicad/$NEWV/"*) ok "U5 the pane reopened on $NEWV by itself"; tm update-reopen "$(( $(date +%s) - t ))";;
        *) bad "U5 the pane did not reopen on $NEWV within 3 min ($(echo "$K" | cut -c1-200))"; cut_ 1 update "after Update the pane stays on $UPDATE_FROM";; esac
      sleep 3; urects | grep -q '"kicad-update"' && bad "U5 the update row is still shown after the update" || ok "U5 the update row is gone"
    else bad "U4 Update installed nothing in 30 min ($(tail -1 "$KH/.install.log" 2>/dev/null))"; cut_ 1 update "Update at Edit did not install $NEWV"; fi
  fi
fi
cp "$HH/win.log" "$OUT/logs/win-update.log" 2>/dev/null; WP=$(cat "$HH/.client.pid" 2>/dev/null); [ -n "$WP" ] && in_box kill "$WP" 2>/dev/null
