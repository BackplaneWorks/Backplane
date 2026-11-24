#!/bin/bash
# Box A of the file-links E2E (run.sh): the project, the stand-in agent and this branch's hub. Runs ON the box, in ~/fl (run.sh copies the hub,
# the stand-in and this script there). Nothing of the person's is here: a temp home, a made-up project, a stand-in `claude`.
#   box_a.sh setup HOST PORT     make the project, start the hub on HOST:PORT (--no-tailscale), print "token=<hub token>" and "proj=<folder>"
#   box_a.sh rss-start           sample the hub's RSS (kB) every 100 ms into ~/fl/rss.log, in the background
#   box_a.sh rss-stop            stop it; print "rss base=<kB> peak=<kB>"
#   box_a.sh rewrite             give mfg/big.bin new contents and mtime, the same size; print "sha=<sha256>"
#   box_a.sh stop                stop the hub (its process group)
set -eu
FL=$HOME/fl; P=$FL/proj; H=$FL/hub; cd "$FL"
case "$1" in
  setup)
    host=$2; port=$3
    rm -rf "$P" "$H" "$FL/outside" "$FL/agents" "$FL/home"; mkdir -p "$P/mfg" "$P/mech" "$P/.git" "$H" "$FL/outside" "$FL/agents" "$FL/home"
    head -c $((150 << 20)) /dev/urandom > "$P/mfg/big.bin"
    printf 'ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((\x27fl e2e\x27),\x272;1\x27);\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n' > "$P/mech/bracket.step"
    echo 'API_KEY=not-a-real-key' > "$P/.env"
    echo 'not a real key' > "$P/id_rsa"
    printf '[core]\n\trepositoryformatversion = 0\n' > "$P/.git/config"
    echo 'outside every project' > "$FL/outside/notes.txt"
    ln -s "$FL/outside/notes.txt" "$P/link-out.txt"
    echo 'a readme' > "$P/README.md"
    # a file another machine's thread sent here (the hub keeps those in <home>/far-files): shareable by its token alone
    mkdir -p "$H/far-files/fl-peer"; printf 'far part\n%.0s' $(seq 64) > "$H/far-files/fl-peer/far-part.txt"
    bun=$(command -v bun || echo "$HOME/.bun/bin/bun")
    cat > "$FL/agents/claude" <<EOF
#!/bin/sh
if [ "\$1" = auth ]; then echo '{"loggedIn":true,"email":"standin@example.com"}'; exit 0; fi
if [ "\$1" = --version ]; then echo '2.1.286 (Claude Code)'; exit 0; fi
export FAKE_LOG="$FL/standin.log" BP_STREAM_N=1 BP_STREAM_MS=20
exec $bun $FL/claude_standin.ts "\$@"
EOF
    chmod +x "$FL/agents/claude" "$FL/backplane-hub"
    env -i PATH=/usr/local/bin:/usr/bin:/bin HOME="$FL/home" BACKPLANE_AGENTS="$FL/agents" BACKPLANE_NO_UPDATE=1 BACKPLANE_NO_TAILSCALE=1 BACKPLANE_NAME=fl-box-a \
      setsid "$FL/backplane-hub" --home "$H" --port "$port" --host "$host" --no-tailscale --foreground --headless > "$FL/hub.log" 2>&1 < /dev/null &
    echo $! > "$FL/hub.pid"
    for _ in $(seq 100); do [ -s "$H/token" ] && break; sleep 0.2; done
    for _ in $(seq 100); do curl -s -o /dev/null "http://$host:$port/hello" && break; sleep 0.2; done
    curl -s -o /dev/null -w '' "http://$host:$port/hello" || { echo "hub did not start"; tail -20 "$FL/hub.log"; exit 1; }
    echo "token=$(cat "$H/token")"; echo "proj=$P"; echo "outside=$FL/outside/notes.txt"; echo "home=$H"; echo "far=$H/far-files/fl-peer/far-part.txt" ;;
  rss-start)
    pid=$(cat "$FL/hub.pid"); : > "$FL/rss.log"
    setsid bash -c "while kill -0 $pid 2>/dev/null; do awk '/^VmRSS/{print \$2}' /proc/$pid/status >> $FL/rss.log; sleep 0.1; done" > /dev/null 2>&1 < /dev/null &
    echo $! > "$FL/rss.pid"; sleep 0.5; echo "rss sampling" ;;
  rss-stop)
    kill "$(cat "$FL/rss.pid")" 2>/dev/null || true
    echo "rss base=$(head -1 "$FL/rss.log") peak=$(sort -n "$FL/rss.log" | tail -1) samples=$(wc -l < "$FL/rss.log")" ;;
  rewrite)
    # the same size, new contents and mtime: a stale If-Range must be told by the mtime part of the ETag, not the size
    sleep 1; head -c $((150 << 20)) /dev/urandom > "$P/mfg/big.bin.new"; mv "$P/mfg/big.bin.new" "$P/mfg/big.bin"
    echo "sha=$(sha256sum "$P/mfg/big.bin" | cut -d' ' -f1)" ;;
  sha) echo "sha=$(sha256sum "$P/mfg/big.bin" | cut -d' ' -f1)" ;;
  stop) [ -f "$FL/hub.pid" ] && kill -TERM -- "-$(cat "$FL/hub.pid")" 2>/dev/null || true; echo stopped ;;
esac
