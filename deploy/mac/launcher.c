// Backplane.app's program (scripts/mac-app.sh builds it; deploy/install.sh.in puts the app in ~/Applications). It replaces itself (execv, same
// process) with the desktop client of the build the guard runs now, <prefix>/builds/$(cat <prefix>/CURRENT)/backplane-gpui, so the process macOS
// launched for the app is the client: the Dock, and the privacy prompts (Documents, Desktop, Downloads, the microphone), name Backplane, and the
// hub the client starts runs as its child, under the same permission. An update only changes CURRENT: the app is never rewritten.
// <prefix> is the first line of Contents/Resources/prefix when the installer wrote one (a BACKPLANE_PREFIX install), else ~/.local/share/backplane.
// CFProcessPath names this program, so the client's main bundle is still Backplane.app (its name, icon and usage descriptions).
#include <CoreFoundation/CoreFoundation.h>
#include <errno.h>
#include <limits.h>
#include <mach-o/dyld.h>
#include <pwd.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static void say(const char *msg) {
    fprintf(stderr, "Backplane: %s\n", msg);
    CFStringRef m = CFStringCreateWithCString(NULL, msg, kCFStringEncodingUTF8);
    if (m) { CFUserNotificationDisplayAlert(0, kCFUserNotificationStopAlertLevel, NULL, NULL, NULL, CFSTR("Backplane cannot start"), m, NULL, NULL, NULL, NULL); CFRelease(m); }
}

// the first line of a file, trimmed; 0 when there is none
static int first_line(const char *path, char *out, size_t n) {
    FILE *f = fopen(path, "r"); if (!f) return 0;
    if (!fgets(out, (int)n, f)) { fclose(f); return 0; }
    fclose(f);
    size_t l = strlen(out); while (l && (out[l - 1] == '\n' || out[l - 1] == '\r' || out[l - 1] == ' ' || out[l - 1] == '\t')) out[--l] = 0;
    return l > 0;
}

int main(int argc, char **argv) {
    char raw[PATH_MAX], me[PATH_MAX]; uint32_t sz = sizeof raw;
    if (_NSGetExecutablePath(raw, &sz) != 0 || !realpath(raw, me)) { say("cannot find its own program."); return 1; }
    // me = <app>/Contents/MacOS/Backplane
    char contents[PATH_MAX]; snprintf(contents, sizeof contents, "%s", me);
    for (int i = 0; i < 2; i++) { char *s = strrchr(contents, '/'); if (s) *s = 0; }
    char prefix[PATH_MAX], file[PATH_MAX + 32];
    snprintf(file, sizeof file, "%s/Resources/prefix", contents);
    if (!first_line(file, prefix, sizeof prefix)) {
        const char *home = getenv("HOME"); struct passwd *pw = getpwuid(getuid());
        if (!home || !*home) home = pw ? pw->pw_dir : NULL;
        if (!home) { say("cannot find your home folder."); return 1; }
        snprintf(prefix, sizeof prefix, "%s/.local/share/backplane", home);
    }
    char cur[256], msg[PATH_MAX * 2 + 200];
    snprintf(file, sizeof file, "%s/CURRENT", prefix);
    if (!first_line(file, cur, sizeof cur) || strchr(cur, '/') || cur[0] == '.') {
        snprintf(msg, sizeof msg, "No Backplane build is installed in %s (its CURRENT names none). Run the installer again.", prefix);
        say(msg); return 1;
    }
    char client[PATH_MAX + 512]; snprintf(client, sizeof client, "%s/builds/%s/backplane-gpui", prefix, cur);
    setenv("CFProcessPath", me, 1);
    argv[0] = client;
    execv(client, argv);
    snprintf(msg, sizeof msg, "Cannot run %s (%s). Run the installer again.", client, strerror(errno));
    say(msg); return 1;
}
