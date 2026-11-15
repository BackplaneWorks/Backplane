// Another app's window, put over part of the host and activated. usage: cover <x> <y> <seconds>
#import <Cocoa/Cocoa.h>
int main(int argc, char **argv) { @autoreleasepool {
  [NSApplication sharedApplication]; [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
  double H = NSScreen.screens[0].frame.size.height, x = atof(argv[1]), y = atof(argv[2]);
  NSWindow *w = [[NSWindow alloc] initWithContentRect:NSMakeRect(x, H - y - 200, 300, 200) styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
  w.title = @"lane-m cover"; w.backgroundColor = NSColor.systemRedColor; [w makeKeyAndOrderFront:nil]; [NSApp activateIgnoringOtherApps:YES];
  printf("%ld\n", (long)w.windowNumber); fflush(stdout);
  [NSTimer scheduledTimerWithTimeInterval:atof(argv[3]) repeats:NO block:^(NSTimer *t) { [NSApp terminate:nil]; }];
  [NSApp run];
}}
