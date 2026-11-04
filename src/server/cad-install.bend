import Base
import ../core/json.bend as J
import ../core/kicad_install.bend as KI

# Owns at most one job per tool per hub. No client owns a download or a
# subprocess: disconnects and navigation cannot cancel it accidentally.
def CadFx.start(home: String, tool: String, spec: String) -> IO(String):
  import "./effects/cad-install.js"

def CadFx.next(home: String, tool: String, attempt: String) -> IO(String):
  import "./effects/cad-install.js"

def CadFx.cancel(home: String, tool: String) -> IO(Unit):
  import "./effects/cad-install.js"

# The compiled catalog is selected by the hub, never by a phone/client.
def CadSpec.kicad(home: String, package: Maybe<&2, KI.Package>) -> J.Json:
  match package:
    case None{}: J.Json.obj([J.KV{"error", J.Str{"No Backplane KiCad package for this hub OS and architecture"}}])
    case Some{KI.Package{+platform, +version, +release, +asset, +sha, +format, +cli}}:
      J.Json.obj([J.KV{"url", J.Str{"https://github.com/i2cjak/Backplane_KiCad/releases/download/" ++ release ++ "/" ++ asset}},
        J.KV{"asset", J.Str{asset}}, J.KV{"sha256", J.Str{sha}}, J.KV{"script", J.Str{KI.Install.script()}},
        J.KV{"args", J.Json.arr([J.Str{home ++ "/kicad"}, J.Str{platform}, J.Str{version}, J.Str{release}, J.Str{asset}, J.Str{sha}, J.Str{format}, J.Str{cli}])}])

def CadSpec.freecad.script() -> String:
  "set -eu\nhome=$1; pkg=$2; platform=$3; version=$4\nif command -v cygpath >/dev/null 2>&1; then home=$(cygpath -u \"$home\"); pkg=$(cygpath -u \"$pkg\"); fi\nparent=$home/freecad\ndir=$parent/$version\nfail() { echo \"$*\" >&2; exit 1; }\n[ ! -e \"$dir\" ] && [ ! -L \"$dir\" ] || fail 'FreeCAD destination exists; existing installation preserved'\nmkdir -p \"$parent\"\nlock=$parent/.install.lock\nmkdir \"$lock\" 2>/dev/null || fail 'Another FreeCAD installation is active'\nstage=; mounted=0\ncleanup() {\n  if [ \"$mounted\" = 1 ]; then hdiutil detach \"$stage/mount\" -quiet || true; fi\n  if [ -n \"$stage\" ]; then rm -rf \"$stage\"; fi\n  rmdir \"$lock\" || true\n}\ntrap cleanup 0\ntrap 'exit 1' 1 2 15\nstage=$(mktemp -d \"$parent/.install-XXXXXX\")\nmkdir \"$stage/runtime\"\necho 'BP_INSTALL extract'\ncase \"$platform\" in\n  linux-*)\n    chmod 700 \"$pkg\"\n    (cd \"$stage/runtime\" && \"$pkg\" --appimage-extract >/dev/null)\n    [ -x \"$stage/runtime/squashfs-root/AppRun\" ] || fail 'FreeCAD AppImage layout is incomplete'\n    [ -x \"$stage/runtime/squashfs-root/usr/bin/freecadcmd\" ] || fail 'FreeCAD console is missing'\n    mkdir \"$stage/runtime/bin\"\n    cat > \"$stage/runtime/bin/freecadcmd\" <<'WRAPPER'\n#!/bin/sh\nhere=$(CDPATH= cd -- \"$(dirname -- \"$0\")/..\" && pwd)\nexec \"$here/squashfs-root/AppRun\" freecadcmd \"$@\"\nWRAPPER\n    cat > \"$stage/runtime/bin/freecad\" <<'WRAPPER'\n#!/bin/sh\nhere=$(CDPATH= cd -- \"$(dirname -- \"$0\")/..\" && pwd)\nexec \"$here/squashfs-root/AppRun\" \"$@\"\nWRAPPER\n    chmod 700 \"$stage/runtime/bin/\"*\n    cli=$dir/bin/freecadcmd\n    ;;\n  darwin-*)\n    mkdir \"$stage/mount\"\n    hdiutil attach \"$pkg\" -readonly -nobrowse -mountpoint \"$stage/mount\" -quiet\n    mounted=1\n    [ -d \"$stage/mount/FreeCAD.app\" ] || fail 'FreeCAD app is missing from disk image'\n    ditto \"$stage/mount/FreeCAD.app\" \"$stage/runtime/FreeCAD.app\"\n    hdiutil detach \"$stage/mount\" -quiet\n    mounted=0\n    cli=$dir/FreeCAD.app/Contents/Resources/bin/freecadcmd\n    [ -x \"$stage/runtime/FreeCAD.app/Contents/Resources/bin/freecadcmd\" ] || fail 'FreeCAD console is missing'\n    ;;\n  windows-x86_64)\n    tar=$(cygpath -S)/tar.exe\n    [ -x \"$tar\" ] || fail 'Windows archive tool is missing'\n    \"$tar\" -tf \"$(cygpath -w \"$pkg\")\" > \"$stage/members\"\n    while IFS= read -r member; do\n      case \"$member\" in /*|*:*|*\\\\*) fail 'Unsafe FreeCAD archive path';; esac\n      case \"/$member/\" in *'/../'*) fail 'Unsafe FreeCAD archive path';; esac\n    done < \"$stage/members\"\n    (cd \"$stage/runtime\" && \"$tar\" -xf \"$(cygpath -w \"$pkg\")\")\n    find \"$stage/runtime\" -type f -iname freecadcmd.exe > \"$stage/commands\"\n    [ \"$(wc -l < \"$stage/commands\" | tr -d ' ')\" = 1 ] || fail 'FreeCAD console layout is ambiguous'\n    IFS= read -r source < \"$stage/commands\"\n    cli=$dir/${source#\"$stage/runtime/\"}\n    ;;\n  *) fail 'No FreeCAD portable package for this hub platform';;\nesac\necho 'BP_INSTALL configure'\n[ ! -e \"$dir\" ] && [ ! -L \"$dir\" ] || fail 'FreeCAD destination appeared; existing installation preserved'\ntrap '' 1 2 15\necho BP_COMMIT_BEGIN\nmv \"$stage/runtime\" \"$dir\"\nif [ \"$platform\" = windows-x86_64 ]; then cli=$(cygpath -w \"$cli\"); fi\n# The command pointer is atomically replaced only after the payload exists.\nprintf '%s\\n' \"$cli\" > \"$stage/command\"\nmv \"$stage/command\" \"$parent/command\"\necho BP_COMMITTED\n"

def CadSpec.freecad.package(platform: String, +asset: String, sha: String) -> J.Json:
  J.Json.obj([J.KV{"platform", J.Str{platform}}, J.KV{"version", J.Str{"1.1.4"}}, J.KV{"asset", J.Str{asset}},
    J.KV{"url", J.Str{"https://github.com/FreeCAD/FreeCAD/releases/download/1.1.4/" ++ asset}}, J.KV{"sha256", J.Str{sha}}, J.KV{"script", J.Str{CadSpec.freecad.script()}}])

# Official release 1.1.4, https://github.com/FreeCAD/FreeCAD/releases/tag/1.1.4
# SHA-256 values are pinned from the release API's asset digests.
def CadSpec.freecad(platform: String) -> J.Json:
  match platform:
    case "linux-x86_64": CadSpec.freecad.package("linux-x86_64", "FreeCAD_1.1.4-Linux-x86_64-py311.AppImage", "f6dc6ba676e5ac96a565ebc8d657232f94c6158e85b4352141bd1a46f6b43434")
    case "linux-arm64": CadSpec.freecad.package("linux-arm64", "FreeCAD_1.1.4-Linux-aarch64-py311.AppImage", "675d1a4295b2183e0e1d816a4b5557dcb63aeced4e60aaa28d6c3686f4c71dc9")
    case "darwin-arm64": CadSpec.freecad.package("darwin-arm64", "FreeCAD_1.1.4-macOS-arm64-py311.dmg", "071343b4abb70492b75c973f41eaf1d2528f9b9c7ea018d22a4f46ae14d27ac0")
    case "darwin-x86_64": CadSpec.freecad.package("darwin-x86_64", "FreeCAD_1.1.4-macOS-x86_64-py311.dmg", "e173d0dcf47a5166b59cbb33adccf54cd751eb9ceaa2c86486230d0d079648c1")
    case "windows-x86_64": CadSpec.freecad.package("windows-x86_64", "FreeCAD_1.1.4-Windows-x86_64-py311.7z", "4828741fc91ee37fafcdb97a1abacb18b04ba451ac4372d9ff7a7349b36f4d6d")
    case _: J.Json.obj([J.KV{"error", J.Str{"Install FreeCAD using the official guide for this hub platform"}}])

def CadSpec.platform.parts(ws: List<&2, String>) -> String:
  match ws:
    case os <> arch <> Nil{}: KI.Install.os(String.to_lower(os)) ++ "-" ++ KI.Install.arch(String.to_lower(arch))
    case _: "unsupported"

def CadSpec.platform(uname: String) -> String:
  CadSpec.platform.parts(List.filter(~String, ~(x => Bool.not(String.is_empty(x))), String.split(String.trim(uname), ' ')))

def CadSpec.of(+tool: String, home: String, +uname: String) -> J.Json:
  Bool.pick(J.Json, String.eq(tool, "kicad"), CadSpec.kicad(home, KI.Install.platform(uname)), CadSpec.freecad(CadSpec.platform(uname)))
