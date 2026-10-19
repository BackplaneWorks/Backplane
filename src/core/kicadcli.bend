import Base
import ./json.bend as J

# Which kicad-cli the hub may use on a machine (pure; laws kicad_cli_*).
# Known fork layouts are Linux x86-64 and Apple-silicon macOS. A layout
# does not promise a published asset: Cli.published pins that separately. Each build
# installs in a folder of its own, so a copy for another machine (the Linux
# one a Mac once downloaded, which cannot run there) is never picked. A Mac
# without the fork uses its KiCad.app, in /Applications or ~/Applications.
# server/kicad.bend resolves the first of these that exists (after an
# explicit BACKPLANE_KICAD_CLI), else PATH's.

def Cli.asset.k(+p: String) -> String:
  Bool.pick(String, String.eq(p, "Linux x86_64"), "linux-x86_64", Bool.pick(String, String.eq(p, "Darwin arm64"), "macos-arm64", ""))

# the fork's build for a machine (`uname -sm`), as its release names it; ""
# where there is none
def Cli.asset(+plat: String) -> String:
  Cli.asset.k(String.trim(plat))

def Cli.fork_runs(+plat: String) -> Bool:
  Bool.not(String.is_empty(Cli.asset(plat)))

def Cli.is_mac(+plat: String) -> Bool:
  String.starts_with(String.trim(plat), "Darwin")

# where the fork installs: Linux x86-64 keeps <data>/kicad/<version> (where
# installs have always been), any other build <version>-<asset>
def Cli.dir(+data: String, +ver: String, +asset: String) -> String:
  data ++ "/kicad/" ++ Bool.pick(String, String.eq(asset, "linux-x86_64"), ver, ver ++ "-" ++ asset)

# the release asset holding a build
def Cli.package(+asset: String, +ver: String) -> String:
  "backplane-kicad-" ++ asset ++ "-" ++ ver ++ ".tar.gz"

# the fork beside this install, then in our data directory
def Cli.fork(+exe: String, +data: String, +ver: String, +asset: String) -> List<&2, String>:
  [exe ++ "/kicad/bin/kicad-cli", Cli.dir(data, ver, asset) ++ "/bin/kicad-cli"]

def Cli.mac(+user: String) -> List<&2, String>:
  ["/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli", user ++ "/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli"]

def Cli.system() -> List<&2, String>:
  ["/usr/bin/kicad-cli", "/usr/local/bin/kicad-cli"]

def Cli.candidates.k(fork: Bool, mac: Bool, +exe: String, +data: String, +ver: String, +asset: String, +user: String) -> List<&2, String>:
  match fork mac:
    case True{} True{}:
      List.append(&2, String, Cli.fork(exe, data, ver, asset), List.append(&2, String, Cli.mac(user), Cli.system()))
    case True{} False{}:
      List.append(&2, String, Cli.fork(exe, data, ver, asset), Cli.system())
    case False{} True{}:
      List.append(&2, String, Cli.mac(user), Cli.system())
    case False{} False{}:
      Cli.system()

# where to look, in order, on a machine (`uname -sm`; user is $HOME)
def Cli.candidates(+plat: String, +exe: String, +data: String, +ver: String, +user: String) -> List<&2, String>:
  Cli.candidates.k(Cli.fork_runs(plat), Cli.is_mac(plat), exe, data, ver, Cli.asset(plat), user)

# what installing the fork answers where there is no build of it
def Cli.no_fork() -> String:
  "The pinned Backplane KiCad runtime is published for Linux x86-64 only. Use a local KiCad installation for exports and checks; IPC requires a compatible Backplane KiCad fork."

# The pinned bp.4 release publishes only Linux x86-64. Keep this allowlist
# tied to the pinned digest; a platform layout is not a download promise.
def Cli.published(+platform: String) -> Bool:
  String.eq(Cli.asset(platform), "linux-x86_64")

def Cli.unsupported(+platform: String, version: String) -> String:
  "Backplane KiCad " ++ version ++ " has no published runtime for " ++ String.trim(platform) ++ ". Use a local KiCad installation for exports and checks; IPC requires a compatible Backplane KiCad fork. The selected runtime was kept."

# Legacy manifests without a platform belong to the Linux-only releases.
# Explicit paths are never silently redirected by this rule.
def Cli.compatible(+platform: String, +declared: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(Cli.asset(platform))),
    Bool.pick(Bool, String.is_empty(declared), String.eq(Cli.asset(platform), "linux-x86_64"), String.eq(Cli.asset(platform), declared)))

# Version directories with a platform suffix only belong to that platform.
# Unsuffixed historical installs are Linux x86-64, never a Mac fallback.
def Cli.installed(+name: String, +platform: String) -> Bool:
  Bool.pick(Bool, String.eq(Cli.asset(platform), "linux-x86_64"),
    Bool.not(String.contains(name, "-macos-")),
    Bool.and(String.eq(Cli.asset(platform), "macos-arm64"), String.ends_with(name, "-macos-arm64")))

def Cli.has(xs: List<&2, J.Json>, +want: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case J.Str{s} <> t: Bool.or(String.eq(s, want), Cli.has(t, want))
    case _ <> t: Cli.has(t, want)

def Cli.api(value: J.Json) -> Bool:
  match value:
    case J.Num{"2"}: True{}
    case _: False{}

# The executable's version command identifies upstream KiCad, whereas the
# manifest and api-server describe identify the fork. All three must agree
# before the staged runtime can replace a working installation.
def Cli.identity.doc(+o: J.Json, +version: String, +asset: String) -> Bool:
  +caps = J.Json.elems(J.Json.get_obj(o, "capabilities"))
  Bool.and(String.eq(J.Json.get_str(o, "sourceRepository", ""), "https://github.com/i2cjak/Backplane_KiCad"),
    Bool.and(String.eq(J.Json.get_str(o, "version", ""), version),
    Bool.and(String.eq(J.Json.get_str(o, "platform", ""), asset),
    Bool.and(Cli.api(J.Json.get_obj(o, "apiVersion")),
    Bool.and(Cli.has(caps, "api-server"), Bool.and(Cli.has(caps, "headless-pcb"), Cli.has(caps, "headless-sch")))))))

def Cli.identity(+manifest: J.Json, +describe: J.Json, +cli: String, +version: String, +asset: String) -> Bool:
  +core = Maybe.default(&2, String, List.head(&2, String, String.split(version, '-')), "")
  Bool.and(Cli.identity.doc(manifest, version, asset),
    Bool.and(Cli.identity.doc(describe, version, asset),
    Bool.and(String.eq(String.trim(cli), core),
    Bool.and(String.eq(J.Json.get_str(manifest, "kicadVersion", ""), core),
    Bool.and(String.eq(J.Json.get_str(describe, "kicadVersion", ""), core),
    Bool.and(String.eq(J.Json.get_str(manifest, "apiDescriptor", ""), "share/backplane/api.desc"),
    Bool.and(String.eq(J.Json.get_str(describe, "apiDescriptor", ""), "share/backplane/api.desc"),
    Bool.and(Bool.not(String.is_empty(J.Json.get_str(manifest, "sourceCommit", ""))),
      String.eq(J.Json.get_str(manifest, "sourceCommit", ""), J.Json.get_str(describe, "sourceCommit", ""))))))))))
