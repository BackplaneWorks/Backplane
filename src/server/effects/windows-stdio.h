// Win32 programs need ordinary pipe handles for stdin/stdout. Keep the
// hub's existing duplex Socket interface and bridge it to two pipes.
// One event-driven worker, bounded buffers, no child waits or signals.
#include <poll.h>
#include <pthread.h>

typedef struct { int socket, input, output; } BpStdio;

static int bp_stdio_again(void) {
  return errno == EINTR || errno == EAGAIN || errno == EWOULDBLOCK;
}

static void* bp_stdio_copy(void* arg) {
  BpStdio* b = arg;
  unsigned char input[16384], output[16384];
  size_t ni = 0, no = 0;
  int reading = 1, writing = 1;
  for (;;) {
    if (!reading && !ni && b->input >= 0) { close(b->input); b->input = -1; }
    if (b->output < 0 && !no && writing) { shutdown(b->socket, SHUT_WR); writing = 0; }
    if (b->input < 0 && !writing) break;
    struct pollfd fds[3] = {
      {b->socket, (reading && ni < sizeof input ? POLLIN : 0) | (no ? POLLOUT : 0), 0},
      {b->input, ni ? POLLOUT : 0, 0},
      {no < sizeof output ? b->output : -1, POLLIN, 0}
    };
    int ready = poll(fds, 3, -1);
    if (ready < 0) { if (errno == EINTR) continue; break; }
    if (fds[0].revents & (POLLHUP | POLLERR | POLLNVAL)) break;
    if (reading && (fds[0].revents & POLLIN)) {
      ssize_t n = recv(b->socket, input + ni, sizeof input - ni, 0);
      if (n > 0) ni += n;
      else if (!n) reading = 0;
      else if (!bp_stdio_again()) break;
    }
    if (fds[1].revents & (POLLERR | POLLHUP | POLLNVAL)) {
      close(b->input); b->input = -1; reading = 0; ni = 0;
    } else if (ni && (fds[1].revents & POLLOUT)) {
      ssize_t n = write(b->input, input, ni);
      if (n > 0) { ni -= n; memmove(input, input + n, ni); }
      else if (n < 0 && !bp_stdio_again()) { close(b->input); b->input = -1; reading = 0; ni = 0; }
    }
    if (fds[2].revents & (POLLIN | POLLHUP | POLLERR | POLLNVAL)) {
      ssize_t n = read(b->output, output + no, sizeof output - no);
      if (n > 0) no += n;
      else if (!n || !bp_stdio_again()) { close(b->output); b->output = -1; }
    }
    if (no && (fds[0].revents & POLLOUT)) {
      ssize_t n = send(b->socket, output, no, MSG_NOSIGNAL);
      if (n > 0) { no -= n; memmove(output, output + n, no); }
      else if (n < 0 && !bp_stdio_again()) break;
    }
  }
  if (b->input >= 0) close(b->input);
  if (b->output >= 0) close(b->output);
  close(b->socket);
  free(b);
  return NULL;
}

// On success the worker owns sv[1] and the parent's pipe ends; the caller
// owns sv[0] and child[0:2]. On failure everything opened here is closed.
static int bp_stdio_pair(int sv[2], int child[2]) {
  int input[2] = {-1, -1}, output[2] = {-1, -1};
  BpStdio* b = NULL;
  sv[0] = sv[1] = -1;
  if (socketpair(AF_UNIX, SOCK_STREAM, 0, sv) < 0 ||
      pipe(input) < 0 || pipe(output) < 0) goto fail;
  int fds[] = {sv[0], sv[1], input[0], input[1], output[0], output[1]};
  for (unsigned i = 0; i < sizeof fds / sizeof *fds; i++)
    if (fcntl(fds[i], F_SETFD, FD_CLOEXEC) < 0) goto fail;
  int nonblock[] = {sv[1], input[1], output[0]};
  for (unsigned i = 0; i < sizeof nonblock / sizeof *nonblock; i++) {
    int flags = fcntl(nonblock[i], F_GETFL);
    if (flags < 0 || fcntl(nonblock[i], F_SETFL, flags | O_NONBLOCK) < 0) goto fail;
  }
  b = malloc(sizeof *b);
  if (!b) goto fail;
  *b = (BpStdio){sv[1], input[1], output[0]};
  pthread_t worker;
  int error = pthread_create(&worker, NULL, bp_stdio_copy, b);
  if (error) { errno = error; goto fail; }
  pthread_detach(worker);
  child[0] = input[0]; child[1] = output[1];
  return 0;
fail:;
  int saved = errno;
  free(b);
  if (sv[0] >= 0) close(sv[0]);
  if (sv[1] >= 0) close(sv[1]);
  for (unsigned i = 0; i < 2; i++) {
    if (input[i] >= 0) close(input[i]);
    if (output[i] >= 0) close(output[i]);
  }
  errno = saved;
  return -1;
}
