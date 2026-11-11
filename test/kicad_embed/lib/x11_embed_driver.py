#!/usr/bin/env python3
"""X11 embedding driver and verification oracle for test/kicad_embed/*.sh.

Compiles a tiny C shared library against libX11 + libXext into $BP_TMP on first run so all Xlib/XShape
structures use system header layouts with zero ctypes struct-padding risk.
"""
import ctypes
import hashlib
import json
import os
import subprocess
import sys
import time
from PIL import Image

C_SOURCE = r"""
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/Xatom.h>
#include <X11/extensions/shape.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static int ignore_xerr(Display *d, XErrorEvent *e) {
    (void)d; (void)e;
    return 0;
}

static volatile sig_atomic_t g_stop = 0;
static void on_sig(int s) { (void)s; g_stop = 1; }

int kembed_container_daemon(unsigned long parent_xid, int x, int y, int w, int h, const char *ready_path) {
    XSetErrorHandler(ignore_xerr);
    Display *d = XOpenDisplay(NULL);
    if (!d) return 2;
    int s = DefaultScreen(d);
    XSetWindowAttributes swa;
    memset(&swa, 0, sizeof(swa));
    swa.background_pixel = BlackPixel(d, s);
    swa.border_pixel = BlackPixel(d, s);
    swa.colormap = DefaultColormap(d, s);
    swa.event_mask = ExposureMask | StructureNotifyMask | SubstructureNotifyMask;
    Window win = XCreateWindow(
        d, (Window)parent_xid, x, y, (unsigned int)(w > 0 ? w : 1), (unsigned int)(h > 0 ? h : 1),
        0, DefaultDepth(d, s), InputOutput, DefaultVisual(d, s),
        CWBackPixel | CWBorderPixel | CWColormap | CWEventMask, &swa
    );
    if (!win) { XCloseDisplay(d); return 3; }
    XStoreName(d, win, "BackplaneKiCadContainer");
    XMapWindow(d, win);
    XSync(d, False);

    FILE *f = fopen(ready_path, "w");
    if (f) {
        fprintf(f, "0x%lx\n", (unsigned long)win);
        fclose(f);
    }
    signal(SIGTERM, on_sig);
    signal(SIGINT, on_sig);
    while (!g_stop) {
        while (XPending(d) > 0) {
            XEvent ev;
            XNextEvent(d, &ev);
        }
        usleep(50000);
    }
    XDestroyWindow(d, win);
    XSync(d, False);
    XCloseDisplay(d);
    return 0;
}

int kembed_send_configure(Display *d, Window child) {
    XWindowAttributes wa;
    if (!XGetWindowAttributes(d, child, &wa)) return 0;
    Window dummy = 0;
    int rx = 0, ry = 0;
    XTranslateCoordinates(d, child, DefaultRootWindow(d), 0, 0, &rx, &ry, &dummy);
    XEvent ev;
    memset(&ev, 0, sizeof(ev));
    ev.xconfigure.type = ConfigureNotify;
    ev.xconfigure.send_event = True;
    ev.xconfigure.display = d;
    ev.xconfigure.event = child;
    ev.xconfigure.window = child;
    ev.xconfigure.x = rx;
    ev.xconfigure.y = ry;
    ev.xconfigure.width = wa.width;
    ev.xconfigure.height = wa.height;
    ev.xconfigure.border_width = 0;
    ev.xconfigure.above = None;
    ev.xconfigure.override_redirect = wa.override_redirect;
    XSendEvent(d, child, False, StructureNotifyMask, &ev);
    XSync(d, False);
    return 1;
}

int kembed_embed(unsigned long container_xid, unsigned long child_xid, int w, int h) {
    XSetErrorHandler(ignore_xerr);
    Display *d = XOpenDisplay(NULL);
    if (!d) return 0;
    Window c = (Window)container_xid;
    Window ch = (Window)child_xid;
    XWithdrawWindow(d, ch, DefaultScreen(d));
    XSync(d, False);
    usleep(120000);
    XSizeHints *sh = XAllocSizeHints();
    if (sh) {
        sh->flags = PMinSize;
        sh->min_width = 120;
        sh->min_height = 100;
        XSetWMNormalHints(d, ch, sh);
        XFree(sh);
    }
    XReparentWindow(d, ch, c, 0, 0);
    XMoveResizeWindow(d, ch, 0, 0, (unsigned int)(w > 0 ? w : 1), (unsigned int)(h > 0 ? h : 1));
    XMapWindow(d, c);
    XMapWindow(d, ch);
    XSync(d, False);
    kembed_send_configure(d, ch);
    XCloseDisplay(d);
    return 1;
}

int kembed_withdraw(unsigned long xid) {
    XSetErrorHandler(ignore_xerr);
    Display *d = XOpenDisplay(NULL);
    if (!d) return 0;
    XWithdrawWindow(d, (Window)xid, DefaultScreen(d));
    XSync(d, False);
    XCloseDisplay(d);
    return 1;
}

int kembed_place(unsigned long container_xid, unsigned long child_xid, int x, int y, int w, int h, int send_cfg) {
    XSetErrorHandler(ignore_xerr);
    Display *d = XOpenDisplay(NULL);
    if (!d) return 0;
    unsigned int uw = (unsigned int)(w > 0 ? w : 1);
    unsigned int uh = (unsigned int)(h > 0 ? h : 1);
    if (container_xid) {
        XMoveResizeWindow(d, (Window)container_xid, x, y, uw, uh);
    }
    if (child_xid) {
        XMoveResizeWindow(d, (Window)child_xid, 0, 0, uw, uh);
    }
    XSync(d, False);
    if (send_cfg && child_xid) {
        kembed_send_configure(d, (Window)child_xid);
    }
    XCloseDisplay(d);
    return 1;
}

int kembed_notify_configure(unsigned long child_xid) {
    XSetErrorHandler(ignore_xerr);
    Display *d = XOpenDisplay(NULL);
    if (!d) return 0;
    int ok = kembed_send_configure(d, (Window)child_xid);
    XCloseDisplay(d);
    return ok;
}

int kembed_show(unsigned long xid, int show) {
    XSetErrorHandler(ignore_xerr);
    Display *d = XOpenDisplay(NULL);
    if (!d) return 0;
    if (show) XMapWindow(d, (Window)xid);
    else XUnmapWindow(d, (Window)xid);
    XSync(d, False);
    XCloseDisplay(d);
    return 1;
}

int kembed_switch(unsigned long show_xid, const unsigned long *hide_xids, int n_hide) {
    XSetErrorHandler(ignore_xerr);
    Display *d = XOpenDisplay(NULL);
    if (!d) return 0;
    for (int i = 0; i < n_hide; i++) {
        if (hide_xids[i] && hide_xids[i] != show_xid) {
            XUnmapWindow(d, (Window)hide_xids[i]);
        }
    }
    if (show_xid) {
        XMapRaised(d, (Window)show_xid);
    }
    XSync(d, False);
    if (show_xid) {
        kembed_send_configure(d, (Window)show_xid);
    }
    XCloseDisplay(d);
    return 1;
}

unsigned long kembed_focus(unsigned long xid) {
    XSetErrorHandler(ignore_xerr);
    Display *d = XOpenDisplay(NULL);
    if (!d) return 0;
    if (xid) {
        XWindowAttributes wa;
        if (XGetWindowAttributes(d, (Window)xid, &wa) && wa.map_state == IsViewable) {
            XSetInputFocus(d, (Window)xid, RevertToParent, CurrentTime);
            XSync(d, False);
        }
    }
    Window cur = 0;
    int rev = 0;
    XGetInputFocus(d, &cur, &rev);
    XCloseDisplay(d);
    return (unsigned long)cur;
}

int kembed_is_descendant(unsigned long ancestor, unsigned long target) {
    if (!ancestor || !target) return 0;
    if (ancestor == target) return 1;
    XSetErrorHandler(ignore_xerr);
    Display *d = XOpenDisplay(NULL);
    if (!d) return 0;
    Window root = 0, parent = 0, *kids = NULL;
    unsigned int nkids = 0;
    Window cur = (Window)target;
    int found = 0;
    for (int depth = 0; depth < 32 && cur != 0; depth++) {
        if (cur == (Window)ancestor) { found = 1; break; }
        if (!XQueryTree(d, cur, &root, &parent, &kids, &nkids)) break;
        if (kids) XFree(kids);
        if (parent == root || parent == cur) break;
        cur = parent;
    }
    XCloseDisplay(d);
    return found;
}

int kembed_shape_cutout(unsigned long xid, int w, int h, int hx, int hy, int hw, int hh, int *out_rects, int max_out) {
    XSetErrorHandler(ignore_xerr);
    Display *d = XOpenDisplay(NULL);
    if (!d) return -1;
    int x1 = hx < 0 ? 0 : hx;
    int y1 = hy < 0 ? 0 : hy;
    int x2 = (hx + hw) > w ? w : (hx + hw);
    int y2 = (hy + hh) > h ? h : (hy + hh);

    XRectangle rects[4];
    int nr = 0;
    if (x1 >= x2 || y1 >= y2) {
        rects[0].x = 0; rects[0].y = 0; rects[0].width = (unsigned short)w; rects[0].height = (unsigned short)h;
        nr = 1;
    } else {
        if (y1 > 0) {
            rects[nr].x = 0; rects[nr].y = 0; rects[nr].width = (unsigned short)w; rects[nr].height = (unsigned short)y1;
            nr++;
        }
        if (y2 < h) {
            rects[nr].x = 0; rects[nr].y = (short)y2; rects[nr].width = (unsigned short)w; rects[nr].height = (unsigned short)(h - y2);
            nr++;
        }
        if (x1 > 0 && y2 > y1) {
            rects[nr].x = 0; rects[nr].y = (short)y1; rects[nr].width = (unsigned short)x1; rects[nr].height = (unsigned short)(y2 - y1);
            nr++;
        }
        if (x2 < w && y2 > y1) {
            rects[nr].x = (short)x2; rects[nr].y = (short)y1; rects[nr].width = (unsigned short)(w - x2); rects[nr].height = (unsigned short)(y2 - y1);
            nr++;
        }
    }
    XShapeCombineRectangles(d, (Window)xid, ShapeBounding, 0, 0, rects, nr, ShapeSet, Unsorted);
    XSync(d, False);

    int count = 0, ordering = 0;
    XRectangle *got = XShapeGetRectangles(d, (Window)xid, ShapeBounding, &count, &ordering);
    if (got) {
        for (int i = 0; i < count && i < max_out; i++) {
            out_rects[i * 4 + 0] = got[i].x;
            out_rects[i * 4 + 1] = got[i].y;
            out_rects[i * 4 + 2] = got[i].width;
            out_rects[i * 4 + 3] = got[i].height;
        }
        XFree(got);
    }
    XCloseDisplay(d);
    return count;
}

int kembed_shape_reset(unsigned long xid) {
    XSetErrorHandler(ignore_xerr);
    Display *d = XOpenDisplay(NULL);
    if (!d) return -1;
    XShapeCombineMask(d, (Window)xid, ShapeBounding, 0, 0, None, ShapeSet);
    XSync(d, False);
    int count = 0, ordering = 0;
    XRectangle *got = XShapeGetRectangles(d, (Window)xid, ShapeBounding, &count, &ordering);
    if (got) XFree(got);
    XCloseDisplay(d);
    return count;
}

int kembed_set_transient(unsigned long dlg_xid, unsigned long parent_xid) {
    XSetErrorHandler(ignore_xerr);
    Display *d = XOpenDisplay(NULL);
    if (!d) return 0;
    XSetTransientForHint(d, (Window)dlg_xid, (Window)parent_xid);
    XRaiseWindow(d, (Window)dlg_xid);
    XSync(d, False);
    XCloseDisplay(d);
    return 1;
}

unsigned long kembed_get_transient(unsigned long dlg_xid) {
    XSetErrorHandler(ignore_xerr);
    Display *d = XOpenDisplay(NULL);
    if (!d) return 0;
    Window prop = 0;
    if (!XGetTransientForHint(d, (Window)dlg_xid, &prop)) prop = 0;
    XCloseDisplay(d);
    return (unsigned long)prop;
}

int kembed_grab_wait_chord(unsigned long host_xid, const char *keysym_name, int timeout_ms) {
    XSetErrorHandler(ignore_xerr);
    Display *d = XOpenDisplay(NULL);
    if (!d) return -1;
    KeySym ks = XStringToKeysym(keysym_name);
    if (ks == NoSymbol) { XCloseDisplay(d); return -2; }
    KeyCode kc = XKeysymToKeycode(d, ks);
    if (!kc) { XCloseDisplay(d); return -3; }

    unsigned int mods[4] = { ControlMask, ControlMask | Mod2Mask, ControlMask | LockMask, ControlMask | Mod2Mask | LockMask };
    for (int i = 0; i < 4; i++) {
        XGrabKey(d, kc, mods[i], (Window)host_xid, True, GrabModeAsync, GrabModeAsync);
    }
    XSelectInput(d, (Window)host_xid, KeyPressMask | KeyReleaseMask);
    XSync(d, False);

    int elapsed = 0;
    int got = 0;
    while (elapsed < timeout_ms) {
        XEvent ev;
        if (XCheckMaskEvent(d, KeyPressMask, &ev)) {
            if (ev.xkey.keycode == kc && (ev.xkey.state & ControlMask)) {
                got = 1;
                break;
            }
        }
        usleep(15000);
        elapsed += 15;
    }
    for (int i = 0; i < 4; i++) {
        XUngrabKey(d, kc, mods[i], (Window)host_xid);
    }
    XSync(d, False);
    XCloseDisplay(d);
    return got;
}

int kembed_win_info(
    unsigned long xid,
    unsigned long *parent_out,
    unsigned long *pid_out,
    unsigned long *transient_out,
    int *geom_out, /* x, y, root_x, root_y, w, h, depth, viewable, override_redirect, root_depth */
    int *painted_out,
    char *title_buf, int title_cap,
    char *class_buf, int class_cap
) {
    XSetErrorHandler(ignore_xerr);
    Display *d = XOpenDisplay(NULL);
    if (!d) return 0;
    Window w = (Window)xid;
    XWindowAttributes wa;
    if (!XGetWindowAttributes(d, w, &wa)) {
        XCloseDisplay(d);
        return 0;
    }
    Window root = 0, parent = 0, *kids = NULL;
    unsigned int nkids = 0;
    if (XQueryTree(d, w, &root, &parent, &kids, &nkids)) {
        if (kids) XFree(kids);
    }
    *parent_out = (unsigned long)parent;

    Window dummy = 0;
    int rx = 0, ry = 0;
    XTranslateCoordinates(d, w, DefaultRootWindow(d), 0, 0, &rx, &ry, &dummy);
    geom_out[0] = wa.x;
    geom_out[1] = wa.y;
    geom_out[2] = rx;
    geom_out[3] = ry;
    geom_out[4] = wa.width;
    geom_out[5] = wa.height;
    geom_out[6] = wa.depth;
    geom_out[7] = (wa.map_state == IsViewable) ? 1 : 0;
    geom_out[8] = wa.override_redirect ? 1 : 0;
    geom_out[9] = DefaultDepth(d, DefaultScreen(d));

    /* _NET_WM_PID */
    *pid_out = 0;
    Atom a_pid = XInternAtom(d, "_NET_WM_PID", True);
    if (a_pid != None) {
        Atom actual; int fmt; unsigned long nitems, after; unsigned char *prop = NULL;
        if (XGetWindowProperty(d, w, a_pid, 0, 1, False, XA_CARDINAL, &actual, &fmt, &nitems, &after, &prop) == Success && prop) {
            if (nitems >= 1 && fmt == 32) *pid_out = *(unsigned long *)prop;
            XFree(prop);
        }
    }

    Window tr = 0;
    if (!XGetTransientForHint(d, w, &tr)) tr = 0;
    *transient_out = (unsigned long)tr;

    /* Title: _NET_WM_NAME (UTF8_STRING) then WM_NAME */
    title_buf[0] = '\0';
    Atom a_net_name = XInternAtom(d, "_NET_WM_NAME", True);
    Atom a_utf8 = XInternAtom(d, "UTF8_STRING", True);
    if (a_net_name != None && a_utf8 != None) {
        Atom actual; int fmt; unsigned long nitems, after; unsigned char *prop = NULL;
        if (XGetWindowProperty(d, w, a_net_name, 0, 256, False, a_utf8, &actual, &fmt, &nitems, &after, &prop) == Success && prop) {
            snprintf(title_buf, (size_t)title_cap, "%.*s", (int)nitems, (const char *)prop);
            XFree(prop);
        }
    }
    if (title_buf[0] == '\0') {
        char *wm_name = NULL;
        if (XFetchName(d, w, &wm_name) && wm_name) {
            snprintf(title_buf, (size_t)title_cap, "%s", wm_name);
            XFree(wm_name);
        }
    }

    /* WM_CLASS */
    class_buf[0] = '\0';
    XClassHint ch;
    if (XGetClassHint(d, w, &ch)) {
        snprintf(class_buf, (size_t)class_cap, "%s:%s", ch.res_name ? ch.res_name : "", ch.res_class ? ch.res_class : "");
        if (ch.res_name) XFree(ch.res_name);
        if (ch.res_class) XFree(ch.res_class);
    }

    /* Sample painted pixels from composited root screen over viewable bounds (avoids BadMatch on clipped subwindows & reads child GLX canvases) */
    if (*painted_out >= 0) {
        int painted = 0;
        int scr = DefaultScreen(d);
        int disp_w = DisplayWidth(d, scr);
        int disp_h = DisplayHeight(d, scr);
        int x1 = rx < 0 ? 0 : rx;
        int y1 = ry < 0 ? 0 : ry;
        int x2 = (rx + wa.width) > disp_w ? disp_w : (rx + wa.width);
        int y2 = (ry + wa.height) > disp_h ? disp_h : (ry + wa.height);
        if (parent != 0 && parent != DefaultRootWindow(d)) {
            XWindowAttributes pwa;
            Window pdummy = 0;
            int prx = 0, pry = 0;
            if (XGetWindowAttributes(d, parent, &pwa) && XTranslateCoordinates(d, parent, DefaultRootWindow(d), 0, 0, &prx, &pry, &pdummy)) {
                if (pwa.map_state != IsViewable) { x2 = x1; }
                if (prx > x1) x1 = prx;
                if (pry > y1) y1 = pry;
                if (prx + pwa.width < x2) x2 = prx + pwa.width;
                if (pry + pwa.height < y2) y2 = pry + pwa.height;
            }
        }
        int sw = x2 - x1;
        int sh = y2 - y1;
        if (wa.map_state == IsViewable && sw > 24 && sh > 24) {
            if (sw > 900) sw = 900;
            if (sh > 700) sh = 700;
            XImage *img = XGetImage(d, DefaultRootWindow(d), x1, y1, (unsigned int)sw, (unsigned int)sh, AllPlanes, ZPixmap);
            if (img) {
                unsigned int seen[8];
                int nseen = 0;
                for (int py = 12; py < sh - 12; py += 19) {
                    for (int px = 12; px < sw - 12; px += 23) {
                        unsigned long p = XGetPixel(img, px, py);
                        unsigned int r = (p >> 16) & 0xff;
                        unsigned int g = (p >> 8) & 0xff;
                        unsigned int b = p & 0xff;
                        int is_placeholder = (r <= 12 && g >= 8 && g <= 28 && b >= 20 && b <= 52);
                        if (!is_placeholder && (r > 12 || g > 12 || b > 12) && (r < 245 || g < 245 || b < 245)) painted++;
                        unsigned int q = ((r >> 4) << 8) | ((g >> 4) << 4) | (b >> 4);
                        int have = 0;
                        for (int k = 0; k < nseen; k++) { if (seen[k] == q) { have = 1; break; } }
                        if (!have && nseen < 8) seen[nseen++] = q;
                    }
                }
                if (nseen < 4) painted = 0;
                XDestroyImage(img);
            }
        }
        *painted_out = painted;
    }

    XCloseDisplay(d);
    return 1;
}

int kembed_list_tree(unsigned long *out_xids, int max_out) {
    XSetErrorHandler(ignore_xerr);
    Display *d = XOpenDisplay(NULL);
    if (!d) return 0;
    Window stack[2048];
    int sp = 0, count = 0;
    stack[sp++] = DefaultRootWindow(d);
    while (sp > 0 && count < max_out) {
        Window cur = stack[--sp];
        out_xids[count++] = (unsigned long)cur;
        Window root = 0, parent = 0, *kids = NULL;
        unsigned int nkids = 0;
        if (XQueryTree(d, cur, &root, &parent, &kids, &nkids) && kids) {
            for (unsigned int i = 0; i < nkids && sp < 2048; i++) {
                stack[sp++] = kids[i];
            }
            XFree(kids);
        }
    }
    XCloseDisplay(d);
    return count;
}
"""


def load_lib() -> ctypes.CDLL:
    bp_tmp = os.environ.get("BP_TMP") or os.path.expanduser("~/.cache/bp-tmp")
    os.makedirs(bp_tmp, exist_ok=True)
    digest = hashlib.sha256(C_SOURCE.encode("utf-8")).hexdigest()[:12]
    so_path = os.path.join(bp_tmp, f"kembed_x11_{digest}.so")
    if not os.path.isfile(so_path):
        c_path = os.path.join(bp_tmp, f"kembed_x11_{digest}.{os.getpid()}.c")
        tmp_so = os.path.join(bp_tmp, f"kembed_x11_{digest}.{os.getpid()}.so")
        with open(c_path, "w", encoding="utf-8") as f:
            f.write(C_SOURCE)
        try:
            subprocess.run(
                ["gcc", "-shared", "-fPIC", "-O2", c_path, "-o", tmp_so, "-lX11", "-lXext"],
                check=True,
            )
            os.replace(tmp_so, so_path)
        finally:
            if os.path.exists(c_path):
                os.remove(c_path)
            if os.path.exists(tmp_so):
                os.remove(tmp_so)
    lib = ctypes.CDLL(so_path)
    lib.kembed_container_daemon.argtypes = [ctypes.c_ulong, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_char_p]
    lib.kembed_container_daemon.restype = ctypes.c_int
    lib.kembed_embed.argtypes = [ctypes.c_ulong, ctypes.c_ulong, ctypes.c_int, ctypes.c_int]
    lib.kembed_embed.restype = ctypes.c_int
    lib.kembed_withdraw.argtypes = [ctypes.c_ulong]
    lib.kembed_withdraw.restype = ctypes.c_int
    lib.kembed_place.argtypes = [ctypes.c_ulong, ctypes.c_ulong, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int]
    lib.kembed_place.restype = ctypes.c_int
    lib.kembed_notify_configure.argtypes = [ctypes.c_ulong]
    lib.kembed_notify_configure.restype = ctypes.c_int
    lib.kembed_show.argtypes = [ctypes.c_ulong, ctypes.c_int]
    lib.kembed_show.restype = ctypes.c_int
    lib.kembed_switch.argtypes = [ctypes.c_ulong, ctypes.POINTER(ctypes.c_ulong), ctypes.c_int]
    lib.kembed_switch.restype = ctypes.c_int
    lib.kembed_focus.argtypes = [ctypes.c_ulong]
    lib.kembed_focus.restype = ctypes.c_ulong
    lib.kembed_is_descendant.argtypes = [ctypes.c_ulong, ctypes.c_ulong]
    lib.kembed_is_descendant.restype = ctypes.c_int
    lib.kembed_shape_cutout.argtypes = [ctypes.c_ulong, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.POINTER(ctypes.c_int), ctypes.c_int]
    lib.kembed_shape_cutout.restype = ctypes.c_int
    lib.kembed_shape_reset.argtypes = [ctypes.c_ulong]
    lib.kembed_shape_reset.restype = ctypes.c_int
    lib.kembed_set_transient.argtypes = [ctypes.c_ulong, ctypes.c_ulong]
    lib.kembed_set_transient.restype = ctypes.c_int
    lib.kembed_get_transient.argtypes = [ctypes.c_ulong]
    lib.kembed_get_transient.restype = ctypes.c_ulong
    lib.kembed_grab_wait_chord.argtypes = [ctypes.c_ulong, ctypes.c_char_p, ctypes.c_int]
    lib.kembed_grab_wait_chord.restype = ctypes.c_int
    lib.kembed_win_info.argtypes = [
        ctypes.c_ulong,
        ctypes.POINTER(ctypes.c_ulong),
        ctypes.POINTER(ctypes.c_ulong),
        ctypes.POINTER(ctypes.c_ulong),
        ctypes.POINTER(ctypes.c_int),
        ctypes.POINTER(ctypes.c_int),
        ctypes.c_char_p, ctypes.c_int,
        ctypes.c_char_p, ctypes.c_int,
    ]
    lib.kembed_win_info.restype = ctypes.c_int
    lib.kembed_list_tree.argtypes = [ctypes.POINTER(ctypes.c_ulong), ctypes.c_int]
    lib.kembed_list_tree.restype = ctypes.c_int
    return lib


def parse_xid(s: str) -> int:
    return int(s, 0)


def fmt_xid(x: int) -> str:
    return f"0x{x:x}"


def win_info(lib: ctypes.CDLL, xid: int, sample_painted: bool = False) -> dict | None:
    parent = ctypes.c_ulong(0)
    pid = ctypes.c_ulong(0)
    transient = ctypes.c_ulong(0)
    geom = (ctypes.c_int * 10)()
    painted = ctypes.c_int(0 if sample_painted else -1)
    tbuf = ctypes.create_string_buffer(512)
    cbuf = ctypes.create_string_buffer(256)
    if not lib.kembed_win_info(xid, ctypes.byref(parent), ctypes.byref(pid), ctypes.byref(transient), geom, ctypes.byref(painted), tbuf, 512, cbuf, 256):
        return None
    d = {
        "xid": fmt_xid(xid),
        "xid_int": xid,
        "parent": fmt_xid(parent.value),
        "pid": pid.value,
        "transient_for": fmt_xid(transient.value) if transient.value else "",
        "x": geom[0],
        "y": geom[1],
        "root_x": geom[2],
        "root_y": geom[3],
        "w": geom[4],
        "h": geom[5],
        "depth": geom[6],
        "shown": bool(geom[7]),
        "override_redirect": bool(geom[8]),
        "root_depth": geom[9],
        "title": tbuf.value.decode("utf-8", errors="replace"),
        "wm_class": cbuf.value.decode("utf-8", errors="replace"),
    }
    if sample_painted:
        d["painted"] = painted.value
    return d


def list_all_windows(lib: ctypes.CDLL) -> list[dict]:
    cap = 2048
    arr = (ctypes.c_ulong * cap)()
    n = lib.kembed_list_tree(arr, cap)
    out = []
    for i in range(n):
        info = win_info(lib, arr[i], sample_painted=False)
        if info:
            out.append(info)
    return out


def compute_pane_rect(gw: int, gh: int, client_log: str | None, sidebar_folded: bool = False) -> dict:
    # Read GPUI_RECT lines if available; match against current window width gw.
    rects: dict[str, dict] = {}
    if client_log and os.path.isfile(client_log):
        with open(client_log, encoding="utf-8", errors="ignore") as f:
            for line in f:
                if line.startswith("GPUI_RECT "):
                    try:
                        obj = json.loads(line[len("GPUI_RECT "):])
                        rects[obj["name"]] = obj
                    except Exception:
                        pass
    rail = rects.get("rail")
    main_r = rects.get("main")
    if rail and main_r and abs((rail["x"] + rail["w"]) - gw) <= 4:
        vx = int(round(main_r["x"] + main_r["w"]))
        rx = int(round(rail["x"]))
        vw = rx - vx
        if vw >= 200:
            vy = int(round(rail["y"])) + 36
            vh = max(120, int(round(rail["h"])) - 36)
            return {"x": vx, "y": vy, "w": vw, "h": vh, "source": "gpui_rect"}

    # Deterministic fallback when viewer isn't open or rects didn't match after toggle:
    side_w = 44 if sidebar_folded else 240
    rx = max(400, gw - 72)
    vx = side_w + (rx - side_w) // 2
    vy = 72
    return {"x": vx, "y": vy, "w": max(200, rx - vx), "h": max(120, gh - vy), "source": "computed"}


def main() -> None:
    if len(sys.argv) < 2:
        sys.exit("usage: x11_embed_driver.py <cmd> [args...]")
    cmd = sys.argv[1]
    args = sys.argv[2:]
    lib = load_lib()

    if cmd == "container-daemon":
        parent = parse_xid(args[0])
        x, y, w, h = map(int, args[1:5])
        ready = args[5].encode("utf-8")
        rc = lib.kembed_container_daemon(parent, x, y, w, h, ready)
        sys.exit(rc)

    if cmd == "find-gpui":
        client_log = None
        folded = False
        as_shell = False
        i = 0
        while i < len(args):
            if args[i] == "--client-log" and i + 1 < len(args):
                client_log = args[i + 1]
                i += 2
            elif args[i] == "--folded":
                folded = True
                i += 1
            elif args[i] == "--shell":
                as_shell = True
                i += 1
            else:
                i += 1
        wins = list_all_windows(lib)
        gpui = None
        for w in wins:
            if w["title"].startswith("Backplane GPUI") and w["w"] >= 300 and w["h"] >= 200:
                gpui = w
                break
        if not gpui:
            sys.exit(1)
        pane = compute_pane_rect(gpui["w"], gpui["h"], client_log, sidebar_folded=folded)
        if as_shell:
            print(
                f"GPUI_XID={gpui['xid']} GX={gpui['root_x']} GY={gpui['root_y']} "
                f"GW={gpui['w']} GH={gpui['h']} GPUI_DEPTH={gpui['depth']} ROOT_DEPTH={gpui['root_depth']} "
                f"VX={pane['x']} VY={pane['y']} VW={pane['w']} VH={pane['h']}"
            )
            return
        print(
            json.dumps(
                {
                    "gpui_xid": gpui["xid"],
                    "gx": gpui["root_x"],
                    "gy": gpui["root_y"],
                    "gw": gpui["w"],
                    "gh": gpui["h"],
                    "gpui_depth": gpui["depth"],
                    "root_depth": gpui["root_depth"],
                    "pane": pane,
                }
            )
        )
        return

    if cmd == "list-windows":
        want_pid = None
        match_sub = None
        only_popup = False
        i = 0
        while i < len(args):
            if args[i] == "--pid" and i + 1 < len(args):
                want_pid = int(args[i + 1])
                i += 2
            elif args[i] == "--match" and i + 1 < len(args):
                match_sub = args[i + 1].lower()
                i += 2
            elif args[i] == "--popup":
                only_popup = True
                i += 1
            else:
                i += 1
        wins = list_all_windows(lib)
        filtered = []
        for w in wins:
            if want_pid is not None and w["pid"] != want_pid:
                # Override-redirect GTK menus sometimes omit _NET_WM_PID; allow matching wm_class kicad when --popup
                if not (only_popup and ("kicad" in w["wm_class"].lower() or "pcbnew" in w["wm_class"].lower() or "eeschema" in w["wm_class"].lower())):
                    continue
            if only_popup and not (w["override_redirect"] and w["shown"] and w["w"] >= 40 and w["h"] >= 20):
                continue
            if match_sub and match_sub not in w["title"].lower() and match_sub not in w["wm_class"].lower():
                continue
            if not only_popup and not w["title"] and not w["wm_class"]:
                continue
            filtered.append(w)
        print(json.dumps(filtered))
        return

    if cmd == "embed":
        c_xid = parse_xid(args[0])
        ch_xid = parse_xid(args[1])
        w, h = int(args[2]), int(args[3])
        ok = bool(lib.kembed_embed(c_xid, ch_xid, w, h))
        info = win_info(lib, ch_xid, sample_painted=True) or {}
        print(json.dumps({"event": "kicad", "op": "embed", "ok": ok, "container_xid": fmt_xid(c_xid), "native": info}))
        sys.exit(0 if ok else 1)

    if cmd == "place":
        c_xid = parse_xid(args[0])
        ch_xid = parse_xid(args[1])
        x, y, w, h = map(int, args[2:6])
        send_cfg = 0 if "--no-configure" in args[6:] else 1
        ok = bool(lib.kembed_place(c_xid, ch_xid, x, y, w, h, send_cfg))
        info = win_info(lib, ch_xid, sample_painted=False) if ch_xid else {}
        print(json.dumps({"ok": ok, "native": info}))
        sys.exit(0 if ok else 1)

    if cmd == "send-configure":
        ch_xid = parse_xid(args[0])
        ok = bool(lib.kembed_notify_configure(ch_xid))
        info = win_info(lib, ch_xid, sample_painted=False) or {}
        print(json.dumps({"ok": ok, "native": info}))
        sys.exit(0 if ok else 1)

    if cmd == "show":
        xid = parse_xid(args[0])
        on = int(args[1])
        ok = bool(lib.kembed_show(xid, on))
        print(json.dumps({"ok": ok, "xid": fmt_xid(xid), "shown": bool(on)}))
        return

    if cmd == "withdraw":
        xid = parse_xid(args[0])
        ok = bool(lib.kembed_withdraw(xid))
        print(json.dumps({"ok": ok, "xid": fmt_xid(xid)}))
        return

    if cmd == "switch":
        show_xid = parse_xid(args[0])
        hides = [parse_xid(x) for x in args[1:]]
        arr = (ctypes.c_ulong * max(1, len(hides)))(*hides)
        t0 = time.monotonic()
        ok = bool(lib.kembed_switch(show_xid, arr, len(hides)))
        dt_ms = (time.monotonic() - t0) * 1000.0
        info = win_info(lib, show_xid, sample_painted=False) or {}
        print(json.dumps({"ok": ok, "elapsed_ms": round(dt_ms, 2), "shown": info.get("shown", False), "xid": fmt_xid(show_xid)}))
        return

    if cmd == "focus":
        target = parse_xid(args[0]) if args else 0
        cur = lib.kembed_focus(target)
        print(json.dumps({"focus": fmt_xid(cur)}))
        return

    if cmd == "shape-cutout":
        xid = parse_xid(args[0])
        w, h, hx, hy, hw, hh = map(int, args[1:7])
        buf = (ctypes.c_int * 64)()
        cnt = lib.kembed_shape_cutout(xid, w, h, hx, hy, hw, hh, buf, 16)
        rects = []
        for i in range(min(max(cnt, 0), 16)):
            rects.append({"x": buf[i * 4], "y": buf[i * 4 + 1], "w": buf[i * 4 + 2], "h": buf[i * 4 + 3]})
        print(json.dumps({"rect_count": cnt, "rects": rects}))
        return

    if cmd == "shape-reset":
        xid = parse_xid(args[0])
        cnt = lib.kembed_shape_reset(xid)
        print(json.dumps({"rect_count": cnt}))
        return

    if cmd == "set-transient":
        dlg, parent = parse_xid(args[0]), parse_xid(args[1])
        ok = bool(lib.kembed_set_transient(dlg, parent))
        got = lib.kembed_get_transient(dlg)
        info = win_info(lib, dlg, sample_painted=False) or {}
        print(json.dumps({"ok": ok, "transient_for": fmt_xid(got), "dialog": info}))
        return

    if cmd == "get-transient":
        dlg = parse_xid(args[0])
        got = lib.kembed_get_transient(dlg)
        info = win_info(lib, dlg, sample_painted=False) or {}
        print(json.dumps({"transient_for": fmt_xid(got), "dialog": info}))
        return

    if cmd == "grab-chord-probe":
        host_xid = parse_xid(args[0])
        keysym = args[1]
        xinput_py = args[2]
        # Spawn delayed XTest chord while kembed_grab_wait_chord listens with passive XGrabKey
        proc = subprocess.Popen(
            [sys.executable, "-c", f"import subprocess,sys,time; time.sleep(0.18); subprocess.run([sys.executable, {xinput_py!r}, 'chord', 'ctrl', {keysym!r}], check=True)"],
        )
        t0 = time.monotonic()
        got = lib.kembed_grab_wait_chord(host_xid, keysym.encode("utf-8"), 2500)
        dt_ms = (time.monotonic() - t0) * 1000.0
        proc.wait(timeout=5)
        print(json.dumps({"intercepted": got == 1, "rc": got, "elapsed_ms": round(dt_ms, 1)}))
        sys.exit(0 if got == 1 else 1)

    if cmd == "report":
        gpui_xid = parse_xid(args[0])
        c_xid = parse_xid(args[1])
        ch_xid = parse_xid(args[2])
        c_info = win_info(lib, c_xid, sample_painted=False) or {}
        ch_info = win_info(lib, ch_xid, sample_painted=True) or {}
        cur_focus = lib.kembed_focus(0)
        taken = bool(lib.kembed_is_descendant(c_xid, cur_focus))
        in_gpui = bool(lib.kembed_is_descendant(gpui_xid, ch_xid))
        in_cont = bool(lib.kembed_is_descendant(c_xid, ch_xid))
        print(
            json.dumps(
                {
                    "event": "kicad",
                    "gpui_xid": fmt_xid(gpui_xid),
                    "container_xid": fmt_xid(c_xid),
                    "container": c_info,
                    "in_gpui_tree": in_gpui,
                    "in_container": in_cont,
                    "taken": taken,
                    "focus_in_child": taken,
                    "focus_xid": fmt_xid(cur_focus),
                    "native": ch_info,
                }
            )
        )
        return

    if cmd == "img-diff":
        pa, pb = args[0], args[1]
        im_a = Image.open(pa).convert("RGB")
        im_b = Image.open(pb).convert("RGB")
        if len(args) >= 6:
            x, y, w, h = map(int, args[2:6])
            im_a = im_a.crop((x, y, x + w, y + h))
            im_b = im_b.crop((x, y, x + w, y + h))
        if im_a.size != im_b.size:
            im_b = im_b.resize(im_a.size)
        ba = im_a.tobytes()
        bb = im_b.tobytes()
        total_px = im_a.size[0] * im_a.size[1]
        changed = 0
        diff_sum = 0
        for i in range(0, len(ba), 3):
            dr = abs(ba[i] - bb[i])
            dg = abs(ba[i + 1] - bb[i + 1])
            db = abs(ba[i + 2] - bb[i + 2])
            diff_sum += dr + dg + db
            if dr > 8 or dg > 8 or db > 8:
                changed += 1
        print(
            json.dumps(
                {
                    "changed_pixels": changed,
                    "total_pixels": total_px,
                    "changed_ratio": round(changed / max(1, total_px), 5),
                    "mean_abs_diff": round(diff_sum / max(1, total_px * 3), 3),
                }
            )
        )
        return

    sys.exit(f"unknown command: {cmd}")


if __name__ == "__main__":
    main()
