// Test-only X11 surface with independently specified RGB pixels.
#include <X11/Xlib.h>
#include <stdio.h>
#include <string.h>
int main(void) {
  Display *d = XOpenDisplay(NULL);
  if (!d) return 1;
  int screen = DefaultScreen(d);
  Visual *v = DefaultVisual(d,screen);
  if(v->red_mask!=0xff0000 || v->green_mask!=0xff00 || v->blue_mask!=0xff) return 1;
  Window w=XCreateSimpleWindow(d,RootWindow(d,screen),0,0,96,64,0,0,0);
  GC gc=XCreateGC(d,w,0,NULL);
  XMapWindow(d,w);XSync(d,False);
  setvbuf(stdout,NULL,_IOLBF,0);
  printf("window %lu\n",w);
  char command[32];
  while(fgets(command,sizeof(command),stdin)) {
    if(!strcmp(command,"quit\n")) break;
    for(int y=0;y<64;y++) for(int x=0;x<96;x++) {
      unsigned r=0,g=0,b=0;
      if(!strcmp(command,"red\n")) r=255;
      else if(!strcmp(command,"green\n")) g=255;
      else if(!strcmp(command,"pattern\n")) {r=(17*x+3*y)&255;g=(5*x+19*y)&255;b=(11*x+7*y)&255;}
      else return 2;
      XSetForeground(d,gc,(r<<16)|(g<<8)|b);XDrawPoint(d,w,gc,x,y);
    }
    XSync(d,False);
    printf("ready %s",command);
  }
  XFreeGC(d,gc);XDestroyWindow(d,w);XCloseDisplay(d);return 0;
}
