// Stand-in for Backplane's window: a titled window that moves and resizes itself at constant speed,
// then reactivates itself (as a click on Backplane would). Prints its CGWindowID on stdout.
// usage: host <seconds> <speed px/s>
#import <Cocoa/Cocoa.h>
int main(int argc, char **argv) { @autoreleasepool {
  double secs = argc > 1 ? atof(argv[1]) : 6, v = argc > 2 ? atof(argv[2]) : 600;
  [NSApplication sharedApplication]; [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
  NSRect r = NSMakeRect(40, 150, 600, 400);
  NSWindow *w = [[NSWindow alloc] initWithContentRect:r styleMask:NSWindowStyleMaskTitled|NSWindowStyleMaskResizable backing:NSBackingStoreBuffered defer:NO];
  w.title = @"lane-m host"; w.backgroundColor = NSColor.darkGrayColor; [w makeKeyAndOrderFront:nil]; [NSApp activateIgnoringOtherApps:YES];
  printf("%ld\n", (long)w.windowNumber); fflush(stdout);
  NSDate *t0 = [NSDate date];
  [NSTimer scheduledTimerWithTimeInterval:1.0/120 repeats:YES block:^(NSTimer *t) {
    double e = -[t0 timeIntervalSinceNow];
    if (e < 1) return;                       // a second still, for the follower to settle
    double m = e - 1, p = fmod(m * v, 600);  // x sweeps 0..300..0
    double x = 40 + (p < 300 ? p : 600 - p);
    double grow = (m > secs/2) ? fmod((m - secs/2) * v/2, 150) : 0; // second half: resize too
    [w setFrame:NSMakeRect(x, 150, 600 + grow, 400 + grow/2) display:YES];
    if (fabs(m - secs*0.75) < 0.005) { [NSApp activateIgnoringOtherApps:YES]; [w orderFront:nil]; printf("reactivated %f\n", e); fflush(stdout); }
    if (m > secs) { [NSApp terminate:nil]; }
  }];
  [NSApp run];
}}
