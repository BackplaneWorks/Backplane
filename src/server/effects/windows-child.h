// Cygwin has no waitid(WNOWAIT). Its SA_SIGINFO child notifications carry
// the POSIX wait status even for non-Cygwin executables. Observe those
// notifications without consuming the child: Proc.wait still uses waitpid.
// Cygwin queues every SA_SIGINFO notification (including SIGCHLD), rather
// than coalescing them. Nothing here sends signals or waits for children.
#include <sys/cygwin.h>
#include <stdatomic.h>

_Static_assert(ATOMIC_LLONG_LOCK_FREE == 2, "Windows child status needs lock-free atomics");
static _Atomic unsigned long long* bp_child_status;
static _Atomic unsigned* bp_child_birth;
static _Atomic unsigned bp_child_serial = 1;
static unsigned bp_child_limit;

static void bp_child_notice(int sig, siginfo_t* si, void* context) {
  (void)sig;
  (void)context;
  unsigned pid = (unsigned)si->si_pid;
  if (pid >= bp_child_limit ||
      (si->si_code != CLD_EXITED && si->si_code != CLD_KILLED && si->si_code != CLD_DUMPED)) return;
  unsigned seq = atomic_fetch_add(&bp_child_serial, 1) + 1;
  unsigned long long next = ((unsigned long long)seq << 32) | ((unsigned)si->si_status & 0xffffu);
  unsigned long long prev = atomic_load(&bp_child_status[pid]);
  while ((unsigned)(prev >> 32) < seq &&
         !atomic_compare_exchange_weak(&bp_child_status[pid], &prev, next)) {}
}

static void __attribute__((constructor)) bp_child_setup(void) {
  bp_child_limit = (unsigned)cygwin_internal(CW_MAX_CYGWIN_PID);
  bp_child_status = calloc(bp_child_limit, sizeof(*bp_child_status));
  bp_child_birth = calloc(bp_child_limit, sizeof(*bp_child_birth));
  struct sigaction old, action;
  memset(&action, 0, sizeof action);
  action.sa_sigaction = bp_child_notice;
  action.sa_flags = SA_SIGINFO | SA_NOCLDSTOP | SA_RESTART;
  sigemptyset(&action.sa_mask);
  if (!bp_child_limit || !bp_child_status || !bp_child_birth ||
      sigaction(SIGCHLD, NULL, &old) != 0 || old.sa_handler != SIG_DFL ||
      sigaction(SIGCHLD, &action, NULL) != 0) {
    fputs("backplane: cannot initialize Windows child status\n", stderr);
    abort();
  }
}

static unsigned bp_child_begin(void) {
  return atomic_load(&bp_child_serial);
}

static void bp_child_started(pid_t pid, unsigned serial) {
  if ((unsigned)pid < bp_child_limit) atomic_store(&bp_child_birth[pid], serial);
}

static uint32_t bp_child_exited(pid_t pid) {
  if (pid <= 1 || (unsigned)pid >= bp_child_limit ||
      !cygwin_internal(CW_CYGWIN_PID_TO_WINPID, pid)) return 255;
  unsigned born = atomic_load(&bp_child_birth[pid]);
  if (!born) return 255;
  unsigned long long seen = atomic_load(&bp_child_status[pid]);
  if ((unsigned)(seen >> 32) <= born) return UINT32_MAX;
  int status = (unsigned)seen & 0xffffu;
  return WIFEXITED(status) ? WEXITSTATUS(status) : 128u + WTERMSIG(status);
}
