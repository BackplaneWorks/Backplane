#!/usr/bin/env python3
"""Prepare pinned Bend C for the Windows Cygwin target.

Windows x64 passes Env (two machine words) indirectly. Bend's musttail
segments reuse that argument after the caller's frame is gone. Use SysV
for the internal functions marked PRESERVE and their matching function
pointers: Env then travels in registers. Foreign/Win32 functions retain
Windows' normal ABI. This changes no Bend terms or laws.
"""
from pathlib import Path
import re
import sys

source, output = map(Path, sys.argv[1:])
text = source.read_text()
pattern = r"(?m)^#define PRESERVE\(A\)[^\n]*$"
if len(re.findall(pattern, text)) != 2:
    sys.exit("windows-runtime: pinned Bend PRESERVE declarations changed")
text = re.sub(pattern, "#define PRESERVE(A) __attribute__((sysv_abi))", text)
# Base's process effect includes this Linux-only header outside its syscall
# feature guards. Cygwin has the portable POSIX fallback used below them.
text = text.replace("#include <sys/syscall.h>", "#if defined(__linux__)\n#include <sys/syscall.h>\n#endif")
# Base only selects getrandom on non-Apple hosts. Use Windows' system RNG
# without modifying the pinned compiler installation.
random_call = "  io_sys_end(w, getrandom(&w->word, sizeof(w->word), 0));"
if random_call in text:
    if text.count(random_call) != 1:
        sys.exit("windows-runtime: pinned Bend random effect changed")
    text = text.replace(random_call, """#ifdef __CYGWIN__
  w->code = BCryptGenRandom(NULL, (PUCHAR)&w->word, sizeof(w->word),
                           BCRYPT_USE_SYSTEM_PREFERRED_RNG) == 0 ? 0 : EIO;
#else
""" + random_call + "\n#endif")
    text = text.replace("#include <sys/random.h>\n#endif",
                        "#include <sys/random.h>\n#elif defined(__CYGWIN__)\n#include <windows.h>\n#include <bcrypt.h>\n#endif")
output.write_text(text)
