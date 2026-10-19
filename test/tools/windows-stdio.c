// Compile/run in Windows Cygwin. Exercise the actual product byte bridge
// with a plain Win32 command and a native PowerShell binary echo larger
// than its buffers; neither child uses the Cygwin DLL.
#define _GNU_SOURCE
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <signal.h>
#include <spawn.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/socket.h>
#include <sys/wait.h>
#ifndef MSG_NOSIGNAL
#define MSG_NOSIGNAL 0
#endif
#include "../../src/server/effects/windows-stdio.h"
extern char** environ;
static void check(int ok, const char* what) {
  if (!ok) { fprintf(stderr, "FAIL %s errno=%d\n", what, errno); exit(1); }
}
static pid_t start(char** args, int* socket) {
  int sv[2], child[2]; check(bp_stdio_pair(sv, child) == 0, "pair");
  posix_spawn_file_actions_t fa; posix_spawn_file_actions_init(&fa);
  posix_spawn_file_actions_adddup2(&fa, child[0], 0);
  posix_spawn_file_actions_adddup2(&fa, child[1], 1);
  posix_spawn_file_actions_adddup2(&fa, child[1], 2);
  posix_spawn_file_actions_addclose(&fa, sv[0]);
  posix_spawn_file_actions_addclose(&fa, sv[1]);
  posix_spawn_file_actions_addclose(&fa, child[0]);
  posix_spawn_file_actions_addclose(&fa, child[1]);
  pid_t pid; int rc = posix_spawn(&pid, args[0], &fa, NULL, args, environ);
  posix_spawn_file_actions_destroy(&fa); close(child[0]); close(child[1]);
  check(rc == 0, "spawn"); *socket = sv[0]; return pid;
}
static unsigned char payload[524288];
static void* writer(void* arg) {
  int fd = *(int*)arg; size_t at = 0;
  while (at < sizeof payload) {
    ssize_t n = send(fd, payload + at, sizeof payload - at, MSG_NOSIGNAL);
    if (n < 0 && errno == EINTR) continue;
    check(n > 0, "send"); at += n;
  }
  check(shutdown(fd, SHUT_WR) == 0, "input EOF"); return NULL;
}
int main(void) {
  alarm(90); signal(SIGPIPE, SIG_IGN);
  char* cmd[] = {"/cygdrive/c/Windows/System32/cmd.exe", "/d", "/c", "echo WINDOWS_NATIVE_PIPE_OK & exit /b 7", NULL};
  int fd; pid_t pid = start(cmd, &fd); char text[1024]; size_t at = 0;
  for (;;) { ssize_t n = recv(fd, text + at, sizeof text - 1 - at, 0); check(n >= 0, "cmd read"); if (!n) break; at += n; }
  text[at] = 0; check(strstr(text, "WINDOWS_NATIVE_PIPE_OK") != NULL, "Win32 stdout"); close(fd);
  int status; check(waitpid(pid, &status, 0) == pid && WIFEXITED(status) && WEXITSTATUS(status) == 7, "cmd status");
  puts("WINDOWS_CMD_STDIO_PASS");
  char* ps[] = {"/cygdrive/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe", "-NoProfile", "-NonInteractive", "-Command",
    "$i=[Console]::OpenStandardInput();$o=[Console]::OpenStandardOutput();$b=New-Object byte[] 8192;while(($n=$i.Read($b,0,$b.Length)) -gt 0){$o.Write($b,0,$n)};$o.Flush();exit 7", NULL};
  for (size_t i = 0; i < sizeof payload; i++) payload[i] = i % 251;
  pid = start(ps, &fd); pthread_t thread; check(pthread_create(&thread, NULL, writer, &fd) == 0, "writer");
  unsigned char bytes[8192]; at = 0;
  for (;;) {
    ssize_t n = recv(fd, bytes, sizeof bytes, 0); if (n < 0 && errno == EINTR) continue;
    check(n >= 0, "PowerShell read"); if (!n) break;
    check(at + n <= sizeof payload && memcmp(bytes, payload + at, n) == 0, "binary bytes"); at += n;
  }
  pthread_join(thread, NULL); close(fd);
  check(at == sizeof payload, "all bytes");
  check(waitpid(pid, &status, 0) == pid && WIFEXITED(status) && WEXITSTATUS(status) == 7, "PowerShell status");
  puts("WINDOWS_NATIVE_STDIO_PASS binary=524288 duplex=1 EOF=1");
}
