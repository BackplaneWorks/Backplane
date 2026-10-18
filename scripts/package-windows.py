#!/usr/bin/env python3
"""Package our PE, web files and launchers; Cygwin is installed separately."""
from pathlib import Path
import hashlib, json, re, subprocess, sys, zipfile

root = Path(__file__).resolve().parent.parent
version = sys.argv[1]
if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9.+_-]*", version):
    sys.exit("package-windows: invalid version")
exe = root / "dist/backplane.exe"
if exe.read_bytes()[:2] != b"MZ":
    sys.exit("package-windows: expected a Windows PE executable")
web = root / "dist/web"
if not (web / "index.html").is_file():
    sys.exit("package-windows: web bundle missing")
target = root / f"dist/backplane-{version}-windows-x64.zip"
prefix = "backplane-windows-x64/"
files = [(exe, "backplane.exe"), (root / "LICENSE", "LICENSE"),
         (root / "scripts/setup-windows.ps1", "Install-Runtime.ps1"),
         (root / "scripts/install-windows-providers.ps1", "Install-Providers.ps1")]
for name in ['backplane-kipc.exe','backplane-step2glb.exe']:
    helper=root/'dist'/name
    if helper.read_bytes()[:2] != b'MZ': sys.exit('package-windows: expected native helper '+name)
    files.append((helper,name))
files.append((root/'dist/WINDOWS-HELPERS.json','WINDOWS-HELPERS.json'))
licenses=root/'dist/licenses'
if not (licenses/'bun/LICENSE.md').is_file() or not (licenses/'step2glb/license.occt.txt').is_file():
    sys.exit('package-windows: helper dependency licenses missing')
files += [(p,'licenses/'+p.relative_to(licenses).as_posix()) for p in sorted(licenses.rglob('*')) if p.is_file()]
files += [(p, p.name) for p in sorted((root / "deploy/windows").iterdir()) if p.is_file()]
files += [(p, "web/" + p.relative_to(web).as_posix()) for p in sorted(web.rglob("*")) if p.is_file()]
manifest = {"version": version, "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(), "target": "windows-x64",
            "runtime": "Cygwin (installed separately)",
            "sha256": {name: hashlib.sha256(p.read_bytes()).hexdigest() for p, name in files}}
with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as z:
    for p, name in files:
        z.write(p, prefix + name)
    z.writestr(prefix + "BUILD.json", json.dumps(manifest, indent=2) + "\n")
digest = hashlib.sha256(target.read_bytes()).hexdigest()
target.with_suffix(".zip.sha256").write_text(f"{digest}  {target.name}\n")
print(target)
