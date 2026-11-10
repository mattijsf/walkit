// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import SwiftUI

/// Runs a skin and shows it: a tick on every display frame, a new frame when the skin asks for
/// one, and the mouse passed back to the engine. The skin's windows are set out apart over the
/// whole view, and each drags anywhere in it by its title bar, as on Winamp's desktop. The view
/// zooms in whole steps: one layout pixel is one point, two once the view is twice the size the
/// skin's windows took when it opened, and so on.
struct SkinView: NSViewRepresentable {
    let skin: Skin?
    /// The size the skin's windows took as one stack, in layout pixels: the view is at least
    /// this big.
    @Binding var size: CGSize

    func makeNSView(context: Context) -> SkinNSView {
        let view = SkinNSView()
        view.onResize = { size = $0 }
        return view
    }

    func updateNSView(_ view: SkinNSView, context: Context) {
        view.onResize = { size = $0 }
        view.skin = skin
    }
}

final class SkinNSView: NSView {
    var skin: Skin? {
        didSet {
            guard skin !== oldValue else { return }
            dirty = true
            pressed = false
        }
    }
    var onResize: ((CGSize) -> Void)?

    private var dirty = true
    /// The frame shown.
    private var image: CGImage?
    private var pressed = false
    private var shownSize = CGSize.zero
    private var link: CADisplayLink?
    /// Points per layout pixel, and the view's size in layout pixels: the space the windows move
    /// in.
    private var zoom = 1
    private var viewport = (width: 1, height: 1)
    /// The layer the frame shows in.
    private let skinLayer = CALayer()

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.backgroundColor = NSColor(white: 0.125, alpha: 1).cgColor
        skinLayer.contentsGravity = .resize
        skinLayer.magnificationFilter = .nearest
        layer?.addSublayer(skinLayer)
    }

    required init?(coder: NSCoder) { fatalError("not from a nib") }

    override var isFlipped: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    // a window of the skin moves with the mouse, not the app's window
    override var mouseDownCanMoveWindow: Bool { false }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        link?.invalidate()
        link = nil
        guard window != nil else { return }
        let link = displayLink(target: self, selector: #selector(frame(_:)))
        link.add(to: .main, forMode: .common)
        self.link = link
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        dirty = true
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        dirty = true
    }

    /// The largest whole zoom at which the skin's windows fit the view, and the view's size in
    /// layout pixels at it.
    private func place(_ size: CGSize) {
        guard size.width > 0, size.height > 0 else { return }
        let fit = min(bounds.width / size.width, bounds.height / size.height)
        zoom = max(1, Int(fit.rounded(.down)))
        viewport = (
            max(1, Int(bounds.width) / zoom),
            max(1, Int(bounds.height) / zoom)
        )
    }

    /// Once per display frame: the engine runs its timers and animations, and the view draws
    /// again when something changed.
    @objc private func frame(_ link: CADisplayLink) {
        guard let skin else {
            image = nil
            skinLayer.contents = nil
            return
        }
        let changed = skin.tick()
        guard changed || dirty else { return }
        dirty = false
        let size = skin.size
        place(size)
        skin.setView(width: viewport.width, height: viewport.height)
        // device pixels per layout pixel: the zoom on a Retina screen's two per point
        let backing = max(1, Int((window?.backingScaleFactor ?? 2).rounded()))
        let shown = CGSize(width: viewport.width * zoom, height: viewport.height * zoom)
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        image = skin.draw(width: viewport.width, height: viewport.height, scale: zoom * backing)
        skinLayer.contents = image
        skinLayer.contentsScale = CGFloat(backing)
        // the view is flipped, its layer is not: y counts from the bottom here
        skinLayer.frame = CGRect(
            x: 0, y: bounds.height - shown.height, width: shown.width, height: shown.height)
        CATransaction.commit()
        if size != shownSize {
            shownSize = size
            onResize?(size)
        }
    }

    /// Draws the frame shown when the view is drawn into an image ([`NSView.cacheDisplay`]),
    /// as `--screenshot` does; on the screen the layer shows it.
    override func draw(_ dirtyRect: NSRect) {
        NSColor(white: 0.125, alpha: 1).setFill()
        bounds.fill()
        guard let image else { return }
        let rect = CGRect(origin: .zero, size: skinLayer.frame.size)
        NSGraphicsContext.current?.imageInterpolation = .none
        NSImage(cgImage: image, size: rect.size).draw(
            in: rect, from: .zero, operation: .copy, fraction: 1, respectFlipped: true, hints: nil)
    }

    /// The mouse in layout pixels.
    private func point(_ event: NSEvent) -> (Int32, Int32) {
        let p = convert(event.locationInWindow, from: nil)
        let z = CGFloat(zoom)
        return (Int32((p.x / z).rounded(.down)), Int32((p.y / z).rounded(.down)))
    }

    override func mouseDown(with event: NSEvent) {
        guard let skin else { return }
        let (x, y) = point(event)
        pressed = true
        skin.pointerDown(x, y)
        if event.clickCount == 2 {
            skin.doubleClick(x, y)
        }
    }

    override func mouseDragged(with event: NSEvent) {
        guard let skin, pressed else { return }
        let (x, y) = point(event)
        skin.pointerMove(x, y)
    }

    override func mouseUp(with event: NSEvent) {
        guard let skin, pressed else { return }
        pressed = false
        let (x, y) = point(event)
        skin.pointerUp(x, y)
    }
}
