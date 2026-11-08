#!/bin/bash
# windows-shaders.sh HOST: make native/gpui/vendor/gpui/windows-shaders/shaders_bytes.rs, the HLSL shader bytes a Windows client cross-built here embeds.
# gpui compiles its shaders with the Windows SDK's fxc.exe only when the build host is Windows; this runs the same fxc commands (gpui's build.rs,
# windows::compile_shaders) on HOST, a Windows machine reachable by ssh with the SDK installed, and writes the Rust bindings the way gpui does.
# The file's first line is the FNV-1a 64 of every .hlsl there (by name); the vendored build.rs refuses bytes made from other sources. Run again when gpui changes.
set -euo pipefail
host=${1:?usage: windows-shaders.sh HOST}
HERE=$(cd "$(dirname "$0")/.." && pwd); G=$HERE/native/gpui/vendor/gpui; S=$G/src/platform/windows
. "$HERE/scripts/lib-bptmp.sh"; w=$(mktemp -d "$BP_TMP/winshaders.XXXXXX"); trap 'rm -rf "$w"' EXIT
hash=$(python3 -I - "$S" <<'PY'
import sys, os
h = 0xcbf29ce484222325
d = sys.argv[1]
for p in sorted(os.path.join(d, f) for f in os.listdir(d) if f.endswith('.hlsl')):
    for b in open(p, 'rb').read(): h = ((h ^ b) * 0x100000001b3) & 0xffffffffffffffff
print(f"{h:016x}")
PY
)
r='C:\bp\shaders'
ssh -o BatchMode=yes "$host" "Remove-Item -Recurse -Force $r -EA SilentlyContinue; New-Item -ItemType Directory $r | Out-Null" 2>&1 | grep -v '^\*\*' || true
scp -q "$S"/*.hlsl "$host:C:/bp/shaders/"   # the entry files and what they #include
# the modules and entry points of gpui's windows::compile_shaders, in its order
cat > "$w/fxc.ps1" <<'PS'
$ErrorActionPreference = 'Stop'
$fxc = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin\*\x64\fxc.exe' | Sort-Object FullName | Select-Object -Last 1
if (-not $fxc) { throw 'no fxc.exe (Windows SDK)' }
"fxc $($fxc.FullName)"
Set-Location C:\bp\shaders
$mods = @('quad','shadow','path_rasterization','path_sprite','underline','monochrome_sprite','polychrome_sprite') | % { ,@($_, 'shaders.hlsl') }
$mods += ,@('emoji_rasterization', 'color_text_raster.hlsl')
foreach ($m in $mods) { foreach ($k in @(@('vertex','vs_4_1','vs','VERTEX'), @('fragment','ps_4_1','ps','FRAGMENT'))) {
  $n = $m[0]; & $fxc.FullName /nologo /T $k[1] /E "$($n)_$($k[0])" /Fh "$($n)_$($k[2]).h" /Vn "$($n.ToUpper())_$($k[3])_BYTES" /O3 $m[1] | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "fxc failed: $n $($k[0])" }
} }
PS
scp -q "$w/fxc.ps1" "$host:C:/bp/shaders/fxc.ps1"
ssh -o BatchMode=yes "$host" "powershell -NoProfile -ExecutionPolicy Bypass -File $r\\fxc.ps1" 2>&1 | grep -v '^\*\*'
scp -q "$host:C:/bp/shaders/*.h" "$w/"
out=$G/windows-shaders/shaders_bytes.rs; mkdir -p "$(dirname "$out")"
python3 -I - "$w" "$out" "$hash" <<'PY'
import sys, os
w, out, h = sys.argv[1:]
mods = ['quad','shadow','path_rasterization','path_sprite','underline','monochrome_sprite','polychrome_sprite','emoji_rasterization']
lines = [f"// hlsl fnv1a64 {h}\n", "// made by scripts/windows-shaders.sh: fxc /O3 on gpui 0.2.2's shaders.hlsl and color_text_raster.hlsl, bindings as gpui's build.rs writes them\n"]
for m in mods:
    for s, k in (('vs', 'VERTEX'), ('ps', 'FRAGMENT')):
        t = open(os.path.join(w, f"{m}_{s}.h"), encoding='latin-1').read()
        g = t[t.index("const BYTE"):]; d = g[g.index('=') + 1:].strip()
        lines.append(f"const {m.upper()}_{k}_BYTES: &[u8] = &{d.replace('{', '[').replace('}', ']')}\n")
open(out, 'w').write(''.join(lines))
PY
echo "windows-shaders: $out ($(wc -c < "$out") bytes, hlsl $hash)"
