#!/bin/sh
# Stand-in for the Backplane KiCad installer: same arguments ($1 parent, $3 version, $5 asset, $8 cli). The first run fails, the next succeeds.
n=$(cat "$RIG/runs" 2>/dev/null || echo 0); n=$((n+1)); echo $n > "$RIG/runs"
echo "Downloading $5"; sleep 5
echo "Checking the checksum"; sleep 2
if [ "$n" = 1 ]; then echo "Could not download https://github.com/i2cjak/Backplane_KiCad/releases/download/backplane-kicad-v$3/$5" >&2; exit 1; fi
echo "Unpacking"; sleep 2
d=$1/$3; mkdir -p "$d/bin" "$d/share/backplane"
printf '#!/bin/sh\necho %s\n' "$3" > "$d/$8"; chmod +x "$d/$8"
printf '{"version":"%s","platform":"linux-x86_64","apiVersion":99}\n' "$3" > "$d/manifest.json"; echo x > "$d/share/backplane/api.desc"
echo "Installed in $d"
