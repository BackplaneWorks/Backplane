import Base

# The event log must be read completely before the hub may append to it.
# Optional caches may use Files.read's empty fallback; persisted history may
# not. Only ENOENT means a new home with no log yet.

def LogRead.missing(code: U32) -> Bool:
  U32.is_eq(code, 2)

# Base's native File.read_bytes caps one read at INT32_MAX. Refuse a larger
# file before allocating rather than silently replaying only its prefix.
def LogRead.supported(size: U32) -> Bool:
  U32.is_le(size, 2147483647)

def LogRead.complete(size: U32, bytes: Nat) -> Bool:
  Nat.is_eq(U32.to_nat(size), bytes)

# Base's List.length is not tail-recursive. Logs may contain millions of
# bytes, so count once with an accumulator rather than growing the stack.
def LogRead.count(bytes: List<&2, U32>, acc: Nat) -> Nat:
  match bytes:
    case Nil{}:
      acc
    case _ <> rest:
      LogRead.count(rest, 1n+acc)

def LogRead.error(path: String, op: String, code: U32, msg: String) -> IO(List<&2, U32>):
  IO.die(List<&2, U32>, 1, "could not read event log " ++ path ++ " (" ++ op ++ "): " ++ U32.show(code) ++ ": " ++ msg)

def LogRead.failed(file: File, path: String, op: String, code: U32, msg: String) -> IO(List<&2, U32>):
  do IO<List<&2, U32>>:
    File.close(file)
    LogRead.error(path, op, code, msg)

def LogRead.checked(complete: Bool, path: String, size: U32, actual: Nat, bytes: List<&2, U32>) -> IO(List<&2, U32>):
  match complete:
    case True{}:
      IO.pure(List<&2, U32>, bytes)
    case False{}:
      IO.die(List<&2, U32>, 1, "could not read event log " ++ path ++ ": incomplete read (expected " ++ U32.show(size) ++ " bytes, got " ++ Nat.show(actual) ++ ")")

def LogRead.read(r: File & Result<&1, &1, U32 & String, List<&2, U32>>, +path: String, +size: U32) -> IO(List<&2, U32>):
  (file, got) = r
  match got:
    case Fail{(code, msg)}:
      LogRead.failed(file, path, "read", code, msg)
    case Done{+bytes}:
      do IO<List<&2, U32>>:
        File.close(file)
        +actual : Nat = LogRead.count(bytes, 0n)
        LogRead.checked(LogRead.complete(size, actual), path, size, actual, bytes)

def LogRead.allowed(supported: Bool, file: File, +path: String, +size: U32) -> IO(List<&2, U32>):
  match supported:
    case True{}:
      IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, List<&2, U32>, File.read_bytes(file, size), r => LogRead.read(r, path, size))
    case False{}:
      do IO<List<&2, U32>>:
        File.close(file)
        IO.die(List<&2, U32>, 1, "could not read event log " ++ path ++ ": file exceeds the complete-read limit of 2147483647 bytes")

def LogRead.sized(r: File & Result<&1, &1, U32 & String, U32>, +path: String) -> IO(List<&2, U32>):
  (file, got) = r
  match got:
    case Fail{(code, msg)}:
      LogRead.failed(file, path, "size", code, msg)
    case Done{+size}:
      LogRead.allowed(LogRead.supported(size), file, path, size)

def LogRead.absent(missing: Bool, path: String, code: U32, msg: String) -> IO(List<&2, U32>):
  match missing:
    case True{}:
      IO.pure(List<&2, U32>, Nil{})
    case False{}:
      LogRead.error(path, "open", code, msg)

def LogRead.opened(r: Result<&1, &1, U32 & String, File>, +path: String) -> IO(List<&2, U32>):
  match r:
    case Fail{(+code, msg)}:
      LogRead.absent(LogRead.missing(code), path, code, msg)
    case Done{file}:
      IO.bind(File & Result<&1, &1, U32 & String, U32>, List<&2, U32>, File.size(file), r => LogRead.sized(r, path))

def LogRead.bytes(+path: String) -> IO(List<&2, U32>):
  IO.bind(Result<&1, &1, U32 & String, File>, List<&2, U32>, File.open(path, "r"), r => LogRead.opened(r, path))
