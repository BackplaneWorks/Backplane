import Base
import ./check.bend as C
import ../src/core/clipath.bend as CP

def Show.list(xs: List<&2, String>) -> String:
  String.join(xs, "|")

def Show.probe(p: CP.CliProbe) -> String:
  match p:
    case CP.CliProbe{l, ds}:
      l ++ "#" ++ Show.list(ds)

def probe() -> String:
  CP.CliPath.probe.sh()

def next(cur: String, home: String, out: String) -> String:
  CP.CliPath.next(cur, home, out)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("split drops empties", Show.list(CP.CliPath.split("/usr/bin::/bin:")), "/usr/bin|/bin")
    C.Check.str("split empty", Show.list(CP.CliPath.split("")), "")
    C.Check.str("merge keeps current first", CP.CliPath.merge("/usr/bin:/bin", "/a/x:/bin:/b", ["/c"]), "/usr/bin:/bin:/a/x:/b:/c")
    C.Check.str("merge no dupes", CP.CliPath.merge("/a", "/b:/a:/b", ["/c", "/b", "/c"]), "/a:/b:/c")
    C.Check.str("merge empty login", CP.CliPath.merge("/usr/bin", "", ["/opt/homebrew/bin"]), "/usr/bin:/opt/homebrew/bin")
    C.Check.str("merge empty current", CP.CliPath.merge("", "/a", Nil{}), "/a")
    C.Check.str("merge idempotent", CP.CliPath.merge(CP.CliPath.merge("/usr/bin", "/l:/usr/bin", ["/x"]), "/l:/usr/bin", ["/x"]), CP.CliPath.merge("/usr/bin", "/l:/usr/bin", ["/x"]))
    C.Check.str("candidates nvm newest first",
      Show.list(CP.CliPath.candidates("/home/h", ["/home/h/.nvm/versions/node/v9.11.2/bin", "/home/h/.nvm/versions/node/v24.13.1/bin", "/home/h/.nvm/versions/node/v18.0.0/bin"])),
      "/home/h/.nvm/versions/node/v24.13.1/bin|/home/h/.nvm/versions/node/v18.0.0/bin|/home/h/.nvm/versions/node/v9.11.2/bin")
    C.Check.str("candidates order and existing only",
      Show.list(CP.CliPath.candidates("/home/h", ["/usr/local/bin", "/home/h/.cargo/bin", "/home/h/.local/bin", "/home/h/.nvm/versions/node/v20.1.0/bin", "/home/h/.volta/bin"])),
      "/home/h/.nvm/versions/node/v20.1.0/bin|/home/h/.volta/bin|/home/h/.local/bin|/home/h/.cargo/bin|/usr/local/bin")
    C.Check.str("candidates macos", Show.list(CP.CliPath.candidates("/Users/k", ["/opt/homebrew/bin", "/usr/local/bin", "/Users/k/.bun/bin", "/elsewhere"])), "/Users/k/.bun/bin|/opt/homebrew/bin|/usr/local/bin")
    C.Check.str("candidates nvm minor and patch order", Show.list(CP.CliPath.candidates("/h", ["/h/.nvm/versions/node/v24.9.9/bin", "/h/.nvm/versions/node/v24.13.0/bin", "/h/.nvm/versions/node/v24.13.1/bin"])), "/h/.nvm/versions/node/v24.13.1/bin|/h/.nvm/versions/node/v24.13.0/bin|/h/.nvm/versions/node/v24.9.9/bin")
    C.Check.str("candidates none", Show.list(CP.CliPath.candidates("/home/h", Nil{})), "")
    C.Check.str("candidates other home's nvm ignored", Show.list(CP.CliPath.candidates("/home/h", ["/home/z/.nvm/versions/node/v20.1.0/bin"])), "")
    C.Check.str("parse", Show.probe(CP.CliPath.parse("login\t/a:/b\ndir\t/x\ndir\t/y\n")), "/a:/b#/x|/y")
    C.Check.str("parse empty login", Show.probe(CP.CliPath.parse("login\t\ndir\t/x\n")), "#/x")
    C.Check.str("parse junk lines", Show.probe(CP.CliPath.parse("motd junk\nlogin\t/a\nstray\ndir\t/x\n")), "/a#/x")
    C.Check.str("parse nothing", Show.probe(CP.CliPath.parse("")), "#")
    C.Check.str("next", CP.CliPath.next("/usr/bin:/bin", "/home/h", "login\t/l:/bin\ndir\t/home/h/.nvm/versions/node/v24.1.0/bin\ndir\t/usr/local/bin\n"), "/usr/bin:/bin:/l:/home/h/.nvm/versions/node/v24.1.0/bin:/usr/local/bin")
    C.Check.str("probe has marker and tmo", Bool.show(Bool.and(String.contains(CP.CliPath.probe.sh(), "__BP_PATH__"), String.contains(CP.CliPath.probe.sh(), "tmo 5"))), "True")
    C.Check.str("native Windows merge keeps drive colons and semicolon separators",
      CP.CliPath.next("C:\\Runtime\\bin;C:\\Windows\\System32", "C:\\Users\\Person 日本",
        "login\t/usr/bin:/bin\ndir\t/cygdrive/c/Users/Person 日本/.local/bin\nwinhome\tC:/Users/Person 日本\nwinlogin\tC:/Runtime/bin;D:/Tools\nwindir\tC:/Users/Person 日本/.local/bin\n"),
      "C:\\Runtime\\bin;C:\\Windows\\System32;C:/Runtime/bin;D:/Tools;C:/Users/Person 日本/.local/bin")
    C.Check.str("native Windows missing probe preserves PATH",
      CP.CliPath.next("C:\\Runtime\\bin", "C:\\Users\\Person", "login\t/usr/bin\ndir\t/cygdrive/c/Users/Person/.local/bin\n"), "C:\\Runtime\\bin")
    C.Check.str("native Windows explicit HOME uses converted probe home",
      CP.CliPath.next("C:/Runtime/bin", "D:\\Custom home", "winhome\tD:/Custom home\nwindir\tD:/Custom home/.local/bin\nwindir\tC:/Users/Other/.local/bin\n"),
      "C:/Runtime/bin;D:/Custom home/.local/bin")
    C.Check.str("native Windows UNC home and empty PATH",
      CP.CliPath.next("", "\\\\server\\share\\Person", "winhome\t//server/share/Person\nwindir\t//server/share/Person/.local/bin\n"), "//server/share/Person/.local/bin")
    C.Check.str("Cygwin keeps consuming POSIX probe rows",
      CP.CliPath.next("/usr/bin", "/cygdrive/c/Users/Person", "login\t/usr/bin\ndir\t/cygdrive/c/Users/Person/.local/bin\nwinhome\tC:/Users/Person\nwindir\tC:/Users/Person/.local/bin\n"),
      "/usr/bin:/cygdrive/c/Users/Person/.local/bin")
    C.Check.str("native Windows refresh is idempotent",
      CP.CliPath.next("C:/Runtime/bin;C:/Users/P/.local/bin", "C:/Users/P", "winhome\tC:/Users/P\nwinlogin\tC:/Runtime/bin\nwindir\tC:/Users/P/.local/bin\n"),
      "C:/Runtime/bin;C:/Users/P/.local/bin")
