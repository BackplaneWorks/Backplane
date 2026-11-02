import Base
import ./port.bend as PT
import ./semver.bend as SV

# Where the provider CLIs are found. A hub started from a launcher or a
# systemd user service has a PATH without the login shell's additions
# (nvm, volta, ~/.local/bin...), so `command -v codex` fails and the agent
# cannot be spawned. The server runs `CliPath.probe.sh()`, parses it, and
# sets PATH to `CliPath.merge`: the hub's own entries first, then the login
# shell's, then the usual install directories that exist. Pure; the
# server does the IO (laws clipath_*, test/clipath_test.bend).

def CliPath.has(xs: List<&2, String>, +s: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.pick(Bool, String.eq(h, s), True{}, CliPath.has(t, s))

# a kept entry goes onto the accumulator (reversed) unless empty or already there
def CliPath.push(+x: String, +seen: List<&2, String>) -> List<&2, String>:
  Bool.pick(List<&2, String>, Bool.or(String.is_empty(x), CliPath.has(seen, x)), seen, x <> seen)

# each entry once, the first of each, in order, no empty ones
def CliPath.uniq.go(xs: List<&2, String>, +seen: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      List.reverse(&2, String, seen)
    case +x <> t:
      CliPath.uniq.go(t, CliPath.push(x, seen))

# a PATH string's entries, empty ones dropped
def CliPath.split(+path: String) -> List<&2, String>:
  CliPath.uniq.go(String.split(path, ':'), Nil{})

def CliPath.cat(a: List<&2, String>, b: List<&2, String>) -> List<&2, String>:
  List.append(&2, String, a, b)

# the new PATH: the current entries first in order, then the login shell's
# not there yet, then the extra directories not there yet; ':'-joined
def CliPath.merge.sep(+sep: Char, +cur: String, +login: String, extra: List<&2, String>) -> String:
  String.join(CliPath.uniq.go(CliPath.cat(CliPath.cat(String.split(cur, sep), String.split(login, sep)), extra), Nil{}), SCon{sep, ""})

def CliPath.merge(+cur: String, +login: String, extra: List<&2, String>) -> String:
  CliPath.merge.sep(':', cur, login, extra)

# nvm's node versions: "<home>/.nvm/versions/node/v24.13.1/bin"
def CliPath.nvm.root(+home: String) -> String:
  home ++ "/.nvm/versions/node/"

def CliPath.digits(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      CliPath.digits(t, Bool.pick(Nat, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32))), acc))

def CliPath.ver.of(parts: List<&2, String>) -> SV.Ver:
  match parts:
    case a <> b <> c <> _:
      SV.Ver{CliPath.digits(a, 0n), CliPath.digits(b, 0n), CliPath.digits(c, 0n), False{}, 0n, 0n}
    case a <> b <> Nil{}:
      SV.Ver{CliPath.digits(a, 0n), CliPath.digits(b, 0n), 0n, False{}, 0n, 0n}
    case a <> Nil{}:
      SV.Ver{CliPath.digits(a, 0n), 0n, 0n, False{}, 0n, 0n}
    case Nil{}:
      SV.Ver{0n, 0n, 0n, False{}, 0n, 0n}

# the version a node bin directory names ("v24.13.1" under the nvm root)
def CliPath.nvm.ver(+dir: String, +home: String) -> SV.Ver:
  CliPath.ver.of(String.split(SV.Ver.strip(Maybe.default(&2, String, List.head(&2, String, String.split(String.drop(dir, String.length(CliPath.nvm.root(home))), '/')), "")), '.'))

def CliPath.nvm.is(+dir: String, +home: String) -> Bool:
  Bool.and(String.starts_with(dir, CliPath.nvm.root(home)), String.ends_with(dir, "/bin"))

# newest version first; equal versions keep their order
def CliPath.ins(+x: String, sorted: List<&2, String>, +home: String) -> List<&2, String>:
  match sorted:
    case Nil{}:
      x <> Nil{}
    case +h <> +t:
      Bool.pick(List<&2, String>, Cmp.is_ge(SV.Ver.cmp(CliPath.nvm.ver(x, home), CliPath.nvm.ver(h, home))), x <> (h <> t), h <> CliPath.ins(x, t, home))

def CliPath.nvm.sort(xs: List<&2, String>, +acc: List<&2, String>, +home: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      acc
    case +x <> t:
      CliPath.nvm.sort(t, CliPath.ins(x, acc, home), home)

def CliPath.nvm.dirs(xs: List<&2, String>, +acc: List<&2, String>, +home: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      acc
    case +x <> t:
      CliPath.nvm.dirs(t, Bool.pick(List<&2, String>, CliPath.nvm.is(x, home), x <> acc, acc), home)

# the fixed places, after nvm's, in priority order (the first ones under home)
def CliPath.fixed(+home: String) -> List<&2, String>:
  (home ++ "/.volta/bin") <> (home ++ "/.local/share/fnm/aliases/default/bin") <> (home ++ "/.fnm/aliases/default/bin")
    <> (home ++ "/.bun/bin") <> (home ++ "/.local/bin") <> (home ++ "/.npm-global/bin") <> (home ++ "/.cargo/bin")
    <> (home ++ "/.grok/bin") <> "/opt/homebrew/bin" <> "/usr/local/bin" <> Nil{}

def CliPath.keep(c: Bool, +x: String, +rest: List<&2, String>) -> List<&2, String>:
  match c:
    case True{}:
      x <> rest
    case False{}:
      rest

def CliPath.only(xs: List<&2, String>, +listed: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      CliPath.keep(CliPath.has(listed, x), x, CliPath.only(t, listed))

# the directories that may hold a CLI, best first: nvm's node bins newest
# version first, then the fixed places; only those `listed` (that exist)
def CliPath.candidates(+home: String, +listed: List<&2, String>) -> List<&2, String>:
  CliPath.cat(CliPath.nvm.sort(CliPath.nvm.dirs(listed, Nil{}, home), Nil{}, home), CliPath.only(CliPath.fixed(home), listed))

# sh that prints the login shell's PATH (`login<TAB>PATH`, empty when it
# fails; an interactive rc file's chatter is cut away by the marker) and
# `dir<TAB>path` for each candidate that exists. Cygwin additionally reports
# native Windows paths: the Bun hub needs ';' and drive letters, whereas
# an older Cygwin hub still consumes the original POSIX rows.
def CliPath.probe.sh() -> String:
  PT.Port.tmo()
    ++ "native=; case $(uname -s) in CYGWIN*|MSYS*|MINGW*) native=1 ;; esac; "
    ++ "l=$(tmo 5 \"${SHELL:-/bin/sh}\" -lic 'printf \"\\n__BP_PATH__%s\\n\" \"$PATH\"' </dev/null 2>/dev/null | sed -n 's/^__BP_PATH__//p' | tail -n 1); "
    ++ "printf 'login\\t%s\\n' \"$l\"; "
    ++ "if [ -n \"$native\" ]; then h=$(cygpath -m \"$HOME\") && printf 'winhome\\t%s\\n' \"$h\"; "
    ++ "if [ -n \"$l\" ]; then w=$(cygpath -mp \"$l\") && printf 'winlogin\\t%s\\n' \"$w\"; fi; fi; "
    ++ "for d in \"$HOME\"/.nvm/versions/node/*/bin \"$HOME/.volta/bin\" \"$HOME/.local/share/fnm/aliases/default/bin\" \"$HOME/.fnm/aliases/default/bin\" "
    ++ "\"$FNM_MULTISHELL_PATH/bin\" \"$HOME/.bun/bin\" \"$HOME/.local/bin\" \"$HOME/.npm-global/bin\" \"$HOME/.cargo/bin\" \"$HOME/.grok/bin\" "
    ++ "/opt/homebrew/bin /usr/local/bin; do if [ -d \"$d\" ]; then printf 'dir\\t%s\\n' \"$d\"; "
    ++ "if [ -n \"$native\" ]; then w=$(cygpath -m \"$d\") && printf 'windir\\t%s\\n' \"$w\"; fi; fi; done; true"

type CliProbe is Data:
  CliProbe{login: String, dirs: List<&2, String>}

def CliPath.parse.go(ls: List<&2, String>, +login: String, +dirs: List<&2, String>) -> CliProbe:
  match ls:
    case Nil{}:
      CliProbe{login, List.reverse(&2, String, dirs)}
    case +l <> t:
      CliPath.parse.go(t,
        Bool.pick(String, String.starts_with(l, "login\t"), String.drop(l, 6n), login),
        Bool.pick(List<&2, String>, String.starts_with(l, "dir\t"), String.drop(l, 4n) <> dirs, dirs))

def CliPath.parse(+out: String) -> CliProbe:
  CliPath.parse.go(String.lines(out), "", Nil{})

def CliPath.next.k(+cur: String, +home: String, p: CliProbe) -> String:
  match p:
    case CliProbe{login, dirs}:
      CliPath.merge(cur, login, CliPath.candidates(home, dirs))

def CliPath.native(+s: String) -> Bool:
  Bool.or(String.starts_with(s, "\\\\"), String.ends_with(String.take(s, 2n), ":"))

# An absent/failed native conversion must leave the hub's PATH intact.
def CliPath.win.finish(missing: Bool, +cur: String, +home: String, login: String, dirs: List<&2, String>) -> String:
  match missing:
    case True{}:
      cur
    case False{}:
      CliPath.merge.sep(';', cur, login, CliPath.candidates(home, List.reverse(&2, String, dirs)))

def CliPath.win.rows(ls: List<&2, String>, +cur: String, +home: String, +login: String, +dirs: List<&2, String>) -> String:
  match ls:
    case Nil{}:
      CliPath.win.finish(String.is_empty(home), cur, home, login, dirs)
    case +l <> t:
      CliPath.win.rows(t, cur,
        Bool.pick(String, String.starts_with(l, "winhome\t"), String.drop(l, 8n), home),
        Bool.pick(String, String.starts_with(l, "winlogin\t"), String.drop(l, 9n), login),
        Bool.pick(List<&2, String>, String.starts_with(l, "windir\t"), String.drop(l, 7n) <> dirs, dirs))

def CliPath.next.platform(native: Bool, +cur: String, home: String, +out: String) -> String:
  match native:
    case True{}:
      CliPath.win.rows(String.lines(out), cur, "", "", Nil{})
    case False{}:
      CliPath.next.k(cur, home, CliPath.parse(out))

# The native hub's HOME/PATH are Windows paths; a Cygwin hub's are POSIX.
def CliPath.next(+cur: String, +home: String, +out: String) -> String:
  CliPath.next.platform(Bool.or(CliPath.native(home), CliPath.native(cur)), cur, home, out)
