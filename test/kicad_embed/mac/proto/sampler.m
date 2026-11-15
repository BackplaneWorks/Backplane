// A third process sampling the windows every 2 ms (CGWindowList; no permission needed).
// usage: sampler <host> <follower> <seconds> [cover-id file]
// CSV: t_ms, host x y w h, follower x y w h, follower directly above host (0/1), follower on screen, cover id, stacking front to back (H F C)
#import <Cocoa/Cocoa.h>
int main(int argc, char **argv) { @autoreleasepool {
  long host = atol(argv[1]), fol = atol(argv[2]); double secs = atof(argv[3]); NSString *cp = argc > 4 ? @(argv[4]) : nil; long cov = 0;
  NSDate *t0 = [NSDate date];
  while (-[t0 timeIntervalSinceNow] < secs) {
    if (cp && !cov) cov = [[NSString stringWithContentsOfFile:cp encoding:NSUTF8StringEncoding error:nil] integerValue];
    CFArrayRef a = CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, kCGNullWindowID);
    NSArray *l = (__bridge NSArray*)a; CGRect hb = CGRectNull, fb = CGRectNull; NSInteger hi = -1, fi = -1, k = 0; NSMutableString *order = [NSMutableString string];
    for (NSDictionary *d in l) { if ([[d objectForKey:(__bridge NSString*)kCGWindowLayer] intValue] != 0) continue; long n = [[d objectForKey:(__bridge NSString*)kCGWindowNumber] longValue];
      if (n == host) { CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)[d objectForKey:(__bridge NSString*)kCGWindowBounds], &hb); hi = k; [order appendString:@"H"]; }
      else if (n == fol) { CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)[d objectForKey:(__bridge NSString*)kCGWindowBounds], &fb); fi = k; [order appendString:@"F"]; }
      else if (cov && n == cov) [order appendString:@"C"];
      k++; }
    CFRelease(a);
    printf("%.1f,%.0f,%.0f,%.0f,%.0f,%.0f,%.0f,%.0f,%.0f,%d,%d,%ld,%s\n", -[t0 timeIntervalSinceNow]*1000, hb.origin.x, hb.origin.y, hb.size.width, hb.size.height, fb.origin.x, fb.origin.y, fb.size.width, fb.size.height, (hi >= 0 && fi == hi - 1) ? 1 : 0, fi >= 0, cov, order.UTF8String);
    fflush(stdout); usleep(2000);
  }
}}
