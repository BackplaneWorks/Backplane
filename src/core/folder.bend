import Base

# A bare drive prefix becomes drive-relative if its root slash is lost.
def Folder.drive(r: String) -> Bool:
  match r:
    case SCon{c, SCon{colon, SNil{}}}:
      Bool.and(Char.is_alpha(c), Char.is_eq(colon, ':'))
    case _:
      False{}

# Native absolute folders only. Empty/relative roots must never inherit cwd.
def Folder.absolute(+p: String) -> Bool:
  Bool.and(Bool.not(String.contains(p, SCon{Chr{0}, SNil{}})), Bool.or(String.starts_with(p, "/"),
    Bool.or(String.starts_with(p, "\\\\"), Bool.and(Folder.drive(String.take(p, 2n)),
      Bool.or(String.starts_with(String.drop(p, 2n), "/"), String.starts_with(String.drop(p, 2n), "\\"))))))

def Folder.root.fin(+r: String) -> String:
  Bool.pick(String, String.is_empty(r), "/", Bool.pick(String, Folder.drive(r), r ++ "/", r))

# Project picker filesystem scripts. Paths are arguments, never shell text.
# Cygwin resolves Windows inputs, but the Bun hub persists native paths.

def Folder.path.sh() -> String:
  "d=\"$1\"; if command -v cygpath >/dev/null 2>&1; then case \"$d\" in /[a-zA-Z]:[\\\\/]*) d=\"${d#/}\";; esac; case \"$d\" in [a-zA-Z]:[\\\\/]*|\\\\\\\\*|//*) d=$(cygpath -au -- \"$d\") || { echo \"cannot resolve folder: $1\"; exit 2; };; [a-zA-Z]:*) echo \"invalid drive path: $d (use an absolute folder)\"; exit 2;; esac; fi; case \"$d\" in ''|'~') d=\"$HOME\";; '~/'*) d=\"$HOME/${d#??}\";; /*) ;; [a-zA-Z]:*) echo \"cannot resolve Windows folder: $d\"; exit 2;; *) d=\"$HOME/$d\";; esac; "

def Folder.native.sh() -> String:
  "if command -v cygpath >/dev/null 2>&1; then d=$(cygpath -am -- \"$d\") || { echo \"cannot resolve folder: $d\"; exit 2; }; fi; "

def Folder.dir.sh() -> String:
  "[ -e \"$d\" ] || { echo \"no such folder: $d\"; exit 2; }; [ -d \"$d\" ] || { echo \"not a folder: $d\"; exit 2; }; [ -r \"$d\" ] && [ -x \"$d\" ] && d=$(cd -P -- \"$d\" && pwd -P) || { echo \"cannot open: $d\"; exit 2; }; "
