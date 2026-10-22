import Base

# Pseudo-terminals for the integrated terminal. pty.c (native) and pty.js
# (Bun, posix_spawn through bun:ffi) implement them; see docs/terminal.md.
#
# The master side comes back as a Socket, so host.bend's Sock.recv_bytes
# and Sock.send_bytes read and write it (a closed child reads as []), and
# Socket.close hangs it up. Proc.wait / Proc.kill take the pid.

# start cmd (searched on PATH) with args in cwd ("" keeps ours) on a new
# cols x rows pty, as a session leader with the pty as its controlling
# terminal and TERM=xterm-256color. Answers (pid, master).
def Pty.spawn(cmd: String, args: List<&2, String>, cwd: String, cols: U32, rows: U32) ->
  IO(Result<&1, &1, U32 & String, U32 & Socket>):
  import "./effects/pty.js"

# a new window size; the child gets SIGWINCH
def Pty.resize(s: Socket, cols: U32, rows: U32) -> IO(Socket):
  import "./effects/pty.js"
