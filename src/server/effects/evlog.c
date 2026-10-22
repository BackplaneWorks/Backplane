// EvLog.read_from (host.bend): the event log from a byte offset, as
// [tail's fix, offset past the last whole line, lines...], the lines
// decoded as core/utf8.bend's Utf8.decode does (U+FFFD for each byte that
// starts no character, the code point of an overlong form, U+FFFD for a
// surrogate or past U+10FFFF): what core/store.bend's Store.read.from says.

#ifdef CID(EvLog.read_from)

#include <fcntl.h>
#include <inttypes.h>
#include <stdio.h>
#include <sys/stat.h>
#include <unistd.h>

// Utf8.decode over p[0..n) into out (at most n code points); the count
static u64 evlog_decode(const uint8_t* p, u64 n, uint32_t* out) {
  u64 k = 0, owe = 0;
  uint32_t cp = 0;
  for (u64 i = 0; i < n; i += 1) {
    uint32_t x = p[i];
    if (owe > 0) {
      if ((x & 192) == 128) {
        cp = (cp << 6) | (x & 63);
        if (--owe == 0) {
          out[k++] = (cp > 1114111 || (cp >= 55296 && cp <= 57343)) ? 65533 : cp;
        }
        continue;
      }
      out[k++] = 65533;
      owe = 0;
    }
    if (x < 128) {
      out[k++] = x;
    } else if ((x & 224) == 192) {
      cp = x & 31; owe = 1;
    } else if ((x & 240) == 224) {
      cp = x & 15; owe = 2;
    } else if ((x & 248) == 240) {
      cp = x & 7; owe = 3;
    } else {
      out[k++] = 65533;
    }
  }
  if (owe > 0) {
    out[k++] = 65533;
  }
  return k;
}

// code points to a String, built as io_str builds one
static Term evlog_str(Env e, const uint32_t* cs, u64 n) {
  Term s = term_pak(CID(SNil), 0);
  Term* hole = &s;
  for (u64 i = 0; i < n; i += 1) {
    u64  l = heap_alloc(e, 1);
    Term t = term_ctr(CID(SCon), l);
    e.mem[l] = cs[i];
    *hole = hole == &s ? t : io_seal(e, t, CID(SCon));
    hole = &e.mem[l + 1];
  }
  *hole = io_seal(e, term_pak(CID(SNil), 0), CID(SCon));
  return s;
}

// the bytes from off to the end (NULL when the file cannot be read)
static uint8_t* evlog_slurp(const char* path, uint64_t off, u64* size) {
  int fd = open(path, O_RDONLY | O_CLOEXEC);
  if (fd < 0) {
    return NULL;
  }
  struct stat st;
  if (fstat(fd, &st) < 0) {
    close(fd);
    return NULL;
  }
  u64 all = st.st_size > 0 ? (u64)st.st_size : 0;
  u64 cap = all > off ? all - off : 0, got = 0;
  uint8_t* b = malloc(cap + 1);
  while (b != NULL) {
    if (got == cap) {
      uint8_t* nb = realloc(b, cap * 2 + 4096);
      if (nb == NULL) { free(b); b = NULL; break; }
      b = nb; cap = cap * 2 + 4096;
    }
    ssize_t r = pread(fd, b + got, cap - got, (off_t)(off + got));
    if (r < 0 && errno == EINTR) continue;
    if (r <= 0) break;
    got += (u64)r;
  }
  close(fd);
  *size = got;
  return b;
}

static Term evlog_num(Env e, uint64_t v) {
  char buf[32];
  int n = snprintf(buf, sizeof buf, "%" PRIu64, v);
  return io_str(e, buf, (u64)n);
}

Term evlog_read_from_run(Env e, Term* f, IoWork* w) {
  u64 n = 0, m = 0, size = 0;
  char* path = io_cstr(e, f[0], &n);
  char* offs = io_cstr(e, f[1], &m);
  uint64_t off = 0;
  for (u64 i = 0; i < m && offs[i] >= '0' && offs[i] <= '9'; i += 1) {
    off = off * 10 + (uint64_t)(offs[i] - '0');
  }
  free(offs);
  uint8_t* b = io_nul(path, n) ? NULL : evlog_slurp(path, off, &size);
  free(path);
  Term xs = term_pak(CID(Nil), 0);
  if (b == NULL || size == 0) {
    free(b);
    xs = io_node(e, CID(Con), evlog_num(e, off), xs);
    return io_node(e, CID(Con), term_pak(CID(SNil), 0), xs);
  }
  int torn = b[size - 1] != 10;
  // the whole lines, last first, so the list is built from its end
  u64 end = size;
  while (end > 0 && b[end - 1] != 10) end -= 1;
  u64 whole = end;
  uint32_t* cs = malloc(sizeof(uint32_t) * (end + 1));
  while (end > 0) {
    u64 stop = end - 1, start = stop;
    while (start > 0 && b[start - 1] != 10) start -= 1;
    u64 k = evlog_decode(b + start, stop - start, cs);
    xs = io_node(e, CID(Con), evlog_str(e, cs, k), xs);
    end = start;
  }
  free(cs);
  free(b);
  xs = io_node(e, CID(Con), evlog_num(e, off + whole), xs);
  return io_node(e, CID(Con), torn ? io_str(e, "\n", 1) : term_pak(CID(SNil), 0), xs);
}

static void __attribute__((constructor)) evlog_read_from_use(void) {
  io_eff(CID(EvLog.read_from), evlog_read_from_run);
}

#endif
