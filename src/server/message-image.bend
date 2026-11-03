import Base
import ../core/images.bend as IM
import ../core/model.bend as M

# Only the indexed assistant message grants a path. The HTTP caller has
# already proved its current pairing credential; neither IDs nor URLs grant it.
def MessageImage.text(entry: String) -> IO(String):
  import "./effects/message_image.js"

def MessageImage.snapshot(entry: String, path: String) -> IO(String):
  import "./effects/message_image.js"

def MessageImage.retain(entry: String, path: String) -> IO(Unit):
  import "./effects/message_image.js"

def MessageImage.paths(+entry: String, paths: List<&2, String>) -> IO(Unit):
  match paths:
    case Nil{}: IO.pure(Unit, Unit{})
    case path <> rest:
      do IO<Unit>:
        MessageImage.retain(entry, path)
        MessageImage.paths(entry, rest)

def MessageImage.change(c: M.Change) -> IO(Unit):
  match c:
    case M.MessagePosted{_, entry, M.Assistant{}, text, _}:
      MessageImage.paths(entry, IM.Img.paths(text))
    case _: IO.pure(Unit, Unit{})

# After commit, before announcing the message. A missing or rejected image
# never prevents delivery of the text; it renders as unavailable instead.
def MessageImage.changes(cs: List<&2, M.Change>) -> IO(Unit):
  match cs:
    case Nil{}: IO.pure(Unit, Unit{})
    case c <> rest:
      do IO<Unit>:
        MessageImage.change(c)
        MessageImage.changes(rest)
