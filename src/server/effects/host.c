// Backplane host effects
// ======================
//
// The few things Base's effects do not cover: byte-exact sockets, child
// processes over a socketpair (so TCP.recv/TCP.send/TCP.poll serve their
// stdio), a wall clock, and filesystem metadata. Each block is compiled
// only when its def is used (#ifdef CID_...), as Base's own effects do.

#include <dirent.h>
#include <fcntl.h>
#include <signal.h>
#include <spawn.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>

#if defined(__CYGWIN__) && defined(CID(Proc.exited))
#include "windows-child.h"
#endif

extern char** environ;

#ifndef MSG_NOSIGNAL
#define MSG_NOSIGNAL 0
#endif

#if defined(__CYGWIN__) && defined(CID(Proc.spawn))
#include "windows-stdio.h"
#endif

// A peer that hangs up must not kill the server.
static void __attribute__((constructor)) host_sigpipe(void) {
  signal(SIGPIPE, SIG_IGN);
}

static __attribute__((unused)) Term host_unit(void) {
  return term_pak(CID_UNIT, 0);
}

static __attribute__((unused)) Term host_bytes(Env e, const uint8_t* p, u64 n) {
  Term xs = term_pak(CID_NIL, 0);
  for (u64 i = n; i > 0; i -= 1) {
    xs = io_node(e, CID_CON, (Term)p[i - 1], xs);
  }
  return xs;
}

// Socket handles may also wrap a pty master (see pty.c): recv and send
// answer ENOTSOCK there, so they fall back to read and write. A pty whose
// other side has closed reads EIO, which means end of stream.
static __attribute__((unused)) ssize_t host_read(int fd, void* p, size_t n) {
  ssize_t r = recv(fd, p, n, 0);
  if (r < 0 && errno == ENOTSOCK) {
    r = read(fd, p, n);
    if (r < 0 && errno == EIO) {
      r = 0;
    }
  }
  return r;
}

static __attribute__((unused)) ssize_t host_write(int fd, const void* p, size_t n) {
  ssize_t r = send(fd, p, n, MSG_NOSIGNAL);
  if (r < 0 && errno == ENOTSOCK) {
    r = write(fd, p, n);
  }
  return r;
}

// Sends what is left of w->data; a full socket parks until writable.
static __attribute__((unused)) Term host_send_more(Env e, IoWork* w) {
  int fd = (int)w->hand;
  while (w->code == 0 && (u64)w->made < w->size) {
    ssize_t n = host_write(fd, w->data + w->made, w->size - (u64)w->made);
    if (n < 0 && errno == EAGAIN) {
      return io_wait_on(w, fd, POLLOUT, 0, host_send_more);
    }
    w->made += io_sys_end(w, n);
  }
  Term r = w->code != 0 ? io_fail(e, w->code, NULL) : io_done(e, host_unit());
  free(w->data);
  return io_tup(e, io_hand(w->hand), r);
}

// Clock
// =====

#ifdef CID(Clock.epoch)

Term clock_epoch_run(Env e, Term* f, IoWork* w) {
  return (Term)(uint32_t)time(NULL);
}

static void __attribute__((constructor)) clock_epoch_use(void) {
  io_eff(CID(Clock.epoch), clock_epoch_run);
}

#endif

// Sock
// ====

#ifdef CID(Sock.recv_bytes)

static Term sock_recv_bytes_more(Env e, IoWork* w) {
  int fd  = (int)w->hand;
  w->size = io_sys_end(w, host_read(fd, w->data, (size_t)w->made));
  if (w->code == EAGAIN) {
    return io_wait_on(w, fd, POLLIN, 0, sock_recv_bytes_more);
  }
  Term r = w->code ? io_fail(e, w->code, NULL)
    : io_done(e, host_bytes(e, (uint8_t*)w->data, w->size));
  free(w->data);
  return io_tup(e, io_hand(w->hand), r);
}

// Up to max bytes as they are; [] means the peer closed its side.
Term sock_recv_bytes_run(Env e, Term* f, IoWork* w) {
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->made = f[1] < INT32_MAX ? (intptr_t)f[1] : INT32_MAX;
  w->made = w->made > 0 ? w->made : 1;
  w->data = io_mem(malloc((size_t)w->made));
  // park before the first try: a socket with a steady backlog would never
  // let the loop poll (the C side of bend 2.0.36 has no IO_READ need)
  return io_wait_on(w, (int)w->hand, POLLIN, 0, sock_recv_bytes_more);
}

static void __attribute__((constructor)) sock_recv_bytes_use(void) {
  io_eff(CID(Sock.recv_bytes), sock_recv_bytes_run);
}

#endif

#ifdef CID(Sock.save)

// Copies what is left (w->size bytes) from the socket to the file w->made,
// a piece at a time, parking while the socket has nothing.
static Term sock_save_more(Env e, IoWork* w) {
  int fd  = (int)w->hand;
  int out = (int)w->made;
  while (w->code == 0 && w->size > 0) {
    ssize_t n = host_read(fd, w->data, w->size < 65536 ? (size_t)w->size : 65536);
    if (n < 0 && errno == EAGAIN) {
      return io_wait_on(w, fd, POLLIN, 0, sock_save_more);
    }
    if (n <= 0) {
      w->code = n == 0 ? EPIPE : (u32)errno;
      break;
    }
    ssize_t off = 0;
    while (w->code == 0 && off < n) {
      ssize_t k = write(out, w->data + off, (size_t)(n - off));
      if (k < 0 && errno == EINTR) {
        continue;
      }
      w->code = k < 0 ? (u32)errno : 0;
      off += k < 0 ? 0 : k;
    }
    w->size -= (u64)n;
  }
  close(out);
  free(w->data);
  Term r = w->code ? io_fail(e, w->code, NULL) : io_done(e, host_unit());
  return io_tup(e, io_hand(w->hand), r);
}

// The next n bytes of the socket appended to the file at path, never held
// as terms: the body of an upload goes straight to disk. A peer that
// closes before n bytes fails with EPIPE.
Term sock_save_run(Env e, Term* f, IoWork* w) {
  w->hand = (intptr_t)io_hand_v(f[0]);
  u64 n = 0;
  char* p = io_cstr(e, f[1], &n);
  int out = io_nul(p, n) ? -1 : open(p, O_WRONLY | O_APPEND | O_CREAT, 0600);
  int code = out < 0 ? (io_nul(p, n) ? EILSEQ : errno) : 0;
  free(p);
  if (code) {
    return io_tup(e, io_hand(w->hand), io_fail(e, code, NULL));
  }
  w->made = out;
  w->size = (u64)(uint32_t)f[2];
  w->code = 0;
  w->data = io_mem(malloc(65536));
  return w->size ? io_wait_on(w, (int)w->hand, POLLIN, 0, sock_save_more) : sock_save_more(e, w);
}

static void __attribute__((constructor)) sock_save_use(void) {
  io_eff(CID(Sock.save), sock_save_run);
}

#endif

#ifdef CID(Sock.send_bytes)

Term sock_send_bytes_run(Env e, Term* f, IoWork* w) {
  u64  cap = 256;
  Term xs  = f[1];
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->code = 0;
  w->size = 0;
  w->made = 0;
  w->data = io_mem(malloc(cap));
  while (term_aux(xs) == CID_CON) {
    Term fb[2];
    spare_free(e, cls_fit(2), ctr_take(e, xs, 2, fb));
    if (w->size == cap) {
      cap *= 2;
      w->data = io_mem(realloc(w->data, cap));
    }
    w->code = fb[0] > 255 ? EINVAL : w->code;
    w->data[w->size++] = (char)fb[0];
    xs = fb[1];
  }
  if (w->code) {
    free(w->data);
    return io_tup(e, io_hand(w->hand), io_fail(e, w->code, NULL));
  }
  return host_send_more(e, w);
}

static void __attribute__((constructor)) sock_send_bytes_use(void) {
  io_eff(CID(Sock.send_bytes), sock_send_bytes_run);
}

#endif

#ifdef CID(Sock.send_until)

// Like Sock.send_bytes, with a deadline: a peer that stops reading cannot
// hold the writer past it. The deadline is renewed after every write that
// made progress, so a slow reader that keeps reading is never cut off,
// only one that stalls for the whole span. The deadline and the span (both
// in io_tick units) sit in the first 16 bytes of w->data; the frame
// follows. A send that fails or runs out of time shuts the socket both
// ways, so its reader sees the end too and the client leaves.
static Term sock_send_until_more(Env e, IoWork* w) {
  int fd = (int)w->hand;
  u64 deadline;
  u64 span;
  memcpy(&deadline, w->data, sizeof deadline);
  memcpy(&span, w->data + sizeof deadline, sizeof span);
  while (w->code == 0 && (u64)w->made < w->size) {
    ssize_t n = host_write(fd, w->data + w->made, w->size - (u64)w->made);
    if (n < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
      if (io_tick() >= deadline) {
        w->code = ETIMEDOUT;
        break;
      }
      return io_wait_on(w, fd, POLLOUT, deadline, sock_send_until_more);
    }
    w->made += io_sys_end(w, n);
    if (n > 0) {
      deadline = io_tick() + span;
      memcpy(w->data, &deadline, sizeof deadline);
    }
  }
  if (w->code != 0) {
    shutdown(fd, SHUT_RDWR);
  }
  Term r = w->code != 0 ? io_fail(e, w->code, NULL) : io_done(e, host_unit());
  free(w->data);
  return io_tup(e, io_hand(w->hand), r);
}

Term sock_send_until_run(Env e, Term* f, IoWork* w) {
  u64  cap = 256;
  Term xs  = f[1];
  u64  span = (u64)f[2] * 1000000ull;
  u64  deadline = io_tick() + span;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->code = 0;
  w->size = sizeof deadline + sizeof span;
  w->made = sizeof deadline + sizeof span;
  w->data = io_mem(malloc(cap));
  memcpy(w->data, &deadline, sizeof deadline);
  memcpy(w->data + sizeof deadline, &span, sizeof span);
  while (term_aux(xs) == CID_CON) {
    Term fb[2];
    spare_free(e, cls_fit(2), ctr_take(e, xs, 2, fb));
    if (w->size == cap) {
      cap *= 2;
      w->data = io_mem(realloc(w->data, cap));
    }
    w->code = fb[0] > 255 ? EINVAL : w->code;
    w->data[w->size++] = (char)fb[0];
    xs = fb[1];
  }
  if (w->code) {
    free(w->data);
    return io_tup(e, io_hand(w->hand), io_fail(e, w->code, NULL));
  }
  return sock_send_until_more(e, w);
}

static void __attribute__((constructor)) sock_send_until_use(void) {
  io_eff(CID(Sock.send_until), sock_send_until_run);
}

#endif

#ifdef CID(Sock.send_text)

// A String sent as UTF-8, never raising SIGPIPE.
Term sock_send_text_run(Env e, Term* f, IoWork* w) {
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->data = io_cstr(e, f[1], &w->size);
  w->made = 0;
  w->code = 0;
  return host_send_more(e, w);
}

static void __attribute__((constructor)) sock_send_text_use(void) {
  io_eff(CID(Sock.send_text), sock_send_text_run);
}

#endif

#ifdef CID(Sock.dup)

#include <netinet/in.h>
#include <netinet/tcp.h>

// A second handle on the same socket, so one computation reads while
// another writes. Each handle is closed on its own. A socket split this way
// is a live link (a client's WebSocket, the window's link to another hub):
// each frame goes out at once, not held by Nagle until the last one is
// acknowledged (a round trip per small frame on a phone's link). A pipe or
// socketpair refuses the option, which changes nothing.
Term sock_dup_run(Env e, Term* f, IoWork* w) {
  int fd  = (int)io_hand_v(f[0]);
  int one = 1;
  setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &one, sizeof one);
  // a peer gone without a word (a phone that lost its network) is found
  // in about 35 s idle, and unacknowledged data gives up after 30 s; on a
  // socketpair these just fail
  setsockopt(fd, SOL_SOCKET, SO_KEEPALIVE, &one, sizeof one);
#ifdef TCP_KEEPIDLE
  int idle = 20, intvl = 5, cnt = 3;
  setsockopt(fd, IPPROTO_TCP, TCP_KEEPIDLE, &idle, sizeof idle);
  setsockopt(fd, IPPROTO_TCP, TCP_KEEPINTVL, &intvl, sizeof intvl);
  setsockopt(fd, IPPROTO_TCP, TCP_KEEPCNT, &cnt, sizeof cnt);
#endif
#ifdef TCP_USER_TIMEOUT
  unsigned int uto = 30000;
  setsockopt(fd, IPPROTO_TCP, TCP_USER_TIMEOUT, &uto, sizeof uto);
#endif
  int got = dup(fd);
  if (got >= 0) {
    fcntl(got, F_SETFD, FD_CLOEXEC);
  }
  Term r = got < 0 ? io_fail(e, errno, NULL) : io_done(e, io_hand(got));
  return io_tup(e, io_hand(fd), r);
}

static void __attribute__((constructor)) sock_dup_use(void) {
  io_eff(CID(Sock.dup), sock_dup_run);
}

#endif

#ifdef CID(Sock.peer)

#include <arpa/inet.h>
#include <netinet/in.h>

// The peer's IP address as text ("" when it has none, e.g. a socketpair).
Term sock_peer_run(Env e, Term* f, IoWork* w) {
  int fd = (int)io_hand_v(f[0]);
  struct sockaddr_storage a;
  socklen_t n = sizeof a;
  char buf[INET6_ADDRSTRLEN] = {0};
  if (getpeername(fd, (struct sockaddr*)&a, &n) == 0) {
    if (a.ss_family == AF_INET) {
      inet_ntop(AF_INET, &((struct sockaddr_in*)&a)->sin_addr, buf, sizeof buf);
    } else if (a.ss_family == AF_INET6) {
      inet_ntop(AF_INET6, &((struct sockaddr_in6*)&a)->sin6_addr, buf, sizeof buf);
    }
  }
  return io_tup(e, io_hand(fd), io_str(e, buf, strlen(buf)));
}

static void __attribute__((constructor)) sock_peer_use(void) {
  io_eff(CID(Sock.peer), sock_peer_run);
}

#endif

#ifdef CID(Sock.shutdown)

// Ends our writing side (the peer reads EOF); reading continues.
Term sock_shutdown_run(Env e, Term* f, IoWork* w) {
  int fd = (int)io_hand_v(f[0]);
  shutdown(fd, SHUT_WR);
  return io_hand(fd);
}

static void __attribute__((constructor)) sock_shutdown_use(void) {
  io_eff(CID(Sock.shutdown), sock_shutdown_run);
}

#endif

// Proc
// ====

#if defined(CID(Proc.spawn)) || defined(CID(Sys.exec))

static char** host_argv(Env e, Term cmd, Term xs, int* ok) {
  u64 cap = 8, n = 0, len = 0;
  char** v = (char**)io_mem(malloc(cap * sizeof(char*)));
  v[n++] = io_cstr(e, cmd, &len);
  *ok = !io_nul(v[0], len);
  while (term_aux(xs) == CID_CON) {
    Term fb[2];
    spare_free(e, cls_fit(2), ctr_take(e, xs, 2, fb));
    if (n + 1 >= cap) {
      cap *= 2;
      v = (char**)io_mem(realloc(v, cap * sizeof(char*)));
    }
    v[n] = io_cstr(e, fb[0], &len);
    *ok = *ok && !io_nul(v[n], len);
    n += 1;
    xs = fb[1];
  }
  v[n] = NULL;
  return v;
}

#endif

#ifdef CID(Proc.spawn)

// Where an agent's stderr goes (err 3): ${BACKPLANE_ERR_DIR:-${TMPDIR:-/tmp}/backplane-err-<uid>}/<pid>.err
static int host_err_dir(char* out, size_t n) {
  const char* d = getenv("BACKPLANE_ERR_DIR");
  int k;
  if (d && *d) {
    k = snprintf(out, n, "%s", d);
  } else {
    const char* t = getenv("TMPDIR");
    k = snprintf(out, n, "%s/backplane-err-%u", t && *t ? t : "/tmp", (unsigned)getuid());
  }
  if (k <= 0 || (size_t)k >= n) return 0;
  mkdir(out, 0700);
  return 1;
}

static int host_err_path(char* out, size_t n, unsigned pid) {
  char d[4096];
  if (!host_err_dir(d, sizeof d)) return 0;
  int k = snprintf(out, n, "%s/%u.err", d, pid);
  return k > 0 && (size_t)k < n;
}

// files of processes that ended unread (their hub was killed) go, once per
// hub: those whose pid no longer runs
static void host_err_sweep(const char* d) {
  DIR* dir = opendir(d);
  if (dir == NULL) return;
  struct dirent* ent;
  while ((ent = readdir(dir)) != NULL) {
    char* end = NULL;
    unsigned long pid = strtoul(ent->d_name, &end, 10);
    if (end == NULL || strcmp(end, ".err") != 0 || pid <= 1) continue;
    if (kill((pid_t)pid, 0) != 0 && errno == ESRCH) {
      char at[4200];
      snprintf(at, sizeof at, "%s/%s", d, ent->d_name);
      unlink(at);
    }
  }
  closedir(dir);
}

static int host_err_open(char* tmp, size_t n) {
  static unsigned seq = 0;
  char d[4096];
  if (!host_err_dir(d, sizeof d)) return -1;
  if (seq == 0) host_err_sweep(d);
  int k = snprintf(tmp, n, "%s/spawn.%u.%u", d, (unsigned)getpid(), ++seq);
  if (k <= 0 || (size_t)k >= n) return -1;
  return open(tmp, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0600);
}

// Starts cmd (searched on PATH) in cwd. Its stdin and stdout are one end of
// a socketpair; we keep the other, non-blocking. stderr: 0 discards it, 1
// joins it to stdout, 2 inherits ours, 3 goes to a file read back by
// Proc.errtail. Answers (pid, socket).
Term proc_spawn_run(Env e, Term* f, IoWork* w) {
  int   ok   = 1;
  u64   clen = 0;
  char** argv = host_argv(e, f[0], f[1], &ok);
  char* cwd  = io_cstr(e, f[2], &clen);
  u32   err  = (u32)f[3];
  int   sv[2] = { -1, -1 };
#ifdef __CYGWIN__
  int   stdio[2] = { -1, -1 };
#endif
  int   code = 0;
  pid_t pid  = 0;
  int   errfd = -1;
  char  errtmp[4200];
  if (!ok || io_nul(cwd, clen)) {
    code = EILSEQ;
#ifdef __CYGWIN__
  } else if (bp_stdio_pair(sv, stdio) < 0) {
#else
  } else if (socketpair(AF_UNIX, SOCK_STREAM, 0, sv) < 0) {
#endif
    code = errno;
  } else {
    posix_spawn_file_actions_t fa;
    posix_spawn_file_actions_init(&fa);
    if (clen > 0) {
      posix_spawn_file_actions_addchdir_np(&fa, cwd);
    }
    int input = sv[1], output = sv[1];
#ifdef __CYGWIN__
    input = stdio[0]; output = stdio[1];
#endif
    posix_spawn_file_actions_adddup2(&fa, input, 0);
    posix_spawn_file_actions_adddup2(&fa, output, 1);
    if (err == 0) {
      posix_spawn_file_actions_addopen(&fa, 2, "/dev/null", O_WRONLY, 0);
    } else if (err == 1) {
      posix_spawn_file_actions_adddup2(&fa, output, 2);
    } else if (err == 3) {
      // stderr to a file of its own, named for the pid once it is known
      // (Proc.errtail reads its end and removes it)
      errfd = host_err_open(errtmp, sizeof errtmp);
      if (errfd >= 0) {
        posix_spawn_file_actions_adddup2(&fa, errfd, 2);
      } else {
        posix_spawn_file_actions_addopen(&fa, 2, "/dev/null", O_WRONLY, 0);
      }
    }
    posix_spawn_file_actions_addclose(&fa, sv[0]);
    // the child keeps only the copies on 0-2: left open, a process it puts
    // in the background would hold the pipe, and the caller never sees EOF
    if (sv[1] > 2) {
      posix_spawn_file_actions_addclose(&fa, sv[1]);
    }
#ifdef __CYGWIN__
    if (input > 2) posix_spawn_file_actions_addclose(&fa, input);
    if (output > 2) posix_spawn_file_actions_addclose(&fa, output);
#endif
    posix_spawnattr_t at;
    posix_spawnattr_init(&at);
    sigset_t def;
    sigemptyset(&def);
    sigaddset(&def, SIGPIPE);
    posix_spawnattr_setsigdefault(&at, &def);
    posix_spawnattr_setflags(&at, POSIX_SPAWN_SETSIGDEF);
#if defined(__CYGWIN__) && defined(CID(Proc.exited))
    unsigned serial = bp_child_begin();
#endif
    code = posix_spawnp(&pid, argv[0], &fa, &at, argv, environ);
#if defined(__CYGWIN__) && defined(CID(Proc.exited))
    if (code == 0) bp_child_started(pid, serial);
#endif
    posix_spawn_file_actions_destroy(&fa);
    posix_spawnattr_destroy(&at);
#ifdef __CYGWIN__
    close(input); close(output);
#else
    close(sv[1]);
#endif
    if (errfd >= 0) {
      close(errfd);
      char at[4200];
      if (code == 0 && host_err_path(at, sizeof at, (unsigned)pid)) rename(errtmp, at);
      else unlink(errtmp);
    }
    if (code != 0) {
      close(sv[0]);
    } else {
      fcntl(sv[0], F_SETFL, fcntl(sv[0], F_GETFL) | O_NONBLOCK);
      fcntl(sv[0], F_SETFD, FD_CLOEXEC);
    }
  }
  for (char** a = argv; *a != NULL; a += 1) {
    free(*a);
  }
  free(argv);
  free(cwd);
  if (code != 0) {
    return io_fail(e, code, NULL);
  }
  return io_done(e, io_tup(e, (Term)(uint32_t)pid, io_hand(sv[0])));
}

static void __attribute__((constructor)) proc_spawn_use(void) {
  io_eff(CID(Proc.spawn), proc_spawn_run);
}

#endif

#if defined(CID(Proc.errtail)) && defined(CID(Proc.spawn))

// The last 2048 bytes a process started with err 3 wrote to stderr ("" when
// none); its file is removed.
Term proc_errtail_run(Env e, Term* f, IoWork* w) {
  char path[4200];
  char buf[2048];
  ssize_t got = 0;
  if (host_err_path(path, sizeof path, (unsigned)(uint32_t)f[0])) {
    int fd = open(path, O_RDONLY | O_CLOEXEC);
    if (fd >= 0) {
      off_t end = lseek(fd, 0, SEEK_END);
      off_t at = end > (off_t)sizeof buf ? end - (off_t)sizeof buf : 0;
      if (end > 0 && lseek(fd, at, SEEK_SET) >= 0) {
        got = read(fd, buf, sizeof buf);
        if (got < 0) got = 0;
      }
      close(fd);
      unlink(path);
    }
  }
  return io_str(e, buf, (u64)got);
}

static void __attribute__((constructor)) proc_errtail_use(void) {
  io_eff(CID(Proc.errtail), proc_errtail_run);
}

#endif

#ifdef CID(Proc.wait)

static void proc_wait_call(IoWork* w) {
  int st = 0;
  pid_t r;
  do {
    r = waitpid((pid_t)w->word, &st, 0);
  } while (r < 0 && errno == EINTR);
  w->made = r < 0 ? 255 : WIFEXITED(st) ? WEXITSTATUS(st) : 128 + WTERMSIG(st);
}

static Term proc_wait_pack(Env e, IoWork* w) {
  return (Term)(uint32_t)w->made;
}

// The exit code (128 + signal when killed), once the child ends.
Term proc_wait_run(Env e, Term* f, IoWork* w) {
  w->word = (uint32_t)f[0];
  return io_work(w, proc_wait_call, proc_wait_pack);
}

static void __attribute__((constructor)) proc_wait_use(void) {
  io_eff(CID(Proc.wait), proc_wait_run);
}

#endif

#ifdef CID(Proc.exited)

// Proc.exited: the exit code of a child that has exited (left unreaped:
// WNOWAIT), 4294967295 while it runs; one already reaped reads 255.
Term proc_exited_run(Env e, Term* f, IoWork* w) {
#ifdef __CYGWIN__
  return (Term)bp_child_exited((pid_t)(uint32_t)f[0]);
#else
  siginfo_t si;
  memset(&si, 0, sizeof si);
  int r = waitid(P_PID, (id_t)(uint32_t)f[0], &si, WEXITED | WNOHANG | WNOWAIT);
  uint32_t code;
  if (r < 0) {
    code = 255;
  } else if (si.si_pid == 0) {
    code = 4294967295u;
  } else {
    code = si.si_code == CLD_EXITED ? (uint32_t)si.si_status : 128u + (uint32_t)si.si_status;
  }
  return (Term)code;
#endif
}

static void __attribute__((constructor)) proc_exited_use(void) {
  io_eff(CID(Proc.exited), proc_exited_run);
}

#endif

#ifdef CID(Sock.shut_read)

// Ends our reading side: a read blocked on this socket returns 0.
Term sock_shut_read_run(Env e, Term* f, IoWork* w) {
  int fd = (int)io_hand_v(f[0]);
  shutdown(fd, SHUT_RD);
  return io_hand(fd);
}

static void __attribute__((constructor)) sock_shut_read_use(void) {
  io_eff(CID(Sock.shut_read), sock_shut_read_run);
}

#endif

#ifdef CID(Sock.raw)

// A raw descriptor on the same socket (a dup, close-on-exec), for the hub
// to shut a client's connection at once from outside its writer
// (Fd.shut); 0 when none could be made.
Term sock_raw_run(Env e, Term* f, IoWork* w) {
  int fd  = (int)io_hand_v(f[0]);
  int got = dup(fd);
  if (got >= 0) {
    fcntl(got, F_SETFD, FD_CLOEXEC);
  }
  return io_tup(e, io_hand(fd), (Term)(uint32_t)(got < 3 ? 0 : got));
}

static void __attribute__((constructor)) sock_raw_use(void) {
  io_eff(CID(Sock.raw), sock_raw_run);
}

#endif

#ifdef CID(Fd.shut)

// Shuts the socket under a raw descriptor both ways (every handle on it:
// a writer blocked on it fails, its reader reads the end) and closes the
// descriptor. Never 0, 1 or 2.
Term fd_shut_run(Env e, Term* f, IoWork* w) {
  int fd = (int)(uint32_t)f[0];
  if (fd > 2) {
    shutdown(fd, SHUT_RDWR);
    close(fd);
  }
  return host_unit();
}

static void __attribute__((constructor)) fd_shut_use(void) {
  io_eff(CID(Fd.shut), fd_shut_run);
}

#endif

#ifdef CID(Fd.close)

// Closes a raw descriptor (never 0, 1 or 2); the socket stays as it is.
Term fd_close_run(Env e, Term* f, IoWork* w) {
  int fd = (int)(uint32_t)f[0];
  if (fd > 2) {
    close(fd);
  }
  return host_unit();
}

static void __attribute__((constructor)) fd_close_use(void) {
  io_eff(CID(Fd.close), fd_close_run);
}

#endif

#ifdef CID(Proc.kill)

Term proc_kill_run(Env e, Term* f, IoWork* w) {
  kill((pid_t)(uint32_t)f[0], (int)(uint32_t)f[1]);
  return host_unit();
}

static void __attribute__((constructor)) proc_kill_use(void) {
  io_eff(CID(Proc.kill), proc_kill_run);
}

#endif

// FS
// ==

#ifdef CID(FS.stat)

// (kind, (size, mtime)): kind 0 file, 1 directory, 2 other.
Term fs_stat_run(Env e, Term* f, IoWork* w) {
  u64 n = 0;
  char* p = io_cstr(e, f[0], &n);
  struct stat st;
  int r = io_nul(p, n) ? -1 : stat(p, &st);
  int code = r < 0 ? (io_nul(p, n) ? EILSEQ : errno) : 0;
  free(p);
  if (code) {
    return io_fail(e, code, NULL);
  }
  u32 kind = S_ISREG(st.st_mode) ? 0 : S_ISDIR(st.st_mode) ? 1 : 2;
  u32 size = st.st_size > 0xFFFFFFFFll ? 0xFFFFFFFFu : (u32)st.st_size;
  return io_done(e, io_tup(e, (Term)kind,
    io_tup(e, (Term)size, (Term)(uint32_t)st.st_mtime)));
}

static void __attribute__((constructor)) fs_stat_use(void) {
  io_eff(CID(FS.stat), fs_stat_run);
}

#endif

#ifdef CID(FS.list)

// The names in a directory, without "." and "..", in no set order.
Term fs_list_run(Env e, Term* f, IoWork* w) {
  u64 n = 0;
  char* p = io_cstr(e, f[0], &n);
  DIR* d = io_nul(p, n) ? NULL : opendir(p);
  int code = d == NULL ? (io_nul(p, n) ? EILSEQ : errno) : 0;
  free(p);
  if (code) {
    return io_fail(e, code, NULL);
  }
  Term xs = term_pak(CID_NIL, 0);
  struct dirent* ent;
  while ((ent = readdir(d)) != NULL) {
    const char* s = ent->d_name;
    if (strcmp(s, ".") == 0 || strcmp(s, "..") == 0) {
      continue;
    }
    xs = io_node(e, CID_CON, io_str(e, s, strlen(s)), xs);
  }
  closedir(d);
  return io_done(e, xs);
}

static void __attribute__((constructor)) fs_list_use(void) {
  io_eff(CID(FS.list), fs_list_run);
}

#endif

#ifdef CID(FS.mkdirs)

// mkdir -p
Term fs_mkdirs_run(Env e, Term* f, IoWork* w) {
  u64 n = 0;
  char* p = io_cstr(e, f[0], &n);
  int code = io_nul(p, n) ? EILSEQ : 0;
  for (u64 i = 1; code == 0 && i <= n; i += 1) {
    if (i == n || p[i] == '/') {
      char c = p[i];
      p[i] = 0;
      if (mkdir(p, 0755) < 0 && errno != EEXIST) {
        code = errno;
      }
      p[i] = c;
    }
  }
  free(p);
  return code ? io_fail(e, code, NULL) : io_done(e, host_unit());
}

static void __attribute__((constructor)) fs_mkdirs_use(void) {
  io_eff(CID(FS.mkdirs), fs_mkdirs_run);
}

#endif

#ifdef CID(FS.rename)

// Atomic within a filesystem: writers stage a file, then rename it over.
Term fs_rename_run(Env e, Term* f, IoWork* w) {
  u64 a = 0, b = 0;
  char* from = io_cstr(e, f[0], &a);
  char* to   = io_cstr(e, f[1], &b);
  int code = io_nul(from, a) || io_nul(to, b) ? EILSEQ
    : rename(from, to) < 0 ? errno : 0;
  free(from);
  free(to);
  return code ? io_fail(e, code, NULL) : io_done(e, host_unit());
}

static void __attribute__((constructor)) fs_rename_use(void) {
  io_eff(CID(FS.rename), fs_rename_run);
}

#endif

#ifdef CID(FS.remove)

// Removes a file or an empty directory.
Term fs_remove_run(Env e, Term* f, IoWork* w) {
  u64 n = 0;
  char* p = io_cstr(e, f[0], &n);
  int code = io_nul(p, n) ? EILSEQ : remove(p) < 0 ? errno : 0;
  free(p);
  return code ? io_fail(e, code, NULL) : io_done(e, host_unit());
}

static void __attribute__((constructor)) fs_remove_use(void) {
  io_eff(CID(FS.remove), fs_remove_run);
}

#endif

#ifdef CID(FS.chmod)

Term fs_chmod_run(Env e, Term* f, IoWork* w) {
  u64 n = 0;
  char* p = io_cstr(e, f[0], &n);
  int code = io_nul(p, n) ? EILSEQ : chmod(p, (mode_t)(uint32_t)f[1]) < 0 ? errno : 0;
  free(p);
  return code ? io_fail(e, code, NULL) : io_done(e, host_unit());
}

static void __attribute__((constructor)) fs_chmod_use(void) {
  io_eff(CID(FS.chmod), fs_chmod_run);
}

#endif

// Net
// ===

#ifdef CID(Net.listen)

// TCP.listen binds every interface; the harness runs agents with shell
// access, so it listens on the host it is told (127.0.0.1 by default).
Term net_listen_run(Env e, Term* f, IoWork* w) {
  u64 n = 0;
  char* host = io_cstr(e, f[0], &n);
  uint32_t port = (uint32_t)f[1];
  struct sockaddr_in at;
  int code = 0, fd = -1;
  if (io_nul(host, n) || io_sys_addr(host, port, &at) < 0) {
    code = EINVAL;
  } else if ((fd = socket(AF_INET, SOCK_STREAM, 0)) < 0) {
    code = errno;
  } else {
    int one = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one));
    if (bind(fd, (struct sockaddr*)&at, sizeof(at)) < 0 || listen(fd, 64) < 0
      || fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_NONBLOCK) < 0) {
      code = errno;
      close(fd);
    } else {
      fcntl(fd, F_SETFD, FD_CLOEXEC);
    }
  }
  free(host);
  return code ? io_fail(e, code, NULL) : io_done(e, io_hand(fd));
}

static void __attribute__((constructor)) net_listen_use(void) {
  io_eff(CID(Net.listen), net_listen_run);
}

#endif

// Sys
// ===

#if defined(CID(Sys.exe_dir)) || defined(CID(Sys.exe_path))

#ifdef __APPLE__
#include <mach-o/dyld.h>
#endif

// the running binary's full path (n = its length; 0 when unknown)
static ssize_t host_exe(char* buf, size_t cap) {
  ssize_t n = -1;
#ifdef __APPLE__
  uint32_t size = (uint32_t)cap;
  char raw[4096];
  if (_NSGetExecutablePath(raw, &size) == 0 && realpath(raw, buf) != NULL) {
    n = (ssize_t)strlen(buf);
  }
#else
  n = readlink("/proc/self/exe", buf, cap - 1);
#endif
  if (n > 0) {
    buf[n] = 0;
  }
  return n;
}

#endif

#ifdef CID(Sys.exe_path)

Term sys_exe_path_run(Env e, Term* f, IoWork* w) {
  char buf[4096];
  ssize_t n = host_exe(buf, sizeof buf);
  return io_str(e, buf, n > 0 ? (u64)n : 0);
}

static void __attribute__((constructor)) sys_exe_path_use(void) {
  io_eff(CID(Sys.exe_path), sys_exe_path_run);
}

#endif

#ifdef CID(Env.put)

// setenv(name, value, 1): processes started afterwards inherit it.
Term env_put_run(Env e, Term* f, IoWork* w) {
  u64 a = 0, b = 0;
  char* name  = io_cstr(e, f[0], &a);
  char* value = io_cstr(e, f[1], &b);
  if (a > 0 && !io_nul(name, a) && !io_nul(value, b)) {
    setenv(name, value, 1);
  }
  free(name);
  free(value);
  return host_unit();
}

static void __attribute__((constructor)) env_put_use(void) {
  io_eff(CID(Env.put), env_put_run);
}

#endif

#ifdef CID(Sys.exe_dir)

// The directory holding the running binary ("" when unknown).
Term sys_exe_dir_run(Env e, Term* f, IoWork* w) {
  char buf[4096];
  ssize_t n = host_exe(buf, sizeof buf);
  if (n <= 0) {
    return io_str(e, "", 0);
  }
  char* slash = strrchr(buf, '/');
  size_t len = slash == NULL ? 0 : (size_t)(slash - buf);
  return io_str(e, buf, len);
}

static void __attribute__((constructor)) sys_exe_dir_use(void) {
  io_eff(CID(Sys.exe_dir), sys_exe_dir_run);
}

#endif

#ifdef CID(Sys.exec)

// Replace this process with path (argv[0] = path, then args). Descriptors
// are close-on-exec, so agents see their stdin close and exit. Answers only
// on failure.
// the agents kept through the exec (effects/adopt.c, when linked)
extern void bp_adopt_exec(int keep) __attribute__((weak));
// the hub's lock on its home, kept through the exec (effects/hublock.c)
extern void bp_hublock_exec(int keep) __attribute__((weak));

Term sys_exec_run(Env e, Term* f, IoWork* w) {
  int ok = 1;
  char** argv = host_argv(e, f[0], f[1], &ok);
  int code = ok ? 0 : EILSEQ;
  if (ok) {
    if (bp_adopt_exec) bp_adopt_exec(1);
    if (bp_hublock_exec) bp_hublock_exec(1);
    execv(argv[0], argv);
    code = errno;
    if (bp_hublock_exec) bp_hublock_exec(0);
    if (bp_adopt_exec) bp_adopt_exec(0);
  }
  for (char** a = argv; *a != NULL; a += 1) {
    free(*a);
  }
  free(argv);
  return io_fail(e, code, NULL);
}

static void __attribute__((constructor)) sys_exec_use(void) {
  io_eff(CID(Sys.exec), sys_exec_run);
}

#endif

// JSON text to CBOR, as Bend does it
// ===================================
// core/cbor.bend's Cbor.encode over J.Json.parse, made in one pass over the
// text: numbers that are plain 32-bit integers as major 0 or 1, any other
// number as tag 7 over its text; strings in the words dictionary as tag 6
// over their index; map keys in the keys dictionary as their index; heads in
// shortest form; definite lengths. Text that does not parse is null, as
// Maybe.default(J.Null) makes it. Sock.send_cbor frames and sends it. The
// hub only ever sends JSON that Bend printed (Json.show); for that the bytes
// are the same (test/native/cbor_frame_test.bend). Bend's parser is lenient
// about malformed text (it skips commas and colons) and this one is not.

#if defined(CID(Sock.send_cbor)) || defined(CID(Json.cbor_frame)) || defined(CID(Sock.send_items)) || defined(CID(Json.items_frames))

typedef struct { uint8_t* p; size_t n, cap; int bad; } JcBuf;
typedef struct { const char* p[128]; size_t n[128]; int len; } JcDict;
typedef struct { const char* s; size_t i, n; JcBuf* out; JcBuf tmp; JcDict* keys; JcDict* words; int depth; } Jc;

static void jc_room(JcBuf* b, size_t more) {
  if (b->n + more <= b->cap) return;
  size_t cap = b->cap ? b->cap * 2 : 4096;
  while (cap < b->n + more) cap *= 2;
  uint8_t* q = realloc(b->p, cap);
  if (!q) { b->bad = 1; return; }
  b->p = q;
  b->cap = cap;
}

static void jc_put(JcBuf* b, const void* s, size_t n) {
  jc_room(b, n);
  if (b->bad) return;
  memcpy(b->p + b->n, s, n);
  b->n += n;
}

static void jc_byte(JcBuf* b, uint8_t c) { jc_put(b, &c, 1); }

// an item head in shortest form (mb: the major type shifted, major * 32)
static size_t jc_headw(uint32_t v) { return v < 24 ? 1 : v < 256 ? 2 : v < 65536 ? 3 : 5; }
static void jc_head_at(uint8_t* q, uint8_t mb, uint32_t v) {
  if (v < 24) { q[0] = mb | v; }
  else if (v < 256) { q[0] = mb | 24; q[1] = v; }
  else if (v < 65536) { q[0] = mb | 25; q[1] = v >> 8; q[2] = v; }
  else { q[0] = mb | 26; q[1] = v >> 24; q[2] = v >> 16; q[3] = v >> 8; q[4] = v; }
}
static void jc_head(JcBuf* b, uint8_t mb, uint32_t v) {
  uint8_t q[5];
  jc_head_at(q, mb, v);
  jc_put(b, q, jc_headw(v));
}

static void jc_dict(JcDict* d, const char* s, size_t n) {
  d->len = 0;
  size_t i = 0;
  while (i <= n && d->len < 128) {
    size_t j = i;
    while (j < n && s[j] != '\n') j += 1;
    if (j > i || j < n) { d->p[d->len] = s + i; d->n[d->len] = j - i; d->len += 1; }
    i = j + 1;
  }
}

static int jc_find(JcDict* d, const uint8_t* s, size_t n) {
  for (int i = 0; i < d->len; i += 1) {
    if (d->n[i] == n && memcmp(d->p[i], s, n) == 0) return i;
  }
  return -1;
}

static void jc_ws(Jc* c) {
  while (c->i < c->n && (c->s[c->i] == ' ' || c->s[c->i] == '\t' || c->s[c->i] == '\n' || c->s[c->i] == '\r')) c->i += 1;
}

static void jc_utf8(JcBuf* b, uint32_t cp) {
  if (cp < 0x80) jc_byte(b, cp);
  else if (cp < 0x800) { jc_byte(b, 0xC0 | (cp >> 6)); jc_byte(b, 0x80 | (cp & 63)); }
  else if (cp < 0x10000) { jc_byte(b, 0xE0 | (cp >> 12)); jc_byte(b, 0x80 | ((cp >> 6) & 63)); jc_byte(b, 0x80 | (cp & 63)); }
  else { jc_byte(b, 0xF0 | (cp >> 18)); jc_byte(b, 0x80 | ((cp >> 12) & 63)); jc_byte(b, 0x80 | ((cp >> 6) & 63)); jc_byte(b, 0x80 | (cp & 63)); }
}

static int jc_hex4(Jc* c, uint32_t* v) {
  if (c->i + 4 > c->n) return 0;
  uint32_t x = 0;
  for (int k = 0; k < 4; k += 1) {
    char h = c->s[c->i + k];
    x <<= 4;
    if (h >= '0' && h <= '9') x |= h - '0';
    else if (h >= 'a' && h <= 'f') x |= h - 'a' + 10;
    else if (h >= 'A' && h <= 'F') x |= h - 'A' + 10;
    else return 0;
  }
  c->i += 4;
  *v = x;
  return 1;
}

// a string's text (after its opening quote) into c->tmp, unescaped
static int jc_string(Jc* c) {
  c->tmp.n = 0;
  while (c->i < c->n) {
    char ch = c->s[c->i++];
    if (ch == '"') return !c->tmp.bad;
    if (ch != '\\') { jc_byte(&c->tmp, (uint8_t)ch); continue; }
    if (c->i >= c->n) return 0;
    char e = c->s[c->i++];
    switch (e) {
      case '"': jc_byte(&c->tmp, '"'); break;
      case '\\': jc_byte(&c->tmp, '\\'); break;
      case '/': jc_byte(&c->tmp, '/'); break;
      case 'b': jc_byte(&c->tmp, 8); break;
      case 'f': jc_byte(&c->tmp, 12); break;
      case 'n': jc_byte(&c->tmp, 10); break;
      case 'r': jc_byte(&c->tmp, 13); break;
      case 't': jc_byte(&c->tmp, 9); break;
      case 'u': {
        uint32_t v;
        // each \uXXXX is one character, a surrogate's half too, as Bend's
        // lexer reads it (core/json.bend's Lex.uni)
        if (!jc_hex4(c, &v)) return 0;
        jc_utf8(&c->tmp, v);
        break;
      }
      default: return 0;
    }
  }
  return 0;
}

static void jc_text(JcBuf* b, const uint8_t* s, size_t n) {
  jc_head(b, 0x60, (uint32_t)n);
  jc_put(b, s, n);
}

// digits only, 1..10 of them, no leading zero, below 2^32
static int jc_pos(const char* s, size_t n, uint32_t* v) {
  if (n < 1 || n > 10) return 0;
  if (s[0] == '0' && n > 1) return 0;
  uint64_t x = 0;
  for (size_t k = 0; k < n; k += 1) {
    if (s[k] < '0' || s[k] > '9') return 0;
    x = x * 10 + (uint64_t)(s[k] - '0');
  }
  if (x > 0xFFFFFFFFull) return 0;
  *v = (uint32_t)x;
  return 1;
}

static void jc_number(Jc* c, JcBuf* b) {
  size_t a = c->i;
  while (c->i < c->n) {
    char ch = c->s[c->i];
    if ((ch >= '0' && ch <= '9') || ch == '-' || ch == '+' || ch == '.' || ch == 'e' || ch == 'E') c->i += 1;
    else break;
  }
  const char* r = c->s + a;
  size_t n = c->i - a;
  uint32_t v;
  if (n > 0 && r[0] == '-') {
    if (jc_pos(r + 1, n - 1, &v) && v > 0) { jc_head(b, 0x20, v - 1); return; }
  } else if (jc_pos(r, n, &v)) {
    jc_head(b, 0x00, v);
    return;
  }
  jc_byte(b, 0xC7);
  jc_text(b, (const uint8_t*)r, n);
}

static int jc_value(Jc* c, JcBuf* b);

// a container: its items written after room for the longest head, then the
// head put in front of them in its shortest form
static int jc_items(Jc* c, JcBuf* b, int map) {
  size_t at = b->n;
  jc_room(b, 5);
  if (b->bad) return 0;
  b->n += 5;
  uint32_t count = 0;
  jc_ws(c);
  if (c->i < c->n && c->s[c->i] == (map ? '}' : ']')) {
    c->i += 1;
  } else {
    for (;;) {
      jc_ws(c);
      if (map) {
        if (c->i >= c->n || c->s[c->i] != '"') return 0;
        c->i += 1;
        if (!jc_string(c)) return 0;
        int k = jc_find(c->keys, c->tmp.p, c->tmp.n);
        if (k >= 0) jc_head(b, 0x00, (uint32_t)k);
        else jc_text(b, c->tmp.p, c->tmp.n);
        jc_ws(c);
        if (c->i >= c->n || c->s[c->i] != ':') return 0;
        c->i += 1;
      }
      if (!jc_value(c, b)) return 0;
      count += 1;
      jc_ws(c);
      if (c->i >= c->n) return 0;
      char ch = c->s[c->i++];
      if (ch == ',') continue;
      if (ch == (map ? '}' : ']')) break;
      return 0;
    }
  }
  size_t w = jc_headw(count);
  memmove(b->p + at + w, b->p + at + 5, b->n - at - 5);
  b->n -= 5 - w;
  jc_head_at(b->p + at, map ? 0xA0 : 0x80, count);
  return !b->bad;
}

static int jc_value(Jc* c, JcBuf* b) {
  if (++c->depth > 512) return 0;
  jc_ws(c);
  int ok = 0;
  if (c->i >= c->n) ok = 0;
  else {
    char ch = c->s[c->i];
    if (ch == '{') { c->i += 1; ok = jc_items(c, b, 1); }
    else if (ch == '[') { c->i += 1; ok = jc_items(c, b, 0); }
    else if (ch == '"') {
      c->i += 1;
      ok = jc_string(c);
      if (ok) {
        int w = jc_find(c->words, c->tmp.p, c->tmp.n);
        if (w >= 0) { jc_byte(b, 0xC6); jc_head(b, 0x00, (uint32_t)w); }
        else jc_text(b, c->tmp.p, c->tmp.n);
      }
    }
    else if (c->i + 4 <= c->n && memcmp(c->s + c->i, "true", 4) == 0) { c->i += 4; jc_byte(b, 0xF5); ok = 1; }
    else if (c->i + 5 <= c->n && memcmp(c->s + c->i, "false", 5) == 0) { c->i += 5; jc_byte(b, 0xF4); ok = 1; }
    else if (c->i + 4 <= c->n && memcmp(c->s + c->i, "null", 4) == 0) { c->i += 4; jc_byte(b, 0xF6); ok = 1; }
    else if (ch == '-' || (ch >= '0' && ch <= '9')) { jc_number(c, b); ok = 1; }
  }
  c->depth -= 1;
  return ok && !b->bad;
}

// the WebSocket frame (FIN, binary) of text's CBOR, after `pre` bytes left
// for the caller; 0 when out of memory
static int jc_frame(const char* text, size_t n, const char* ks, size_t kn, const char* ws, size_t wn, JcBuf* out, size_t pre) {
  JcDict keys, words;
  jc_dict(&keys, ks, kn);
  jc_dict(&words, ws, wn);
  JcBuf body = {0};
  Jc c = { text, 0, n, &body, {0}, &keys, &words, 0 };
  int ok = jc_value(&c, &body);
  jc_ws(&c);
  if (!ok || c.i != n) { body.n = 0; body.bad = 0; jc_byte(&body, 0xF6); }
  free(c.tmp.p);
  if (body.bad) { free(body.p); return 0; }
  uint8_t h[10];
  size_t hn;
  h[0] = 0x82;
  if (body.n < 126) { h[1] = body.n; hn = 2; }
  else if (body.n < 65536) { h[1] = 126; h[2] = body.n >> 8; h[3] = body.n; hn = 4; }
  else { h[1] = 127; h[2] = h[3] = h[4] = h[5] = 0; h[6] = body.n >> 24; h[7] = body.n >> 16; h[8] = body.n >> 8; h[9] = body.n; hn = 10; }
  out->p = malloc(pre + hn + body.n);
  if (!out->p) { free(body.p); return 0; }
  memcpy(out->p + pre, h, hn);
  memcpy(out->p + pre + hn, body.p, body.n);
  out->n = pre + hn + body.n;
  free(body.p);
  return 1;
}

#endif

#if defined(CID(Sock.send_items)) || defined(CID(Json.items_frames))

// A join frame's lines (core/outbox.bend's Items.texts.s) as the binary
// WebSocket frames of their CBOR, back to back: the first frame is hd, then
// plain (or split when the lines take more frames), the lines joined by
// commas, then tl; each later one a "changes" frame with "boot": true, the
// very last "boot": false when ends. A frame takes at least one line, then
// lines while they fit in `bytes` characters, `most` at most (Join.count.at:
// a line is counted in code points, as String.length counts it). The lines
// are read once here: in Bend each was measured three times and joined, a
// character at a time, for every frame.

typedef struct { char* p; u64 n; u64 cps; } JiLine;

static u64 ji_cps(const char* p, u64 n) {
  u64 c = 0;
  for (u64 i = 0; i < n; i += 1) c += ((uint8_t)p[i] & 0xC0) != 0x80;
  return c;
}

static u64 ji_count(JiLine* ls, u64 i, u64 n, u64 most, u64 bytes) {
  if (i >= n) return 0;
  u64 r = ls[i].cps > bytes ? 0 : bytes - ls[i].cps, c = 1;
  u64 left = most > 0 ? most - 1 : 0;
  for (u64 j = i + 1; j < n && left > 0; j += 1, left -= 1) {
    if (ls[j].cps > r) break;
    r -= ls[j].cps;
    c += 1;
  }
  return c;
}

static void ji_join(JcBuf* t, JiLine* ls, u64 i, u64 c) {
  for (u64 j = i; j < i + c; j += 1) {
    if (j > i) jc_byte(t, ',');
    jc_put(t, ls[j].p, ls[j].n);
  }
}

// one frame's text out as its WebSocket frame, appended to out
static int ji_put(JcBuf* t, const char* ks, u64 kn, const char* ws, u64 wn, JcBuf* out) {
  if (t->bad) return 0;
  JcBuf f = {0};
  int ok = jc_frame((const char*)t->p, t->n, ks, kn, ws, wn, &f, 0);
  if (ok) jc_put(out, f.p, f.n);
  free(f.p);
  t->n = 0;
  return ok && !out->bad;
}

static int ji_frames(const char* hd, u64 hn, const char* pl, u64 pn, const char* sp, u64 sn, JiLine* ls, u64 n,
                     const char* tl, u64 tn, int ends, u64 most, u64 bytes, const char* ks, u64 kn, const char* ws, u64 wn, JcBuf* out) {
  JcBuf t = {0};
  u64 c = ji_count(ls, 0, n, most, bytes);
  jc_put(&t, hd, hn);
  if (c >= n) jc_put(&t, pl, pn); else jc_put(&t, sp, sn);
  ji_join(&t, ls, 0, c);
  jc_put(&t, tl, tn);
  int ok = ji_put(&t, ks, kn, ws, wn, out);
  for (u64 i = c; ok && i < n; i += c) {
    c = ji_count(ls, i, n, most, bytes);
    const char* h = ends && i + c >= n ? "{\"t\":\"changes\",\"boot\":false,\"items\":[" : "{\"t\":\"changes\",\"boot\":true,\"items\":[";
    jc_put(&t, h, strlen(h));
    ji_join(&t, ls, i, c);
    jc_put(&t, "]}", 2);
    ok = ji_put(&t, ks, kn, ws, wn, out);
  }
  free(t.p);
  return ok;
}

// f[at..]: hd, plain, split, lines, tl, ends, most, bytes, keys, words
static int ji_run(Env e, Term* f, JcBuf* out) {
  u64 hn, pn, sn, tn, kn, wn, cap = 64, n = 0;
  char* hd = io_cstr(e, f[0], &hn);
  char* pl = io_cstr(e, f[1], &pn);
  char* sp = io_cstr(e, f[2], &sn);
  JiLine* ls = (JiLine*)io_mem(malloc(cap * sizeof(JiLine)));
  Term xs = f[3];
  while (term_aux(xs) == CID_CON) {
    Term fb[2];
    spare_free(e, cls_fit(2), ctr_take(e, xs, 2, fb));
    if (n >= cap) { cap *= 2; ls = (JiLine*)io_mem(realloc(ls, cap * sizeof(JiLine))); }
    ls[n].p = io_cstr(e, fb[0], &ls[n].n);
    ls[n].cps = ji_cps(ls[n].p, ls[n].n);
    n += 1;
    xs = fb[1];
  }
  char* tl = io_cstr(e, f[4], &tn);
  char* k = io_cstr(e, f[8], &kn);
  char* ws = io_cstr(e, f[9], &wn);
  int ok = ji_frames(hd, hn, pl, pn, sp, sn, ls, n, tl, tn, (u32)f[5] != 0, (u64)(u32)f[6], (u64)(u32)f[7], k, kn, ws, wn, out);
  for (u64 i = 0; i < n; i += 1) free(ls[i].p);
  free(ls); free(hd); free(pl); free(sp); free(tl); free(k); free(ws);
  return ok;
}

#endif

#ifdef CID(Json.cbor_frame)

Term json_cbor_frame_run(Env e, Term* f, IoWork* w) {
  u64 tn, kn, wn;
  char* t = io_cstr(e, f[0], &tn);
  char* k = io_cstr(e, f[1], &kn);
  char* ws = io_cstr(e, f[2], &wn);
  JcBuf out = {0};
  int ok = jc_frame(t, tn, k, kn, ws, wn, &out, 0);
  free(t); free(k); free(ws);
  Term r = ok ? host_bytes(e, out.p, out.n) : term_pak(CID_NIL, 0);
  free(out.p);
  return r;
}

static void __attribute__((constructor)) json_cbor_frame_use(void) {
  io_eff(CID(Json.cbor_frame), json_cbor_frame_run);
}

#endif

#if defined(CID(Sock.send_cbor)) || defined(CID(Sock.send_items))

// as sock_send_until_more: data holds the deadline and span, then the frame
static Term sock_send_cbor_more(Env e, IoWork* w) {
  int fd = (int)w->hand;
  u64 deadline, span;
  memcpy(&deadline, w->data, sizeof deadline);
  memcpy(&span, w->data + sizeof deadline, sizeof span);
  while (w->code == 0 && (u64)w->made < w->size) {
    ssize_t n = host_write(fd, w->data + w->made, w->size - (u64)w->made);
    if (n < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
      if (io_tick() >= deadline) { w->code = ETIMEDOUT; break; }
      return io_wait_on(w, fd, POLLOUT, deadline, sock_send_cbor_more);
    }
    w->made += io_sys_end(w, n);
    if (n > 0) {
      deadline = io_tick() + span;
      memcpy(w->data, &deadline, sizeof deadline);
    }
  }
  if (w->code != 0) shutdown(fd, SHUT_RDWR);
  Term r = w->code != 0 ? io_fail(e, w->code, NULL) : io_done(e, host_unit());
  free(w->data);
  return io_tup(e, io_hand(w->hand), r);
}

#endif

#ifdef CID(Sock.send_cbor)

Term sock_send_cbor_run(Env e, Term* f, IoWork* w) {
  u64 tn, kn, wn;
  char* t = io_cstr(e, f[1], &tn);
  char* k = io_cstr(e, f[2], &kn);
  char* ws = io_cstr(e, f[3], &wn);
  u64 span = (u64)f[4] * 1000000ull;
  u64 deadline = io_tick() + span;
  JcBuf out = {0};
  size_t pre = sizeof deadline + sizeof span;
  int ok = jc_frame(t, tn, k, kn, ws, wn, &out, pre);
  free(t); free(k); free(ws);
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->code = 0;
  if (!ok) return io_tup(e, io_hand(w->hand), io_fail(e, ENOMEM, NULL));
  memcpy(out.p, &deadline, sizeof deadline);
  memcpy(out.p + sizeof deadline, &span, sizeof span);
  w->data = io_mem(out.p);
  w->size = out.n;
  w->made = pre;
  return sock_send_cbor_more(e, w);
}

static void __attribute__((constructor)) sock_send_cbor_use(void) {
  io_eff(CID(Sock.send_cbor), sock_send_cbor_run);
}

#endif

#ifdef CID(Json.items_frames)

Term json_items_frames_run(Env e, Term* f, IoWork* w) {
  JcBuf out = {0};
  int ok = ji_run(e, f, &out);
  Term r = ok ? host_bytes(e, out.p, out.n) : term_pak(CID_NIL, 0);
  free(out.p);
  return r;
}

static void __attribute__((constructor)) json_items_frames_use(void) {
  io_eff(CID(Json.items_frames), json_items_frames_run);
}

#endif

#ifdef CID(Sock.send_items)

// as sock_send_cbor: data holds the deadline and span, then the frames
Term sock_send_items_run(Env e, Term* f, IoWork* w) {
  u64 span = (u64)f[11] * 1000000ull;
  u64 deadline = io_tick() + span;
  size_t pre = sizeof deadline + sizeof span;
  JcBuf out = {0};
  jc_room(&out, pre);
  out.n = pre;
  int ok = !out.bad && ji_run(e, f + 1, &out);
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->code = 0;
  if (!ok) { free(out.p); return io_tup(e, io_hand(w->hand), io_fail(e, ENOMEM, NULL)); }
  memcpy(out.p, &deadline, sizeof deadline);
  memcpy(out.p + sizeof deadline, &span, sizeof span);
  w->data = io_mem(out.p);
  w->size = out.n;
  w->made = pre;
  return sock_send_cbor_more(e, w);
}

static void __attribute__((constructor)) sock_send_items_use(void) {
  io_eff(CID(Sock.send_items), sock_send_items_run);
}

#endif
