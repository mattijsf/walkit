// SPDX-License-Identifier: GPL-3.0-or-later

import CoreGraphics
import Foundation

/// A loaded skin: the engine behind the C interface in `rust/include/walkit_macos.h`. It runs
/// on the main thread, as every call into the engine does.
@MainActor
final class Skin {
    private let handle: OpaquePointer
    /// Numbers each press, as the engine tells touches apart.
    private var touch: Int32 = 0

    /// The skin's file name, shown as the window's title.
    let name: String

    struct OpenError: LocalizedError {
        let message: String
        var errorDescription: String? { message }
    }

    /// Opens the skin archive `bytes`, named `name`. A classic skin lies over `base`, a modern
    /// one over `wasabi`; the stand-in fonts come from the folder `fonts`.
    init(bytes: Data, name: String, base: Data?, wasabi: Data?, fonts: URL?) throws {
        var error: UnsafeMutablePointer<CChar>?
        let handle = bytes.withPointer { skin, skinLen in
            base.withPointer { base, baseLen in
                wasabi.withPointer { wasabi, wasabiLen in
                    walkit_skin_open(
                        skin, skinLen, name, base, baseLen, wasabi, wasabiLen,
                        fonts?.path, &error)
                }
            }
        }
        guard let handle else {
            let message = error.map { String(cString: $0) } ?? "the skin does not open"
            walkit_string_free(error)
            throw OpenError(message: message)
        }
        self.handle = handle
        self.name = name
    }

    isolated deinit {
        walkit_skin_free(handle)
    }

    /// Runs the engine's timers and animations; true when the skin wants a new frame.
    func tick() -> Bool {
        walkit_skin_tick(handle)
    }

    /// The size the skin's windows take, in layout pixels: points on the screen.
    var size: CGSize {
        var w: Int32 = 0
        var h: Int32 = 0
        walkit_skin_size(handle, &w, &h)
        return CGSize(width: Int(w), height: Int(h))
    }

    /// Draws a frame of the skin's size at `scale` device pixels per point.
    func draw(scale: Int) -> CGImage? {
        let size = size
        let w = Int(size.width) * scale
        let h = Int(size.height) * scale
        guard let pixels = walkit_skin_draw(handle, Int32(w), Int32(h), Int32(scale)) else {
            return nil
        }
        // each pixel is 0x00RRGGBB, stored little end first: B, G, R, then the unused byte
        let data = Data(bytes: pixels, count: w * h * 4) as CFData
        guard let provider = CGDataProvider(data: data) else { return nil }
        return CGImage(
            width: w, height: h, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: w * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGBitmapInfo(
                rawValue: CGBitmapInfo.byteOrder32Little.rawValue
                    | CGImageAlphaInfo.noneSkipFirst.rawValue),
            provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
    }

    func pointerDown(_ x: Int32, _ y: Int32) {
        touch += 1
        walkit_skin_pointer_down(handle, x, y, touch)
    }

    func pointerMove(_ x: Int32, _ y: Int32) {
        walkit_skin_pointer_move(handle, x, y)
    }

    func pointerUp(_ x: Int32, _ y: Int32) {
        walkit_skin_pointer_up(handle, x, y)
    }

    func doubleClick(_ x: Int32, _ y: Int32) {
        walkit_skin_double_click(handle, x, y, touch)
    }
}

extension Optional where Wrapped == Data {
    /// Calls `body` with the bytes and their count, or null and 0 for no data.
    fileprivate func withPointer<T>(_ body: (UnsafePointer<UInt8>?, Int) -> T) -> T {
        switch self {
        case .some(let data): return data.withPointer(body)
        case .none: return body(nil, 0)
        }
    }
}

extension Data {
    fileprivate func withPointer<T>(_ body: (UnsafePointer<UInt8>?, Int) -> T) -> T {
        withUnsafeBytes { raw in
            body(raw.bindMemory(to: UInt8.self).baseAddress, raw.count)
        }
    }
}
