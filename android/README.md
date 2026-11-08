# walkit on Android

The engine as an Android library: `nl.mattix.walkit:walkit`, one AAR with the native engine
(`libwalkit.so` for `arm64-v8a` and `x86_64`, built from `crates/walkit-android`) and the
Kotlin API a player drives it through (package `nl.mattix.walkit`). The Kotlin API mirrors the
`skin` package of Andamp's `:modernskin` module, so a player written against that module
changes its imports and little else.

## Building

```bash
cd android
./gradlew assembleRelease          # the AAR, native library included: walkit/build/outputs/aar/
./gradlew publishToMavenLocal      # the same into ~/.m2, as nl.mattix.walkit:walkit:0.5.16
./gradlew gate                     # formatting (ktlint through spotless) and the JVM unit tests
```

`Presentation.freeWindows` sets the windows out apart instead of stacking them: each window
where it is (`SkinRuntime.placeWindow` puts one where the host kept it; a new one opens at its
container's `default_x`, `default_y`), moved by a drag on its move areas and sized by its grip
through the pointer calls, docking and snapping as Winamp 5.666 does; the frame's window rects
say where each one is.

The AAR ships consumer ProGuard rules for what the native side reaches by name, so a
consumer's shrinker needs no extra rules. Its test fixtures
(`testImplementation(testFixtures("nl.mattix.walkit:walkit:0.5.16"))`) carry the engine built
for the machine that published them (`cargoBuildHost`, a plain `cargo build`), so a consumer's
JVM unit tests run skins on it: `SkinRuntime` loads `libwalkit.so` on Android and falls back to
that build on a JVM (`HostLibrary`). The fixtures also carry `nl.mattix.walkit.fixtures`, helpers
that do to a loaded skin what its scripts would (resize a window, switch a layout, set an
attribute, pop a menu, start a timer, make a Config item, hear events), for a host's tests.

`assembleRelease` runs `cargo ndk` itself (the `cargoBuild` task), so the whole build is one
Gradle call. On a fresh Mac it needs:

- a JDK 17 (Android Studio's, or `brew install openjdk@17`) and the Android SDK with platform
  36, build tools 36.1.0 and NDK 28.2.13676358 (Android Studio's SDK Manager installs them;
  `ANDROID_HOME` or `android/local.properties` with `sdk.dir=` points Gradle at the SDK);
- a Rust toolchain from rustup with the Android targets:
  `rustup target add aarch64-linux-android x86_64-linux-android`;
- `cargo install cargo-ndk`, which finds the NDK through `ANDROID_NDK_HOME` (the task sets it
  from the SDK's NDK folder).

The native library is built with the `release` profile (speed first; LTO, one codegen unit,
stripped). The Rust side of the gate (`tools/gate.sh` at the repository root) checks this crate
with clippy as part of the workspace; the Kotlin side's `gate` task is run from there too when
the SDK and `cargo-ndk` are present.

## Using it

```kotlin
import nl.mattix.walkit.*

val files = LayeredSkinFiles(ZipSkinFiles(File("skin.wal")), defaultSet)   // the skin over the default set
val rt = SkinRuntime(files, host, player, SkinSetup(skinName = "Winamp Modern"))
rt.load()
if (rt.tick()) for ((rect, cmds) in rt.drawWindows()) draw(rect, cmds)
rt.close()                                   // frees the engine's side
```

The engine runs on the thread that calls it, one call at a time, as the Kotlin engine does; it
reads the skin's files, asks the `SkinHost` and drives the `Player` through callbacks on that
thread. The player is read once per call, as one snapshot of its numbers (its playlist once per
change, when the player numbers it), and what a host answers for a skin for good (a picture's
size, a text's width) is asked once. A frame crosses
the boundary once per `drawWindows`, as one packed buffer decoded here into `DrawCmd`s. The stand-in fonts for Windows' system fonts (`fonts/LiberationSans.ttf` and
the like) are read from the skin's files, so a host layers its default set, with those fonts,
under the skin as Andamp does.

Every member of `SkinRuntime` is carried. The engine plans each frame as the reference's planner
does (`drawPlanned`): which stretches of a window change from frame to frame and may be kept apart,
and a version for the rest. Only the commands that changed since the frame before cross the
boundary; a command unchanged is the same object as in that frame, so a host may keep what it made
from it.

## The parity run on a device

`CorpusTraceTest` (an instrumented test) runs every reference scenario through the AAR and
writes the trace of each, the same text the reference engine's `EngineTraceTest` writes, for a
byte compare. The inputs are the ones `ENGINEERING.md` names, pushed to the device first:

```bash
ADB="adb -s emulator-5690"                                   # a probe emulator, never a phone
D=/data/local/tmp/walkit
$ADB shell mkdir -p $D/skins $D/small $D/fonts
$ADB push <andamp>/modernskin/src/test/resources/*.wal $D/skins/
$ADB push ~/Development/winamp-vm/skins/*.wal $D/small/
$ADB push ~/Development/winamp-vm/wasabi-defaults.zip ~/Development/winamp-vm/scenarios.json \
  ~/Development/winamp-vm/scenarios-small.json $D/
$ADB push <fonts>/*.ttf $D/fonts/
ANDROID_SERIAL=emulator-5690 ./gradlew :walkit:connectedAndroidTest \
  -Pandroid.injected.androidTest.leaveApksInstalledAfterRun=true   # the traces go with the APK otherwise
$ADB pull /sdcard/Android/data/nl.mattix.walkit.test/files/traces traces-device
$ADB uninstall nl.mattix.walkit.test
WALKIT_REFERENCE_TRACES=~/Development/winamp-vm/traces-kotlin ../tools/trace-diff.sh traces-device
```

The native binary can be checked on the device as well, without the AAR: `cargo ndk -t
arm64-v8a build -p walkit-trace --release`, push `target/aarch64-linux-android/release/walkit-trace`
beside the inputs, run it there as on a desktop, and pull `traces/`.
