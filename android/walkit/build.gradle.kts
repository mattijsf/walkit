// SPDX-License-Identifier: GPL-3.0-or-later

import org.jetbrains.kotlin.gradle.dsl.ExplicitApiMode

plugins {
    alias(libs.plugins.android.library)
    `maven-publish`
}

group = "nl.mattix.walkit"
version = "0.1.3"

// r27+ is the floor for 16 KB page alignment, required from Android 15
val ndkVersionUsed = "28.2.13676358"

android {
    namespace = "nl.mattix.walkit"
    compileSdk = 36
    buildToolsVersion = "36.1.0"
    ndkVersion = ndkVersionUsed

    defaultConfig {
        minSdk = 26
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        // arm64 is the first-class target; x86_64 keeps an Intel emulator usable
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlin {
        explicitApi = ExplicitApiMode.Strict
    }

    publishing {
        singleVariant("release") {
            withSourcesJar()
        }
    }
}

// The engine itself: `cargo ndk` builds crates/walkit-android (the JNI library) for each ABI
// with the release profile and drops `libwalkit.so` where the AAR picks it up. It needs the
// Rust targets for Android and cargo-ndk (see README.md); `~/.cargo/bin` is put on the path
// so a cargo from rustup is found from Android Studio as well.
// the default jniLibs folder of the main source set; not committed (see .gitignore)
val rustJniLibs = layout.projectDirectory.dir("src/main/jniLibs")
val cargoBuild =
    tasks.register<Exec>("cargoBuild") {
        group = "build"
        description = "Builds libwalkit.so for every ABI with cargo ndk."
        val repo = rootDir.parentFile
        workingDir = repo
        inputs.dir(repo.resolve("crates"))
        inputs.file(repo.resolve("Cargo.toml"))
        inputs.file(repo.resolve("Cargo.lock"))
        outputs.dir(rustJniLibs)
        val home = System.getProperty("user.home")
        environment("PATH", "$home/.cargo/bin:" + System.getenv("PATH"))
        // the NDK of the SDK this build uses, unless the environment names one
        val sdk = System.getenv("ANDROID_HOME") ?: "$home/Library/Android/sdk"
        environment("ANDROID_NDK_HOME", System.getenv("ANDROID_NDK_HOME") ?: "$sdk/ndk/$ndkVersionUsed")
        commandLine(
            "cargo",
            "ndk",
            "-t",
            "arm64-v8a",
            "-t",
            "x86_64",
            "-o",
            rustJniLibs.asFile.absolutePath,
            "build",
            "-p",
            "walkit-android",
            "--release",
        )
    }
tasks.named("preBuild") { dependsOn(cargoBuild) }

dependencies {
    testImplementation(libs.junit)
    androidTestImplementation(libs.junit)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.ext.junit)
}

publishing {
    publications {
        register<MavenPublication>("release") {
            groupId = "nl.mattix.walkit"
            artifactId = "walkit"
            version = project.version.toString()
            afterEvaluate { from(components["release"]) }
            pom {
                name.set("walkit")
                description.set("The walkit modern Winamp skin engine for Android: the native engine and its Kotlin API.")
                url.set("https://github.com/mattijsf/walkit")
                licenses {
                    license {
                        name.set("GPL-3.0-or-later")
                        url.set("https://www.gnu.org/licenses/gpl-3.0.txt")
                    }
                }
            }
        }
    }
}
