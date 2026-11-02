import Base

# The response codec is negotiated only by /far/log v4. Empty from unpack
# means an invalid or oversized encoded body; a plain legacy body passes on.
def FarCodec.pack(response: List<&2, U32>) -> IO(List<&2, U32>):
  import "./effects/farcodec.js"

def FarCodec.unpack(body: String) -> IO(String):
  import "./effects/farcodec.js"
