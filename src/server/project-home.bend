import Base

# Bounded async reads, revision-checked intent writes. Only the hub calls this.
def ProjectHome.read(root: String, request: String) -> IO(String):
  import "./effects/project-home.js"
