# walkit Android example

A small app that draws a Winamp skin with the walkit AAR. "Winamp classic" and "Winamp Modern"
download Winamp's own skins from the Winamp Skin Museum; when a download fails, the app shows
the original skin of the same kind from [`../skins`](../skins), packed into its assets. The
classic one opens on the first start; "walkit classic" and "walkit modern" open the original
skins directly. The stand-in fonts in `../skins/fonts` sit under every skin, read from the
assets. There is also an "Open skin" button for a `.wsz`
(classic) or `.wal` (modern) skin, a "Wasabi base" button for a zip of Winamp's
`wasabi` folder, which some modern skins draw lists and frames from (classic skins never do;
[how to make the zip](https://mattijsf.github.io/walkit/wasabi-base/)), and a view that runs and draws the skin. The skin opened last opens again on
start.

- `MainActivity.kt`: the buttons, the picks, and the skin's files (`ZipSkinFiles`, with
  `LayeredSkinFiles` for the wasabi base).
- `SkinView.kt`: the `SkinRuntime`, the `SkinHost` (pictures through `BitmapFactory`, text
  widths through `Paint`, the log to Logcat), a tick on every display frame, every `DrawCmd` on a
  `Canvas`, and touches to the engine.

## Building

The app builds the library from `../../android` (an `includeBuild` in `settings.gradle.kts`),
so it needs what that build needs: a JDK 17, the Android SDK with platform 36, build tools
36.1.0 and NDK 28.2.13676358, and Rust with cargo-ndk (see `android/README.md`).

```bash
cd examples/android
./gradlew assembleDebug            # app/build/outputs/apk/debug/app-debug.apk
./gradlew installDebug             # onto the one device attached
```

`ANDROID_HOME`, or a `local.properties` with `sdk.dir=`, points Gradle at the SDK.

## Opening a skin without the picker

A debug build imports files from its external files folder, named on `am start`:

```bash
D=/sdcard/Android/data/nl.mattix.walkit.example/files   # made on the app's first start
adb push skin.wal wasabi.zip $D/
adb shell am start -S -n nl.mattix.walkit.example/.MainActivity --es skin skin.wal --es wasabi wasabi.zip
```
