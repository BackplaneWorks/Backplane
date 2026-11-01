import Base
import ./check.bend as C
import ../src/core/landing.bend as L

# What a page reply redraws, from host.js's text (regions and the delta's
# paths, a line each, steps tab-separated)
def main() -> IO(Unit):
  do IO<Unit>:
    +regions : String = "timeline\tshown\tkids\t0\tkids\t3\ntopbar\tshown\tkids\t0\tkids\t1"
    C.Check.str("rows under the timeline", L.Land.of(regions, "shown\tkids\t0\tkids\t3\tkids\nwindow\tolder"), "timeline ")
    C.Check.str("the top bar's count and the rows", L.Land.of(regions, "shown\tkids\t0\tkids\t1\tkids\t4\nshown\tkids\t0\tkids\t3\tkids"), "timeline topbar ")
    C.Check.str("a sidebar row: the page", L.Land.of(regions, "shown\tkids\t0\tkids\t3\tkids\nshown\tkids\t0\tkids\t2\tkids\t7"), "page")
    C.Check.str("the session: the page", L.Land.of(regions, "session\tdrafts\tt"), "page")
    C.Check.str("a whole reply (the root): the page", L.Land.of(regions, "\n"), "page")
    C.Check.str("only the window: nothing", L.Land.of(regions, "window\tolder"), "")
    C.Check.str("nothing written: nothing", L.Land.of(regions, ""), "")
    C.Check.str("a sibling with a longer index is not inside", L.Land.of(regions, "shown\tkids\t0\tkids\t31"), "page")
