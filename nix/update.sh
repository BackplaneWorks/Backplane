#!/usr/bin/env bash
# Rewrite nix/sources.nix from an official GitHub release.
#   nix/update.sh              # latest stable release
#   nix/update.sh v0.11.1      # one tag
# Then run `nix fmt` (alejandra).
set -euo pipefail

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tag=${1:-}

if ! command -v python3 >/dev/null 2>&1; then
  echo "update.sh needs python3" >&2
  exit 1
fi
if ! command -v curl >/dev/null 2>&1; then
  echo "update.sh needs curl" >&2
  exit 1
fi

if [ -z "$tag" ]; then
  tag=$(
    curl -fsSL https://api.github.com/repos/i2cjak/Backplane/releases/latest |
      python3 -c 'import json,sys; print(json.load(sys.stdin)["tag_name"])'
  )
fi
case $tag in
  v*) ;;
  *)
    echo "tag must look like v0.11.1 (got $tag)" >&2
    exit 1
    ;;
esac

echo "pinning $tag"
sums=$(curl -fsSL "https://github.com/i2cjak/Backplane/releases/download/${tag}/SHA256SUMS")
kicad=$(curl -fsSL "https://raw.githubusercontent.com/i2cjak/Backplane/${tag}/src/server/kicad.bend")

ROOT="$root" TAG="$tag" SUMS="$sums" KICAD="$kicad" python3 - <<'PY'
import base64, os, re, textwrap

tag = os.environ["TAG"]
version = tag[1:]
sums = os.environ["SUMS"]
kicad = os.environ["KICAD"]
m = re.search(r'def KiStack\.revision\(\) -> String:\n  "([0-9a-f]{40})"', kicad)
if not m:
    raise SystemExit("could not find KiStack.revision() in that tag's kicad.bend")
rev = m.group(1)

wanted = {
    "x86_64-linux": ("linux-x64", f"backplane-{tag}-linux-x64.tar.gz"),
    "aarch64-linux": ("linux-arm64", f"backplane-{tag}-linux-arm64.tar.gz"),
}
found = {}
for line in sums.splitlines():
    parts = line.split()
    if len(parts) != 2:
        continue
    digest, name = parts
    for system, (directory, filename) in wanted.items():
        if name == filename:
            raw = bytes.fromhex(digest)
            found[system] = (directory, filename, "sha256-" + base64.b64encode(raw).decode())

missing = [s for s in wanted if s not in found]
if missing:
    raise SystemExit(f"{tag} is missing release archives for: {', '.join(missing)}")

def block(system):
    directory, filename, sri = found[system]
    url = f"https://github.com/i2cjak/Backplane/releases/download/{tag}/{filename}"
    return textwrap.dedent(f"""\
      {system} = {{
        dir = "{directory}";
        url = "{url}";
        hash = "{sri}";
      }};
    """).rstrip()

body = "\n".join(block(s) for s in ("aarch64-linux", "x86_64-linux"))
text = f"""# Pins for the official release binaries. nix/update.sh rewrites this from
# GitHub's SHA256SUMS and from the tag's KiStack.revision().
{{
  version = "{version}";
  # src/server/kicad.bend KiStack.revision() at this tag.
  kistackRevision = "{rev}";
  platforms = {{
{textwrap.indent(body, "    ")}
  }};
}}
"""
path = os.environ["ROOT"] + "/nix/sources.nix"
with open(path, "w", encoding="utf-8") as f:
    f.write(text)
print(f"wrote {path}")
print(f"version {version}")
print(f"kistack {rev}")
PY

echo "run nix fmt to normalize nix/sources.nix"
