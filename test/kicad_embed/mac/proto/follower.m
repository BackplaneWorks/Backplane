// Stand-in for KiCad's frame in option (b): an accessory app's borderless window kept over the host's pane
// (inset from the host's content) by polling the host window's bounds (CGWindowList, no permission needed),
// ordered directly above the host window. usage: follower <host CGWindowID> <seconds> <mode: poll|link>
#import <Cocoa/Cocoa.h>
#import <QuartzCore/QuartzCore.h>
static CGRect hostBounds(CGWindowID id, int *onscreen) {
  CFArrayRef a = CGWindowListCopyWindowInfo(kCGWindowListOptionIncludingWindow, id); CGRect r = CGRectNull; *onscreen = 0;
  if (a && CFArrayGetCount(a)) { NSDictionary *d = ((__bridge NSArray*)a)[0]; CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)[d objectForKey:(__bridge NSString*)kCGWindowBounds], &r); *onscreen = [[d objectForKey:(__bridge NSString*)kCGWindowIsOnscreen] boolValue]; }
  if (a) CFRelease(a); return r;
}
int main(int argc, char **argv) { @autoreleasepool {
  CGWindowID host = (CGWindowID)atol(argv[1]); double secs = atof(argv[2]);
  [NSApplication sharedApplication]; [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
  NSWindow *w = [[NSWindow alloc] initWithContentRect:NSMakeRect(0,0,100,100) styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:NO];
  w.backgroundColor = NSColor.systemGreenColor; w.hasShadow = NO; [w orderFront:nil];
  printf("%ld\n", (long)w.windowNumber); fflush(stdout);
  double H = NSScreen.screens[0].frame.size.height; __block long reorders = 0;
  NSDate *t0 = [NSDate date];
  void (^tick)(void) = ^{
    int on; CGRect b = hostBounds(host, &on); if (CGRectIsNull(b)) return;
    // the pane: 200 px from the host's left, 60 from its top (title bar + a bar), to 20 px from the right and bottom
    CGRect p = CGRectMake(b.origin.x + 200, b.origin.y + 60, b.size.width - 220, b.size.height - 80);
    NSRect f = NSMakeRect(p.origin.x, H - (p.origin.y + p.size.height), p.size.width, p.size.height);
    if (!NSEqualRects(f, w.frame)) [w setFrame:f display:NO];
    // keep directly above the host: only reorder when something other than us sits right above it
    CFArrayRef a = CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenAboveWindow, host); BOOL ok = NO;
    if (a) { NSArray *l = (__bridge NSArray*)a; for (NSInteger i = l.count - 1; i >= 0; i--) { NSDictionary *d = l[i]; if ([[d objectForKey:(__bridge NSString*)kCGWindowLayer] intValue] != 0) continue; ok = [[d objectForKey:(__bridge NSString*)kCGWindowNumber] longValue] == w.windowNumber; break; } CFRelease(a); }
    if (!ok) { [w orderWindow:NSWindowAbove relativeTo:host]; reorders++; }
    if (-[t0 timeIntervalSinceNow] > secs) { printf("reorders %ld\n", reorders); fflush(stdout); exit(0); }
  };
  const char *mode = argc > 3 ? argv[3] : "poll";
  if (!strcmp(mode, "link")) { CADisplayLink *dl = [w.screen displayLinkWithTarget:[NSBlockOperation blockOperationWithBlock:tick] selector:@selector(main)]; [dl addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes]; }
  else [NSTimer scheduledTimerWithTimeInterval:1.0/120 repeats:YES block:^(NSTimer *t){ tick(); }];
  [NSApp run];
}}
