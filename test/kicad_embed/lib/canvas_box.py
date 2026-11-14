"""The board canvas inside an editor frame's screenshot: canvas_box(rgb array of the frame) -> (x0, y0, x1, y1) in that array.
KiCad's GL canvas is no X window of its own, so it is found by its background: the colour with the longest horizontal run in the frame
(the canvas's margin above or below the board after zoom to fit; gaps up to 40 px, text and outlines drawn over it, kept), then that
colour's vertical run at the run's left edge, which must cover most of the frame (a menu or tool bar is as wide, but short). The side panels (Appearance, Selection Filter, Properties: bp.12.x keeps them open) and the toolbars fall outside, so a
colour searched for in the canvas is never a panel's checkbox (Nov 13, bp.12.2: the Selection Filter's Pads box matched the corridors'
orange and was clicked as c-q)."""
import numpy as np

def _run(on, gap=40):
    best, start, last = (0, 0), None, None
    for i, v in enumerate(list(on) + [False] * (gap + 1)):
        if v:
            if start is None: start = i
            last = i
        elif start is not None and i - last > gap:
            if last - start > best[1] - best[0]: best = (start, last)
            start = None
    return best

def canvas_box(a):
    q = a // 4
    flat = q.reshape(-1, 3)
    cols, counts = np.unique(flat, axis=0, return_counts=True)
    found = []  # (horizontal run, colour, x0, x1) per colour and sampled row
    for c in cols[counts >= 0.01 * len(flat)]:
        m = np.all(q == c, axis=-1)
        for y in range(0, a.shape[0], 4):
            x0, x1 = _run(m[y])
            if x1 > x0: found.append((x1 - x0, tuple(c), x0, x1))
    # the canvas: the longest run whose colour also runs down most of the frame at its left edge (a menu or tool bar is as wide but short)
    for _, c, x0, x1 in sorted(found, reverse=True):
        m = np.all(q == np.array(c), axis=-1)
        y0, y1 = _run(m[:, min(x0 + 3, x1)])
        # (and starts below the menu and tool bars: a bar's colour runs from the frame's top down the vertical tool bar)
        if y1 - y0 >= 0.4 * a.shape[0] and y0 >= 0.08 * a.shape[0]: return x0, y0, x1, y1
    return 0, 0, a.shape[1] - 1, a.shape[0] - 1
