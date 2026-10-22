import Base
import ./host.bend as H
import ./proc.bend as P
import ../core/json.bend as J
import ../core/semver.bend as SV
import ../core/version.bend as VR

# Updates from GitHub releases. A check lists the recent releases, takes
# the newest one the update channel allows (SV.Chan.pick: `stable` only full
# releases, `nightly` also the nightly prereleases), and announces it when
# it is newer than this build (law update_never_same). Applying downloads this platform's tarball, checks its
# sha256 against the release's SHA256SUMS, unpacks it beside the running
# binary, swaps the files in with renames, and restarts in place.

def Update.repo() -> String:
  "i2cjak/Backplane"

# stable asks for the latest full release (however many nightlies follow
# it); nightly lists the recent ones
def Update.api(+channel: String) -> String:
  "https://api.github.com/repos/" ++ Update.repo() ++ Bool.pick(String, String.eq(SV.Chan.of(channel), "nightly"), "/releases?per_page=40", "/releases/latest")

# the releases an answer holds (a list, or the one /releases/latest gives)
def Update.items(j: J.Json) -> List<&2, J.Json>:
  match j:
    case J.Arr{xs}:
      J.Json.elems(J.Arr{xs})
    case J.Obj{fs}:
      [J.Obj{fs}]
    case _:
      Nil{}

type Asset is Data:
  Asset{name: String, url: String}

type Release is Data:
  Release{tag: String, assets: List<&2, Asset>}

def Update.assets(xs: List<&2, J.Json>) -> List<&2, Asset>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Asset{J.Json.get_str(x, "name", ""), J.Json.get_str(x, "browser_download_url", "")} <> Update.assets(t)

# the releases as the channel picker sees them (drafts are left out)
def Update.rels.k(draft: Bool, +x: J.Json, rest: List<&2, SV.Rel>) -> List<&2, SV.Rel>:
  match draft:
    case True{}:
      rest
    case False{}:
      SV.Rel{J.Json.get_str(x, "tag_name", ""), J.Json.get_bool(x, "prerelease", False{})} <> rest

def Update.rels(xs: List<&2, J.Json>) -> List<&2, SV.Rel>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Update.rels.k(J.Json.get_bool(x, "draft", False{}), x, Update.rels(t))

def Update.named.k(hit: Bool, +x: J.Json, +tag: String, rest: Maybe<&2, Release>) -> Maybe<&2, Release>:
  match hit:
    case True{}:
      Some{Release{tag, Update.assets(J.Json.elems(J.Json.get_obj(x, "assets")))}}
    case False{}:
      rest

def Update.named(xs: List<&2, J.Json>, +tag: String) -> Maybe<&2, Release>:
  match xs:
    case Nil{}:
      None{}
    case +x <> t:
      Update.named.k(Bool.and(Bool.not(String.is_empty(tag)), String.eq(J.Json.get_str(x, "tag_name", ""), tag)), x, tag, Update.named(t, tag))

# the release a channel takes from the listed ones
def Update.pick(+channel: String, +xs: List<&2, J.Json>) -> Maybe<&2, Release>:
  Update.named(xs, SV.Chan.pick(SV.Chan.of(channel), Update.rels(xs)))

def Update.ran(r: P.Ran, +channel: String) -> Maybe<&2, Release>:
  match r:
    case P.Ran{0, out}:
      Update.pick(channel, Update.items(Maybe.default(&2, J.Json, J.Json.parse(out), J.Null{})))
    case _:
      None{}

# the newest release on a channel, or None when offline or rate-limited
def Update.latest(+channel: String) -> IO(Maybe<&2, Release>):
  do IO<Maybe<&2, Release>>:
    r : P.Ran <- P.Proc.run("curl", ["-fsSL", "--max-time", "20", "-H", "Accept: application/vnd.github+json", Update.api(channel)], "")
    return Update.ran(r, channel)

# "linux-x64", "linux-arm64", "darwin-arm64", "darwin-x64"
def Update.plat.k(linux: Bool, arm: Bool) -> String:
  Bool.pick(String, linux, "linux-", "darwin-") ++ Bool.pick(String, arm, "arm64", "x64")

def Update.plat(+uname: String) -> String:
  Update.plat.k(String.starts_with(uname, "Linux"), Bool.or(String.contains(uname, "arm64"), String.contains(uname, "aarch64")))

def Update.out(r: P.Ran) -> String:
  match r:
    case P.Ran{_, out}:
      String.trim(out)

def Update.platform() -> IO(String):
  do IO<String>:
    r : P.Ran <- P.Proc.run("uname", ["-sm"], "")
    return Update.plat(Update.out(r))

# the asset whose name ends with the suffix
def Update.find.hit(xs: List<&2, Asset>, +suffix: String) -> Bool:
  match xs:
    case Asset{n, _} <> _:
      String.ends_with(n, suffix)
    case Nil{}:
      False{}

def Update.find.go(xs: List<&2, Asset>, hit: Bool, +suffix: String) -> Maybe<&2, Asset>:
  match xs hit:
    case a <> _ True{}:
      Some{a}
    case _ <> +t False{}:
      Update.find.go(t, Update.find.hit(t, suffix), suffix)
    case Nil{} _:
      None{}

def Update.find(+xs: List<&2, Asset>, +suffix: String) -> Maybe<&2, Asset>:
  Update.find.go(xs, Update.find.hit(xs, suffix), suffix)

# Checking
# --------

def Update.newer(r: Maybe<&2, Release>) -> String:
  match r:
    case Some{Release{+tag, _}}:
      Bool.pick(String, SV.Ver.newer(SV.Ver.parse(tag), SV.Ver.parse(VR.Version.current())), tag, "")
    case None{}:
      ""

def Update.then.k(ok: Bool, err: String, k: Unit -> IO(String)) -> IO(String):
  match ok:
    case True{}:
      k(Unit{})
    case False{}:
      IO.pure(String, err)

# Applying
# --------

# run k unless an earlier step failed ("" means it did not)
def Update.then(+err: String, k: Unit -> IO(String)) -> IO(String):
  Update.then.k(String.is_empty(err), err, k)

def Update.run.k(r: P.Ran, +what: String) -> String:
  match r:
    case P.Ran{0, _}:
      ""
    case P.Ran{_, out}:
      what ++ " failed: " ++ String.trim(out)

def Update.run(cmd: String, args: List<&2, String>, +what: String) -> IO(String):
  do IO<String>:
    r : P.Ran <- P.Proc.run(cmd, args, "")
    return Update.run.k(r, what)

def Update.text(r: P.Ran) -> String:
  match r:
    case P.Ran{_, out}:
      out

# the sha256 SHA256SUMS lists for a file name ("" when absent)
def Update.sum.k(ls: List<&2, String>, +name: String) -> String:
  match ls:
    case Nil{}:
      ""
    case +l <> t:
      Bool.pick(String, String.ends_with(String.trim(l), name), String.take(l, 64n), Update.sum.k(t, name))

def Update.verify.k(+got: String, +want: String) -> String:
  Bool.pick(String, Bool.and(Bool.not(String.is_empty(want)), String.eq(got, want)), "", "checksum mismatch for the downloaded release")

# compare the file's sha256 with the published one
def Update.verify(+file: String, +name: String, +sums: String) -> IO(String):
  do IO<String>:
    r : P.Ran <- P.Proc.run("sh", ["-c", "command -v sha256sum >/dev/null && sha256sum \"$1\" || shasum -a 256 \"$1\"", "sh", file], "")
    return Update.verify.k(String.take(Update.text(r), 64n), Update.sum.k(String.lines(sums), name))

def Update.swap.err(e: U32 & String, +to: String) -> String:
  (_, m) = e
  "cannot replace " ++ to ++ ": " ++ m ++ " (reinstall with install.sh)"

def Update.swap.k(r: Result<&1, &1, U32 & String, Unit>, +to: String) -> String:
  match r:
    case Done{_}:
      ""
    case Fail{e}:
      Update.swap.err(e, to)

def Update.swap.one(+from: String, +to: String) -> IO(String):
  do IO<String>:
    r : Result<&1, &1, U32 & String, Unit> <- H.FS.rename(from, to)
    return Update.swap.k(r, to)

# the release's archive for this platform
def Update.archive(+plat: String) -> String:
  plat ++ ".tar.gz"

# the release's own installer swaps the package in
def Update.swap(+src: String, +exe: String, +tag: String) -> IO(String):
  Update.run("sh", [src ++ "/install-release.sh", src, exe], "install release " ++ tag)

def Update.install.go(+tag: String, +plat: String, +tn: String, +tu: String, +su: String, +exe: String, +dir: String) -> IO(String):
  do IO<String>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(dir)
    e1 : String <- Update.run("curl", ["-fsSL", "--max-time", "600", "-o", dir ++ "/" ++ tn, tu], "download")
    Update.then(e1, u =>
      IO.bind(P.Ran, String, P.Proc.run("curl", ["-fsSL", "--max-time", "60", su], ""), sums =>
        IO.bind(String, String, Update.verify(dir ++ "/" ++ tn, tn, Update.text(sums)), e2 =>
          Update.then(e2, u2 =>
            IO.bind(String, String, Update.run("tar", ["-xzf", dir ++ "/" ++ tn, "-C", dir], "unpack"), e3 =>
              Update.then(e3, u3 => Update.swap(dir ++ "/backplane-" ++ plat, exe, tag)))))))

# download, verify, unpack, swap in; answers "" or what went wrong
def Update.install(+tag: String, +plat: String, +tar: Asset, +sums: Asset, +exe: String, +home: String) -> IO(String):
  match tar sums:
    case Asset{+tn, +tu} Asset{_, +su}:
      Update.install.go(tag, plat, tn, tu, su, exe, home ++ "/updates/" ++ tag)

def Update.apply.assets(t: Maybe<&2, Asset>, s: Maybe<&2, Asset>, +tag: String, +plat: String, +exe: String, +home: String) -> IO(String):
  match t s:
    case Some{ta} Some{sa}:
      Update.install(tag, plat, ta, sa, exe, home)
    case _ _:
      IO.pure(String, "the release has no build for " ++ plat)

def Update.apply.k(r: Maybe<&2, Release>, +plat: String, +exe: String, +home: String) -> IO(String):
  match r:
    case Some{Release{+tag, +assets}}:
      Update.apply.assets(Update.find(assets, Update.archive(plat)), Update.find(assets, "SHA256SUMS"), tag, plat, exe, home)
    case None{}:
      IO.pure(String, "no release found")

# install the channel's newest release beside the running binary
def Update.apply(+channel: String, +exe: String, +home: String) -> IO(String):
  do IO<String>:
    r : Maybe<&2, Release> <- Update.latest(channel)
    +plat : String <- Update.platform()
    Update.apply.k(r, plat, exe, home)
