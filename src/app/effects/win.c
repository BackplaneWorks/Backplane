// Backplane window (X11)
// ======================
//
// Base's Window is fixed-size and reports no text, wheel, resize or
// clipboard, so the app has its own. libX11 is opened at run time (no link
// flags, and a machine without X still runs the server). A frame is the
// Bend Image quadtree; present() fills each solid region as one rectangle
// and sends the X server only the 64 px blocks whose pixels changed.
//
// Events are five words (kind, a, b, c, d):
//   0 key (keysym, code point typed, mods, down)   1 button (x, y, button, down)
//   2 move (x, y, mods)   3 wheel (x, y, dir)   4 size (w, h)   5 expose
//   6 quit   7 paste (text via Win.pasted)
//   8 drag over (x, y)   9 drag left   10 drop (text/uri-list via Win.dropped)
//   11 focus lost
// mods: 1 shift, 2 ctrl, 4 alt, 8 super.

#if defined(__linux__)

#include <dlfcn.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <X11/Xlib.h>

/* popen is POSIX. These units are compiled as -std=c11, and this file is
   included after other headers, so a feature-test macro would be too late. */
extern FILE* popen(const char*, const char*);
extern int pclose(FILE*);
#include <X11/Xutil.h>
#include <X11/Xatom.h>
#include <X11/keysym.h>

#define WIN_FNS(X) \
  X(XOpenDisplay, Display*, (const char*)) \
  X(XCloseDisplay, int, (Display*)) \
  X(XCreateSimpleWindow, Window, (Display*, Window, int, int, unsigned, unsigned, unsigned, unsigned long, unsigned long)) \
  X(XInternAtom, Atom, (Display*, const char*, Bool)) \
  X(XSetWMProtocols, Status, (Display*, Window, Atom*, int)) \
  X(XStoreName, int, (Display*, Window, const char*)) \
  X(XSelectInput, int, (Display*, Window, long)) \
  X(XMapRaised, int, (Display*, Window)) \
  X(XFlush, int, (Display*)) \
  X(XPending, int, (Display*)) \
  X(XEventsQueued, int, (Display*, int)) \
  X(XNextEvent, int, (Display*, XEvent*)) \
  X(XQueryKeymap, int, (Display*, char[32])) \
  X(XLookupString, int, (XKeyEvent*, char*, int, KeySym*, XComposeStatus*)) \
  X(XCreateImage, XImage*, (Display*, Visual*, unsigned, int, int, char*, unsigned, unsigned, int, int)) \
  X(XPutImage, int, (Display*, Drawable, GC, XImage*, int, int, int, int, unsigned, unsigned)) \
  X(XSetSelectionOwner, int, (Display*, Atom, Window, Time)) \
  X(XGetSelectionOwner, Window, (Display*, Atom)) \
  X(XConvertSelection, int, (Display*, Atom, Atom, Atom, Window, Time)) \
  X(XGetWindowProperty, int, (Display*, Window, Atom, long, long, Bool, Atom, Atom*, int*, unsigned long*, unsigned long*, unsigned char**)) \
  X(XChangeProperty, int, (Display*, Window, Atom, Atom, int, int, const unsigned char*, int)) \
  X(XSendEvent, Status, (Display*, Window, Bool, long, XEvent*)) \
  X(XFree, int, (void*)) \
  X(XDeleteProperty, int, (Display*, Window, Atom)) \
  X(XTranslateCoordinates, Bool, (Display*, Window, Window, int, int, int*, int*, Window*))

#define WIN_PTR(name, ret, args) static ret (*x_##name) args;
WIN_FNS(WIN_PTR)

// a key held down sends presses with no releases between (so a repeat can
// be told from a new press); a server without it sends a release and a
// press of the same key at the same time for each repeat (win_pump asks the
// server whether the key is still down and, if so, drops the release)
static Bool (*x_detect_repeat)(Display*, Bool, Bool*);

static int win_load(void) {
  static int state = 0;
  if (state != 0) {
    return state > 0;
  }
  void* lib = dlopen("libX11.so.6", RTLD_NOW | RTLD_LOCAL);
  state = -1;
  if (lib == NULL) {
    return 0;
  }
#define WIN_SYM(name, ret, args) \
  if ((x_##name = (ret (*) args)dlsym(lib, #name)) == NULL) { return 0; }
  WIN_FNS(WIN_SYM)
  x_detect_repeat = (Bool (*)(Display*, Bool, Bool*))dlsym(lib, "XkbSetDetectableAutoRepeat");
  state = 1;
  return 1;
}

typedef struct {
  Display*  dpy;
  Window    win;
  Atom      del, clip, utf8, targets, prop, incr;
  XImage*   img;
  // w, h are the size the app lays out. pw, ph are the X window. Hyprland's
  // xwayland.force_zero_scaling leaves an X11 window unscaled by the
  // compositor; scale (BACKPLANE_UI_SCALE, else the focused monitor) draws
  // the same layout into a larger window. scale == 1 keeps pw == w.
  XImage*   scaled;
  u32       w, h;
  u32       pw, ph;
  double    scale;
  // present: the blocks (64 px) a fill changed, and whether the next
  // present must send the whole frame (a new image, or an expose)
  u8*       dirty;
  u32       dw, dh;
  _Atomic int full;
  // the buffer holds something other than the last shown frame (a
  // lightbox, a new size): the next show fills it whole
  int       stale;
  u32       n, cap;
  u32*      evs;
  char*     copy;
  u64       copy_len;
  char*     paste;
  u64       paste_len;
  // a paste too big for one property arrives in pieces (INCR)
  int       paste_incr;
  // drag and drop (XDND)
  Atom      dnd_aware, dnd_enter, dnd_position, dnd_status, dnd_leave, dnd_drop,
            dnd_finished, dnd_sel, dnd_copy, dnd_uri, dnd_types, dnd_prop;
  Window    dnd_src;
  int       dnd_ok;
  char*     drop;
  u64       drop_len;
  // keys: whether the server sends repeats as bare presses, and the keysym
  // each key (by keycode) went down as (0: up), so its release says the same
  int       detect;
  KeySym    down[256];
} AppWin;

static void win_push(AppWin* a, u32 kind, u32 p, u32 q, u32 r, u32 s) {
  if (a->n == a->cap) {
    a->cap = a->cap == 0 ? 64 : a->cap * 2;
    a->evs = io_mem(realloc(a->evs, (size_t)a->cap * 20));
  }
  u32 ev[5] = { kind, p, q, r, s };
  memcpy(a->evs + (size_t)a->n * 5, ev, sizeof ev);
  a->n += 1;
}

static u32 win_mods(unsigned state) {
  return (state & ShiftMask ? 1u : 0) | (state & ControlMask ? 2u : 0)
    | (state & Mod1Mask ? 4u : 0) | (state & Mod4Mask ? 8u : 0);
}

// A usable monitor factor. Values near 1 draw 1:1, so the extra image is skipped.
static double win_scale_clamp(double v) {
  if (v < 0.5 || v > 4.0) {
    return 0;
  }
  if (v > 0.99 && v < 1.01) {
    return 1.0;
  }
  return v;
}

// BACKPLANE_UI_SCALE overrides. Otherwise the focused Hyprland monitor's
// scale is used. No Hyprland, or a bad value, leaves pixels at 1.
static double win_scale_env(void) {
  const char* s = getenv("BACKPLANE_UI_SCALE");
  char* end = NULL;
  double v;
  if (s == NULL || *s == 0) {
    return 0;
  }
  v = strtod(s, &end);
  if (end == s) {
    return 0;
  }
  return win_scale_clamp(v);
}

static double win_scale_hypr(void) {
  FILE* p;
  char line[512];
  double cur = 0, chosen = 0;
  int have = 0;
  p = popen("hyprctl monitors 2>/dev/null", "r");
  if (p == NULL) {
    return 0;
  }
  while (fgets(line, (int)sizeof line, p) != NULL) {
    double v;
    char* s = line;
    while (*s == ' ' || *s == '\t') {
      s += 1;
    }
    if (strncmp(s, "Monitor ", 8) == 0) {
      cur = 0;
    } else if (sscanf(s, "scale: %lf", &v) == 1) {
      cur = v;
    } else if (strncmp(s, "focused: yes", 12) == 0 && cur != 0) {
      chosen = cur;
      have = 1;
      break;
    }
  }
  pclose(p);
  return have ? win_scale_clamp(chosen) : 0;
}

static double win_ui_scale(void) {
  double v = win_scale_env();
  if (v == 0) {
    v = win_scale_hypr();
  }
  return v == 0 ? 1.0 : v;
}

static u32 win_phys(u32 logical, double scale) {
  double p = (double)logical * scale + 0.5;
  if (p < 1.0) {
    return 1;
  }
  if (p > 16384.0) {
    return 16384;
  }
  return (u32)p;
}

static u32 win_log_px(u32 physical, double scale) {
  double l = (double)physical / scale + 0.5;
  if (l < 1.0) {
    return 1;
  }
  return (u32)l;
}

// a physical window pixel as the layout pixel under it
static u32 win_to_log(u32 phys, u32 logical, u32 physical) {
  u32 v;
  if (physical <= 1 || logical == 0) {
    return 0;
  }
  v = (u32)((double)phys * (double)logical / (double)physical);
  return v >= logical ? logical - 1 : v;
}

// The window's icon (_NET_WM_ICON): the mark over #111410 at 64 and 32 px.
// Each row is the mark's coverage, one hex digit a pixel.
// icon rows (scripts/icons.sh)
static const char* win_icon_rows[64] = {
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "000000000000000000000000000000000000027b600000000000000000000000",
  "000000000000000000000000000000000038cfff600000000000000000000000",
  "000000000000000000000000000000049dffc74f600000000000000000000000",
  "00000000000000000000000000015aeffa61002f600000000000000000000000",
  "00000000000000000000000016aefea61000002f600000000000000000000000",
  "00000000000000000000027cffd840000000002f600000000000000000000000",
  "00000000000000000038cffc830000000000301e600000000000000000000000",
  "00000000000000049dffb720000000000000b902400000000000000000000000",
  "0000000000015aeffa610000000000000000bfc1000000000000000000000000",
  "00000000000afea510000000000180000000bffe300000000000000000000000",
  "00000000000ad000000000000001fb100000bffff60000000000000000000000",
  "00000000000ad000000200000001ffd20000bfffff9000000000000000000000",
  "00000000000ad0000007a1000001fffe5000bffffffb10000000000000000000",
  "00000000000ad0000007fc200001fffff700afffffffd3000000000000000000",
  "00000000000ad0000007ffe40001ffffffb12cffffffff600000000000000000",
  "00000000000ad0000007ffff7001fffffffd209ffffffff90000000000000000",
  "00000000000ad0000007fffff901dfffffffe406ffffffffb100000000000000",
  "00000000000ad0000007ffffffc11affffffff704efffffffd30000000000000",
  "00000000000ad0000007fffffffd307ffffffffa12cfffffffe5000000000000",
  "00000000000ad0000007fffffffff605efffffffc21affffffec600000000000",
  "00000000000ad0000007ffffffffff902dfffffffe406fffffc5a00000000000",
  "00000000000ad0000007fffffffffffb11bffffffff606ffffc5a00000000000",
  "00000000000ad0000007ffffffffffffd308ffffff9c34ffffc5a00000000000",
  "00000000000ad0000007ffffffffffffff605fffff6b44ffffc5a00000000000",
  "00000000000ad0000007ffffffffffffffd809ffff6b44ffffc5a00000000000",
  "00000000000ad0000007ffffffffffffff2d09ffff6b44ffffc5a00000000000",
  "00000000000ad0000007ffffffffffffff2d09ffff6b44ffffc5a00000000000",
  "00000000000ad00000009fffffffffffff2d09ffff6b44ffffc5a00000000000",
  "00000000000ad000000006ffffffffffff2d09ffff6b44ffffc5a00000000000",
  "00000000000ad0000000004effffffffff2d09ffff6b42dfffc5a00000000000",
  "00000000000ad0000027b902cfffffffff2d09ffff6b401bffc5a00000000000",
  "00000000000ad0038cffd8201affffffff2d09ffff6b40008fc5a00000000000",
  "00000000000ae8dffc720000007fffffff2d02cfff6b400005c5a00000000000",
  "00000000000affb6100000000004efffff2d0009ff6b40000025a00000000000",
  "00000000000651000000000000002dffff2d00006f6b40000001600000000000",
  "000000000000000000000000000001bfff2d0000045b40000000000000000000",
  "00000000000000000000000000000008ff2d0000000a40000000000000000000",
  "000000000000000000000000000000005e2d0000000110000000000000000000",
  "00000000000000000000000000000000032d0000000000000000000000000000",
  "0000000000000000000000000000000000190000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000000"
};
// icon rows end

static u32 win_icon_cov(int x, int y) {
  char c = win_icon_rows[y][x];
  return (u32)(c <= '9' ? c - '0' : c - 'a' + 10) * 17;
}

static unsigned long win_icon_argb(u32 c) {
  u32 r = (0x11 * (255 - c) + 0xe6 * c) / 255;
  u32 g = (0x14 * (255 - c) + 0xe9 * c) / 255;
  u32 b = (0x10 * (255 - c) + 0xde * c) / 255;
  return 0xff000000ul | (r << 16) | (g << 8) | b;
}

// both sizes in one property (width, height, then the pixels; a 32-bit
// property travels as longs), and WM_CLASS for the desktop file to match
static void win_icon(Display* dpy, Window win) {
  static unsigned long d[2 + 64 * 64 + 2 + 32 * 32];
  unsigned long* p = d;
  *p++ = 64;
  *p++ = 64;
  for (int y = 0; y < 64; y++) {
    for (int x = 0; x < 64; x++) {
      *p++ = win_icon_argb(win_icon_cov(x, y));
    }
  }
  *p++ = 32;
  *p++ = 32;
  for (int y = 0; y < 64; y += 2) {
    for (int x = 0; x < 64; x += 2) {
      *p++ = win_icon_argb((win_icon_cov(x, y) + win_icon_cov(x + 1, y)
        + win_icon_cov(x, y + 1) + win_icon_cov(x + 1, y + 1)) / 4);
    }
  }
  x_XChangeProperty(dpy, win, x_XInternAtom(dpy, "_NET_WM_ICON", False), XA_CARDINAL, 32,
    PropModeReplace, (unsigned char*)d, (int)(p - d));
  static const char cls[] = "backplane\0Backplane";
  x_XChangeProperty(dpy, win, XA_WM_CLASS, XA_STRING, 8, PropModeReplace,
    (const unsigned char*)cls, (int)sizeof cls);
}

// the code point a key types, or 0 (keysyms name Latin-1 directly and
// the rest of Unicode as 0x01000000 + code point)
static u32 win_text(XKeyEvent* ev, KeySym* sym) {
  char buf[16];
  XKeyEvent copy = *ev;
  copy.state &= ~(ControlMask | Mod1Mask | Mod4Mask);
  int n = x_XLookupString(&copy, buf, sizeof buf, sym, NULL);
  KeySym ks = *sym;
  if (ks >= 0x20 && ks <= 0xff) {
    return (u32)ks;
  }
  if (ks >= 0x01000100 && ks <= 0x0110ffff) {
    return (u32)(ks - 0x01000000);
  }
  if (n == 1 && (unsigned char)buf[0] >= 32 && buf[0] != 127) {
    return (unsigned char)buf[0];
  }
  return 0;
}

static void win_answer(AppWin* a, XSelectionRequestEvent* rq) {
  XEvent ev;
  memset(&ev, 0, sizeof ev);
  ev.xselection.type      = SelectionNotify;
  ev.xselection.requestor = rq->requestor;
  ev.xselection.selection = rq->selection;
  ev.xselection.target    = rq->target;
  ev.xselection.time      = rq->time;
  ev.xselection.property  = None;
  if (rq->target == a->targets) {
    Atom ok[2] = { a->utf8, XA_STRING };
    x_XChangeProperty(a->dpy, rq->requestor, rq->property, XA_ATOM, 32,
      PropModeReplace, (unsigned char*)ok, 2);
    ev.xselection.property = rq->property;
  } else if ((rq->target == a->utf8 || rq->target == XA_STRING) && a->copy) {
    x_XChangeProperty(a->dpy, rq->requestor, rq->property, rq->target, 8,
      PropModeReplace, (unsigned char*)a->copy, (int)a->copy_len);
    ev.xselection.property = rq->property;
  }
  x_XSendEvent(a->dpy, rq->requestor, False, 0, &ev);
}

static void win_paste_add(AppWin* a, unsigned char* data, unsigned long n) {
  a->paste = io_mem(realloc(a->paste, a->paste_len + n + 1));
  memcpy(a->paste + a->paste_len, data, n);
  a->paste_len += n;
}

// the clipboard's answer: the text whole, or the start of an INCR
// transfer whose pieces follow as PropertyNotify (win_paste_piece)
static void win_pasted(AppWin* a) {
  Atom type;
  int fmt;
  unsigned long n = 0, left = 0;
  unsigned char* data = NULL;
  if (x_XGetWindowProperty(a->dpy, a->win, a->prop, 0, 1 << 24, True,
    AnyPropertyType, &type, &fmt, &n, &left, &data) == Success && data) {
    free(a->paste);
    a->paste = NULL;
    a->paste_len = 0;
    a->paste_incr = type == a->incr;
    if (!a->paste_incr) {
      win_paste_add(a, data, n);
      win_push(a, 7, 0, 0, 0, 0);
    }
    x_XFree(data);
  }
}

// one piece of an INCR paste; the empty piece ends it
static void win_paste_piece(AppWin* a) {
  Atom type;
  int fmt;
  unsigned long n = 0, left = 0;
  unsigned char* data = NULL;
  if (x_XGetWindowProperty(a->dpy, a->win, a->prop, 0, 1 << 24, True,
    AnyPropertyType, &type, &fmt, &n, &left, &data) != Success) {
    return;
  }
  if (n > 0 && data) {
    win_paste_add(a, data, n);
  } else {
    a->paste_incr = 0;
    win_push(a, 7, 0, 0, 0, 0);
  }
  if (data) {
    x_XFree(data);
  }
}

// Drag and drop (XDND 5). A file manager's drag offers text/uri-list;
// while one hovers the app hears 8 (x, y), when it leaves 9, and when it
// lands 10, its list read like a paste (Win.dropped). Drags without files
// are refused and never reach the app.
static void win_dnd_send(AppWin* a, Atom type, long l1, long l2, long l3, long l4) {
  XEvent ev;
  memset(&ev, 0, sizeof ev);
  ev.xclient.type         = ClientMessage;
  ev.xclient.display      = a->dpy;
  ev.xclient.window       = a->dnd_src;
  ev.xclient.message_type = type;
  ev.xclient.format       = 32;
  ev.xclient.data.l[0]    = (long)a->win;
  ev.xclient.data.l[1]    = l1;
  ev.xclient.data.l[2]    = l2;
  ev.xclient.data.l[3]    = l3;
  ev.xclient.data.l[4]    = l4;
  x_XSendEvent(a->dpy, a->dnd_src, False, NoEventMask, &ev);
  x_XFlush(a->dpy);
}

// whether an XdndEnter offers text/uri-list (more than three types are
// on the source's XdndTypeList)
static int win_dnd_offers(AppWin* a, XClientMessageEvent* m) {
  if (m->data.l[1] & 1) {
    Atom type;
    int fmt, ok = 0;
    unsigned long n = 0, left = 0;
    unsigned char* data = NULL;
    if (x_XGetWindowProperty(a->dpy, (Window)m->data.l[0], a->dnd_types, 0, 1024, False,
      XA_ATOM, &type, &fmt, &n, &left, &data) == Success && data) {
      for (unsigned long i = 0; i < n; i += 1) {
        ok |= ((Atom*)data)[i] == a->dnd_uri;
      }
      x_XFree(data);
    }
    return ok;
  }
  return (Atom)m->data.l[2] == a->dnd_uri || (Atom)m->data.l[3] == a->dnd_uri
    || (Atom)m->data.l[4] == a->dnd_uri;
}

// an Xdnd client message (1), or not one (0)
static int win_dnd(AppWin* a, XClientMessageEvent* m) {
  Atom t = m->message_type;
  if (t == a->dnd_enter) {
    a->dnd_src = (Window)m->data.l[0];
    a->dnd_ok  = win_dnd_offers(a, m);
  } else if (t == a->dnd_position) {
    a->dnd_src = (Window)m->data.l[0];
    int rx = (int)(((unsigned long)m->data.l[2] >> 16) & 0xffff);
    int ry = (int)((unsigned long)m->data.l[2] & 0xffff);
    int x = 0, y = 0;
    Window child;
    x_XTranslateCoordinates(a->dpy, DefaultRootWindow(a->dpy), a->win, rx, ry, &x, &y, &child);
    win_dnd_send(a, a->dnd_status, a->dnd_ok ? 3 : 2, 0, 0, a->dnd_ok ? (long)a->dnd_copy : 0);
    if (a->dnd_ok) {
      u32 lx = x < 0 ? 0 : win_to_log((u32)x, a->w, a->pw);
      u32 ly = y < 0 ? 0 : win_to_log((u32)y, a->h, a->ph);
      win_push(a, 8, lx, ly, 0, 0);
    }
  } else if (t == a->dnd_leave) {
    if (a->dnd_ok) {
      win_push(a, 9, 0, 0, 0, 0);
    }
    a->dnd_ok = 0;
  } else if (t == a->dnd_drop) {
    a->dnd_src = (Window)m->data.l[0];
    if (a->dnd_ok) {
      x_XConvertSelection(a->dpy, a->dnd_sel, a->dnd_uri, a->dnd_prop, a->win, (Time)m->data.l[2]);
      x_XFlush(a->dpy);
    } else {
      win_dnd_send(a, a->dnd_finished, 0, 0, 0, 0);
    }
    a->dnd_ok = 0;
  } else {
    return 0;
  }
  return 1;
}

// the dropped list arrived (or the source could not give it)
static void win_dropped(AppWin* a, Atom prop) {
  Atom type;
  int fmt;
  unsigned long n = 0, left = 0;
  unsigned char* data = NULL;
  if (prop != None && x_XGetWindowProperty(a->dpy, a->win, prop, 0, 1 << 24, True,
    AnyPropertyType, &type, &fmt, &n, &left, &data) == Success && data) {
    free(a->drop);
    a->drop = io_mem(malloc(n + 1));
    memcpy(a->drop, data, n);
    a->drop_len = n;
    x_XFree(data);
    win_push(a, 10, 0, 0, 0, 0);
    win_dnd_send(a, a->dnd_finished, 1, (long)a->dnd_copy, 0, 0);
  } else {
    win_push(a, 9, 0, 0, 0, 0);
    win_dnd_send(a, a->dnd_finished, 0, 0, 0, 0);
  }
}

// whether the server has the key (by keycode) down now
static int win_key_down(AppWin* a, unsigned kc) {
  char km[32];
  x_XQueryKeymap(a->dpy, km);
  return (km[kc >> 3] >> (kc & 7)) & 1;
}

// the focus is back: a key that went down in this window and is up now was
// let go while the window could not hear, so report its release
static void win_keys_missed(AppWin* a) {
  char km[32];
  x_XQueryKeymap(a->dpy, km);
  for (unsigned kc = 0; kc < 256; kc++) {
    if (a->down[kc] != 0 && !((km[kc >> 3] >> (kc & 7)) & 1)) {
      win_push(a, 0, (u32)a->down[kc], 0, 0, kc << 1);
      a->down[kc] = 0;
    }
  }
}

static void win_pump(AppWin* a) {
  while (x_XPending(a->dpy) > 0) {
    XEvent ev;
    x_XNextEvent(a->dpy, &ev);
    switch (ev.type) {
      case KeyPress:
      case KeyRelease: {
        KeySym sym = 0;
        unsigned kc = ev.xkey.keycode & 255;
        if (ev.type == KeyRelease && !a->detect && win_key_down(a, kc)) {
          // a repeat on a server without detectable repeat: the key is
          // still down, so this release is no release
          break;
        }
        u32 text = win_text(&ev.xkey, &sym);
        if (ev.type == KeyPress) {
          a->down[kc] = sym;
        } else if (a->down[kc] != 0) {
          sym = a->down[kc];
          a->down[kc] = 0;
        }
        win_push(a, 0, (u32)sym, ev.type == KeyPress ? text : 0,
          win_mods(ev.xkey.state), (ev.type == KeyPress ? 1u : 0u) | (kc << 1));
        break;
      }
      case ButtonPress:
      case ButtonRelease: {
        u32 b = ev.xbutton.button;
        u32 x = ev.xbutton.x < 0 ? 0 : win_to_log((u32)ev.xbutton.x, a->w, a->pw);
        u32 y = ev.xbutton.y < 0 ? 0 : win_to_log((u32)ev.xbutton.y, a->h, a->ph);
        if (b >= 4 && b <= 7) {
          if (ev.type == ButtonPress) {
            win_push(a, 3, x, y, b - 4, 0);
          }
        } else {
          win_push(a, 1, x, y, b, ev.type == ButtonPress);
        }
        break;
      }
      case MotionNotify:
        win_push(a, 2,
          ev.xmotion.x < 0 ? 0 : win_to_log((u32)ev.xmotion.x, a->w, a->pw),
          ev.xmotion.y < 0 ? 0 : win_to_log((u32)ev.xmotion.y, a->h, a->ph),
          win_mods(ev.xmotion.state), 0);
        break;
      case FocusOut:
        win_push(a, 11, 0, 0, 0, 0);
        break;
      case FocusIn:
        win_keys_missed(a);
        win_push(a, 12, 0, 0, 0, 0);
        break;
      case ConfigureNotify: {
        u32 nw = (u32)ev.xconfigure.width;
        u32 nh = (u32)ev.xconfigure.height;
        u32 lw, lh;
        if (nw == a->pw && nh == a->ph) {
          break;
        }
        a->pw = nw;
        a->ph = nh;
        lw = a->scale == 1.0 ? nw : win_log_px(nw, a->scale);
        lh = a->scale == 1.0 ? nh : win_log_px(nh, a->scale);
        if (lw != a->w || lh != a->h) {
          a->w = lw;
          a->h = lh;
          win_push(a, 4, a->w, a->h, 0, 0);
        }
        break;
      }
      case Expose:
        a->full = 1;
        if (ev.xexpose.count == 0) {
          win_push(a, 5, 0, 0, 0, 0);
        }
        break;
      case ClientMessage:
        if (win_dnd(a, &ev.xclient)) {
          break;
        }
        if ((Atom)ev.xclient.data.l[0] == a->del) {
          win_push(a, 6, 0, 0, 0, 0);
        }
        break;
      case SelectionRequest:
        win_answer(a, &ev.xselectionrequest);
        break;
      case SelectionClear:
        // another program owns the clipboard now: pastes ask it
        if (ev.xselectionclear.selection == a->clip) {
          free(a->copy);
          a->copy = NULL;
          a->copy_len = 0;
        }
        break;
      case SelectionNotify:
        if (ev.xselection.selection == a->dnd_sel) {
          win_dropped(a, ev.xselection.property);
        } else if (ev.xselection.property != None) {
          win_pasted(a);
        } else if (ev.xselection.target == a->utf8) {
          // an owner without UTF8_STRING may still have plain STRING
          x_XConvertSelection(a->dpy, a->clip, XA_STRING, a->prop, a->win, CurrentTime);
        }
        break;
      case PropertyNotify:
        if (a->paste_incr && ev.xproperty.atom == a->prop
          && ev.xproperty.state == PropertyNewValue) {
          win_paste_piece(a);
        }
        break;
    }
  }
}

// Any Xlib call off the pump (XPutImage, XFlush...) may read waiting
// input into Xlib's own queue; the pump sleeps on the socket, which is
// then empty, so those events would wait for the next one to arrive (a
// key typed would show only on the key after it). After such a call,
// queued events make the window send itself a no-op ClientMessage: the
// socket wakes the pump, which drains the queue.
static void win_nudge(AppWin* a) {
  if (x_XEventsQueued(a->dpy, QueuedAlready) == 0) {
    return;
  }
  XEvent ev;
  memset(&ev, 0, sizeof ev);
  ev.xclient.type = ClientMessage;
  ev.xclient.window = a->win;
  ev.xclient.message_type = a->prop;
  ev.xclient.format = 32;
  x_XSendEvent(a->dpy, a->win, False, NoEventMask, &ev);
  x_XFlush(a->dpy);
}

// the pending events as a flat list of words, five per event (kind, a,
// b, c, d); Bend's Win.decode turns them into WinEv (a module's
// constructor ids depend on its path, so C only speaks words)
#ifdef CID_WIN_WORDS
static Term win_list(Env e, AppWin* a) {
  Term list = term_pak(CID_NIL, 0);
  for (u64 i = (u64)a->n * 5; i > 0;) {
    i -= 1;
    list = io_node(e, CID_CON, (Term)a->evs[i], list);
  }
  a->n = 0;
  return list;
}
#endif

#ifdef CID_WIN_OPEN

Term win_open_run(Env e, Term* f, IoWork* w) {
  u64 n = 0;
  char* title = io_cstr(e, f[0], &n);
  u32 ww = (u32)f[1], hh = (u32)f[2];
  const char* why = NULL;
  int code = 0;
  AppWin* a = NULL;
  Display* dpy = NULL;
  if (!win_load()) {
    code = ENOTSUP;
    why = "the window needs libX11 (install libx11-6)";
  } else if ((dpy = x_XOpenDisplay(NULL)) == NULL) {
    code = ENOTSUP;
    why = "no display: run Backplane from a desktop session, or use `backplane serve`";
  } else {
    int scr = DefaultScreen(dpy);
    a = io_mem(calloc(1, sizeof *a));
    a->dpy = dpy;
    a->scale = win_ui_scale();
    a->w = ww;
    a->h = hh;
    a->pw = a->scale == 1.0 ? ww : win_phys(ww, a->scale);
    a->ph = a->scale == 1.0 ? hh : win_phys(hh, a->scale);
    if (a->scale != 1.0) {
      fprintf(stderr, "backplane: UI scale %g (window %ux%u for a %ux%u layout)\n",
        a->scale, a->pw, a->ph, a->w, a->h);
    }
    a->win = x_XCreateSimpleWindow(dpy, RootWindow(dpy, scr), 0, 0, a->pw, a->ph, 0, 0,
      BlackPixel(dpy, scr));
    a->del     = x_XInternAtom(dpy, "WM_DELETE_WINDOW", False);
    a->clip    = x_XInternAtom(dpy, "CLIPBOARD", False);
    a->utf8    = x_XInternAtom(dpy, "UTF8_STRING", False);
    a->targets = x_XInternAtom(dpy, "TARGETS", False);
    a->prop    = x_XInternAtom(dpy, "BACKPLANE_PASTE", False);
    a->incr    = x_XInternAtom(dpy, "INCR", False);
    x_XSetWMProtocols(dpy, a->win, &a->del, 1);
    a->dnd_aware    = x_XInternAtom(dpy, "XdndAware", False);
    a->dnd_enter    = x_XInternAtom(dpy, "XdndEnter", False);
    a->dnd_position = x_XInternAtom(dpy, "XdndPosition", False);
    a->dnd_status   = x_XInternAtom(dpy, "XdndStatus", False);
    a->dnd_leave    = x_XInternAtom(dpy, "XdndLeave", False);
    a->dnd_drop     = x_XInternAtom(dpy, "XdndDrop", False);
    a->dnd_finished = x_XInternAtom(dpy, "XdndFinished", False);
    a->dnd_sel      = x_XInternAtom(dpy, "XdndSelection", False);
    a->dnd_copy     = x_XInternAtom(dpy, "XdndActionCopy", False);
    a->dnd_uri      = x_XInternAtom(dpy, "text/uri-list", False);
    a->dnd_types    = x_XInternAtom(dpy, "XdndTypeList", False);
    a->dnd_prop     = x_XInternAtom(dpy, "BACKPLANE_DROP", False);
    Atom xdnd_version = 5;
    x_XChangeProperty(dpy, a->win, a->dnd_aware, XA_ATOM, 32, PropModeReplace,
      (unsigned char*)&xdnd_version, 1);
    x_XStoreName(dpy, a->win, title);
    // (BACKPLANE_NO_DETECTABLE_REPEAT leaves it unasked, to try the
    // fallback on a server that supports it)
    if (x_detect_repeat != NULL && getenv("BACKPLANE_NO_DETECTABLE_REPEAT") == NULL) {
      Bool got = False;
      Bool ok = x_detect_repeat(dpy, True, &got);
      a->detect = ok && got;
    }
    win_icon(dpy, a->win);
    x_XSelectInput(dpy, a->win, KeyPressMask | KeyReleaseMask | ButtonPressMask
      | ButtonReleaseMask | PointerMotionMask | StructureNotifyMask | ExposureMask
      | FocusChangeMask | PropertyChangeMask);
    x_XMapRaised(dpy, a->win);
    x_XFlush(dpy);
  }
  free(title);
  if (code) {
    return io_fail(e, code, why);
  }
  return io_done(e, io_hand((intptr_t)a));
}

static void __attribute__((constructor)) win_open_use(void) {
  io_eff(CID_WIN_OPEN, win_open_run, 0);
}

#endif

#ifdef CID_WIN_WORDS

static Term win_words_more(Env e, IoWork* w) {
  AppWin* a = (AppWin*)w->hand;
  win_pump(a);
  if (a->n == 0) {
    // the nudge can still lose a race with the presenting thread (an event
    // read into Xlib's queue after it looked); a short deadline re-checks
    // the queue, so no key waits for the next one to show
    return io_wait_on(w, ConnectionNumber(a->dpy), POLLIN, io_tick() + 30000000ull, win_words_more);
  }
  return io_tup(e, io_hand(w->hand), win_list(e, a));
}

// the events since the last call; waits (parked) until there is one
Term win_words_run(Env e, Term* f, IoWork* w) {
  w->hand = (intptr_t)io_hand_v(f[0]);
  return win_words_more(e, w);
}

static void __attribute__((constructor)) win_words_use(void) {
  io_eff(CID_WIN_WORDS, win_words_run, 0);
}

#endif

#ifdef CID_WIN_PRESENT

// fill the part of an n x n region at (x, y) that lies in the w x h frame,
// marking each 64 px block in which a pixel changed (dirty is dw wide)
static void win_fill(Corpus H, u32* pix, u32 w, u32 h, u8* dirty, u32 dw, Term t, u32 x, u32 y, u32 n) {
  if (x >= w || y >= h) {
    return;
  }
  if (term_tag(t) == TAG_CTR && n > 1) {
    Loc l = term_rfc(t) ? H[term_loc(t)] >> 24 : term_loc(t);
    u32 m = n / 2;
    win_fill(H, pix, w, h, dirty, dw, H[l + 0], x, y, m);
    win_fill(H, pix, w, h, dirty, dw, H[l + 1], x + m, y, m);
    win_fill(H, pix, w, h, dirty, dw, H[l + 2], x, y + m, m);
    win_fill(H, pix, w, h, dirty, dw, H[l + 3], x + m, y + m, m);
    return;
  }
  while (term_tag(t) == TAG_CTR) {
    Loc l = term_rfc(t) ? H[term_loc(t)] >> 24 : term_loc(t);
    t = H[l];
  }
  u32 c  = (u32)term_loc(t) & 0xFFFFFF;
  u32 x1 = x + n < w ? x + n : w;
  u32 y1 = y + n < h ? y + n : h;
  // a region lies within one block column unless it is 64 px or wider,
  // and then it starts on a block edge: each row is checked per block
  for (u32 yy = y; yy < y1; yy += 1) {
    u32* row = pix + (size_t)yy * w;
    u8*  dr  = dirty + (size_t)(yy >> 6) * dw;
    for (u32 bx = x; bx < x1; bx = (bx | 63) + 1) {
      u32 be = (bx | 63) + 1 < x1 ? (bx | 63) + 1 : x1;
      u32 diff = 0;
      for (u32 xx = bx; xx < be; xx += 1) {
        diff |= row[xx] ^ c;
        row[xx] = c;
      }
      if (diff != 0) {
        dr[bx >> 6] = 1;
      }
    }
  }
}

// send the changed blocks: each block row's runs of dirty blocks as one
// rectangle
static void win_put(AppWin* a) {
  GC gc = DefaultGC(a->dpy, DefaultScreen(a->dpy));
  for (u32 by = 0; by < a->dh; by += 1) {
    u8* dr = a->dirty + (size_t)by * a->dw;
    u32 bx = 0;
    while (bx < a->dw) {
      if (!dr[bx]) {
        bx += 1;
        continue;
      }
      u32 b0 = bx;
      while (bx < a->dw && dr[bx]) {
        dr[bx] = 0;
        bx += 1;
      }
      u32 x0 = b0 * 64, y0 = by * 64;
      u32 x1 = bx * 64 < a->w ? bx * 64 : a->w;
      u32 y1 = y0 + 64 < a->h ? y0 + 64 : a->h;
      x_XPutImage(a->dpy, a->win, gc, a->img, (int)x0, (int)y0, (int)x0, (int)y0, x1 - x0, y1 - y0);
    }
  }
}

// the node a term names (a shared one through its count cell)
static inline Loc win_src(Corpus H, Term t) {
  return term_rfc(t) ? H[term_loc(t)] >> 24 : term_loc(t);
}

// win_fill over the regions where the frame differs from the last one
// shown (old, whose pixels the buffer holds): a region that is the very
// same node in both is skipped. Both trees are alive here, so one node
// can't stand for two images.
static void win_fill_diff(Corpus H, u32* pix, u32 w, u32 h, u8* dirty, u32 dw, Term t, Term old, u32 x, u32 y, u32 n) {
  if (x >= w || y >= h) {
    return;
  }
  if (term_tag(t) != TAG_CTR || n <= 1 || term_tag(old) != TAG_CTR) {
    win_fill(H, pix, w, h, dirty, dw, t, x, y, n);
    return;
  }
  Loc l = win_src(H, t);
  Loc o = win_src(H, old);
  if (l == o) {
    return;
  }
  u32 m = n / 2;
  win_fill_diff(H, pix, w, h, dirty, dw, H[l + 0], H[o + 0], x, y, m);
  win_fill_diff(H, pix, w, h, dirty, dw, H[l + 1], H[o + 1], x + m, y, m);
  win_fill_diff(H, pix, w, h, dirty, dw, H[l + 2], H[o + 2], x, y + m, m);
  win_fill_diff(H, pix, w, h, dirty, dw, H[l + 3], H[o + 3], x + m, y + m, m);
}

// BACKPLANE_SNAP=<path>: every presented frame is also written there as
// a PPM (tests and bug reports; off unless set)
static void win_snap(AppWin* a) {
  const char* path = getenv("BACKPLANE_SNAP");
  if (path == NULL || *path == 0) {
    return;
  }
  FILE* out = fopen(path, "wb");
  if (out == NULL) {
    return;
  }
  fprintf(out, "P6\n%u %u\n255\n", a->w, a->h);
  u32* pix = (u32*)a->img->data;
  for (u64 i = 0; i < (u64)a->w * a->h; i += 1) {
    unsigned char rgb[3] = { (pix[i] >> 16) & 255, (pix[i] >> 8) & 255, pix[i] & 255 };
    fwrite(rgb, 1, 3, out);
  }
  fclose(out);
}

static void win_scaled_fit(AppWin* a) {
  int scr;
  if (a->scale == 1.0) {
    return;
  }
  if (a->scaled != NULL && (u32)a->scaled->width == a->pw && (u32)a->scaled->height == a->ph) {
    return;
  }
  if (a->scaled != NULL) {
    XDestroyImage(a->scaled);
    a->scaled = NULL;
  }
  scr = DefaultScreen(a->dpy);
  a->scaled = x_XCreateImage(a->dpy, DefaultVisual(a->dpy, scr), DefaultDepth(a->dpy, scr),
    ZPixmap, 0, io_mem(calloc((size_t)a->pw * a->ph, 4)), a->pw, a->ph, 32, (int)a->pw * 4);
  a->scaled->byte_order = LSBFirst;
}

// sRGB <-> linear, so a fractional enlarge blends coverage instead of
// doubling pixels. Built once. win_srgb is indexed by a 16-bit linear value.
static uint16_t win_lin[256];
static uint8_t win_srgb[65536];
static int win_lut_ready;

static void win_lut_init(void) {
  int i;
  if (win_lut_ready) {
    return;
  }
  for (i = 0; i < 256; i += 1) {
    double c = (double)i / 255.0;
    double l = c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4);
    int v = (int)(l * 65535.0 + 0.5);
    win_lin[i] = (uint16_t)(v < 0 ? 0 : v > 65535 ? 65535 : v);
  }
  for (i = 0; i < 65536; i += 1) {
    double l = (double)i / 65535.0;
    double c = l <= 0.0031308 ? l * 12.92 : 1.055 * pow(l, 1.0 / 2.4) - 0.055;
    int s = (int)(c * 255.0 + 0.5);
    win_srgb[i] = (uint8_t)(s < 0 ? 0 : s > 255 ? 255 : s);
  }
  win_lut_ready = 1;
}

// dest pixel i as a source pixel index and a 16-bit blend toward the next.
// The sample sits on the dest pixel's center.
static u32 win_axis(u32 i, u32 sn, u32 dn, u32* frac) {
  u64 q;
  u32 max;
  if (sn <= 1 || dn <= 1) {
    *frac = 0;
    return 0;
  }
  q = ((u64)(i * 2u + 1u) * sn << 15) / dn;
  if (q < (1u << 15)) {
    *frac = 0;
    return 0;
  }
  q -= 1u << 15;
  max = (sn - 1u) << 16;
  if (q > max) {
    q = max;
  }
  *frac = (u32)(q & 0xffff);
  return (u32)(q >> 16);
}

static u32 win_lerp16(u32 a, u32 b, u32 t) {
  return (u32)(((u64)a * (65536u - t) + (u64)b * t + 32768u) >> 16);
}

static u32 win_pack(u32 r, u32 g, u32 b) {
  if (r > 65535u) {
    r = 65535u;
  }
  if (g > 65535u) {
    g = 65535u;
  }
  if (b > 65535u) {
    b = 65535u;
  }
  return ((u32)win_srgb[r] << 16) | ((u32)win_srgb[g] << 8) | (u32)win_srgb[b];
}

// one bilinear sample. Channels are 0x00RRGGBB, which is what the frame stores.
static u32 win_bilinear(const u32* src, u32 sw, u32 sh, u32 i, u32 j, u32 dw, u32 dh) {
  u32 fx, fy, x0, y0, x1, y1;
  u32 p00, p10, p01, p11;
  u32 r0, g0, b0, r1, g1, b1;
  x0 = win_axis(i, sw, dw, &fx);
  y0 = win_axis(j, sh, dh, &fy);
  x1 = x0 + 1u < sw ? x0 + 1u : x0;
  y1 = y0 + 1u < sh ? y0 + 1u : y0;
  p00 = src[(size_t)y0 * sw + x0];
  p10 = src[(size_t)y0 * sw + x1];
  p01 = src[(size_t)y1 * sw + x0];
  p11 = src[(size_t)y1 * sw + x1];
  r0 = win_lerp16(win_lin[(p00 >> 16) & 255], win_lin[(p10 >> 16) & 255], fx);
  g0 = win_lerp16(win_lin[(p00 >> 8) & 255], win_lin[(p10 >> 8) & 255], fx);
  b0 = win_lerp16(win_lin[p00 & 255], win_lin[p10 & 255], fx);
  r1 = win_lerp16(win_lin[(p01 >> 16) & 255], win_lin[(p11 >> 16) & 255], fx);
  g1 = win_lerp16(win_lin[(p01 >> 8) & 255], win_lin[(p11 >> 8) & 255], fx);
  b1 = win_lerp16(win_lin[p01 & 255], win_lin[p11 & 255], fx);
  return win_pack(win_lerp16(r0, r1, fy), win_lerp16(g0, g1, fy), win_lerp16(b0, b1, fy));
}

// the logical framebuffer, bilinear, into the physical window.
// A 1.6 monitor scale is not an integer, so nearest-neighbor copies some
// source pixels twice and others once and the glyphs look stair-stepped.
static void win_blit_scaled(AppWin* a) {
  const u32* src = (const u32*)a->img->data;
  u32* dst;
  u32 sw = a->w, sh = a->h, dw = a->pw, dh = a->ph;
  u32 j;
  win_lut_init();
  win_scaled_fit(a);
  if (a->scaled == NULL || sw == 0 || sh == 0) {
    return;
  }
  dst = (u32*)a->scaled->data;
  for (j = 0; j < dh; j += 1) {
    u32* row = dst + (size_t)j * dw;
    u32 i;
    for (i = 0; i < dw; i += 1) {
      row[i] = win_bilinear(src, sw, sh, i, j, dw, dh);
    }
  }
  x_XPutImage(a->dpy, a->win, DefaultGC(a->dpy, DefaultScreen(a->dpy)), a->scaled,
    0, 0, 0, 0, dw, dh);
}

static int win_dirty_any(AppWin* a) {
  u32 i, n = a->dw * a->dh;
  for (i = 0; i < n; i += 1) {
    if (a->dirty[i]) {
      return 1;
    }
  }
  return 0;
}

// fill the buffer with image (over old where it may skip, has_old) and
// send what changed
static void win_show_image(Env e, AppWin* a, Term image, Term old, int has_old) {
  io_sync();
  int whole = atomic_exchange(&a->full, 0);
  if (a->img == NULL || (u32)a->img->width != a->w || (u32)a->img->height != a->h) {
    if (a->img) {
      XDestroyImage(a->img);
    }
    int scr = DefaultScreen(a->dpy);
    a->img = x_XCreateImage(a->dpy, DefaultVisual(a->dpy, scr), DefaultDepth(a->dpy, scr),
      ZPixmap, 0, io_mem(calloc((size_t)a->w * a->h, 4)), a->w, a->h, 32, (int)a->w * 4);
    a->img->byte_order = LSBFirst;
    a->dw = (a->w + 63) / 64;
    a->dh = (a->h + 63) / 64;
    free(a->dirty);
    a->dirty = io_mem(calloc((size_t)a->dw * a->dh, 1));
    whole = 1;
    a->stale = 1;
  }
  u32 n = 1;
  while (n < a->w || n < a->h) {
    n *= 2;
  }
  if (has_old && !a->stale) {
    win_fill_diff(e.mem, (u32*)a->img->data, a->w, a->h, a->dirty, a->dw, image, old, 0, 0, n);
  } else {
    win_fill(e.mem, (u32*)a->img->data, a->w, a->h, a->dirty, a->dw, image, 0, 0, n);
  }
  a->stale = !has_old;
  if (a->scale != 1.0) {
    if (whole || win_dirty_any(a)) {
      memset(a->dirty, 0, (size_t)a->dw * a->dh);
      win_blit_scaled(a);
    }
  } else if (whole) {
    memset(a->dirty, 0, (size_t)a->dw * a->dh);
    x_XPutImage(a->dpy, a->win, DefaultGC(a->dpy, DefaultScreen(a->dpy)), a->img,
      0, 0, 0, 0, a->w, a->h);
  } else {
    win_put(a);
  }
  x_XFlush(a->dpy);
  win_nudge(a);
  win_snap(a);
}

// a frame that is not the window's own (the lightbox): the next show
// fills whole
Term win_present_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  win_show_image(e, a, f[1], 0, 0);
  return io_tup(e, f[0], f[1]);
}

static void __attribute__((constructor)) win_present_use(void) {
  io_eff(CID_WIN_PRESENT, win_present_run, 0);
}

#endif

#ifdef CID_WIN_SHOW

// the window's frame, and the last one it showed (the buffer holds it)
Term win_show_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  win_show_image(e, a, f[1], f[2], 1);
  return io_tup(e, f[0], io_tup(e, f[1], f[2]));
}

static void __attribute__((constructor)) win_show_use(void) {
  io_eff(CID_WIN_SHOW, win_show_run, 0);
}

#endif

#ifdef CID_WIN_COPY

// own the clipboard with this text
Term win_copy_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  free(a->copy);
  a->copy = io_cstr(e, f[1], &a->copy_len);
  x_XSetSelectionOwner(a->dpy, a->clip, a->win, CurrentTime);
  x_XFlush(a->dpy);
  win_nudge(a);
  return f[0];
}

static void __attribute__((constructor)) win_copy_use(void) {
  io_eff(CID_WIN_COPY, win_copy_run, 0);
}

#endif

#ifdef CID_WIN_PASTE

// ask for the clipboard; it arrives later as WPaste. Only while this
// window still owns it is the last copy pasted directly; otherwise the
// owner (another program) is asked.
Term win_paste_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  if (a->copy != NULL && x_XGetSelectionOwner(a->dpy, a->clip) == a->win) {
    free(a->paste);
    a->paste = io_mem(malloc(a->copy_len + 1));
    memcpy(a->paste, a->copy, a->copy_len);
    a->paste_len = a->copy_len;
    win_push(a, 7, 0, 0, 0, 0);
  } else {
    x_XConvertSelection(a->dpy, a->clip, a->utf8, a->prop, a->win, CurrentTime);
    x_XFlush(a->dpy);
    win_nudge(a);
  }
  return f[0];
}

static void __attribute__((constructor)) win_paste_use(void) {
  io_eff(CID_WIN_PASTE, win_paste_run, 0);
}

#endif

#ifdef CID_WIN_PASTED

// the text of the last paste event
Term win_pasted_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  return io_tup(e, f[0], io_str(e, a->paste ? a->paste : "", a->paste ? a->paste_len : 0));
}

static void __attribute__((constructor)) win_pasted_use(void) {
  io_eff(CID_WIN_PASTED, win_pasted_run, 0);
}

#endif

#ifdef CID_WIN_DROPPED

// the text/uri-list of the last drop event
Term win_dropped_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  return io_tup(e, f[0], io_str(e, a->drop ? a->drop : "", a->drop ? a->drop_len : 0));
}

static void __attribute__((constructor)) win_dropped_use(void) {
  io_eff(CID_WIN_DROPPED, win_dropped_run, 0);
}

#endif

#ifdef CID_WIN_SIZE

Term win_size_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  return io_tup(e, f[0], io_tup(e, (Term)a->w, (Term)a->h));
}

static void __attribute__((constructor)) win_size_use(void) {
  io_eff(CID_WIN_SIZE, win_size_run, 0);
}

#endif

#ifdef CID_WIN_TWIN

// a second handle on the same window: one computation waits for events
// while another presents frames
Term win_twin_run(Env e, Term* f, IoWork* w) {
  return io_tup(e, f[0], f[0]);
}

static void __attribute__((constructor)) win_twin_use(void) {
  io_eff(CID_WIN_TWIN, win_twin_run, 0);
}

#endif

#ifdef CID_WIN_TITLE

Term win_title_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  u64 n = 0;
  char* t = io_cstr(e, f[1], &n);
  x_XStoreName(a->dpy, a->win, t);
  x_XFlush(a->dpy);
  win_nudge(a);
  free(t);
  return f[0];
}

static void __attribute__((constructor)) win_title_use(void) {
  io_eff(CID_WIN_TITLE, win_title_run, 0);
}

#endif

#else

// No window on this platform yet (macOS gets its own AppKit twin).

#ifdef CID_WIN_OPEN
Term win_open_run(Env e, Term* f, IoWork* w) {
  return io_fail(e, ENOTSUP, "the native window is Linux-only for now; run `backplane serve`");
}
static void __attribute__((constructor)) win_open_use(void) {
  io_eff(CID_WIN_OPEN, win_open_run, 0);
}
#endif

#endif
