// Run in Windows Cygwin. Checks the product's non-reaping status adapter
// against waitpid for Cygwin and plain Win32 children, including exit 15
// versus SIGTERM, fast children and a burst of simultaneous completions.
#define _GNU_SOURCE
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <signal.h>
#include <spawn.h>
#include <unistd.h>
#include <sys/wait.h>
#include "../../src/server/effects/windows-child.h"
extern char** environ;
static void check(int ok, const char* name) {
  if (!ok) { fprintf(stderr, "FAIL %s\n", name); exit(1); }
}
static pid_t start(int native, int code, int* gate, int fast) {
  int pipefd[2]; check(pipe(pipefd) == 0, "pipe");
  posix_spawn_file_actions_t fa; posix_spawn_file_actions_init(&fa);
  posix_spawn_file_actions_adddup2(&fa, pipefd[0], 0);
  posix_spawn_file_actions_addclose(&fa, pipefd[1]);
  char command[128]; snprintf(command, sizeof command,
      native ? (fast ? "exit /b %d" : "set /p gate= & exit /b %d")
             : (fast ? "exit %d" : "read gate; exit %d"), code);
  char* sh[] = {"/bin/sh", "-c", command, NULL};
  char* cmd[] = {"/cygdrive/c/Windows/System32/cmd.exe", "/d", "/c", command, NULL};
  char** args = native ? cmd : sh;
  unsigned serial = bp_child_begin(); pid_t pid;
  check(posix_spawn(&pid, args[0], &fa, NULL, args, environ) == 0, "spawn");
  bp_child_started(pid, serial);
  posix_spawn_file_actions_destroy(&fa); close(pipefd[0]); *gate = pipefd[1];
  return pid;
}
static unsigned observe(pid_t pid, const sigset_t* unblocked) {
  unsigned status;
  while ((status = bp_child_exited(pid)) == UINT32_MAX) sigsuspend(unblocked);
  return status;
}
int main(void) {
  alarm(120);
  sigset_t mask, old; sigemptyset(&mask); sigaddset(&mask, SIGCHLD);
  check(sigprocmask(SIG_BLOCK, &mask, &old) == 0, "mask");
  int codes[] = {0, 7, 15, 259};
  for (int native = 0; native < 2; native++) for (int mode = 0; mode < 5; mode++) {
    int gate; int wanted = mode == 4 ? 143 : codes[mode] & 255;
    pid_t pid = start(native, mode == 4 ? 0 : codes[mode], &gate, 0);
    check(bp_child_exited(pid) == UINT32_MAX, "running");
    if (mode == 4) check(kill(pid, SIGTERM) == 0, "signal own child");
    else check(write(gate, "go\n", 3) == 3, "release child");
    close(gate);
    unsigned seen = observe(pid, &old);
    check(seen == (unsigned)wanted, "observed status");
    check(bp_child_exited(pid) == seen, "second observation retains status");
    int status; check(waitpid(pid, &status, 0) == pid, "still waitable");
    check((WIFEXITED(status) ? WEXITSTATUS(status) : 128 + WTERMSIG(status)) == wanted, "wait agrees");
    check(bp_child_exited(pid) == 255, "reaped");
    printf("PASS native=%d mode=%d result=%d\n", native, mode, wanted);
  }
  enum { N = 48 }; pid_t pids[N]; int gates[N];
  for (int i = 0; i < N; i++) pids[i] = start(i % 2, i, &gates[i], 0);
  for (int i = 0; i < N; i++) { write(gates[i], "go\n", 3); close(gates[i]); }
  for (int i = 0; i < N; i++) {
    check(observe(pids[i], &old) == (unsigned)i, "burst observed");
    int status; check(waitpid(pids[i], &status, 0) == pids[i] && WIFEXITED(status) && WEXITSTATUS(status) == i, "burst wait");
  }
  for (int i = 0; i < 100; i++) {
    int gate; pid_t pid = start(i % 2, i % 31, &gate, 1); close(gate);
    check(observe(pid, &old) == (unsigned)(i % 31), "fast observed");
    int status; check(waitpid(pid, &status, 0) == pid && WEXITSTATUS(status) == i % 31, "fast wait");
  }
  puts("WINDOWS_CHILD_STATUS_PASS burst=48 fast=100");
}
