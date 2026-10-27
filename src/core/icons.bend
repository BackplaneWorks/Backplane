import Base
import ./svg.bend as S

# The icon set: SVG path data on a 24 x 24 grid, drawn with a 2-unit
# round stroke unless a layer says fill. One source for both UIs: the
# window rasterizes these (src/gfx/icon.bend), the web client writes them
# as inline <svg> (src/web/icon.bend).
#
# The paths are Lucide's (https://lucide.dev), with <circle>, <rect> and
# <line> elements written as paths and rectangle corners squared (house
# rule). Lucide's licence:
#
#   ISC License
#
#   Copyright (c) for portions of Lucide are held by Cole Bemis 2013-2022
#   as part of Feather (MIT). All other copyright (c) for Lucide are held
#   by Lucide Contributors 2022.
#
#   Permission to use, copy, modify, and/or distribute this software for
#   any purpose with or without fee is hereby granted, provided that the
#   above copyright notice and this permission notice appear in all
#   copies.
#
#   THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL
#   WARRANTIES WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED
#   WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE
#   AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL
#   DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR
#   PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER
#   TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR
#   PERFORMANCE OF THIS SOFTWARE.
#
#   Feather (MIT): Copyright (c) 2013-2022 Cole Bemis. Permission is
#   hereby granted, free of charge, to any person obtaining a copy of this
#   software to deal in it without restriction, subject to including this
#   notice; it is provided "as is", without warranty of any kind.

# one path of an icon: stroked, or filled (even-odd)
type Layer is Data:
  Layer{d: String, fill: Bool}

type IDef is Data:
  IDef{name: String, layers: List<&2, Layer>}

def Icon.s(d: String) -> Layer:
  Layer{d, False{}}

def Icon.all() -> List<&2, IDef>:
  [IDef{"plus", [Icon.s("M5 12h14"), Icon.s("M12 5v14")]},
   IDef{"x", [Icon.s("M18 6 6 18"), Icon.s("m6 6 12 12")]},
   IDef{"chevron-down", [Icon.s("m6 9 6 6 6-6")]},
   IDef{"search", [Icon.s("M3 11a8 8 0 1 0 16 0a8 8 0 1 0-16 0"), Icon.s("m21 21-4.3-4.3")]},
   IDef{"chevron-right", [Icon.s("m9 18 6-6-6-6")]},
   IDef{"settings", [Icon.s("M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"),
     Icon.s("M9 12a3 3 0 1 0 6 0a3 3 0 1 0-6 0")]},
   IDef{"paperclip", [Icon.s("m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l8.57-8.57A4 4 0 1 1 18 8.84l-8.59 8.57a2 2 0 0 1-2.83-2.83l8.49-8.48")]},
   IDef{"arrow-up", [Icon.s("m5 12 7-7 7 7"), Icon.s("M12 19V5")]},
   IDef{"send", [Icon.s("M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z"), Icon.s("m21.854 2.147-10.94 10.939")]},
   IDef{"stop", [Layer{"M6 6h12v12H6z", True{}}]},
   IDef{"play", [Layer{"M7 4l13 8-13 8z", True{}}]},
   IDef{"pause", [Layer{"M6 4h4v16H6z", True{}}, Layer{"M14 4h4v16h-4z", True{}}]},
   IDef{"chevron-left", [Icon.s("m15 18-6-6 6-6")]},
   IDef{"skip-back", [Icon.s("M19 20 9 12l10-8z"), Icon.s("M5 19V5")]},
   IDef{"skip-forward", [Icon.s("M5 4l10 8-10 8z"), Icon.s("M19 5v14")]},
   IDef{"history", [Icon.s("M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"), Icon.s("M3 3v5h5"), Icon.s("M12 7v5l4 2")]},
   IDef{"git-compare", [Icon.s("M15 18a3 3 0 1 0 6 0a3 3 0 1 0-6 0"), Icon.s("M3 6a3 3 0 1 0 6 0a3 3 0 1 0-6 0"), Icon.s("M13 6h3a2 2 0 0 1 2 2v7"), Icon.s("M11 18H8a2 2 0 0 1-2-2V9")]},
   IDef{"message-square", [Icon.s("M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z")]},
   IDef{"sun", [Icon.s("M8 12a4 4 0 1 0 8 0a4 4 0 1 0-8 0"), Icon.s("M12 2v2"), Icon.s("M12 20v2"), Icon.s("m4.93 4.93 1.41 1.41"), Icon.s("m17.66 17.66 1.41 1.41"),
     Icon.s("M2 12h2"), Icon.s("M20 12h2"), Icon.s("m6.34 17.66-1.41 1.41"), Icon.s("m19.07 4.93-1.41 1.41")]},
   IDef{"moon", [Icon.s("M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z")]},
   IDef{"mic", [Icon.s("M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z"), Icon.s("M19 10v2a7 7 0 0 1-14 0v-2"), Icon.s("M12 19v3")]},
   IDef{"terminal", [Icon.s("m4 17 6-6-6-6"), Icon.s("M12 19h8")]},
   IDef{"git-branch", [Icon.s("M6 3v12"), Icon.s("M15 6a3 3 0 1 0 6 0a3 3 0 1 0-6 0"), Icon.s("M3 18a3 3 0 1 0 6 0a3 3 0 1 0-6 0"), Icon.s("M18 9a9 9 0 0 1-9 9")]},
   IDef{"panel-right", [Icon.s("M3 3h18v18H3z"), Icon.s("M15 3v18")]},
   IDef{"shopping-cart", [Icon.s("M7 21a1 1 0 1 0 2 0a1 1 0 1 0-2 0"), Icon.s("M18 21a1 1 0 1 0 2 0a1 1 0 1 0-2 0"), Icon.s("M2.05 2.05h2l2.66 12.42a2 2 0 0 0 2 1.58h9.78a2 2 0 0 0 1.95-1.57l1.65-7.43H5.12")]},
   IDef{"link", [Icon.s("M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"), Icon.s("M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71")]},
   IDef{"external-link", [Icon.s("M15 3h6v6"), Icon.s("M10 14 21 3"), Icon.s("M18 13v8H3V6h8")]},
   IDef{"zoom-in", [Icon.s("M3 11a8 8 0 1 0 16 0a8 8 0 1 0-16 0"), Icon.s("m21 21-4.35-4.35"), Icon.s("M11 8v6"), Icon.s("M8 11h6")]},
   IDef{"zoom-out", [Icon.s("M3 11a8 8 0 1 0 16 0a8 8 0 1 0-16 0"), Icon.s("m21 21-4.35-4.35"), Icon.s("M8 11h6")]},
   IDef{"maximize", [Icon.s("M15 3h6v6"), Icon.s("M9 21H3v-6"), Icon.s("m21 3-7 7"), Icon.s("m3 21 7-7")]},
   IDef{"circuit-board", [Icon.s("M3 3h18v18H3z"), Icon.s("M11 9h4a2 2 0 0 0 2-2V3"), Icon.s("M7 9a2 2 0 1 0 4 0a2 2 0 1 0-4 0"), Icon.s("M7 21v-4a2 2 0 0 1 2-2h4"), Icon.s("M13 15a2 2 0 1 0 4 0a2 2 0 1 0-4 0")]},
   IDef{"box", [Icon.s("M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z"), Icon.s("m3.3 7 8.7 5 8.7-5"), Icon.s("M12 22V12")]},
   IDef{"diff", [Icon.s("M12 3v14"), Icon.s("M5 10h14"), Icon.s("M5 21h14")]},
   IDef{"copy", [Icon.s("M8 8h14v14H8z"), Icon.s("M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2")]},
   IDef{"folder-plus", [Icon.s("M12 10v6"), Icon.s("M9 13h6"), Icon.s("M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z")]},
   IDef{"file", [Icon.s("M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"), Icon.s("M14 2v4a2 2 0 0 0 2 2h4")]},
   IDef{"file-text", [Icon.s("M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"), Icon.s("M14 2v4a2 2 0 0 0 2 2h4"), Icon.s("M10 9H8"), Icon.s("M16 13H8"), Icon.s("M16 17H8")]},
   IDef{"square-pen", [Icon.s("M12 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"), Icon.s("M18.4 2.6a2.1 2.1 0 0 1 3 3L12 15l-4 1 1-4Z")]},
   IDef{"image", [Icon.s("M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Z"), Icon.s("M9 7a2 2 0 1 0 0 4a2 2 0 1 0 0-4Z"), Icon.s("m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21")]},
   IDef{"folder", [Icon.s("M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z")]},
   IDef{"layout-panel-left", [Icon.s("M3 3h7v18H3z"), Icon.s("M14 3h7v7h-7z"), Icon.s("M14 14h7v7h-7z")]},
   IDef{"cpu", [Icon.s("M4 4h16v16H4z"), Icon.s("M9 9h6v6H9z"), Icon.s("M15 2v2"), Icon.s("M15 20v2"), Icon.s("M2 15h2"), Icon.s("M2 9h2"), Icon.s("M20 15h2"), Icon.s("M20 9h2"), Icon.s("M9 2v2"), Icon.s("M9 20v2")]},
   IDef{"gauge", [Icon.s("M12 14L16 10"), Icon.s("M3.34 19a10 10 0 1 1 17.32 0")]},
   IDef{"zap", [Icon.s("M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z")]},
   IDef{"radio", [Icon.s("M4.9 19.1C1 15.2 1 8.8 4.9 4.9"), Icon.s("M7.8 16.2c-2.3-2.3-2.3-6.1 0-8.5"), Icon.s("M10 12a2 2 0 1 0 4 0a2 2 0 1 0-4 0"), Icon.s("M16.2 7.8c2.3 2.3 2.3 6.1 0 8.5"), Icon.s("M19.1 4.9C23 8.8 23 15.1 19.1 19")]},
   IDef{"layers", [Icon.s("M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z"), Icon.s("m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65"), Icon.s("m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65")]},
   IDef{"code", [Icon.s("m16 18 6-6-6-6"), Icon.s("m8 6-6 6 6 6")]},
   IDef{"globe", [Icon.s("M2 12a10 10 0 1 0 20 0a10 10 0 1 0-20 0"), Icon.s("M12 2a14.5 14.5 0 0 0 0 20a14.5 14.5 0 0 0 0-20"), Icon.s("M2 12h20")]},
   IDef{"wrench", [Icon.s("M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z")]},
   IDef{"flask-conical", [Icon.s("M10 2v7.53a2 2 0 0 1-.21.9L4.72 20.55a1 1 0 0 0 .9 1.45h12.76a1 1 0 0 0 .9-1.45l-5.07-10.13A2 2 0 0 1 14 9.53V2"), Icon.s("M8.5 2h7"), Icon.s("M7 16h10")]},
   IDef{"book", [Icon.s("M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20")]},
   IDef{"star", [Icon.s("M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z")]},
   IDef{"pin", [Icon.s("M12 17v5"), Icon.s("M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z")]},
   IDef{"circle-check", [Icon.s("M2 12a10 10 0 1 0 20 0a10 10 0 1 0-20 0"), Icon.s("m9 12 2 2 4-4")]},
   IDef{"clock", [Icon.s("M2 12a10 10 0 1 0 20 0a10 10 0 1 0-20 0"), Icon.s("M12 6v6l4 2")]},
   IDef{"archive", [Icon.s("M2 3h20v5H2z"), Icon.s("M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"), Icon.s("M10 12h4")]},
   IDef{"trash", [Icon.s("M3 6h18"), Icon.s("M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"), Icon.s("M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"), Icon.s("M10 11v6"), Icon.s("M14 11v6")]},
   IDef{"git-fork", [Icon.s("M9 18a3 3 0 1 0 6 0a3 3 0 1 0-6 0"), Icon.s("M3 6a3 3 0 1 0 6 0a3 3 0 1 0-6 0"), Icon.s("M15 6a3 3 0 1 0 6 0a3 3 0 1 0-6 0"), Icon.s("M18 9v2c0 .6-.4 1-1 1H7c-.6 0-1-.4-1-1V9"), Icon.s("M12 12v3")]},
   IDef{"undo", [Icon.s("M9 14 4 9l5-5"), Icon.s("M4 9h10.5a5.5 5.5 0 0 1 5.5 5.5a5.5 5.5 0 0 1-5.5 5.5H11")]},
   IDef{"bell", [Icon.s("M10.268 21a2 2 0 0 0 3.464 0"), Icon.s("M3.262 15.326A1 1 0 0 0 4 17h16a1 1 0 0 0 .74-1.673C19.41 13.956 18 12.499 18 8A6 6 0 0 0 6 8c0 4.499-1.411 5.956-2.738 7.326")]},
   IDef{"repeat", [Icon.s("m17 2 4 4-4 4"), Icon.s("M3 11v-1a4 4 0 0 1 4-4h14"), Icon.s("m7 22-4-4 4-4"), Icon.s("M21 13v1a4 4 0 0 1-4 4H3")]}]

def Icon.pick(hit: Bool, ls: List<&2, Layer>, found: List<&2, Layer>) -> List<&2, Layer>:
  match hit:
    case True{}:
      ls
    case False{}:
      found

def Icon.find(ds: List<&2, IDef>, +name: String, found: List<&2, Layer>) -> List<&2, Layer>:
  match ds:
    case Nil{}:
      found
    case IDef{+n, ls} <> t:
      Icon.find(t, name, Icon.pick(String.eq(n, name), ls, found))

# an icon's layers (none for a name not in the set)
def Icon.layers(+name: String) -> List<&2, Layer>:
  Icon.find(Icon.all(), name, Nil{})

def Icon.names.go(ds: List<&2, IDef>) -> List<&2, String>:
  match ds:
    case Nil{}:
      Nil{}
    case IDef{n, _} <> t:
      n <> Icon.names.go(t)

def Icon.names() -> List<&2, String>:
  Icon.names.go(Icon.all())

# the icons the two UIs draw (laws icons_*: each is in the set)
def Icon.used() -> List<&2, String>:
  ["plus", "x", "chevron-down", "chevron-right", "settings", "paperclip", "arrow-up", "stop", "mic", "search", "sun", "moon",
   "play", "pause", "chevron-left", "skip-back", "skip-forward", "history", "git-compare", "message-square",
   "pin", "circle-check", "clock", "archive", "trash", "git-fork", "undo", "terminal", "panel-right", "diff",
   "folder", "file", "file-text", "image", "circuit-board", "cpu", "box", "bell", "repeat", "square-pen", "gauge", "shopping-cart", "layout-panel-left"]

def Icon.missing.k(none: Bool, +n: String, rest: List<&2, String>) -> List<&2, String>:
  match none:
    case True{}:
      n <> rest
    case False{}:
      rest

# the names that have no icon
def Icon.missing(ns: List<&2, String>) -> List<&2, String>:
  match ns:
    case Nil{}:
      Nil{}
    case +n <> t:
      Icon.missing.k(List.is_empty(&2, Layer, Icon.layers(n)), n, Icon.missing(t))

def Icon.starts(ss: List<&2, S.Seg>) -> Bool:
  match ss:
    case S.SM{_, _} <> _:
      True{}
    case _:
      False{}

# some layer's path does not begin with a moveto (or is empty)
def Icon.lost(ls: List<&2, Layer>) -> Bool:
  match ls:
    case Nil{}:
      False{}
    case Layer{d, _} <> t:
      Bool.or(Bool.not(Icon.starts(S.Svg.parse(d))), Icon.lost(t))

# the icons with a path that draws nothing (law icons_all_draw)
def Icon.bad(ds: List<&2, IDef>) -> List<&2, String>:
  match ds:
    case Nil{}:
      Nil{}
    case IDef{+n, ls} <> t:
      Icon.missing.k(Icon.lost(ls), n, Icon.bad(t))
