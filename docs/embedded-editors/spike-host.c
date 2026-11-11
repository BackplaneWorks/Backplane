// host: a fake "Backplane" top-level; reparent a client window into it.
// usage: host create  -> prints parent xid, stays running
//        host embed PARENT CHILD X Y W H
//        host focus CHILD
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
int main(int c, char**v){
  Display*d=XOpenDisplay(NULL); if(!d) return 1;
  if(!strcmp(v[1],"create")){
    Window w=XCreateSimpleWindow(d,DefaultRootWindow(d),0,0,1400,900,0,0,0x303040);
    XStoreName(d,w,"FakeBackplane"); XMapWindow(d,w); XSync(d,0);
    printf("%lu\n",w); fflush(stdout);
    XSelectInput(d,w,ExposureMask); for(;;){XEvent e; XNextEvent(d,&e);} }
  if(!strcmp(v[1],"embed")){
    Window p=strtoul(v[2],0,0), ch=strtoul(v[3],0,0);
    XUnmapWindow(d,ch); XSync(d,0); usleep(300000);
    XReparentWindow(d,ch,p,atoi(v[4]),atoi(v[5]));
    XResizeWindow(d,ch,atoi(v[6]),atoi(v[7]));
    XMapWindow(d,ch); XSync(d,0); return 0; }
  if(!strcmp(v[1],"focus")){ XSetInputFocus(d,strtoul(v[2],0,0),RevertToParent,CurrentTime); XSync(d,0); return 0;}
  if(!strcmp(v[1],"resize")){ XMoveResizeWindow(d,strtoul(v[2],0,0),atoi(v[3]),atoi(v[4]),atoi(v[5]),atoi(v[6])); XSync(d,0); return 0;}
  return 2; }
