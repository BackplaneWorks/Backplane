import Base

# Host effects Base does not have. host.c / host.js implement them.


def Clock.epoch() -> IO(U32):
  import "./effects/host.c"
  import "./effects/host.js"

# The event log read in one go: first the text that closes off a torn
# tail ("\n" when the last byte is not one, else ""), then its whole
# lines; exactly core/store.bend's Store.read.of over the file's bytes
# (test/evlog_read_test.bend), without a List cell per byte (252M of them
# on a real log, the JS hub's 25 GB). A file that cannot be read is [""].
def EvLog.read(path: String) -> IO(List<&2, String>):
  import "./effects/evlog.c"
  import "./effects/evlog.js"

# up to max bytes as they arrive; [] means the peer closed
def Sock.recv_bytes(sock: Socket, max: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, List<&2, U32>>):
  import "./effects/host.c"
  import "./effects/host.js"

# the next n bytes of the socket appended to the file at path, straight
# to disk (an upload's body); fails when the peer closes before n bytes
def Sock.save(sock: Socket, path: String, n: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effects/host.c"
  import "./effects/host.js"

def Sock.send_bytes(sock: Socket, data: List<&2, U32>) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effects/host.c"
  import "./effects/host.js"

# send_bytes with one deadline (ms) for all of it: a peer that stops
# reading fails it (ETIMEDOUT) and the socket is shut both ways
def Sock.send_until(sock: Socket, data: List<&2, U32>, ms: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effects/host.c"
  import "./effects/host.js"

# A message's JSON text as one binary WebSocket frame of its CBOR, sent with
# a deadline as send_until: the same bytes as core/http.bend's Ws.cbor, made
# in C straight from the text (Bend's encoder parsed it into a tree and built
# a list cell per byte, and a long log's join went out at ~1.5 MB/s). keys
# and words are Cbor.keys() and Cbor.words(), each joined by newlines.
def Sock.send_cbor(sock: Socket, text: String, keys: String, words: String, ms: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effects/host.c"
  import "./effects/host.js"

# the frame send_cbor sends, as bytes (for tests)
def Json.cbor_frame(text: String, keys: String, words: String) -> IO(List<&2, U32>):
  import "./effects/host.c"
  import "./effects/host.js"

# A join frame's lines (core/outbox.bend's Items.texts.s: hd, plain or
# split, the lines, tl; then "changes" frames, the last one's "boot" false
# when ends is 1; at most `most` lines and `bytes` characters a frame) as
# the binary WebSocket frames of their CBOR, sent with a deadline as
# send_cbor. The lines are measured, joined and encoded in C: Bend measured
# each line three times and joined them a character at a time per frame.
def Sock.send_items(sock: Socket, hd: String, plain: String, split: String, lines: List<&2, String>, tl: String, ends: U32, most: U32, bytes: U32, keys: String, words: String, ms: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effects/host.c"
  import "./effects/host.js"

# the frames send_items sends, as bytes (for tests)
def Json.items_frames(hd: String, plain: String, split: String, lines: List<&2, String>, tl: String, ends: U32, most: U32, bytes: U32, keys: String, words: String) -> IO(List<&2, U32>):
  import "./effects/host.c"
  import "./effects/host.js"

def Sock.send_text(sock: Socket, text: String) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effects/host.c"
  import "./effects/host.js"

# a second handle on the same socket (one reader, one writer)
def Sock.dup(sock: Socket) -> IO(Socket & Result<&1, &1, U32 & String, Socket>):
  import "./effects/host.c"
  import "./effects/host.js"

# the peer's IP address ("" when unknown)
def Sock.peer(sock: Socket) -> IO(Socket & String):
  import "./effects/host.c"
  import "./effects/host.js"

# end our writing side; the peer reads EOF
def Sock.shutdown(sock: Socket) -> IO(Socket):
  import "./effects/host.c"
  import "./effects/host.js"

# start cmd in cwd; stdin+stdout are the returned socket. err: 0 drop
# stderr, 1 merge it into stdout, 2 inherit, 3 to a file of its own that
# Proc.errtail reads back
def Proc.spawn(cmd: String, args: List<&2, String>, cwd: String, err: U32) ->
  IO(Result<&1, &1, U32 & String, U32 & Socket>):
  import "./effects/host.c"
  import "./effects/host.js"

# the last 2048 bytes a process started with err 3 wrote to stderr ("" when
# none); its file is removed (call it once the process is reaped)
def Proc.errtail(pid: U32) -> IO(String):
  import "./effects/host.c"
  import "./effects/host.js"

def Proc.wait(pid: U32) -> IO(U32):
  import "./effects/host.c"
  import "./effects/host.js"

def Proc.kill(pid: U32, sig: U32) -> IO(Unit):
  import "./effects/host.c"
  import "./effects/host.js"

# whether a child has exited, without reaping it (the reader's wait does):
# its exit code (128 + signal when killed), or 4294967295 while it runs.
def Proc.exited(pid: U32) -> IO(U32):
  import "./effects/host.c"
  import "./effects/host.js"

# end our reading side: a read blocked on the socket (through any handle)
# returns end of stream
def Sock.shut_read(sock: Socket) -> IO(Socket):
  import "./effects/host.c"
  import "./effects/host.js"

# a raw descriptor on the same socket (0 when none), so the hub can shut a
# client's connection at once (Fd.shut) while its writer holds the socket
def Sock.raw(sock: Socket) -> IO(Socket & U32):
  import "./effects/host.c"
  import "./effects/host.js"

# shut the socket under a raw descriptor both ways, then close it (never
# 0, 1 or 2)
def Fd.shut(fd: U32) -> IO(Unit):
  import "./effects/host.c"
  import "./effects/host.js"

# close a raw descriptor (never 0, 1 or 2)
def Fd.close(fd: U32) -> IO(Unit):
  import "./effects/host.c"
  import "./effects/host.js"

# (kind, (size, mtime)): kind 0 file, 1 dir, 2 other
def FS.stat(path: String) -> IO(Result<&1, &1, U32 & String, U32 & U32 & U32>):
  import "./effects/host.c"
  import "./effects/host.js"

# Size and an exact revision for prompt schematic refresh. Includes file
# identity and nanosecond mtime/ctime, without changing FS.stat's contract.
def FS.revision(path: String) -> IO(Result<&1, &1, U32 & String, U32 & String>):
  import "./effects/host.c"
  import "./effects/host.js"

def FS.list(path: String) -> IO(Result<&1, &1, U32 & String, List<String>>):
  import "./effects/host.c"
  import "./effects/host.js"

def FS.mkdirs(path: String) -> IO(Result<&1, &1, U32 & String, Unit>):
  import "./effects/host.c"
  import "./effects/host.js"

def FS.rename(from: String, to: String) -> IO(Result<&1, &1, U32 & String, Unit>):
  import "./effects/host.c"
  import "./effects/host.js"

def FS.remove(path: String) -> IO(Result<&1, &1, U32 & String, Unit>):
  import "./effects/host.c"
  import "./effects/host.js"

def FS.chmod(path: String, mode: U32) -> IO(Result<&1, &1, U32 & String, Unit>):
  import "./effects/host.c"
  import "./effects/host.js"

# listen on one interface (Base's TCP.listen binds all of them)
def Net.listen(host: String, port: U32) -> IO(Result<&1, &1, U32 & String, Listener>):
  import "./effects/host.c"
  import "./effects/host.js"

# set an environment variable of this process (children started later see it)
def Env.put(name: String, value: String) -> IO(Unit):
  import "./effects/host.c"
  import "./effects/host.js"

# the directory holding the running binary
def Sys.exe_dir() -> IO(String):
  import "./effects/host.c"
  import "./effects/host.js"

# replace this process with path and args (answers only on failure)
def Sys.exec(path: String, args: List<&2, String>) -> IO(Result<&1, &1, U32 & String, Unit>):
  import "./effects/host.c"
  import "./effects/host.js"

# the running binary's full path
def Sys.exe_path() -> IO(String):
  import "./effects/host.c"
  import "./effects/host.js"

# Native OS process ID (Win32 on Cygwin), for native child supervision.
def Sys.native_pid() -> IO(U32):
  import "./effects/host.c"
  import "./effects/host.js"

# A filesystem argument for a native OS tool (Win32 paths on Cygwin).
def Sys.native_path(path: String) -> IO(String):
  import "./effects/host.c"
  import "./effects/host.js"
