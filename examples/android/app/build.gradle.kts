// SPDX-License-Identifier: GPL-3.0-or-later

plugins {
    id("com.android.application")
}

android {
    namespace = "nl.mattix.walkit.example"
    compileSdk = 36
    buildToolsVersion = "36.1.0"
    ndkVersion = "28.2.13676358"

    defaultConfig {
        applicationId = "nl.mattix.walkit.example"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    // built from ../../android through the includeBuild in settings.gradle.kts
    implementation("nl.mattix.walkit:walkit:0.7.3")
}
