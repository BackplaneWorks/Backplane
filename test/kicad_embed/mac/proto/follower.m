// Stand-in for KiCad's frame in option (b): a borderless window kept over the host's pane.
// usage: follower <mode poll|push> <host CGWindowID> <host pid> <socket path> <policy accessory|regular|switch> <seconds>
//  poll: reads the host's bounds from CGWindowList at 120 Hz and keeps itself directly above the host (the first prototype)
//  push: only does what the host says over the socket ("rect x y w h hostid", "hide", "raise"); no polling
// Either way it watches the host's pid (kqueue) and hides and exits the moment the host is gone: never a stale window.
// policy: accessory (no Dock icon, no Cmd-Tab), regular, switch (accessory, regular while its window is key).
// It has a main menu ("KiCadStandIn" with File and Edit) to see what the menu bar shows when it has the keyboard.
#import <Cocoa/Cocoa.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/event.h>
static NSWindow *w; static long host; static double H; static dispatch_source_t readSource; // held: a local would be released at its block's end (ARC) and read nothing
static void place(CGRect p) { NSRect f = NSMakeRect(p.origin.x, H - (p.origin.y + p.size.height), p.size.width, p.size.height); if (!NSEqualRects(f, w.frame)) [w setFrame:f display:NO]; }
static void above(void) { if (w.visible) [w orderWindow:NSWindowAbove relativeTo:host]; }
@interface D : NSObject <NSWindowDelegate> @property NSString *policy; @end
@implementation D
- (void)windowDidBecomeKey:(NSNotification *)n { if ([self.policy isEqual:@"switch"]) [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular]; printf("key %.3f\n", NSDate.date.timeIntervalSince1970); fflush(stdout); }
- (void)windowDidResignKey:(NSNotification *)n { if ([self.policy isEqual:@"switch"]) [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory]; }
@end
@interface V : NSView @end
@implementation V - (void)mouseDown:(NSEvent *)e { printf("click %.3f active=%d key=%d\n", NSDate.date.timeIntervalSince1970, NSApp.active, self.window.keyWindow); fflush(stdout); } - (BOOL)acceptsFirstMouse:(NSEvent *)e { return YES; } @end
@interface KeyWin : NSWindow @end
@implementation KeyWin - (BOOL)canBecomeKeyWindow { return YES; } - (BOOL)canBecomeMainWindow { return YES; } @end
int main(int argc, char **argv) { @autoreleasepool {
  NSString *mode = @(argv[1]); host = atol(argv[2]); pid_t hpid = atoi(argv[3]); const char *path = argv[4]; NSString *policy = @(argv[5]); double secs = atof(argv[6]);
  [NSApplication sharedApplication]; D *d = [D new]; d.policy = policy;
  [NSApp setActivationPolicy:[policy isEqual:@"regular"] ? NSApplicationActivationPolicyRegular : NSApplicationActivationPolicyAccessory];
  NSMenu *bar = [NSMenu new]; for (NSString *t in @[@"KiCadStandIn", @"File", @"Edit"]) { NSMenuItem *i = [NSMenuItem new]; i.submenu = [[NSMenu alloc] initWithTitle:t]; [i.submenu addItemWithTitle:[t stringByAppendingString:@" item"] action:nil keyEquivalent:@""]; [bar addItem:i]; }
  NSApp.mainMenu = bar;
  H = NSScreen.screens[0].frame.size.height;
  w = [[KeyWin alloc] initWithContentRect:NSMakeRect(0,0,100,100) styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:NO];
  w.backgroundColor = NSColor.systemGreenColor; w.hasShadow = NO; w.delegate = d; w.contentView = [V new]; [w orderFront:nil];
  printf("%ld\n", (long)w.windowNumber); fflush(stdout);
  [[NSString stringWithFormat:@"%ld", (long)w.windowNumber] writeToFile:[@(path) stringByAppendingString:@".id"] atomically:YES encoding:NSUTF8StringEncoding error:nil];
  // the host's death: kqueue NOTE_EXIT on its pid, delivered on the main queue
  dispatch_source_t ds = dispatch_source_create(DISPATCH_SOURCE_TYPE_PROC, hpid, DISPATCH_PROC_EXIT, dispatch_get_main_queue());
  dispatch_source_set_event_handler(ds, ^{ [w orderOut:nil]; printf("host gone %.3f\n", NSDate.date.timeIntervalSince1970); fflush(stdout); exit(0); });
  dispatch_resume(ds);
  if ([mode isEqual:@"push"]) {
    int s = socket(AF_UNIX, SOCK_DGRAM, 0); struct sockaddr_un a = {0}; a.sun_family = AF_UNIX; strncpy(a.sun_path, path, sizeof a.sun_path - 1); unlink(path); a.sun_len = (unsigned char)SUN_LEN(&a);
    if (bind(s, (struct sockaddr *)&a, SUN_LEN(&a))) { perror("bind"); exit(2); }
    readSource = dispatch_source_create(DISPATCH_SOURCE_TYPE_READ, s, 0, dispatch_get_main_queue());
    dispatch_source_set_event_handler(readSource, ^{ char b[256]; ssize_t n; while ((n = recv(s, b, sizeof b - 1, MSG_DONTWAIT)) > 0) { b[n] = 0; static int got; if (got++ < 3) { fprintf(stderr, "got %s\n", b); }
      double x, y, ww, hh; long hid;
      if (sscanf(b, "rect %lf %lf %lf %lf %ld", &x, &y, &ww, &hh, &hid) == 5) { host = hid; place(CGRectMake(x, y, ww, hh)); if (!w.visible) [w orderWindow:NSWindowAbove relativeTo:host]; }
      else if (!strcmp(b, "hide")) [w orderOut:nil];
      else if (!strcmp(b, "raise")) above(); } });
    dispatch_resume(readSource);
  } else {
    [NSTimer scheduledTimerWithTimeInterval:1.0/120 repeats:YES block:^(NSTimer *t) {
      CFArrayRef a = CGWindowListCopyWindowInfo(kCGWindowListOptionIncludingWindow, (CGWindowID)host); CGRect b = CGRectNull;
      if (a && CFArrayGetCount(a)) CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)[((__bridge NSArray *)a)[0] objectForKey:(__bridge NSString *)kCGWindowBounds], &b);
      if (a) CFRelease(a);
      if (CGRectIsNull(b)) { [w orderOut:nil]; return; }
      place(CGRectMake(b.origin.x + 200, b.origin.y + 60, b.size.width - 220, b.size.height - 80));
      CFArrayRef o = CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenAboveWindow, (CGWindowID)host); BOOL ok = NO;
      if (o) { NSArray *l = (__bridge NSArray *)o; for (NSInteger i = l.count - 1; i >= 0; i--) { NSDictionary *dd = l[i]; if ([[dd objectForKey:(__bridge NSString *)kCGWindowLayer] intValue]) continue; ok = [[dd objectForKey:(__bridge NSString *)kCGWindowNumber] longValue] == w.windowNumber; break; } CFRelease(o); }
      if (!ok) above(); }];
  }
  [NSTimer scheduledTimerWithTimeInterval:secs repeats:NO block:^(NSTimer *t) { exit(0); }];
  [NSApp run];
}}
