// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import SwiftUI

/// A small macOS app that shows a Winamp skin with the walkit engine.
@main
struct WalkitExampleApp: App {
    @NSApplicationDelegateAdaptor private var delegate: AppDelegate

    init() {
        // the app reads its own arguments (ContentView): AppKit is not to open a skin file named
        // there as a document, which would leave the window shut
        UserDefaults.standard.register(defaults: ["NSTreatUnknownArgumentsAsOpen": "NO"])
    }

    var body: some Scene {
        Window("walkit", id: "skin") {
            ContentView()
        }
        .windowResizability(.contentSize)
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        // a line at a time, also into a pipe
        setvbuf(stdout, nil, _IOLBF, 0)
        // started from a terminal, the app comes to the front all the same
        NSApp.setActivationPolicy(.regular)
        NSApp.activate()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ app: NSApplication) -> Bool { true }
}
