# walkit on Android

The engine as an Android library: `nl.mattix.walkit:walkit`, one AAR with the native engine
(`libwalkit.so` for `arm64-v8a` and `x86_64`, built from `crates/walkit-android`) and a Kotlin
API in package `nl.mattix.walkit`.

## Building

```bash
cd android
./gradlew assembleRelease          # the AAR, native library included: walkit/build/outputs/aar/
./gradlew publishToMavenLocal      # the same into ~/.m2, as nl.mattix.walkit:walkit:0.6.0
./gradlew gate                     # formatting (ktlint through spotless) and the JVM unit tests
```

`assembleRelease` runs `cargo ndk` itself (the `cargoBuild` task), so the whole build is one
Gradle call. On a fresh Mac it needs:

- a JDK 17 (Android Studio's, or `brew install openjdk@17`) and the Android SDK with platform
  36, build tools 36.1.0 and NDK 28.2.13676358 (Android Studio's SDK Manager installs them;
  `ANDROID_HOME` or `android/local.properties` with `sdk.dir=` points Gradle at the SDK);
- a Rust toolchain from rustup with the Android targets:
  `rustup target add aarch64-linux-android x86_64-linux-android`;
- `cargo install cargo-ndk`, which finds the NDK through `ANDROID_NDK_HOME` (the task sets it
  from the SDK's NDK folder).

The native library uses the `release` profile: LTO, one codegen unit, stripped. `tools/gate.sh`
at the repository root runs clippy on this crate with the rest of the workspace, and runs the
Gradle `gate` task when the SDK and `cargo-ndk` are present.

The AAR ships consumer ProGuard rules for the classes the native side reaches by name, so a
consumer's shrinker needs no extra rules.

## Test fixtures

`testImplementation(testFixtures("nl.mattix.walkit:walkit:0.6.0"))` adds the engine built for
the publishing machine (the `cargoBuildHost` task, a plain `cargo build`), so a consumer's JVM
unit tests can run skins.

`SkinRuntime` loads `libwalkit.so` on Android and falls back to that build on a JVM
(`HostLibrary`).

The fixtures also include `nl.mattix.walkit.fixtures`: helpers that do to a loaded skin what its
scripts could (resize a window, switch a layout, set an attribute, pop a menu, start a timer,
make a Config item, listen for events).

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

`drawPlanned` returns each frame with a plan: which stretches of a window change from frame to
frame and can be kept apart, and a version for the rest.

Only the commands that changed since the previous frame cross the boundary. An unchanged command
is the same object as in the previous frame, so a host can reuse what it built from it.

`Presentation.freeWindows` lays the windows out apart instead of stacking them. A new window
opens at its container's `default_x`, `default_y`, and `SkinRuntime.placeWindow` puts one back
where the host kept it.

Windows move by a drag on their move areas and resize by their grip, and they dock and snap as
in Winamp 5.666. The frame's window rects say where each one is.

## The trace run on a device

`CorpusTraceTest`, an instrumented test, runs every reference scenario through the AAR and
writes a trace of each, to compare byte for byte with the traces the same commit writes on a
desktop.
Push the inputs [ENGINEERING.md](../ENGINEERING.md) describes to the device first:

```bash
ADB="adb -s emulator-5690"                                   # an emulator
D=/data/local/tmp/walkit
$ADB shell mkdir -p $D/skins $D/small $D/fonts
$ADB push <corpus skins>/*.wal $D/skins/
$ADB push <small skins>/*.wal $D/small/
$ADB push <wasabi-defaults.zip> <scenarios.json> <scenarios-small.json> $D/
$ADB push <fonts>/*.ttf $D/fonts/
ANDROID_SERIAL=emulator-5690 ./gradlew :walkit:connectedAndroidTest \
  -Pandroid.injected.androidTest.leaveApksInstalledAfterRun=true   # keeps the APK and its traces
$ADB pull /sdcard/Android/data/nl.mattix.walkit.test/files/traces traces-device
$ADB uninstall nl.mattix.walkit.test
WALKIT_REFERENCE_TRACES=<desktop traces> ../tools/trace-diff.sh traces-device
```

To check the native binary on the device without the AAR, build it with `cargo ndk -t arm64-v8a
build -p walkit-trace --release`, push `target/aarch64-linux-android/release/walkit-trace`
beside the inputs, run it there as on a desktop, and pull `traces/`.
