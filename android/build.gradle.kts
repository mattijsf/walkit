// SPDX-License-Identifier: GPL-3.0-or-later

plugins {
    alias(libs.plugins.android.library) apply false
    alias(libs.plugins.spotless)
}

spotless {
    val ktlintVersion = libs.versions.ktlint.get()
    kotlin {
        target("**/src/**/*.kt")
        targetExclude("**/build/**")
        ktlint(ktlintVersion)
    }
    kotlinGradle {
        target("**/*.gradle.kts")
        targetExclude("**/build/**")
        ktlint(ktlintVersion)
    }
}

// What a push has to pass on the Kotlin side, without a device: the formatting and the JVM unit
// tests. The instrumented parity test runs on an emulator (see README.md).
tasks.register("gate") {
    group = "verification"
    description = "Format check and the JVM unit tests: what a push has to pass without a device."
    dependsOn("spotlessCheck", ":walkit:testDebugUnitTest")
}
