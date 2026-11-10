// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import SwiftUI
import UniformTypeIdentifiers

/// The buttons and the skin under them. The window opens at the skin's size and resizes; it
/// never gets smaller than the skin.
struct ContentView: View {
    @State private var library = SkinLibrary()
    @State private var size = CGSize(width: 275, height: 116)
    @State private var picking: Pick?

    /// What the file picker is open for.
    private enum Pick { case skin, wasabi }

    private static let skinTypes = ["wsz", "wal", "zip"].compactMap { UTType(filenameExtension: $0) }

    var body: some View {
        VStack(spacing: 0) {
            buttons
            Divider()
            SkinView(skin: library.skin, size: $size)
                .frame(
                    minWidth: size.width, idealWidth: size.width, maxWidth: .infinity,
                    minHeight: size.height, idealHeight: size.height, maxHeight: .infinity)
                .overlay {
                    if library.skin == nil {
                        Text(library.loading ? "Loading a skin" : "No skin").foregroundStyle(.secondary)
                    }
                }
                .onDrop(of: [.fileURL], isTargeted: nil) { providers in
                    _ = providers.first?.loadObject(ofClass: URL.self) { url, _ in
                        if let url { Task { @MainActor in library.open(file: url) } }
                    }
                    return true
                }
            if let message = library.message {
                Text(message)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .padding(6)
                    .frame(maxWidth: .infinity)
            }
        }
        .navigationTitle(library.skin?.name ?? "walkit")
        .fileImporter(
            isPresented: Binding(get: { picking != nil }, set: { if !$0 { picking = nil } }),
            allowedContentTypes: picking == .wasabi ? [.zip] : Self.skinTypes
        ) { result in
            guard case .success(let url) = result else { return }
            switch picking {
            case .wasabi: library.setWasabi(file: url)
            default: library.open(file: url)
            }
        }
        .task { await openFromArguments() }
    }

    private var buttons: some View {
        // two rows, so the bar fits a classic skin's 275 points
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Button("Winamp classic") { library.openWinampClassic() }
                Button("Winamp Modern") { library.openWinampModern() }
                Button("walkit classic") { library.openBuiltin(modern: false) }
                Button("walkit modern") { library.openBuiltin(modern: true) }
            }
            HStack(spacing: 6) {
                Button("Open skin…") { picking = .skin }
                Button("Wasabi base…") { picking = .wasabi }
                if let name = library.wasabiName {
                    Text(name).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
            }
        }
        .controlSize(.small)
        .padding(8)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// The skin the command line names: a skin file, `--modern`, `--builtin` (the original
    /// skin of the kind) and `--wasabi <zip>`; Winamp's base skin with none.
    /// `--screenshot <out.png>` writes the window as a PNG once the skin shows, and quits.
    private func openFromArguments() async {
        var args = CommandLine.arguments.dropFirst()
        var modern = false
        var builtin = false
        var file: URL?
        var screenshot: URL?
        while let arg = args.popFirst() {
            switch arg {
            case "--modern": modern = true
            case "--builtin": builtin = true
            case "--screenshot": screenshot = args.popFirst().map { URL(fileURLWithPath: $0) }
            case "--wasabi":
                if let path = args.popFirst() { library.setWasabi(file: URL(fileURLWithPath: path)) }
            // the arguments Xcode and macOS add, such as -NSDocumentRevisionsDebugMode YES
            case _ where arg.hasPrefix("-") && !arg.hasPrefix("--"):
                args = args.dropFirst()
            default: file = URL(fileURLWithPath: arg)
            }
        }
        if let file {
            library.open(file: file)
        } else if builtin {
            library.openBuiltin(modern: modern)
        } else if modern {
            library.openWinampModern()
        } else {
            library.openWinampClassic()
        }
        if let screenshot {
            await writeScreenshot(to: screenshot)
        }
    }

    /// Waits for the skin to show and run a second, writes the window to `file`, and quits.
    private func writeScreenshot(to file: URL) async {
        try? await Task.sleep(for: .milliseconds(200))
        while library.loading {
            try? await Task.sleep(for: .milliseconds(100))
        }
        try? await Task.sleep(for: .seconds(1))
        guard let window = NSApp.windows.first(where: { $0.isVisible }),
            let view = window.contentView?.superview ?? window.contentView,
            let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds)
        else { return }
        view.cacheDisplay(in: view.bounds, to: rep)
        do {
            try rep.representation(using: .png, properties: [:])?.write(to: file)
        } catch {
            print("\(file.path): \(error.localizedDescription)")
        }
        NSApp.terminate(nil)
    }
}
