import Base

# Decoding images for the native window (effects/img.c, stb_image): the
# pixels of an image file, shrunk to fit maxw x maxh, as (w, (h, rgb bytes)).
def Imgfx.decode(path: String, maxw: U32, maxh: U32) ->
  IO(Result<&1, &1, U32 & String, U32 & U32 & List<&2, U32>>):
  import "./effects/img.js"

# the same pixels as gfx/bitmap.bend's Bitmap.image holds them: an Image
# quadtree (decoded off the loop, built in C), as (w, (h, tree))
def Imgfx.tree(path: String, maxw: U32, maxh: U32) ->
  IO(Result<&1, &1, U32 & String, U32 & U32 & Image>):
  import "./effects/img.js"
