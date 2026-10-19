// xpoke: send synthetic input to the Backplane window (tests only).
//   xpoke click X Y        left click
//   xpoke rclick X Y       right click
//   xpoke type TEXT        type characters (ASCII)
//   xpoke key NAME         press a named key (Return, BackSpace, Escape...)
//   xpoke ctrl KEY         ctrl+key (an upper-case letter adds shift)
//   xpoke wheel X Y up|down
//   xpoke drag X Y X2 Y2   press at X Y, move to X2 Y2, release there
//   xpoke move X Y         move the pointer there (no button: a hover)
//   xpoke sweep X Y X2 Y2 N MS  N moves from X Y to X2 Y2, MS ms apart
//   xpoke size W H         resize the window
//   xpoke hold KEY MS      hold a key for MS ms as a keyboard does: a press, then
//                          repeats (presses with no release between), a release
//   xpoke holdx KEY MS     the same for a server without detectable repeat: each
//                          repeat is a release and a press with the same time
//   xpoke down KEY / up KEY  one press / one release
//   xpoke xdown KEY / xup KEY / xhold KEY MS   the same through the XTest extension (the
//                          server's own key state and auto-repeat, which XQueryKeymap sees)
//   xpoke blur / focus     the window loses / gets the input focus
// Build: cc -I<x11 include> test/tools/xpoke.c -o build/xpoke -lX11
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <stdio.h>
#include <dlfcn.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static Window find(Display* d, Window w, const char* name) {
  char* n = NULL;
  if (XFetchName(d, w, &n) && n) {
    int hit = strcmp(n, name) == 0;
    XFree(n);
    if (hit) return w;
  }
  Window root, parent, *kids = NULL;
  unsigned nk = 0;
  if (!XQueryTree(d, w, &root, &parent, &kids, &nk)) return 0;
  Window got = 0;
  for (unsigned i = 0; i < nk && !got; i++) got = find(d, kids[i], name);
  if (kids) XFree(kids);
  return got;
}

static void key(Display* d, Window w, KeySym ks, unsigned state) {
  XKeyEvent e = { 0 };
  e.display = d; e.window = w; e.root = DefaultRootWindow(d);
  e.same_screen = True; e.state = state;
  e.keycode = XKeysymToKeycode(d, ks);
  e.type = KeyPress; XSendEvent(d, w, True, KeyPressMask, (XEvent*)&e);
  e.type = KeyRelease; XSendEvent(d, w, True, KeyReleaseMask, (XEvent*)&e);
}

static void kev(Display* d, Window w, KeySym ks, int type, Time t) {
  XKeyEvent e = { 0 };
  e.display = d; e.window = w; e.root = DefaultRootWindow(d);
  e.same_screen = True; e.keycode = XKeysymToKeycode(d, ks);
  e.type = type; e.time = t;
  XSendEvent(d, w, True, type == KeyPress ? KeyPressMask : KeyReleaseMask, (XEvent*)&e);
  XFlush(d);
}

static void hold(Display* d, Window w, KeySym ks, int ms, int synth) {
  Time t = 1000;
  kev(d, w, ks, KeyPress, t);
  for (int at = 0; at + 30 <= ms; at += 30) {
    usleep(30000);
    t += 30;
    if (synth) kev(d, w, ks, KeyRelease, t);
    kev(d, w, ks, KeyPress, t);
  }
  usleep(30000);
  kev(d, w, ks, KeyRelease, t + 30);
}

// real key events from the server's XTest extension (libXtst, loaded here: no header needed)
static void xtest(Display* d, KeySym ks, int press) {
  static int (*fake)(Display*, unsigned, int, unsigned long);
  if (!fake) {
    void* h = dlopen("libXtst.so.6", RTLD_NOW);
    fake = h ? (int (*)(Display*, unsigned, int, unsigned long))dlsym(h, "XTestFakeKeyEvent") : NULL;
    if (!fake) { fprintf(stderr, "no libXtst\n"); exit(1); }
  }
  fake(d, XKeysymToKeycode(d, ks), press, 0);
  XFlush(d);
}

static void button(Display* d, Window w, int x, int y, unsigned b) {
  XButtonEvent e = { 0 };
  e.display = d; e.window = w; e.root = DefaultRootWindow(d);
  e.same_screen = True; e.x = x; e.y = y; e.button = b;
  e.type = ButtonPress; XSendEvent(d, w, True, ButtonPressMask, (XEvent*)&e);
  e.type = ButtonRelease; XSendEvent(d, w, True, ButtonReleaseMask, (XEvent*)&e);
}

static void drag(Display* d, Window w, int x, int y, int x2, int y2) {
  XButtonEvent e = { 0 };
  e.display = d; e.window = w; e.root = DefaultRootWindow(d);
  e.same_screen = True; e.x = x; e.y = y; e.button = 1;
  e.type = ButtonPress; XSendEvent(d, w, True, ButtonPressMask, (XEvent*)&e);
  XFlush(d);
  XMotionEvent m = { 0 };
  m.display = d; m.window = w; m.root = DefaultRootWindow(d);
  m.same_screen = True; m.x = x2; m.y = y2; m.state = Button1Mask;
  m.type = MotionNotify; XSendEvent(d, w, True, PointerMotionMask | Button1MotionMask, (XEvent*)&m);
  XFlush(d);
  e.x = x2; e.y = y2; e.state = Button1Mask;
  e.type = ButtonRelease; XSendEvent(d, w, True, ButtonReleaseMask, (XEvent*)&e);
}

static void move(Display* d, Window w, int x, int y) {
  XMotionEvent m = { 0 };
  m.display = d; m.window = w; m.root = DefaultRootWindow(d);
  m.same_screen = True; m.x = x; m.y = y;
  m.type = MotionNotify; XSendEvent(d, w, True, PointerMotionMask, (XEvent*)&m);
  XFlush(d);
}

int main(int argc, char** argv) {
  Display* d = XOpenDisplay(NULL);
  if (!d || argc < 2) return 1;
  // XPOKE_WIN=<id> picks one window when several Backplanes share a display
  const char* pick = getenv("XPOKE_WIN");
  Window w = pick && *pick ? (Window)strtoul(pick, NULL, 0) : find(d, DefaultRootWindow(d), "Backplane");
  if (!w) { fprintf(stderr, "no Backplane window\n"); return 1; }
  if (!strcmp(argv[1], "click") && argc == 4) {
    button(d, w, atoi(argv[2]), atoi(argv[3]), 1);
  } else if (!strcmp(argv[1], "rclick") && argc == 4) {
    button(d, w, atoi(argv[2]), atoi(argv[3]), 3);
  } else if (!strcmp(argv[1], "move") && argc == 4) {
    move(d, w, atoi(argv[2]), atoi(argv[3]));
  } else if (!strcmp(argv[1], "sweep") && argc == 8) {
    int x = atoi(argv[2]), y = atoi(argv[3]), x2 = atoi(argv[4]), y2 = atoi(argv[5]), n = atoi(argv[6]), ms = atoi(argv[7]);
    for (int i = 0; i <= n; i++) {
      move(d, w, x + (x2 - x) * i / (n ? n : 1), y + (y2 - y) * i / (n ? n : 1));
      usleep(ms * 1000);
    }
  } else if (!strcmp(argv[1], "drag") && argc == 6) {
    drag(d, w, atoi(argv[2]), atoi(argv[3]), atoi(argv[4]), atoi(argv[5]));
  } else if (!strcmp(argv[1], "wheel") && argc == 5) {
    button(d, w, atoi(argv[2]), atoi(argv[3]), strcmp(argv[4], "up") ? 5 : 4);
  } else if (!strcmp(argv[1], "type") && argc == 3) {
    for (const char* c = argv[2]; *c; c++) {
      char name[2] = { *c, 0 };
      KeySym ks = *c == ' ' ? XK_space : *c == '.' ? XK_period : *c == ',' ? XK_comma
        : *c == '?' ? XK_question : *c == '!' ? XK_exclam : *c == '/' ? XK_slash
        : *c == '-' ? XK_minus : XStringToKeysym(name);
      unsigned shift = (*c >= 'A' && *c <= 'Z') || *c == '?' || *c == '!' ? ShiftMask : 0;
      key(d, w, ks, shift);
    }
  } else if ((!strcmp(argv[1], "hold") || !strcmp(argv[1], "holdx")) && argc == 4) {
    hold(d, w, XStringToKeysym(argv[2]), atoi(argv[3]), argv[1][4] == 'x');
  } else if (!strcmp(argv[1], "down") && argc == 3) {
    kev(d, w, XStringToKeysym(argv[2]), KeyPress, 1000);
  } else if (!strcmp(argv[1], "up") && argc == 3) {
    kev(d, w, XStringToKeysym(argv[2]), KeyRelease, 1000);
  } else if (!strcmp(argv[1], "xdown") && argc == 3) {
    xtest(d, XStringToKeysym(argv[2]), 1);
  } else if (!strcmp(argv[1], "xup") && argc == 3) {
    xtest(d, XStringToKeysym(argv[2]), 0);
  } else if (!strcmp(argv[1], "xhold") && argc == 4) {
    xtest(d, XStringToKeysym(argv[2]), 1);
    usleep((useconds_t)atoi(argv[3]) * 1000);
    xtest(d, XStringToKeysym(argv[2]), 0);
  } else if (!strcmp(argv[1], "blur") && argc == 2) {
    XSetInputFocus(d, DefaultRootWindow(d), RevertToNone, CurrentTime);
  } else if (!strcmp(argv[1], "focus") && argc == 2) {
    XSetInputFocus(d, w, RevertToParent, CurrentTime);
  } else if (!strcmp(argv[1], "size") && argc == 4) {
    XResizeWindow(d, w, atoi(argv[2]), atoi(argv[3]));
  } else if (!strcmp(argv[1], "key") && argc == 3) {
    key(d, w, XStringToKeysym(argv[2]), 0);
  } else if (!strcmp(argv[1], "ctrl") && argc == 3) {
    // xpoke ctrl j / xpoke ctrl S (upper case adds shift)
    const char* k = argv[2];
    unsigned st = ControlMask | ((k[0] >= 'A' && k[0] <= 'Z' && !k[1]) ? ShiftMask : 0);
    key(d, w, XStringToKeysym(k), st);
  } else {
    return 2;
  }
  XFlush(d);
  XCloseDisplay(d);
  return 0;
}
