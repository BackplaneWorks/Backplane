// Stand-in for Backplane's window. usage: host <scenario> <follower-id file> [counter]
//  move:   1-4 s moves at 600 px/s, 4-5.5 s also resizes, 7 s reactivates (a click on Backplane), 9-12 s full screen and back
//  still:  stays put (the runner script drags its title bar with real mouse input, and puts another app over it)
// counter: when it becomes key, it orders itself just below the follower (our own window, so no wait on the other process)
#import <Cocoa/Cocoa.h>
@interface D : NSObject <NSWindowDelegate> @property NSString *fidPath; @property BOOL counter; @end
@implementation D
- (void)windowDidBecomeKey:(NSNotification *)n { if (!self.counter) return; NSWindow *w = n.object;
  long fid = [[NSString stringWithContentsOfFile:self.fidPath encoding:NSUTF8StringEncoding error:nil] integerValue];
  if (fid > 0) [w orderWindow:NSWindowBelow relativeTo:fid]; }
@end
int main(int argc, char **argv) { @autoreleasepool {
  NSString *sc = @(argv[1]); D *d = [D new]; d.fidPath = @(argv[2]); d.counter = argc > 3 && !strcmp(argv[3], "counter");
  [NSApplication sharedApplication]; [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
  NSWindow *w = [[NSWindow alloc] initWithContentRect:NSMakeRect(40, 150, 600, 400) styleMask:NSWindowStyleMaskTitled|NSWindowStyleMaskResizable|NSWindowStyleMaskMiniaturizable backing:NSBackingStoreBuffered defer:NO];
  w.title = @"lane-m host"; w.delegate = d; w.collectionBehavior = NSWindowCollectionBehaviorFullScreenPrimary;
  w.backgroundColor = NSColor.darkGrayColor; [w makeKeyAndOrderFront:nil]; [NSApp activateIgnoringOtherApps:YES];
  printf("%ld\n", (long)w.windowNumber); fflush(stdout);
  NSDate *t0 = [NSDate date]; __block int done = 0; double v = 600;
  [NSTimer scheduledTimerWithTimeInterval:1.0/120 repeats:YES block:^(NSTimer *t) {
    double e = -[t0 timeIntervalSinceNow];
    if ([sc isEqual:@"move"]) {
      if (e > 1 && e < 5.5) { double m = e - 1, p = fmod(m * v, 600), x = 40 + (p < 300 ? p : 600 - p), g = m > 3 ? fmod((m - 3) * v/2, 150) : 0;
        [w setFrame:NSMakeRect(x, 150, 600 + g, 400 + g/2) display:YES]; }
      if (e > 7 && !(done & 1)) { done |= 1; [NSApp activateIgnoringOtherApps:YES]; [w makeKeyAndOrderFront:nil]; printf("reactivated %.3f\n", e); fflush(stdout); }
      if (e > 9 && !(done & 2)) { done |= 2; [w toggleFullScreen:nil]; printf("fullscreen on %.3f\n", e); fflush(stdout); }
      if (e > 12 && !(done & 4)) { done |= 4; [w toggleFullScreen:nil]; printf("fullscreen off %.3f\n", e); fflush(stdout); }
      if (e > 14.5) [NSApp terminate:nil];
    } else if (e > 20) [NSApp terminate:nil];
  }];
  [NSApp run];
}}
