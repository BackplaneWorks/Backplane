// A third process sampling both windows' bounds and their stacking every 2 ms (CGWindowList; no permission needed).
// Prints CSV: t_ms, host x y w h, follower x y w h, follower_directly_above_host(0/1)
#import <Cocoa/Cocoa.h>
int main(int argc, char **argv) { @autoreleasepool {
  long host = atol(argv[1]), fol = atol(argv[2]); double secs = atof(argv[3]);
  NSDate *t0 = [NSDate date];
  while (-[t0 timeIntervalSinceNow] < secs) {
    CFArrayRef a = CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, kCGNullWindowID);
    NSArray *l = (__bridge NSArray*)a; CGRect hb = CGRectNull, fb = CGRectNull; NSInteger hi = -1, fi = -1, k = 0;
    for (NSDictionary *d in l) { if ([[d objectForKey:(__bridge NSString*)kCGWindowLayer] intValue] != 0) continue; long n = [[d objectForKey:(__bridge NSString*)kCGWindowNumber] longValue];
      if (n == host) { CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)[d objectForKey:(__bridge NSString*)kCGWindowBounds], &hb); hi = k; }
      if (n == fol) { CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)[d objectForKey:(__bridge NSString*)kCGWindowBounds], &fb); fi = k; }
      k++; }
    CFRelease(a);
    printf("%.1f,%.0f,%.0f,%.0f,%.0f,%.0f,%.0f,%.0f,%.0f,%d\n", -[t0 timeIntervalSinceNow]*1000, hb.origin.x, hb.origin.y, hb.size.width, hb.size.height, fb.origin.x, fb.origin.y, fb.size.width, fb.size.height, (hi >= 0 && fi == hi - 1) ? 1 : 0);
    usleep(2000);
  }
}}
