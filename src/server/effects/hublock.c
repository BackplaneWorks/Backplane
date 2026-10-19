// One hub per home: an exclusive flock on <home>/hub.lock, held for the
// life of the process (core/onehub.bend). The kernel lets it go when the
// process dies, however it dies, so the file left behind never blocks the
// next hub. The descriptor is close-on-exec, so no child (agents, plot
// children, shells) holds it; before an exec (a restart in place, host.c's
// Sys.exec) it survives the exec instead and its number goes in
// BACKPLANE_HUB_LOCK_FD, and the new image takes that same descriptor back
// (the lock is never let go across a restart).
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/file.h>
#include <sys/stat.h>
#include <unistd.h>

static int bp_hub_fd = -1;

// Before an exec (keep 1) the lock's descriptor survives it and is named
// in the environment; after an exec that failed (keep 0) it is
// close-on-exec again. host.c calls it through a weak reference.
void bp_hublock_exec(int keep) {
  if (bp_hub_fd < 0) return;
  if (keep) {
    char n[16];
    snprintf(n, sizeof n, "%d", bp_hub_fd);
    fcntl(bp_hub_fd, F_SETFD, 0);
    setenv("BACKPLANE_HUB_LOCK_FD", n, 1);
  } else {
    fcntl(bp_hub_fd, F_SETFD, FD_CLOEXEC);
    unsetenv("BACKPLANE_HUB_LOCK_FD");
  }
}

// An image an exec handed the lock to: its descriptor is close-on-exec
// again before main runs, so no child the image starts before it takes the
// lock back (probes, fetches) can hold it past the hub's death.
static void __attribute__((constructor)) bp_hub_inherit(void) {
  const char* env = getenv("BACKPLANE_HUB_LOCK_FD");
  int fd = env && *env ? atoi(env) : -1;
  if (fd > 2) fcntl(fd, F_SETFD, FD_CLOEXEC);
}

// fd is open on the file at path
static int bp_hub_same(int fd, const char* path) {
  struct stat a, b;
  if (fd < 0 || fstat(fd, &a) != 0 || stat(path, &b) != 0) return 0;
  return a.st_dev == b.st_dev && a.st_ino == b.st_ino;
}

static void bp_hub_note(int fd, const char* line) {
  size_t n = strlen(line);
  if (ftruncate(fd, 0) == 0) {
#ifdef __CYGWIN__
    // Cygwin's positional-I/O handle is invalid after exec (EBADF), while
    // the inherited descriptor still owns its flock and supports seek/write.
    // Only startup writes this descriptor; readers open their own descriptor.
    if (lseek(fd, 0, SEEK_SET) != 0) return;
    ssize_t w = write(fd, line, n);
    if (w == (ssize_t)n) w = write(fd, "\n", 1);
#else
    ssize_t w = pwrite(fd, line, n, 0);
    if (w == (ssize_t)n) w = pwrite(fd, "\n", 1, (off_t)n);
#endif
    (void)w;
  }
}

// what the holder wrote ("<pid> <port>"); a holder that has just taken the
// lock writes it at once, so an empty file is read again for a moment
static void bp_hub_read(const char* path, char* out, size_t size) {
  out[0] = 0;
  for (int i = 0; i < 20; i++) {
    int fd = open(path, O_RDONLY | O_CLOEXEC);
    if (fd >= 0) {
      ssize_t n = read(fd, out, size - 1);
      close(fd);
      if (n > 0) {
        out[n] = 0;
        char* nl = strchr(out, '\n');
        if (nl) *nl = 0;
        if (out[0]) return;
      }
    }
    usleep(50000);
  }
  out[0] = 0;
}

#ifdef CID(HubLock.take)
// "ok" (this process holds the lock, and the file says "<pid> <port>
// flock"), "held <pid> <port> flock" (another process holds it) or
// "fail <why>". ("flock": a hub that wrote it holds the lock while it
// lives, so the file alone never means a hub runs: server.bend's Lock.*)
Term hub_lock_take_run(Env e, Term* f, IoWork* w) {
  u64 len = 0;
  char* path = io_cstr(e, f[0], &len);
  char* port = io_cstr(e, f[1], &len);
  char line[128];
  snprintf(line, sizeof line, "%ld %s flock", (long)getpid(), port);
  free(port);
  char out[512];
  int fd = -1;
  if (bp_hub_same(bp_hub_fd, path)) {
    fd = bp_hub_fd;
  } else {
    const char* env = getenv("BACKPLANE_HUB_LOCK_FD");
    if (env && *env) {
      int got = atoi(env);
      unsetenv("BACKPLANE_HUB_LOCK_FD");
      // the descriptor the image before the exec held, on this same file
      if (got > 2 && bp_hub_same(got, path) && flock(got, LOCK_EX | LOCK_NB) == 0) {
        fcntl(got, F_SETFD, FD_CLOEXEC);
        fd = got;
      }
    }
  }
  if (fd < 0) {
    int opened = open(path, O_RDWR | O_CREAT | O_CLOEXEC, 0644);
    if (opened < 0) {
      snprintf(out, sizeof out, "fail %s: %s", path, strerror(errno));
      goto done;
    }
    fd = fcntl(opened, F_DUPFD_CLOEXEC, 3);
    close(opened);
    if (fd < 0) {
      snprintf(out, sizeof out, "fail %s: %s", path, strerror(errno));
      goto done;
    }
    if (flock(fd, LOCK_EX | LOCK_NB) != 0) {
      int err = errno;
      close(fd);
      if (err == EWOULDBLOCK) {
        char held[256];
        bp_hub_read(path, held, sizeof held);
        snprintf(out, sizeof out, "held %s", held);
      } else {
        snprintf(out, sizeof out, "fail %s: %s", path, strerror(err));
      }
      goto done;
    }
  }
  if (bp_hub_fd >= 0 && bp_hub_fd != fd) close(bp_hub_fd);
  bp_hub_fd = fd;
  bp_hub_note(fd, line);
  snprintf(out, sizeof out, "ok");
done:
  free(path);
  return io_str(e, out, strlen(out));
}

static void __attribute__((constructor)) hub_lock_take_use(void) {
  io_eff(CID(HubLock.take), hub_lock_take_run);
}
#endif

#ifdef CID(HubLock.held)
// "" when no other process holds the lock (or this one does), else what
// the holder wrote ("<pid> <port>"; "? ?" when it wrote nothing)
Term hub_lock_held_run(Env e, Term* f, IoWork* w) {
  u64 len = 0;
  char* path = io_cstr(e, f[0], &len);
  char out[256];
  out[0] = 0;
  if (!bp_hub_same(bp_hub_fd, path)) {
    int fd = open(path, O_RDONLY | O_CLOEXEC);
    if (fd >= 0) {
      if (flock(fd, LOCK_SH | LOCK_NB) == 0) {
        flock(fd, LOCK_UN);
      } else if (errno == EWOULDBLOCK) {
        bp_hub_read(path, out, sizeof out);
        if (!out[0]) snprintf(out, sizeof out, "? ?");
      }
      close(fd);
    }
  }
  free(path);
  return io_str(e, out, strlen(out));
}

static void __attribute__((constructor)) hub_lock_held_use(void) {
  io_eff(CID(HubLock.held), hub_lock_held_run);
}
#endif
