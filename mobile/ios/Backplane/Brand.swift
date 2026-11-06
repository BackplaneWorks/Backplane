import SwiftUI

// The Backplane mark, drawn as vector paths from assets/icon/backplane.svg
// (the web's favicon.svg and favicon-light.svg are the same paths): never a
// raster copy, so it is crisp at every scale. Dark: the mark #e6e9de on
// #111410 (the web's apple-touch-icon); light: #000000 on #ffffff
// (src/web/favicon-light.svg, the #922 light-mode fix; scripts/icons.sh).
// The web draws it 24 x 24 in the sidebar and phone headers.
enum BrandArt {
    static let viewBox: CGFloat = 256
    static let tx: CGFloat = 26.950
    static let ty: CGFloat = 33.004
    static let scale: CGFloat = 0.8600
    static let path: Path = catPath("M130.50000,32.69817L91.68666,44.82734L91.50000,44.67666L91.50000,44.88567L52.85411,56.96251L52.50000,56.67666L52.50000,57.07317L28.50000,64.57317L28.50000,169.23933L66.84865,157.25538L73.39830,162.54245L21.50000,178.76067L21.50000,59.42683L156.50000,17.23933L156.50000,53.66461L149.50000,48.01401L149.50000,26.76067L130.51921,32.69217L130.50000,32.67666ZM213.50000,179.67666L207.70652,175.00000L209.00000,175.00000L209.00000,109.00000L205.00000,109.00000L205.00000,172.81522L181.50000,153.84534L181.50000,117.32727L137.50000,81.80919L137.50000,47.32334L213.50000,108.67273ZM98.50000,59.32334L174.50000,120.67273L174.50000,191.67666L168.70652,187.00000L170.00000,187.00000L170.00000,121.00000L166.00000,121.00000L166.00000,184.81522L142.50000,165.84534L142.50000,129.32727L98.50000,93.80919ZM59.50000,71.32334L135.50000,132.67273L135.50000,203.67666L129.70652,199.00000L131.00000,199.00000L131.00000,133.00000L127.00000,133.00000L127.00000,196.81522L59.50000,142.32727Z")
}

struct BrandMark: View {
    var size: CGFloat = 24
    // the web's .conn square on the mark's corner (style.css .brand-mark .conn): nil draws none
    var conn: Bool? = nil
    @Environment(\.colorScheme) private var scheme

    private func rgb(_ v: Int) -> Color {
        Color(red: Double((v >> 16) & 255) / 255, green: Double((v >> 8) & 255) / 255, blue: Double(v & 255) / 255)
    }

    var body: some View {
        let dark = scheme == .dark
        let bg = dark ? Color(red: 0x11 / 255, green: 0x14 / 255, blue: 0x10 / 255) : Color.white
        let fg = dark ? Color(red: 0xe6 / 255, green: 0xe9 / 255, blue: 0xde / 255) : Color.black
        Canvas { ctx, sz in
            let s = min(sz.width, sz.height) / BrandArt.viewBox
            ctx.fill(Path(CGRect(x: 0, y: 0, width: sz.width, height: sz.height)), with: .color(bg))
            var c = ctx
            c.scaleBy(x: s, y: s)
            c.translateBy(x: BrandArt.tx, y: BrandArt.ty)
            c.scaleBy(x: BrandArt.scale, y: BrandArt.scale)
            c.fill(BrandArt.path, with: .color(fg), style: FillStyle(eoFill: true))
        }
        .frame(width: size, height: size)
        .overlay(alignment: .bottomTrailing) {
            if let on = conn {
                // 7 px square, a 1 px ring in the panel colour and a 1 px halo; right -2, bottom -1
                Rectangle().fill(rgb(on ? (dark ? 0x58c28a : 0x1c8550) : (dark ? 0xf06a6a : 0xc93434)))
                    .frame(width: 7, height: 7)
                    .padding(2)
                    .background(rgb(dark ? 0x13161b : 0xf1f0ec))
                    .offset(x: 3, y: 2)
            }
        }
        .geomLog("brand")
        .accessibilityHidden(true)
    }
}

#if DEBUG
// BACKPLANE_GEOM=1: where a logo or cat is drawn, in points, once it appears (the
// visual check, test/visual, cuts these rectangles out of a screenshot)
private let geomOn = ProcessInfo.processInfo.environment["BACKPLANE_GEOM"] == "1"
extension View {
    func geomLog(_ name: String) -> some View {
        background(GeometryReader { g in
            Color.clear.onAppear {
                guard geomOn else { return }
                let f = g.frame(in: .global)
                NSLog("Backplane geom %@ %.1f %.1f %.1f %.1f", name, f.minX, f.minY, f.width, f.height)
            }
        })
    }
}
#else
extension View { func geomLog(_ name: String) -> some View { self } }
#endif
