import Base

# Nonblocking POSIX flock guard. A captured child holds each granted flock;
# a pipe ties its lifetime to the hub, including a hub killed with SIGKILL.
def Guard.take(name: String, holder: String, home: String, script: String) -> IO(Bool):
  import "./effects/locks.js"

def Guard.drop(name: String, holder: String) -> IO(Unit):
  import "./effects/locks.js"

# Keep the flock until a detached owned job stops, even on hub SIGKILL.
# PID and start identity come from the existing work runner's own files.
def Guard.cleanup() -> String:
  "d=$1/work/$2\ncase $2 in w-*) ;; *) exit 0 ;; esac\n# A committed job may still be in its launch fiber when the hub dies.\nif [ -e \"$d/intent\" ]; then\n  : > \"$d/cancel\"\n  for attempt in 1 2; do\n    [ -s \"$d/pid\" ] && [ -s \"$d/pidid\" ] && break\n    sleep 1\n  done\nfi\npid_id() {\n  q=$1\n  if [ -r \"/proc/$q/stat\" ]; then\n    b=$(cat /proc/sys/kernel/random/boot_id 2>/dev/null)\n    st=$(sed 's/^.*) //' \"/proc/$q/stat\" | cut -d' ' -f20)\n    echo \"$b:$st\"\n  elif ps -p \"$q\" >/dev/null 2>&1; then\n    ps -o lstart= -p \"$q\" | tr -s ' ' '_'\n  fi\n}\np=$(cat \"$d/pid\" 2>/dev/null)\nsaved=$(cat \"$d/pidid\" 2>/dev/null)\n[ -n \"$p\" ] && [ \"$p\" -gt 1 ] 2>/dev/null && [ -n \"$saved\" ] && [ \"$(pid_id \"$p\")\" = \"$saved\" ] || exit 0\nkill -TERM \"-$p\" 2>/dev/null || :\nkill -TERM \"$p\" 2>/dev/null || :\nfor attempt in 1 2; do\n  if kill -0 \"-$p\" 2>/dev/null || kill -0 \"$p\" 2>/dev/null; then sleep 1; else exit 0; fi\ndone\nkill -KILL \"-$p\" 2>/dev/null || :\n[ \"$(pid_id \"$p\")\" != \"$saved\" ] || kill -KILL \"$p\" 2>/dev/null || :\nfor attempt in 1 2; do\n  if kill -0 \"-$p\" 2>/dev/null || kill -0 \"$p\" 2>/dev/null; then sleep 1; else break; fi\ndone\nexit 0\n"

def Guard.script() -> String:
  "cat >/dev/null\n" ++ Guard.cleanup()

# Who holds the flock outside the hub: {"pid":N,"cmd":"..."} on stdout, or nothing.
# Linux reads /proc/locks for a FLOCK on the file's inode, macOS asks lsof.
# A lock the hub itself holds (the sh's parent) is not "outside".
def Guard.who() -> String:
  "f=${BACKPLANE_LOCK_DIR:-/tmp}/bp-$1.lock\n[ -e \"$f\" ] || exit 0\npid=\nif [ -r /proc/locks ]; then\n  ino=$(stat -c %i \"$f\" 2>/dev/null) || exit 0\n  pid=$(awk -v i=\"$ino\" -v me=\"$PPID\" '$2==\"FLOCK\" { n=split($6,a,\":\"); if (a[n]==i && $5!=me) { print $5; exit } }' /proc/locks)\n  [ -n \"$pid\" ] && cmd=$(tr '\\000' ' ' < \"/proc/$pid/cmdline\" 2>/dev/null | cut -c1-120)\nelif command -v lsof >/dev/null 2>&1; then\n  pid=$(lsof -t \"$f\" 2>/dev/null | grep -v \"^$PPID$\" | head -1)\n  [ -n \"$pid\" ] && cmd=$(ps -o command= -p \"$pid\" 2>/dev/null | cut -c1-120)\nfi\n[ -n \"$pid\" ] || exit 0\ncmd=$(printf '%s' \"$cmd\" | tr -d '\\000-\\037' | sed 's/\\\\/\\\\\\\\/g;s/\"/\\\\\"/g')\nprintf '{\"pid\":%s,\"cmd\":\"%s\"}' \"$pid\" \"$cmd\"\n"
