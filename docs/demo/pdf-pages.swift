// Renders a PDF's pages to PNG frames for a demo video (macOS, PDFKit): two pages side by side on
// a grey 2880×1800 canvas, so a `pictures` line in a ui_demo script can show what was printed.
// Usage: swift docs/demo/pdf-pages.swift in.pdf out-dir/ [first-page] [last-page]
import AppKit
import PDFKit

let args = CommandLine.arguments
guard args.count >= 3, let doc = PDFDocument(url: URL(fileURLWithPath: args[1])) else {
    FileHandle.standardError.write("usage: pdf-pages.swift in.pdf out-dir/ [first] [last]\n".data(using: .utf8)!)
    exit(1)
}
let out = URL(fileURLWithPath: args[2], isDirectory: true)
try? FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
let first = max(1, args.count > 3 ? Int(args[3]) ?? 1 : 1)
let last = min(doc.pageCount, args.count > 4 ? Int(args[4]) ?? doc.pageCount : doc.pageCount)
let canvas = NSSize(width: 2880, height: 1800)

func frame(_ pages: [PDFPage], _ n: Int) {
    guard let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(canvas.width), pixelsHigh: Int(canvas.height),
                                     bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                                     colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0),
          let gctx = NSGraphicsContext(bitmapImageRep: rep) else { return }
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = gctx
    NSColor(calibratedWhite: 0.36, alpha: 1).setFill()
    NSRect(origin: .zero, size: canvas).fill()
    let gap: CGFloat = 80
    let h = canvas.height - 2 * gap
    var x = gap
    let widths = pages.map { p -> CGFloat in
        let b = p.bounds(for: .mediaBox)
        return b.width * h / b.height
    }
    x = (canvas.width - widths.reduce(0, +) - gap * CGFloat(pages.count - 1)) / 2
    for (p, w) in zip(pages, widths) {
        let r = NSRect(x: x, y: gap, width: w, height: h)
        NSColor.white.setFill()
        r.fill()
        if let ctx = NSGraphicsContext.current?.cgContext {
            ctx.saveGState()
            let b = p.bounds(for: .mediaBox)
            ctx.translateBy(x: r.minX, y: r.minY)
            ctx.scaleBy(x: w / b.width, y: h / b.height)
            p.draw(with: .mediaBox, to: ctx)
            ctx.restoreGState()
        }
        x += w + gap
    }
    NSGraphicsContext.restoreGraphicsState()
    if let png = rep.representation(using: .png, properties: [:]) {
        try? png.write(to: out.appendingPathComponent(String(format: "p%03d.png", n)))
    }
}

var n = 0
var i = first
while i <= last {
    let pages = [i, i + 1].filter { $0 <= last }.compactMap { doc.page(at: $0 - 1) }
    frame(pages, n)
    n += 1
    i += 2
}
print("rendered \(n) frames from \(args[1])")
