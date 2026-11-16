// The Mac mini gate's helper, run in the logged-in user's session as a LaunchAgent (gate_agent.sh): it answers what the gate
// needs from the window server that no TCC grant guards, over a Unix socket, one JSON line per request line.
//   windows            on-screen windows front to back: number, pid, owner, layer, bounds (never titles: those need Screen Recording)
//   active PID         whether that app is the active one (NSRunningApplication: an accessory app is never frontmostApplication)
//   displays           the active displays: id, bounds in points, pixel size, main, and which ones this helper made
//   virtual N WxH      N virtual displays of W x H points (CGVirtualDisplay, a private API: test rig only, never the product);
//                      they live as long as this helper (or until `virtual 0`)
//   quit
// clang -fobjc-arc -framework Foundation -framework AppKit -framework CoreGraphics gate_helper.m -o gate_helper
//   ./gate_helper serve ~/bp-gate-m/helper.sock      (or one request: ./gate_helper windows)
#import <AppKit/AppKit.h>
#import <CoreGraphics/CoreGraphics.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <signal.h>
#include <unistd.h>

// ---- CGVirtualDisplay (CoreGraphics, private; the same declarations Chromium's test utility uses) ----
@interface CGVirtualDisplayDescriptor : NSObject
@property( retain, nonatomic ) dispatch_queue_t queue;
@property( retain, nonatomic ) NSString* name;
@property( nonatomic ) unsigned int maxPixelsHigh;
@property( nonatomic ) unsigned int maxPixelsWide;
@property( nonatomic ) CGSize sizeInMillimeters;
@property( nonatomic ) unsigned int serialNum;
@property( nonatomic ) unsigned int productID;
@property( nonatomic ) unsigned int vendorID;
@property( copy, nonatomic ) void ( ^terminationHandler )( id, id );
@end

@interface CGVirtualDisplayMode : NSObject
- (instancetype)initWithWidth:(unsigned int)width height:(unsigned int)height refreshRate:(double)refreshRate;
@end

@interface CGVirtualDisplaySettings : NSObject
@property( retain, nonatomic ) NSArray* modes;
@property( nonatomic ) unsigned int hiDPI;
@end

@interface CGVirtualDisplay : NSObject
- (instancetype)initWithDescriptor:(CGVirtualDisplayDescriptor*)descriptor;
- (BOOL)applySettings:(CGVirtualDisplaySettings*)settings;
@property( readonly, nonatomic ) unsigned int displayID;
@end

static NSMutableArray* s_virtual;

static id windows( void )
{
    NSMutableArray* out = [NSMutableArray array];
    CFArrayRef      list = CGWindowListCopyWindowInfo( kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
                                                       kCGNullWindowID );
    for( NSDictionary* w in (__bridge NSArray*) list )
    {
        CGRect r = CGRectZero;
        CGRectMakeWithDictionaryRepresentation( (__bridge CFDictionaryRef) w[(id) kCGWindowBounds], &r );
        [out addObject:@{ @"number": w[(id) kCGWindowNumber] ?: @0, @"pid": w[(id) kCGWindowOwnerPID] ?: @0,
                          @"owner": w[(id) kCGWindowOwnerName] ?: @"", @"layer": w[(id) kCGWindowLayer] ?: @0,
                          @"bounds": @[ @( r.origin.x ), @( r.origin.y ), @( r.size.width ), @( r.size.height ) ] }];
    }
    if( list )
        CFRelease( list );
    return out;
}

static id displays( void )
{
    CGDirectDisplayID ids[16];
    uint32_t          n = 0;
    CGGetActiveDisplayList( 16, ids, &n );
    NSMutableSet* mine = [NSMutableSet set];
    for( CGVirtualDisplay* d in s_virtual )
        [mine addObject:@( d.displayID )];
    NSMutableArray* out = [NSMutableArray array];
    for( uint32_t i = 0; i < n; i++ )
    {
        CGRect r = CGDisplayBounds( ids[i] );
        [out addObject:@{ @"id": @( ids[i] ), @"bounds": @[ @( r.origin.x ), @( r.origin.y ), @( r.size.width ), @( r.size.height ) ],
                          @"pixels": @[ @( CGDisplayPixelsWide( ids[i] ) ), @( CGDisplayPixelsHigh( ids[i] ) ) ],
                          @"main": @( CGDisplayIsMain( ids[i] ) != 0 ), @"builtin": @( CGDisplayIsBuiltin( ids[i] ) != 0 ),
                          @"ours": @( [mine containsObject:@( ids[i] )] ) }];
    }
    return out;
}

static id makeVirtual( int count, unsigned w, unsigned h )
{
    s_virtual = [NSMutableArray array];   // the old ones go with their last reference
    for( int i = 0; i < count; i++ )
    {
        CGVirtualDisplayDescriptor* desc = [[CGVirtualDisplayDescriptor alloc] init];
        desc.queue = dispatch_get_main_queue();
        desc.name = [NSString stringWithFormat:@"Backplane gate %d", i + 1];
        desc.maxPixelsWide = w * 2;
        desc.maxPixelsHigh = h * 2;
        desc.sizeInMillimeters = CGSizeMake( w * 0.25, h * 0.25 );
        desc.serialNum = 0x6a7e0 + i;
        desc.productID = 0x6a7e;
        desc.vendorID = 0xb9;
        desc.terminationHandler = ^( id a, id b ) {};
        CGVirtualDisplay* d = [[CGVirtualDisplay alloc] initWithDescriptor:desc];
        if( !d )
            return @{ @"error": [NSString stringWithFormat:@"CGVirtualDisplay %d was not created", i + 1] };
        CGVirtualDisplaySettings* s = [[CGVirtualDisplaySettings alloc] init];
        s.hiDPI = 0;
        s.modes = @[ [[CGVirtualDisplayMode alloc] initWithWidth:w height:h refreshRate:60] ];
        if( ![d applySettings:s] )
            return @{ @"error": [NSString stringWithFormat:@"display %d refused its mode %ux%u", i + 1, w, h] };
        [s_virtual addObject:d];
    }
    return @{ @"made": @( s_virtual.count ) };
}

static NSString* answer( NSString* line )
{
    NSArray* a = [line componentsSeparatedByCharactersInSet:NSCharacterSet.whitespaceCharacterSet];
    NSString* cmd = a.count ? a[0] : @"";
    id        out;
    if( [cmd isEqualToString:@"windows"] )
        out = windows();
    else if( [cmd isEqualToString:@"displays"] )
        out = displays();
    else if( [cmd isEqualToString:@"active"] && a.count > 1 )
        out = @{ @"active": @( [NSRunningApplication runningApplicationWithProcessIdentifier:[a[1] intValue]].isActive ) };
    else if( [cmd isEqualToString:@"virtual"] && a.count > 1 )
    {
        unsigned w = 1440, h = 900;
        if( a.count > 2 )
            sscanf( [a[2] UTF8String], "%ux%u", &w, &h );
        out = makeVirtual( [a[1] intValue], w, h );
    }
    else
        out = @{ @"error": [@"unknown request: " stringByAppendingString:cmd] };
    NSData* d = [NSJSONSerialization dataWithJSONObject:out options:0 error:nil];
    return [[NSString alloc] initWithData:d encoding:NSUTF8StringEncoding];
}

// every answer on the main queue: NSRunningApplication and the virtual displays belong to the main run loop
static NSString* onMain( NSString* line )
{
    __block NSString* r;
    dispatch_sync( dispatch_get_main_queue(), ^{ r = answer( line ); } );
    return r;
}

static void serve( const char* path )
{
    int s = socket( AF_UNIX, SOCK_STREAM, 0 );
    struct sockaddr_un a = { .sun_family = AF_UNIX };
    strlcpy( a.sun_path, path, sizeof a.sun_path );
    unlink( path );
    if( bind( s, (struct sockaddr*) &a, sizeof a ) || listen( s, 4 ) )
    {
        perror( "gate_helper: bind" );
        exit( 1 );
    }
    chmod( path, 0600 );
    for( ;; )
    {
        int c = accept( s, NULL, NULL );
        if( c < 0 )
            continue;
        FILE* f = fdopen( c, "r+" );
        char  buf[512];
        while( fgets( buf, sizeof buf, f ) )
        {
            NSString* line = [[NSString stringWithUTF8String:buf]
                    stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
            if( [line isEqualToString:@"quit"] )
            {
                unlink( path );
                exit( 0 );
            }
            fprintf( f, "%s\n", onMain( line ).UTF8String );
            fflush( f );
        }
        fclose( f );
    }
}

int main( int argc, const char** argv )
{
    @autoreleasepool
    {
        if( argc > 2 && !strcmp( argv[1], "serve" ) )
        {
            // a client gone before its answer is written: the write fails, the helper goes on (SIGPIPE ended it once)
            signal( SIGPIPE, SIG_IGN );
            NSString* path = [NSString stringWithUTF8String:argv[2]];
            dispatch_async( dispatch_get_global_queue( 0, 0 ), ^{ serve( path.UTF8String ); } );
            CFRunLoopRun();
            return 0;
        }
        NSMutableArray* words = [NSMutableArray array];
        for( int i = 1; i < argc; i++ )
            [words addObject:[NSString stringWithUTF8String:argv[i]]];
        printf( "%s\n", answer( [words componentsJoinedByString:@" "] ).UTF8String );
    }
    return 0;
}
