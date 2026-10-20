import Base
import ./model.bend as M

# A persistent desktop/web typography preference, separate from display DPI.
# Native phone clients retain the operating system's accessibility text size.
# A closed set bounds work and lets the native client keep one font per size.
def Scale.options() -> List<&2, String>:
  ["85", "100", "115", "130", "150"]

def Scale.read(+s: String) -> U32:
  Bool.pick(U32, String.eq(s, "85"), 85,
    Bool.pick(U32, String.eq(s, "115"), 115,
      Bool.pick(U32, String.eq(s, "130"), 130,
        Bool.pick(U32, String.eq(s, "150"), 150, 100))))

def Scale.of(st: M.State) -> U32:
  Scale.read(M.Setting.get(st, "ui.text_scale", "100"))

def Scale.label(s: String) -> String:
  U32.show(Scale.read(s)) ++ "%"

# Rounded physical pixels; callers supply bounded UI dimensions, not epochs.
def Scale.px(+pct: U32, +px: U32) -> U32:
  ((px * pct + 50 : U32) / 100 : U32)
