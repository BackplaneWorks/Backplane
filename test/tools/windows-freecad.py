#!/usr/bin/env python3
"""Real Windows FreeCAD GUI fallback for native_box.py's orange STEP fixture.

Run in Cygwin with BACKPLANE_FREECAD naming FreeCADCmd.exe. Arguments are the
native STEP helper executable and the STEP exported by the fixture.
"""
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

if sys.platform != "cygwin" or not os.environ.get("BACKPLANE_FREECAD"):
    raise SystemExit("Run in Windows Cygwin with BACKPLANE_FREECAD set")
helper, fixture = (Path(p).resolve(strict=True) for p in sys.argv[1:3])
with tempfile.TemporaryDirectory(prefix="bp-freecad-native-") as directory:
    root=Path(directory)
    source=root/'orange.step'
    source.write_bytes(fixture.read_bytes() + b' '*(13*1024*1024))
    target=root/'orange.glb'
    result=subprocess.run([str(helper),str(source),str(target)],
                          capture_output=True,text=True,encoding='utf-8',
                          timeout=60,check=True)
    data=target.read_bytes()
    assert data[:4] == b'glTF'
    size,kind=struct.unpack_from('<II',data,12)
    assert kind == 0x4e4f534a
    gltf=json.loads(data[20:20+size])
    assert gltf['asset']['generator']=='backplane-step2glb (FreeCAD)',gltf['asset']
    colors=[m['pbrMetallicRoughness']['baseColorFactor'][:3] for m in gltf['materials']]
    assert any(all(abs(a-b)<0.02 for a,b in zip(c,[0.9,0.35,0.1])) for c in colors),colors
    print(result.stderr.strip())
    print('WINDOWS_FREECAD_GUI_COLOR_AND_PIPE_COMPLETION_PASS')
