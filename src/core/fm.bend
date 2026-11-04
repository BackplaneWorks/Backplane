import Base
import ./diff.bend as DF
import ./utf8.bend as U8

# The viewer's Files tab (pure): a thread's project folder listed a folder
# at a time (the hub's files.dir: "d|f <tab> size <tab> name" lines, folders
# first, hidden ones left out), what each file opens as in the viewer, and
# paths from the project's root that never leave it (laws fm_*).
#
# A board or schematic opens in the board viewer, a STEP or GLB in 3D, an
# image or a PDF in a view of its own; project notes open as read-only text.
# Anything else is listed only.

type FEnt is Data:
  FEnt{dir: Bool, size: Nat, name: String}

def Fm.digits.go(s: String, +n: Nat) -> Nat:
  match s:
    case SNil{}:
      n
    case SCon{Chr{+c}, t}:
      Fm.digits.go(t, Bool.pick(Nat, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Nat.add(Nat.mul(n, 10n), U32.to_nat((c - 48 : U32))), n))

# a size the listing wrote (digits; anything else is 0)
def Fm.num(s: String) -> Nat:
  Fm.digits.go(s, 0n)

def Fm.ent.put(none: Bool, e: FEnt, rest: List<&2, FEnt>) -> List<&2, FEnt>:
  match none:
    case True{}:
      rest
    case False{}:
      e <> rest

def Fm.ent.k(ps: List<&2, String>, rest: List<&2, FEnt>) -> List<&2, FEnt>:
  match ps:
    case +k <> +n <> +name:
      Fm.ent.put(List.is_empty(&2, String, name), FEnt{String.eq(k, "d"), Fm.num(n), String.join(name, "\t")}, rest)
    case _:
      rest

def Fm.parse.go(ls: List<&2, String>) -> List<&2, FEnt>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Fm.ent.k(String.split(l, '\t'), Fm.parse.go(t))

# the listing's entries, in its order
def Fm.parse(+text: String) -> List<&2, FEnt>:
  Fm.parse.go(DF.Diff.lines(text))

def Fm.ends(xs: List<&2, String>, +p: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +x <> t:
      Bool.or(String.ends_with(p, x), Fm.ends(t, p))

def Fm.lower.c(+c: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)), (c + 32 : U32), c)

def Fm.lower.go(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      SCon{Chr{Fm.lower.c(c)}, Fm.lower.go(t)}

# a name's extension, lower case (a name is short: its last 12 characters)
def Fm.ext(+name: String) -> String:
  Fm.lower.go(String.drop(name, Nat.sub(String.length(name), 12n)))

def Fm.kind.k(+e: String) -> String:
  Bool.pick(String, String.ends_with(e, ".kicad_pcb"), "board",
    Bool.pick(String, String.ends_with(e, ".kicad_sch"), "schematic",
      Bool.pick(String, Fm.ends([".step", ".stp", ".glb"], e), "3d",
        Bool.pick(String, Fm.ends([".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp"], e), "image",
          Bool.pick(String, String.ends_with(e, ".pdf"), "pdf", "")))))

# what a file opens as in the viewer: "board", "schematic", "3d", "image",
# "pdf", or "" (no design viewer)
def Fm.kind(+name: String) -> String:
  Fm.kind.k(Fm.ext(name))

# Files can read project notes as well as opening design viewers. Keep
# that extra preview separate from the design-file classification.
def Fm.preview(+name: String) -> String:
  Bool.pick(String, Fm.ends([".txt", ".md", ".markdown", ".csv", ".json", ".log", ".yaml", ".yml"], Fm.ext(name)), "text", Fm.kind(name))

# a Markdown note: it opens as text, which the web's Files tab draws as
# Markdown (view.bend's View.md)
def Fm.md(+name: String) -> Bool:
  Fm.ends([".md", ".markdown"], Fm.ext(name))

# a path from the root: a folder's entry, the folder itself when "" is the root
def Fm.join(+dir: String, +name: String) -> String:
  Bool.pick(String, String.is_empty(dir), name, dir ++ "/" ++ name)

def Fm.last(+p: String) -> String:
  Maybe.default(&2, String, List.last(&2, String, String.split(p, '/')), p)

# the folder above ("" above a folder at the root, and above the root)
def Fm.parent(+dir: String) -> String:
  String.take(dir, Nat.sub(Nat.sub(String.length(dir), String.length(Fm.last(dir))), Bool.pick(Nat, String.contains(dir, "/"), 1n, 0n)))

# a path the hub may list or serve from the project: from its root, no ".."
# part, never absolute
def Fm.safe.go(ps: List<&2, String>) -> Bool:
  match ps:
    case Nil{}:
      True{}
    case +p <> t:
      Bool.and(Bool.not(String.eq(p, "..")), Fm.safe.go(t))

def Fm.safe(+rel: String) -> Bool:
  Bool.and(Bool.not(String.starts_with(rel, "/")), Fm.safe.go(String.split(rel, '/')))

# a path from the root made absolute
def Fm.abs(+root: String, +rel: String) -> String:
  Bool.pick(String, String.is_empty(rel), root, root ++ "/" ++ rel)

# an absolute path back to one from the root ("" when it is not under it)
def Fm.rel(+root: String, +p: String) -> String:
  Bool.pick(String, String.starts_with(p, root ++ "/"), String.drop(p, 1n+String.length(root)), "")

def Fm.tenths(+n: Nat, +unit: Nat, +name: String) -> String:
  Nat.show(Nat.div(n, unit)) ++ "." ++ Nat.show(Nat.div(Nat.mod(n, unit), Nat.div(unit, 10n))) ++ " " ++ name

# a file's size as a person reads it
def Fm.size(+n: Nat) -> String:
  Bool.pick(String, Nat.is_lt(n, 1024n), Nat.show(n) ++ " B",
    Bool.pick(String, Nat.is_lt(n, 1048576n), Nat.show(Nat.div(Nat.add(n, 512n), 1024n)) ++ " KB",
      Bool.pick(String, Nat.is_lt(n, 1073741824n), Fm.tenths(n, 1048576n, "MB"), Fm.tenths(n, 1073741824n, "GB"))))

# the folder's name as a trail shows it ("Project" for the root)
def Fm.title(+dir: String) -> String:
  Bool.pick(String, String.is_empty(dir), "Project", Fm.last(dir))

# Downloads (laws fm_zip_*, fm_get_*, fm_files_*): a file downloads as
# itself, a folder as <name>.zip, from the hub's /files/get and /files/zip
# (server.bend's FilesHttp.*), each named by the thread and a path from its
# folder's root (Fm.files.safe). A zip leaves out what is never the design
# or must not leave the machine: git's and the hub's own history, the
# threads' worktrees (each a copy of the project), KiCad's backup folders,
# autosaves and locks, node_modules, caches, and environment files (.env,
# .env.*: secrets). Other dotfiles of the project (.backplane.json,
# .gitignore, .github) stay in. The rules are data the hub's zip writer
# reads (effects/zip.js's zip_skip, which refuses a kind it does not know;
# test/files_zip_test.bend checks the two agree), each a kind and a text:
# "=" a name, "*" the end of a name, "^" the start of a name, "/" a path
# from the root.
# Hard limits for a folder download: 1 GiB of source data, 10,000 entries.
# Checked before headers, and again while streaming if the tree changes.
def Fm.zip.max_bytes() -> U32:
  1073741824

def Fm.zip.max_entries() -> U32:
  10000

def Fm.zip.rules() -> List<&2, String>:
  ["*.pem", "*.key", "=id_rsa", "=id_ed25519", "=.git-credentials", "=.netrc", "=.npmrc", "=.ssh", "=.aws", "=.gnupg", "=.config", "=.codex", "=.claude", "=.backplane-bend", "=secrets", "=vault", "=credentials", "=auth.json", "=token", "=.git", "=node_modules", "=.ds_store", "=__pycache__", "=.env", "^.env.", "*-backups", "*.lck", "^_autosave-", "/.backplane/history.git", "/.backplane/worktrees"]

def Fm.zip.rule(+r: String, +p: String) -> Bool:
  Bool.pick(Bool, String.starts_with(r, "="), String.eq(Fm.last(p), String.drop(r, 1n)),
    Bool.pick(Bool, String.starts_with(r, "*"), String.ends_with(Fm.last(p), String.drop(r, 1n)),
      Bool.pick(Bool, String.starts_with(r, "^"), String.starts_with(Fm.last(p), String.drop(r, 1n)),
        Bool.and(String.starts_with(r, "/"), String.eq(p, String.drop(r, 1n))))))

def Fm.zip.any(rs: List<&2, String>, +p: String) -> Bool:
  match rs:
    case Nil{}:
      False{}
    case +r <> t:
      Bool.or(Fm.zip.rule(r, p), Fm.zip.any(t, p))

# whether a zip leaves an entry out (p: its path from the project's root)
def Fm.zip.skip(+p: String) -> Bool:
  Fm.zip.any(Fm.zip.rules(), String.to_lower(p))

def Fm.zip.kind(+r: String) -> Bool:
  Bool.or(Bool.or(String.starts_with(r, "="), String.starts_with(r, "*")), Bool.or(String.starts_with(r, "^"), String.starts_with(r, "/")))

# whether every rule is of a kind the writer reads (law fm_zip_rules_known)
def Fm.zip.known(rs: List<&2, String>) -> Bool:
  match rs:
    case Nil{}:
      True{}
    case +r <> t:
      Bool.and(Fm.zip.kind(r), Fm.zip.known(t))

def Fm.zip.within.go(ps: List<&2, String>, +at: String) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case +p <> t:
      Bool.or(Fm.zip.skip(Fm.join(at, p)), Fm.zip.within.go(t, Fm.join(at, p)))

# whether a path is, or is inside, something a zip leaves out: such a
# folder is not zipped and such a file is not served (".backplane/history.git/HEAD")
def Fm.zip.within(+p: String) -> Bool:
  Fm.zip.within.go(String.split(p, '/'), "")

# what a zip keeps as it is, not deflated: files already compressed
def Fm.zip.stored() -> List<&2, String>:
  [".zip", ".gz", ".tgz", ".xz", ".bz2", ".zst", ".7z", ".rar", ".jar", ".png", ".jpg", ".jpeg", ".gif", ".webp", ".pdf", ".glb", ".mp4", ".mov", ".docx", ".xlsx", ".pptx"]

def Fm.zip.store(+name: String) -> Bool:
  Fm.ends(Fm.zip.stored(), Fm.ext(name))

# the characters an entry's name may not hold in a zip, each made "_": a
# backslash, which Windows' unpackers read as a folder ("..\..\evil.bat").
# (The hub's writer leaves a file named with one out, and says so in the
# note, outside Windows: its runtime reads the name as a path.)
def Fm.zip.swap() -> String:
  "\\"

def Fm.zip.entry.go(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      SCon{Chr{Bool.pick(U32, U32.is_eq(c, 92), 95, c)}, Fm.zip.entry.go(t)}

# one name as a zip holds it
def Fm.zip.entry(+name: String) -> String:
  Fm.zip.entry.go(name)

# the note at the top of a zip when something could not be put in it, and
# what it says: a line, then each path with why ("<code>=<words>", "*" for
# any other code)
def Fm.zip.note() -> String:
  "SKIPPED.txt"

def Fm.zip.note.head() -> String:
  "Backplane could not put these in the zip:\n"

def Fm.zip.why() -> List<&2, String>:
  ["EACCES=no permission to read it", "EPERM=no permission to read it", "ENOENT=it went away while the zip was made", "ELOOP=it changed into a link while the zip was made", "short=it got shorter while the zip was made", "backslash=its name holds a backslash, which the hub does not read", "*=it could not be read"]

def Fm.zip.char(+c: U32) -> U32:
  Bool.pick(U32, Bool.or(Bool.or(U32.is_lt(c, 32), Bool.or(U32.is_eq(c, 47), Bool.or(U32.is_eq(c, 92), U32.is_eq(c, 58)))),
    Bool.or(Bool.or(U32.is_eq(c, 63), U32.is_eq(c, 42)), Bool.or(Bool.or(U32.is_eq(c, 60), U32.is_eq(c, 62)), Bool.or(U32.is_eq(c, 124), U32.is_eq(c, 34))))), 95, c)

def Fm.zip.clean.go(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      SCon{Chr{Fm.zip.char(c)}, Fm.zip.clean.go(t)}

def Fm.zip.clean.k(+n: String) -> String:
  Bool.pick(String, Bool.or(String.is_empty(n), Bool.or(String.eq(n, "."), String.eq(n, ".."))), "project", n)

# the folder a zip holds everything in: the folder's name, or the
# project's title for its root; one name, never "." or "..", nothing a
# file system refuses in a name (/ \ : ? * < > | " and controls are "_")
def Fm.zip.top(+title: String, +dir: String) -> String:
  Fm.zip.clean.k(Fm.zip.clean.go(Bool.pick(String, String.is_empty(dir), title, Fm.last(dir))))

# the name a folder downloads as
def Fm.zip.name(+title: String, +dir: String) -> String:
  Fm.zip.top(title, dir) ++ ".zip"

def Fm.drive(+rel: String) -> Bool:
  String.eq(String.drop(String.take(rel, 2n), 1n), ":")

# a path a download may name: Fm.safe, and nothing Windows would read as
# another place (a backslash, a drive "C:", a share "//host" or "\\host")
def Fm.files.safe(+rel: String) -> Bool:
  Bool.and(Fm.safe(rel), Bool.and(Bool.not(String.contains(rel, "\\")), Bool.not(Fm.drive(rel))))

def Fm.hex(+n: U32) -> Char:
  Chr{U32.add(n, Bool.pick(U32, U32.is_lt(n, 10), 48, 55))}

# a byte kept as it is: letters, digits, - . _ ~ (and / where slash)
def Fm.plain(+slash: Bool, +c: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90))),
    Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), Bool.or(Bool.or(U32.is_eq(c, 45), U32.is_eq(c, 46)), Bool.or(Bool.or(U32.is_eq(c, 95), U32.is_eq(c, 126)), Bool.and(slash, U32.is_eq(c, 47))))))

def Fm.pct.k(ok: Bool, +c: U32, rest: String) -> String:
  match ok:
    case True{}:
      SCon{Chr{c}, rest}
    case False{}:
      SCon{'%', SCon{Fm.hex(U32.shrn(c, 4n)), SCon{Fm.hex(U32.and(c, 15)), rest}}}

def Fm.pct.go(bs: List<&2, U32>, +slash: Bool) -> String:
  match bs:
    case Nil{}:
      SNil{}
    case +b <> t:
      Fm.pct.k(Fm.plain(slash, b), b, Fm.pct.go(t, slash))

# a text percent-encoded as UTF-8 (a path keeps its slashes)
def Fm.pct(+slash: Bool, +s: String) -> String:
  Fm.pct.go(U8.Utf8.encode(s), slash)

# where a folder of a thread's project downloads as a zip
def Fm.zip.url(+thread: String, +dir: String) -> String:
  "/files/zip?thread=" ++ Fm.pct(False{}, thread) ++ "&path=" ++ Fm.pct(True{}, dir)

# where one of its files downloads
def Fm.get.url(+thread: String, +path: String) -> String:
  "/files/get?thread=" ++ Fm.pct(False{}, thread) ++ "&path=" ++ Fm.pct(True{}, path)

def Fm.ascii.c(+c: U32) -> U32:
  Bool.pick(U32, Bool.or(Bool.or(U32.is_lt(c, 32), U32.is_gt(c, 126)), Bool.or(U32.is_eq(c, 34), Bool.or(U32.is_eq(c, 92), U32.is_eq(c, 37)))), 95, c)

def Fm.ascii.go(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      SCon{Chr{Fm.ascii.c(c)}, Fm.ascii.go(t)}

# the Content-Disposition a download goes with: the name as ASCII for old
# browsers (anything else, quotes and controls made "_"), and exactly as
# UTF-8 (RFC 5987); one header line whatever the name holds
def Fm.disposition(+name: String) -> String:
  "attachment; filename=\"" ++ Fm.ascii.go(name) ++ "\"; filename*=UTF-8''" ++ Fm.pct(False{}, name)
