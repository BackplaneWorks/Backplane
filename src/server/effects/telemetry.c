// Compile-time defaults are empty. No telemetry endpoint ships by default.
#if __has_include("bp_telemetry.h")
#include "bp_telemetry.h"
#endif
#ifndef BP_TELEMETRY_URL
#define BP_TELEMETRY_URL ""
#endif
#ifndef BP_TELEMETRY_KID
#define BP_TELEMETRY_KID ""
#endif
#ifndef BP_TELEMETRY_KEY
#define BP_TELEMETRY_KEY ""
#endif
#ifndef BP_TELEMETRY_CHANNEL
#define BP_TELEMETRY_CHANNEL ""
#endif
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <sys/utsname.h>
#ifdef CID(Tf.baked)
static Term tf_baked_run(Env e, Term *f, IoWork *w) {
  u64 n=0; char *name=io_cstr(e,f[0],&n); const char *v="";
  if (!strcmp(name,"BACKPLANE_TELEMETRY_URL")) v=BP_TELEMETRY_URL;
  else if (!strcmp(name,"BACKPLANE_TELEMETRY_KID")) v=BP_TELEMETRY_KID;
  else if (!strcmp(name,"BACKPLANE_TELEMETRY_KEY")) v=BP_TELEMETRY_KEY;
  else if (!strcmp(name,"BACKPLANE_TELEMETRY_CHANNEL")) v=BP_TELEMETRY_CHANNEL;
  free(name); return io_str(e,v,strlen(v));
}
static void __attribute__((constructor)) tf_baked_use(void) { io_eff(CID(Tf.baked),tf_baked_run); }
#endif
#ifdef CID(Tf.platform)
static Term tf_platform_run(Env e, Term *f, IoWork *w) {
  struct utsname u; const char *os="linux", *arch="x64"; char b[32];
#ifdef __CYGWIN__
  os="windows";
#endif
  if (!uname(&u)) { if (!strcmp(u.sysname,"Darwin")) os="darwin";
    if (!strcmp(u.machine,"arm64") || !strcmp(u.machine,"aarch64")) arch="arm64"; }
  snprintf(b,sizeof(b),"%s-%s",os,arch); return io_str(e,b,strlen(b));
}
static void __attribute__((constructor)) tf_platform_use(void) { io_eff(CID(Tf.platform),tf_platform_run); }
#endif
#ifdef CID(Tf.pow)
// A bounded native search in the disposable --telemetry-pow child. Its
// answer is independently verified by pure core/sha256.bend in the job.
static uint32_t tp_rotr(uint32_t x, unsigned n) { return (x>>n)|(x<<(32-n)); }
static const uint32_t tp_k[64]={
  0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
  0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
  0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
  0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
  0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
  0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
  0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
  0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2};
static uint32_t tp_hash(const char *s, size_t n) {
  uint8_t block[128]={0}; uint32_t h[8]={0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19};
  if (n>119) return UINT32_MAX;
  memcpy(block,s,n); block[n]=0x80; size_t len=n<56?64:128; uint64_t bits=n*8;
  for(int i=0;i<8;i++) block[len-1-i]=(uint8_t)(bits>>(i*8));
  for(size_t off=0;off<len;off+=64) {
    uint32_t v[64]; for(int i=0;i<16;i++) { const uint8_t *p=block+off+i*4; v[i]=((uint32_t)p[0]<<24)|((uint32_t)p[1]<<16)|((uint32_t)p[2]<<8)|p[3]; }
    for(int i=16;i<64;i++) { uint32_t a=v[i-15],b=v[i-2]; v[i]=v[i-16]+(tp_rotr(a,7)^tp_rotr(a,18)^(a>>3))+v[i-7]+(tp_rotr(b,17)^tp_rotr(b,19)^(b>>10)); }
    uint32_t a=h[0],b=h[1],c=h[2],d=h[3],e=h[4],f=h[5],g=h[6],hh=h[7];
    for(int i=0;i<64;i++) { uint32_t t=hh+(tp_rotr(e,6)^tp_rotr(e,11)^tp_rotr(e,25))+((e&f)^((~e)&g))+tp_k[i]+v[i]; uint32_t u=(tp_rotr(a,2)^tp_rotr(a,13)^tp_rotr(a,22))+((a&b)^(a&c)^(b&c)); hh=g;g=f;f=e;e=d+t;d=c;c=b;b=a;a=t+u; }
    h[0]+=a;h[1]+=b;h[2]+=c;h[3]+=d;h[4]+=e;h[5]+=f;h[6]+=g;h[7]+=hh;
  } return h[0];
}
static Term tf_pow_run(Env e, Term *f, IoWork *w) {
  u64 n=0; char *id=io_cstr(e,f[0],&n); char msg[96],pow[17]="";
  if (n==36) for(uint32_t i=0;i<(1u<<26);i++) {
    snprintf(pow,sizeof(pow),"%x",i); int m=snprintf(msg,sizeof(msg),"bp-pow1:%s:%s",id,pow);
    if ((tp_hash(msg,(size_t)m)>>14)==0) { free(id);return io_str(e,pow,strlen(pow)); }
  }
  free(id);return io_str(e,"",0);
}
static void __attribute__((constructor)) tf_pow_use(void) { io_eff(CID(Tf.pow),tf_pow_run); }
#endif
