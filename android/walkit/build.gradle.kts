// SPDX-License-Identifier: GPL-3.0-or-later

import org.jetbrains.kotlin.gradle.dsl.ExplicitApiMode

plugins {
    alias(libs.plugins.android.library)
    `maven-publish`
}

group = "nl.mattix.walkit"
version = "0.8.0"

// r27+ is the floor for 16 KB page alignment, required from Android 15
val ndkVersionUsed = "28.2.13676358"

android {
    namespace = "nl.mattix.walkit"
    compileSdk = 36
    buildToolsVersion = "36.1.0"
    ndkVersion = ndkVersionUsed

    defaultConfig {
        minSdk = 26
        // what a consumer's shrinker keeps: the JNI methods and the bridge objects the native side calls by name
        consumerProguardFiles("consumer-rules.pro")
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

    // the test fixtures carry the engine built for the machine, so a host's JVM unit tests run the engine
    testFixtures { enable = true }

    publishing {
        singleVariant("release") {
            withSourcesJar()
        }
    }
}

// The engine itself: `cargo ndk` builds crates/walkit-android (the JNI library) for each ABI
// with the release profile and drops `libwalkit.so` where the AAR picks it up. It needs the
// Rust targets for Android and cargo-ndk (see README.md). The cargo from rustup is called by
// its path (`~/.cargo/bin`), so it is found from Android Studio as well and another cargo
// earlier on the path (Homebrew's) does not pick a different toolchain.
val cargoHome = File(System.getProperty("user.home"), ".cargo/bin")
val cargo = cargoHome.resolve("cargo").takeIf { it.canExecute() }?.absolutePath ?: "cargo"
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
        environment("PATH", cargoHome.absolutePath + ":" + System.getenv("PATH"))
        // the NDK of the SDK this build uses, unless the environment names one
        val sdk = System.getenv("ANDROID_HOME") ?: "$home/Library/Android/sdk"
        environment("ANDROID_NDK_HOME", System.getenv("ANDROID_NDK_HOME") ?: "$sdk/ndk/$ndkVersionUsed")
        commandLine(
            cargo,
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

// The engine for the machine this builds on (a plain `cargo build`, the release profile), for
// the test fixtures: a host's JVM unit tests load it through HostLibrary when there is no
// Android. Named by the JVM's os and cpu, as HostLibrary looks it up.
val hostOs = System.getProperty("os.name").lowercase()
val hostFolder =
    when {
        hostOs.startsWith("mac") -> "darwin"
        hostOs.startsWith("linux") -> "linux"
        else -> error("no host build of the engine for $hostOs")
    }
val hostCpu =
    when (val arch = System.getProperty("os.arch").lowercase()) {
        "aarch64", "arm64" -> "aarch64"
        "amd64", "x86_64" -> "x86_64"
        else -> error("no host build of the engine for $arch")
    }
val hostExtension = if (hostFolder == "darwin") "dylib" else "so"
val cargoBuildHost =
    tasks.register<Exec>("cargoBuildHost") {
        group = "build"
        description = "Builds the engine for this machine with cargo, for the test fixtures."
        val repo = rootDir.parentFile
        workingDir = repo
        inputs.dir(repo.resolve("crates"))
        inputs.file(repo.resolve("Cargo.toml"))
        inputs.file(repo.resolve("Cargo.lock"))
        outputs.file(repo.resolve("target/release/libwalkit.$hostExtension"))
        environment("PATH", cargoHome.absolutePath + ":" + System.getenv("PATH"))
        commandLine(cargo, "build", "-p", "walkit-android", "--release")
    }
// the resources of the test fixtures; not committed (see .gitignore)
val hostLibrary =
    tasks.register<Copy>("hostLibrary") {
        group = "build"
        description = "Puts the engine built for this machine where the test fixtures carry it."
        from(cargoBuildHost)
        into(layout.projectDirectory.dir("src/testFixtures/resources/nl/mattix/walkit/host/$hostFolder-$hostCpu"))
    }
tasks.named("preBuild") { dependsOn(hostLibrary) }

dependencies {
    testImplementation(libs.junit)
    // the JVM tests run the engine through the fixtures' host build
    testImplementation(testFixtures(project(":walkit")))
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
