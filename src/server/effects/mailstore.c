// Low-level regular-file primitives. Protocol and repair policy live in Bend.
#include <sys/stat.h>
#include <stdint.h>
#include <fcntl.h>
#include <unistd.h>
#include <errno.h>
#include <string.h>

#ifdef CID(MailFile.open)
static void mailfile_open_call(IoWork* w) {
  char *slash = strrchr(w->data, '/');
  int code = 0, fd = -1;
  if (!slash || !slash[1] || io_nul(w->data, w->size)) code = EINVAL;
  else {
    *slash = 0;
    int dir = open(w->data, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (dir < 0) code = errno;
    else {
      int flags = w->word ? O_RDWR | O_CREAT : O_RDONLY;
      fd = openat(dir, slash + 1, flags | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC, 0600);
      if (fd < 0) code = errno;
      else {
        struct stat st;
        if (fstat(fd, &st)) code = errno;
        else if (!S_ISREG(st.st_mode)) code = EINVAL;
        // A writable body file must not be a hard link to a file elsewhere.
        else if (w->word && st.st_nlink != 1) code = EINVAL;
      }
      if (close(dir) && !code) code = errno;
    }
  }
  if (code && fd >= 0) close(fd);
  w->made = fd; w->code = code;
}
static Term mailfile_open_pack(Env e, IoWork* w) {
  free(w->data);
  return w->code ? io_fail(e, w->code, NULL) : io_done(e, io_hand(w->made));
}
Term mailfile_open_run(Env e, Term* f, IoWork* w) {
  w->data = io_cstr(e, f[0], &w->size);
  w->word = (u32)f[1];
  return io_work(w, mailfile_open_call, mailfile_open_pack);
}
static void __attribute__((constructor)) mailfile_open_use(void) {
  io_eff(CID(MailFile.open), mailfile_open_run);
}
#endif

#ifdef CID(MailFile.finish)
static void mailfile_finish_call(IoWork* w) {
  int fd = (int)w->hand, code = 0;
  if ((uint64_t)w->word + w->size > UINT32_MAX) code = EFBIG;
  if (!code && ftruncate(fd, (off_t)w->word)) code = errno;
  if (!code && lseek(fd, 0, SEEK_END) < 0) code = errno;
  uint64_t at = 0;
  while (!code && at < w->size) {
    ssize_t n = write(fd, w->data + at, w->size - at);
    if (n < 0) { if (errno != EINTR) code = errno; }
    else if (n == 0) code = EIO;
    else at += (uint64_t)n;
  }
  if (close(fd) && !code) code = errno;
  w->code = code;
}
static Term mailfile_finish_pack(Env e, IoWork* w) {
  free(w->data);
  return w->code ? io_fail(e, w->code, NULL) : io_done(e, term_pak(CID(Unit), 0));
}
Term mailfile_finish_run(Env e, Term* f, IoWork* w) {
  w->hand = io_hand_v(f[0]); w->word = (u32)f[1];
  w->data = io_cstr(e, f[2], &w->size);
  return io_work(w, mailfile_finish_call, mailfile_finish_pack);
}
static void __attribute__((constructor)) mailfile_finish_use(void) {
  io_eff(CID(MailFile.finish), mailfile_finish_run);
}
#endif

#if defined(CID(MailFile.text)) || defined(CID(MailFile.patch)) || defined(CID(MailFile.texts))
// Safe path-based positional IO. No body/index interpretation lives here.
static int mi_open_regular(const char *path, int writeable) {
  char *copy = strdup(path);
  if (!copy) { errno = ENOMEM; return -1; }
  char *slash = strrchr(copy, '/');
  int dir = -1, fd = -1, code = 0;
  if (!slash || slash == copy || !slash[1]) code = EINVAL;
  else {
    *slash = 0;
    dir = open(copy, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (dir < 0) code = errno;
    else {
      fd = openat(dir, slash + 1, (writeable ? O_RDWR | O_CREAT : O_RDONLY) | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC, 0600);
      if (fd < 0) code = errno;
      else {
        struct stat st;
        if (fstat(fd, &st)) code = errno;
        else if (!S_ISREG(st.st_mode) || (writeable && st.st_nlink != 1)) code = EINVAL;
        else if (st.st_size < 0 || (uint64_t)st.st_size > UINT32_MAX) code = EFBIG;
      }
    }
  }
  if (dir >= 0 && close(dir) && !code) code = errno;
  free(copy);
  if (code) { if (fd >= 0) close(fd); errno = code; return -1; }
  return fd;
}
#endif

#ifdef CID(MailFile.text)
static void mi_text_call(IoWork *w) {
  int fd = -1, code = 0;
  if (io_nul(w->data, w->size) || (uint64_t)w->word + w->hand > UINT32_MAX) code = EINVAL;
  else { fd = mi_open_regular(w->data, 0); if (fd < 0) code = errno; }
  free(w->data); w->data = NULL; w->size = 0;
  if (!code) {
    w->data = malloc((size_t)w->hand + 1);
    if (!w->data) code = ENOMEM;
  }
  while (!code && w->size < w->hand) {
    ssize_t n = pread(fd, w->data + w->size, w->hand - w->size, (off_t)w->word + w->size);
    if (n < 0) { if (errno != EINTR) code = errno; }
    else if (!n) code = EIO;
    else w->size += (uint64_t)n;
  }
  if (fd >= 0 && close(fd) && !code) code = errno;
  w->code = code;
}
static Term mi_text_pack(Env e, IoWork *w) {
  Term r = w->code ? io_fail(e, w->code, NULL) : io_done(e, io_str(e, w->data, w->size));
  free(w->data); return r;
}
Term mi_text_run(Env e, Term *f, IoWork *w) {
  w->data = io_cstr(e, f[0], &w->size); w->word = (u32)f[1]; w->hand = (u32)f[2];
  return io_work(w, mi_text_call, mi_text_pack);
}
static void __attribute__((constructor)) mi_text_use(void) { io_eff(CID(MailFile.text), mi_text_run); }
#endif

#ifdef CID(MailFile.patch)
static void mi_patch_call(IoWork *w) {
  // io_cstr supplied path and text as two adjacent buffers, separated by NUL.
  char *text = w->data + w->made;
  uint64_t size = w->size - w->made;
  int fd = -1, code = 0;
  if ((uint64_t)w->word + size > UINT32_MAX) code = EFBIG;
  else { fd = mi_open_regular(w->data, 1); if (fd < 0) code = errno; }
  uint64_t at = 0;
  while (!code && at < size) {
    ssize_t n = pwrite(fd, text + at, size - at, (off_t)w->word + at);
    if (n < 0) { if (errno != EINTR) code = errno; }
    else if (!n) code = EIO;
    else at += (uint64_t)n;
  }
  if (!code && w->hand && ftruncate(fd, (off_t)w->word + size)) code = errno;
  if (fd >= 0 && close(fd) && !code) code = errno;
  w->code = code;
}
static Term mi_patch_pack(Env e, IoWork *w) {
  free(w->data);
  return w->code ? io_fail(e, w->code, NULL) : io_done(e, term_pak(CID(Unit), 0));
}
Term mi_patch_run(Env e, Term *f, IoWork *w) {
  uint64_t pn = 0, tn = 0;
  char *path = io_cstr(e, f[0], &pn), *text = io_cstr(e, f[2], &tn);
  if (io_nul(path, pn)) { free(path); free(text); return io_fail(e, EINVAL, NULL); }
  w->made = pn + 1; w->size = pn + 1 + tn;
  w->data = malloc(w->size + 1);
  if (!w->data) { free(path); free(text); return io_fail(e, ENOMEM, NULL); }
  memcpy(w->data, path, pn + 1); memcpy(w->data + pn + 1, text, tn);
  free(path); free(text); w->word = (u32)f[1]; w->hand = (u32)f[3];
  return io_work(w, mi_patch_call, mi_patch_pack);
}
static void __attribute__((constructor)) mi_patch_use(void) { io_eff(CID(MailFile.patch), mi_patch_run); }
#endif

#ifdef CID(MailFile.texts)
// Vectored positional text reads. Ranges arrive as offset,length pairs;
// this primitive knows no journal format, IDs, cursors or index policy.
typedef struct { u32 at, size; char *text; } MiRange;
typedef struct { char *path; u64 n, cap; MiRange *ranges; } MiTexts;
static void mi_texts_call(IoWork *w) {
  MiTexts *v = (MiTexts *)w->data;
  int fd = -1, code = w->code;
  if (!code) { fd = mi_open_regular(v->path, 0); if (fd < 0) code = errno; }
  for (u64 i = 0; !code && i < v->n; i++) {
    MiRange *r = &v->ranges[i];
    if ((u64)r->at + r->size > UINT32_MAX) { code = EFBIG; break; }
    r->text = malloc((size_t)r->size + 1);
    if (!r->text) { code = ENOMEM; break; }
    u64 at = 0;
    while (!code && at < r->size) {
      ssize_t n = pread(fd, r->text + at, r->size - at, (off_t)r->at + at);
      if (n < 0) { if (errno != EINTR) code = errno; }
      else if (!n) code = EIO;
      else at += (u64)n;
    }
  }
  if (fd >= 0 && close(fd) && !code) code = errno;
  w->code = code;
}
static Term mi_texts_pack(Env e, IoWork *w) {
  MiTexts *v = (MiTexts *)w->data;
  Term xs = term_pak(CID_NIL, 0);
  if (!w->code) for (u64 i = v->n; i > 0; i--)
    xs = io_node(e, CID_CON, io_str(e, v->ranges[i-1].text, v->ranges[i-1].size), xs);
  for (u64 i = 0; i < v->n; i++) free(v->ranges[i].text);
  free(v->ranges); free(v->path); free(v);
  return w->code ? io_fail(e, w->code, NULL) : io_done(e, xs);
}
Term mi_texts_run(Env e, Term *f, IoWork *w) {
  MiTexts *v = io_mem(calloc(1, sizeof(MiTexts)));
  u64 pn = 0;
  v->path = io_cstr(e, f[0], &pn);
  w->code = io_nul(v->path, pn) ? EINVAL : 0;
  Term xs = f[1]; u32 at = 0; int pair = 0;
  while (term_aux(xs) == CID_CON) {
    Term fb[2];
    spare_free(e, cls_fit(2), ctr_take(e, xs, 2, fb));
    if (!pair) at = (u32)fb[0];
    else {
      if (v->n == v->cap) {
        v->cap = v->cap ? v->cap * 2 : 64;
        v->ranges = io_mem(realloc(v->ranges, v->cap * sizeof(MiRange)));
      }
      v->ranges[v->n++] = (MiRange){at, (u32)fb[0], NULL};
    }
    pair = !pair; xs = fb[1];
  }
  if (pair) w->code = EINVAL;
  w->data = (char *)v;
  return io_work(w, mi_texts_call, mi_texts_pack);
}
static void __attribute__((constructor)) mi_texts_use(void) { io_eff(CID(MailFile.texts), mi_texts_run); }
#endif
