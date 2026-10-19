// Independent X-server framebuffer readback for the fixed-host gate.
// This observes pixels, not compositor presentation or physical scanout.
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <dlfcn.h>
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static uint64_t micros(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC,&t);
  return (uint64_t)t.tv_sec*1000000u+t.tv_nsec/1000u;
}
static long number(const char *s) {
  char *end; errno=0; long value=strtol(s,&end,0);
  if(errno || !*s || *end) {fprintf(stderr,"invalid numeric argument\n");exit(2);}
  return value;
}
int main(int argc,char **argv) {
  if(argc!=9) {fprintf(stderr,"usage: present_observer WINDOW X Y WIDTH HEIGHT DURATION_MS SAMPLE_HZ REGION\n");return 2;}
  long window=number(argv[1]),x=number(argv[2]),y=number(argv[3]),w=number(argv[4]),h=number(argv[5]);
  long duration=number(argv[6]),hz=number(argv[7]);
  const char *region=argv[8];
  if(window<=0 || x<0 || y<0 || w<1 || h<1 || w>16384 || h>16384 || w*h>16000000
      || duration<100 || duration>60000 || hz<30 || hz>240 || strspn(region,"abcdefghijklmnopqrstuvwxyz0123456789-_")!=strlen(region) || !*region) return 2;
  void *lib=dlopen("libcrypto.so.3",RTLD_NOW|RTLD_LOCAL);
  if(!lib) lib=dlopen("libcrypto.so",RTLD_NOW|RTLD_LOCAL);
  if(!lib) {fprintf(stderr,"SHA256 implementation unavailable\n");return 1;}
  void *(*ctx_new)(void)=dlsym(lib,"EVP_MD_CTX_new");
  void (*ctx_free)(void*)=dlsym(lib,"EVP_MD_CTX_free");
  const void *(*sha256)(void)=dlsym(lib,"EVP_sha256");
  int (*init)(void*,const void*,void*)=dlsym(lib,"EVP_DigestInit_ex");
  int (*update)(void*,const void*,size_t)=dlsym(lib,"EVP_DigestUpdate");
  int (*finish)(void*,unsigned char*,unsigned*)=dlsym(lib,"EVP_DigestFinal_ex");
  if(!ctx_new || !ctx_free || !sha256 || !init || !update || !finish) return 1;
  void *ctx=ctx_new();
  Display *display=XOpenDisplay(NULL);
  if(!ctx || !display) return 1;
  XWindowAttributes attrs;
  if(!XGetWindowAttributes(display,(Window)window,&attrs) || x+w>attrs.width || y+h>attrs.height) return 1;
  unsigned char *rgb=malloc((size_t)w*h*3);
  if(!rgb) return 1;
  const uint64_t start=micros(),end=start+(uint64_t)duration*1000;
  uint64_t sequence=0;
  setvbuf(stdout,NULL,_IOLBF,0);
  printf("{\"kind\":\"observer\",\"boundary\":\"xserver-readback\",\"region\":\"%s\",\"x\":%ld,\"y\":%ld,\"width\":%ld,\"height\":%ld,\"start_us\":%llu,\"end_us\":%llu,\"sample_hz\":%ld,\"physical_scanout_verified\":false}\n",region,x,y,w,h,(unsigned long long)start,(unsigned long long)end,hz);
  while(micros()<end) {
    XImage *image=XGetImage(display,(Window)window,(int)x,(int)y,(unsigned)w,(unsigned)h,AllPlanes,ZPixmap);
    uint64_t at=micros();
    if(!image || image->bits_per_pixel!=32 || image->byte_order!=LSBFirst
        || image->red_mask!=0xff0000 || image->green_mask!=0xff00 || image->blue_mask!=0xff) {
      fprintf(stderr,"unsupported or missing framebuffer image\n");return 1;
    }
    for(long yy=0;yy<h;yy++) {
      const uint32_t *row=(const uint32_t*)(image->data+(size_t)yy*image->bytes_per_line);
      for(long xx=0;xx<w;xx++) {
        uint32_t pixel=row[xx];size_t offset=((size_t)yy*w+xx)*3;
        rgb[offset]=(pixel>>16)&255;rgb[offset+1]=(pixel>>8)&255;rgb[offset+2]=pixel&255;
      }
    }
    XDestroyImage(image);
    unsigned char digest[32];unsigned size=0;
    if(init(ctx,sha256(),NULL)!=1 || update(ctx,rgb,(size_t)w*h*3)!=1 || finish(ctx,digest,&size)!=1 || size!=32) return 1;
    printf("{\"kind\":\"pixels\",\"sample\":%llu,\"at_us\":%llu,\"region\":\"%s\",\"pixel_sha256\":\"",(unsigned long long)++sequence,(unsigned long long)at,region);
    for(unsigned i=0;i<32;i++) printf("%02x",digest[i]);
    printf("\"}\n");
    uint64_t due=start+sequence*1000000u/(uint64_t)hz;
    struct timespec deadline={(time_t)(due/1000000u),(long)(due%1000000u)*1000};
    while(clock_nanosleep(CLOCK_MONOTONIC,TIMER_ABSTIME,&deadline,NULL)==EINTR) {}
  }
  free(rgb);ctx_free(ctx);XCloseDisplay(display);dlclose(lib);
  return 0;
}
