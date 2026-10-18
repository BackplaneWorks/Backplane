// xlat: input-to-pixels latency of a window, measured on the X screen.
//
//   xlat WIN X Y MODE N        MODE: key | wheel | until:RRGGBB
//
// key/wheel: N times, read the screen pixel at (X, Y) of WIN, inject one
// XTest key press (or wheel notch) there, and poll the pixel every 0.2 ms
// until it changes; print each latency in ms, then a JSON summary.
// until:RRGGBB: wait (up to 30 s) until the pixel has that colour, print the
// wall-clock ms when it did.
//
// Only for a private Xvfb display: XTest input goes to whatever has focus.
// libXtst is dlopen'd (no headers needed).
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

typedef int (*key_f)(Display*, unsigned, Bool, unsigned long);
typedef int (*btn_f)(Display*, unsigned, Bool, unsigned long);
typedef int (*mot_f)(Display*, int, int, int, unsigned long);

static double now_ms(void) {
  struct timespec ts;
  clock_gettime(CLOCK_REALTIME, &ts);
  return ts.tv_sec * 1000.0 + ts.tv_nsec / 1e6;
}

static unsigned long pixel(Display* d, Window root, int x, int y) {
  XImage* im = XGetImage(d, root, x, y, 1, 1, AllPlanes, ZPixmap);
  if (!im) return ~0ul;
  unsigned long p = XGetPixel(im, 0, 0) & 0xffffff;
  XDestroyImage(im);
  return p;
}

static int cmp(const void* a, const void* b) {
  double x = *(const double*)a, y = *(const double*)b;
  return x < y ? -1 : x > y;
}

int main(int argc, char** argv) {
  if (argc < 6) { fprintf(stderr, "usage: xlat WIN X Y key|wheel|until:RRGGBB N\n"); return 2; }
  Display* d = XOpenDisplay(NULL);
  if (!d) { fprintf(stderr, "xlat: no display\n"); return 1; }
  Window win = strtoul(argv[1], NULL, 0), root = DefaultRootWindow(d), kid;
  int x = atoi(argv[2]), y = atoi(argv[3]), n = atoi(argv[5]), ax, ay;
  const char* mode = argv[4];
  if (!XTranslateCoordinates(d, win, root, x, y, &ax, &ay, &kid)) { fprintf(stderr, "xlat: bad window\n"); return 1; }

  if (!strncmp(mode, "until:", 6)) {
    unsigned long want = strtoul(mode + 6, NULL, 16);
    double t0 = now_ms();
    while (now_ms() - t0 < 30000) {
      if (pixel(d, root, ax, ay) == want) { printf("{\"t\":%.1f}\n", now_ms()); return 0; }
      usleep(500);
    }
    printf("{\"timeout\":true}\n");
    return 1;
  }

  void* xt = dlopen("libXtst.so.6", RTLD_NOW);
  if (!xt) { fprintf(stderr, "xlat: no libXtst.so.6\n"); return 1; }
  key_f key = (key_f)dlsym(xt, "XTestFakeKeyEvent");
  btn_f btn = (btn_f)dlsym(xt, "XTestFakeButtonEvent");
  mot_f mot = (mot_f)dlsym(xt, "XTestFakeMotionEvent");
  // pointer onto the probe point; one click there gives the page focus
  mot(d, -1, ax, ay, 0);
  btn(d, 1, True, 0); btn(d, 1, False, 0);
  XSync(d, False);
  usleep(300000);
  unsigned kc = XKeysymToKeycode(d, XStringToKeysym("a"));
  int wheel = !strcmp(mode, "wheel");
  double* lat = calloc(n, sizeof(double));
  int got = 0;
  for (int i = 0; i < n; i++) {
    unsigned long before = pixel(d, root, ax, ay);
    double t0 = now_ms();
    if (wheel) { btn(d, 5, True, 0); btn(d, 5, False, 0); }
    else { key(d, kc, True, 0); key(d, kc, False, 0); }
    XFlush(d);
    double dt = -1;
    while (now_ms() - t0 < 2000) {
      if (pixel(d, root, ax, ay) != before) { dt = now_ms() - t0; break; }
      usleep(200);
    }
    if (dt >= 0) lat[got++] = dt;
    fprintf(stderr, "%s %d: %.2f ms\n", mode, i, dt);
    usleep(150000);  // let the page settle (and smooth scroll finish)
  }
  qsort(lat, got, sizeof(double), cmp);
  double sum = 0;
  for (int i = 0; i < got; i++) sum += lat[i];
  printf("{\"mode\":\"%s\",\"n\":%d,\"ok\":%d,\"mean\":%.2f,\"p50\":%.2f,\"p95\":%.2f,\"max\":%.2f}\n", mode, n, got,
         got ? sum / got : -1, got ? lat[got / 2] : -1, got ? lat[(got * 95) / 100 < got ? (got * 95) / 100 : got - 1] : -1,
         got ? lat[got - 1] : -1);
  return got == n ? 0 : 1;
}
