// SPDX-License-Identifier: GPL-3.0-or-later

import Foundation
import Observation

/// The skins the app shows, and what lies under them: Winamp's base skin under every classic
/// skin, the wasabi base under a modern one, and the stand-in fonts under both.
@MainActor
@Observable
final class SkinLibrary {
    /// One of Winamp's own skins in the Winamp Skin Museum, and the original skin of the same
    /// kind, built into the app, shown when the download fails.
    struct Remote {
        let url: URL
        let name: String
        let fallback: String

        /// The built-in skin's bytes and file name.
        func builtin() throws -> (Data, String) {
            guard let url = Bundle.main.url(forResource: fallback, withExtension: nil) else {
                throw Skin.OpenError(message: "\(fallback) is not in the app")
            }
            return (try Data(contentsOf: url), fallback)
        }

        /// The skin's bytes and file name: downloaded, or the built-in fallback.
        func fetch() async throws -> (Data, String) {
            var request = URLRequest(url: url)
            request.timeoutInterval = 10
            do {
                let (data, response) = try await URLSession.shared.data(for: request)
                if let http = response as? HTTPURLResponse, http.statusCode != 200 {
                    throw URLError(.badServerResponse)
                }
                return (data, name)
            } catch {
                print("\(url): \(error.localizedDescription); showing \(fallback)")
                return try builtin()
            }
        }
    }

    static let classic = Remote(
        url: URL(string: "https://r2.webampskins.org/skins/5e4f10275dcb1fb211d4a8b4f1bda236.wsz")!,
        name: "Winamp Base.wsz",
        fallback: "walkit-classic.wsz")
    static let modern = Remote(
        url: URL(string: "https://r2.webampskins.org/skins/b3fa3160814d1689a3f2bacda72b2926.wal")!,
        name: "Winamp Modern.wal",
        fallback: "walkit-modern.wal")

    /// The skin shown, and what went wrong last.
    private(set) var skin: Skin?
    private(set) var message: String?
    private(set) var loading = false

    /// A zip of Winamp's wasabi folder, laid under modern skins; its file name.
    private(set) var wasabiName: String?
    private var wasabi: Data?

    /// The archive of the skin shown and its file name.
    private var shown: (Data, String)?

    /// Winamp's base skin (or the built-in classic skin when the download fails), fetched once.
    private var classicBase: Task<(Data, String), Error>?

    /// The folder of the free fonts that stand in for Arial and Tahoma.
    private let fonts = Bundle.main.url(forResource: "fonts", withExtension: nil)

    private func baseSkin() async throws -> (Data, String) {
        if classicBase == nil {
            classicBase = Task { try await Self.classic.fetch() }
        }
        return try await classicBase!.value
    }

    /// Winamp's base skin, which also lies under every classic skin.
    func openWinampClassic() {
        load { try await self.baseSkin() }
    }

    func openWinampModern() {
        load { try await Self.modern.fetch() }
    }

    /// The original skin of the kind, without trying the download.
    func openBuiltin(modern: Bool) {
        load { try (modern ? Self.modern : Self.classic).builtin() }
    }

    /// A skin file of the user's.
    func open(file: URL) {
        load {
            let data = try Self.read(file)
            return (data, file.lastPathComponent)
        }
    }

    /// Takes a zip of Winamp's wasabi folder for modern skins, and opens the skin shown again
    /// over it.
    func setWasabi(file: URL) {
        do {
            wasabi = try Self.read(file)
            wasabiName = file.lastPathComponent
            message = nil
            if let shown { load { shown } }
        } catch {
            message = "\(file.lastPathComponent): \(error.localizedDescription)"
        }
    }

    private static func read(_ file: URL) throws -> Data {
        let scoped = file.startAccessingSecurityScopedResource()
        defer { if scoped { file.stopAccessingSecurityScopedResource() } }
        return try Data(contentsOf: file)
    }

    /// Shows the skin `source` gives in place of the one shown.
    private func load(_ source: @escaping () async throws -> (Data, String)) {
        loading = true
        Task {
            defer { loading = false }
            do {
                let (bytes, name) = try await source()
                let base = try? await baseSkin().0
                skin = try Skin(bytes: bytes, name: name, base: base, wasabi: wasabi, fonts: fonts)
                shown = (bytes, name)
                message = nil
            } catch {
                message = error.localizedDescription
            }
        }
    }
}
