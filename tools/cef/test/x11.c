// Private-Xvfb parent and XTest driver; EOF destroys only our parent window.
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef int (*key_f)(Display*, unsigned, Bool, unsigned long);
typedef int (*btn_f)(Display*, unsigned, Bool, unsigned long);
typedef int (*mot_f)(Display*, int, int, int, unsigned long);
static void info(Display* d, Window w) {
  XWindowAttributes a; Window root, par, *kids = NULL; unsigned n;
  XGetWindowAttributes(d, w, &a); XQueryTree(d, w, &root, &par, &kids, &n);
  if (kids) XFree(kids);
  printf("{\"xid\":%lu,\"parent\":%lu,\"x\":%d,\"y\":%d,\"w\":%d,\"h\":%d,\"inputOnly\":%s,\"mapped\":%s}", w, par, a.x, a.y, a.width, a.height, a.class == InputOnly ? "true" : "false", a.map_state == IsViewable ? "true" : "false");
}
int main(void) {
  const char* display = getenv("DISPLAY");
  // private displays only: :86..:89 or :96..:99, never the person's
  if (!display || strlen(display) != 3 || display[0] != ':' || (display[1] != '8' && display[1] != '9') || display[2] < '6' || display[2] > '9') return 2;
  Display* d = XOpenDisplay(NULL); if (!d) return 1;
  Window root = DefaultRootWindow(d);
  Window parent = XCreateSimpleWindow(d, root, 0, 0, 1400, 900, 0, 0, 0xffffff);
  XStoreName(d, parent, "CEF acceptance parent"); XMapWindow(d, parent); XSync(d, False);
  void* xt = dlopen("libXtst.so.6", RTLD_NOW); if (!xt) return 3;
  key_f key = (key_f)dlsym(xt, "XTestFakeKeyEvent");
  btn_f btn = (btn_f)dlsym(xt, "XTestFakeButtonEvent");
  mot_f mot = (mot_f)dlsym(xt, "XTestFakeMotionEvent");
  printf("{\"parent\":%lu}\n", parent); fflush(stdout);
  char line[256], cmd[64], word[64], mod[64];
  while (fgets(line, sizeof line, stdin)) {
    unsigned long win; int x, y, button;
    if (sscanf(line, "%63s", cmd) != 1) continue;
    if (!strcmp(cmd, "info") && sscanf(line, "%*s %lu", &win) == 1) info(d, win);
    else if (!strcmp(cmd, "children")) {
      Window rr, pp, *kids = NULL; unsigned n; win = parent; sscanf(line, "%*s %lu", &win); XQueryTree(d, win, &rr, &pp, &kids, &n);
      printf("{\"children\":[");
      for (unsigned i = 0; i < n; ++i) { if (i) printf(","); info(d, kids[i]); }
      printf("]}"); if (kids) XFree(kids);
    } else if (!strcmp(cmd, "tops")) {
      Window rr, pp, *kids = NULL; unsigned n; XQueryTree(d, root, &rr, &pp, &kids, &n);
      int chromium = 0, listed = 0;
      printf("{\"windows\":[");
      for (unsigned i = 0; i < n; ++i) {
        char* name = NULL; XFetchName(d, kids[i], &name);
        if (name && (strstr(name, "Chromium") || strstr(name, "Chrome"))) {
          XWindowAttributes a; XGetWindowAttributes(d, kids[i], &a);
          // CEF's unmapped X selection owner is infrastructure, not a page UI.
          if (strcmp(name, "Chromium clipboard") || a.map_state != IsUnmapped) chromium++;
          if (listed++) printf(",");
          printf("{\"title\":\"");
          for (const unsigned char* p = (unsigned char*)name; *p; p++) {
            if (*p == '"' || *p == '\\') putchar('\\');
            if (*p >= 32) putchar(*p);
          }
          printf("\",\"xid\":%lu}", kids[i]);
        }
        if (name) XFree(name);
      }
      if (kids) XFree(kids); printf("],\"chromium\":%d}", chromium);
    } else if (!strcmp(cmd, "click") && sscanf(line, "%*s %d %d %d", &x, &y, &button) == 3) {
      // A WM focuses only the embedding top level; the page must take focus itself.
      XSetInputFocus(d, parent, RevertToParent, CurrentTime);
      mot(d, -1, x, y, 0); btn(d, button, True, 0); btn(d, button, False, 0);
      XSync(d, False); printf("{}");
    } else if (!strcmp(cmd, "key") && sscanf(line, "%*s %63s", word) == 1) {
      unsigned kc = XKeysymToKeycode(d, XStringToKeysym(word));
      key(d, kc, True, 0); key(d, kc, False, 0); XSync(d, False); printf("{}");
    } else if (!strcmp(cmd, "chord") && sscanf(line, "%*s %63s %63s", mod, word) == 2) {
      unsigned mk = XKeysymToKeycode(d, XStringToKeysym(mod));
      unsigned kc = XKeysymToKeycode(d, XStringToKeysym(word));
      if (!mk || !kc) { printf("{\"error\":true}"); }
      else { key(d, mk, True, 0); key(d, kc, True, 0); key(d, kc, False, 0); key(d, mk, False, 0); XSync(d, False); printf("{}"); }
    } else if (!strcmp(cmd, "clipboard")) {
      printf("{\"owner\":%lu}", XGetSelectionOwner(d, XInternAtom(d, "CLIPBOARD", False)));
    } else if (!strcmp(cmd, "clearclip")) {
      XSetSelectionOwner(d, XInternAtom(d, "CLIPBOARD", False), None, CurrentTime);
      XSync(d, False); printf("{}");
    } else if (!strcmp(cmd, "close") && sscanf(line, "%*s %lu", &win) == 1) {
      XEvent e = {0}; e.xclient.type = ClientMessage; e.xclient.window = win;
      e.xclient.message_type = XInternAtom(d, "WM_PROTOCOLS", False); e.xclient.format = 32;
      e.xclient.data.l[0] = XInternAtom(d, "WM_DELETE_WINDOW", False);
      XSendEvent(d, win, False, NoEventMask, &e); XSync(d, False); printf("{}");
    } else if (!strcmp(cmd, "focus")) {
      Window w; int revert; XGetInputFocus(d, &w, &revert); printf("{\"focus\":%lu}", w);
    } else printf("{\"error\":true}");
    putchar('\n'); fflush(stdout);
  }
  XDestroyWindow(d, parent); XCloseDisplay(d); return 0;
}
