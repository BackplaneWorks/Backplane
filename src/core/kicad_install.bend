import Base

# Published packages, selected by the hub's uname (never a connecting client).
# Linux keeps its accepted bp.4 pin; Windows and Apple Silicon have bp.6.
type Package is Data:
  Package{platform: String, version: String, release: String, asset: String, sha256: String, format: String, cli: String}

def Install.linux() -> Package:
  Package{"linux-x86_64", "10.99.0-bp.4", "backplane-kicad-v10.99.0-bp.4", "backplane-kicad-linux-x86_64-10.99.0-bp.4.tar.gz", "f60a71817d057f9dcf3faef632b2f85d06a613108dee23807c9de81aae97e0b4", "tar.gz", "bin/kicad-cli"}

def Install.mac() -> Package:
  Package{"darwin-arm64", "10.99.0-bp.6", "backplane-kicad-v10.99.0-bp.6", "backplane-kicad-darwin-arm64-10.99.0-bp.6.tar.gz", "99584fb0ee490354098778d39e3aaed9e0f274ee39972851a9cbeabfb271462d", "tar.gz", "bin/kicad-cli"}

def Install.windows() -> Package:
  Package{"windows-x86_64", "10.99.0-bp.6", "backplane-kicad-v10.99.0-bp.6", "backplane-kicad-windows-x86_64-10.99.0-bp.6.zip", "55f84153c3ba6f9a8e7e005558a02b9c2fa72dd5deeed0211f908336c830870c", "zip", "bin/kicad-cli.exe"}

def Install.os(+os: String) -> String:
  Bool.pick(String, Bool.or(String.starts_with(os, "cygwin_nt-"), Bool.or(String.starts_with(os, "mingw64_nt-"), String.starts_with(os, "msys_nt-"))), "windows", os)

def Install.arch(+arch: String) -> String:
  Bool.pick(String, Bool.or(String.eq(arch, "amd64"), String.eq(arch, "x64")), "x86_64", Bool.pick(String, String.eq(arch, "aarch64"), "arm64", arch))

def Install.pick(os: String, arch: String) -> Maybe<&2, Package>:
  match os arch:
    case "linux" "x86_64": Some{Install.linux()}
    case "darwin" "arm64": Some{Install.mac()}
    case "windows" "x86_64": Some{Install.windows()}
    case "windows_nt" "x86_64": Some{Install.windows()}
    case "win32" "x86_64": Some{Install.windows()}
    case _ _: None{}

def Install.select(os: String, arch: String) -> Maybe<&2, Package>:
  Install.pick(Install.os(String.to_lower(os)), Install.arch(String.to_lower(arch)))

def Install.words(ws: List<&2, String>) -> Maybe<&2, Package>:
  match ws:
    case os <> arch <> Nil{}: Install.select(os, arch)
    case _: None{}

def Install.platform(uname: String) -> Maybe<&2, Package>:
  Install.words(List.filter(~String, ~(x => Bool.not(String.is_empty(x))), String.split(String.trim(uname), ' ')))

def Install.available(p: Maybe<&2, Package>) -> Bool:
  match p:
    case Some{_}: True{}
    case None{}: False{}

def Install.version(p: Package) -> String:
  match p:
    case Package{_, v, _, _, _, _, _}: v

def Install.preferred(p: Maybe<&2, Package>) -> String:
  match p:
    case Some{p}: Install.version(p)
    case None{}: Install.version(Install.linux())

def Install.installed(p: Maybe<&2, Package>) -> String:
  match p:
    case Some{Package{_, v, _, _, _, _, cli}}: v ++ "/" ++ cli
    case None{}: Install.preferred(None{}) ++ "/bin/kicad-cli"

# Embedded so installed native hubs need no source checkout or extra script.
# Tests extract and execute these exact bytes against disposable archives.
def Install.script() -> String:
  "set -eu\n# Arguments come only from the compiled platform catalog, never the client.\nparent=$1; platform=$2; version=$3; release=$4; asset=$5; pinned=$6; format=$7; cli=$8\nfail() { echo \"$*\" >&2; exit 1; }\ncase \"$platform:$format:$cli\" in\n  linux-x86_64:tar.gz:bin/kicad-cli|darwin-arm64:tar.gz:bin/kicad-cli|windows-x86_64:zip:bin/kicad-cli.exe) ;;\n  *) fail 'No published Backplane KiCad package for this hub platform' ;;\nesac\ncase \"$version\" in ''|*[!0-9A-Za-z.-]*) fail 'Invalid runtime version' ;; esac\n[ \"$release\" = \"backplane-kicad-v$version\" ] || fail 'Invalid runtime release'\nroot=backplane-kicad-$platform-$version\n[ \"$asset\" = \"$root.$format\" ] || fail 'Invalid runtime archive'\ncase \"$pinned\" in ''|*[!0-9a-f]*) fail 'Invalid pinned digest' ;; esac\n[ \"${#pinned}\" -eq 64 ] || fail 'Invalid pinned digest'\ndir=$parent/$version\n# Never unpack over an existing runtime (including an incomplete old install).\n[ ! -e \"$dir\" ] && [ ! -L \"$dir\" ] || fail 'Runtime destination already exists; existing installation preserved'\numask 077\nmkdir -p \"$parent\"\nlock=$parent/.install-$version.lock\nmkdir \"$lock\" 2>/dev/null || fail 'Another KiCad installation is active; retry when it finishes'\nstage=\ncleanup() { if [ -n \"$stage\" ]; then rm -rf \"$stage\"; fi; rmdir \"$lock\"; }\ntrap cleanup 0\ntrap 'exit 1' 1 2 15\nstage=$(mktemp -d \"$parent/.install-XXXXXX\")\npkg=$stage/$asset\nbase=https://github.com/i2cjak/Backplane_KiCad/releases/download/$release/\ncurl --proto '=https' --tlsv1.2 -fsSL -o \"$pkg\" \"$base$asset\"\ncurl --proto '=https' --tlsv1.2 -fsSL -o \"$stage/SUMS\" \"${base}SHA256SUMS\"\nwant=$(awk -v n=\"$asset\" '{ f=$2; sub(/^\\*/, \"\", f); sub(/^\\.\\//, \"\", f); if (f==n) print $1 }' \"$stage/SUMS\")\nif command -v sha256sum >/dev/null 2>&1; then\n  got=$(sha256sum \"$pkg\" | cut -d' ' -f1)\nelse\n  got=$(shasum -a 256 \"$pkg\" | cut -d' ' -f1)\nfi\n[ \"$want\" = \"$got\" ] && [ \"$got\" = \"$pinned\" ] || fail 'KiCad archive checksum mismatch'\nmagic=$(od -An -N4 -tx1 \"$pkg\" | tr -d ' \\n')\nmkdir \"$stage/unpack\"\ncase \"$format:$magic\" in\n  tar.gz:1f8b08*)\n    tar -tzf \"$pkg\" > \"$stage/members\"\n    while IFS= read -r member; do\n      case \"$member\" in ./*) member=${member#./} ;; esac\n      case \"/$member/\" in *'/../'*|*'/./'*) fail 'Unsafe runtime archive path' ;; esac\n      case \"$member\" in \"$root\"|\"$root/\"*) ;; *) fail 'Unexpected runtime archive root' ;; esac\n    done < \"$stage/members\"\n    tar -xzf \"$pkg\" -C \"$stage/unpack\"\n    ;;\n  zip:504b0304*)\n    # Windows ships PowerShell; runtime-only Cygwin needs no Python/unzip.\n    # The packaged hub PATH is closed (runtime, System32, Windows): find\n    # Windows PowerShell where Windows keeps it, not on PATH.\n    ps=$(cygpath -S)/WindowsPowerShell/v1.0/powershell.exe\n    [ -f \"$ps\" ] || ps=powershell.exe\n    # Pass native paths as environment values, never interpolate shell text.\n    BP_KICAD_ZIP=$(cygpath -w \"$pkg\") \\\n    BP_KICAD_OUT=$(cygpath -w \"$stage/unpack\") \\\n    BP_KICAD_ROOT=$root \"$ps\" -NoProfile -NonInteractive -Command '\n      $ErrorActionPreference=\"Stop\"\n      Add-Type -AssemblyName System.IO.Compression.FileSystem\n      $zip=[IO.Compression.ZipFile]::OpenRead($env:BP_KICAD_ZIP)\n      try {\n        foreach($entry in $zip.Entries) {\n          $n=$entry.FullName.Replace(\"\\\", \"/\")\n          if (!$n.StartsWith($env:BP_KICAD_ROOT+\"/\", [StringComparison]::Ordinal) -or\n              @($n.Split(\"/\") | Where-Object {$_ -eq \"..\" -or $_ -eq \".\"}).Count -gt 0 -or\n              $n.Contains(\":\")) { throw \"Unsafe runtime archive path\" }\n        }\n      } finally { $zip.Dispose() }\n      Expand-Archive -LiteralPath $env:BP_KICAD_ZIP -DestinationPath $env:BP_KICAD_OUT -ErrorAction Stop\n    '\n    ;;\n  *) fail 'KiCad archive header does not match its platform format' ;;\nesac\nruntime=$stage/unpack/$root\n[ -f \"$runtime/$cli\" ] && [ -s \"$runtime/manifest.json\" ] && [ -s \"$runtime/share/backplane/api.desc\" ] || fail 'KiCad archive runtime layout is incomplete'\nif [ \"$platform\" = windows-x86_64 ]; then\n  [ \"$(od -An -N2 -tx1 \"$runtime/$cli\" | tr -d ' \\n')\" = 4d5a ] || fail 'Windows KiCad CLI is not a PE executable'\nelse\n  [ -x \"$runtime/$cli\" ] || fail 'KiCad launcher is not executable'\nfi\ntr -d '[:space:]' < \"$runtime/manifest.json\" > \"$stage/manifest\"\ngrep -Fq \"\\\"version\\\":\\\"$version\\\"\" \"$stage/manifest\" || fail 'KiCad manifest version mismatch'\ngrep -Fq \"\\\"platform\\\":\\\"$platform\\\"\" \"$stage/manifest\" || fail 'KiCad manifest platform mismatch'\n[ ! -e \"$dir\" ] && [ ! -L \"$dir\" ] || fail 'Runtime destination appeared; existing installation preserved'\nmv \"$runtime\" \"$dir\"\n"
