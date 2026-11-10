// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import SwiftUI

/// Runs a skin and shows it: a tick on every display frame, a new frame when the skin asks for
/// one, and the mouse passed back to the engine. One point is one layout pixel.
struct SkinView: NSViewRepresentable {
    let skin: Skin?
    /// The size the skin's windows take, in points: the view asks for it.
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

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.backgroundColor = NSColor(white: 0.125, alpha: 1).cgColor
        layer?.contentsGravity = .topLeft
        layer?.magnificationFilter = .nearest
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

    /// Once per display frame: the engine runs its timers and animations, and the view draws
    /// again when something changed.
    @objc private func frame(_ link: CADisplayLink) {
        guard let skin else {
            image = nil
            layer?.contents = nil
            return
        }
        let changed = skin.tick()
        guard changed || dirty else { return }
        dirty = false
        let scale = max(1, Int((window?.backingScaleFactor ?? 2).rounded()))
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        image = skin.draw(scale: scale)
        layer?.contents = image
        layer?.contentsScale = CGFloat(scale)
        CATransaction.commit()
        let size = skin.size
        if size != shownSize {
            shownSize = size
            onResize?(size)
        }
    }

    /// Draws the frame shown when the view is drawn into an image ([`NSView.cacheDisplay`]),
    /// as `--screenshot` does; on the screen the layer shows it.
    override func draw(_ dirtyRect: NSRect) {
        guard let image else { return }
        NSGraphicsContext.current?.imageInterpolation = .none
        NSImage(cgImage: image, size: bounds.size).draw(
            in: bounds, from: .zero, operation: .copy, fraction: 1, respectFlipped: true, hints: nil)
    }

    /// The mouse in layout pixels.
    private func point(_ event: NSEvent) -> (Int32, Int32) {
        let p = convert(event.locationInWindow, from: nil)
        return (Int32(p.x.rounded(.down)), Int32(p.y.rounded(.down)))
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
