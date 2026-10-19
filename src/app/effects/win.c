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
//   11 focus lost   12 focus gained   13 image paste (Win.pasted_image)
//   14 ime commit (id)   15 ime preedit draw (first, length, id,
//   caret | only << 31)   16 ime preedit caret (position)
//   17 ime preedit start   18 ime preedit done
// The text of 14 and 15 is not in the event: each owns a payload under its
// serial id (Win.payload hands it over once), so events queued before the
// app drains them never share a buffer.
// mods: 1 shift, 2 ctrl, 4 alt, 8 super.

#if defined(__linux__)

#include <dlfcn.h>
#include <pthread.h>
#include <time.h>
#include <locale.h>
#include <langinfo.h>
#include <wchar.h>
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/Xatom.h>
#include <X11/keysym.h>

#define WIN_FNS(X) \
  X(XGetInputFocus, int, (Display*, Window*, int*)) \
  X(XSetInputFocus, int, (Display*, Window, int, Time)) \
  X(XOpenDisplay, Display*, (const char*)) \
  X(XCloseDisplay, int, (Display*)) \
  X(XCreateSimpleWindow, Window, (Display*, Window, int, int, unsigned, unsigned, unsigned, unsigned long, unsigned long)) \
  X(XInternAtom, Atom, (Display*, const char*, Bool)) \
  X(XGetAtomName, char*, (Display*, Atom)) \
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
  X(XGetWindowAttributes, Status, (Display*, Window, XWindowAttributes*)) \
  X(XDeleteProperty, int, (Display*, Window, Atom)) \
  X(XTranslateCoordinates, Bool, (Display*, Window, Window, int, int, int*, int*, Window*))

#define WIN_PTR(name, ret, args) static ret (*x_##name) args;
WIN_FNS(WIN_PTR)

// a key held down sends presses with no releases between (so a repeat can
// be told from a new press); a server without it sends a release and a
// press of the same key at the same time for each repeat (win_pump asks the
// server whether the key is still down and, if so, drops the release)
static Bool (*x_detect_repeat)(Display*, Bool, Bool*);

// Input methods (XIM) are optional: a libX11 without them, or any symbol
// missing, leaves the plain keyboard working (win_ime_ok stays 0).
#define IME_FNS(X) \
  X(XSetLocaleModifiers, char*, (const char*)) \
  X(XSupportsLocale, Bool, (void)) \
  X(XOpenIM, XIM, (Display*, void*, char*, char*)) \
  X(XCloseIM, Status, (XIM)) \
  X(XGetIMValues, char*, (XIM, ...)) \
  X(XCreateIC, XIC, (XIM, ...)) \
  X(XDestroyIC, void, (XIC)) \
  X(XSetICFocus, void, (XIC)) \
  X(XUnsetICFocus, void, (XIC)) \
  X(XGetICValues, char*, (XIC, ...)) \
  X(XSetICValues, char*, (XIC, ...)) \
  X(XFilterEvent, Bool, (XEvent*, Window)) \
  X(Xutf8LookupString, int, (XIC, XKeyPressedEvent*, char*, int, KeySym*, Status*)) \
  X(Xutf8ResetIC, char*, (XIC)) \
  X(XSetIMValues, char*, (XIM, ...)) \
  X(XVaCreateNestedList, XVaNestedList, (int, ...))

#define IME_PTR(name, ret, args) static ret (*x_##name) args;
IME_FNS(IME_PTR)
static int win_ime_ok = 0;
static Status (*x_InitThreads)(void);
static Bool (*x_RegisterIMInstantiate)(Display*, void*, char*, char*, void (*)(Display*, XPointer, XPointer), XPointer);

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
  x_InitThreads = (Status (*)(void))dlsym(lib, "XInitThreads");
  x_RegisterIMInstantiate = (Bool (*)(Display*, void*, char*, char*, void (*)(Display*, XPointer, XPointer), XPointer))dlsym(lib, "XRegisterIMInstantiateCallback");
  win_ime_ok = 1;
#define IME_SYM(name, ret, args) \
  if ((x_##name = (ret (*) args)dlsym(lib, #name)) == NULL) { win_ime_ok = 0; }
  IME_FNS(IME_SYM)
  state = 1;
  return 1;
}

typedef struct {
  Display*  dpy;
  Window    win;
  Atom      del, clip, utf8, targets, prop, incr;
  XImage*   img;
  u32       w, h;
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
  int       paste_incr, paste_busy, paste_ready;
  char*     paste_mime;
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
  // input method: the method and context (NULL: none, plain keys), how
  // preedit is shown (0 none, 1 drawn by the app through callbacks, 2 the
  // method draws it), the style and filter mask asked for, and the length
  // and caret of the preedit the callbacks have built so far
  XIM       xim;
  XIC       xic;
  int       ime_mode;
  XIMStyle  ime_style;
  XIMCallback pe_cb[4];
  XIMCallback ds_cb;
  u32       pe_len, pe_caret;
  int       spot_x, spot_y;
} AppWin;

// events are queued by the pump (and by calls from other threads)
static pthread_mutex_t win_qlock = PTHREAD_MUTEX_INITIALIZER;

// n words (a multiple of five: whole events) onto the queue
static void win_put_words(AppWin* a, const u32* ws, size_t n) {
  pthread_mutex_lock(&win_qlock);
  while ((size_t)a->n * 5 + n > (size_t)a->cap * 5) {
    a->cap = a->cap == 0 ? 64 : a->cap * 2;
    a->evs = io_mem(realloc(a->evs, (size_t)a->cap * 20));
  }
  memcpy(a->evs + (size_t)a->n * 5, ws, n * 4);
  a->n += (u32)(n / 5);
  pthread_mutex_unlock(&win_qlock);
}

static void win_push(AppWin* a, u32 kind, u32 p, u32 q, u32 r, u32 s) {
  u32 ev[5] = { kind, p, q, r, s };
  win_put_words(a, ev, 5);
}

// A payload: the text of one ime event (code points, and for a preedit
// one feedback value per character), kept under its serial id until the
// app takes it. Events are queued whole, so a later one never changes an
// earlier one's text.
typedef struct WinPayload {
  u32 id;
  size_t n;
  u32* cps;
  u8* fb;
  struct WinPayload* next;
} WinPayload;

static WinPayload* win_pl = NULL;
static u32 win_pl_next = 1;

static u32 win_payload(const u32* cps, const u8* fb, size_t n) {
  WinPayload* p = io_mem(calloc(1, sizeof *p));
  p->n = n;
  p->cps = io_mem(malloc((n ? n : 1) * 4));
  p->fb = io_mem(malloc(n ? n : 1));
  if (n) {
    memcpy(p->cps, cps, n * 4);
    if (fb) memcpy(p->fb, fb, n); else memset(p->fb, 0, n);
  }
  pthread_mutex_lock(&win_qlock);
  p->id = win_pl_next++;
  p->next = win_pl;
  win_pl = p;
  pthread_mutex_unlock(&win_qlock);
  return p->id;
}

// take the payload of id out of the store (NULL: none, e.g. taken already)
static WinPayload* win_payload_take(u32 id) {
  pthread_mutex_lock(&win_qlock);
  WinPayload** pp = &win_pl;
  while (*pp != NULL && (*pp)->id != id) pp = &(*pp)->next;
  WinPayload* p = *pp;
  if (p != NULL) *pp = p->next;
  pthread_mutex_unlock(&win_qlock);
  return p;
}

// UTF-8 to code points (invalid bytes become U+FFFD); answers the count
static size_t win_utf8(const char* s, size_t len, u32* out) {
  size_t n = 0;
  for (size_t i = 0; i < len;) {
    unsigned char c = (unsigned char)s[i];
    u32 cp = 0xfffd;
    size_t k = 1;
    if (c < 0x80) { cp = c; }
    else if (c >= 0xc2 && c < 0xe0 && i + 1 < len && (s[i + 1] & 0xc0) == 0x80) {
      cp = ((u32)(c & 0x1f) << 6) | (s[i + 1] & 0x3f); k = 2;
    } else if (c >= 0xe0 && c < 0xf0 && i + 2 < len && (s[i + 1] & 0xc0) == 0x80 && (s[i + 2] & 0xc0) == 0x80) {
      cp = ((u32)(c & 0x0f) << 12) | ((u32)(s[i + 1] & 0x3f) << 6) | (s[i + 2] & 0x3f); k = 3;
      if (cp < 0x800 || (cp >= 0xd800 && cp < 0xe000)) cp = 0xfffd;
    } else if (c >= 0xf0 && c < 0xf5 && i + 3 < len && (s[i + 1] & 0xc0) == 0x80 && (s[i + 2] & 0xc0) == 0x80 && (s[i + 3] & 0xc0) == 0x80) {
      cp = ((u32)(c & 0x07) << 18) | ((u32)(s[i + 1] & 0x3f) << 12) | ((u32)(s[i + 2] & 0x3f) << 6) | (s[i + 3] & 0x3f); k = 4;
      if (cp < 0x10000 || cp > 0x10ffff) cp = 0xfffd;
    }
    if (out) out[n] = cp;
    n += 1;
    i += k;
  }
  return n;
}

// text the input method committed: one event that owns its characters
static void win_commit(AppWin* a, const char* s, size_t len) {
  u32* cps = io_mem(malloc((len ? len : 1) * 4));
  size_t n = win_utf8(s, len, cps);
  if (n > 0) win_push(a, 14, win_payload(cps, NULL, n), 0, 0, 0);
  free(cps);
}

static u32 win_mods(unsigned state) {
  return (state & ShiftMask ? 1u : 0) | (state & ControlMask ? 2u : 0)
    | (state & Mod1Mask ? 4u : 0) | (state & Mod4Mask ? 8u : 0);
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
  // Images over the attachment cap are drained but not retained. Bend
  // still receives their size, and uses the normal oversize refusal.
  if (!a->paste_mime || a->paste_len + n <= 10485760) {
    a->paste = io_mem(realloc(a->paste, a->paste_len + n + 1));
    memcpy(a->paste + a->paste_len, data, n);
  }
  a->paste_len += n;
}

static void win_paste_done(AppWin* a) {
  a->paste_busy = 0;
  a->paste_ready = 1;
  win_push(a, a->paste_mime ? 13 : 7, 0, 0, 0, 0);
}

// TARGETS are Xlib Atoms (longs even on a 64-bit host). Prefer a lossless
// screenshot over text or other representations of the same clipboard.
static void win_paste_targets(AppWin* a) {
  const char* preferred[] = { "image/png", "image/jpeg", "image/webp", "image/gif", "image/bmp" };
  Atom type, chosen = a->utf8;
  int fmt, best = 6;
  unsigned long n = 0, left = 0;
  unsigned char* data = NULL;
  if (x_XGetWindowProperty(a->dpy, a->win, a->prop, 0, 65536, True,
      XA_ATOM, &type, &fmt, &n, &left, &data) == Success && data && fmt == 32 && type == XA_ATOM) {
    for (unsigned long i = 0; i < n; i++) {
      char* name = x_XGetAtomName(a->dpy, ((Atom*)data)[i]);
      if (name && !strncmp(name, "image/", 6)) {
        int rank = 5;
        for (int j = 0; j < 5; j++) if (!strcmp(name, preferred[j])) rank = j;
        if (rank < best) {
          best = rank;
          chosen = ((Atom*)data)[i];
          free(a->paste_mime);
          a->paste_mime = io_mem(strdup(name));
        }
      }
      if (name) x_XFree(name);
    }
  }
  if (data) x_XFree(data);
  x_XConvertSelection(a->dpy, a->clip, chosen, a->prop, a->win, CurrentTime);
  x_XFlush(a->dpy);
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
      if (fmt == 8 && left == 0) win_paste_done(a);
      else a->paste_busy = 0;
    }
    x_XFree(data);
  } else {
    a->paste_busy = 0;
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
    win_paste_done(a);
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
      win_push(a, 8, x < 0 ? 0 : (u32)x, y < 0 ? 0 : (u32)y, 0, 0);
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

// Input method
// ------------
// The method (ibus, fcitx, Xlib's own compose) is asked for preedit through
// callbacks, so the app draws the text being composed itself: the callbacks
// turn into events 17-18 (start, done), 15 (draw: replace a range of the
// preedit, or change only its feedback) and 16 (caret), and what the method
// commits is event 14. All of it runs on the pump's thread, inside
// XFilterEvent. A method that cannot do callbacks is used with its own
// preedit window (mode 2), and a missing or lost method leaves plain keys
// (mode 0); BACKPLANE_IME=debug says which on stderr.

static u32 win_cp(u32 c) {
  return (c > 0x10ffff || (c >= 0xd800 && c < 0xe000) || c == 0) ? 0xfffd : c;
}

static int win_pe_start(XIC ic, XPointer client, XPointer call) {
  AppWin* a = (AppWin*)client;
  a->pe_len = 0;
  a->pe_caret = 0;
  win_push(a, 17, 0, 0, 0, 0);
  return -1;
}

static void win_pe_done(XIC ic, XPointer client, XPointer call) {
  AppWin* a = (AppWin*)client;
  a->pe_len = 0;
  a->pe_caret = 0;
  win_push(a, 18, 0, 0, 0, 0);
}

// a draw: characters [chg_first, chg_first + chg_length) become the text
// (NULL: they are deleted); a text with no string only changes feedback
static void win_pe_draw(XIC ic, XPointer client, XIMPreeditDrawCallbackStruct* d) {
  AppWin* a = (AppWin*)client;
  XIMText* t = d->text;
  u32 first = d->chg_first < 0 ? 0 : (u32)d->chg_first;
  u32 len = d->chg_length < 0 ? 0 : (u32)d->chg_length;
  u32* cps = NULL;
  u8* fbs = NULL;
  size_t n = 0;
  u32 only = 0;
  if (t != NULL && t->length > 0) {
    n = (size_t)t->length;
    cps = io_mem(calloc(n + 1, 4));
    fbs = io_mem(calloc(n + 1, 1));
    int has = t->encoding_is_wchar ? t->string.wide_char != NULL : t->string.multi_byte != NULL;
    only = has ? 0 : 1;
    if (has && t->encoding_is_wchar) {
      for (size_t i = 0; i < n; i++) cps[i] = win_cp((u32)t->string.wide_char[i]);
    } else if (has) {
      // the multibyte text is in the (UTF-8) locale's encoding
      size_t bl = strlen(t->string.multi_byte);
      u32* tmp = io_mem(calloc(bl + 1, 4));
      size_t m = win_utf8(t->string.multi_byte, bl, tmp);
      memcpy(cps, tmp, (m < n ? m : n) * 4);
      if (m < n) n = m;
      free(tmp);
    }
    for (size_t i = 0; i < n; i++) {
      fbs[i] = t->feedback ? (u8)((u32)t->feedback[i] & 63) : 2;
    }
  }
  if (!only) {
    u32 total = a->pe_len;
    u32 cut = first > total ? total : first;
    u32 end = cut + len > total ? total : cut + len;
    a->pe_len = total - (end - cut) + (u32)n;
  }
  a->pe_caret = d->caret < 0 ? 0 : (u32)d->caret;
  win_push(a, 15, first, len, win_payload(cps, fbs, n), (a->pe_caret & 0x7fffffffu) | (only << 31));
  free(cps);
  free(fbs);
}

static void win_pe_caret(XIC ic, XPointer client, XIMPreeditCaretCallbackStruct* c) {
  AppWin* a = (AppWin*)client;
  long pos = c->position;
  switch (c->direction) {
    case XIMForwardChar: pos = (long)a->pe_caret + 1; break;
    case XIMBackwardChar: pos = (long)a->pe_caret - 1; break;
    case XIMAbsolutePosition: break;
    default: pos = (long)a->pe_caret; break;
  }
  if (pos < 0) pos = 0;
  if (pos > (long)a->pe_len) pos = (long)a->pe_len;
  a->pe_caret = (u32)pos;
  c->position = (int)pos;
  win_push(a, 16, a->pe_caret, 0, 0, 0);
}

static void win_ime_open(AppWin* a);
static void win_ime_watch(AppWin* a);

// a method started after the window (or came back): open it
static void win_ime_back(Display* dpy, XPointer client, XPointer call) {
  AppWin* a = (AppWin*)client;
  if (a->xic == NULL) {
    win_ime_open(a);
  }
}

static void win_ime_watch(AppWin* a) {
  if (x_RegisterIMInstantiate && a->xic == NULL) {
    x_RegisterIMInstantiate(a->dpy, NULL, NULL, NULL, win_ime_back, (XPointer)a);
  }
}

// the method went away (its server died): no context or method is left to
// free; typing goes on plainly and a running composition is dropped
static void win_ime_lost(XIM im, XPointer client, XPointer call) {
  AppWin* a = (AppWin*)client;
  a->xic = NULL;
  a->xim = NULL;
  a->ime_mode = 0;
  a->pe_len = 0;
  a->pe_caret = 0;
  win_push(a, 18, 0, 0, 0, 0);
  if (getenv("BACKPLANE_IME") != NULL) fprintf(stderr, "backplane: input method lost, plain keys\n");
  win_ime_watch(a);
}

static const XIMStyle win_styles[] = {
  XIMPreeditCallbacks | XIMStatusNothing, XIMPreeditCallbacks | XIMStatusNone,
  XIMPreeditNothing | XIMStatusNothing, XIMPreeditNothing | XIMStatusNone,
  XIMPreeditNone | XIMStatusNone, XIMPreeditNone | XIMStatusNothing
};

// open the method and a context on the window; on any failure leave mode 0
static void win_ime_open(AppWin* a) {
  const char* dbg = getenv("BACKPLANE_IME");
  a->xim = NULL;
  a->xic = NULL;
  a->ime_mode = 0;
  if (!win_ime_ok || getenv("BACKPLANE_NO_IME") != NULL) {
    return;
  }
  // the locale from the environment; the text the method sends is UTF-8,
  // so a locale of another encoding is replaced for characters only
  setlocale(LC_CTYPE, "");
  if (strcmp(nl_langinfo(CODESET), "UTF-8") != 0 || !x_XSupportsLocale()) {
    if (setlocale(LC_CTYPE, "C.UTF-8") == NULL) setlocale(LC_CTYPE, "en_US.UTF-8");
  }
  if (!x_XSupportsLocale()) {
    if (dbg) fprintf(stderr, "backplane: input method: locale not supported by Xlib, plain keys\n");
    return;
  }
  x_XSetLocaleModifiers("");
  XIM xim = x_XOpenIM(a->dpy, NULL, NULL, NULL);
  if (xim == NULL) {
    if (dbg) fprintf(stderr, "backplane: input method: none running, plain keys\n");
    win_ime_watch(a);
    return;
  }
  XIMStyles* styles = NULL;
  if (x_XGetIMValues(xim, XNQueryInputStyle, &styles, NULL) != NULL || styles == NULL) {
    x_XCloseIM(xim);
    return;
  }
  XIMStyle want = 0;
  for (size_t k = 0; k < sizeof win_styles / sizeof win_styles[0] && want == 0; k++) {
    for (unsigned i = 0; i < styles->count_styles; i++) {
      if (styles->supported_styles[i] == win_styles[k]) { want = win_styles[k]; break; }
    }
  }
  x_XFree(styles);
  if (want == 0) {
    x_XCloseIM(xim);
    if (dbg) fprintf(stderr, "backplane: input method: no usable style, plain keys\n");
    return;
  }
  XIC ic;
  if (want & XIMPreeditCallbacks) {
    a->pe_cb[0] = (XIMCallback){ (XPointer)a, (XIMProc)win_pe_start };
    a->pe_cb[1] = (XIMCallback){ (XPointer)a, (XIMProc)win_pe_done };
    a->pe_cb[2] = (XIMCallback){ (XPointer)a, (XIMProc)win_pe_draw };
    a->pe_cb[3] = (XIMCallback){ (XPointer)a, (XIMProc)win_pe_caret };
    XVaNestedList nl = x_XVaCreateNestedList(0, XNPreeditStartCallback, &a->pe_cb[0],
      XNPreeditDoneCallback, &a->pe_cb[1], XNPreeditDrawCallback, &a->pe_cb[2],
      XNPreeditCaretCallback, &a->pe_cb[3], NULL);
    ic = x_XCreateIC(xim, XNInputStyle, want, XNClientWindow, a->win, XNFocusWindow, a->win,
      XNPreeditAttributes, nl, NULL);
    x_XFree(nl);
  } else {
    ic = x_XCreateIC(xim, XNInputStyle, want, XNClientWindow, a->win, XNFocusWindow, a->win, NULL);
  }
  if (ic == NULL) {
    x_XCloseIM(xim);
    if (dbg) fprintf(stderr, "backplane: input method: no context, plain keys\n");
    return;
  }
  a->xim = xim;
  a->xic = ic;
  a->ime_style = want;
  a->ime_mode = (want & XIMPreeditCallbacks) ? 1 : 2;
  // the events the method wants besides the keys
  unsigned long mask = 0;
  if (x_XGetICValues(ic, XNFilterEvents, &mask, NULL) == NULL && mask != 0) {
    XWindowAttributes wa;
    if (x_XGetWindowAttributes(a->dpy, a->win, &wa)) {
      x_XSelectInput(a->dpy, a->win, wa.your_event_mask | (long)mask);
    }
  }
  a->ds_cb = (XIMCallback){ (XPointer)a, (XIMProc)win_ime_lost };
  x_XSetIMValues(xim, XNDestroyCallback, &a->ds_cb, NULL);
  x_XSetICFocus(ic);
  if (dbg) {
    fprintf(stderr, "backplane: input method: %s\n", a->ime_mode == 1
      ? "preedit drawn by the app (callbacks)"
      : "preedit drawn by the method (no callbacks; fallback)");
  }
}

static void win_pump(AppWin* a) {
  while (x_XPending(a->dpy) > 0) {
    XEvent ev;
    x_XNextEvent(a->dpy, &ev);
    // the input method sees every event first; one it takes (a key of a
    // composition, its own protocol messages) is not the app's. A release
    // of a key the app saw go down is still reported, or it would stay down.
    if (a->xic != NULL && x_XFilterEvent(&ev, None)) {
      if (ev.type == KeyRelease) {
        unsigned fk = ev.xkey.keycode & 255;
        if (a->down[fk] != 0 && (a->detect || !win_key_down(a, fk))) {
          win_push(a, 0, (u32)a->down[fk], 0, win_mods(ev.xkey.state), fk << 1);
          a->down[fk] = 0;
        }
      }
      continue;
    }
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
        if (ev.type == KeyPress && a->xic != NULL) {
          // what the method (or a compose sequence) made of the key: text
          // from the method itself (keycode 0) or of several characters is
          // a commit; one character is what the key typed
          int plain = kc != 0 && !(ev.xkey.state & (ControlMask | Mod1Mask | Mod4Mask));
          if (kc == 0 || plain) {
            char sb[64];
            char* buf = sb;
            int cap = (int)sizeof sb;
            Status st = 0;
            KeySym ks = 0;
            int n = x_Xutf8LookupString(a->xic, &ev.xkey, buf, cap, &ks, &st);
            if (st == XBufferOverflow) {
              cap = n + 1;
              buf = io_mem(malloc((size_t)cap));
              n = x_Xutf8LookupString(a->xic, &ev.xkey, buf, cap, &ks, &st);
            }
            if ((st == XLookupChars || st == XLookupBoth) && n > 0) {
              size_t nc = win_utf8(buf, (size_t)n, NULL);
              u32 one = 0;
              if (nc == 1) win_utf8(buf, (size_t)n, &one);
              if (kc == 0 || nc > 1) {
                win_commit(a, buf, (size_t)n);
                if (buf != sb) free(buf);
                break;
              }
              if (one >= 32 && one != 127) text = one;
            }
            if (buf != sb) free(buf);
            if (kc == 0) break;
          }
        }
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
        if (ev.type == ButtonPress) {
          Window focus; int revert;
          x_XGetInputFocus(a->dpy, &focus, &revert);
          if (focus != a->win) x_XSetInputFocus(a->dpy, a->win, RevertToParent, CurrentTime);
        }
        u32 b = ev.xbutton.button;
        u32 x = ev.xbutton.x < 0 ? 0 : (u32)ev.xbutton.x;
        u32 y = ev.xbutton.y < 0 ? 0 : (u32)ev.xbutton.y;
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
        win_push(a, 2, ev.xmotion.x < 0 ? 0 : (u32)ev.xmotion.x,
          ev.xmotion.y < 0 ? 0 : (u32)ev.xmotion.y, win_mods(ev.xmotion.state), 0);
        break;
      case FocusOut:
        if (a->xic != NULL) {
          // a composition does not outlive the focus: the method drops it
          // (what it would have committed is thrown away)
          x_XUnsetICFocus(a->xic);
          char* left = x_Xutf8ResetIC(a->xic);
          if (left != NULL) x_XFree(left);
          a->pe_len = 0;
          a->pe_caret = 0;
        }
        win_push(a, 11, 0, 0, 0, 0);
        break;
      case FocusIn:
        // (a method that was not there when the window opened, or went away,
        // is looked for again whenever the window gets the focus)
        if (a->xic != NULL) x_XSetICFocus(a->xic);
        else win_ime_open(a);
        win_keys_missed(a);
        win_push(a, 12, 0, 0, 0, 0);
        break;
      case ConfigureNotify:
        if ((u32)ev.xconfigure.width != a->w || (u32)ev.xconfigure.height != a->h) {
          a->w = (u32)ev.xconfigure.width;
          a->h = (u32)ev.xconfigure.height;
          win_push(a, 4, a->w, a->h, 0, 0);
        }
        break;
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
        } else if (ev.xselection.selection == a->clip && a->paste_busy && ev.xselection.target == a->targets) {
          // Refusing TARGETS is legal: retain UTF8_STRING/STRING fallback.
          win_paste_targets(a);
        } else if (ev.xselection.selection == a->clip && a->paste_busy && ev.xselection.property == a->prop) {
          win_pasted(a);
        } else if (ev.xselection.selection == a->clip && a->paste_busy && ev.xselection.target == a->utf8) {
          // an owner without UTF8_STRING may still have plain STRING
          x_XConvertSelection(a->dpy, a->clip, XA_STRING, a->prop, a->win, CurrentTime);
          x_XFlush(a->dpy);
        } else if (ev.xselection.selection == a->clip) {
          a->paste_busy = 0;
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
#ifdef CID(Win.words)
static Term win_list(Env e, AppWin* a) {
  Term list = term_pak(CID_NIL, 0);
  pthread_mutex_lock(&win_qlock);
  for (u64 i = (u64)a->n * 5; i > 0;) {
    i -= 1;
    list = io_node(e, CID_CON, (Term)a->evs[i], list);
  }
  a->n = 0;
  pthread_mutex_unlock(&win_qlock);
  return list;
}
#endif

#ifdef CID(Win.open)

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
  } else if ((dpy = (x_InitThreads != NULL ? (void)x_InitThreads() : (void)0, x_XOpenDisplay(NULL))) == NULL) {
    // (Xlib is made thread-safe first: the pump and the presenting thread
    // and the input method's callbacks all call it)
    code = ENOTSUP;
    why = "no display: run Backplane from a desktop session, or use `backplane serve`";
  } else {
    int scr = DefaultScreen(dpy);
    a = io_mem(calloc(1, sizeof *a));
    a->dpy = dpy;
    a->w = ww;
    a->h = hh;
    a->win = x_XCreateSimpleWindow(dpy, RootWindow(dpy, scr), 0, 0, ww, hh, 0, 0,
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
    win_ime_open(a);
    x_XFlush(dpy);
  }
  free(title);
  if (code) {
    return io_fail(e, code, why);
  }
  return io_done(e, io_hand((intptr_t)a));
}

static void __attribute__((constructor)) win_open_use(void) {
  io_eff(CID(Win.open), win_open_run);
}

#endif

#ifdef CID(Win.words)

static Term win_words_more(Env e, IoWork* w) {
  AppWin* a = (AppWin*)w->hand;
  win_pump(a);
  pthread_mutex_lock(&win_qlock);
  u32 pending = a->n;
  pthread_mutex_unlock(&win_qlock);
  if (pending == 0) {
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
  io_eff(CID(Win.words), win_words_run);
}

#endif

#ifdef CID(Win.present)

// fill the part of an n x n region at (x, y) that lies in the w x h frame,
// marking each 64 px block in which a pixel changed (dirty is dw wide)
// one pixel of a 1 x 1 region (a split one reads its first quadrant, as
// win_fill does)
static inline void win_px(u64* H, u32* pix, u32 w, u32 h, u8* dirty, u32 dw, Term t, u32 x, u32 y) {
  if (x >= w || y >= h) {
    return;
  }
  while (term_tag(t) == TAG_CTR) {
    u64 l = term_rfc(t) ? H[term_loc(t)] >> 24 : term_loc(t);
    t = H[l];
  }
  u32 c = (u32)term_loc(t) & 0xFFFFFF;
  u32* p = pix + (size_t)y * w + x;
  if (*p != c) {
    *p = c;
    dirty[(size_t)(y >> 6) * dw + (x >> 6)] = 1;
  }
}

static void win_fill(u64* H, u32* pix, u32 w, u32 h, u8* dirty, u32 dw, Term t, u32 x, u32 y, u32 n) {
  if (x >= w || y >= h) {
    return;
  }
  // a split 2 x 2 region: its four pixels at once (most of a frame's text
  // is regions this small; no call per pixel)
  if (term_tag(t) == TAG_CTR && n == 2) {
    u64 l = term_rfc(t) ? H[term_loc(t)] >> 24 : term_loc(t);
    win_px(H, pix, w, h, dirty, dw, H[l + 0], x, y);
    win_px(H, pix, w, h, dirty, dw, H[l + 1], x + 1, y);
    win_px(H, pix, w, h, dirty, dw, H[l + 2], x, y + 1);
    win_px(H, pix, w, h, dirty, dw, H[l + 3], x + 1, y + 1);
    return;
  }
  if (term_tag(t) == TAG_CTR && n > 1) {
    u64 l = term_rfc(t) ? H[term_loc(t)] >> 24 : term_loc(t);
    u32 m = n / 2;
    win_fill(H, pix, w, h, dirty, dw, H[l + 0], x, y, m);
    win_fill(H, pix, w, h, dirty, dw, H[l + 1], x + m, y, m);
    win_fill(H, pix, w, h, dirty, dw, H[l + 2], x, y + m, m);
    win_fill(H, pix, w, h, dirty, dw, H[l + 3], x + m, y + m, m);
    return;
  }
  while (term_tag(t) == TAG_CTR) {
    u64 l = term_rfc(t) ? H[term_loc(t)] >> 24 : term_loc(t);
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

// readings for the perf table (effects/perf.c's, when the program has it)
extern void bp_perf_rec(const char* name, uint64_t v, int span) __attribute__((weak));

static uint64_t win_us(void) {
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (uint64_t)ts.tv_sec * 1000000 + (uint64_t)ts.tv_nsec / 1000;
}

static void win_rec(const char* name, uint64_t v, int span) {
  if (bp_perf_rec) {
    bp_perf_rec(name, v, span);
  }
}

// rectangles and pixels sent by the last win_put
static u32 win_put_rects = 0;
static u64 win_put_px = 0;

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
      win_put_rects += 1;
      win_put_px += (u64)(x1 - x0) * (y1 - y0);
    }
  }
}

// the node a term names (a shared one through its count cell)
static inline u64 win_src(u64* H, Term t) {
  return term_rfc(t) ? H[term_loc(t)] >> 24 : term_loc(t);
}

// win_fill over the regions where the frame differs from the last one
// shown (old, whose pixels the buffer holds): a region that is the very
// same node in both is skipped. Both trees are alive here, so one node
// can't stand for two images.
static void win_fill_diff(u64* H, u32* pix, u32 w, u32 h, u8* dirty, u32 dw, Term t, Term old, u32 x, u32 y, u32 n) {
  if (x >= w || y >= h) {
    return;
  }
  if (term_tag(t) != TAG_CTR || n <= 1 || term_tag(old) != TAG_CTR) {
    win_fill(H, pix, w, h, dirty, dw, t, x, y, n);
    return;
  }
  u64 l = win_src(H, t);
  u64 o = win_src(H, old);
  if (l == o) {
    return;
  }
  u32 m = n / 2;
  win_fill_diff(H, pix, w, h, dirty, dw, H[l + 0], H[o + 0], x, y, m);
  win_fill_diff(H, pix, w, h, dirty, dw, H[l + 1], H[o + 1], x + m, y, m);
  win_fill_diff(H, pix, w, h, dirty, dw, H[l + 2], H[o + 2], x, y + m, m);
  win_fill_diff(H, pix, w, h, dirty, dw, H[l + 3], H[o + 3], x + m, y + m, m);
}

// Filling in parallel
// -------------------
// A frame whose changed regions are large (a thread opened, a page of it
// scrolled in) spends most of its present walking the new tiles' trees
// pixel by pixel. The regions 128 px wide that differ from the last frame
// are collected and, when they cover enough, filled by a few threads at
// once: each region owns its pixels and its 64 px blocks (dirty flags),
// so nothing is shared but the heap they read.

#define WIN_PAR_MAX 256
#define WIN_PAR_SIDE 128

typedef struct {
  Term t, old;
  u32 x, y, n;
} WinPart;

typedef struct {
  u64* H;
  u32* pix;
  u32 w, h, dw;
  u8* dirty;
  WinPart* parts;
  u32 np;
  u32 lanes, lane;
} WinLane;

// the regions WIN_PAR_SIDE wide (or leaves above that size) where t and
// old differ, onto parts; answers how many
static u32 win_parts(u64* H, u32 w, u32 h, Term t, Term old, u32 x, u32 y, u32 n, WinPart* parts, u32 np) {
  if (x >= w || y >= h || np >= WIN_PAR_MAX) {
    return np;
  }
  if (term_tag(t) == TAG_CTR && term_tag(old) == TAG_CTR && n > 1 && win_src(H, t) == win_src(H, old)) {
    return np;
  }
  if (n <= WIN_PAR_SIDE || term_tag(t) != TAG_CTR || term_tag(old) != TAG_CTR) {
    parts[np] = (WinPart){ t, old, x, y, n };
    return np + 1;
  }
  u64 l = win_src(H, t);
  u64 o = win_src(H, old);
  u32 m = n / 2;
  np = win_parts(H, w, h, H[l + 0], H[o + 0], x, y, m, parts, np);
  np = win_parts(H, w, h, H[l + 1], H[o + 1], x + m, y, m, parts, np);
  np = win_parts(H, w, h, H[l + 2], H[o + 2], x, y + m, m, parts, np);
  np = win_parts(H, w, h, H[l + 3], H[o + 3], x + m, y + m, m, parts, np);
  return np;
}

static void* win_lane(void* arg) {
  WinLane* ln = (WinLane*)arg;
  for (u32 i = ln->lane; i < ln->np; i += ln->lanes) {
    WinPart* p = &ln->parts[i];
    win_fill_diff(ln->H, ln->pix, ln->w, ln->h, ln->dirty, ln->dw, p->t, p->old, p->x, p->y, p->n);
  }
  return NULL;
}

// win_fill_diff over the frame, the large changes split over threads
static void win_fill_par(u64* H, u32* pix, u32 w, u32 h, u8* dirty, u32 dw, Term t, Term old, u32 n) {
  static WinPart parts[WIN_PAR_MAX];
  u32 np = win_parts(H, w, h, t, old, 0, 0, n, parts, 0);
  u32 lanes = np / 4;
  if (np >= WIN_PAR_MAX || lanes < 2) {
    if (np >= WIN_PAR_MAX) {
      win_fill_diff(H, pix, w, h, dirty, dw, t, old, 0, 0, n);
      return;
    }
    for (u32 i = 0; i < np; i += 1) {
      win_fill_diff(H, pix, w, h, dirty, dw, parts[i].t, parts[i].old, parts[i].x, parts[i].y, parts[i].n);
    }
    return;
  }
  if (lanes > 8) {
    lanes = 8;
  }
  WinLane ls[8];
  pthread_t th[8];
  int ok[8];
  for (u32 k = 0; k < lanes; k += 1) {
    ls[k] = (WinLane){ H, pix, w, h, dw, dirty, parts, np, lanes, k };
    ok[k] = k == 0 ? 0 : pthread_create(&th[k], NULL, win_lane, &ls[k]) == 0;
  }
  win_lane(&ls[0]);
  for (u32 k = 1; k < lanes; k += 1) {
    if (ok[k]) {
      pthread_join(th[k], NULL);
    } else {
      win_lane(&ls[k]);
    }
  }
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

// fill the buffer with image (over old where it may skip, has_old) and
// send what changed
static void win_show_image(Env e, AppWin* a, Term image, Term old, int has_old) {
  uint64_t ta = win_us();
  io_sync();
  uint64_t tb = win_us();
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
    win_fill_par(e.mem, (u32*)a->img->data, a->w, a->h, a->dirty, a->dw, image, old, n);
  } else {
    win_fill(e.mem, (u32*)a->img->data, a->w, a->h, a->dirty, a->dw, image, 0, 0, n);
  }
  a->stale = !has_old;
  uint64_t tc = win_us();
  win_put_rects = 0;
  win_put_px = 0;
  if (whole) {
    memset(a->dirty, 0, (size_t)a->dw * a->dh);
    x_XPutImage(a->dpy, a->win, DefaultGC(a->dpy, DefaultScreen(a->dpy)), a->img,
      0, 0, 0, 0, a->w, a->h);
    win_put_rects = 1;
    win_put_px = (u64)a->w * a->h;
  } else {
    win_put(a);
  }
  uint64_t td = win_us();
  x_XFlush(a->dpy);
  win_nudge(a);
  uint64_t te = win_us();
  win_snap(a);
  // ui.win.*: the wait for pending effects, the buffer filled from the
  // image, the rectangles put, the flush (us); rects and kpx per frame
  win_rec("ui.win.sync", tb - ta, 1);
  win_rec("ui.win.fill", tc - tb, 1);
  win_rec("ui.win.put", td - tc, 1);
  win_rec("ui.win.flush", te - td, 1);
  win_rec("ui.win.rects", win_put_rects, 0);
  win_rec("ui.win.kpx", win_put_px / 1000, 0);
}

// a frame that is not the window's own (the lightbox): the next show
// fills whole
Term win_present_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  win_show_image(e, a, f[1], 0, 0);
  return io_tup(e, f[0], f[1]);
}

static void __attribute__((constructor)) win_present_use(void) {
  io_eff(CID(Win.present), win_present_run);
}

#endif

#ifdef CID(Win.show)

// the window's frame, and the last one it showed (the buffer holds it)
Term win_show_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  win_show_image(e, a, f[1], f[2], 1);
  return io_tup(e, f[0], io_tup(e, f[1], f[2]));
}

static void __attribute__((constructor)) win_show_use(void) {
  io_eff(CID(Win.show), win_show_run);
}

#endif

#ifdef CID(Win.copy)

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
  io_eff(CID(Win.copy), win_copy_run);
}

#endif

#ifdef CID(Win.paste)

// ask for the clipboard; it arrives later as WPaste. Only while this
// window still owns it is the last copy pasted directly; otherwise the
// owner (another program) is asked.
Term win_paste_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  if (a->paste_busy || a->paste_ready) return f[0];
  free(a->paste_mime);
  a->paste_mime = NULL;
  if (a->copy != NULL && x_XGetSelectionOwner(a->dpy, a->clip) == a->win) {
    free(a->paste);
    a->paste = io_mem(malloc(a->copy_len + 1));
    memcpy(a->paste, a->copy, a->copy_len);
    a->paste_len = a->copy_len;
    a->paste_ready = 1;
    win_push(a, 7, 0, 0, 0, 0);
  } else {
    a->paste_busy = 1;
    x_XDeleteProperty(a->dpy, a->win, a->prop);
    x_XConvertSelection(a->dpy, a->clip, a->targets, a->prop, a->win, CurrentTime);
    x_XFlush(a->dpy);
  }
  win_nudge(a);
  return f[0];
}

static void __attribute__((constructor)) win_paste_use(void) {
  io_eff(CID(Win.paste), win_paste_run);
}

#endif

#ifdef CID(Win.pasted)

// the text of the last paste event
Term win_pasted_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  a->paste_ready = 0;
  return io_tup(e, f[0], io_str(e, a->paste ? a->paste : "", a->paste ? a->paste_len : 0));
}

static void __attribute__((constructor)) win_pasted_use(void) {
  io_eff(CID(Win.pasted), win_pasted_run);
}

#endif

#ifdef CID(Win.pasted_image)

// Bytes never enter a Bend String: only the ASCII base64 representation.
Term win_pasted_image_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  static const char alphabet[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  u64 n = a->paste_len <= 10485760 ? a->paste_len : 0;
  u64 len = ((n + 2) / 3) * 4;
  char* out = io_mem(malloc(len + 1));
  for (u64 i = 0, j = 0; i < n; i += 3) {
    u32 b = (u32)(u8)a->paste[i] << 16;
    if (i + 1 < n) b |= (u32)(u8)a->paste[i + 1] << 8;
    if (i + 2 < n) b |= (u8)a->paste[i + 2];
    out[j++] = alphabet[b >> 18];
    out[j++] = alphabet[(b >> 12) & 63];
    out[j++] = i + 1 < n ? alphabet[(b >> 6) & 63] : '=';
    out[j++] = i + 2 < n ? alphabet[b & 63] : '=';
  }
  const char* mime = a->paste_mime ? a->paste_mime : "";
  Term result = io_tup(e, f[0], io_tup(e, io_str(e, mime, strlen(mime)),
    io_tup(e, (Term)(u32)(a->paste_len > UINT32_MAX ? UINT32_MAX : a->paste_len), io_str(e, out, len))));
  a->paste_ready = 0;
  free(out);
  free(a->paste);
  a->paste = NULL;
  a->paste_len = 0;
  return result;
}
static void __attribute__((constructor)) win_pasted_image_use(void) {
  io_eff(CID(Win.pasted_image), win_pasted_image_run);
}
#endif

#ifdef CID(Win.dropped)

// the text/uri-list of the last drop event
Term win_dropped_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  return io_tup(e, f[0], io_str(e, a->drop ? a->drop : "", a->drop ? a->drop_len : 0));
}

static void __attribute__((constructor)) win_dropped_use(void) {
  io_eff(CID(Win.dropped), win_dropped_run);
}

#endif

#ifdef CID(Win.size)

Term win_size_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  return io_tup(e, f[0], io_tup(e, (Term)a->w, (Term)a->h));
}

static void __attribute__((constructor)) win_size_use(void) {
  io_eff(CID(Win.size), win_size_run);
}

#endif

#ifdef CID(Win.twin)

// a second handle on the same window: one computation waits for events
// while another presents frames
Term win_twin_run(Env e, Term* f, IoWork* w) {
  return io_tup(e, f[0], f[0]);
}

static void __attribute__((constructor)) win_twin_use(void) {
  io_eff(CID(Win.twin), win_twin_run);
}

#endif

#ifdef CID(Win.ime)

// how the input method is used: 0 plain keys, 1 preedit drawn by the app,
// 2 preedit drawn by the method
Term win_ime_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  return io_tup(e, f[0], (Term)(u32)a->ime_mode);
}

static void __attribute__((constructor)) win_ime_use(void) {
  io_eff(CID(Win.ime), win_ime_run);
}

#endif

#ifdef CID(Win.payload)

// the text of an ime event (and its feedback, '0' + bits a character), once
Term win_payload_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  WinPayload* p = win_payload_take((u32)f[1]);
  size_t n = p ? p->n : 0;
  char* out = io_mem(malloc(n * 4 + 1));
  char* fb = io_mem(malloc(n + 1));
  size_t k = 0;
  for (size_t i = 0; i < n; i++) {
    u32 c = p->cps[i];
    if (c < 0x80) { out[k++] = (char)c; }
    else if (c < 0x800) { out[k++] = (char)(0xc0 | (c >> 6)); out[k++] = (char)(0x80 | (c & 63)); }
    else if (c < 0x10000) { out[k++] = (char)(0xe0 | (c >> 12)); out[k++] = (char)(0x80 | ((c >> 6) & 63)); out[k++] = (char)(0x80 | (c & 63)); }
    else { out[k++] = (char)(0xf0 | (c >> 18)); out[k++] = (char)(0x80 | ((c >> 12) & 63)); out[k++] = (char)(0x80 | ((c >> 6) & 63)); out[k++] = (char)(0x80 | (c & 63)); }
    fb[i] = (char)('0' + (p->fb[i] & 63));
  }
  Term r = io_tup(e, f[0], io_tup(e, io_str(e, out, k), io_str(e, fb, n)));
  if (p) { free(p->cps); free(p->fb); free(p); }
  free(out);
  free(fb);
  return r;
}

static void __attribute__((constructor)) win_payload_use(void) {
  io_eff(CID(Win.payload), win_payload_run);
}

#endif

#ifdef CID(Win.ime_reset)

// the field with the keys changed: a composition in the method is dropped,
// whatever it would have committed is thrown away
Term win_ime_reset_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  if (a->xic != NULL) {
    char* left = x_Xutf8ResetIC(a->xic);
    if (left != NULL) x_XFree(left);
    a->pe_len = 0;
    a->pe_caret = 0;
    x_XFlush(a->dpy);
    win_nudge(a);
  }
  return f[0];
}

static void __attribute__((constructor)) win_ime_reset_use(void) {
  io_eff(CID(Win.ime_reset), win_ime_reset_run);
}

#endif

#ifdef CID(Win.ime_spot)

// where the caret is (window pixels), for the method's candidate window
Term win_ime_spot_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  if (a->xic != NULL && ((int)(u32)f[1] != a->spot_x || (int)(u32)f[2] != a->spot_y)) {
    a->spot_x = (int)(u32)f[1];
    a->spot_y = (int)(u32)f[2];
    XPoint pt = { (short)(u32)f[1], (short)(u32)f[2] };
    XVaNestedList nl = x_XVaCreateNestedList(0, XNSpotLocation, &pt, NULL);
    x_XSetICValues(a->xic, XNPreeditAttributes, nl, NULL);
    x_XFree(nl);
    x_XFlush(a->dpy);
    win_nudge(a);
  }
  return f[0];
}

static void __attribute__((constructor)) win_ime_spot_use(void) {
  io_eff(CID(Win.ime_spot), win_ime_spot_run);
}

#endif

#ifdef CID(Win.ime_inject)

// queue what the input method would (tests): kind 0 a commit of the text,
// 1 a preedit replaced whole by it (underlined), 2 the preedit done
Term win_ime_inject_run(Env e, Term* f, IoWork* w) {
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  u64 len = 0;
  char* t = io_cstr(e, f[2], &len);
  u32 kind = (u32)f[1];
  if (kind == 0) {
    win_commit(a, t, (size_t)len);
  } else if (kind == 1) {
    u32* cps = io_mem(malloc((len ? len : 1) * 4));
    u8* fbs = io_mem(malloc((len ? len : 1)));
    size_t n = win_utf8(t, (size_t)len, cps);
    memset(fbs, 2, n);
    win_push(a, 15, 0, a->pe_len, win_payload(cps, fbs, n), (u32)n);
    a->pe_len = (u32)n;
    free(cps);
    free(fbs);
  } else {
    a->pe_len = 0;
    win_push(a, 18, 0, 0, 0, 0);
  }
  free(t);
  return f[0];
}

static void __attribute__((constructor)) win_ime_inject_use(void) {
  io_eff(CID(Win.ime_inject), win_ime_inject_run);
}

#endif

#ifdef CID(Win.title)

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
  io_eff(CID(Win.title), win_title_run);
}

#endif

#elif defined(__CYGWIN__)

// Windows desktop backend. Cygwin supplies the POSIX runtime used by Bend;
// the window, input, clipboard and presentation use Win32, without X11.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <windowsx.h>
#include <wchar.h>
#include <shellapi.h>
#include <imm.h>
#include <sys/cygwin.h>
#include <fcntl.h>

#define BPW_PAINT (WM_APP + 1)
#define BPW_COPY  (WM_APP + 2)
#define BPW_PASTE (WM_APP + 3)
#define BPW_TITLE (WM_APP + 4)

typedef struct {
  HWND hwnd;
  pthread_mutex_t lock;
  pthread_cond_t ready;
  int opened, error, notify[2];
  u32 width, height, pw, ph;
  u32 *events, count, capacity, *pixels;
  wchar_t *title, *copy;
  char *paste, *drop;
  wchar_t surrogate;
} BpWin;

static wchar_t* bpw_wide(const char* text) {
  int n = MultiByteToWideChar(CP_UTF8, 0, text, -1, NULL, 0);
  wchar_t* result = io_mem(calloc(n > 0 ? n : 1, sizeof(wchar_t)));
  if (n > 0) MultiByteToWideChar(CP_UTF8, 0, text, -1, result, n);
  return result;
}

static char* bpw_utf8(const wchar_t* text) {
  int n = WideCharToMultiByte(CP_UTF8, 0, text, -1, NULL, 0, NULL, NULL);
  char* result = io_mem(calloc(n > 0 ? n : 1, 1));
  if (n > 0) WideCharToMultiByte(CP_UTF8, 0, text, -1, result, n, NULL, NULL);
  return result;
}

static void bpw_push(BpWin* a, u32 kind, u32 p, u32 q, u32 r, u32 s) {
  pthread_mutex_lock(&a->lock);
  if (a->count + 5 > a->capacity) {
    a->capacity = a->capacity ? a->capacity * 2 : 320;
    a->events = io_mem(realloc(a->events, a->capacity * sizeof(u32)));
  }
  u32 ev[5] = {kind, p, q, r, s};
  memcpy(a->events + a->count, ev, sizeof ev);
  if (a->count == 0) {
    char byte = 1;
    (void)write(a->notify[1], &byte, 1);
  }
  a->count += 5;
  pthread_mutex_unlock(&a->lock);
}

static u32 bpw_mods(void) {
  return ((GetKeyState(VK_SHIFT) & 0x8000) ? 1 : 0)
    | ((GetKeyState(VK_CONTROL) & 0x8000) ? 2 : 0)
    | ((GetKeyState(VK_MENU) & 0x8000) ? 4 : 0)
    | (((GetKeyState(VK_LWIN) | GetKeyState(VK_RWIN)) & 0x8000) ? 8 : 0);
}

// Keep the portable input contract: navigation keys use X keysyms.
static u32 bpw_key(WPARAM key) {
  switch (key) {
    case VK_BACK: return 0xff08; case VK_TAB: return 0xff09;
    case VK_RETURN: return 0xff0d; case VK_ESCAPE: return 0xff1b;
    case VK_HOME: return 0xff50; case VK_LEFT: return 0xff51;
    case VK_UP: return 0xff52; case VK_RIGHT: return 0xff53;
    case VK_DOWN: return 0xff54; case VK_PRIOR: return 0xff55;
    case VK_NEXT: return 0xff56; case VK_END: return 0xff57;
    case VK_INSERT: return 0xff63; case VK_DELETE: return 0xffff;
    case VK_SHIFT: return 0xffe1; case VK_CONTROL: return 0xffe3;
    case VK_MENU: return 0xffe9; case VK_LWIN: return 0xffeb;
    case VK_RWIN: return 0xffec;
  }
  if (key >= VK_F1 && key <= VK_F24) return 0xffbe + key - VK_F1;
  return key >= 'A' && key <= 'Z' ? key + ('a' - 'A') : (u32)key;
}

static void bpw_text(BpWin* a, wchar_t c) {
  u32 cp = c;
  if (c >= 0xd800 && c <= 0xdbff) { a->surrogate = c; return; }
  if (c >= 0xdc00 && c <= 0xdfff && a->surrogate) {
    cp = 0x10000 + ((a->surrogate - 0xd800) << 10) + c - 0xdc00;
  }
  a->surrogate = 0;
  if (cp >= 32 && cp != 127 && !(bpw_mods() & 2))
    bpw_push(a, 0, 0, cp, bpw_mods(), 1);
}

static void bpw_clipboard(BpWin* a, int copy) {
  if (!OpenClipboard(a->hwnd)) return;
  if (copy) {
    pthread_mutex_lock(&a->lock);
    if (a->copy) {
      size_t n = (wcslen(a->copy) + 1) * sizeof(wchar_t);
      HGLOBAL data = GlobalAlloc(GMEM_MOVEABLE, n);
      void* dest = data ? GlobalLock(data) : NULL;
      if (dest) {
        memcpy(dest, a->copy, n); GlobalUnlock(data);
        if (!EmptyClipboard() || !SetClipboardData(CF_UNICODETEXT, data)) GlobalFree(data);
      } else if (data) GlobalFree(data);
    }
    pthread_mutex_unlock(&a->lock);
  } else {
    HANDLE data = GetClipboardData(CF_UNICODETEXT);
    const wchar_t* text = data ? GlobalLock(data) : NULL;
    if (text) {
      char* utf8 = bpw_utf8(text);
      GlobalUnlock(data);
      pthread_mutex_lock(&a->lock);
      free(a->paste); a->paste = utf8;
      pthread_mutex_unlock(&a->lock);
      bpw_push(a, 7, 0, 0, 0, 0);
    }
  }
  CloseClipboard();
}

static void bpw_drop(BpWin* a, HDROP files) {
  UINT count = DragQueryFileW(files, 0xffffffff, NULL, 0);
  char* result = io_mem(calloc(1, 1)); size_t size = 0;
  for (UINT i = 0; i < count; i++) {
    UINT n = DragQueryFileW(files, i, NULL, 0);
    wchar_t* path = io_mem(calloc(n + 1, sizeof(wchar_t)));
    DragQueryFileW(files, i, path, n + 1);
    ssize_t need = cygwin_conv_path(CCP_WIN_W_TO_POSIX, path, NULL, 0);
    if (need > 0) {
      char* posix = io_mem(malloc(need));
      if (cygwin_conv_path(CCP_WIN_W_TO_POSIX, path, posix, need) == 0) {
        size_t len = strlen(posix);
        result = io_mem(realloc(result, size + len * 3 + 10));
        memcpy(result + size, "file://", 7); size += 7;
        for (size_t k = 0; k < len; k++) {
          unsigned char c = (unsigned char)posix[k];
          if (c == '/' || c == '-' || c == '_' || c == '.' || c == '~'
              || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9')) result[size++] = c;
          else { static const char hex[] = "0123456789ABCDEF"; result[size++] = '%'; result[size++] = hex[c >> 4]; result[size++] = hex[c & 15]; }
        }
        result[size++] = '\n'; result[size] = 0;
      }
      free(posix);
    }
    free(path);
  }
  DragFinish(files);
  pthread_mutex_lock(&a->lock); free(a->drop); a->drop = result; pthread_mutex_unlock(&a->lock);
  bpw_push(a, 10, 0, 0, 0, 0);
}

static LRESULT CALLBACK bpw_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
  BpWin* a = (BpWin*)GetWindowLongPtrW(hwnd, GWLP_USERDATA);
  if (msg == WM_NCCREATE) {
    a = (BpWin*)((CREATESTRUCTW*)lp)->lpCreateParams;
    a->hwnd = hwnd; SetWindowLongPtrW(hwnd, GWLP_USERDATA, (LONG_PTR)a);
  }
  if (!a) return DefWindowProcW(hwnd, msg, wp, lp);
  switch (msg) {
    case WM_CLOSE: bpw_push(a, 6, 0, 0, 0, 0); DestroyWindow(hwnd); return 0;
    case WM_DESTROY: PostQuitMessage(0); return 0;
    case WM_ERASEBKGND: return 1;
    case WM_SIZE:
      if (wp != SIZE_MINIMIZED && LOWORD(lp) && HIWORD(lp)) {
        pthread_mutex_lock(&a->lock); a->width = LOWORD(lp); a->height = HIWORD(lp); pthread_mutex_unlock(&a->lock);
        bpw_push(a, 4, LOWORD(lp), HIWORD(lp), 0, 0);
      }
      return 0;
    case WM_SETFOCUS: bpw_push(a, 12, 0, 0, 0, 0); return 0;
    case WM_KILLFOCUS: a->surrogate = 0; bpw_push(a, 11, 0, 0, 0, 0); return 0;
    case WM_KEYDOWN: case WM_SYSKEYDOWN: case WM_KEYUP: case WM_SYSKEYUP: {
      u32 down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
      u32 scan = ((u32)lp >> 16) & 0x1ff;
      bpw_push(a, 0, bpw_key(wp), 0, bpw_mods(), scan * 2 + down);
      // Alt-F4 remains a normal Windows close action.
      if (msg == WM_SYSKEYDOWN && wp == VK_F4) return DefWindowProcW(hwnd, msg, wp, lp);
      return 0;
    }
    case WM_CHAR: bpw_text(a, (wchar_t)wp); return 0;
    case WM_SYSCHAR: return 0;
    case WM_MOUSEMOVE: bpw_push(a, 2, GET_X_LPARAM(lp), GET_Y_LPARAM(lp), bpw_mods(), 0); return 0;
    case WM_LBUTTONDOWN: case WM_RBUTTONDOWN: case WM_MBUTTONDOWN:
    case WM_LBUTTONUP: case WM_RBUTTONUP: case WM_MBUTTONUP: {
      int down = msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN || msg == WM_MBUTTONDOWN;
      u32 button = msg == WM_LBUTTONDOWN || msg == WM_LBUTTONUP ? 1 : msg == WM_MBUTTONDOWN || msg == WM_MBUTTONUP ? 2 : 3;
      if (down) { SetFocus(hwnd); SetCapture(hwnd); } else ReleaseCapture();
      bpw_push(a, 1, GET_X_LPARAM(lp), GET_Y_LPARAM(lp), button, down); return 0;
    }
    case WM_MOUSEWHEEL: case WM_MOUSEHWHEEL: {
      POINT pos = { GET_X_LPARAM(lp), GET_Y_LPARAM(lp) }; ScreenToClient(hwnd, &pos);
      int delta = GET_WHEEL_DELTA_WPARAM(wp);
      u32 dir = msg == WM_MOUSEWHEEL ? (delta > 0 ? 0 : 1) : (delta > 0 ? 3 : 2);
      for (int i = 0; i < abs(delta); i += WHEEL_DELTA) bpw_push(a, 3, pos.x, pos.y, dir, 0);
      return 0;
    }
    case WM_DROPFILES: bpw_drop(a, (HDROP)wp); return 0;
    case BPW_COPY: bpw_clipboard(a, 1); return 0;
    case BPW_PASTE: bpw_clipboard(a, 0); return 0;
    case BPW_TITLE:
      pthread_mutex_lock(&a->lock); SetWindowTextW(hwnd, a->title); pthread_mutex_unlock(&a->lock); return 0;
    case BPW_PAINT: InvalidateRect(hwnd, NULL, FALSE); return 0;
    case WM_PAINT: {
      PAINTSTRUCT paint; HDC dc = BeginPaint(hwnd, &paint);
      pthread_mutex_lock(&a->lock);
      if (a->pixels) {
        BITMAPINFO bi = {0}; bi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
        bi.bmiHeader.biWidth = a->pw; bi.bmiHeader.biHeight = -(LONG)a->ph;
        bi.bmiHeader.biPlanes = 1; bi.bmiHeader.biBitCount = 32; bi.bmiHeader.biCompression = BI_RGB;
        SetDIBitsToDevice(dc, 0, 0, a->pw, a->ph, 0, 0, 0, a->ph, a->pixels, &bi, DIB_RGB_COLORS);
      }
      pthread_mutex_unlock(&a->lock); EndPaint(hwnd, &paint); return 0;
    }
  }
  return DefWindowProcW(hwnd, msg, wp, lp);
}

static void* bpw_thread(void* arg) {
  BpWin* a = arg;
  HINSTANCE instance = GetModuleHandleW(NULL);
  WNDCLASSW wc = {0}; wc.lpfnWndProc = bpw_proc; wc.hInstance = instance;
  wc.lpszClassName = L"BackplaneNativeWindow"; wc.hCursor = LoadCursorW(NULL, MAKEINTRESOURCEW(32512));
  RegisterClassW(&wc);
  RECT rect = {0, 0, a->width, a->height}; AdjustWindowRect(&rect, WS_OVERLAPPEDWINDOW, FALSE);
  HWND hwnd = CreateWindowExW(0, wc.lpszClassName, a->title, WS_OVERLAPPEDWINDOW,
    CW_USEDEFAULT, CW_USEDEFAULT, rect.right - rect.left, rect.bottom - rect.top,
    NULL, NULL, instance, a);
  pthread_mutex_lock(&a->lock); a->error = hwnd ? 0 : EIO; a->opened = 1; pthread_cond_signal(&a->ready); pthread_mutex_unlock(&a->lock);
  if (!hwnd) return NULL;
  DragAcceptFiles(hwnd, TRUE); ShowWindow(hwnd, SW_SHOW); UpdateWindow(hwnd);
  bpw_push(a, 5, 0, 0, 0, 0);
  MSG msg;
  while (GetMessageW(&msg, NULL, 0, 0) > 0) { TranslateMessage(&msg); DispatchMessageW(&msg); }
  return NULL;
}

static Term bpw_open(Env e, Term* f) {
  BpWin* a = io_mem(calloc(1, sizeof *a));
  pthread_mutex_init(&a->lock, NULL); pthread_cond_init(&a->ready, NULL);
  u64 n = 0; char* title = io_cstr(e, f[0], &n); a->title = bpw_wide(title); free(title);
  a->width = f[1]; a->height = f[2];
  if (pipe(a->notify) != 0) { free(a->title); free(a); return io_fail(e, errno, NULL); }
  for (u32 i = 0; i < 2; i++) { fcntl(a->notify[i], F_SETFL, O_NONBLOCK); fcntl(a->notify[i], F_SETFD, FD_CLOEXEC); }
  pthread_t thread;
  int error = pthread_create(&thread, NULL, bpw_thread, a);
  if (error) { close(a->notify[0]); close(a->notify[1]); free(a->title); free(a); return io_fail(e, error, NULL); }
  pthread_detach(thread);
  pthread_mutex_lock(&a->lock);
  while (!a->opened) pthread_cond_wait(&a->ready, &a->lock);
  error = a->error; pthread_mutex_unlock(&a->lock);
  return error ? io_fail(e, error, NULL) : io_done(e, io_hand((intptr_t)a));
}

static Term bpw_words_more(Env e, IoWork* w) {
  BpWin* a = (BpWin*)w->hand;
  pthread_mutex_lock(&a->lock);
  if (!a->count) { pthread_mutex_unlock(&a->lock); return io_wait_on(w, a->notify[0], POLLIN, 0, bpw_words_more); }
  char bytes[128]; while (read(a->notify[0], bytes, sizeof bytes) > 0) {}
  Term xs = term_pak(CID_NIL, 0);
  for (u32 i = a->count; i > 0; i--) xs = io_node(e, CID_CON, (Term)a->events[i - 1], xs);
  a->count = 0; pthread_mutex_unlock(&a->lock);
  return io_tup(e, io_hand(w->hand), xs);
}

static void bpw_fill(u64* H, BpWin* a, Term image, u32 x, u32 y, u32 size) {
  if (x >= a->pw || y >= a->ph) return;
  if (term_tag(image) == TAG_CTR && size > 1) {
    u64 at = term_rfc(image) ? H[term_loc(image)] >> 24 : term_loc(image);
    u32 half = size / 2;
    bpw_fill(H, a, H[at], x, y, half); bpw_fill(H, a, H[at+1], x+half, y, half);
    bpw_fill(H, a, H[at+2], x, y+half, half); bpw_fill(H, a, H[at+3], x+half, y+half, half);
  } else {
    while (term_tag(image) == TAG_CTR) { u64 at = term_rfc(image) ? H[term_loc(image)] >> 24 : term_loc(image); image = H[at]; }
    u32 color = (u32)term_loc(image) & 0xffffff;
    for (u32 row = y; row < a->ph && row < y+size; row++)
      for (u32 col = x; col < a->pw && col < x+size; col++) a->pixels[(size_t)row*a->pw+col] = color;
  }
}

static void bpw_show(Env e, BpWin* a, Term image) {
  pthread_mutex_lock(&a->lock);
  if (a->pw != a->width || a->ph != a->height || !a->pixels) {
    a->pw = a->width; a->ph = a->height;
    a->pixels = io_mem(realloc(a->pixels, (size_t)a->pw*a->ph*sizeof(u32)));
  }
  u32 size = 1; while (size < a->pw || size < a->ph) size *= 2;
  bpw_fill(e.mem, a, image, 0, 0, size);
  pthread_mutex_unlock(&a->lock);
  PostMessageW(a->hwnd, BPW_PAINT, 0, 0);
}

#ifdef CID(Win.open)
Term win_open_run(Env e, Term* f, IoWork* w) {
  return bpw_open(e, f);
}
static void __attribute__((constructor)) win_open_use(void) { io_eff(CID(Win.open), win_open_run); }
#endif

#ifdef CID(Win.words)
Term win_words_run(Env e, Term* f, IoWork* w) {
  w->hand = (intptr_t)io_hand_v(f[0]); return bpw_words_more(e, w);
}
static void __attribute__((constructor)) win_words_use(void) { io_eff(CID(Win.words), win_words_run); }
#endif

#ifdef CID(Win.present)
Term win_present_run(Env e, Term* f, IoWork* w) {
  BpWin* a = (BpWin*)io_hand_v(f[0]); bpw_show(e, a, f[1]); return io_tup(e, f[0], f[1]);
}
static void __attribute__((constructor)) win_present_use(void) { io_eff(CID(Win.present), win_present_run); }
#endif

#ifdef CID(Win.show)
Term win_show_run(Env e, Term* f, IoWork* w) {
  BpWin* a = (BpWin*)io_hand_v(f[0]); bpw_show(e, a, f[1]); return io_tup(e, f[0], io_tup(e, f[1], f[2]));
}
static void __attribute__((constructor)) win_show_use(void) { io_eff(CID(Win.show), win_show_run); }
#endif

#ifdef CID(Win.copy)
Term win_copy_run(Env e, Term* f, IoWork* w) {
  BpWin* a = (BpWin*)io_hand_v(f[0]); u64 n = 0; char* text = io_cstr(e, f[1], &n); wchar_t* wide = bpw_wide(text); free(text); pthread_mutex_lock(&a->lock); free(a->copy); a->copy = wide; pthread_mutex_unlock(&a->lock); PostMessageW(a->hwnd, BPW_COPY, 0, 0); return f[0];
}
static void __attribute__((constructor)) win_copy_use(void) { io_eff(CID(Win.copy), win_copy_run); }
#endif

#ifdef CID(Win.paste)
Term win_paste_run(Env e, Term* f, IoWork* w) {
  BpWin* a = (BpWin*)io_hand_v(f[0]); PostMessageW(a->hwnd, BPW_PASTE, 0, 0); return f[0];
}
static void __attribute__((constructor)) win_paste_use(void) { io_eff(CID(Win.paste), win_paste_run); }
#endif

#ifdef CID(Win.pasted)
Term win_pasted_run(Env e, Term* f, IoWork* w) {
  BpWin* a = (BpWin*)io_hand_v(f[0]); pthread_mutex_lock(&a->lock); Term text = io_str(e, a->paste ? a->paste : "", a->paste ? strlen(a->paste) : 0); pthread_mutex_unlock(&a->lock); return io_tup(e, f[0], text);
}
static void __attribute__((constructor)) win_pasted_use(void) { io_eff(CID(Win.pasted), win_pasted_run); }
#endif

#ifdef CID(Win.dropped)
Term win_dropped_run(Env e, Term* f, IoWork* w) {
  BpWin* a = (BpWin*)io_hand_v(f[0]); pthread_mutex_lock(&a->lock); Term text = io_str(e, a->drop ? a->drop : "", a->drop ? strlen(a->drop) : 0); pthread_mutex_unlock(&a->lock); return io_tup(e, f[0], text);
}
static void __attribute__((constructor)) win_dropped_use(void) { io_eff(CID(Win.dropped), win_dropped_run); }
#endif

#ifdef CID(Win.size)
Term win_size_run(Env e, Term* f, IoWork* w) {
  BpWin* a = (BpWin*)io_hand_v(f[0]); pthread_mutex_lock(&a->lock); u32 width = a->width, height = a->height; pthread_mutex_unlock(&a->lock); return io_tup(e, f[0], io_tup(e, (Term)width, (Term)height));
}
static void __attribute__((constructor)) win_size_use(void) { io_eff(CID(Win.size), win_size_run); }
#endif

#ifdef CID(Win.twin)
Term win_twin_run(Env e, Term* f, IoWork* w) {
  return io_tup(e, f[0], f[0]);
}
static void __attribute__((constructor)) win_twin_use(void) { io_eff(CID(Win.twin), win_twin_run); }
#endif

#ifdef CID(Win.title)
Term win_title_run(Env e, Term* f, IoWork* w) {
  BpWin* a = (BpWin*)io_hand_v(f[0]); u64 n = 0; char* text = io_cstr(e, f[1], &n); wchar_t* wide = bpw_wide(text); free(text); pthread_mutex_lock(&a->lock); free(a->title); a->title = wide; pthread_mutex_unlock(&a->lock); PostMessageW(a->hwnd, BPW_TITLE, 0, 0); return f[0];
}
static void __attribute__((constructor)) win_title_use(void) { io_eff(CID(Win.title), win_title_run); }
#endif

#ifdef CID(Win.ime)
Term win_ime_run(Env e, Term* f, IoWork* w) {
  return io_tup(e, f[0], (Term)0);
}
static void __attribute__((constructor)) win_ime_use(void) { io_eff(CID(Win.ime), win_ime_run); }
#endif

#ifdef CID(Win.ime_reset)
Term win_ime_reset_run(Env e, Term* f, IoWork* w) {
  return f[0];
}
static void __attribute__((constructor)) win_ime_reset_use(void) { io_eff(CID(Win.ime_reset), win_ime_reset_run); }
#endif

#ifdef CID(Win.ime_spot)
Term win_ime_spot_run(Env e, Term* f, IoWork* w) {
  return f[0];
}
static void __attribute__((constructor)) win_ime_spot_use(void) { io_eff(CID(Win.ime_spot), win_ime_spot_run); }
#endif

#ifdef CID(Win.ime_inject)
Term win_ime_inject_run(Env e, Term* f, IoWork* w) {
  return f[0];
}
static void __attribute__((constructor)) win_ime_inject_use(void) { io_eff(CID(Win.ime_inject), win_ime_inject_run); }
#endif

#ifdef CID(Win.payload)
Term win_payload_run(Env e, Term* f, IoWork* w) {
  return io_tup(e, f[0], io_tup(e, io_str(e,"",0), io_str(e,"",0)));
}
static void __attribute__((constructor)) win_payload_use(void) { io_eff(CID(Win.payload), win_payload_run); }
#endif

#ifdef CID(Win.pasted_image)
Term win_pasted_image_run(Env e, Term* f, IoWork* w) {
  return io_tup(e, f[0], io_tup(e, io_str(e,"",0), io_tup(e, (Term)0, io_str(e,"",0))));
}
static void __attribute__((constructor)) win_pasted_image_use(void) { io_eff(CID(Win.pasted_image), win_pasted_image_run); }
#endif

#else

// No window on this platform yet (macOS gets its own AppKit twin).

#ifdef CID(Win.open)
Term win_open_run(Env e, Term* f, IoWork* w) {
  return io_fail(e, ENOTSUP, "the native window is Linux-only for now; run `backplane serve`");
}
static void __attribute__((constructor)) win_open_use(void) {
  io_eff(CID(Win.open), win_open_run);
}
#endif

#endif

#ifdef CID(Win.xid)
Term win_xid_run(Env e, Term* f, IoWork* w) {
#if defined(__linux__)
  AppWin* a = (AppWin*)io_hand_v(f[0]);
  return io_tup(e, f[0], (Term)(u32)a->win);
#else
  return io_tup(e, f[0], (Term)0);
#endif
}
static void __attribute__((constructor)) win_xid_use(void) {
  io_eff(CID(Win.xid), win_xid_run);
}
#endif
