#!/usr/bin/env python3
"""Compile the actual FS.revision C body against small effect-value stubs.

The Darwin field-name branch is compiled against Linux aliases as a source
check; this is not Darwin/Windows runtime or package acceptance.
"""
import os, subprocess, tempfile
from pathlib import Path
source=Path('src/server/effects/host.c').read_text()
body=source[source.index('Term fs_revision_run('):source.index('static void __attribute__((constructor)) fs_revision_use')]
preamble=r'''
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <errno.h>
typedef uintptr_t Term;
typedef void* Env;
typedef void IoWork;
typedef uint64_t u64;
typedef uint32_t u32;
static char* io_cstr(Env e, Term v, u64* n) { *n=strlen((char*)v);return strdup((char*)v); }
static int io_nul(char* p,u64 n) { return memchr(p,0,n)!=NULL; }
static Term io_str(Env e,const char* p,u64 n) { return (Term)strndup(p,n); }
static Term io_tup(Env e,Term a,Term b) { Term* r=malloc(2*sizeof(Term));r[0]=a;r[1]=b;return (Term)r; }
static Term io_done(Env e,Term v) { return v; }
static Term io_fail(Env e,int code,void* text) { return 0; }
'''
main=r'''
int main(int argc,char** argv) {
  Term f[1]={(Term)argv[1]}, r=fs_revision_run(NULL,f,NULL);
  if (!r) return 2;
  Term* v=(Term*)r;printf("%u\n%s\n",(u32)v[0],(char*)v[1]);return 0;
}
'''
with tempfile.TemporaryDirectory(prefix='bp-revision-c-') as tmp:
    d=Path(tmp);p=d/'s.kicad_sch'
    for mode,flags in [('posix',[]),('darwin-field-branch',['-D__APPLE__','-Dst_mtimespec=st_mtim','-Dst_ctimespec=st_ctim'])]:
        code=d/f'{mode}.c';binary=d/mode;code.write_text(preamble+body+main)
        subprocess.run(['cc','-O2','-Werror',*flags,str(code),'-o',str(binary)],check=True)
        def rev():
            r=subprocess.run([str(binary),str(p)],text=True,capture_output=True)
            if r.returncode:return None
            size,revision=r.stdout.splitlines();return int(size),revision
        p.write_text('REV000');a=rev();p.read_bytes();assert rev()==a
        p.write_text('REV001');b=rev();assert b[0]==a[0] and b[1]!=a[1]
        st=p.stat();replacement=d/'atomic';replacement.write_text('REV002');os.utime(replacement,ns=(st.st_atime_ns,st.st_mtime_ns));replacement.replace(p)
        c=rev();assert c[0]==b[0] and c[1]!=b[1]
        p.write_text('REV003');os.utime(p,ns=(st.st_atime_ns,st.st_mtime_ns));e=rev();assert e!=c
        p.unlink();assert rev() is None
        print(f'ok actual C FS.revision {mode}: unchanged read, rapid edit, atomic replace, preserved mtime, missing file')
