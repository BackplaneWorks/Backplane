// Exercise the real lock helpers across exec on Windows/Cygwin or POSIX.
// The Bend-only effect registrations are skipped when CID is absent.
#define _GNU_SOURCE 1
#include "../../src/server/effects/hublock.c"
#include <spawn.h>
#include <sys/wait.h>

extern char** environ;

static void require(int ok, const char* message) {
  if (!ok) { fprintf(stderr, "FAIL %s: %s\n", message, strerror(errno)); exit(1); }
}

static void metadata(const char* path, const char* expected) {
  char out[128] = {0};
  int fd = open(path, O_RDONLY | O_CLOEXEC);
  require(fd >= 0, "open metadata separately");
  ssize_t n = read(fd, out, sizeof out - 1);
  close(fd);
  require(n == (ssize_t)strlen(expected) && !strcmp(out, expected), "metadata survives exec exactly");
}

static void contender(const char* self, const char* path) {
  pid_t child;
  char* args[] = {(char*)self, "contend", (char*)path, NULL};
  require(posix_spawn(&child, self, NULL, NULL, args, environ) == 0, "spawn lock contender");
  int status = 0;
  require(waitpid(child, &status, 0) == child && WIFEXITED(status) && WEXITSTATUS(status) == 0,
          "another process cannot acquire the lock");
}

int main(int argc, char** argv) {
  if (argc == 3 && !strcmp(argv[1], "contend")) {
    int fd = open(argv[2], O_RDWR | O_CLOEXEC);
    require(fd >= 0, "contender open");
    errno = 0;
    int taken = flock(fd, LOCK_EX | LOCK_NB);
    require(taken == -1 && (errno == EWOULDBLOCK || errno == EAGAIN), "exclusive lock remains held");
    close(fd);
    return 0;
  }
  char path[128];
  if (argc == 1) {
    strcpy(path, "/tmp/bp-windows-hublock-XXXXXX");
    bp_hub_fd = mkstemp(path);
    require(bp_hub_fd > 2, "create owned lock file");
    require(fcntl(bp_hub_fd, F_SETFD, FD_CLOEXEC) == 0, "initial close-on-exec");
    require(flock(bp_hub_fd, LOCK_EX | LOCK_NB) == 0, "initial lock");
  } else {
    require(argc == 3 && !strcmp(argv[1], "inherit"), "test arguments");
    require(strlen(argv[2]) < sizeof path, "path fits");
    strcpy(path, argv[2]);
    const char* inherited = getenv("BACKPLANE_HUB_LOCK_FD");
    require(inherited != NULL, "lock handover environment");
    bp_hub_fd = atoi(inherited);
    require(bp_hub_same(bp_hub_fd, path), "inherited descriptor identity");
    require((fcntl(bp_hub_fd, F_GETFD) & FD_CLOEXEC) != 0, "constructor restores close-on-exec");
    require(flock(bp_hub_fd, LOCK_EX | LOCK_NB) == 0, "take back inherited lock");
  }
  char line[100], expected[102];
  snprintf(line, sizeof line, "%ld 3998 flock", (long)getpid());
  snprintf(expected, sizeof expected, "%s\n", line);
  bp_hub_note(bp_hub_fd, line);
  metadata(path, expected);
  contender(argv[0], path);
  if (argc == 1) {
    bp_hublock_exec(1);
    char* args[] = {argv[0], "inherit", path, NULL};
    execv(argv[0], args);
    require(0, "exec handover");
  }
  close(bp_hub_fd);
  require(unlink(path) == 0, "remove own test file");
  puts("WINDOWS_HUBLOCK_EXEC_METADATA_AND_EXCLUSION_PASS");
  return 0;
}
