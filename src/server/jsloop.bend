import Base

# Which loop runs the program: "bun" when
# effects/loop.js put the runtime on Bun's event loop (sockets, processes
# and timers through Bun's APIs, so a Windows build serves; the default),
# else "sync" (the runtime's own select loop, POSIX only, under
# BACKPLANE_JS_LOOP=sync).
# Every main asks it first: loop.js is bundled only when a def it backs is
# reached.
def Rt.loop() -> IO(String):
  import "./effects/loop.js"
