import Base

# Bounded local metadata sampling. No subprocess operation or network IO.
def Progress.sample(plan: String, home: String) -> IO(String):
  import "./effects/liveness.js"
