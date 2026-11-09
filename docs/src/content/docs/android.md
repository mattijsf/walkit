---
title: Android
---

The engine as an Android library: `nl.mattix.walkit:walkit`, one AAR with the native engine
(`libwalkit.so` for `arm64-v8a` and `x86_64`) and a Kotlin API in package `nl.mattix.walkit`.

## Building

```bash
cd android
./gradlew assembleRelease          # the AAR, native library included
./gradlew publishToMavenLocal      # the same into ~/.m2
```

`assembleRelease` runs `cargo ndk` itself, so the whole build is one Gradle call. It needs:

- a JDK 17 and the Android SDK with platform 36, build tools 36.1.0 and NDK 28.2.13676358;
- a Rust toolchain from rustup with the Android targets:
  `rustup target add aarch64-linux-android x86_64-linux-android`;
- `cargo install cargo-ndk`.

The AAR ships consumer ProGuard rules for the classes the native side reaches by name, so a
consumer's shrinker needs no extra rules.

## Using it

```kotlin
import nl.mattix.walkit.*

val files = LayeredSkinFiles(ZipSkinFiles(File("skin.wal")), defaultSet)   // the skin over the default set
val rt = SkinRuntime(files, host, player, SkinSetup(skinName = "Winamp Modern"))
rt.load()
if (rt.tick()) for ((rect, cmds) in rt.drawWindows()) draw(rect, cmds)
rt.close()                                   // frees the engine's side
```

The engine runs on the thread that calls it, one call at a time. It reads the skin's files, asks
the `SkinHost` and drives the `Player` through callbacks on that thread.

- The player is read once per call, as one snapshot. A player that sets
  `Player.playlistVersion` has its playlist read only when that number changes.
- Answers that stay fixed for a skin, such as a picture's size or a text's width, are asked
  once.
- A frame crosses the boundary once per `drawWindows`, as one packed buffer decoded into
  `DrawCmd`s.
- The fonts that stand in for Windows' system fonts (`fonts/LiberationSans.ttf` and others)
  are read from the skin's files, so a host puts its default set, fonts included, under the
  skin.

## Frames

`drawPlanned` returns each frame with a plan: which stretches of a window change from frame to
frame and can be kept apart, and a version for the rest.

Only the commands that changed since the previous frame cross the boundary. An unchanged command
is the same object as in the previous frame, so a host can reuse what it built from it.

## Free windows

`Presentation.freeWindows` lays the windows out apart instead of stacking them. A new window
opens at its container's `default_x`, `default_y`, and `SkinRuntime.placeWindow` puts one back
where the host kept it.

Windows move by a drag on their move areas and resize by their grip, and they dock and snap as
in Winamp 5.666.

## Test fixtures

`testImplementation(testFixtures("nl.mattix.walkit:walkit:<version>"))` adds the engine built
for the publishing machine, so a consumer's JVM unit tests can run skins. `SkinRuntime` loads
`libwalkit.so` on Android and falls back to that build on a JVM.

The fixtures also include `nl.mattix.walkit.fixtures`: helpers that do to a loaded skin what its
scripts could (resize a window, switch a layout, set an attribute, pop a menu, start a timer,
make a Config item, listen for events).
