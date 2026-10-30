import Base

# Nonblocking POSIX flock guard. A captured child holds each granted flock;
# the pipe closes on hub death; owned work retains exclusion until it ends.
def Guard.take(name: String, holder: String, home: String, script: String) -> IO(Bool):
  import "./effects/locks.js"

def Guard.drop(name: String, holder: String) -> IO(Unit):
  import "./effects/locks.js"

# Keep the flock until a detached owned job stops, even on hub SIGKILL.
# PID and start identity come from the existing work runner's own files.
def Guard.cleanup() -> String:
  "pid_id() {\n q=$1\n if [ -r \"/proc/$q/stat\" ]; then\n  b=$(cat /proc/sys/kernel/random/boot_id 2>/dev/null)\n  rest=$(sed 's/^.*) //' \"/proc/$q/stat\")\n  case $rest in Z\\ *|X\\ *) return ;; esac\n  st=$(printf '%s\\n' \"$rest\" | cut -d' ' -f20)\n  echo \"$b:$st\"\n elif ps -p \"$q\" >/dev/null 2>&1; then\n  ps -o lstart= -p \"$q\" | tr -s ' ' '_'\n fi\n}\nalive() {\n p=$(cat \"$1.pid\" 2>/dev/null); saved=$(cat \"$1.id\" 2>/dev/null)\n [ -n \"$p\" ] && [ \"$p\" -gt 1 ] 2>/dev/null && [ -n \"$saved\" ] && [ \"$(pid_id \"$p\")\" = \"$saved\" ]\n}\ncase $2 in\n w-*)\n  d=$1/work/$2\n  # Retain exclusion through launch and hub death; never signal the job.\n  while [ -e \"$d/intent\" ] && { [ ! -s \"$d/pid\" ] || [ ! -s \"$d/pidid\" ]; } && [ ! -e \"$d/code\" ] && [ ! -e \"$d/cancel\" ]; do sleep 1; done\n  p=$(cat \"$d/pid\" 2>/dev/null); saved=$(cat \"$d/pidid\" 2>/dev/null)\n  [ -n \"$p\" ] && [ \"$p\" -gt 1 ] 2>/dev/null && [ -n \"$saved\" ] || exit 0\n  while [ \"$(pid_id \"$p\")\" = \"$saved\" ]; do sleep 1; done ;;\n pid:*:bp-lock-client.*)\n  d=/tmp/${2##*:}\n  # The helper records its command before exec; removed metadata means it\n  # fell back to acquiring this same native flock before starting work.\n  while [ -d \"$d\" ] && [ ! -s \"$d/cmd.id\" ] && { alive \"$d/owner\" || { [ -e \"$d/intent\" ] && [ ! -e \"$d/done\" ]; }; }; do sleep 1; done\n  while alive \"$d/cmd\"; do sleep 1; done ;;\nesac\nexit 0\n"

def Guard.script() -> String:
  "cat >/dev/null\n" ++ Guard.cleanup()

# Who holds the flock outside the hub: {"pid":N,"cmd":"..."} on stdout, or nothing.
# Linux reads /proc/locks for a FLOCK on the file's inode, macOS asks lsof.
# A lock the hub itself holds (the sh's parent) is not "outside".
def Guard.who() -> String:
  "f=${BACKPLANE_LOCK_DIR:-/tmp}/bp-$1.lock\n[ -e \"$f\" ] || exit 0\npid=\nif [ -r /proc/locks ]; then\n  ino=$(stat -c %i \"$f\" 2>/dev/null) || exit 0\n  pid=$(awk -v i=\"$ino\" -v me=\"$PPID\" '$2==\"FLOCK\" { n=split($6,a,\":\"); if (a[n]==i && $5!=me) { print $5; exit } }' /proc/locks)\n  [ -n \"$pid\" ] && cmd=$(tr '\\000' ' ' < \"/proc/$pid/cmdline\" 2>/dev/null | cut -c1-120)\nelif command -v lsof >/dev/null 2>&1; then\n  pid=$(lsof -t \"$f\" 2>/dev/null | grep -v \"^$PPID$\" | head -1)\n  [ -n \"$pid\" ] && cmd=$(ps -o command= -p \"$pid\" 2>/dev/null | cut -c1-120)\nfi\n[ -n \"$pid\" ] || exit 0\ncmd=$(printf '%s' \"$cmd\" | tr -d '\\000-\\037' | sed 's/\\\\/\\\\\\\\/g;s/\"/\\\\\"/g')\nprintf '{\"pid\":%s,\"cmd\":\"%s\"}' \"$pid\" \"$cmd\"\n"

# Reap ended guards without waiting for work they still protect.
def Guard.reap() -> IO(U32):
  import "./effects/locks.js"
