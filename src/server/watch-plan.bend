import Base

# Pure work-monitor commands. The copy gate is shared by launch, check, stop
# and intent; IO and the hub actor depend on this planner, never conversely.

def Watch.dir(+home: String, +id: String) -> String:
  home ++ "/work/" ++ id

# The shell each watch script runs as; none for a hub on a copied log
# (BACKPLANE_COPY=1): it never launches, re-launches or signals anything
# the log names, and every watch reads as gone (laws watch_copy_*)
def Watch.argv.k(copy: Bool, xs: List<&2, String>) -> List<&2, String>:
  match copy:
    case True{}:
      Nil{}
    case False{}:
      xs

def Watch.launch.sh() -> String:
  "d=$1; c=$2; cmd=$3\npid_id() { q=$1; if [ -r \"/proc/$q/stat\" ]; then b=$(cat /proc/sys/kernel/random/boot_id 2>/dev/null); st=$(sed 's/^.*) //' \"/proc/$q/stat\" | cut -d' ' -f20); echo \"$b:$st\"; elif ps -p \"$q\" >/dev/null 2>&1; then ps -o lstart= -p \"$q\" | tr -s ' ' '_'; fi; }\nmkdir -p \"$d\" || { echo gone; exit 0; }\n[ -e \"$d/cancel\" ] && { echo gone; exit 0; }\n[ -e \"$d/code\" ] && { echo running; exit 0; }\nif [ -e \"$d/started\" ]; then { [ -e \"$d/out\" ] || [ -e \"$d/how\" ]; } && { echo running; exit 0; }; fi\nprintf '%s\\n' \"$cmd\" > \"$d/cmd\"\n{ printf ': > \"%s/out\"\\n' \"$d\"; printf 'cd \"%s\" || exit 127\\n' \"$c\"; printf '[ -e \"%s/cancel\" ] && exit 143\\n' \"$d\"; printf 'sh \"%s/cmd\" > \"%s/out\" 2>&1\\n' \"$d\" \"$d\"; printf 'echo $? > \"%s/code.tmp\" && mv \"%s/code.tmp\" \"%s/code\"\\n' \"$d\" \"$d\" \"$d\"; } > \"$d/run.sh\"\n: > \"$d/started\"\nif command -v setsid >/dev/null 2>&1; then setsid nohup sh \"$d/run.sh\" >/dev/null 2>&1 < /dev/null &\nelse perl -MPOSIX -e 'POSIX::setsid(); exec {$ARGV[0]} @ARGV or exit 127' nohup sh \"$d/run.sh\" >/dev/null 2>&1 < /dev/null &\nfi\necho $! > \"$d/pid\"\npid_id $! > \"$d/pidid\"\necho pid > \"$d/how\"\necho running\n"

def Watch.check.sh() -> String:
  "k=$1; t=$2; id=$3; d=$4\ncase $k in\nrun)\n  [ -e \"$d/stop\" ] && { echo stop; exit 0; }\n  [ -e \"$d/code\" ] && { echo \"done $(cat \"$d/code\")\"; exit 0; }\n  p=$(cat \"$d/pid\" 2>/dev/null)\n  saved=$(cat \"$d/pidid\" 2>/dev/null)\n  if [ -r \"/proc/$p/stat\" ]; then b=$(cat /proc/sys/kernel/random/boot_id 2>/dev/null); st=$(sed 's/^.*) //' \"/proc/$p/stat\" | cut -d' ' -f20); actual=\"$b:$st\"; case $(sed 's/^.*) //' \"/proc/$p/stat\") in Z\\ *|X\\ *) actual= ;; esac\n  elif [ -n \"$p\" ] && ps -p \"$p\" >/dev/null 2>&1; then actual=$(ps -o lstart= -p \"$p\" | tr -s ' ' '_')\n  else actual=; fi\n  [ -n \"$saved\" ] && [ \"$saved\" = \"$actual\" ] && { echo running; exit 0; }\n  [ -e \"$d/code\" ] && { echo \"done $(cat \"$d/code\")\"; exit 0; }\n  echo gone ;;\npid)\n  [ -e \"$d/stop\" ] && { echo stop; exit 0; }\n  if [ -r \"/proc/$t/stat\" ]; then b=$(cat /proc/sys/kernel/random/boot_id 2>/dev/null); st=$(sed 's/^.*) //' \"/proc/$t/stat\" | cut -d' ' -f20); n=\"$b:$st\"; case $(sed 's/^.*) //' \"/proc/$t/stat\") in Z\\ *|X\\ *) echo gone; exit 0 ;; esac\n  elif ps -p \"$t\" >/dev/null 2>&1; then n=$(ps -o lstart= -p \"$t\" | tr -s ' ' '_')\n  else echo gone; exit 0; fi\n  [ \"$id\" = - ] && { echo \"ident $n\"; echo running; exit 0; }\n  [ \"$n\" = \"$id\" ] && echo running || echo gone ;;\nfile)\n  [ -e \"$d/stop\" ] && { echo stop; exit 0; }\n  [ -f \"$t\" ] && grep -qF \"$id\" \"$t\" 2>/dev/null && { echo found; exit 0; }\n  echo running ;;\n*) echo gone ;;\nesac\n"

def Watch.stop.sh() -> String:
  "pid_id() { q=$1; if [ -r \"/proc/$q/stat\" ]; then b=$(cat /proc/sys/kernel/random/boot_id 2>/dev/null); st=$(sed 's/^.*) //' \"/proc/$q/stat\" | cut -d' ' -f20); echo \"$b:$st\"; elif ps -p \"$q\" >/dev/null 2>&1; then ps -o lstart= -p \"$q\" | tr -s ' ' '_'; fi; }\nk=$1; t=$2; id=$3; d=$4\nmkdir -p \"$d\"\ncase $k in\nunwatch) : > \"$d/stop\" ;;\nrun) : > \"$d/cancel\"; p=$(cat \"$d/pid\" 2>/dev/null); n=$(pid_id \"$p\"); [ -n \"$p\" ] && [ \"$p\" -gt 1 ] && [ -n \"$n\" ] && [ \"$n\" = \"$(cat \"$d/pidid\" 2>/dev/null)\" ] && { kill -TERM \"-$p\" 2>/dev/null; kill -TERM \"$p\" 2>/dev/null; } ;;\npid) [ \"$t\" -gt 1 ] 2>/dev/null && [ \"$id\" != - ] && [ \"$(pid_id \"$t\")\" = \"$id\" ] && kill -TERM \"$t\" 2>/dev/null ;;\nesac\ntrue\n"

def Watch.tail.sh() -> String:
  "d=$1; [ -e \"$d/out\" ] && tail -c 2048 \"$d/out\"; true\n"

def Watch.launch.argv(copy: Bool, +home: String, +id: String, +cwd: String, +target: String) -> List<&2, String>:
  Watch.argv.k(copy, ["-c", Watch.launch.sh(), "sh", Watch.dir(home, id), cwd, target])

def Watch.check.argv(copy: Bool, +kind: String, +target: String, +ident: String, +home: String, +id: String) -> List<&2, String>:
  Watch.argv.k(copy, ["-c", Watch.check.sh(), "sh", kind, target, ident, Watch.dir(home, id)])

def Watch.stop.argv(copy: Bool, +kind: String, +target: String, +ident: String, +home: String, +id: String) -> List<&2, String>:
  Watch.argv.k(copy, ["-c", Watch.stop.sh(), "sh", kind, target, ident, Watch.dir(home, id)])

def Watch.intent.argv(copy: Bool, +home: String, +id: String) -> List<&2, String>:
  Watch.argv.k(copy, ["-c", "mkdir -p \"$1\" && : > \"$1/intent\"", "sh", Watch.dir(home, id)])
