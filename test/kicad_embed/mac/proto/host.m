// Stand-in for Backplane's window. usage: host <scenario> <follower socket> <activation variant>
// It pushes its pane's rect (CG screen coords, top-left origin) to the follower over a Unix datagram socket on every
// move/resize (as the client would over the embed IPC), and tells it to hide when the window is minimised, full screen or
// on another Space. Scenario move: 1-5.5 s moves and resizes at 600 px/s; 5.8 s another app covers part of it; 7 s
//   reactivates; 9-12 s full screen and back; 13 s minimised, 14 s back; 15.5 s ends (or is killed by the script).
// Scenario still: stays put (the script drags its title bar, clicks the follower).
// Activation variants: none | below (on didBecomeKey order ourselves below the follower) | willbelow (on
//   applicationWillBecomeActive, earlier) | tell (on didBecomeActive tell the follower to order itself above us) | both (below + tell)
#import <Cocoa/Cocoa.h>
#include <sys/socket.h>
#include <sys/un.h>
static int sock = -1; static struct sockaddr_un to; static NSWindow *w; static NSString *act; static long fid;
static void say(NSString *s) { const char *c = s.UTF8String; sendto(sock, c, strlen(c), 0, (struct sockaddr *)&to, sizeof to); }
static void pushRect(void) {
  if (w.miniaturized || (w.styleMask & NSWindowStyleMaskFullScreen) || !w.isOnActiveSpace || !w.visible) { say(@"hide"); return; }
  double H = NSScreen.screens[0].frame.size.height; NSRect f = w.frame; // pane: 200 from left, 60 from top, to 20 from right/bottom
  say([NSString stringWithFormat:@"rect %.0f %.0f %.0f %.0f %ld", f.origin.x + 200, H - (f.origin.y + f.size.height) + 60, f.size.width - 220, f.size.height - 80, (long)w.windowNumber]);
}
@interface D : NSObject <NSWindowDelegate, NSApplicationDelegate> @end
@implementation D
- (void)windowDidMove:(NSNotification *)n { pushRect(); }
- (void)windowDidResize:(NSNotification *)n { pushRect(); }
- (void)windowDidMiniaturize:(NSNotification *)n { say(@"hide"); }
- (void)windowDidDeminiaturize:(NSNotification *)n { pushRect(); }
- (void)windowWillEnterFullScreen:(NSNotification *)n { say(@"hide"); }
- (void)windowDidExitFullScreen:(NSNotification *)n { pushRect(); }
- (void)windowDidChangeOcclusionState:(NSNotification *)n { pushRect(); }
- (void)fid { NSString *s = [NSString stringWithContentsOfFile:[@(to.sun_path) stringByAppendingString:@".id"] encoding:NSUTF8StringEncoding error:nil]; fid = s.integerValue; }
- (void)windowDidBecomeKey:(NSNotification *)n { [self fid];
  if (fid && ([act isEqual:@"below"] || [act isEqual:@"both"])) [w orderWindow:NSWindowBelow relativeTo:fid]; }
- (void)applicationWillBecomeActive:(NSNotification *)n { [self fid]; if (fid && [act isEqual:@"willbelow"]) [w orderWindow:NSWindowBelow relativeTo:fid]; }
- (void)applicationDidBecomeActive:(NSNotification *)n { if ([act isEqual:@"tell"] || [act isEqual:@"both"]) say(@"raise"); pushRect(); }
@end
int main(int argc, char **argv) { @autoreleasepool {
  NSString *sc = @(argv[1]); act = @(argv[3]); D *d = [D new];
  sock = socket(AF_UNIX, SOCK_DGRAM, 0); memset(&to, 0, sizeof to); to.sun_family = AF_UNIX; strncpy(to.sun_path, argv[2], sizeof to.sun_path - 1);
  [NSApplication sharedApplication]; [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular]; NSApp.delegate = d;
  w = [[NSWindow alloc] initWithContentRect:NSMakeRect(40, 150, 600, 400) styleMask:NSWindowStyleMaskTitled|NSWindowStyleMaskResizable|NSWindowStyleMaskMiniaturizable backing:NSBackingStoreBuffered defer:NO];
  w.title = @"lane-m host"; w.delegate = d; w.collectionBehavior = NSWindowCollectionBehaviorFullScreenPrimary;
  w.backgroundColor = NSColor.darkGrayColor; [w makeKeyAndOrderFront:nil]; [NSApp activateIgnoringOtherApps:YES];
  printf("%ld %d\n", (long)w.windowNumber, getpid()); fflush(stdout);
  NSDate *t0 = [NSDate date]; __block int done = 0; double v = 600;
  [NSTimer scheduledTimerWithTimeInterval:1.0/120 repeats:YES block:^(NSTimer *t) {
    double e = -[t0 timeIntervalSinceNow];
    if (e > 0.5 && !(done & 32)) { done |= 32; pushRect(); }
    if (e > 0.9 && !(done & 64)) { done |= 64; pushRect(); }  // again, in case the follower bound its socket late
    if (![sc isEqual:@"move"]) { if (e > 30) [NSApp terminate:nil]; return; }
    if (e > 1 && e < 5.5) { double m = e - 1, p = fmod(m * v, 600), x = 40 + (p < 300 ? p : 600 - p), g = m > 3 ? fmod((m - 3) * v/2, 150) : 0;
      [w setFrame:NSMakeRect(x, 150, 600 + g, 400 + g/2) display:YES]; }
    #define AT(s, bit, code) if (e > s && !(done & bit)) { done |= bit; code; printf("%s %.3f\n", #code, e); fflush(stdout); }
    AT(7, 1, ([NSApp activateIgnoringOtherApps:YES], [w makeKeyAndOrderFront:nil]))
    AT(9, 2, [w toggleFullScreen:nil])
    AT(12, 4, [w toggleFullScreen:nil])
    AT(13.5, 8, [w miniaturize:nil])
    AT(14.5, 16, [w deminiaturize:nil])
    if (e > 16) [NSApp terminate:nil];
  }];
  [NSApp run];
}}
