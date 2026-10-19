// Private-Xvfb parent and XTest driver; EOF destroys only our parent window.
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/Xatom.h>
#include <unistd.h>
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
  Window owned_parent=parent;
  unsigned long pid = getpid();
  XChangeProperty(d,parent,XInternAtom(d,"_NET_WM_PID",False),XA_CARDINAL,32,PropModeReplace,(unsigned char*)&pid,1);
  // A real desktop WM marks its client window. Xvfb has no WM; without
  // WM_STATE GTK searches into Chromium's private subwindows for a target.
  Atom wm_state = XInternAtom(d,"WM_STATE",False);
  unsigned long wm_values[2] = {NormalState,None};
  XChangeProperty(d,parent,wm_state,wm_state,32,PropModeReplace,(unsigned char*)wm_values,2);
  XStoreName(d, parent, "CEF acceptance parent"); XMapWindow(d, parent); XSync(d, False);
  void* xt = dlopen("libXtst.so.6", RTLD_NOW); if (!xt) return 3;
  key_f key = (key_f)dlsym(xt, "XTestFakeKeyEvent");
  btn_f btn = (btn_f)dlsym(xt, "XTestFakeButtonEvent");
  mot_f mot = (mot_f)dlsym(xt, "XTestFakeMotionEvent");
  printf("{\"parent\":%lu}\n", parent); fflush(stdout);
  char line[256], cmd[64], word[64], mod[64];
  while (fgets(line, sizeof line, stdin)) {
    unsigned long win; int x, y, button, width, height;
    if (sscanf(line, "%63s", cmd) != 1) continue;
    if (!strcmp(cmd, "info") && sscanf(line, "%*s %lu", &win) == 1) info(d, win);
    else if (!strcmp(cmd,"target") && sscanf(line,"%*s %lu",&win)==1) { parent=win;printf("{}"); }
    else if (!strcmp(cmd, "children")) {
      Window rr, pp, *kids = NULL; unsigned n; win = parent; sscanf(line, "%*s %lu", &win); XQueryTree(d, win, &rr, &pp, &kids, &n);
      printf("{\"children\":[");
      for (unsigned i = 0; i < n; ++i) { if (i) printf(","); info(d, kids[i]); }
      printf("]}"); if (kids) XFree(kids);
    } else if (!strcmp(cmd, "proxy")) {
      win=parent; sscanf(line,"%*s %lu",&win);
      Atom type; int format; unsigned long n, after; unsigned char* data=NULL;
      XGetWindowProperty(d,win,XInternAtom(d,"XdndProxy",False),0,1,False,XA_WINDOW,&type,&format,&n,&after,&data);
      printf("{\"proxy\":%lu}",data&&format==32&&n?*(unsigned long*)data:0);
      if(data)XFree(data);
    } else if (!strcmp(cmd, "native-target")) {
      Atom type;int format;unsigned long n,left;unsigned char* data=NULL;
      XGetWindowProperty(d,parent,XInternAtom(d,"_BACKPLANE_CEF_DND_TARGET",False),0,5,False,XA_CARDINAL,&type,&format,&n,&left,&data);
      unsigned long target=data&&format==32&&n==5?*(unsigned long*)data:0;
      char name[80];snprintf(name,sizeof name,"_BACKPLANE_CEF_DND_%lu",parent);
      printf("{\"target\":%lu,\"owner\":%lu}",target,XGetSelectionOwner(d,XInternAtom(d,name,False)));if(data)XFree(data);
    } else if (!strcmp(cmd,"stale-target")) {
      unsigned long values[]={0xdeadbeef,0,0,1400,900};
      XChangeProperty(d,parent,XInternAtom(d,"_BACKPLANE_CEF_DND_TARGET",False),XA_CARDINAL,32,PropModeReplace,(unsigned char*)values,5);XSync(d,False);printf("{}");
    } else if (!strcmp(cmd, "place") && sscanf(line,"%*s %lu %d %d %d %d",&win,&x,&y,&width,&height)==5) {
      XMoveResizeWindow(d,win,x,y,width,height);XSync(d,False);printf("{}");
    } else if (!strcmp(cmd,"drag")) {
      int x2,y2;
      if(sscanf(line,"%*s %d %d %d %d",&x,&y,&x2,&y2)!=4)printf("{\"error\":true}");
      else {
        mot(d,-1,x,y,0);btn(d,1,True,0);XSync(d,False);usleep(100000);
        for(int step=1;step<=30;step++){mot(d,-1,x+(x2-x)*step/30,y+(y2-y)*step/30,0);XFlush(d);usleep(30000);}
        usleep(200000);btn(d,1,False,0);XSync(d,False);printf("{}");
      }
    } else if (!strcmp(cmd, "messages")) {
      XEvent event;int n=0;printf("{\"messages\":[");
      while(XCheckTypedEvent(d,ClientMessage,&event)){
        char* atom=XGetAtomName(d,event.xclient.message_type);
        if(n++)printf(",");printf("{\"name\":\"%s\",\"window\":%lu}",atom,event.xclient.window);
        if(atom)XFree(atom);
      }
      printf("]}");
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
    } else if (!strcmp(cmd,"focuswin") && sscanf(line,"%*s %lu",&win)==1) {
      XSetInputFocus(d,win,RevertToParent,CurrentTime);XSync(d,False);printf("{}");
    } else if (!strcmp(cmd, "click") && sscanf(line, "%*s %d %d %d", &x, &y, &button) == 3) {
      // A WM focuses only the embedding top level; the page must take focus itself.
      XSetInputFocus(d, parent, RevertToParent, CurrentTime);
      mot(d, -1, x, y, 0); btn(d, button, True, 0); btn(d, button, False, 0);
      XSync(d, False); printf("{}");
    } else if (!strcmp(cmd, "key") && sscanf(line, "%*s %63s", word) == 1) {
      KeySym sym = XStringToKeysym(word);
      unsigned kc = XKeysymToKeycode(d, sym);
      if (!kc && sym != NoSymbol) {
        // A private Xvfb's US map lacks Japanese mode keys. Map one unused
        // test key so the actual IBus engine receives a real X key event.
        int first,last;XDisplayKeycodes(d,&first,&last);kc=last;
        XChangeKeyboardMapping(d,kc,1,&sym,1);XSync(d,False);
      }
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
  XDestroyWindow(d, owned_parent); XCloseDisplay(d); return 0;
}
