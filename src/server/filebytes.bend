import Base

# A whole-file read that preserves failures. In particular, Done{[]} means
# a readable empty file; an open, size, or read error must not become one.
def FileBytes.result(r: Result<&1, &1, U32 & String, List<&2, U32>>) -> Result<&2, &2, String, List<&2, U32>>:
  match r:
    case Done{bs}:
      Done{bs}
    case Fail{(_, why)}:
      Fail{why}

def FileBytes.done(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(Result<&2, &2, String, List<&2, U32>>):
  (f, res) = r
  do IO<Result<&2, &2, String, List<&2, U32>>>:
    File.close(f)
    return FileBytes.result(res)

def FileBytes.size(r: Result<&1, &1, U32 & String, U32>, f: File) -> IO(Result<&2, &2, String, List<&2, U32>>):
  match r:
    case Done{n}:
      IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, Result<&2, &2, String, List<&2, U32>>, File.read_bytes(f, U32.max(n, 1)), FileBytes.done)
    case Fail{(_, why)}:
      do IO<Result<&2, &2, String, List<&2, U32>>>:
        File.close(f)
        return Fail{why}

def FileBytes.sized(r: File & Result<&1, &1, U32 & String, U32>) -> IO(Result<&2, &2, String, List<&2, U32>>):
  (f, res) = r
  FileBytes.size(res, f)

def FileBytes.opened(r: Result<&1, &1, U32 & String, File>) -> IO(Result<&2, &2, String, List<&2, U32>>):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, U32>, Result<&2, &2, String, List<&2, U32>>, File.size(f), FileBytes.sized)
    case Fail{(_, why)}:
      IO.pure(Result<&2, &2, String, List<&2, U32>>, Fail{why})

def FileBytes.read(path: String) -> IO(Result<&2, &2, String, List<&2, U32>>):
  IO.bind(Result<&1, &1, U32 & String, File>, Result<&2, &2, String, List<&2, U32>>, File.open(path, "r"), FileBytes.opened)
